use crate::{plan::LockedRun, queue::read_json};
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{self, ProcessResult},
    store::{hash_file, ObjectId, Store, MAX_CHUNK},
    worker::{self, Request},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutorIdentity {
    pub kind: String,
    pub build: ObjectId,
    pub policy: Value,
    pub tools: BTreeMap<String, ObjectId>,
}
pub trait Executor {
    fn identity(&mut self) -> Result<ExecutorIdentity>;
    fn run(
        &mut self,
        store: &Store,
        run: &LockedRun,
        cancel: Arc<AtomicBool>,
        out: &Path,
    ) -> Result<ProcessResult>;
    fn recover(&mut self, _out: &Path) -> Result<()> {
        Ok(())
    }
}
pub struct LocalExecutor;
impl Executor for LocalExecutor {
    fn identity(&mut self) -> Result<ExecutorIdentity> {
        Ok(ExecutorIdentity {
            kind: "local-isolated-process".into(),
            build: hash_file(&std::env::current_exe()?)?,
            policy: json!({"runtime":"process-spec/2","transport":"none"}),
            tools: local_tools()?,
        })
    }
    fn run(
        &mut self,
        store: &Store,
        run: &LockedRun,
        cancel: Arc<AtomicBool>,
        out: &Path,
    ) -> Result<ProcessResult> {
        ensure!(
            local_tools()? == run.backend_tools,
            "local isolation backend differs from locked plan"
        );
        isolation::execute_dashboard_recorded(
            store,
            run.process
                .as_ref()
                .context("no executable specification")?,
            cancel,
            Some(isolation::TraceFiles {
                stdout: File::create_new(out.join("local-stdout.log"))?,
                stderr: File::create_new(out.join("local-stderr.log"))?,
            }),
            None,
            None,
            Some(&out.join("dashboard.json")),
        )
    }
}
pub fn local_tools() -> Result<BTreeMap<String, ObjectId>> {
    Ok(BTreeMap::from([
        ("bubblewrap".into(), hash_file(Path::new("/usr/bin/bwrap"))?),
        ("unshare".into(), hash_file(Path::new("/usr/bin/unshare"))?),
    ]))
}

/// Implementations must describe the actual transport. A private inherited
/// descriptor is process IPC; it does not establish TCP, SSH or an external host.
pub trait WorkerTransport {
    fn request(&mut self, request: &Request) -> Result<Value>;
    fn description(&self) -> String;
}
pub struct SocketTransport(pub PathBuf);
impl WorkerTransport for SocketTransport {
    fn request(&mut self, request: &Request) -> Result<Value> {
        worker::request(&self.0, request)
    }
    fn description(&self) -> String {
        format!("Unix listener {}", self.0.display())
    }
}
/// An owned connected channel. An uncertain exchange closes it permanently.
/// A complete validated worker error permits a later independent request.
pub struct ConnectedTransport {
    stream: Option<std::os::unix::net::UnixStream>,
}
impl ConnectedTransport {
    /// The caller must not send requests through another copy of this stream.
    /// Recovery after an uncertain exchange requires a fresh channel and token.
    pub fn new(stream: std::os::unix::net::UnixStream) -> Self {
        Self {
            stream: Some(stream),
        }
    }
}
impl WorkerTransport for ConnectedTransport {
    fn request(&mut self, request: &Request) -> Result<Value> {
        let mut stream = self
            .stream
            .take()
            .context("worker connected channel is closed after a failed exchange")?;
        let result = worker::request_stream(&mut stream, request);
        let complete_error = result
            .as_ref()
            .err()
            .is_some_and(|error| error.is::<worker::CompletedWorkerError>());
        if result.is_ok() || complete_error {
            self.stream = Some(stream);
            result
        } else {
            result.context("worker connected exchange failed; channel closed")
        }
    }
    fn description(&self) -> String {
        "private inherited connected process IPC; no listener or external host".into()
    }
}

/// A real worker subprocess with private connected IPC. This is an isolated
/// deployment test on the same host, not an external physical-host claim.
pub struct PrivateTransport {
    channel: ConnectedTransport,
    deployment: String,
}
impl WorkerTransport for PrivateTransport {
    fn request(&mut self, request: &Request) -> Result<Value> {
        self.channel.request(request)
    }
    fn description(&self) -> String {
        self.deployment.clone()
    }
}
pub struct PrivateWorker {
    pub executor: WorkerExecutor<PrivateTransport>,
    child: std::process::Child,
}
impl PrivateWorker {
    pub fn spawn(program: &Path, root: &Path) -> Result<Self> {
        use std::os::{fd::AsRawFd, unix::process::CommandExt};
        fs::create_dir_all(root)?;
        ensure!(
            fs::symlink_metadata(root)?.is_dir(),
            "worker root must be a real directory"
        );
        let root = root.canonicalize()?;
        let (client, server) = std::os::unix::net::UnixStream::pair()?;
        let fd = server.as_raw_fd();
        let parent = std::process::id() as libc::pid_t;
        let mut command = std::process::Command::new(program);
        command
            .arg(&root)
            .arg(fd.to_string())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                    || libc::getppid() != parent
                    || libc::fcntl(fd, libc::F_SETFD, 0) < 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn()?;
        drop(server);
        Ok(Self {
            executor: WorkerExecutor {
                transport: PrivateTransport { channel: ConnectedTransport::new(client), deployment: format!("private inherited connected IPC; same host worker root {}; no listener or external host", root.display()) },
            },
            child,
        })
    }
}
impl Drop for PrivateWorker {
    fn drop(&mut self) {
        let _ = self.executor.transport.request(&Request::Shutdown);
        let start = Instant::now();
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if start.elapsed() < Duration::from_secs(2) => {
                    thread::sleep(Duration::from_millis(20))
                }
                _ => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    break;
                }
            }
        }
    }
}
pub struct WorkerExecutor<T: WorkerTransport> {
    pub transport: T,
}

/// Every external retention acknowledgment must bind all of these identities.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RetentionIdentity {
    pub locked_run_hash: String,
    pub token: String,
    pub run_id: String,
    pub result: ObjectId,
}

/// The exact result is verified and synchronized before the callback runs.
pub struct RetentionRequest {
    pub identity: RetentionIdentity,
    pub result_path: PathBuf,
}

type Retainer<'a> = dyn FnMut(&RetentionRequest) -> Result<RetentionIdentity> + 'a;

fn locked_hash(saved: &Value) -> Result<String> {
    let hash = saved["locked_run_hash"]
        .as_str()
        .context("external retention requires a saved locked-run hash")?;
    ObjectId {
        sha256: hash.into(),
        size: 0,
    }
    .validate()?;
    Ok(hash.into())
}

fn retain_result(out: &Path, id: &str, result: &ObjectId, retain: &mut Retainer<'_>) -> Result<()> {
    let saved: Value = read_json(&out.join("worker-request.json"), 65536)?;
    let token = saved["token"]
        .as_str()
        .context("external retention requires a saved execution token")?;
    ObjectId {
        sha256: token.into(),
        size: 0,
    }
    .validate()?;
    ensure!(
        id == format!("run-{token}"),
        "retention run and token differ"
    );
    result.validate()?;
    let request = RetentionRequest {
        identity: RetentionIdentity {
            locked_run_hash: locked_hash(&saved)?,
            token: token.into(),
            run_id: id.into(),
            result: result.clone(),
        },
        result_path: out.join("worker-result.json"),
    };
    ensure!(
        hash_file(&request.result_path)? == *result,
        "result changed before external retention"
    );
    let receipt = retain(&request)?;
    ensure!(
        receipt == request.identity,
        "external retention acknowledgment differs"
    );
    isolation::save_json(&out.join("worker-retention.json"), &receipt)?;
    File::open(out)?.sync_all()?;
    Ok(())
}
/// Transfer a verified immutable input through the common worker protocol.
pub fn transfer_object(
    transport: &mut impl WorkerTransport,
    store: &Store,
    id: &ObjectId,
    cancel: &AtomicBool,
) -> Result<()> {
    let mut file = File::open(store.verified_path(id)?)?;
    let mut offset = transport.request(&Request::Begin { object: id.clone() })?["offset"]
        .as_u64()
        .context("worker offset missing")?;
    ensure!(offset <= id.size, "worker offset exceeds object size");
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0; MAX_CHUNK];
    while offset < id.size {
        ensure!(
            !cancel.load(Ordering::Acquire),
            "cancelled during artifact transfer"
        );
        let n = file.read(&mut bytes)?;
        ensure!(n > 0, "source ended before its declared size");
        let next = offset
            .checked_add(n as u64)
            .context("transfer offset overflow")?;
        let response = transport.request(&Request::Chunk {
            object: id.clone(),
            offset,
            hex: hex::encode(&bytes[..n]),
        })?;
        ensure!(
            response["offset"].as_u64() == Some(next),
            "worker acknowledged a different offset"
        );
        offset = next;
    }
    let result = transport.request(&Request::Finish { object: id.clone() })?;
    ensure!(
        serde_json::from_value::<ObjectId>(result["verified"].clone())? == *id,
        "destination identity differs"
    );
    Ok(())
}

impl<T: WorkerTransport> WorkerExecutor<T> {
    fn collect(
        &mut self,
        id: &str,
        out: &Path,
        max_bytes: u64,
        trace_bound: u64,
        retain: Option<&mut Retainer<'_>>,
    ) -> Result<ProcessResult> {
        let summary = self
            .transport
            .request(&Request::Result { run_id: id.into() })?;
        let object: ObjectId = serde_json::from_value(summary["result_identity"].clone())?;
        object.validate()?;
        ensure!(
            object.size <= max_bytes,
            "worker result exceeds locked budget"
        );
        let path = out.join("worker-result.json");
        let mut file = File::options().write(true).create_new(true).open(&path)?;
        let mut offset = 0;
        while offset < object.size {
            let response = self.transport.request(&Request::ResultData {
                run_id: id.into(),
                result: object.clone(),
                offset,
                count: MAX_CHUNK,
            })?;
            let bytes = hex::decode(
                response["hex"]
                    .as_str()
                    .context("missing worker result chunk")?,
            )?;
            ensure!(
                !bytes.is_empty() && bytes.len() <= MAX_CHUNK,
                "invalid worker result chunk"
            );
            let next = offset + bytes.len() as u64;
            ensure!(
                next <= object.size
                    && response["offset"].as_u64() == Some(offset)
                    && response["next_offset"].as_u64() == Some(next)
                    && serde_json::from_value::<ObjectId>(response["result_identity"].clone())?
                        == object,
                "worker result range differs"
            );
            file.write_all(&bytes)?;
            offset = next;
        }
        file.sync_all()?;
        ensure!(
            hash_file(&path)? == object,
            "retrieved worker result hash differs"
        );
        let result: ProcessResult = read_json(&path, max_bytes)?;
        crate::remote_trace::finish(out, &result, trace_bound)?;
        // Persist both exact bytes and their directory entry before cleanup.
        File::open(out)?.sync_all()?;
        if let Some(retain) = retain {
            retain_result(out, id, &object, retain)?;
        }
        let cleanup = self.transport.request(&Request::Cleanup {
            run_id: id.into(),
            result: object.clone(),
        })?;
        isolation::save_json(&out.join("worker-cleanup-response.json"), &cleanup)?;
        ensure!(
            cleanup["run_id"] == id
                && cleanup["removed"] == true
                && serde_json::from_value::<ObjectId>(cleanup["acknowledged_result"].clone())?
                    == object,
            "worker cleanup acknowledgement differs"
        );
        isolation::save_json(&out.join("worker-cleanup.json"), &cleanup)?;
        Ok(result)
    }
}
impl<T: WorkerTransport> Executor for WorkerExecutor<T> {
    fn identity(&mut self) -> Result<ExecutorIdentity> {
        let probe = self.transport.request(&Request::Probe)?;
        ensure!(
            probe["capabilities"]["status_saved_log_lengths"] == true,
            "worker lacks bounded live-log status support"
        );
        let mut tools = BTreeMap::new();
        for name in ["bubblewrap", "unshare"] {
            tools.insert(
                name.into(),
                serde_json::from_value(probe["doctor"]["tools"][name]["identity"].clone())?,
            );
        }
        Ok(ExecutorIdentity {
            kind: "worker-isolated-process".into(),
            build: serde_json::from_value(probe["worker_build"].clone())?,
            policy: json!({"limits":probe["limits"],"protocol":probe["protocol"],"capabilities":probe["capabilities"],"transport":self.transport.description()}),
            tools,
        })
    }
    fn run(
        &mut self,
        store: &Store,
        run: &LockedRun,
        cancel: Arc<AtomicBool>,
        out: &Path,
    ) -> Result<ProcessResult> {
        self.run_internal(store, run, cancel, out, None)
    }
    fn recover(&mut self, out: &Path) -> Result<()> {
        self.recover_internal(out, None)
    }
}

impl<T: WorkerTransport> WorkerExecutor<T> {
    /// The callback must retain the exact bytes outside the client's loss
    /// boundary before acknowledging them. An error prevents worker cleanup.
    pub fn run_with_retention(
        &mut self,
        store: &Store,
        run: &LockedRun,
        cancel: Arc<AtomicBool>,
        out: &Path,
        retain: &mut impl FnMut(&RetentionRequest) -> Result<RetentionIdentity>,
    ) -> Result<ProcessResult> {
        self.run_internal(store, run, cancel, out, Some(retain))
    }

    /// Recovery uses the same barrier, including when cleanup was already
    /// acknowledged. Old requests without a locked-run hash are refused.
    pub fn recover_with_retention(
        &mut self,
        out: &Path,
        retain: &mut impl FnMut(&RetentionRequest) -> Result<RetentionIdentity>,
    ) -> Result<()> {
        self.recover_internal(out, Some(retain))
    }

    fn run_internal(
        &mut self,
        store: &Store,
        run: &LockedRun,
        cancel: Arc<AtomicBool>,
        out: &Path,
        retain: Option<&mut Retainer<'_>>,
    ) -> Result<ProcessResult> {
        ensure!(
            self.identity()?.tools == run.backend_tools,
            "worker isolation backend differs from locked plan"
        );
        let spec = run
            .process
            .as_ref()
            .context("no executable specification")?;
        let mut transferred = BTreeMap::new();
        for object in run.inputs.values() {
            ensure!(
                !cancel.load(Ordering::Acquire),
                "cancelled during artifact transfer"
            );
            if transferred
                .insert(object.sha256.clone(), object.clone())
                .is_none()
            {
                transfer_object(&mut self.transport, store, object, &cancel)?;
            }
        }
        isolation::save_json(
            &out.join("artifact-transfer.json"),
            &json!({"verified_at_destination":transferred,"transport":self.transport.description()}),
        )?;
        let mut random = [0u8; 32];
        File::open("/dev/urandom")?.read_exact(&mut random)?;
        let token = hex::encode(random);
        let mut request_record = json!({"token":token,"locked_run_hash":run.hash()?,"identity":self.identity()?,"trace_bound":spec.limits.trace_bytes,
            "result_bound":spec.limits.trace_bytes*3+1024*1024});
        if retain.is_some() {
            request_record["external_retention_required"] = json!(true);
        }
        isolation::save_json(&out.join("worker-request.json"), &request_record)?;
        File::open(out)?.sync_all()?;
        let started = self.transport.request(&Request::ExecuteToken {
            token,
            spec: spec.clone(),
        })?;
        let id = started["run_id"]
            .as_str()
            .context("worker run ID missing")?
            .to_string();
        isolation::save_json(
            &out.join("worker-run.json"),
            &json!({"run_id":id,"identity":self.identity()?,"result_bound":spec.limits.trace_bytes*3+1024*1024}),
        )?;
        let result = (|| -> Result<ProcessResult> {
            let mut trace = crate::remote_trace::RemoteTrace::create(out, spec.limits.trace_bytes)?;
            let start = Instant::now();
            loop {
                if cancel.load(Ordering::Acquire) {
                    self.transport
                        .request(&Request::Stop { run_id: id.clone() })?;
                    break;
                }
                ensure!(
                    start.elapsed() < Duration::from_secs(spec.limits.wall_seconds + 30),
                    "worker result deadline"
                );
                let status = self
                    .transport
                    .request(&Request::Status { run_id: id.clone() })?;
                if status["state"]["state"] != "booting" {
                    break;
                }
                trace.poll(&mut self.transport, &id, &status)?;
                thread::sleep(Duration::from_millis(50));
            }
            drop(trace);
            self.collect(
                &id,
                out,
                spec.limits.trace_bytes * 3 + 1024 * 1024,
                spec.limits.trace_bytes,
                retain,
            )
        })();
        if result.is_err() {
            let _ = self.transport.request(&Request::Stop { run_id: id });
        }
        result
    }
    fn recover_internal(&mut self, out: &Path, retain: Option<&mut Retainer<'_>>) -> Result<()> {
        if !out.join("worker-request.json").exists() {
            return Ok(());
        }
        if out.join("worker-cleanup.json").exists() {
            if let Some(retain) = retain {
                let cleanup: Value = read_json(&out.join("worker-cleanup.json"), 65536)?;
                let id = cleanup["run_id"]
                    .as_str()
                    .context("cleanup run ID absent")?;
                let result: ObjectId =
                    serde_json::from_value(cleanup["acknowledged_result"].clone())?;
                retain_result(out, id, &result, retain)?;
            }
            return Ok(());
        }
        let saved: Value = read_json(&out.join("worker-request.json"), 65536)?;
        ensure!(
            saved["external_retention_required"] != true || retain.is_some(),
            "saved attempt requires external retention before cleanup"
        );
        if retain.is_some() {
            locked_hash(&saved)?;
        }
        ensure!(
            crate::identity(&saved["identity"])? == crate::identity(&self.identity()?)?,
            "interrupted worker identity differs"
        );
        let token = saved["token"]
            .as_str()
            .context("missing persisted execution token")?;
        let found = self.transport.request(&Request::LookupToken {
            token: token.into(),
        })?;
        if found["found"] == false {
            return Ok(());
        }
        let id = found["run_id"]
            .as_str()
            .context("worker lookup omitted run ID")?;
        if found["state"] == "cleaned" {
            let expected: ObjectId = serde_json::from_value(found["acknowledged_result"].clone())?;
            ensure!(
                hash_file(&out.join("worker-result.json"))? == expected,
                "worker cleaned result is absent or changed locally"
            );
            if let Some(retain) = retain {
                retain_result(out, id, &expected, retain)?;
            }
            isolation::save_json(&out.join("worker-cleanup.json"), &found)?;
            return Ok(());
        }
        self.transport
            .request(&Request::Stop { run_id: id.into() })?;
        // Preserve every partial retrieval under a distinct name. A repeated loss
        // cannot overwrite a previous fragment or silently discard evidence.
        let source = out.join("worker-result.json");
        if source.exists() {
            let target = (0..64)
                .map(|i| out.join(format!("worker-result-loss-{i:02}.json")))
                .find(|p| !p.exists())
                .context("interrupted retrieval history limit reached")?;
            crate::queue::move_without_replace(&source, &target)?;
            File::open(out)?.sync_all()?;
        }
        let _ = self.collect(
            id,
            out,
            saved["result_bound"]
                .as_u64()
                .context("missing result bound")?,
            saved["trace_bound"]
                .as_u64()
                .unwrap_or(isolation::MAX_TRACE_BYTES),
            retain,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod connected_transport_tests {
    use super::*;
    use std::{
        io,
        os::fd::{AsRawFd, OwnedFd},
        os::unix::net::UnixStream,
        sync::mpsc,
    };

    fn transport(stream: UnixStream, private: bool) -> Box<dyn WorkerTransport> {
        let channel = ConnectedTransport::new(stream);
        if private {
            Box::new(PrivateTransport {
                channel,
                deployment: "private test socket pair".into(),
            })
        } else {
            Box::new(channel)
        }
    }

    fn pair() -> (UnixStream, File) {
        let (client, peer) = UnixStream::pair().unwrap();
        client.set_nonblocking(true).unwrap();
        peer.set_nonblocking(true).unwrap();
        // Match production IPC: plain read/write on the real socket descriptor.
        // Some supported hosts deny the recv/send variants used by UnixStream.
        let peer: OwnedFd = peer.into();
        (client, File::from(peer))
    }

    fn readable(stream: &File, timeout_ms: i32) -> bool {
        let mut event = libc::pollfd {
            fd: stream.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let count = unsafe { libc::poll(&mut event, 1, timeout_ms) };
        assert!(count >= 0, "{}", io::Error::last_os_error());
        count != 0 && event.revents & libc::POLLIN != 0
    }

    fn frame(stream: &mut File) -> Value {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut bytes = Vec::new();
        while bytes.last() != Some(&b'\n') {
            assert!(Instant::now() < deadline, "test frame deadline");
            assert!(readable(stream, 1000), "test frame did not arrive");
            let mut byte = [0];
            assert_eq!(stream.read(&mut byte).unwrap(), 1);
            bytes.push(byte[0]);
            assert!(bytes.len() <= 4096, "test frame bound");
        }
        serde_json::from_slice(&bytes).unwrap()
    }

    fn reply(stream: &mut File, result: Value) {
        let mut bytes =
            serde_json::to_vec(&json!({"protocol":"md-sim-worker/2","ok":true,"result":result}))
                .unwrap();
        bytes.push(b'\n');
        stream.write_all(&bytes).unwrap();
    }

    fn assert_timeout_closed(private: bool) {
        let (client, mut peer) = pair();
        // The observer keeps the kernel channel alive to queue a late response.
        // It sends no requests. The transport must neither read that response
        // nor send a second request after its real five-second I/O timeout.
        let observer: OwnedFd = client.try_clone().unwrap().into();
        let mut observer = File::from(observer);
        let mut transport = transport(client, private);
        let error = transport.request(&Request::Probe).unwrap_err();
        assert!(error
            .chain()
            .filter_map(|cause| cause.downcast_ref::<io::Error>())
            .any(|cause| cause.kind() == io::ErrorKind::TimedOut));
        assert_eq!(frame(&mut peer), json!({"operation":"probe"}));
        reply(&mut peer, json!({"late_reply":true}));
        assert!(readable(&observer, 1000));
        let error = transport.request(&Request::Shutdown).unwrap_err();
        assert!(error.to_string().contains("closed after a failed exchange"));
        assert_eq!(frame(&mut observer)["result"]["late_reply"], true);
        assert!(!readable(&peer, 50), "failed channel sent another request");
    }

    #[test]
    fn connected_timeout_rejects_late_reply_and_retry() {
        assert_timeout_closed(false);
    }

    #[test]
    fn private_timeout_rejects_late_reply_and_retry() {
        assert_timeout_closed(true);
    }

    #[test]
    fn successful_channels_preserve_sequential_requests() {
        for private in [false, true] {
            let (client, mut peer) = pair();
            let (finished, hold) = mpsc::channel();
            let server = thread::spawn(move || {
                assert_eq!(frame(&mut peer), json!({"operation":"probe"}));
                reply(&mut peer, json!({"sequence":1}));
                assert_eq!(frame(&mut peer), json!({"operation":"shutdown"}));
                reply(&mut peer, json!({"stopping":true}));
                hold.recv_timeout(Duration::from_secs(2)).unwrap();
            });
            let mut transport = transport(client, private);
            assert_eq!(transport.request(&Request::Probe).unwrap()["sequence"], 1);
            assert_eq!(
                transport.request(&Request::Shutdown).unwrap()["stopping"],
                true
            );
            finished.send(()).unwrap();
            server.join().unwrap();
        }
    }

    #[test]
    fn complete_worker_errors_preserve_independent_requests_on_both_transports() {
        for private in [false, true] {
            let (client, mut peer) = pair();
            let (finished, hold) = mpsc::channel();
            let expected = json!({"protocol":"md-sim-worker/2","ok":false,
                "error":"selected run is absent","detail":{"run_id":"missing"}});
            let envelope = expected.clone();
            let server = thread::spawn(move || {
                assert_eq!(
                    frame(&mut peer),
                    json!({"operation":"status","run_id":"missing"})
                );
                peer.write_all(&serde_json::to_vec(&envelope).unwrap())
                    .unwrap();
                peer.write_all(b"\n").unwrap();
                assert_eq!(frame(&mut peer), json!({"operation":"probe"}));
                reply(&mut peer, json!({"independent_probe":true}));
                hold.recv_timeout(Duration::from_secs(2)).unwrap();
            });
            let mut transport = transport(client, private);
            let error = transport
                .request(&Request::Status {
                    run_id: "missing".into(),
                })
                .unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<worker::CompletedWorkerError>()
                    .unwrap()
                    .response(),
                &expected
            );
            assert_eq!(
                transport.request(&Request::Probe).unwrap()["independent_probe"],
                true
            );
            finished.send(()).unwrap();
            server.join().unwrap();
        }
    }

    #[test]
    fn malformed_error_responses_close_both_transports_without_retry() {
        for private in [false, true] {
            for response in [
                &b"invalid JSON\n"[..],
                &b"{\"protocol\":\"md-sim-worker/2\",\"ok\":false,\"error\":null}\n"[..],
                &b"{\"protocol\":\"md-sim-worker/2\",\"ok\":false,\"error\":\"\"}\n"[..],
                &b"{\"protocol\":\"md-sim-worker/1\",\"ok\":false,\"error\":\"rejected\"}\n"[..],
            ] {
                let (client, mut peer) = pair();
                let _observer = client.try_clone().unwrap();
                peer.write_all(response).unwrap();
                let mut transport = transport(client, private);
                let error = transport.request(&Request::Probe).unwrap_err();
                assert!(!error.is::<worker::CompletedWorkerError>());
                assert_eq!(frame(&mut peer), json!({"operation":"probe"}));
                assert!(transport.request(&Request::Shutdown).is_err());
                assert!(!readable(&peer, 50), "failed channel was reused");
            }
        }
    }
}

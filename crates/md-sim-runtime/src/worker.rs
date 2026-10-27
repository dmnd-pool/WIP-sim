pub use crate::worker_budget::WorkerLimits;
use crate::{
    doctor,
    file_lock::ExclusiveFileLock,
    isolation::{self, ProcessSpec},
    store::{hash_file, ObjectId, Store, MAX_CHUNK},
    worker_budget,
};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};

/// Version used by strict worker request and response envelopes.
pub const PROTOCOL_VERSION: &str = "md-sim-worker/2";
pub const MAX_CONTROL_MESSAGE: u64 = (MAX_CHUNK * 2 + 65536) as u64;
pub const MAX_RELAY_REQUESTS: u32 = 65536;
const MAX_REQUEST: u64 = MAX_CONTROL_MESSAGE;
const MAX_COMPLETED_ERROR_BYTES: usize = 8192;
#[path = "worker_channel.rs"]
mod channel;
#[path = "worker_result.rs"]
mod result_reader;
#[path = "worker_timeout.rs"]
mod timeout;
pub use channel::{PrivateChannelBinding, PrivateChannelLeaseSpec, PrivateChannelLifecycle};
pub use timeout::request_with_timeout;

/// A complete, validated worker error reply. It does not leave an uncertain
/// response on the channel. Relays can forward the original parsed envelope.
#[derive(Debug)]
pub struct CompletedWorkerError {
    response: Value,
}

impl CompletedWorkerError {
    pub fn response(&self) -> &Value {
        &self.response
    }
}

impl std::fmt::Display for CompletedWorkerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "worker error: {}",
            self.response["error"]
                .as_str()
                .expect("completed worker error was validated")
        )
    }
}

impl std::error::Error for CompletedWorkerError {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogStream {
    Stdout,
    Stderr,
}

impl LogStream {
    fn filename(&self) -> &'static str {
        match self {
            Self::Stdout => "stdout.log",
            Self::Stderr => "stderr.log",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Request {
    Probe,
    Begin {
        object: ObjectId,
    },
    Chunk {
        object: ObjectId,
        offset: u64,
        hex: String,
    },
    Finish {
        object: ObjectId,
    },
    Abort {
        object: ObjectId,
    },
    Get {
        object: ObjectId,
        offset: u64,
        count: usize,
    },
    Execute {
        spec: ProcessSpec,
    },
    /// Durable idempotent submission. Cleanup retains a bounded token receipt.
    ExecuteToken {
        token: String,
        spec: ProcessSpec,
    },
    LookupToken {
        token: String,
    },
    /// Fresh owning-provider observation. It does not infer carrier liveness
    /// from the job's process state or from a network address.
    PrivateChannelStatus {
        token: String,
        challenge: String,
    },
    Status {
        run_id: String,
    },
    Result {
        run_id: String,
    },
    ResultData {
        run_id: String,
        result: ObjectId,
        offset: u64,
        count: usize,
    },
    Logs {
        run_id: String,
        stream: LogStream,
        offset: u64,
        count: usize,
    },
    Stop {
        run_id: String,
    },
    /// Remove a completed run only after its result identity is acknowledged.
    /// Published inputs and other runs are retained.
    Cleanup {
        run_id: String,
        result: ObjectId,
    },
    Shutdown,
}

struct Active {
    cancel: Arc<AtomicBool>,
    join: JoinHandle<()>,
    reserved_bytes: u64,
}

pub fn request(socket: &Path, request: &Request) -> Result<Value> {
    let mut stream = UnixStream::connect(socket).context("connect to worker")?;
    request_stream(&mut stream, request)
}

/// One control request over an already connected private IPC channel.
pub fn request_stream(stream: &mut UnixStream, request: &Request) -> Result<Value> {
    stream.set_nonblocking(true)?;
    let fd: OwnedFd = stream.try_clone()?.into();
    request_channel(File::from(fd), request)
}

/// One request over an already opened private virtual character channel.
/// The trusted caller selects the device, checks its identity and serializes
/// exchanges. This adds no listener and does not establish SSH authentication.
pub fn request_device(device: &mut File, request: &Request) -> Result<Value> {
    ensure!(
        device.metadata()?.file_type().is_char_device(),
        "worker device channel is not a character device"
    );
    let flags = unsafe { libc::fcntl(device.as_raw_fd(), libc::F_GETFL) };
    ensure!(
        flags >= 0
            && unsafe { libc::fcntl(device.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) }
                == 0,
        "cannot set bounded device channel I/O"
    );
    request_channel(device.try_clone()?, request)
}

fn request_channel(channel: File, request: &Request) -> Result<Value> {
    let mut stream = Ipc(channel);
    serde_json::to_writer(&mut stream, request)?;
    stream.write_all(b"\n")?;
    let response = read_message(&mut stream)?;
    check_response(response)
}

/// Decode one complete response from a bounded, authenticated transport.
pub fn decode_response(bytes: &[u8]) -> Result<Value> {
    ensure!(
        bytes.len() as u64 <= MAX_REQUEST,
        "oversized worker response"
    );
    ensure!(
        bytes.ends_with(b"\n") && !bytes[..bytes.len() - 1].contains(&b'\n'),
        "worker response must be one complete line"
    );
    check_response(serde_json::from_slice(bytes)?)
}

fn check_response(response: Value) -> Result<Value> {
    ensure!(
        response["protocol"] == PROTOCOL_VERSION,
        "worker protocol mismatch"
    );
    match response["ok"].as_bool() {
        Some(true) => Ok(response["result"].clone()),
        Some(false) => {
            let message = response["error"]
                .as_str()
                .context("worker error reply has no error string")?;
            ensure!(
                !message.is_empty()
                    && message.len() <= MAX_COMPLETED_ERROR_BYTES
                    && !message.contains('\0'),
                "worker error reply has an invalid error string"
            );
            Err(CompletedWorkerError { response }.into())
        }
        None => bail!("worker response status is not boolean"),
    }
}

/// The SSH server runs this one-shot relay as its authenticated account.
/// Control remains on an owner-only Unix socket. No network listener is added.
pub fn relay(socket: &Path, input: impl Read, output: impl Write) -> Result<()> {
    relay_request(input, output, |message| request(socket, message))
}

/// Forward one bounded protocol request through a caller-owned transport.
/// The caller supplies I/O deadlines and authenticates the incoming peer.
/// Dispatch finishes before response delivery, even when that peer disconnects.
pub fn relay_request(
    input: impl Read,
    mut output: impl Write,
    dispatch: impl FnOnce(&Request) -> Result<Value>,
) -> Result<()> {
    let result = read_message(&mut input.take(MAX_REQUEST + 1))
        .and_then(|value| Ok(serde_json::from_value::<Request>(value)?))
        .and_then(|message| dispatch(&message));
    let response = match result {
        Ok(value) => json!({"protocol":PROTOCOL_VERSION,"ok":true,"result":value}),
        Err(error) => match error.downcast_ref::<CompletedWorkerError>() {
            Some(remote) => remote.response().clone(),
            None => json!({"protocol":PROTOCOL_VERSION,"ok":false,"error":format!("{error:#}")}),
        },
    };
    serde_json::to_writer(&mut output, &response)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}

/// Reuse one authenticated SSH channel. Each frame still uses the owner-only
/// worker socket and the same bounded request protocol. A partial frame is an
/// error, never a request to retry an operation whose outcome is unknown.
pub fn relay_stream(socket: &Path, input: impl Read, mut output: impl Write) -> Result<()> {
    let mut input = BufReader::new(input);
    for _ in 0..MAX_RELAY_REQUESTS {
        let Some(bytes) = relay_frame(&mut input)? else {
            return Ok(());
        };
        let result = serde_json::from_slice::<Request>(&bytes)
            .map_err(anyhow::Error::from)
            .and_then(|message| request(socket, &message));
        let response = match result {
            Ok(value) => json!({"protocol":PROTOCOL_VERSION,"ok":true,"result":value}),
            Err(error) => match error.downcast_ref::<CompletedWorkerError>() {
                Some(remote) => remote.response().clone(),
                None => {
                    json!({"protocol":PROTOCOL_VERSION,"ok":false,"error":format!("{error:#}")})
                }
            },
        };
        serde_json::to_writer(&mut output, &response)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    bail!("SSH relay session request limit")
}

fn relay_frame(input: &mut impl BufRead) -> Result<Option<Vec<u8>>> {
    let mut frame = Vec::new();
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            ensure!(frame.is_empty(), "partial SSH relay request");
            return Ok(None);
        }
        let end = available.iter().position(|byte| *byte == b'\n');
        let count = end.map_or(available.len(), |index| index + 1);
        ensure!(
            frame.len() + count <= MAX_REQUEST as usize,
            "oversized SSH relay request"
        );
        frame.extend_from_slice(&available[..count]);
        input.consume(count);
        if end.is_some() {
            return Ok(Some(frame));
        }
    }
}

#[cfg(test)]
mod relay_frame_tests {
    use super::*;
    #[test]
    fn dispatched_worker_error_preserves_its_complete_envelope() {
        let response = json!({"protocol":PROTOCOL_VERSION,"ok":false,
            "error":"unknown selected run","detail":{"run_id":"run-absent"}});
        let mut wire = serde_json::to_vec(&response).unwrap();
        wire.push(b'\n');
        let mut output = Vec::new();
        relay_request(
            &b"{\"operation\":\"probe\"}\n"[..],
            &mut output,
            |request| {
                assert!(matches!(request, Request::Probe));
                decode_response(&wire)
            },
        )
        .unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&output).unwrap(), response);
    }

    #[test]
    fn invalid_input_is_not_dispatched_and_dispatch_finishes_before_output() {
        for input in [
            b"{\"operation\":\"probe\"}".to_vec(),
            vec![b'x'; MAX_REQUEST as usize + 1],
        ] {
            let mut output = Vec::new();
            relay_request(input.as_slice(), &mut output, |_| {
                panic!("invalid request dispatched")
            })
            .unwrap();
            assert_eq!(
                serde_json::from_slice::<Value>(&output).unwrap()["ok"],
                false
            );
        }
        struct Gone;
        impl Write for Gone {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut dispatched = false;
        assert!(
            relay_request(&b"{\"operation\":\"probe\"}\n"[..], Gone, |_| {
                dispatched = true;
                Ok(json!({"complete":true}))
            })
            .is_err()
        );
        assert!(dispatched);
    }

    #[test]
    fn frames_preserve_buffered_successors_and_reject_partial_or_oversized_input() {
        let mut input =
            BufReader::new(&b"{\"operation\":\"probe\"}\n{\"operation\":\"shutdown\"}\n"[..]);
        assert_eq!(
            relay_frame(&mut input).unwrap().unwrap(),
            b"{\"operation\":\"probe\"}\n"
        );
        assert_eq!(
            relay_frame(&mut input).unwrap().unwrap(),
            b"{\"operation\":\"shutdown\"}\n"
        );
        assert!(relay_frame(&mut input).unwrap().is_none());
        assert!(relay_frame(&mut BufReader::new(&b"{}"[..])).is_err());
        let large = vec![b'x'; MAX_REQUEST as usize + 1];
        assert!(relay_frame(&mut BufReader::new(large.as_slice())).is_err());
    }
}

fn read_message(stream: &mut impl Read) -> Result<Value> {
    let mut line = String::new();
    let n = BufReader::new(stream.take(MAX_REQUEST + 1)).read_line(&mut line)?;
    ensure!(
        n as u64 <= MAX_REQUEST && line.ends_with('\n'),
        "missing or oversized protocol message"
    );
    Ok(serde_json::from_str(&line)?)
}

fn peer_is_owner(stream: &UnixStream) -> Result<()> {
    let mut cred = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    unsafe {
        ensure!(
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut cred as *mut libc::ucred).cast(),
                &mut len
            ) == 0,
            "cannot read worker peer identity"
        );
        ensure!(
            cred.uid == libc::geteuid(),
            "worker peer is not the worker owner"
        );
    }
    Ok(())
}

pub(crate) fn validate_run_id(run_id: &str) -> Result<()> {
    ensure!(
        matches!(run_id.len(), 36 | 68)
            && run_id.starts_with("run-")
            && run_id[4..].bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid run ID"
    );
    Ok(())
}

fn run_path(root: &Path, run_id: &str) -> Result<PathBuf> {
    validate_run_id(run_id)?;
    let p = root.join("runs").join(run_id);
    ensure!(
        p.is_dir() && !fs::symlink_metadata(&p)?.file_type().is_symlink(),
        "run is not owned by this worker"
    );
    Ok(p)
}

fn new_run(root: &Path, token: Option<&str>) -> Result<(String, PathBuf)> {
    let mut random = [0u8; 16];
    File::open("/dev/urandom")?.read_exact(&mut random)?;
    let id = format!(
        "run-{}",
        token
            .map(str::to_owned)
            .unwrap_or_else(|| hex::encode(random))
    );
    let p = root.join("runs").join(&id);
    fs::create_dir(&p)?;
    File::open(root.join("runs"))?.sync_all()?;
    Ok((id, p))
}

fn owned_run_files(path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(path)? {
        ensure!(files.len() < 32, "run file count limit exceeded");
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        ensure!(
            metadata.is_file() && metadata.nlink() == 1,
            "run contains an unsafe file"
        );
        let name = entry.file_name();
        let name = name.to_str().context("run file name is not UTF-8")?;
        let ordinary = [
            "spec.json",
            "lock.json",
            "state.json",
            "state-before-loss.json",
            "result.json",
            "stdout.log",
            "stderr.log",
            "cleanup.json",
            "cgroup.json",
            "dashboard.json",
        ]
        .contains(&name);
        // save_json can leave an incomplete atomic write after abrupt loss.
        // Retain it as failure evidence. Only its exact known name is owned.
        let partial = name.split_once(".tmp-").is_some_and(|(stem, suffix)| {
            let (pid, random) = suffix
                .split_once('-')
                .map_or((suffix, None), |(pid, random)| (pid, Some(random)));
            [
                "spec",
                "lock",
                "state",
                "result",
                "cleanup",
                "cgroup",
                "dashboard",
            ]
            .contains(&stem)
                && !pid.is_empty()
                && pid.len() <= 10
                && pid.bytes().all(|b| b.is_ascii_digit())
                && random.is_none_or(|text| {
                    text.len() == 24
                        && text
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
        });
        ensure!(ordinary || partial, "run contains an unknown file");
        files.push(entry.path());
    }
    Ok(files)
}

fn recover_run_cgroup(path: &Path) -> Result<()> {
    if path.join("cgroup.json").exists() {
        ensure!(
            fs::metadata(path.join("lock.json"))?.len() <= MAX_REQUEST,
            "worker lock is oversized"
        );
        let lock: Value = serde_json::from_slice(&fs::read(path.join("lock.json"))?)?;
        let parent = lock["worker_limits"]["cgroup_parent"]
            .as_str()
            .context("cgroup receipt lacks locked delegation")?;
        crate::cgroup::recover(&path.join("cgroup.json"), Path::new(parent))?;
    }
    Ok(())
}

fn recover_interrupted_runs(root: &Path) -> Result<()> {
    for (count, entry) in fs::read_dir(root.join("runs"))?.enumerate() {
        ensure!(count < 65536, "run storage exceeds scan limit");
        let entry = entry?;
        let id = entry.file_name();
        let path = run_path(root, id.to_str().context("run ID is not UTF-8")?)?;
        owned_run_files(&path)?;
        recover_run_cgroup(&path)?;
        if path.join("cleanup.json").exists() {
            read_cleanup(&path, id.to_str().context("run ID is not UTF-8")?)?;
            finish_token_cleanup(&path)?;
            continue;
        }
        if path.join("result.json").exists() {
            if !path.join("state.json").exists() {
                isolation::save_json(
                    &path.join("state.json"),
                    &json!({
                        "state":"lost", "firmware_gate":"not-run",
                        "reason":"worker lost the final state commit; inspect the retained result",
                        "result_retained":true
                    }),
                )?;
            }
            continue;
        }
        if path.join("state.json").exists() {
            ensure!(
                !path.join("state-before-loss.json").exists(),
                "previous loss state already exists; preserve both states for inspection"
            );
            fs::rename(path.join("state.json"), path.join("state-before-loss.json"))?;
        }
        let mut retained = BTreeMap::new();
        for file in owned_run_files(&path)? {
            retained.insert(
                file.file_name()
                    .context("missing run file name")?
                    .to_string_lossy()
                    .into_owned(),
                hash_file(&file)?,
            );
        }
        // The worker lock excludes a live supervisor. No complete result was
        // committed. Keep surviving bytes, including partial JSON writes, and
        // create an explicitly incomplete result so retrieval/cleanup can work.
        isolation::save_json(
            &path.join("result.json"),
            &json!({
                "status":"lost", "firmware_gate":"not-run",
                "reason":"worker restarted before a complete process result was saved",
                "retained_files":retained,
                "recovery_worker_build":hash_file(&std::env::current_exe()?)?,
            }),
        )?;
        isolation::save_json(
            &path.join("state.json"),
            &json!({
                "state":"lost", "firmware_gate":"not-run", "incomplete_result_retained":true
            }),
        )?;
    }
    Ok(())
}

/// Private local worker transport. SSH must carry this protocol before remote
/// support can be claimed; this listener never binds a public network address.
pub fn serve(root: &Path, socket: &Path) -> Result<()> {
    serve_with_limits(root, socket, WorkerLimits::default())
}

pub fn serve_with_limits(root: &Path, socket: &Path, limits: WorkerLimits) -> Result<()> {
    limits.validate()?;
    let (root, _root_lease) = acquire_worker_root(root)?;
    let endpoint = endpoint::Endpoint::bind(&root, socket)?;
    let result = serve_locked_connections(
        &root,
        endpoint.listener().incoming().map(|r| {
            let stream = r?;
            peer_is_owner(&stream)?;
            Ok(stream)
        }),
        limits,
        None,
    );
    endpoint.close()?;
    result
}

/// A dispatcher can supply private, connected IPC streams. This supports tests
/// when the host prohibits listeners. It is not a claim of SSH transport.
pub fn serve_connections(
    root: &Path,
    connections: impl IntoIterator<Item = Result<UnixStream>>,
) -> Result<()> {
    serve_connections_with_limits(root, connections, WorkerLimits::default())
}

pub fn serve_connections_with_limits(
    root: &Path,
    connections: impl IntoIterator<Item = Result<UnixStream>>,
    limits: WorkerLimits,
) -> Result<()> {
    limits.validate()?;
    let (root, _root_lease) = acquire_worker_root(root)?;
    serve_locked_connections(&root, connections, limits, None)
}

/// Serve with one trusted provider bound to an exact canonical process.
/// Requests cannot install or replace the binding. Token lookup remains
/// idempotent after its single accepted execution, including after cleanup.
pub fn serve_connections_with_channel(
    root: &Path,
    connections: impl IntoIterator<Item = Result<UnixStream>>,
    limits: WorkerLimits,
    binding: PrivateChannelBinding,
) -> Result<()> {
    limits.validate()?;
    let lease = WorkerRootLease::acquire(root)?;
    serve_connections_with_channel_owned(&lease, connections, limits, binding)
}

/// An owned worker-root lease for a trusted supervisor that must retain root
/// ownership while it prepares a provider and publishes its final evidence.
/// All worker entry points use the same exclusive acquisition path.
pub struct WorkerRootLease {
    root: PathBuf,
    _lease: ExclusiveFileLock,
}

impl WorkerRootLease {
    pub fn acquire(root: &Path) -> Result<Self> {
        let (root, lease) = acquire_worker_root(root)?;
        Ok(Self {
            root,
            _lease: lease,
        })
    }
    pub fn path(&self) -> &Path {
        &self.root
    }
}

/// Keep the caller's root lease held after service return. The caller can
/// publish provider evidence before dropping that lease.
pub fn serve_connections_with_channel_owned(
    lease: &WorkerRootLease,
    connections: impl IntoIterator<Item = Result<UnixStream>>,
    limits: WorkerLimits,
    binding: PrivateChannelBinding,
) -> Result<()> {
    limits.validate()?;
    serve_locked_connections(lease.path(), connections, limits, Some(binding))
}

#[path = "worker_endpoint.rs"]
mod endpoint;

fn acquire_worker_root(root: &Path) -> Result<(PathBuf, ExclusiveFileLock)> {
    fs::create_dir_all(root)?;
    ensure!(
        fs::symlink_metadata(root)?.is_dir(),
        "worker root must be a real directory"
    );
    fs::create_dir_all(root.join("runs"))?;
    ensure!(
        fs::symlink_metadata(root.join("runs"))?.is_dir(),
        "worker runs must be a real directory"
    );
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    let root = root.canonicalize()?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(root.join("worker.lock"))?;
    ensure!(
        lock.metadata()?.is_file(),
        "worker lock must be a regular file"
    );
    let lock = ExclusiveFileLock::acquire(lock).context("worker root is already in use")?;
    Ok((root, lock))
}

fn serve_locked_connections(
    root: &Path,
    connections: impl IntoIterator<Item = Result<UnixStream>>,
    limits: WorkerLimits,
    mut channel: Option<PrivateChannelBinding>,
) -> Result<()> {
    let store = Arc::new(Store::open_with_limits(
        &root.join("store"),
        limits.store.clone(),
    )?);
    recover_interrupted_runs(root)?;
    let mut active: BTreeMap<String, Active> = BTreeMap::new();
    let mut results = result_reader::Reader::default();
    let service_result = (|| -> Result<()> {
        for connection in connections {
            reap_finished(root, &mut active)?;
            let stream = connection?;
            stream.set_nonblocking(true)?;
            let fd: OwnedFd = stream.into();
            let mut stream = Ipc(File::from(fd));
            let parsed =
                read_message(&mut stream).and_then(|v| Ok(serde_json::from_value::<Request>(v)?));
            let mut shutdown = false;
            let result = parsed.and_then(|req| {
                if matches!(req, Request::Shutdown) {
                    shutdown = true;
                }
                handle(
                    req,
                    root,
                    &store,
                    &mut active,
                    &limits,
                    &mut results,
                    &mut channel,
                )
            });
            let reply = match result {
                Ok(value) => json!({"protocol":PROTOCOL_VERSION,"ok":true,"result":value}),
                Err(e) => json!({"protocol":PROTOCOL_VERSION,"ok":false,"error":format!("{e:#}")}),
            };
            // A client disconnect does not discard a run or stop the worker.
            let _ = serde_json::to_writer(&mut stream, &reply);
            let _ = stream.write_all(b"\n");
            if shutdown {
                break;
            }
        }
        Ok(())
    })();
    for (_, run) in active {
        run.cancel.store(true, Ordering::Release);
        let _ = run.join.join();
    }
    service_result
}

fn reap_finished(root: &Path, active: &mut BTreeMap<String, Active>) -> Result<()> {
    let finished: Vec<_> = active
        .iter()
        .filter(|(_, run)| run.join.is_finished())
        .map(|(id, _)| id.clone())
        .collect();
    for id in finished {
        let run = active.remove(&id).expect("collected active run");
        if run.join.join().is_err() {
            let path = run_path(root, &id)?;
            isolation::save_json(
                &path.join("state.json"),
                &json!({
                    "state":"failed", "reason":"worker run thread panicked", "firmware_gate":"not-run"
                }),
            )?;
        }
    }
    Ok(())
}

fn validate_token(token: &str) -> Result<()> {
    ObjectId {
        sha256: token.into(),
        size: 0,
    }
    .validate()
}

/// Identity of the canonical serialized ProcessSpec. This includes all declared
/// inputs and limits. It is not a replacement for a resolved target identity.
pub fn process_identity(spec: &ProcessSpec) -> Result<ObjectId> {
    let bytes = serde_json::to_vec(spec)?;
    Ok(ObjectId {
        sha256: hex::encode(Sha256::digest(&bytes)),
        size: bytes.len() as u64,
    })
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CleanupReceipt {
    token: String,
    process_identity: Option<ObjectId>,
    acknowledged_result: ObjectId,
}

fn read_cleanup(path: &Path, run_id: &str) -> Result<CleanupReceipt> {
    let bytes = fs::read(path.join("cleanup.json"))?;
    ensure!(bytes.len() <= 4096, "cleanup receipt is oversized");
    let receipt: CleanupReceipt = serde_json::from_slice(&bytes)?;
    validate_token(&receipt.token)?;
    ensure!(
        run_id == format!("run-{}", receipt.token),
        "cleanup token differs from run"
    );
    receipt.acknowledged_result.validate()?;
    if let Some(id) = &receipt.process_identity {
        id.validate()?;
    }
    Ok(receipt)
}

fn finish_token_cleanup(path: &Path) -> Result<()> {
    for file in owned_run_files(path)? {
        if file.file_name().is_some_and(|n| n == "cleanup.json") {
            continue;
        }
        fs::remove_file(file)?;
    }
    File::open(path)?.sync_all()?;
    Ok(())
}

fn lookup_token(root: &Path, token: &str) -> Result<Value> {
    validate_token(token)?;
    let run_id = format!("run-{token}");
    let candidate = root.join("runs").join(&run_id);
    if !candidate.try_exists()? {
        return Ok(json!({"found":false,"token":token}));
    }
    let path = run_path(root, &run_id)?;
    owned_run_files(&path)?;
    if path.join("cleanup.json").exists() {
        let receipt = read_cleanup(&path, &run_id)?;
        return Ok(
            json!({"found":true,"run_id":run_id,"token":token,"state":"cleaned",
            "token_process_identity":receipt.process_identity,"acknowledged_result":receipt.acknowledged_result}),
        );
    }
    let spec_path = path.join("spec.json");
    let identity = if spec_path.exists() {
        ensure!(
            spec_path.metadata()?.len() <= MAX_REQUEST,
            "saved specification is oversized"
        );
        let spec: ProcessSpec = serde_json::from_slice(&fs::read(spec_path)?)?;
        Some(process_identity(&spec)?)
    } else {
        None
    };
    let state = if path.join("state.json").exists() {
        serde_json::from_slice::<Value>(&fs::read(path.join("state.json"))?)?["state"].clone()
    } else {
        json!("reserved")
    };
    Ok(
        json!({"found":true,"run_id":run_id,"token":token,"state":state,"token_process_identity":identity}),
    )
}

fn execute_request(
    spec: ProcessSpec,
    token: Option<&str>,
    root: &Path,
    store: &Arc<Store>,
    active: &mut BTreeMap<String, Active>,
    limits: &WorkerLimits,
    channel: &mut Option<PrivateChannelBinding>,
) -> Result<Value> {
    if let Some(token) = token {
        validate_token(token)?;
        let found = lookup_token(root, token)?;
        if found["found"] == true {
            let expected = process_identity(&spec)?;
            ensure!(found["token_process_identity"] == serde_json::to_value(expected)?,
                "execution token belongs to a different or incompletely committed process specification");
            return Ok(found);
        }
    }
    spec.validate()?;
    if spec.private_channel.is_some() {
        let binding = channel
            .as_ref()
            .context("private channel is not configured for this worker")?;
        binding.check(&spec)?;
        binding.check_token(token)?;
    }
    if let Some(input) = &spec.stdin_object {
        store.verified_path(input)?;
    }
    for input in &spec.inputs {
        store.verified_path(&input.object)?;
    }
    let reservations = active
        .iter()
        .map(|(id, run)| (id.clone(), run.reserved_bytes))
        .collect();
    let reserved_bytes = worker_budget::admit(limits, root, &reservations, &spec)?;
    let provider = if spec.private_channel.is_some() {
        Some(
            channel
                .as_mut()
                .context("private channel is not configured for this worker")?
                .take(&spec)?,
        )
    } else {
        None
    };
    let (id, p) = new_run(root, token)?;
    isolation::save_json(&p.join("spec.json"), &spec)?;
    let spec_hash = hash_file(&p.join("spec.json"))?;
    let worker_build = hash_file(&std::env::current_exe()?)?;
    let mut lock = json!({"process_spec":spec_hash,"worker_build":worker_build,"worker_limits":limits,"reserved_evidence_bytes":reserved_bytes,"claim":"isolated-process primitive; no firmware capability claim"});
    if provider.is_some() {
        lock["private_channel_binding"] = json!({"process_identity":process_identity(&spec)?,"provider_consumed":true,"scope":"one trusted provider; peer topology requires separate evidence"});
    }
    isolation::save_json(&p.join("lock.json"), &lock)?;
    isolation::save_json(
        &p.join("state.json"),
        &json!({"state":"booting","process_started_is_firmware_readiness":false}),
    )?;
    if provider.is_some() {
        channel
            .as_mut()
            .context("private channel binding absent")?
            .bind_run(token, &id)?;
    }
    let new_log = |name: &str| -> Result<File> {
        Ok(fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(p.join(name))?)
    };
    let trace = isolation::TraceFiles {
        stdout: new_log("stdout.log")?,
        stderr: new_log("stderr.log")?,
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let c = cancel.clone();
    let s = store.clone();
    let cgroup_parent = limits.cgroup_parent.clone();
    let join = thread::spawn(move || {
        let channel_binding_consumed = provider.is_some();
        let result = if let Some(provider) = provider {
            isolation::execute_private_channel_recorded(
                &s,
                &spec,
                c,
                Some(trace),
                cgroup_parent.as_deref(),
                cgroup_parent
                    .as_ref()
                    .map(|_| p.join("cgroup.json"))
                    .as_deref(),
                Some(&p.join("dashboard.json")),
                None,
                provider,
            )
        } else {
            isolation::execute_dashboard_recorded(
                &s,
                &spec,
                c,
                Some(trace),
                cgroup_parent.as_deref(),
                cgroup_parent
                    .as_ref()
                    .map(|_| p.join("cgroup.json"))
                    .as_deref(),
                Some(&p.join("dashboard.json")),
            )
        };
        let (state, evidence) = match result {
            Ok(result) => {
                let state = if result.status == "exited" && result.exit_code == Some(0) {
                    "stopped"
                } else {
                    "failed"
                };
                (state, json!(result))
            }
            Err(e) => {
                let error = format!("{e:#}");
                let saved: String = error.chars().take(8192).collect();
                let mut evidence =
                    json!({"infrastructure_error":saved,"error_truncated":saved.len()<error.len()});
                if channel_binding_consumed {
                    evidence["private_channel_binding_consumed"] = json!(true);
                }
                ("failed", evidence)
            }
        };
        if isolation::save_json(&p.join("result.json"), &evidence).is_ok() {
            let _ = isolation::save_json(
                &p.join("state.json"),
                &json!({"state":state,"firmware_gate":"not-run","scratch_storage":"released with namespace"}),
            );
        }
    });
    active.insert(
        id.clone(),
        Active {
            cancel,
            join,
            reserved_bytes,
        },
    );
    if let Some(token) = token {
        lookup_token(root, token)
    } else {
        Ok(json!({"run_id":id,"state":"booting","process_spec":spec_hash}))
    }
}

fn handle(
    req: Request,
    root: &Path,
    store: &Arc<Store>,
    active: &mut BTreeMap<String, Active>,
    limits: &WorkerLimits,
    results: &mut result_reader::Reader,
    channel: &mut Option<PrivateChannelBinding>,
) -> Result<Value> {
    Ok(match req {
        Request::Probe => {
            let reservations = active
                .iter()
                .map(|(id, run)| (id.clone(), run.reserved_bytes))
                .collect();
            json!({"doctor":doctor::inspect(),"protocol":PROTOCOL_VERSION,"capabilities":{"status_saved_log_lengths":true,"private_channel_status":true},"worker_build":hash_file(&std::env::current_exe()?)?,"limits":limits,"retained_usage":worker_budget::retained_usage(root, &reservations)?,"limits_scope":"host input and evidence bytes; per-process guest limits are separate"})
        }
        Request::Begin { object } => json!({"offset":store.begin(&object)?}),
        Request::Chunk {
            object,
            offset,
            hex,
        } => {
            ensure!(hex.len() <= MAX_CHUNK * 2, "chunk too large");
            json!({"offset":store.chunk(&object,offset,&hex::decode(hex)?)?})
        }
        Request::Finish { object } => {
            store.finish(&object)?;
            json!({"verified":object})
        }
        Request::Abort { object } => {
            store.abort(&object)?;
            json!({"partial_transfer_removed":object,"published_objects_retained":true})
        }
        Request::Get {
            object,
            offset,
            count,
        } => {
            json!({"object":object,"offset":offset,"hex":hex::encode(store.read_chunk(&object,offset,count)?) })
        }
        Request::Execute { spec } => {
            execute_request(spec, None, root, store, active, limits, channel)?
        }
        Request::ExecuteToken { token, spec } => {
            execute_request(spec, Some(&token), root, store, active, limits, channel)?
        }
        Request::LookupToken { token } => lookup_token(root, &token)?,
        Request::PrivateChannelStatus { token, challenge } => {
            let found = lookup_token(root, &token)?;
            channel
                .as_mut()
                .context("private channel is not configured for this worker")?
                .lease(&token, &challenge, &found)?
        }
        Request::Status { run_id } => {
            let p = run_path(root, &run_id)?;
            if p.join("cleanup.json").exists() {
                let receipt = read_cleanup(&p, &run_id)?;
                return Ok(
                    json!({"run_id":run_id,"state":{"state":"cleaned"},"acknowledged_result":receipt.acknowledged_result}),
                );
            }
            let mut state: Value = serde_json::from_slice(&fs::read(p.join("state.json"))?)?;
            if !active.contains_key(&run_id) && state["state"] == "booting" {
                state["state"] = json!("failed");
                state["reason"] = json!("worker restarted before the final state was committed; inspect the retained result");
                isolation::save_json(&p.join("state.json"), &state)?;
            }
            let mut saved_logs = BTreeMap::new();
            for stream in [LogStream::Stdout, LogStream::Stderr] {
                let file = fs::OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(p.join(stream.filename()))?;
                let metadata = file.metadata()?;
                ensure!(
                    metadata.is_file() && metadata.nlink() == 1,
                    "unsafe log file"
                );
                ensure!(
                    metadata.len() <= isolation::MAX_TRACE_BYTES,
                    "saved log exceeds bound"
                );
                saved_logs.insert(
                    match stream {
                        LogStream::Stdout => "stdout",
                        LogStream::Stderr => "stderr",
                    },
                    metadata.len(),
                );
            }
            ensure!(
                saved_logs.values().sum::<u64>() <= isolation::MAX_TRACE_BYTES,
                "saved logs exceed bound"
            );
            let dashboard = if active.contains_key(&run_id) && p.join("dashboard.json").exists() {
                let spec: ProcessSpec = serde_json::from_slice(&fs::read(p.join("spec.json"))?)?;
                let binding = crate::dashboard::Binding::read(
                    &p.join("dashboard.json"),
                    spec.dashboard
                        .as_ref()
                        .context("dashboard absent in selected process")?,
                    &spec,
                )?;
                Some(binding)
            } else {
                None
            };
            json!({"run_id":run_id,"state":state,"saved_logs":saved_logs,"dashboard":dashboard})
        }
        Request::Result { run_id } => {
            let p = run_path(root, &run_id)?;
            let file = p.join("result.json");
            ensure!(!active.contains_key(&run_id), "result is not complete");
            ensure!(file.exists(), "result is not complete");
            let mut summary: Value = serde_json::from_slice(&fs::read(&file)?)?;
            if let Some(fields) = summary.as_object_mut() {
                fields.remove("stdout");
                fields.remove("stderr");
            }
            json!({"run_id":run_id,"result":summary,"result_identity":hash_file(&file)?,"complete_result_operation":"result-data","trace_operation":"logs"})
        }
        Request::ResultData {
            run_id,
            result,
            offset,
            count,
        } => {
            ensure!(count > 0 && count <= MAX_CHUNK, "invalid result chunk size");
            let p = run_path(root, &run_id)?;
            ensure!(!active.contains_key(&run_id), "result is not complete");
            result.validate()?;
            let path = p.join("result.json");
            let bytes = results.read(&path, &result, offset, count)?;
            json!({"run_id":run_id,"result_identity":result,"offset":offset,"next_offset":offset+bytes.len() as u64,"hex":hex::encode(bytes)})
        }
        Request::Logs {
            run_id,
            stream,
            offset,
            count,
        } => {
            ensure!(count > 0 && count <= MAX_CHUNK, "invalid log chunk size");
            let p = run_path(root, &run_id)?;
            let path = p.join(stream.filename());
            let mut file = fs::OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&path)?;
            let metadata = file.metadata()?;
            ensure!(metadata.is_file(), "log must be a regular file");
            let available = metadata.len();
            ensure!(offset <= available, "log offset exceeds saved bytes");
            file.seek(SeekFrom::Start(offset))?;
            let mut bytes = Vec::new();
            file.take((available - offset).min(count as u64))
                .read_to_end(&mut bytes)?;
            let complete = !active.contains_key(&run_id) && p.join("result.json").is_file();
            let identity = if complete {
                Some(hash_file(&path)?)
            } else {
                None
            };
            json!({"run_id":run_id,"stream":stream,"offset":offset,"next_offset":offset+bytes.len() as u64,"saved_bytes":available,"hex":hex::encode(bytes),"process_result_complete":complete,"saved_log_identity":identity,"claim":"bounded saved output; see process result for trace truncation"})
        }
        Request::Stop { run_id } => {
            let p = run_path(root, &run_id)?;
            if let Some(run) = active.remove(&run_id) {
                run.cancel.store(true, Ordering::Release);
                run.join
                    .join()
                    .map_err(|_| anyhow::anyhow!("worker run panicked"))?;
            }
            let state: Value = serde_json::from_slice(&fs::read(p.join("state.json"))?)?;
            json!({"run_id":run_id,"state":state,"reports_retained":true})
        }
        Request::Cleanup { run_id, result } => {
            let p = run_path(root, &run_id)?;
            ensure!(!active.contains_key(&run_id), "stop the run before cleanup");
            result.validate()?;
            if p.join("cleanup.json").exists() {
                let receipt = read_cleanup(&p, &run_id)?;
                ensure!(
                    receipt.acknowledged_result == result,
                    "cleanup result acknowledgement differs"
                );
                finish_token_cleanup(&p)?;
                return Ok(
                    json!({"run_id":run_id,"removed":true,"acknowledged_result":result,"token_receipt_retained":true,"published_inputs_retained":true}),
                );
            }
            ensure!(
                hash_file(&p.join("result.json"))? == result,
                "result acknowledgement does not match saved evidence"
            );
            // Reject unexpected files before removing any owned evidence.
            let owned_files = owned_run_files(&p)?;
            recover_run_cgroup(&p)?;
            if run_id.len() == 68 {
                let token = &run_id[4..];
                let found = lookup_token(root, token)?;
                let receipt = CleanupReceipt {
                    token: token.into(),
                    process_identity: serde_json::from_value(
                        found["token_process_identity"].clone(),
                    )?,
                    acknowledged_result: result.clone(),
                };
                isolation::save_json(&p.join("cleanup.json"), &receipt)?;
                finish_token_cleanup(&p)?;
                return Ok(
                    json!({"run_id":run_id,"removed":true,"acknowledged_result":result,"token_receipt_retained":true,"published_inputs_retained":true}),
                );
            }
            for file in owned_files {
                fs::remove_file(file)?;
            }
            fs::remove_dir(p)?;
            File::open(root.join("runs"))?.sync_all()?;
            json!({"run_id":run_id,"removed":true,"acknowledged_result":result,"published_inputs_retained":true})
        }
        Request::Shutdown => {
            for run in active.values() {
                run.cancel.store(true, Ordering::Release);
            }
            json!({"stopping":true})
        }
    })
}

pub fn upload(socket: &Path, source: &Path, expected: &ObjectId) -> Result<()> {
    ensure!(hash_file(source)? == *expected, "source identity mismatch");
    let mut offset = request(
        socket,
        &Request::Begin {
            object: expected.clone(),
        },
    )?["offset"]
        .as_u64()
        .context("worker offset missing")?;
    ensure!(offset <= expected.size, "worker offset exceeds object size");
    let mut file = File::open(source)?;
    use std::io::Seek;
    file.seek(std::io::SeekFrom::Start(offset))?;
    let mut buffer = vec![0; MAX_CHUNK];
    while offset < expected.size {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            bail!("source changed during transfer");
        }
        let end = offset + n as u64;
        let reply = request(
            socket,
            &Request::Chunk {
                object: expected.clone(),
                offset,
                hex: hex::encode(&buffer[..n]),
            },
        )?;
        ensure!(
            reply["offset"].as_u64() == Some(end),
            "worker acknowledged unexpected offset"
        );
        offset = end;
    }
    let reply = request(
        socket,
        &Request::Finish {
            object: expected.clone(),
        },
    )?;
    let verified: ObjectId = serde_json::from_value(reply["verified"].clone())?;
    ensure!(
        verified == *expected,
        "destination did not verify requested identity"
    );
    Ok(())
}

/// Bounded POSIX I/O for a channel already connected by the trusted parent.
/// This is process IPC, not a socket listener or an external network transport.
struct Ipc(File);
impl Ipc {
    fn wait(&self, events: i16) -> std::io::Result<()> {
        let mut descriptor = libc::pollfd {
            fd: self.0.as_raw_fd(),
            events,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut descriptor, 1, 5000) };
        if result < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if result == 0 {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        Ok(())
    }
}
impl Read for Ipc {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.wait(libc::POLLIN)?;
        self.0.read(buffer)
    }
}
impl Write for Ipc {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.wait(libc::POLLOUT)?;
        self.0.write(buffer)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

#[cfg(test)]
mod completed_response_tests {
    use super::*;

    fn wire(response: &Value) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(response).unwrap();
        bytes.push(b'\n');
        bytes
    }

    #[test]
    fn complete_error_retains_the_original_envelope() {
        let response = json!({
            "protocol":PROTOCOL_VERSION,"ok":false,"error":"selected run is absent",
            "detail":{"run_id":"run-missing","retryable":false}
        });
        let error = decode_response(&wire(&response)).unwrap_err();
        let completed = error.downcast_ref::<CompletedWorkerError>().unwrap();
        assert_eq!(completed.response(), &response);
        assert_eq!(
            completed.to_string(),
            "worker error: selected run is absent"
        );
    }

    #[test]
    fn incomplete_or_malformed_errors_are_never_completed_worker_errors() {
        let valid = json!({"protocol":PROTOCOL_VERSION,"ok":false,"error":"rejected"});
        let mut frames = Vec::new();
        for field in ["protocol", "ok", "error"] {
            let mut response = valid.clone();
            response.as_object_mut().unwrap().remove(field);
            frames.push(wire(&response));
        }
        for (field, value) in [
            ("protocol", json!("md-sim-worker/1")),
            ("ok", json!("false")),
            ("error", json!(null)),
            ("error", json!(17)),
            ("error", json!("")),
            ("error", json!("bad\0message")),
            (
                "error",
                json!("é".repeat(MAX_COMPLETED_ERROR_BYTES / 2 + 1)),
            ),
        ] {
            let mut response = valid.clone();
            response[field] = value;
            frames.push(wire(&response));
        }
        let complete = wire(&valid);
        frames.push(complete[..complete.len() - 1].to_vec());
        frames.push([complete.clone(), complete].concat());
        frames.push(b"invalid JSON\n".to_vec());
        for bytes in frames {
            let error = decode_response(&bytes).unwrap_err();
            assert!(!error.is::<CompletedWorkerError>());
        }
        let mut boundary = valid;
        boundary["error"] = json!("é".repeat(MAX_COMPLETED_ERROR_BYTES / 2));
        assert!(decode_response(&wire(&boundary))
            .unwrap_err()
            .is::<CompletedWorkerError>());
    }
}

#[cfg(test)]
mod ownership_tests {
    use super::*;
    #[test]
    fn device_channel_rejects_a_regular_file_without_changing_it() {
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"retained input").unwrap();
        assert!(request_device(&mut file, &Request::Probe).is_err());
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"retained input");
    }
    #[test]
    fn token_reserved_before_spec_commit_is_lost_and_never_launched() {
        let temp = tempfile::tempdir().unwrap();
        let token = "b2".repeat(32);
        let id = format!("run-{token}");
        let path = temp.path().join("runs").join(&id);
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("spec.tmp-123-0123456789abcdef01234567"),
            b"{\"schema_version\":",
        )
        .unwrap();
        recover_interrupted_runs(temp.path()).unwrap();
        let found = lookup_token(temp.path(), &token).unwrap();
        assert_eq!(found["found"], true);
        assert_eq!(found["state"], "lost");
        assert!(found["token_process_identity"].is_null());
        let result = hash_file(&path.join("result.json")).unwrap();
        let store =
            Arc::new(Store::open(&temp.path().join("store"), 8 * 1024 * 1024 * 1024).unwrap());
        let mut active = BTreeMap::new();
        let spec = ProcessSpec {
            support_virtualization: None,
            private_channel: None,
            stdin_stream: None,
            namespace_uid: 0,
            dashboard: None,
            loopback: false,
            schema_version: "2.0".into(),
            argv: vec!["/absent".into()],
            inputs: vec![],
            stdin_object: None,
            limits: isolation::Limits::default(),
            environment: BTreeMap::new(),
        };
        assert!(execute_request(
            spec,
            Some(&token),
            temp.path(),
            &store,
            &mut active,
            &WorkerLimits::default(),
            &mut None,
        )
        .is_err());
        assert!(active.is_empty());
        handle(
            Request::Cleanup {
                run_id: id.clone(),
                result: result.clone(),
            },
            temp.path(),
            &store,
            &mut active,
            &WorkerLimits::default(),
            &mut result_reader::Reader::default(),
            &mut None,
        )
        .unwrap();
        assert_eq!(
            lookup_token(temp.path(), &token).unwrap()["state"],
            "cleaned"
        );
        assert_eq!(
            read_cleanup(&path, &id).unwrap().acknowledged_result,
            result
        );
    }

    #[test]
    fn restart_finishes_only_acknowledged_token_cleanup() {
        let temp = tempfile::tempdir().unwrap();
        let token = "c3".repeat(32);
        let id = format!("run-{token}");
        let path = temp.path().join("runs").join(&id);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("result.json"), b"{\"status\":\"lost\"}\n").unwrap();
        let receipt = CleanupReceipt {
            token: token.clone(),
            process_identity: None,
            acknowledged_result: hash_file(&path.join("result.json")).unwrap(),
        };
        isolation::save_json(&path.join("cleanup.json"), &receipt).unwrap();
        fs::write(
            path.join("stdout.log"),
            b"partial run evidence already acknowledged",
        )
        .unwrap();
        recover_interrupted_runs(temp.path()).unwrap();
        assert_eq!(fs::read_dir(&path).unwrap().count(), 1);
        assert_eq!(
            lookup_token(temp.path(), &token).unwrap()["state"],
            "cleaned"
        );
    }

    #[test]
    fn loss_between_result_and_state_commits_keeps_result_identity() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp
            .path()
            .join("runs/run-1234567890abcdef1234567890abcdef");
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("result.json"), b"{\"status\":\"lost\"}\n").unwrap();
        let identity = hash_file(&path.join("result.json")).unwrap();
        let old_fragment = path.join(format!("state.tmp-{}", std::process::id()));
        fs::write(&old_fragment, b"{\"state\":").unwrap();
        recover_interrupted_runs(temp.path()).unwrap();
        assert_eq!(hash_file(&path.join("result.json")).unwrap(), identity);
        assert_eq!(fs::read(old_fragment).unwrap(), b"{\"state\":");
        let state: Value =
            serde_json::from_slice(&fs::read(path.join("state.json")).unwrap()).unwrap();
        assert_eq!(state["state"], "lost");
        assert_eq!(state["firmware_gate"], "not-run");
        recover_interrupted_runs(temp.path()).unwrap();
        assert_eq!(hash_file(&path.join("result.json")).unwrap(), identity);
    }
    #[test]
    fn worker_rejects_root_runs_and_lock_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let root = temp.path().join("root-link");
        std::os::unix::fs::symlink(&outside, &root).unwrap();
        assert!(serve_connections(&root, std::iter::empty::<Result<UnixStream>>()).is_err());
        assert!(!outside.join("runs").exists());
        let root = temp.path().join("worker");
        fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("runs")).unwrap();
        assert!(serve_connections(&root, std::iter::empty::<Result<UnixStream>>()).is_err());
        fs::remove_file(root.join("runs")).unwrap();
        let sentinel = outside.join("sentinel");
        fs::write(&sentinel, b"preserve").unwrap();
        std::os::unix::fs::symlink(&sentinel, root.join("worker.lock")).unwrap();
        assert!(serve_connections(&root, std::iter::empty::<Result<UnixStream>>()).is_err());
        assert_eq!(fs::read(sentinel).unwrap(), b"preserve");
    }
}

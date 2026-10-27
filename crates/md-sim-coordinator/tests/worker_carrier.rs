//! Real worker CLI, isolated guest and private socket transport. This is not
//! native firmware, mining, SSH, dashboard or external-host conformance.
use md_sim_coordinator::{ExecutionScope, LockedRun, SuiteIdentity};
use md_sim_runtime::{
    isolation::{Limits, Mount, ProcessSpec},
    private_channel::{Protocol, Spec},
    store::{hash_file, ObjectId, Store, MAX_CHUNK},
    worker::{process_identity, request_stream, Request, WorkerRootLease},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{fs::MetadataExt, net::UnixStream, process::CommandExt},
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[path = "support/elf.rs"]
mod elf;

fn frame(payload: &[u8]) -> Vec<u8> {
    let mut wire = ((payload.len() + 14) as u32).to_be_bytes().to_vec();
    wire.extend(0u8..14);
    wire.extend(payload);
    wire
}

#[test]
#[ignore = "isolated executable entry; invoked by actual_worker_cli_carrier"]
fn isolated_challenge_guest() {
    for fd in [4, 64, 65, 77] {
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EBADF)
        );
    }
    assert!(!Path::new(&std::env::var("HOST_MARKER").unwrap()).exists());
    assert!(!Path::new("/etc/passwd").exists());
    let mut socket = unsafe { File::from_raw_fd(3) };
    let inode = socket.metadata().unwrap().ino();
    for key in ["WORKER_CONTROL_INODE", "SUPPORT_ENDPOINT_INODE"] {
        assert_ne!(inode, std::env::var(key).unwrap().parse::<u64>().unwrap());
    }
    assert!(fs::read_link("/proc/self/fd/3")
        .unwrap()
        .to_string_lossy()
        .starts_with("socket:["));
    assert!(fs::read_dir("/proc/self/fd/3").is_err());
    for _ in 0..2 {
        let mut prefix = [0; 4];
        socket.read_exact(&mut prefix).unwrap();
        assert_eq!(u32::from_be_bytes(prefix), 46);
        let mut body = [0; 46];
        socket.read_exact(&mut body).unwrap();
        assert_eq!(&body[..14], &(0u8..14).collect::<Vec<_>>());
        let response = Sha256::digest(&body[14..]);
        socket.write_all(&frame(&response)).unwrap();
    }
    println!("ACTUAL_CARRIER_CHALLENGES_COMPLETE");
    if std::env::var("CANCEL_AFTER_CHALLENGES").unwrap() == "1" {
        loop {
            thread::sleep(Duration::from_millis(100));
        }
    }
}

fn duplicate_high(file: &impl AsRawFd) -> File {
    let fd = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 64) };
    assert!(fd >= 64);
    unsafe { File::from_raw_fd(fd) }
}

struct WorkerProcess {
    child: Child,
    control: UnixStream,
    stderr: PathBuf,
}
impl WorkerProcess {
    fn request(&mut self, request: &Request) -> Value {
        request_stream(&mut self.control, request).unwrap_or_else(|error| {
            panic!(
                "worker request failed: {error:#}; stderr: {}",
                fs::read_to_string(&self.stderr).unwrap_or_default()
            )
        })
    }
    fn result(&mut self, run: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(value) =
                request_stream(&mut self.control, &Request::Result { run_id: run.into() })
            {
                return value;
            }
            assert!(Instant::now() < deadline, "result deadline");
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "worker exited before result"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn finish(&mut self) {
        self.request(&Request::Shutdown);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "worker shutdown: {status}; {}",
                    fs::read_to_string(&self.stderr).unwrap()
                );
                return;
            }
            assert!(Instant::now() < deadline, "worker ignored shutdown");
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for WorkerProcess {
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn exchange(file: &mut File, send: &[u8], expected: usize) -> Vec<u8> {
    let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
        0
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut written = 0;
    let mut received = Vec::new();
    loop {
        assert!(Instant::now() < deadline, "support endpoint deadline");
        if written < send.len() {
            match file.write(&send[written..(written + 3).min(send.len())]) {
                Ok(n) => written += n,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => panic!("support write: {error}"),
            }
        }
        let mut buffer = [0; 512];
        match file.read(&mut buffer) {
            Ok(0) => return received,
            Ok(n) => received.extend_from_slice(&buffer[..n]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("support read: {error}"),
        }
        assert!(received.len() <= 512, "unexpected support response extent");
        if written == send.len() && received.len() >= expected {
            return received;
        }
        thread::sleep(Duration::from_millis(1));
    }
}

fn upload(worker: &mut WorkerProcess, store: &Store, object: &ObjectId) {
    let begin = worker.request(&Request::Begin {
        object: object.clone(),
    });
    let mut offset = begin["offset"].as_u64().unwrap();
    while offset < object.size {
        let bytes = store.read_chunk(object, offset, MAX_CHUNK).unwrap();
        let response = worker.request(&Request::Chunk {
            object: object.clone(),
            offset,
            hex: hex::encode(&bytes),
        });
        offset += bytes.len() as u64;
        assert_eq!(response["offset"], offset);
    }
    assert_eq!(
        worker.request(&Request::Finish {
            object: object.clone()
        })["verified"],
        serde_json::to_value(object).unwrap()
    );
}

fn complete_result(worker: &mut WorkerProcess, run: &str, identity: &ObjectId) -> Value {
    let mut bytes = Vec::new();
    while (bytes.len() as u64) < identity.size {
        let response = worker.request(&Request::ResultData {
            run_id: run.into(),
            result: identity.clone(),
            offset: bytes.len() as u64,
            count: MAX_CHUNK,
        });
        let part = hex::decode(response["hex"].as_str().unwrap()).unwrap();
        assert!(!part.is_empty());
        bytes.extend(part);
        assert!(bytes.len() as u64 <= identity.size);
    }
    assert_eq!(hex::encode(Sha256::digest(&bytes)), identity.sha256);
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn actual_worker_cli_carrier() {
    let temporary = tempfile::Builder::new()
        .prefix("md-sim-cli-carrier-")
        .tempdir()
        .unwrap();
    let client_store = Store::open(&temporary.path().join("client-store"), 128 << 20).unwrap();
    let mut inputs = elf::inputs(
        &client_store,
        &std::env::current_exe().unwrap(),
        "/challenge-guest",
    );
    let support = temporary.path().join("support-contract.json");
    fs::write(
        &support,
        b"{\"scope\":\"private test peer, not a support VM\"}\n",
    )
    .unwrap();
    let support_identity = hash_file(&support).unwrap();
    client_store.import(&support, &support_identity).unwrap();
    inputs.push(Mount {
        object: support_identity.clone(),
        guest_path: "/support-contract.json".into(),
    });
    let marker = temporary.path().join("host-only-marker");
    fs::write(&marker, b"must never enter the guest").unwrap();
    let secret = File::open(&marker).unwrap();
    let binary = Path::new(env!("CARGO_BIN_EXE_md-sim-worker-ipc"));
    let worker_build = hash_file(binary).unwrap();

    for (case_name, cancel, leased, peer_failure) in [
        ("complete", false, false, None),
        ("cancel", true, false, None),
        ("lease-complete", false, true, None),
        ("lease-peer-eof", true, true, Some(false)),
        ("lease-peer-invalid", true, true, Some(true)),
    ] {
        let case = temporary.path().join(case_name);
        fs::create_dir(&case).unwrap();
        let root = case.join("worker");
        let (control_client, control_server) = UnixStream::pair().unwrap();
        let control_inode = File::from(OwnedFd::from(control_server.try_clone().unwrap()))
            .metadata()
            .unwrap()
            .ino();
        let (support_client, support_server) = UnixStream::pair().unwrap();
        let support_inode = File::from(OwnedFd::from(support_server.try_clone().unwrap()))
            .metadata()
            .unwrap()
            .ino();
        let policy = Spec {
            protocol: Protocol::QemuEthernetV1,
            guest_fd: 3,
            max_frame_bytes: 2048,
            max_wire_bytes: 65536,
            max_packets: 64,
        };
        let process = ProcessSpec {
            support_virtualization: None,
            schema_version: "2.0".into(),
            namespace_uid: 0,
            argv: vec![
                "/challenge-guest".into(),
                "--ignored".into(),
                "--exact".into(),
                "isolated_challenge_guest".into(),
                "--nocapture".into(),
                "--test-threads=1".into(),
            ],
            inputs: inputs.clone(),
            limits: Limits {
                wall_seconds: 12,
                cpu_seconds: 6,
                memory_bytes: 512 << 20,
                processes: 64,
                trace_bytes: 16384,
                ..Limits::default()
            },
            environment: BTreeMap::from([
                ("HOST_MARKER".into(), marker.to_str().unwrap().into()),
                ("WORKER_CONTROL_INODE".into(), control_inode.to_string()),
                ("SUPPORT_ENDPOINT_INODE".into(), support_inode.to_string()),
                (
                    "CANCEL_AFTER_CHALLENGES".into(),
                    if cancel { "1" } else { "0" }.into(),
                ),
            ]),
            loopback: false,
            dashboard: None,
            stdin_object: None,
            stdin_stream: None,
            private_channel: Some(policy),
        };
        let identity = process_identity(&process).unwrap();
        let policy_object = hash_file(&support).unwrap();
        let doctor = md_sim_runtime::doctor::inspect();
        let lock = LockedRun {
            name: format!("actual-provider-{case_name}"),
            scope: ExecutionScope::ProcessPrimitive {
                description: "Actual isolated carrier guest and live provider observation".into(),
            },
            process: Some(process.clone()),
            suite: SuiteIdentity {
                id: "worker-carrier".into(),
                version: "provider-live-1".into(),
                suite: support_identity.clone(),
                verifier: support_identity.clone(),
                required_gates: ["actual-carrier-lifecycle".into()].into(),
            },
            inputs: inputs
                .iter()
                .map(|mount| (mount.guest_path.clone(), mount.object.clone()))
                .collect(),
            backend_tools: ["bubblewrap", "unshare", "prlimit"]
                .into_iter()
                .map(|name| {
                    (
                        name.into(),
                        serde_json::from_value(doctor["tools"][name]["identity"].clone()).unwrap(),
                    )
                })
                .collect(),
            policies: json!({"carrier_dashboard":policy_object}),
            capability_limits: vec!["Support test process; no miner or dashboard claim".into()],
        };
        lock.validate().unwrap();
        let config = case.join("provider.json");
        let mut configuration = json!({"schema":"md-sim-worker-private-carrier/1","process":process,"support_identity":support_identity,"handoff_seconds":30,"wall_seconds":12});
        if leased {
            configuration["lease"] = json!({"locked_run":lock,"policy":policy_object});
        }
        fs::write(&config, serde_json::to_vec_pretty(&configuration).unwrap()).unwrap();
        let control_source = duplicate_high(&control_server);
        let support_source = duplicate_high(&support_server);
        let secret_source = duplicate_high(&secret);
        let (control_fd, support_fd, secret_fd) = (
            control_source.as_raw_fd(),
            support_source.as_raw_fd(),
            secret_source.as_raw_fd(),
        );
        let parent = std::process::id() as libc::pid_t;
        let stderr = case.join("worker.stderr");
        let mut command = Command::new(binary);
        command
            .arg(&root)
            .arg("3")
            .arg("--private-carrier")
            .arg(&config)
            .arg("4")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(File::create(&stderr).unwrap());
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::getppid() != parent {
                    return Err(std::io::Error::from_raw_os_error(libc::ECANCELED));
                }
                for (source, destination) in [(control_fd, 3), (support_fd, 4), (secret_fd, 77)] {
                    if libc::dup2(source, destination) < 0
                        || libc::fcntl(destination, libc::F_SETFD, 0) < 0
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let mut worker = WorkerProcess {
            child: command.spawn().unwrap(),
            control: control_client,
            stderr,
        };
        drop((
            command,
            control_server,
            support_server,
            control_source,
            support_source,
            secret_source,
        ));
        let mut peer = Some(File::from(OwnedFd::from(support_client)));
        assert_eq!(
            worker.request(&Request::Probe)["worker_build"],
            serde_json::to_value(&worker_build).unwrap()
        );
        assert!(WorkerRootLease::acquire(&root).is_err());
        for input in &inputs {
            upload(&mut worker, &client_store, &input.object);
        }
        let mut wrong = process.clone();
        wrong.private_channel.as_mut().unwrap().max_packets += 1;
        assert!(
            request_stream(&mut worker.control, &Request::Execute { spec: wrong })
                .unwrap_err()
                .to_string()
                .contains("identity mismatch")
        );
        assert_eq!(fs::read_dir(root.join("runs")).unwrap().count(), 0);
        let token = if cancel { "b6" } else { "a5" }.repeat(32);
        let request = Request::ExecuteToken {
            token: token.clone(),
            spec: process.clone(),
        };
        let accepted = worker.request(&request);
        let run = accepted["run_id"].as_str().unwrap();
        assert_eq!(worker.request(&request)["run_id"], run);
        let mut leases = Vec::new();
        let challenge = "c3".repeat(32);
        let query = Request::PrivateChannelStatus {
            token: token.clone(),
            challenge: challenge.clone(),
        };
        if leased {
            assert!(request_stream(
                &mut worker.control,
                &Request::PrivateChannelStatus {
                    token: "d4".repeat(32),
                    challenge: challenge.clone()
                }
            )
            .is_err());
            for bad in [
                String::new(),
                "C".repeat(64),
                "a".repeat(63),
                "x".repeat(64),
            ] {
                assert!(request_stream(
                    &mut worker.control,
                    &Request::PrivateChannelStatus {
                        token: token.clone(),
                        challenge: bad
                    }
                )
                .is_err());
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let lease = worker.request(&query);
                assert_eq!(lease["schema"], "md-sim-carrier-provider-lease/1");
                assert_eq!(
                    lease["configuration"],
                    serde_json::to_value(hash_file(&config).unwrap()).unwrap()
                );
                assert_eq!(
                    lease["identity"],
                    json!({"locked_run":lock.object().unwrap(),"process":identity,"policy":policy_object,"support":support_identity,"token":token,"run_id":run})
                );
                assert_eq!(lease["challenge"], challenge);
                if let Some(previous) = leases.last() {
                    let previous: &Value = previous;
                    assert!(
                        lease["sequence"].as_u64().unwrap()
                            > previous["sequence"].as_u64().unwrap()
                    );
                }
                let active = lease["state"] == "active";
                assert!(active || lease["state"] == "waiting");
                leases.push(lease);
                if active {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "actual provider handoff deadline"
                );
                thread::sleep(Duration::from_millis(10));
            }
        } else {
            assert!(request_stream(&mut worker.control, &query)
                .unwrap_err()
                .to_string()
                .contains("lease is not configured"));
        }
        let mut sent = Vec::new();
        let mut expected = Vec::new();
        let mut random = File::open("/dev/urandom").unwrap();
        for _ in 0..2 {
            let mut challenge = [0; 32];
            random.read_exact(&mut challenge).unwrap();
            sent.extend(frame(&challenge));
            expected.extend(frame(&Sha256::digest(challenge)));
        }
        assert_eq!(
            exchange(peer.as_mut().unwrap(), &sent, expected.len()),
            expected
        );
        if cancel {
            // Response delivery can precede the guest's following stdout
            // marker. Observe the saved marker before Stop so cancellation
            // cannot race the evidence assertion below.
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let log = worker.request(&Request::Logs {
                    run_id: run.into(),
                    stream: md_sim_runtime::worker::LogStream::Stdout,
                    offset: 0,
                    count: 16384,
                });
                let bytes = hex::decode(log["hex"].as_str().unwrap()).unwrap();
                if String::from_utf8(bytes)
                    .unwrap()
                    .contains("ACTUAL_CARRIER_CHALLENGES_COMPLETE")
                {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "guest completion marker deadline"
                );
                thread::sleep(Duration::from_millis(10));
            }
            if let Some(invalid) = peer_failure {
                assert_eq!(
                    worker.request(&Request::Status { run_id: run.into() })["state"]["state"],
                    "booting"
                );
                if invalid {
                    assert!(
                        exchange(peer.as_mut().unwrap(), &u32::MAX.to_be_bytes(), usize::MAX)
                            .is_empty()
                    );
                } else {
                    // Closing the owned endpoint creates actual peer EOF.
                    // No optional shutdown syscall or retained alias is used.
                    drop(peer.take());
                }
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    let lease = worker.request(&query);
                    assert!(
                        lease["sequence"].as_u64().unwrap()
                            > leases.last().unwrap()["sequence"].as_u64().unwrap()
                    );
                    let terminal = lease["state"] == if invalid { "failed" } else { "closed" };
                    leases.push(lease);
                    if terminal {
                        break;
                    }
                    assert!(
                        Instant::now() < deadline,
                        "provider did not observe actual peer failure"
                    );
                    thread::sleep(Duration::from_millis(10));
                }
                // The guest deliberately remains alive after its exchanges.
                // Provider EOF/failure must not be inferred from this state.
                assert_eq!(
                    worker.request(&Request::Status { run_id: run.into() })["state"]["state"],
                    "booting"
                );
            }
            worker.request(&Request::Stop { run_id: run.into() });
        }
        if let Some(peer) = &mut peer {
            assert!(exchange(peer, &[], usize::MAX).is_empty());
        }
        let summary = worker.result(run);
        let result_identity: ObjectId =
            serde_json::from_value(summary["result_identity"].clone()).unwrap();
        let result = complete_result(&mut worker, run, &result_identity);
        assert_eq!(result["trace_truncated"], false);
        assert_eq!(
            result["status"],
            if cancel { "cancelled" } else { "exited" }
        );
        if !cancel {
            assert_eq!(result["exit_code"], 0);
        }
        assert!(
            String::from_utf8(hex::decode(result["stdout"].as_str().unwrap()).unwrap())
                .unwrap()
                .contains("ACTUAL_CARRIER_CHALLENGES_COMPLETE")
        );
        assert_eq!(
            result["private_channel"]["guest_to_peer"]["forwarded_sha256"],
            hex::encode(Sha256::digest(&expected))
        );
        assert_eq!(
            result["private_channel"]["peer_to_guest"]["forwarded_sha256"],
            hex::encode(Sha256::digest(&sent))
        );
        assert!(request_stream(
            &mut worker.control,
            &Request::Execute {
                spec: process.clone()
            }
        )
        .unwrap_err()
        .to_string()
        .contains("already consumed"));
        worker.request(&Request::Cleanup {
            run_id: run.into(),
            result: result_identity,
        });
        assert_eq!(worker.request(&request)["state"], "cleaned");
        if leased {
            let lease = worker.request(&query);
            assert!(
                lease["sequence"].as_u64().unwrap()
                    > leases.last().unwrap()["sequence"].as_u64().unwrap()
            );
            assert_eq!(
                lease["state"],
                if peer_failure == Some(true) {
                    "failed"
                } else {
                    "closed"
                }
            );
            leases.push(lease);
        }
        assert!(WorkerRootLease::acquire(&root).is_err());
        if peer_failure == Some(true) {
            worker.request(&Request::Shutdown);
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Some(status) = worker.child.try_wait().unwrap() {
                    assert!(
                        !status.success(),
                        "failed provider became a successful CLI exit"
                    );
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "failed provider shutdown deadline"
                );
                thread::sleep(Duration::from_millis(10));
            }
        } else {
            worker.finish();
        }
        let report: Value =
            serde_json::from_slice(&fs::read(root.join("private-carrier.json")).unwrap()).unwrap();
        assert_eq!(
            report["configuration"],
            serde_json::to_value(hash_file(&config).unwrap()).unwrap()
        );
        assert_eq!(report["process"], serde_json::to_value(&identity).unwrap());
        assert_eq!(
            report["support_identity"],
            serde_json::to_value(&support_identity).unwrap()
        );
        assert_eq!(
            report["receipt"]["end"],
            match peer_failure {
                Some(true) => "failed",
                Some(false) => "peer-eof",
                None => "guest-eof",
            }
        );
        assert_eq!(
            report["receipt"]["error"].is_null(),
            peer_failure != Some(true)
        );
        assert_eq!(
            report["receipt"]["guest_to_peer"]["forwarded_sha256"],
            hex::encode(Sha256::digest(&expected))
        );
        assert_eq!(
            report["receipt"]["peer_to_guest"]["forwarded_sha256"],
            hex::encode(Sha256::digest(&sent))
        );
        let report_identity = hash_file(&root.join("private-carrier.json")).unwrap();
        let lease = WorkerRootLease::acquire(&root).unwrap();
        assert_eq!(
            report_identity,
            hash_file(&lease.path().join("private-carrier.json")).unwrap()
        );
        drop(lease);
        let retained_store = Store::open_with_limits(
            &root.join("store"),
            md_sim_runtime::worker::WorkerLimits::default().store,
        )
        .unwrap();
        for input in &inputs {
            retained_store.verified_path(&input.object).unwrap();
        }
        println!(
            "ACTUAL_WORKER_CLI_CARRIER_EVIDENCE {}",
            json!({"case":case_name,"cancelled":cancel,"worker_build":worker_build,"process":process,"accepted":accepted,"provider_observations":leases,"request_wire":hex::encode(sent),"response_wire":hex::encode(expected),"result":result,"provider_report":report,"provider_report_identity":report_identity,"cleanup_token_retained":true,"input_objects_retained":true,"scope":"Actual local worker executable and isolated test guest. Support endpoint is a test process socket, not a support VM or external host."})
        );
    }
}

use md_sim_runtime::{
    isolation::{execute, Limits, Mount, ProcessSpec},
    store::{hash_file, Store},
    worker::{
        request_stream, serve_connections, serve_connections_with_limits, LogStream, Request,
        WorkerLimits,
    },
};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{atomic::AtomicBool, mpsc, Arc},
    thread,
    time::{Duration, Instant},
};

// These are installed host test tools, never downloaded firmware. ldd is used
// only on this fixed list. Never use ldd to inspect an untrusted firmware ELF.
fn input_closure(store: &Store, programs: &[&str]) -> Vec<Mount> {
    let mut files = BTreeMap::<String, PathBuf>::new();
    for path in programs {
        files.insert(path.to_string(), PathBuf::from(path));
        let output = std::process::Command::new("/usr/bin/ldd")
            .arg(path)
            .output()
            .unwrap();
        assert!(output.status.success(), "ldd failed for host test tool");
        for field in String::from_utf8(output.stdout).unwrap().split_whitespace() {
            if field.starts_with('/') {
                files.insert(field.into(), PathBuf::from(field));
            }
        }
    }
    files
        .into_iter()
        .map(|(guest_path, source)| {
            let object = hash_file(&source).unwrap();
            store.import(&source, &object).unwrap();
            Mount { object, guest_path }
        })
        .collect()
}

#[test]
fn guest_process_bound_is_preserved_when_host_uid_exceeds_it() {
    struct OwnedProcesses(Vec<std::process::Child>);
    impl Drop for OwnedProcesses {
        fn drop(&mut self) {
            for child in &mut self.0 {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    let mut owned = OwnedProcesses(Vec::new());
    for _ in 0..24 {
        owned.0.push(
            std::process::Command::new("/usr/bin/sleep")
                .arg("30")
                .spawn()
                .unwrap(),
        );
    }
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(&tmp.path().join("store"), 16 << 20).unwrap();
    let mut spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        schema_version: "2.0".into(),
        argv: vec!["/usr/bin/sh".into(), "-c".into(),
            "test ! -e /home/user && test ! -e /etc/passwd && /usr/bin/true && while read a b soft hard rest; do if [ \"$a $b\" = 'Max processes' ]; then test \"$soft\" = 16 && test \"$hard\" = 16 || exit 2; echo limit-16; fi; done < /proc/self/limits".into()],
        inputs: input_closure(&store, &["/usr/bin/sh", "/usr/bin/true"]),
        stdin_object: None,
        environment: BTreeMap::new(),
        loopback: false,
        dashboard: None,
        limits: Limits { processes: 16, ..Limits::default() },
    };
    for uid in [0, 1000] {
        spec.namespace_uid = uid;
        let result = execute(&store, &spec, Arc::new(AtomicBool::new(false))).unwrap();
        assert_eq!(
            result.exit_code,
            Some(0),
            "{}",
            String::from_utf8_lossy(&hex::decode(&result.stderr).unwrap())
        );
        assert_eq!(hex::decode(result.stdout).unwrap(), b"limit-16\n");
        assert!(!result.trace_truncated);
        assert_eq!(
            result.tools["prlimit"],
            hash_file(std::path::Path::new("/usr/bin/prlimit")).unwrap()
        );
    }
}

#[test]
fn nonroot_namespace_identity_keeps_isolation_and_the_default_wire_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(&tmp.path().join("store"), 16 << 20).unwrap();
    let inputs = input_closure(&store, &["/usr/bin/sh", "/usr/bin/id"]);
    let mut spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None, stdin_stream: None,
        namespace_uid: 0,
        schema_version: "2.0".into(),
        argv: vec!["/usr/bin/sh".into(), "-c".into(),
            "test ! -e /home/user && test ! -e /etc/passwd && ! (echo bad > /escape) && echo scratch > /work/test && /usr/bin/id -u && /usr/bin/id -g && while read key value rest; do case $key in CapEff:|CapPrm:|CapBnd:|NoNewPrivs:) printf '%s %s\\n' \"$key\" \"$value\";; esac; done < /proc/self/status && while read inside outside count; do printf 'uid-map %s %s %s\\n' \"$inside\" \"$outside\" \"$count\"; done < /proc/self/uid_map".into()],
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        loopback: false,
        dashboard: None,
        limits: Limits::default(),
    };
    let legacy = serde_json::to_value(&spec).unwrap();
    assert!(legacy.get("namespace_uid").is_none());
    let decoded: ProcessSpec = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(decoded.namespace_uid, 0);
    assert_eq!(serde_json::to_value(decoded).unwrap(), legacy);
    spec.namespace_uid = 65535;
    assert!(spec.validate().is_err());
    spec.namespace_uid = 1000;
    assert_eq!(serde_json::to_value(&spec).unwrap()["namespace_uid"], 1000);
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false))).unwrap();
    assert_eq!(
        result.exit_code,
        Some(0),
        "{}",
        String::from_utf8_lossy(&hex::decode(&result.stderr).unwrap())
    );
    assert!(!result.trace_truncated);
    let actual = String::from_utf8(hex::decode(result.stdout).unwrap()).unwrap();
    let (privileges, mapping) = actual.split_once("uid-map ").unwrap();
    assert_eq!(privileges, "1000\n1000\nCapPrm: 0000000000000000\nCapEff: 0000000000000000\nCapBnd: 0000000000000000\nNoNewPrivs: 1\n");
    let fields: Vec<_> = mapping.split_whitespace().collect();
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0], "1000");
    assert_eq!(fields[2], "1");
    // The middle number belongs to Bubblewrap's parent namespace. It is not
    // a global host UID, and can be0 when Bubblewrap adds an intermediate map.
    fields[1].parse::<u32>().unwrap();
}

#[test]
fn isolated_process_has_only_locked_inputs_and_bounded_trace() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(&tmp.path().join("store"), 16 * 1024 * 1024).unwrap();
    let inputs = input_closure(&store, &["/usr/bin/sh"]);
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None, stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false, stdin_object: None,schema_version:"2.0".into(),argv:vec!["/usr/bin/sh".into(),"-c".into(),"test ! -e /home/user && test ! -e /etc/passwd && ! (echo bad > /usr/bin/sh) && ! (echo bad > /escape) && ! (echo bad > /usr/unbounded) && echo scratch > /tmp/allowed && echo scratch > /work/allowed && echo isolated".into()],inputs,limits:Limits::default(),environment:BTreeMap::new()};
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false))).unwrap();
    assert_eq!(
        result.exit_code,
        Some(0),
        "{:?}",
        String::from_utf8(hex::decode(&result.stderr).unwrap())
    );
    assert_eq!(hex::decode(result.stdout).unwrap(), b"isolated\n");
    assert!(!result.trace_truncated);
    let mut noisy = spec.clone();
    noisy.argv[2] =
        "i=0; while [ $i -lt 100 ]; do echo abcdefghijklmnopqrstuvwxyz; i=$((i+1)); done".into();
    noisy.limits.trace_bytes = 64;
    let result = execute(&store, &noisy, Arc::new(AtomicBool::new(false))).unwrap();
    assert!(result.trace_truncated);
    assert_eq!(hex::decode(result.stdout).unwrap().len(), 64);
    assert!(hex::decode(result.stderr).unwrap().is_empty());

    // A complete asymmetric trace fits the aggregate allowance. Check both
    // live files against the returned bytes, not only their lengths.
    noisy.argv[2] =
        "printf 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRS'; printf '0123456789' >&2".into();
    let result = md_sim_runtime::isolation::execute_with_trace(
        &store,
        &noisy,
        Arc::new(AtomicBool::new(false)),
        Some(md_sim_runtime::isolation::TraceFiles {
            stdout: fs::File::create(tmp.path().join("stdout")).unwrap(),
            stderr: fs::File::create(tmp.path().join("stderr")).unwrap(),
        }),
    )
    .unwrap();
    assert_eq!(result.exit_code, Some(0));
    assert!(!result.trace_truncated);
    let stdout = hex::decode(result.stdout).unwrap();
    let stderr = hex::decode(result.stderr).unwrap();
    assert_eq!(stdout, b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRS");
    assert_eq!(stderr, b"0123456789");
    assert_eq!(fs::read(tmp.path().join("stdout")).unwrap(), stdout);
    assert_eq!(fs::read(tmp.path().join("stderr")).unwrap(), stderr);
}

#[test]
fn immutable_stdin_is_a_pipe_and_delivers_exact_binary_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(&tmp.path().join("store"), 16 * 1024 * 1024).unwrap();
    let inputs = input_closure(&store, &["/usr/bin/sh", "/usr/bin/cat"]);
    let source = tmp.path().join("stdin.bin");
    let bytes: Vec<u8> = (0..=255).cycle().take(32768).collect();
    fs::write(&source, &bytes).unwrap();
    let object = hash_file(&source).unwrap();
    store.import(&source, &object).unwrap();
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec![
            "/usr/bin/sh".into(),
            "-c".into(),
            "test -p /proc/self/fd/0 && exec /usr/bin/cat".into(),
        ],
        inputs,
        stdin_object: Some(object.clone()),
        limits: Limits::default(),
        environment: BTreeMap::new(),
    };
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false))).unwrap();
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(hex::decode(result.stdout).unwrap(), bytes);
    assert!(!result.trace_truncated);
    assert_eq!(
        hash_file(&store.verified_path(&object).unwrap()).unwrap(),
        object
    );
    let mut invalid = spec;
    invalid.stdin_object.as_mut().unwrap().sha256 = "0".repeat(64);
    assert!(execute(&store, &invalid, Arc::new(AtomicBool::new(false))).is_err());
}

#[test]
fn streamed_stdin_is_a_bounded_recorded_pipe_and_legacy_execution_stays_closed() {
    use md_sim_runtime::isolation::{execute_with_streamed_input, StdinStream};
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(&tmp.path().join("store"), 16 << 20).unwrap();
    let mut spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: Some(StdinStream {
            protocol: "test-receipt/1".into(),
            max_bytes: 65536,
        }),
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        stdin_object: None,
        schema_version: "2.0".into(),
        environment: BTreeMap::new(),
        argv: vec![
            "/usr/bin/sh".into(),
            "-c".into(),
            "test -p /proc/self/fd/0 && exec /usr/bin/cat".into(),
        ],
        inputs: input_closure(&store, &["/usr/bin/sh", "/usr/bin/cat", "/usr/bin/sleep"]),
        limits: Limits::default(),
    };
    assert!(execute(&store, &spec, Arc::new(AtomicBool::new(false))).is_err());
    let bytes: Vec<u8> = (0..=255).cycle().take(32768).collect();
    let (tx, rx) = mpsc::channel();
    tx.send(bytes[..123].to_vec()).unwrap();
    tx.send(bytes[123..].to_vec()).unwrap();
    drop(tx);
    let result =
        execute_with_streamed_input(&store, &spec, Arc::new(AtomicBool::new(false)), None, rx)
            .unwrap();
    assert_eq!(result.status, "exited");
    assert_eq!(result.exit_code, Some(0));
    assert_eq!(hex::decode(result.stdout).unwrap(), bytes);
    let receipt = result.stdin_stream.unwrap();
    assert_eq!(receipt["sent_hex"], hex::encode(&bytes));
    assert!(receipt["error"].is_null());

    spec.stdin_stream.as_mut().unwrap().max_bytes = 3;
    let (tx, rx) = mpsc::channel();
    tx.send(b"oversized".to_vec()).unwrap();
    drop(tx);
    let result =
        execute_with_streamed_input(&store, &spec, Arc::new(AtomicBool::new(false)), None, rx)
            .unwrap();
    assert_eq!(result.status, "input-failed");
    assert_eq!(result.stdin_stream.as_ref().unwrap()["sent_hex"], "");
    assert!(result.stdin_stream.unwrap()["error"]
        .as_str()
        .unwrap()
        .contains("byte limit"));

    spec.stdin_stream = None;
    assert!(serde_json::to_value(&spec)
        .unwrap()
        .get("stdin_stream")
        .is_none());
    let (_tx, rx) = mpsc::channel();
    assert!(
        execute_with_streamed_input(&store, &spec, Arc::new(AtomicBool::new(false)), None, rx)
            .is_err()
    );
}

#[test]
fn unread_or_idle_streamed_stdin_cannot_hold_the_namespace_after_timeout() {
    use md_sim_runtime::isolation::{execute_with_streamed_input, StdinStream};
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(&tmp.path().join("store"), 16 << 20).unwrap();
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: Some(StdinStream {
            protocol: "test-receipt/1".into(),
            max_bytes: 65536,
        }),
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        stdin_object: None,
        schema_version: "2.0".into(),
        environment: BTreeMap::new(),
        argv: vec![
            "/usr/bin/sh".into(),
            "-c".into(),
            "/usr/bin/sleep 30 & wait".into(),
        ],
        inputs: input_closure(&store, &["/usr/bin/sh", "/usr/bin/sleep"]),
        limits: Limits {
            wall_seconds: 1,
            ..Limits::default()
        },
    };
    for send in [false, true] {
        let (tx, rx) = mpsc::channel();
        if send {
            tx.send(vec![42; 65536]).unwrap();
        }
        let start = Instant::now();
        let result =
            execute_with_streamed_input(&store, &spec, Arc::new(AtomicBool::new(false)), None, rx)
                .unwrap();
        assert_eq!(result.status, "timed-out");
        assert!(start.elapsed() < Duration::from_secs(5));
        drop(tx);
    }
}

#[test]
fn unread_stdin_does_not_prevent_timeout_or_cancellation() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(&tmp.path().join("store"), 16 * 1024 * 1024).unwrap();
    let inputs = input_closure(&store, &["/usr/bin/sh", "/usr/bin/sleep"]);
    let source = tmp.path().join("stdin.bin");
    fs::write(&source, vec![42; 1024 * 1024]).unwrap();
    let object = hash_file(&source).unwrap();
    store.import(&source, &object).unwrap();
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec![
            "/usr/bin/sh".into(),
            "-c".into(),
            "/usr/bin/sleep 30 & wait".into(),
        ],
        inputs,
        stdin_object: Some(object),
        limits: Limits {
            wall_seconds: 1,
            ..Limits::default()
        },
        environment: BTreeMap::new(),
    };
    for (cancelled, expected) in [(false, "timed-out"), (true, "cancelled")] {
        let start = Instant::now();
        let result = execute(&store, &spec, Arc::new(AtomicBool::new(cancelled))).unwrap();
        assert_eq!(result.status, expected);
        assert!(start.elapsed() < Duration::from_secs(5));
    }
}

#[test]
fn execution_token_survives_reconnect_restart_and_acknowledged_cleanup() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("worker");
    let client_store = Store::open(&tmp.path().join("client-store"), 16 * 1024 * 1024).unwrap();
    let inputs = input_closure(&client_store, &["/usr/bin/sh"]);
    // Seed verified files before opening the worker's exclusive store handle.
    let worker_store = Store::open(&root.join("store"), 8 * 1024 * 1024 * 1024).unwrap();
    for input in &inputs {
        worker_store
            .import(
                &client_store.verified_path(&input.object).unwrap(),
                &input.object,
            )
            .unwrap();
    }
    drop(worker_store);
    let token = "a1".repeat(32);
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec!["/usr/bin/sh".into(), "-c".into(), "echo once".into()],
        inputs,
        stdin_object: None,
        limits: Limits::default(),
        environment: BTreeMap::new(),
    };
    let mut acknowledged = None;
    for phase in 0..3 {
        let (send, receive) = mpsc::channel();
        let r = root.clone();
        let worker = thread::spawn(move || serve_connections(&r, receive).unwrap());
        let request = |req: &Request| {
            let (mut client, server) = std::os::unix::net::UnixStream::pair().unwrap();
            send.send(Ok(server)).unwrap();
            request_stream(&mut client, req)
        };
        let lookup = request(&Request::LookupToken {
            token: token.clone(),
        })
        .unwrap();
        assert_eq!(lookup["found"], phase != 0);
        if phase == 0 {
            // Send a complete start request, then disconnect before its reply.
            // The next connection must recover the same first execution.
            use std::io::Write;
            let (client, server) = std::os::unix::net::UnixStream::pair().unwrap();
            send.send(Ok(server)).unwrap();
            let descriptor: std::os::fd::OwnedFd = client.into();
            let mut connection = fs::File::from(descriptor);
            serde_json::to_writer(
                &mut connection,
                &Request::ExecuteToken {
                    token: token.clone(),
                    spec: spec.clone(),
                },
            )
            .unwrap();
            connection.write_all(b"\n").unwrap();
            drop(connection);
            assert_eq!(
                request(&Request::LookupToken {
                    token: token.clone()
                })
                .unwrap()["found"],
                true
            );
        }
        let first = request(&Request::ExecuteToken {
            token: token.clone(),
            spec: spec.clone(),
        })
        .unwrap();
        let run_id = first["run_id"].as_str().unwrap().to_string();
        assert_eq!(run_id, format!("run-{token}"));
        let retry = request(&Request::ExecuteToken {
            token: token.clone(),
            spec: spec.clone(),
        })
        .unwrap();
        assert_eq!(first["run_id"], retry["run_id"]);
        assert_eq!(fs::read_dir(root.join("runs")).unwrap().count(), 1);
        let mut different = spec.clone();
        different.argv[2] = "echo twice".into();
        assert!(request(&Request::ExecuteToken {
            token: token.clone(),
            spec: different
        })
        .is_err());
        if phase == 0 {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Ok(result) = request(&Request::Result {
                    run_id: run_id.clone(),
                }) {
                    assert_eq!(result["result"]["exit_code"], 0);
                    acknowledged =
                        Some(serde_json::from_value(result["result_identity"].clone()).unwrap());
                    break;
                }
                assert!(Instant::now() < deadline);
                thread::sleep(Duration::from_millis(20));
            }
        } else {
            let result = acknowledged.clone().unwrap();
            let cleanup = request(&Request::Cleanup {
                run_id: run_id.clone(),
                result,
            })
            .unwrap();
            assert_eq!(cleanup["token_receipt_retained"], true);
            let retried = request(&Request::ExecuteToken {
                token: token.clone(),
                spec: spec.clone(),
            })
            .unwrap();
            assert_eq!(retried["state"], "cleaned");
            assert_eq!(
                fs::read_dir(root.join("runs").join(&run_id))
                    .unwrap()
                    .count(),
                1
            );
        }
        request(&Request::Shutdown).unwrap();
        worker.join().unwrap();
    }
}

#[test]
fn connected_ipc_transfers_reconnects_cancels_and_retains_results() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("worker");
    let (send, receive) = mpsc::channel();
    let r = root.clone();
    let worker = thread::spawn(move || serve_connections(&r, receive).unwrap());
    let request = |req: &Request| {
        let (mut client, server) = std::os::unix::net::UnixStream::pair().unwrap();
        send.send(Ok(server)).unwrap();
        request_stream(&mut client, req)
    };
    let client_store = Store::open(&tmp.path().join("client-store"), 16 * 1024 * 1024).unwrap();
    let inputs = input_closure(&client_store, &["/usr/bin/sleep", "/usr/bin/sh"]);
    for input in &inputs {
        let source = client_store.verified_path(&input.object).unwrap();
        let mut offset = request(&Request::Begin {
            object: input.object.clone(),
        })
        .unwrap()["offset"]
            .as_u64()
            .unwrap();
        let data = fs::read(source).unwrap();
        for chunk in data[offset as usize..].chunks(md_sim_runtime::store::MAX_CHUNK) {
            request(&Request::Chunk {
                object: input.object.clone(),
                offset,
                hex: hex::encode(chunk),
            })
            .unwrap();
            offset += chunk.len() as u64;
        }
        request(&Request::Finish {
            object: input.object.clone(),
        })
        .unwrap();
        request(&Request::Finish {
            object: input.object.clone(),
        })
        .unwrap();
    }
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        stdin_object: None,
        schema_version: "2.0".into(),
        argv: vec![
            "/usr/bin/sh".into(),
            "-c".into(),
            "echo ready; echo diagnostic >&2; /usr/bin/sleep 20".into(),
        ],
        inputs: inputs.clone(),
        limits: Limits::default(),
        environment: BTreeMap::new(),
    };
    let start = request(&Request::Execute { spec }).unwrap();
    let run_id = start["run_id"].as_str().unwrap().to_string();
    // Every request uses a new connection. Client lifetime never owns the run.
    let deadline = Instant::now() + Duration::from_secs(3);
    let live_log = loop {
        let reply = request(&Request::Logs {
            run_id: run_id.clone(),
            stream: LogStream::Stdout,
            offset: 0,
            count: 100,
        })
        .unwrap();
        if !reply["hex"].as_str().unwrap().is_empty() {
            break reply;
        }
        assert!(
            Instant::now() < deadline,
            "live process output was not available"
        );
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(
        hex::decode(live_log["hex"].as_str().unwrap()).unwrap(),
        b"ready\n"
    );
    assert_eq!(live_log["process_result_complete"], false);
    assert!(live_log["saved_log_identity"].is_null());
    assert!(request(&Request::Cleanup {
        run_id: run_id.clone(),
        result: inputs[0].object.clone()
    })
    .is_err());
    let status = request(&Request::Status {
        run_id: run_id.clone(),
    })
    .unwrap();
    assert_eq!(status["state"]["state"], "booting");
    request(&Request::Stop {
        run_id: run_id.clone(),
    })
    .unwrap();
    request(&Request::Stop {
        run_id: run_id.clone(),
    })
    .unwrap();
    let result = request(&Request::Result {
        run_id: run_id.clone(),
    })
    .unwrap();
    assert_eq!(result["result"]["status"], "cancelled");
    assert!(root.join("runs").join(&run_id).join("lock.json").exists());
    let logs = request(&Request::Logs {
        run_id: run_id.clone(),
        stream: LogStream::Stderr,
        offset: 2,
        count: 4,
    })
    .unwrap();
    assert_eq!(hex::decode(logs["hex"].as_str().unwrap()).unwrap(), b"agno");
    assert_eq!(logs["next_offset"], 6);
    assert_eq!(logs["process_result_complete"], true);
    assert_eq!(logs["saved_log_identity"]["size"], 11);
    assert!(request(&Request::Logs {
        run_id: run_id.clone(),
        stream: LogStream::Stdout,
        offset: 999,
        count: 1
    })
    .is_err());
    assert!(request(&Request::Result {
        run_id: "../../outside".into()
    })
    .is_err());
    request(&Request::Shutdown).unwrap();
    worker.join().unwrap();
    assert!(fs::read_dir(root.join("runs")).unwrap().next().is_some());

    // Recreate the worker, then retrieve the same persistent result and logs.
    let (send, receive) = mpsc::channel();
    let r = root.clone();
    let worker = thread::spawn(move || serve_connections(&r, receive).unwrap());
    let request = |req: &Request| {
        let (mut client, server) = std::os::unix::net::UnixStream::pair().unwrap();
        send.send(Ok(server)).unwrap();
        request_stream(&mut client, req)
    };
    assert_eq!(
        request(&Request::Result {
            run_id: run_id.clone()
        })
        .unwrap(),
        result
    );
    let logs = request(&Request::Logs {
        run_id: run_id.clone(),
        stream: LogStream::Stdout,
        offset: 0,
        count: 100,
    })
    .unwrap();
    assert_eq!(
        hex::decode(logs["hex"].as_str().unwrap()).unwrap(),
        b"ready\n"
    );
    let identity: md_sim_runtime::store::ObjectId =
        serde_json::from_value(result["result_identity"].clone()).unwrap();
    assert!(request(&Request::Cleanup {
        run_id: run_id.clone(),
        result: inputs[0].object.clone()
    })
    .is_err());
    let sentinel = tmp.path().join("unrelated");
    fs::write(&sentinel, b"preserve").unwrap();
    let unexpected = root.join("runs").join(&run_id).join("unexpected");
    std::os::unix::fs::symlink(&sentinel, &unexpected).unwrap();
    assert!(request(&Request::Cleanup {
        run_id: run_id.clone(),
        result: identity.clone()
    })
    .is_err());
    fs::remove_file(unexpected).unwrap();
    assert_eq!(
        request(&Request::Cleanup {
            run_id: run_id.clone(),
            result: identity
        })
        .unwrap()["removed"],
        true
    );
    assert!(!root.join("runs").join(run_id).exists());
    assert_eq!(fs::read(sentinel).unwrap(), b"preserve");

    // A saved result can be larger than a control message. Retrieve its exact
    // bytes in bounded chunks, then check its independently read identity.
    let line = "x".repeat(128);
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        stdin_object: None,
        schema_version: "2.0".into(),
        argv: vec![
            "/usr/bin/sh".into(),
            "-c".into(),
            // Exceed the full1MiB aggregate trace allowance, not only half.
            format!("i=0; while [ $i -lt 10000 ]; do echo {line}; i=$((i+1)); done"),
        ],
        inputs: inputs.clone(),
        limits: Limits::default(),
        environment: BTreeMap::new(),
    };
    let run_id = request(&Request::Execute { spec }).unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_string();
    let deadline = Instant::now() + Duration::from_secs(5);
    let result = loop {
        match request(&Request::Result {
            run_id: run_id.clone(),
        }) {
            Ok(result) => break result,
            Err(error) => {
                assert!(
                    Instant::now() < deadline,
                    "result did not complete: {error:#}"
                );
                thread::sleep(Duration::from_millis(10));
            }
        }
    };
    assert_eq!(result["result"]["trace_truncated"], true);
    assert!(result["result"].get("stdout").is_none());
    let identity: md_sim_runtime::store::ObjectId =
        serde_json::from_value(result["result_identity"].clone()).unwrap();
    assert!(identity.size > (md_sim_runtime::store::MAX_CHUNK * 2 + 65536) as u64);
    let mut complete = Vec::new();
    while (complete.len() as u64) < identity.size {
        let part = request(&Request::ResultData {
            run_id: run_id.clone(),
            result: identity.clone(),
            offset: complete.len() as u64,
            count: md_sim_runtime::store::MAX_CHUNK,
        })
        .unwrap();
        let bytes = hex::decode(part["hex"].as_str().unwrap()).unwrap();
        assert!(!bytes.is_empty());
        complete.extend(bytes);
    }
    let downloaded = tmp.path().join("downloaded-result.json");
    fs::write(&downloaded, &complete).unwrap();
    assert_eq!(hash_file(&downloaded).unwrap(), identity);
    let full: serde_json::Value = serde_json::from_slice(&complete).unwrap();
    assert_eq!(
        hex::decode(full["stdout"].as_str().unwrap()).unwrap().len(),
        1024 * 1024
    );
    assert_eq!(full["stderr"], "");
    assert_eq!(full["exit_code"], 0);
    request(&Request::Cleanup {
        run_id,
        result: identity,
    })
    .unwrap();
    request(&Request::Shutdown).unwrap();
    worker.join().unwrap();
    let worker_store = Store::open(&root.join("store"), 8 * 1024 * 1024 * 1024).unwrap();
    for input in inputs {
        worker_store.verified_path(&input.object).unwrap();
    }
}

#[test]
fn timeout_releases_the_namespace() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path(), 16 * 1024 * 1024).unwrap();
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        stdin_object: None,
        schema_version: "2.0".into(),
        argv: vec!["/usr/bin/sleep".into(), "20".into()],
        inputs: input_closure(&store, &["/usr/bin/sleep"]),
        limits: Limits {
            wall_seconds: 1,
            ..Limits::default()
        },
        environment: BTreeMap::new(),
    };
    let start = Instant::now();
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false))).unwrap();
    assert_eq!(result.status, "timed-out");
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn completed_namespace_drops_descendant_pipe_descriptors() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Store::open(tmp.path(), 16 * 1024 * 1024).unwrap();
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        stdin_object: None,
        schema_version: "2.0".into(),
        argv: vec![
            "/usr/bin/sh".into(),
            "-c".into(),
            "/usr/bin/sleep 20 & exit 0".into(),
        ],
        inputs: input_closure(&store, &["/usr/bin/sh", "/usr/bin/sleep"]),
        limits: Limits::default(),
        environment: BTreeMap::new(),
    };
    let start = Instant::now();
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false))).unwrap();
    assert_eq!(result.exit_code, Some(0));
    assert!(start.elapsed() < Duration::from_secs(5));
}

#[test]
fn worker_admission_preserves_budget_across_restart_and_cleanup() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("worker");
    let store = Store::open(&root.join("store"), 8 * 1024 * 1024 * 1024).unwrap();
    let inputs = input_closure(&store, &["/usr/bin/sh"]);
    drop(store);
    let limits = WorkerLimits {
        max_active_runs: 1,
        max_retained_runs: 1,
        max_retained_bytes: 128 * 1024,
        ..WorkerLimits::default()
    };
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        stdin_object: None,
        schema_version: "2.0".into(),
        argv: vec!["/usr/bin/sh".into(), "-c".into(), "echo budget-test".into()],
        inputs,
        limits: Limits {
            trace_bytes: 64,
            ..Limits::default()
        },
        environment: BTreeMap::new(),
    };
    let mut saved_run = None;
    for restart in [false, true] {
        let (send, receive) = mpsc::channel();
        let r = root.clone();
        let l = limits.clone();
        let worker = thread::spawn(move || serve_connections_with_limits(&r, receive, l).unwrap());
        let request = |req: &Request| {
            let (mut client, server) = std::os::unix::net::UnixStream::pair().unwrap();
            send.send(Ok(server)).unwrap();
            request_stream(&mut client, req)
        };
        if restart {
            // Completed evidence consumes a run slot after a worker restart.
            let error = request(&Request::Execute { spec: spec.clone() }).unwrap_err();
            assert!(format!("{error:#}").contains("retained run limit"));
            let (run_id, result) = saved_run.take().unwrap();
            request(&Request::Cleanup { run_id, result }).unwrap();
        }
        let mut too_large = spec.clone();
        too_large.limits.trace_bytes = 64 * 1024;
        let error = request(&Request::Execute { spec: too_large }).unwrap_err();
        assert!(format!("{error:#}").contains("retained byte limit"));
        assert_eq!(fs::read_dir(root.join("runs")).unwrap().count(), 0);
        let start = request(&Request::Execute { spec: spec.clone() }).unwrap();
        let run_id = start["run_id"].as_str().unwrap().to_string();
        let deadline = Instant::now() + Duration::from_secs(5);
        let result = loop {
            if let Ok(value) = request(&Request::Result {
                run_id: run_id.clone(),
            }) {
                break value;
            }
            assert!(Instant::now() < deadline, "admitted process did not finish");
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(result["result"]["exit_code"], 0);
        let lock: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("runs").join(&run_id).join("lock.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            lock["worker_limits"],
            serde_json::to_value(&limits).unwrap()
        );
        assert!(request(&Request::Execute { spec: spec.clone() }).is_err());
        saved_run = Some((
            run_id,
            serde_json::from_value(result["result_identity"].clone()).unwrap(),
        ));
        request(&Request::Shutdown).unwrap();
        worker.join().unwrap();
    }
}

#[test]
fn supervisor_loss_terminates_its_live_namespace() {
    const CHILD_ENV: &str = "MD_SIM_PARENT_LOSS_TEST_DIRECTORY";
    if let Some(root) = std::env::var_os(CHILD_ENV) {
        let root = PathBuf::from(root);
        let store = Store::open(&root.join("store"), 16 * 1024 * 1024).unwrap();
        let spec = ProcessSpec {
            support_virtualization: None,
            private_channel: None,
            stdin_stream: None,
            namespace_uid: 0,
            dashboard: None,
            loopback: false,
            stdin_object: None,
            schema_version: "2.0".into(),
            argv: vec![
                "/usr/bin/sh".into(),
                "-c".into(),
                "echo live; /usr/bin/sleep 60".into(),
            ],
            inputs: input_closure(&store, &["/usr/bin/sh", "/usr/bin/sleep"]),
            limits: Limits {
                wall_seconds: 90,
                ..Limits::default()
            },
            environment: BTreeMap::new(),
        };
        let trace = md_sim_runtime::isolation::TraceFiles {
            stdout: fs::File::create(root.join("stdout.log")).unwrap(),
            stderr: fs::File::create(root.join("stderr.log")).unwrap(),
        };
        md_sim_runtime::isolation::execute_with_trace(
            &store,
            &spec,
            Arc::new(AtomicBool::new(false)),
            Some(trace),
        )
        .unwrap();
        panic!("parent should kill this supervisor while its guest is live");
    }
    let tmp = tempfile::tempdir().unwrap();
    struct ChildGuard(std::process::Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut guard = ChildGuard(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "supervisor_loss_terminates_its_live_namespace",
                "--nocapture",
            ])
            .env(CHILD_ENV, tmp.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap(),
    );
    let child = &mut guard.0;
    let deadline = Instant::now() + Duration::from_secs(5);
    while fs::read(tmp.path().join("stdout.log")).unwrap_or_default() != b"live\n" {
        if Instant::now() >= deadline || child.try_wait().unwrap().is_some() {
            let _ = child.kill();
            let _ = child.wait();
            panic!("guest did not reach the live marker");
        }
        thread::sleep(Duration::from_millis(20));
    }
    // Observe all live descendants before killing the supervisor. This checks
    // real kernel lifetime behavior, not a reported process-state flag.
    fn descendants(pid: u32, output: &mut Vec<u32>) {
        // fork is called by the Rust test thread, not necessarily the leader.
        if let Ok(tasks) = fs::read_dir(format!("/proc/{pid}/task")) {
            for task in tasks.flatten() {
                let children = fs::read_to_string(task.path().join("children")).unwrap_or_default();
                for child in children
                    .split_whitespace()
                    .map(|s| s.parse::<u32>().unwrap())
                {
                    if !output.contains(&child) {
                        output.push(child);
                        descendants(child, output);
                    }
                }
            }
        }
    }
    let mut guests = Vec::new();
    descendants(child.id(), &mut guests);
    assert!(!guests.is_empty(), "expected live namespace descendants");
    child.kill().unwrap();
    child.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let live: Vec<_> = guests
            .iter()
            .filter(|pid| {
                fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|stat| {
                    // The comm field can contain spaces. A zombie has exited and
                    // cannot retain the guest mounts or continue guest execution.
                    stat.rsplit_once(')')
                        .is_some_and(|(_, fields)| !fields.trim_start().starts_with('Z'))
                })
            })
            .collect();
        if live.is_empty() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "guest descendants survived worker death: {live:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn interrupted_run_recovery_retains_failure_evidence_for_acknowledged_cleanup() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("worker");
    let run_id = "run-1234567890abcdef1234567890abcdef".to_string();
    let run = root.join("runs").join(&run_id);
    fs::create_dir_all(&run).unwrap();
    fs::write(run.join("state.json"), b"{\"state\":\"booting\"}\n").unwrap();
    fs::write(run.join("stdout.log"), b"partial guest trace\n").unwrap();
    fs::write(run.join("stderr.log"), b"diagnostic\n").unwrap();
    fs::write(run.join("result.tmp-123"), b"{\"status\":").unwrap();
    let log_hash = hash_file(&run.join("stdout.log")).unwrap();
    let state_hash = hash_file(&run.join("state.json")).unwrap();
    let fragment_hash = hash_file(&run.join("result.tmp-123")).unwrap();
    let (send, receive) = mpsc::channel();
    let r = root.clone();
    let worker = thread::spawn(move || serve_connections(&r, receive).unwrap());
    let request = |req: &Request| {
        let (mut client, server) = std::os::unix::net::UnixStream::pair().unwrap();
        send.send(Ok(server)).unwrap();
        request_stream(&mut client, req)
    };
    assert_eq!(
        request(&Request::Status {
            run_id: run_id.clone()
        })
        .unwrap()["state"]["state"],
        "lost"
    );
    let result = request(&Request::Result {
        run_id: run_id.clone(),
    })
    .unwrap();
    assert_eq!(result["result"]["status"], "lost");
    assert_eq!(result["result"]["firmware_gate"], "not-run");
    assert_eq!(
        result["result"]["retained_files"]["stdout.log"],
        serde_json::to_value(log_hash).unwrap()
    );
    assert_eq!(
        result["result"]["retained_files"]["state-before-loss.json"],
        serde_json::to_value(state_hash.clone()).unwrap()
    );
    assert_eq!(
        hash_file(&run.join("state-before-loss.json")).unwrap(),
        state_hash
    );
    assert_eq!(
        hash_file(&run.join("result.tmp-123")).unwrap(),
        fragment_hash
    );
    request(&Request::Cleanup {
        run_id,
        result: serde_json::from_value(result["result_identity"].clone()).unwrap(),
    })
    .unwrap();
    assert!(!run.exists());
    request(&Request::Shutdown).unwrap();
    worker.join().unwrap();
}

#[test]
fn active_run_reserves_unwritten_evidence_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("worker");
    let store = Store::open(&root.join("store"), 8 * 1024 * 1024 * 1024).unwrap();
    let inputs = input_closure(&store, &["/usr/bin/sh", "/usr/bin/sleep"]);
    drop(store);
    let limits = WorkerLimits {
        max_active_runs: 2,
        max_retained_runs: 4,
        max_retained_bytes: 320 * 1024,
        ..WorkerLimits::default()
    };
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        stdin_object: None,
        schema_version: "2.0".into(),
        argv: vec![
            "/usr/bin/sh".into(),
            "-c".into(),
            "echo ready; /usr/bin/sleep 20".into(),
        ],
        inputs,
        limits: Limits {
            trace_bytes: 64 * 1024,
            ..Limits::default()
        },
        environment: BTreeMap::new(),
    };
    let (send, receive) = mpsc::channel();
    let r = root.clone();
    let worker = thread::spawn(move || serve_connections_with_limits(&r, receive, limits).unwrap());
    let request = |req: &Request| {
        let (mut client, server) = std::os::unix::net::UnixStream::pair().unwrap();
        send.send(Ok(server)).unwrap();
        request_stream(&mut client, req)
    };
    let first = request(&Request::Execute { spec: spec.clone() }).unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_string();
    // The first guest has written almost nothing. Its possible future output
    // must still exclude a second run from this shared evidence budget.
    let error = request(&Request::Execute { spec: spec.clone() }).unwrap_err();
    assert!(format!("{error:#}").contains("retained byte limit"));
    assert_eq!(fs::read_dir(root.join("runs")).unwrap().count(), 1);
    request(&Request::Stop {
        run_id: first.clone(),
    })
    .unwrap();
    let second = request(&Request::Execute { spec }).unwrap()["run_id"]
        .as_str()
        .unwrap()
        .to_string();
    request(&Request::Stop {
        run_id: second.clone(),
    })
    .unwrap();
    for run_id in [first, second] {
        let result = request(&Request::Result {
            run_id: run_id.clone(),
        })
        .unwrap();
        assert_eq!(result["result"]["status"], "cancelled");
        request(&Request::Cleanup {
            run_id,
            result: serde_json::from_value(result["result_identity"].clone()).unwrap(),
        })
        .unwrap();
    }
    request(&Request::Shutdown).unwrap();
    worker.join().unwrap();
}

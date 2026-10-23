use super::*;
use md_sim_runtime::{
    store::ObjectId,
    worker::{self, PrivateChannelBinding, Request, WorkerLimits},
};
use std::{os::unix::net::UnixStream, path::Path};

struct Worker {
    send: Option<mpsc::Sender<anyhow::Result<UnixStream>>>,
    join: Option<thread::JoinHandle<anyhow::Result<()>>>,
}

#[test]
fn owned_worker_root_stays_exclusive_after_service_return() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("leased-worker");
    let lease = worker::WorkerRootLease::acquire(&root).unwrap();
    assert_eq!(lease.path(), root.canonicalize().unwrap());
    let normal_attempt = |root: PathBuf| {
        thread::spawn(move || {
            worker::serve_connections(&root, std::iter::empty::<anyhow::Result<UnixStream>>())
        })
        .join()
        .unwrap()
    };
    assert!(normal_attempt(root.clone())
        .unwrap_err()
        .to_string()
        .contains("already in use"));
    let (sender, receiver) = mpsc::sync_channel(1);
    let binding = PrivateChannelBinding::new(
        ObjectId {
            sha256: "11".repeat(32),
            size: 1,
        },
        sender,
    )
    .unwrap();
    worker::serve_connections_with_channel_owned(
        &lease,
        std::iter::empty::<anyhow::Result<UnixStream>>(),
        WorkerLimits::default(),
        binding,
    )
    .unwrap();
    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::TryRecvError::Disconnected)
    ));
    assert!(normal_attempt(root.clone())
        .unwrap_err()
        .to_string()
        .contains("already in use"));
    fs::write(
        lease.path().join("provider-evidence"),
        b"written while lease is owned",
    )
    .unwrap();
    drop(lease);
    normal_attempt(root.clone()).unwrap();
    assert_eq!(
        fs::read(root.join("provider-evidence")).unwrap(),
        b"written while lease is owned"
    );
}
impl Worker {
    fn start(root: &Path, binding: Option<PrivateChannelBinding>) -> Self {
        let (send, receive) = mpsc::channel();
        let root = root.to_path_buf();
        let join = thread::spawn(move || match binding {
            Some(binding) => worker::serve_connections_with_channel(
                &root,
                receive,
                WorkerLimits::default(),
                binding,
            ),
            None => worker::serve_connections(&root, receive),
        });
        Self {
            send: Some(send),
            join: Some(join),
        }
    }
    fn request(&self, request: &Request) -> anyhow::Result<serde_json::Value> {
        let (mut client, server) = UnixStream::pair()?;
        self.send.as_ref().unwrap().send(Ok(server))?;
        worker::request_stream(&mut client, request)
    }
    fn result(&self, run: &str) -> serde_json::Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(result) = self.request(&Request::Result { run_id: run.into() }) {
                return result;
            }
            assert!(Instant::now() < deadline, "worker result deadline");
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn cleanup(&self, run: &str, result: &serde_json::Value) {
        let identity: ObjectId = serde_json::from_value(result["result_identity"].clone()).unwrap();
        let bytes = self
            .request(&Request::ResultData {
                run_id: run.into(),
                result: identity.clone(),
                offset: 0,
                count: 65536,
            })
            .unwrap();
        let actual = hex::decode(bytes["hex"].as_str().unwrap()).unwrap();
        assert_eq!(actual.len() as u64, identity.size);
        assert_eq!(hex::encode(Sha256::digest(&actual)), identity.sha256);
        self.request(&Request::Cleanup {
            run_id: run.into(),
            result: identity,
        })
        .unwrap();
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.send.take();
        if let Some(join) = self.join.take() {
            let result = join.join();
            if thread::panicking() {
                eprintln!("worker cleanup while unwinding: {result:?}");
            } else {
                assert!(result.unwrap().is_ok());
            }
        }
    }
}

#[test]
fn worker_binds_one_exact_private_channel_and_retains_token_evidence() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("worker");
    let marker = temp.path().join("host-marker");
    fs::write(&marker, b"host-only").unwrap();
    let store =
        Store::open_with_limits(&root.join("store"), WorkerLimits::default().store).unwrap();
    let mut contract = spec(&store, marker.to_str().unwrap());
    contract
        .environment
        .insert("CHANNEL_FIXTURE_MODE".into(), "duplex".into());
    let identity = worker::process_identity(&contract).unwrap();
    drop(store);
    let canonical = serde_json::to_vec(&contract).unwrap();
    assert_eq!(identity.sha256, hex::encode(Sha256::digest(&canonical)));
    assert_eq!(identity.size, canonical.len() as u64);
    {
        let worker = Worker::start(&root, None);
        let error = worker
            .request(&Request::Execute {
                spec: contract.clone(),
            })
            .unwrap_err();
        assert!(error.to_string().contains("not configured"));
        assert_eq!(fs::read_dir(root.join("runs")).unwrap().count(), 0);
    }
    let (sender, receiver) = mpsc::sync_channel(1);
    let worker = Worker::start(
        &root,
        Some(PrivateChannelBinding::new(identity.clone(), sender).unwrap()),
    );
    let mut wrong_cap = contract.clone();
    wrong_cap.private_channel.as_mut().unwrap().max_wire_bytes += 1;
    let mut wrong_input = contract.clone();
    wrong_input.inputs[0].object.sha256 = "ab".repeat(32);
    for wrong in [wrong_cap, wrong_input] {
        let error = worker
            .request(&Request::Execute { spec: wrong })
            .unwrap_err();
        assert!(error.to_string().contains("identity mismatch"));
        assert_eq!(fs::read_dir(root.join("runs")).unwrap().count(), 0);
        assert!(matches!(
            receiver.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
    }
    let provider = thread::spawn(move || {
        let mut file = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        let mut sent = frame(201, 1500);
        sent.extend(frame(43, 14));
        let mut expected = frame(11, 1200);
        expected.extend(frame(42, 14));
        assert_eq!(transfer(&mut file, &sent, expected.len()), expected);
        assert!(transfer(&mut file, &[], usize::MAX).is_empty());
    });
    let token = "e1".repeat(32);
    let request = Request::ExecuteToken {
        token: token.clone(),
        spec: contract.clone(),
    };
    let accepted = worker.request(&request).unwrap();
    let run = accepted["run_id"].as_str().unwrap();
    assert_eq!(worker.request(&request).unwrap()["run_id"], run);
    let result = worker.result(run);
    provider.join().unwrap();
    assert_eq!(result["result"]["status"], "exited");
    assert_eq!(result["result"]["exit_code"], 0);
    assert_eq!(
        result["result"]["private_channel"]["guest_to_peer"]["forwarded_packets"],
        2
    );
    assert_eq!(
        result["result"]["private_channel"]["peer_to_guest"]["forwarded_packets"],
        2
    );
    let lock: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("runs").join(run).join("lock.json")).unwrap())
            .unwrap();
    assert_eq!(
        lock["private_channel_binding"]["process_identity"],
        serde_json::to_value(identity).unwrap()
    );
    assert_eq!(lock["private_channel_binding"]["provider_consumed"], true);
    assert!(worker
        .request(&Request::Execute {
            spec: contract.clone()
        })
        .unwrap_err()
        .to_string()
        .contains("already consumed"));
    assert_eq!(fs::read_dir(root.join("runs")).unwrap().count(), 1);
    println!(
        "PRIVATE_CHANNEL_WORKER_EVIDENCE {}",
        serde_json::json!({"accepted":accepted,"lock":lock,"result":result})
    );
    worker.cleanup(run, &result);
    let lookup = worker.request(&request).unwrap();
    assert_eq!(lookup["state"], "cleaned");
    assert_eq!(lookup["run_id"], run);
    assert!(root.join("runs").join(run).join("cleanup.json").is_file());
    drop(worker);
    let store =
        Store::open_with_limits(&root.join("store"), WorkerLimits::default().store).unwrap();
    for input in &contract.inputs {
        store.verified_path(&input.object).unwrap();
    }
    drop(store);
    let unconfigured_restart = Worker::start(&root, None);
    assert_eq!(
        unconfigured_restart.request(&request).unwrap()["state"],
        "cleaned"
    );
}

#[test]
fn worker_cancels_carrier_and_consumes_binding_on_launch_failure() {
    for launch_failure in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("worker");
        let marker = temp.path().join("host-marker");
        fs::write(&marker, b"host-only").unwrap();
        let store =
            Store::open_with_limits(&root.join("store"), WorkerLimits::default().store).unwrap();
        let mut contract = spec(&store, marker.to_str().unwrap());
        contract
            .environment
            .insert("CHANNEL_FIXTURE_MODE".into(), "no-read".into());
        contract.limits.wall_seconds = 10;
        drop(store);
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut receiver = Some(receiver);
        if launch_failure {
            receiver.take();
        }
        let binding =
            PrivateChannelBinding::new(worker::process_identity(&contract).unwrap(), sender)
                .unwrap();
        let worker = Worker::start(&root, Some(binding));
        let started = worker
            .request(&Request::Execute {
                spec: contract.clone(),
            })
            .unwrap();
        let run = started["run_id"].as_str().unwrap();
        let mut held_provider = None;
        if let Some(receiver) = receiver {
            held_provider = Some(receiver.recv_timeout(Duration::from_secs(5)).unwrap());
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                let log = worker
                    .request(&Request::Logs {
                        run_id: run.into(),
                        stream: worker::LogStream::Stdout,
                        offset: 0,
                        count: 16384,
                    })
                    .unwrap();
                if String::from_utf8(hex::decode(log["hex"].as_str().unwrap()).unwrap())
                    .unwrap()
                    .contains("PRIVATE_NO_READ_LIVE")
                {
                    break;
                }
                assert!(Instant::now() < deadline, "guest readiness deadline");
                thread::sleep(Duration::from_millis(10));
            }
            worker
                .request(&Request::Stop { run_id: run.into() })
                .unwrap();
        }
        let result = worker.result(run);
        if launch_failure {
            assert_eq!(result["result"]["private_channel_binding_consumed"], true);
            assert!(result["result"]["infrastructure_error"]
                .as_str()
                .unwrap()
                .contains("provider is unavailable"));
        } else {
            assert_eq!(result["result"]["status"], "cancelled");
            assert_eq!(
                result["result"]["private_channel"]["end"],
                "supervisor-closed"
            );
            let mut file = held_provider.take().unwrap();
            assert!(transfer(&mut file, &[], usize::MAX).is_empty());
        }
        assert!(worker
            .request(&Request::Execute { spec: contract })
            .unwrap_err()
            .to_string()
            .contains("already consumed"));
        assert_eq!(fs::read_dir(root.join("runs")).unwrap().count(), 1);
        println!(
            "PRIVATE_CHANNEL_WORKER_STOP_EVIDENCE {}",
            serde_json::json!({"launch_failure":launch_failure,"result":result})
        );
        worker.cleanup(run, &result);
        assert!(!root.join("runs").join(run).exists());
    }
}

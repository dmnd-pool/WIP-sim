//! Contract tests use scripted control responses. The ignored VM test uses real
//! TCP/Unix byte streams, but remains a component fixture, not a firmware pass.
use super::*;
use md_sim_coordinator::{ExecutionScope, SuiteIdentity};
use md_sim_runtime::{
    isolation::{Limits, Mount},
    private_channel::{Protocol, Spec},
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn policy() -> Policy {
    Policy {
        schema: "md-sim-carrier-dashboard-policy/1".into(),
        source: Source::AdaptedNative,
        support_identity: object(b"captured support manifest fixture"),
        destination: "10.231.0.1:18081".parse().unwrap(),
        listen_port: 0,
        connection_limit: 2,
        connection_attempt_limit: 16,
        connection_bytes: 1 << 20,
        run_bytes: 8 << 20,
        connect_milliseconds: 500,
        idle_seconds: 2,
        wall_seconds: 10,
        lease_milliseconds: 1000,
    }
}

pub(crate) fn fixture(policy: &Policy) -> Result<(LockedRun, ObjectId, Vec<u8>, ProviderBinding)> {
    let image = object(b"not executable: contract fixture only");
    let policy_bytes = serde_json::to_vec(policy)?;
    let policy_id = object(&policy_bytes);
    let process = ProcessSpec {
        schema_version: "2.0".into(),
        argv: vec!["/program".into()],
        inputs: vec![
            Mount {
                guest_path: "/program".into(),
                object: image.clone(),
            },
            Mount {
                guest_path: "/support.json".into(),
                object: policy.support_identity.clone(),
            },
        ],
        stdin_object: None,
        stdin_stream: None,
        namespace_uid: 0,
        environment: BTreeMap::new(),
        dashboard: None,
        loopback: false,
        private_channel: Some(Spec {
            protocol: Protocol::QemuEthernetV1,
            guest_fd: 3,
            max_frame_bytes: 65535,
            max_wire_bytes: 1 << 20,
            max_packets: 1024,
        }),
        limits: Limits {
            memory_bytes: 512 << 20,
            processes: 32,
            ..Limits::default()
        },
    };
    let run = LockedRun {
        name: "carrier access contract fixture".into(),
        scope: ExecutionScope::ProcessPrimitive {
            description: "identity fixture only".into(),
        },
        process: Some(process.clone()),
        suite: SuiteIdentity {
            id: "fixture".into(),
            version: "1".into(),
            suite: image.clone(),
            verifier: image.clone(),
            required_gates: BTreeSet::from(["identity".into()]),
        },
        inputs: BTreeMap::from([
            ("image".into(), image.clone()),
            ("support".into(), policy.support_identity.clone()),
            ("policy".into(), policy_id.clone()),
        ]),
        backend_tools: BTreeMap::from([
            ("bubblewrap".into(), image.clone()),
            ("unshare".into(), image),
        ]),
        policies: json!({"carrier_dashboard":policy_id}),
        capability_limits: vec!["not firmware or actual worker execution".into()],
    };
    let selection = Selection::new(&run, policy_id.clone(), &policy_bytes, &"ab".repeat(32))?;
    let config = serde_json::to_vec(&json!({"schema":"md-sim-worker-private-carrier/1",
        "process":process,"support_identity":policy.support_identity,"handoff_seconds":10,"wall_seconds":20,
        "lease":{"locked_run":run,"policy":policy_id}}))?;
    let binding = ProviderBinding::from_config(selection, object(&config), &config)?;
    Ok((run, policy_id, policy_bytes, binding))
}

struct WorkerFixture {
    lookup: Value,
    status: Value,
    requests: Vec<&'static str>,
}

impl WorkerFixture {
    fn new(binding: &ProviderBinding) -> Self {
        let identity = binding.selection().identity();
        Self {
            lookup: json!({"found":true,"token":identity.token,"run_id":identity.run_id,
            "token_process_identity":identity.process,"state":"booting"}),
            status: json!({"run_id":identity.run_id,"state":{"state":"booting"},"dashboard":null}),
            requests: Vec::new(),
        }
    }
}

impl WorkerTransport for WorkerFixture {
    fn request(&mut self, request: &Request) -> Result<Value> {
        match request {
            Request::LookupToken { token } => {
                ensure!(self.lookup["token"] == *token, "unexpected queried token");
                self.requests.push("lookup");
                Ok(self.lookup.clone())
            }
            Request::Status { run_id } => {
                ensure!(self.status["run_id"] == *run_id, "unexpected queried run");
                self.requests.push("status");
                Ok(self.status.clone())
            }
            _ => anyhow::bail!("unexpected mutating request"),
        }
    }
    fn description(&self) -> String {
        "scripted unit-test responses; not a worker".into()
    }
}

struct ProviderFixture {
    sequence: u64,
    state: ProviderState,
    changed: Option<&'static str>,
}
impl Default for ProviderFixture {
    fn default() -> Self {
        Self {
            sequence: 0,
            state: ProviderState::Active,
            changed: None,
        }
    }
}
impl ProviderTransport for ProviderFixture {
    fn lease(&mut self, binding: &ProviderBinding, challenge: &str) -> Result<ProviderLease> {
        self.sequence += 1;
        let mut result = ProviderLease {
            schema: "md-sim-carrier-provider-lease/1".into(),
            identity: binding.selection.identity.clone(),
            configuration: binding.configuration.clone(),
            challenge: challenge.into(),
            sequence: self.sequence,
            state: self.state,
        };
        match self.changed {
            Some("challenge") => result.challenge = "00".repeat(32),
            Some("process") => result.identity.process = object(b"another process"),
            Some("lock") => result.identity.locked_run = object(b"another lock"),
            Some("configuration") => result.configuration = object(b"another provider"),
            Some("token") => result.identity.token = "cd".repeat(32),
            Some("schema") => result.schema = "unknown".into(),
            Some("zero") => result.sequence = 0,
            Some("replay") => result.sequence = 1,
            None => {}
            _ => anyhow::bail!("unknown fixture mutation"),
        }
        Ok(result)
    }
}

#[test]
fn selection_requires_exact_locked_policy_bytes_and_support_input() -> Result<()> {
    let (run, id, bytes, binding) = fixture(&policy())?;
    assert_eq!(binding.selection.identity.locked_run, run.object()?);
    assert_eq!(
        binding.selection.identity.process,
        process_identity(run.process.as_ref().unwrap())?
    );
    let mut changed = bytes.clone();
    changed.push(b' ');
    assert!(Selection::new(&run, id.clone(), &changed, &"ab".repeat(32)).is_err());
    let mut bad = run.clone();
    bad.policies["carrier_dashboard"] = json!(object(b"other policy"));
    assert!(Selection::new(&bad, id.clone(), &bytes, &"ab".repeat(32)).is_err());
    let mut bad = run.clone();
    bad.inputs.remove("policy");
    assert!(Selection::new(&bad, id.clone(), &bytes, &"ab".repeat(32)).is_err());
    let mut bad = run.clone();
    bad.process.as_mut().unwrap().private_channel = None;
    assert!(Selection::new(&bad, id.clone(), &bytes, &"ab".repeat(32)).is_err());
    let mut bad = run.clone();
    bad.process.as_mut().unwrap().inputs.pop();
    assert!(Selection::new(&bad, id.clone(), &bytes, &"ab".repeat(32)).is_err());
    assert!(Selection::new(&run, id, &bytes, "short").is_err());
    Ok(())
}

#[test]
fn policy_rejects_unbounded_or_unselected_addresses_and_unknown_fields() -> Result<()> {
    for address in [
        "0.0.0.0:80",
        "127.0.0.1:80",
        "8.8.8.8:80",
        "224.0.0.1:80",
        "[::1]:80",
        "10.0.2.15:0",
    ] {
        let mut bad = policy();
        bad.destination = address.parse()?;
        assert!(bad.validate().is_err());
    }
    let baseline = serde_json::to_value(policy())?;
    for (field, value) in [
        ("connection_limit", 9),
        ("connection_attempt_limit", 4097),
        ("connection_bytes", (64 << 20) + 1),
        ("run_bytes", (256 << 20) + 1),
        ("connect_milliseconds", 2001),
        ("idle_seconds", 61),
        ("wall_seconds", 3601),
        ("lease_milliseconds", 99),
    ] {
        let mut bad = baseline.clone();
        bad[field] = json!(value);
        assert!(
            serde_json::from_value::<Policy>(bad)?.validate().is_err(),
            "accepted {field}"
        );
    }
    let mut bad = baseline;
    bad["unrecorded_override"] = json!(true);
    assert!(serde_json::from_value::<Policy>(bad).is_err());
    Ok(())
}

#[test]
fn provider_config_cannot_substitute_process_or_support() -> Result<()> {
    let (run, policy_id, _, binding) = fixture(&policy())?;
    for field in ["process", "support_identity", "schema", "lease"] {
        let mut config = json!({"schema":"md-sim-worker-private-carrier/1",
            "process":binding.selection.process,"support_identity":binding.selection.identity.support,
            "lease":{"locked_run":run,"policy":policy_id}});
        match field {
            "process" => config["process"]["argv"] = json!(["/program", "different"]),
            "support_identity" => config[field] = json!(object(b"different support")),
            "lease" => config[field] = Value::Null,
            _ => config[field] = json!("unknown"),
        }
        let bytes = serde_json::to_vec(&config)?;
        assert!(
            ProviderBinding::from_config(binding.selection.clone(), object(&bytes), &bytes)
                .is_err()
        );
    }
    Ok(())
}

#[test]
fn authorization_queries_transports_and_rejects_changed_or_closed_owners() -> Result<()> {
    let (_, _, _, binding) = fixture(&policy())?;
    let mut worker = WorkerFixture::new(&binding);
    let mut provider = ProviderFixture::default();
    let first = binding.authorize(&mut worker, &mut provider)?;
    let second = binding.authorize(&mut worker, &mut provider)?;
    assert_eq!(worker.requests, ["lookup", "status", "lookup", "status"]);
    assert_ne!(first.lease.challenge, second.lease.challenge);
    assert!(second.lease.sequence > first.lease.sequence);
    assert!(first.evidence()["worker_status"]["dashboard"].is_null());
    for changed in [
        "challenge",
        "process",
        "lock",
        "configuration",
        "token",
        "schema",
        "zero",
    ] {
        provider.changed = Some(changed);
        assert!(
            binding.authorize(&mut worker, &mut provider).is_err(),
            "accepted {changed}"
        );
    }
    provider.changed = None;
    provider.state = ProviderState::Waiting;
    assert!(binding
        .authorize_or_wait(&mut worker, &mut provider)?
        .is_none());
    provider.changed = Some("configuration");
    assert!(binding
        .authorize_or_wait(&mut worker, &mut provider)
        .is_err());
    provider.changed = None;
    for state in [
        ProviderState::Waiting,
        ProviderState::Closed,
        ProviderState::Failed,
    ] {
        provider.state = state;
        assert!(binding.authorize(&mut worker, &mut provider).is_err());
    }
    provider.state = ProviderState::Active;
    worker.status["state"]["state"] = json!("completed");
    assert!(binding.authorize(&mut worker, &mut provider).is_err());
    worker.status["state"]["state"] = json!("booting");
    worker.lookup["token_process_identity"] = json!(object(b"another process"));
    assert!(binding.authorize(&mut worker, &mut provider).is_err());
    Ok(())
}

#[test]
fn provider_adapter_uses_readonly_monitor_request_and_strict_reply() -> Result<()> {
    struct Control {
        lease: Value,
        queried: bool,
    }
    impl WorkerTransport for Control {
        fn request(&mut self, request: &Request) -> Result<Value> {
            let Request::PrivateChannelStatus { token, challenge } = request else {
                anyhow::bail!("provider adapter queried an unrelated worker operation");
            };
            ensure!(
                self.lease["identity"]["token"] == *token && self.lease["challenge"] == *challenge,
                "provider request identity differs"
            );
            self.queried = true;
            Ok(self.lease.clone())
        }
        fn description(&self) -> String {
            "scripted monitor contract".into()
        }
    }
    let (_, _, _, binding) = fixture(&policy())?;
    let challenge = "de".repeat(32);
    let lease = ProviderFixture::default().lease(&binding, &challenge)?;
    let mut adapter = WorkerProviderTransport::new(Control {
        lease: serde_json::to_value(&lease)?,
        queried: false,
    });
    assert_eq!(adapter.lease(&binding, &challenge)?.challenge, challenge);
    assert!(adapter.transport.queried);
    adapter.transport.lease["unbound_extra"] = json!(true);
    assert!(adapter.lease(&binding, &challenge).is_err());
    assert!(adapter.lease(&binding, "short").is_err());
    Ok(())
}

#[test]
fn endpoint_rejects_runtime_binding_imitation_or_changed_origin() -> Result<()> {
    let (_, _, _, binding) = fixture(&policy())?;
    let good = CarrierEndpoint {
        schema: "md-sim-carrier-dashboard-endpoint/1".into(),
        identity: binding.selection.identity.clone(),
        provider_configuration: binding.configuration.clone(),
        policy: binding.selection.policy.clone(),
        listen: "127.0.0.1:18088".parse()?,
        local_socket: "/tmp/owned-carrier/http.sock".into(),
        firmware_readiness_established: false,
    };
    good.verify(&binding)?;
    let mut bad = good.clone();
    bad.firmware_readiness_established = true;
    assert!(bad.verify(&binding).is_err());
    let mut bad = good.clone();
    bad.listen = "192.0.2.1:18088".parse()?;
    assert!(bad.verify(&binding).is_err());
    let mut bad = good;
    bad.identity.run_id = format!("run-{}", "cd".repeat(32));
    assert!(bad.verify(&binding).is_err());
    Ok(())
}

#[test]
#[ignore = "requires a disconnected Linux VM with root, ip, and real socket syscalls"]
fn vm_carrier_streams_and_revocation() -> Result<()> {
    use std::{net::Shutdown, os::unix::net::UnixStream, process::Command};
    ensure!(
        unsafe { libc::geteuid() } == 0
            && fs::read_to_string("/proc/cmdline")?.contains("md_sim_private_guest=1"),
        "real carrier fixture requires the isolated test VM"
    );
    let initial = Command::new("/usr/bin/ip")
        .args(["-json", "address", "show", "dev", "lo"])
        .output()?;
    ensure!(
        initial.status.success() && !String::from_utf8(initial.stdout)?.contains("10.231.0.1"),
        "fixture address is already present"
    );
    ensure!(
        Command::new("/usr/bin/ip")
            .args(["address", "add", "10.231.0.1/32", "dev", "lo"])
            .status()?
            .success(),
        "create owned fixture address"
    );
    struct Address;
    impl Drop for Address {
        fn drop(&mut self) {
            let _ = Command::new("/usr/bin/ip")
                .args(["address", "del", "10.231.0.1/32", "dev", "lo"])
                .status();
        }
    }
    let _address = Address;
    fn accept(listener: &TcpListener) -> Result<TcpStream> {
        listener.set_nonblocking(true)?;
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
                    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
                    return Ok(stream);
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    ensure!(Instant::now() < deadline, "native fixture accept deadline");
                    thread::sleep(Duration::from_millis(5));
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    let directory = tempfile::tempdir()?;
    let native = TcpListener::bind("10.231.0.1:0")?;
    let mut policy = policy();
    policy.destination = native.local_addr()?;
    policy.lease_milliseconds = 10000;
    policy.connection_attempt_limit = 2;
    let (_, _, _, binding) = fixture(&policy)?;
    let mut worker = WorkerFixture::new(&binding);
    let mut provider = ProviderFixture::default();
    let mut payload = vec![0; 128 << 10];
    File::open("/dev/urandom")?.read_exact(&mut payload)?;
    let request = payload.clone();
    let native_thread = thread::spawn(move || -> Result<()> {
        for _ in 0..2 {
            let mut stream = accept(&native)?;
            let mut observed = Vec::new();
            stream.read_to_end(&mut observed)?;
            ensure!(
                observed == request,
                "native byte fixture received changed request"
            );
            observed.reverse();
            for chunk in observed.chunks(719) {
                stream.write_all(chunk)?;
            }
            stream.shutdown(Shutdown::Write)?;
        }
        Ok(())
    });
    let mut service = CarrierService::start(
        binding,
        &mut worker,
        &mut provider,
        &directory.path().join("bytes"),
    )?;
    let mut expected = payload.clone();
    expected.reverse();
    let mut tcp = TcpStream::connect(service.endpoint().listen)?;
    tcp.set_read_timeout(Some(Duration::from_secs(3)))?;
    tcp.set_write_timeout(Some(Duration::from_secs(3)))?;
    for chunk in payload.chunks(1031) {
        tcp.write_all(chunk)?;
    }
    tcp.shutdown(Shutdown::Write)?;
    let mut received = Vec::new();
    tcp.read_to_end(&mut received)?;
    ensure!(received == expected, "loopback bytes changed");
    let mut unix = UnixStream::connect(&service.endpoint().local_socket)?;
    unix.set_read_timeout(Some(Duration::from_secs(3)))?;
    unix.set_write_timeout(Some(Duration::from_secs(3)))?;
    unix.write_all(&payload)?;
    unix.shutdown(Shutdown::Write)?;
    received.clear();
    unix.read_to_end(&mut received)?;
    ensure!(received == expected, "Unix bytes changed");
    native_thread
        .join()
        .map_err(|_| anyhow::anyhow!("native fixture panicked"))??;
    let drain_deadline = Instant::now() + Duration::from_secs(2);
    while service.is_running() && Instant::now() < drain_deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(!service.is_running());
    let receipt = service.stop()?;
    assert_eq!(receipt.end, "connection-attempt-limit");
    assert_eq!(receipt.accepted_connections, 2);
    assert_eq!(receipt.charged_bytes, (payload.len() * 4) as u64);
    assert!(receipt
        .connections
        .iter()
        .all(|connection| connection.error.is_none()
            && connection
                .stream
                .as_ref()
                .is_some_and(|stream| stream.completed && stream.error.is_none())));
    assert!(
        receipt.error.is_none()
            && receipt.listeners_closed
            && !receipt.firmware_readiness_established
    );
    assert!(TcpStream::connect(receipt.endpoint.listen).is_err());
    assert!(!receipt.endpoint.local_socket.exists());

    // The owner closes while a real connection is alive. The byte forwarder
    // must close both listeners and the established native TCP stream.
    let native = TcpListener::bind("10.231.0.1:0")?;
    policy.destination = native.local_addr()?;
    let (_, _, _, binding) = fixture(&policy)?;
    let mut worker = WorkerFixture::new(&binding);
    let mut provider = ProviderFixture::default();
    let native_thread = thread::spawn(move || -> Result<()> {
        let mut stream = accept(&native)?;
        let mut request = [0; 4];
        stream.read_exact(&mut request)?;
        ensure!(request == *b"live", "live connection fixture differs");
        stream.write_all(b"seen")?;
        let mut tail = Vec::new();
        stream.read_to_end(&mut tail)?;
        ensure!(tail.is_empty(), "unexpected bytes after owner closure");
        Ok(())
    });
    let mut service = CarrierService::start(
        binding,
        &mut worker,
        &mut provider,
        &directory.path().join("closed"),
    )?;
    let mut tcp = TcpStream::connect(service.endpoint().listen)?;
    tcp.set_read_timeout(Some(Duration::from_secs(3)))?;
    tcp.set_write_timeout(Some(Duration::from_secs(3)))?;
    tcp.write_all(b"live")?;
    let mut ack = [0; 4];
    tcp.read_exact(&mut ack)?;
    assert_eq!(&ack, b"seen");
    provider.state = ProviderState::Closed;
    assert!(service.refresh(&mut worker, &mut provider).is_err());
    let receipt = service.stop()?;
    assert_eq!(receipt.end, "authorization-failed");
    assert!(receipt.error.is_some());
    assert!(TcpStream::connect(receipt.endpoint.listen).is_err());
    native_thread
        .join()
        .map_err(|_| anyhow::anyhow!("native fixture panicked"))??;

    // Independent lease expiry needs no callback from the caller or provider.
    policy.lease_milliseconds = 100;
    let (_, _, _, binding) = fixture(&policy)?;
    let mut worker = WorkerFixture::new(&binding);
    let mut provider = ProviderFixture::default();
    let mut service = CarrierService::start(
        binding,
        &mut worker,
        &mut provider,
        &directory.path().join("expired"),
    )?;
    let deadline = Instant::now() + Duration::from_secs(2);
    while service.is_running() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(!service.is_running());
    let receipt = service.stop()?;
    assert_eq!(receipt.end, "lease-expired");
    assert_eq!(receipt.accepted_connections, 0);

    policy.lease_milliseconds = 1000;
    let (_, _, _, binding) = fixture(&policy)?;
    let mut worker = WorkerFixture::new(&binding);
    let mut provider = ProviderFixture::default();
    let mut service = CarrierService::start(
        binding,
        &mut worker,
        &mut provider,
        &directory.path().join("replay"),
    )?;
    provider.changed = Some("replay");
    assert!(service.refresh(&mut worker, &mut provider).is_err());
    assert_eq!(service.stop()?.end, "authorization-failed");

    let occupied = TcpListener::bind("127.0.0.1:0")?;
    policy.listen_port = occupied.local_addr()?.port();
    let (_, _, _, binding) = fixture(&policy)?;
    let mut worker = WorkerFixture::new(&binding);
    let mut provider = ProviderFixture::default();
    let output = directory.path().join("occupied");
    assert!(CarrierService::start(binding, &mut worker, &mut provider, &output).is_err());
    assert!(!output.join("http.sock").exists());
    println!(
        "MD_SIM_CARRIER_ACCESS_FIXTURE {}",
        json!({"actual_tcp_and_unix":true,
        "random_payload_sha256":object(&payload),"two_half_close_exchanges":true,
        "provider_close_revoked_live_stream":true,"independent_expiry":true,
        "replay_rejected":true,"occupied_listener_rejected":true,"firmware_gate":false,
        "control_scope":"scripted contract replies; actual provider/worker transport remains separate"})
    );
    Ok(())
}

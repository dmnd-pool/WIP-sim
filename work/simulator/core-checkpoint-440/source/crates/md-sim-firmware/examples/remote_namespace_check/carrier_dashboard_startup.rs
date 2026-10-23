//! Actual production Supervisor/child launch in a private VM. Only broker
//! responses are scripted; this is not firmware or HTTP/TLS acceptance.
use super::*;
use std::{
    io::{BufRead, BufReader},
    os::{fd::AsRawFd, unix::net::UnixListener},
    sync::atomic::{AtomicBool, Ordering},
    thread::{self, JoinHandle},
};

struct Broker {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<Vec<Value>>>>,
}
impl Broker {
    fn start(binding: ProviderBinding) -> Result<Self> {
        let listener = UnixListener::bind(SOCKET)?;
        fs::set_permissions(SOCKET, fs::Permissions::from_mode(0o600))?;
        std::os::unix::fs::chown(SOCKET, Some(1000), Some(1000))?;
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let thread = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(25);
            let mut records = Vec::new();
            let mut sequence = 0;
            while !done.load(Ordering::Acquire) {
                ensure!(Instant::now() < deadline, "startup broker deadline");
                let (mut socket, _) = match listener.accept() {
                    Ok(socket) => socket,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };
                ensure!(records.len() < 100, "startup request count bound");
                let mut peer: libc::ucred = unsafe { std::mem::zeroed() };
                let mut size = std::mem::size_of_val(&peer) as libc::socklen_t;
                ensure!(
                    unsafe {
                        libc::getsockopt(
                            socket.as_raw_fd(),
                            libc::SOL_SOCKET,
                            libc::SO_PEERCRED,
                            (&mut peer as *mut libc::ucred).cast(),
                            &mut size,
                        )
                    } == 0
                        && peer.uid == 1000
                        && peer.gid == 1000,
                    "actual broker peer account differs"
                );
                socket.set_read_timeout(Some(Duration::from_secs(1)))?;
                socket.set_write_timeout(Some(Duration::from_secs(1)))?;
                let mut line = String::new();
                BufReader::new((&mut socket).take(4097)).read_line(&mut line)?;
                ensure!(
                    line.len() <= 4096 && line.ends_with('\n'),
                    "startup request bound"
                );
                let request: Request = serde_json::from_str(&line)?;
                let identity = binding.selection().identity();
                let result = match &request {
                    Request::PrivateChannelStatus { token, challenge } => {
                        ensure!(*token == identity.token, "startup provider token");
                        sequence += 1;
                        serde_json::to_value(ProviderLease {
                            schema: "md-sim-carrier-provider-lease/1".into(),
                            identity: identity.clone(),
                            configuration: binding.configuration().clone(),
                            challenge: challenge.clone(),
                            sequence,
                            state: ProviderState::Active,
                        })?
                    }
                    Request::LookupToken { token } => {
                        ensure!(*token == identity.token, "startup lookup token");
                        json!({"found":true,"token":token,"run_id":identity.run_id,
                            "token_process_identity":identity.process,"state":"booting"})
                    }
                    Request::Status { run_id } => {
                        ensure!(*run_id == identity.run_id, "startup status run");
                        json!({"run_id":run_id,"state":{"state":"booting"},"dashboard":null})
                    }
                    _ => anyhow::bail!("unexpected startup broker operation"),
                };
                records.push(
                    json!({"peer":{"pid":peer.pid,"uid":peer.uid,"gid":peer.gid},
                    "request":request,"response":result}),
                );
                serde_json::to_writer(
                    &mut socket,
                    &json!({"protocol":worker::PROTOCOL_VERSION,"ok":true,"result":result}),
                )?;
                socket.write_all(b"\n")?;
            }
            Ok(records)
        });
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }

    fn finish(&mut self) -> Result<Vec<Value>> {
        self.stop.store(true, Ordering::Release);
        self.thread
            .take()
            .context("startup broker already joined")?
            .join()
            .map_err(|_| anyhow::anyhow!("startup broker panicked"))?
    }
}
impl Drop for Broker {
    fn drop(&mut self) {
        if self.thread.is_some() {
            let _ = self.finish();
        }
    }
}

#[test]
#[ignore = "requires captured production helper and root in the isolated test VM"]
fn actual_supervisor_starts_real_uid1000_service() -> Result<()> {
    ensure!(
        unsafe { libc::geteuid() } == 0
            && fs::read_to_string("/proc/cmdline")?
                .split_whitespace()
                .any(|value| value == "md_sim_private_guest=1"),
        "private startup VM required"
    );
    ensure!(
        enabled() && shared_pool::enabled(),
        "explicit startup selection required"
    );
    // Present forbidden inputs in the parent so absence in the child proves
    // mount exclusion rather than missing fixture inputs.
    for path in [
        "/work/broker",
        "/work/pool",
        "/broker-inputs",
        "/credentials",
        "/worker",
        "/worker/access-credentials",
    ] {
        fs::create_dir_all(path)?;
    }
    fs::set_permissions("/work", fs::Permissions::from_mode(0o711))?;
    for path in ["/work/broker", "/worker"] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        std::os::unix::fs::chown(path, Some(1000), Some(1000))?;
    }
    fs::write(
        "/worker/access-credentials/server.key",
        b"private test sentinel, not a key",
    )?;
    fs::write("/work/clientkey", b"private test sentinel, not a key")?;
    let sentinel = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open("/work/parent-descriptor-sentinel")?;
    ensure!(
        unsafe { libc::fcntl(sentinel.as_raw_fd(), libc::F_SETFD, 0) } == 0,
        "cannot create non-CLOEXEC regression descriptor"
    );
    let (selected, manifest) = shared_pool::selected()?;
    let mut nonce = [0; 32];
    File::open("/dev/urandom")?.read_exact(&mut nonce)?;
    let token = hex::encode(nonce);
    let lease = shared_pool::Lease {
        schema_version: "md-sim-shared-pool-lease/1".into(),
        token: token.clone(),
        process: selected.process.clone(),
        locked_run_hash: selected.locked_run_hash.clone(),
        manifest: selected.manifest.clone(),
        pool_input: selected.pool_input.clone(),
        pool_verifier: manifest.pool_verifier,
        request_monotonic_ns: shared_pool::monotonic_ns()?,
        ready_monotonic_ns: Some(shared_pool::monotonic_ns()?),
        execute_monotonic_ns: Some(shared_pool::monotonic_ns()?),
        ready: Some(json!({"scope":"scripted startup fixture only"})),
        run_id: Some(format!("run-{token}")),
    };
    let launch = Launch {
        schema: "md-sim-carrier-dashboard-launch/1".into(),
        lease: lease.clone(),
        lock: hash_file(Path::new(LOCK))?,
        policy: hash_file(Path::new(POLICY))?,
        provider: hash_file(Path::new(PROVIDER))?,
    };
    let binding = binding(&launch)?;
    let mut broker = Broker::start(binding.clone())?;
    save_json(Path::new(shared_pool::LEASE), &lease)?;
    fs::set_permissions(shared_pool::LEASE, fs::Permissions::from_mode(0o600))?;
    std::os::unix::fs::chown(shared_pool::LEASE, Some(1000), Some(1000))?;
    let mut supervisor = Supervisor::new()?;
    let result = (|| -> Result<Value> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !Path::new("/work/access/ready.json").exists() {
            supervisor.poll()?;
            ensure!(
                !supervisor.finished,
                "actual service exited before ready: {}",
                fs::read_to_string("/work/carrier.stderr").unwrap_or_default()
            );
            ensure!(Instant::now() < deadline, "actual startup deadline");
            thread::sleep(Duration::from_millis(10));
        }
        let read = |path| -> Result<Value> { Ok(serde_json::from_slice(&fs::read(path)?)?) };
        let ready = read("/work/access/ready.json")?;
        ensure!(
            ready["identity"] == json!(binding.selection().identity())
                && ready["endpoint_record"]
                    == json!(hash_file(&Path::new(BRIDGE).join("endpoint.json"))?),
            "actual ready identity differs"
        );
        let namespace = read("/work/access/namespace.json")?;
        ensure!(
            namespace["uid"] == 1000
                && namespace["working_directory"] == ACCESS
                && namespace["privilege_drop"]["real_effective_saved_uid_gid"]
                    == json!(vec![1000; 6])
                && namespace["privilege_drop"]["supplementary_groups"] == json!([])
                && !namespace["file_descriptors"]
                    .to_string()
                    .contains("parent-descriptor-sentinel"),
            "actual account or descriptor isolation differs"
        );
        for kind in ["mnt", "pid", "ipc", "uts"] {
            ensure!(
                namespace["namespaces"][kind]
                    != json!(fs::read_link(format!("/proc/self/ns/{kind}"))?),
                "actual child shares {kind}"
            );
        }
        ensure!(
            namespace["namespaces"]["net"] == json!(fs::read_link("/proc/self/ns/net")?)
                && namespace["privilege_drop"]["user_namespace"]
                    == json!(fs::read_link("/proc/self/ns/user")?),
            "service changed the real server network or user namespace"
        );
        supervisor.finish()?;
        ensure!(
            read("/work/carrier-outcome.json")?["success"] == true,
            "actual service stop failed"
        );
        let outcome = read("/work/access/service-outcome.json")?;
        ensure!(
            outcome["error"].is_null()
                && outcome["receipt"]["listeners_closed"] == true
                && !Path::new(BRIDGE).join("http.sock").exists(),
            "actual service stop receipt differs"
        );
        Ok(json!({"ready":ready,"namespace":namespace,"outcome":outcome}))
    })();
    drop(supervisor);
    let records = broker.finish()?;
    println!(
        "MD_SIM_CARRIER_STARTUP_FIXTURE {}",
        json!({
        "scope":"actual production Supervisor and child; scripted worker/provider replies; no firmware or HTTP/TLS pass",
        "production_helper":hash_file(Path::new("/namespace-check"))?,
        "test_helper":hash_file(&std::env::current_exe()?)?,
        "result":result.as_ref().ok(),"error":result.as_ref().err().map(|e|format!("{e:#}")),
        "broker_peers_and_exchanges":records,"stderr":fs::read_to_string("/work/carrier.stderr")?})
    );
    ensure!(records.len() >= 4, "actual broker exchanges absent");
    result.map(|_| ())
}

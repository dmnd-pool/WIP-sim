//! A selected carrier service in its own mount and process namespaces.
use super::{shared_pool, worker_broker::bounded, Owned};
use anyhow::{ensure, Context, Result};
use md_sim_access::carrier::{
    CarrierService, Identity, ProviderBinding, ProviderLease, ProviderState, ProviderTransport,
    Selection, WorkerProviderTransport,
};
use md_sim_coordinator::{LockedRun, WorkerTransport};
use md_sim_runtime::{
    isolation::save_json,
    store::{hash_file, ObjectId},
    worker::{self, Request},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
        process::CommandExt,
    },
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub const ENVIRONMENT: &str = "MD_SIM_CARRIER_DASHBOARD";
pub const LOCK: &str = "/carrier-locked-run.json";
pub const POLICY: &str = "/carrier-dashboard.json";
pub const PROVIDER: &str = "/provider.json";
pub const ACCESS: &str = "/work/access";
pub const BRIDGE: &str = "/work/access/bridge";
const SOCKET: &str = "/worker/worker.sock";
const LAUNCH: &str = "/carrier-launch.json";
const CONTROL: &str = "/carrier-control";
const AUDIT_LIMIT: u64 = 8 << 20;

pub fn enabled() -> bool {
    std::env::var(ENVIRONMENT).as_deref() == Ok("1")
}

fn owned_bytes(path: &Path, limit: u64, owner: u32) -> Result<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file()
            && metadata.uid() == owner
            && metadata.nlink() == 1
            && metadata.mode() & 0o022 == 0
            && metadata.len() <= limit,
        "carrier metadata must be a bounded private regular file"
    );
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "carrier metadata grew beyond its bound"
    );
    Ok(bytes)
}

pub fn prepare(
    scenario: &mut Value,
    lock: &Path,
    objects: &Path,
    run: &LockedRun,
    relay: &Path,
) -> Result<()> {
    let Some(policy) = run.policies.get("carrier_dashboard") else {
        return Ok(());
    };
    let policy: ObjectId = serde_json::from_value(policy.clone())?;
    let policy_path = objects.join(&policy.sha256);
    let bytes = bounded(&policy_path, 65536)?;
    Selection::new(run, policy.clone(), &bytes, &"0".repeat(64))?;
    ensure!(
        hash_file(&policy_path)? == policy,
        "captured carrier policy differs"
    );
    scenario["environment"][ENVIRONMENT] = json!("1");
    for (guest, host) in [(LOCK, lock), (POLICY, policy_path.as_path())] {
        scenario["files"][guest] = json!({"path":host.canonicalize()?,"mode":292});
    }
    scenario["programs"]["/md-sim-access"] =
        json!(relay.with_file_name("md-sim-access").canonicalize()?);
    for program in ["/usr/bin/curl", "/usr/bin/openssl", "/usr/bin/nginx"] {
        scenario["programs"][program] = json!(Path::new(program).canonicalize()?);
    }
    // The parent captures PROVIDER after constructing the outer configuration.
    // It is outside the locked native inputs and cannot be inferred here.
    Ok(())
}

pub fn role_mounts(command: &mut Command) {
    command.args(["--setenv", ENVIRONMENT, "1"]);
    for path in [LOCK, POLICY, PROVIDER] {
        command.args(["--ro-bind", path, path]);
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Launch {
    schema: String,
    lease: shared_pool::Lease,
    lock: ObjectId,
    policy: ObjectId,
    provider: ObjectId,
}

fn binding(launch: &Launch) -> Result<ProviderBinding> {
    ensure!(
        launch.schema == "md-sim-carrier-dashboard-launch/1"
            && launch.lease.schema_version == "md-sim-shared-pool-lease/1",
        "carrier launch schema differs"
    );
    let run: LockedRun = serde_json::from_slice(&owned_bytes(Path::new(LOCK), 4 << 20, 0)?)?;
    run.validate()?;
    ensure!(
        hash_file(Path::new(LOCK))? == launch.lock
            && run.hash()? == launch.lease.locked_run_hash
            && worker::process_identity(run.process.as_ref().context("carrier process absent")?)?
                == launch.lease.process
            && launch.lease.run_id.as_deref()
                == Some(format!("run-{}", launch.lease.token).as_str())
            && launch.lease.ready.is_some()
            && launch.lease.execute_monotonic_ns.is_some(),
        "carrier launch does not name the committed selected execution"
    );
    let selected = Selection::new(
        &run,
        launch.policy.clone(),
        &owned_bytes(Path::new(POLICY), 65536, 0)?,
        &launch.lease.token,
    )?;
    ProviderBinding::from_config(
        selected,
        launch.provider.clone(),
        &owned_bytes(Path::new(PROVIDER), 4 << 20, 0)?,
    )
}

/// Root retains launch and logs. The child only owns ACCESS and the byte sockets.
pub struct Supervisor {
    selected: shared_pool::Selection,
    child: Option<Owned>,
    finished: bool,
}

impl Supervisor {
    pub fn new() -> Result<Self> {
        ensure!(
            enabled() && shared_pool::enabled(),
            "carrier requires shared selection"
        );
        let (selected, _) = shared_pool::selected()?;
        fs::create_dir(ACCESS)?;
        fs::set_permissions(ACCESS, fs::Permissions::from_mode(0o700))?;
        std::os::unix::fs::chown(ACCESS, Some(1000), Some(1000))?;
        fs::create_dir("/work/carrier-control")?;
        fs::set_permissions("/work/carrier-control", fs::Permissions::from_mode(0o755))?;
        Ok(Self {
            selected,
            child: None,
            finished: false,
        })
    }

    fn launch(&mut self, lease: shared_pool::Lease) -> Result<()> {
        ensure!(
            lease.process == self.selected.process
                && lease.locked_run_hash == self.selected.locked_run_hash
                && lease.manifest == self.selected.manifest
                && lease.pool_input == self.selected.pool_input,
            "carrier broker lease differs from frozen selection"
        );
        let launch = Launch {
            schema: "md-sim-carrier-dashboard-launch/1".into(),
            lease,
            lock: hash_file(Path::new(LOCK))?,
            policy: hash_file(Path::new(POLICY))?,
            provider: hash_file(Path::new(PROVIDER))?,
        };
        let selected = binding(&launch)?;
        save_json(Path::new("/work/carrier-launch.json"), &launch)?;
        fs::set_permissions(
            "/work/carrier-launch.json",
            fs::Permissions::from_mode(0o444),
        )?;
        let mut command = Command::new("/usr/bin/bwrap");
        command.args([
            "--unshare-pid",
            "--unshare-ipc",
            "--unshare-uts",
            "--die-with-parent",
            "--new-session",
            "--tmpfs",
            "/",
            "--ro-bind",
            "/usr",
            "/usr",
            "--ro-bind",
            "/lib64",
            "/lib64",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--tmpfs",
            "/tmp",
            "--dir",
            "/work",
            "--bind",
            ACCESS,
            ACCESS,
            "--ro-bind",
            "/worker",
            "/worker",
            "--tmpfs",
            "/worker/access-credentials",
            "--ro-bind",
            "/work/carrier-launch.json",
            LAUNCH,
            "--ro-bind",
            "/work/carrier-control",
            CONTROL,
        ]);
        for path in ["/namespace-check", LOCK, POLICY, PROVIDER] {
            command.args(["--ro-bind", path, path]);
        }
        command
            .args([
                "--cap-drop",
                "ALL",
                "--cap-add",
                "CAP_SETUID",
                "--cap-add",
                "CAP_SETGID",
                "--cap-add",
                "CAP_SETPCAP",
                "--clearenv",
                "--setenv",
                ENVIRONMENT,
                "1",
                "--chdir",
                "/",
                "--",
                "/namespace-check",
                "carrier-dashboard-service",
            ])
            .stdin(Stdio::null())
            .stdout(log(Path::new("/work/carrier.stdout"))?)
            .stderr(log(Path::new("/work/carrier.stderr"))?);
        // The child has no inherited channel. It connects through the checked
        // UID1000 broker socket. Limits also bound a stalled/erroring helper.
        unsafe {
            command.pre_exec(|| {
                for (resource, bound) in [
                    (libc::RLIMIT_AS, 256 << 20),
                    (libc::RLIMIT_FSIZE, AUDIT_LIMIT),
                    (libc::RLIMIT_NOFILE, 128),
                    (libc::RLIMIT_NPROC, 128),
                    (libc::RLIMIT_CORE, 0),
                    (libc::RLIMIT_CPU, 60),
                ] {
                    let limit = libc::rlimit {
                        rlim_cur: bound,
                        rlim_max: bound,
                    };
                    if libc::setrlimit(resource, &limit) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                if libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 4u32) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = Owned(command.spawn()?);
        save_json(
            Path::new("/work/carrier-start.json"),
            &json!({"pid":child.0.id(),
            "identity":selected.selection().identity(),"provider":selected.configuration(),
            "work":ACCESS,"network":"existing server namespace; loopback and private miner NIC",
            "declared_uid":1000,"declared_mounts_exclude":"pool scratch, client keys, server TLS keys; actual child checks are in access/namespace.json"}),
        )?;
        self.child = Some(child);
        Ok(())
    }

    pub fn poll(&mut self) -> Result<()> {
        if let Err(error) = self.poll_inner() {
            self.failure(error)?;
        }
        Ok(())
    }

    fn failure(&mut self, error: anyhow::Error) -> Result<()> {
        // Access failure must not kill the control broker before the native
        // result and pool evidence have been durably retained.
        drop(self.child.take());
        self.finished = true;
        let previous = owned_bytes(Path::new("/work/carrier-outcome.json"), 65536, 0)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
        save_json(
            Path::new("/work/carrier-outcome.json"),
            &json!({"success":false,
            "error":format!("{error:#}"),"previous":previous,"control_broker_preserved":true,
            "service_outcome":"/work/access/service-outcome.json"}),
        )
    }

    fn poll_inner(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        if self.child.is_none() {
            if !Path::new(shared_pool::LEASE).exists() {
                return Ok(());
            }
            let lease: shared_pool::Lease =
                serde_json::from_slice(&owned_bytes(Path::new(shared_pool::LEASE), 65536, 1000)?)?;
            if lease.run_id.is_none() {
                return Ok(());
            }
            self.launch(lease)?;
        }
        for path in ["/work/carrier.stdout", "/work/carrier.stderr"] {
            ensure!(
                fs::metadata(path)?.len() <= AUDIT_LIMIT,
                "carrier log exceeds bound"
            );
        }
        if let Some(status) = self
            .child
            .as_mut()
            .context("carrier child absent")?
            .0
            .try_wait()?
        {
            self.finished = true;
            save_json(
                Path::new("/work/carrier-outcome.json"),
                &json!({"success":status.success(),"exit_code":status.code(),"service_outcome":"/work/access/service-outcome.json"}),
            )?;
            ensure!(
                status.success(),
                "carrier dashboard helper failed: {status}"
            );
        }
        Ok(())
    }

    pub fn finish(&mut self) -> Result<()> {
        if self.child.is_none() {
            return Ok(());
        }
        save_json(
            Path::new("/work/carrier-control/stop.json"),
            &json!({"stop":true}),
        )?;
        let deadline = Instant::now() + Duration::from_secs(15);
        while !self.finished {
            self.poll()?;
            if Instant::now() >= deadline {
                return self.failure(anyhow::anyhow!("carrier helper stop deadline"));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }
}

fn log(path: &Path) -> Result<File> {
    Ok(OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?)
}

struct Audit {
    file: File,
    bytes: u64,
    recent: Vec<Value>,
}
struct Recorded {
    timeout: Duration,
    audit: Arc<Mutex<Audit>>,
}
impl WorkerTransport for Recorded {
    fn request(&mut self, request: &Request) -> Result<Value> {
        let result = worker::request_with_timeout(Path::new(SOCKET), request, self.timeout);
        let record = json!({"request":request,
            "response":result.as_ref().ok(),"error":result.as_ref().err().map(|e|format!("{e:#}"))});
        let mut bytes = serde_json::to_vec(&record)?;
        bytes.push(b'\n');
        let mut audit = self
            .audit
            .lock()
            .map_err(|_| anyhow::anyhow!("carrier audit lock failed"))?;
        ensure!(
            audit.bytes + bytes.len() as u64 <= AUDIT_LIMIT,
            "carrier control audit exceeds bound"
        );
        audit.file.write_all(&bytes)?;
        audit.bytes += bytes.len() as u64;
        if audit.recent.len() == 3 {
            audit.recent.remove(0);
        }
        audit.recent.push(record);
        result
    }
    fn description(&self) -> String {
        format!(
            "Unix listener {SOCKET}; absolute request timeout {:?}",
            self.timeout
        )
    }
}

fn observed(
    provider: &mut impl ProviderTransport,
    binding: &ProviderBinding,
    sequence: &mut u64,
) -> Result<ProviderLease> {
    let mut nonce = [0; 32];
    File::open("/dev/urandom")?.read_exact(&mut nonce)?;
    let challenge = hex::encode(nonce);
    let lease = provider.lease(binding, &challenge)?;
    ensure!(
        lease.schema == "md-sim-carrier-provider-lease/1"
            && lease.identity == *binding.selection().identity()
            && lease.configuration == *binding.configuration()
            && lease.challenge == challenge
            && lease.sequence > *sequence,
        "carrier provider observation identity or sequence differs"
    );
    *sequence = lease.sequence;
    Ok(lease)
}

// A refresh can race a legitimate end of the selected job. Accept only the
// exact successful replies that caused that terminal-state rejection. Keep
// the raw refresh error and the unmodified service receipt in the result.
fn terminal_refresh(
    records: &[Value],
    identity: &Identity,
    configuration: &ObjectId,
    sequence: u64,
    cause: &str,
) -> Option<Value> {
    if !(2..=3).contains(&records.len()) || records.iter().any(|record| !record["error"].is_null())
    {
        return None;
    }
    if records[0]["request"] != json!({"operation":"lookup-token","token":identity.token})
        || records[1]["request"] != json!({"operation":"status","run_id":identity.run_id})
    {
        return None;
    }
    let (lookup, status) = (&records[0]["response"], &records[1]["response"]);
    if lookup["found"] != true
        || lookup["token"] != identity.token
        || lookup["run_id"] != identity.run_id
        || lookup["token_process_identity"] != json!(identity.process)
        || status["run_id"] != identity.run_id
    {
        return None;
    }
    let lookup_state = lookup["state"].as_str()?;
    let status_state = status["state"]["state"].as_str()?;
    let ordinary = |state| matches!(state, "booting" | "stopped" | "cleaned");
    if !ordinary(lookup_state)
        || !ordinary(status_state)
        || (status_state != "cleaned" && status.get("dashboard") != Some(&Value::Null))
    {
        return None;
    }
    if records.len() == 2
        && cause == "selected native worker is not active or its identity differs"
        && (lookup_state != "booting" || status_state != "booting")
    {
        return Some(json!({"terminal":"selected-worker-ended","observations":records}));
    }
    if records.len() == 3
        && lookup_state == "booting"
        && status_state == "booting"
        && cause == "selected owning provider is not live or its lease differs"
    {
        let query = &records[2]["request"];
        let lease: ProviderLease = serde_json::from_value(records[2]["response"].clone()).ok()?;
        if query["operation"] == "private-channel-status"
            && query["token"] == identity.token
            && query["challenge"] == lease.challenge
            && lease.challenge.len() == 64
            && lease
                .challenge
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && lease.schema == "md-sim-carrier-provider-lease/1"
            && lease.identity == *identity
            && lease.configuration == *configuration
            && lease.sequence > sequence
            && lease.state == ProviderState::Closed
        {
            return Some(
                json!({"terminal":"provider-closed-during-refresh","observations":records}),
            );
        }
    }
    None
}

fn wait_worker_end(worker: &mut Recorded, binding: &ProviderBinding) -> Result<Value> {
    let identity = binding.selection().identity();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let lookup_request = Request::LookupToken {
            token: identity.token.clone(),
        };
        let status_request = Request::Status {
            run_id: identity.run_id.clone(),
        };
        worker.timeout = worker.timeout.min(
            deadline
                .checked_duration_since(Instant::now())
                .context("carrier terminal worker deadline")?,
        );
        let lookup = worker.request(&lookup_request)?;
        worker.timeout = worker.timeout.min(
            deadline
                .checked_duration_since(Instant::now())
                .context("carrier terminal worker deadline")?,
        );
        let status = worker.request(&status_request)?;
        ensure!(
            Instant::now() < deadline,
            "carrier terminal worker deadline"
        );
        let records = [
            json!({"request":lookup_request,"response":lookup,"error":null}),
            json!({"request":status_request,"response":status,"error":null}),
        ];
        if let Some(proof) = terminal_refresh(
            &records,
            identity,
            binding.configuration(),
            0,
            "selected native worker is not active or its identity differs",
        ) {
            return Ok(proof);
        }
        ensure!(
            Instant::now() < deadline,
            "carrier closed while selected worker remained active, failed, or mismatched"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn service() -> Result<()> {
    let privileges = drop_service_privileges()?;
    std::env::set_current_dir(ACCESS)?;
    ensure!(
        enabled()
            && unsafe { libc::geteuid() } == 1000
            && fs::read_to_string("/proc/cmdline")?
                .split_whitespace()
                .any(|v| v == "md_sim_private_guest=1"),
        "carrier service requires its isolated guest account"
    );
    for path in [
        "/work/pool",
        "/work/broker",
        "/broker-inputs",
        "/credentials",
        "/work/clientkey",
        "/worker/access-credentials/server.key",
    ] {
        ensure!(
            !Path::new(path).exists(),
            "carrier service sees unrelated {path}"
        );
    }
    let mut namespace = super::namespace_evidence("carrier-dashboard")?;
    namespace["privilege_drop"] = privileges;
    namespace["working_directory"] = json!(std::env::current_dir()?);
    namespace["file_descriptors"] = json!(fs::read_dir("/proc/self/fd")?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| fs::read_link(entry.path()).ok())
        .collect::<Vec<_>>());
    save_json(Path::new("/work/access/namespace.json"), &namespace)?;
    let result = service_inner();
    if let Err(error) = &result {
        save_json(
            Path::new("/work/access/service-failure.json"),
            &json!({"error":format!("{error:#}")}),
        )?;
    }
    result
}

fn drop_service_privileges() -> Result<Value> {
    ensure!(
        enabled()
            && unsafe { libc::getuid() } == 0
            && unsafe { libc::geteuid() } == 0
            && fs::read_to_string("/proc/cmdline")?
                .split_whitespace()
                .any(|value| value == "md_sim_private_guest=1"),
        "carrier privilege entry requires the isolated VM root"
    );
    // Keep the real support user namespace. A new UID map would make broker
    // SO_PEERCRED and the existing UID1000 files name a different account.
    // bwrap retains only these three capabilities for this trusted entry.
    for capability in 0..64 {
        if unsafe { libc::prctl(libc::PR_CAPBSET_DROP, capability, 0, 0, 0) } != 0 {
            ensure!(
                std::io::Error::last_os_error().raw_os_error() == Some(libc::EINVAL),
                "carrier bounding capability drop failed"
            );
            break;
        }
    }
    ensure!(
        unsafe { libc::setgroups(0, std::ptr::null()) } == 0
            && unsafe { libc::setresgid(1000, 1000, 1000) } == 0
            && unsafe { libc::setresuid(1000, 1000, 1000) } == 0,
        "carrier account drop failed"
    );
    // UID changes clear effective/permitted/ambient capabilities. Explicitly
    // clear inheritable capabilities too, then prohibit privilege gain.
    let header = [0x2008_0522u32, 0]; // Linux capability ABI version3, this PID.
    let data = [0u32; 6];
    ensure!(
        unsafe { libc::syscall(libc::SYS_capset, header.as_ptr(), data.as_ptr()) } == 0
            && unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } == 0
            && unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } == 0,
        "carrier capability drop failed"
    );
    let mut ids = [0u32; 6];
    ensure!(
        unsafe { libc::getresuid(&mut ids[0], &mut ids[1], &mut ids[2]) } == 0
            && unsafe { libc::getresgid(&mut ids[3], &mut ids[4], &mut ids[5]) } == 0
            && ids == [1000; 6]
            && unsafe { libc::getgroups(0, std::ptr::null_mut()) } == 0,
        "carrier real, effective, saved IDs or groups differ"
    );
    let status = fs::read_to_string("/proc/self/status")?;
    for key in ["CapInh:", "CapPrm:", "CapEff:", "CapBnd:", "CapAmb:"] {
        ensure!(
            status
                .lines()
                .any(|line| { line.strip_prefix(key).map(str::trim) == Some("0000000000000000") }),
            "carrier retains {key}"
        );
    }
    ensure!(
        status
            .lines()
            .any(|line| line.strip_prefix("NoNewPrivs:").map(str::trim) == Some("1")),
        "carrier no-new-privileges differs"
    );
    Ok(json!({"real_effective_saved_uid_gid":ids,
        "supplementary_groups":[],"proc_status":status,
        "user_namespace":fs::read_link("/proc/self/ns/user")?,
        "uid_map":fs::read_to_string("/proc/self/uid_map")?}))
}

#[cfg(test)]
#[path = "carrier_dashboard_startup.rs"]
mod startup_tests;

fn service_inner() -> Result<()> {
    let launch: Launch = serde_json::from_slice(&owned_bytes(Path::new(LAUNCH), 65536, 0)?)?;
    let binding = binding(&launch)?;
    let audit = Arc::new(Mutex::new(Audit {
        file: log(Path::new("/work/access/control.jsonl"))?,
        bytes: 0,
        recent: Vec::new(),
    }));
    let transport = || Recorded {
        timeout: Duration::from_millis(
            (binding.selection().policy().lease_milliseconds / 4).clamp(25, 1000),
        ),
        audit: audit.clone(),
    };
    let mut worker = transport();
    let mut provider = WorkerProviderTransport::new(transport());
    let mut sequence = 0;
    let startup = Instant::now() + Duration::from_secs(10);
    loop {
        let lease = observed(&mut provider, &binding, &mut sequence)?;
        match lease.state {
            ProviderState::Active => break,
            ProviderState::Waiting => ensure!(
                Instant::now() < startup,
                "carrier provider handoff deadline"
            ),
            _ => anyhow::bail!("carrier provider ended before dashboard startup"),
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let mut service = CarrierService::start(
        binding.clone(),
        &mut worker,
        &mut provider,
        Path::new(BRIDGE),
    )?;
    save_json(
        Path::new("/work/access/ready.json"),
        &json!({"schema":"md-sim-carrier-dashboard-ready/1","identity":service.endpoint().identity,
        "provider_configuration":service.endpoint().provider_configuration,
        "endpoint_record":hash_file(&Path::new(BRIDGE).join("endpoint.json"))?,
        "endpoint_path":Path::new(BRIDGE).join("endpoint.json"),"firmware_readiness_established":false}),
    )?;
    let interval = Duration::from_millis(
        (binding.selection().policy().lease_milliseconds / 4).clamp(25, 1000),
    );
    let mut normal_refresh_end = false;
    let mut confirm_worker_end = false;
    let outcome = (|| -> Result<Value> {
        loop {
            if Path::new(CONTROL).join("stop.json").exists() {
                ensure!(
                    serde_json::from_slice::<Value>(&owned_bytes(
                        &Path::new(CONTROL).join("stop.json"),
                        64,
                        0
                    )?)?["stop"]
                        == true,
                    "carrier stop control differs"
                );
                return Ok(json!({"terminal":"supervisor-stop"}));
            }
            if !service.is_running() {
                return Ok(json!({"terminal":"service-ended"}));
            }
            let lease = observed(&mut provider, &binding, &mut sequence)?;
            match lease.state {
                ProviderState::Closed => {
                    confirm_worker_end = true;
                    return Ok(json!({"terminal":"provider-closed","observation":lease}));
                }
                ProviderState::Failed => anyhow::bail!("actual carrier provider failed"),
                ProviderState::Waiting => {
                    anyhow::bail!("active carrier provider returned to waiting")
                }
                ProviderState::Active => {}
            }
            // Refresh retains the actual selected LookupToken, Status and
            // owning-provider reply. It also independently closes on expiry.
            audit
                .lock()
                .map_err(|_| anyhow::anyhow!("carrier audit lock failed"))?
                .recent
                .clear();
            if let Err(error) = service.refresh(&mut worker, &mut provider) {
                let recent = &audit
                    .lock()
                    .map_err(|_| anyhow::anyhow!("carrier audit lock failed"))?
                    .recent;
                if let Some(terminal) = terminal_refresh(
                    recent,
                    binding.selection().identity(),
                    binding.configuration(),
                    sequence,
                    &error.to_string(),
                ) {
                    normal_refresh_end = true;
                    confirm_worker_end = terminal["terminal"] == "provider-closed-during-refresh";
                    return Ok(
                        json!({"terminal_proof":terminal,"refresh_error":format!("{error:#}")}),
                    );
                }
                return Err(error);
            }
            std::thread::sleep(interval);
        }
    })();
    let receipt = service.stop()?;
    // EOF closes access immediately. It is not a normal job-end assertion:
    // an unrelated peer failure can close the carrier while the guest lives.
    let terminal_worker = if confirm_worker_end {
        wait_worker_end(&mut worker, &binding).map(Some)
    } else {
        Ok(None)
    };
    let report = json!({"schema":"md-sim-support-carrier-dashboard-result/1",
        "identity":binding.selection().identity(),"outcome":outcome.as_ref().ok(),
        "error":outcome.as_ref().err().map(|e|format!("{e:#}")),"receipt":receipt,
        "normal_refresh_terminal":normal_refresh_end && terminal_worker.is_ok(),
        "worker_terminal":terminal_worker.as_ref().ok(),"worker_terminal_error":terminal_worker.as_ref().err().map(|e|format!("{e:#}")),
        "control":hash_file(Path::new("/work/access/control.jsonl"))?,"firmware_readiness_established":false});
    save_json(Path::new("/work/access/service-outcome.json"), &report)?;
    outcome?;
    terminal_worker?;
    ensure!(
        (receipt.error.is_none() || normal_refresh_end)
            && receipt.listeners_closed
            && receipt.end != "lease-expired",
        "carrier service ended with an error or expired owner lease"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_refresh_requires_the_actual_selected_terminal_replies() {
        let object = ObjectId {
            sha256: "a".repeat(64),
            size: 1,
        };
        let identity = Identity {
            locked_run: object.clone(),
            process: object.clone(),
            policy: object.clone(),
            support: object.clone(),
            token: "b".repeat(64),
            run_id: format!("run-{}", "b".repeat(64)),
        };
        let mut records = vec![
            json!({"request":{"operation":"lookup-token","token":identity.token},"response":{
                "found":true,"token":identity.token,"run_id":identity.run_id,"token_process_identity":object,"state":"booting"},"error":null}),
            json!({"request":{"operation":"status","run_id":identity.run_id},"response":{
                "run_id":identity.run_id,"state":{"state":"stopped"},"dashboard":null},"error":null}),
        ];
        let worker_cause = "selected native worker is not active or its identity differs";
        assert!(terminal_refresh(&records, &identity, &object, 4, worker_cause).is_some());
        for bad in ["failed", "booting", "unknown"] {
            let mut changed = records.clone();
            changed[1]["response"]["state"]["state"] = json!(bad);
            assert!(terminal_refresh(&changed, &identity, &object, 4, worker_cause).is_none());
        }
        let mut changed = records.clone();
        changed[0]["response"]["token_process_identity"]["size"] = json!(2);
        assert!(terminal_refresh(&changed, &identity, &object, 4, worker_cause).is_none());
        let mut changed = records.clone();
        changed[1]["error"] = json!("timed out");
        assert!(terminal_refresh(&changed, &identity, &object, 4, worker_cause).is_none());
        assert!(terminal_refresh(
            &records,
            &identity,
            &object,
            4,
            "carrier lease expired or was replayed"
        )
        .is_none());
        records[1]["response"]["state"]["state"] = json!("booting");
        let challenge = "c".repeat(64);
        records.push(json!({"request":{"operation":"private-channel-status","token":identity.token,"challenge":challenge},
            "response":{"schema":"md-sim-carrier-provider-lease/1","identity":identity,"configuration":object,
                "challenge":challenge,"sequence":5,"state":"closed"},"error":null}));
        let provider_cause = "selected owning provider is not live or its lease differs";
        assert!(terminal_refresh(&records, &identity, &object, 4, provider_cause).is_some());
        for (field, value) in [
            ("sequence", json!(4)),
            ("state", json!("failed")),
            ("challenge", json!("d".repeat(64))),
            ("configuration", json!({"sha256":"e".repeat(64),"size":1})),
        ] {
            let mut changed = records.clone();
            changed[2]["response"][field] = value;
            assert!(terminal_refresh(&changed, &identity, &object, 4, provider_cause).is_none());
        }
    }

    #[test]
    fn control_reader_rejects_links_fifo_growth_and_wrong_owner() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("control");
        fs::write(&path, b"abc").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let uid = unsafe { libc::geteuid() };
        assert_eq!(owned_bytes(&path, 3, uid).unwrap(), b"abc");
        assert!(owned_bytes(&path, 2, uid).is_err());
        assert!(owned_bytes(&path, 3, uid + 1).is_err());
        let link = directory.path().join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(owned_bytes(&link, 3, uid).is_err());
        fs::hard_link(&path, &link.with_extension("hard")).unwrap();
        assert!(owned_bytes(&path, 3, uid).is_err());
        let fifo = directory.path().join("fifo");
        let name = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        assert!(owned_bytes(&fifo, 3, uid).is_err());
    }
}

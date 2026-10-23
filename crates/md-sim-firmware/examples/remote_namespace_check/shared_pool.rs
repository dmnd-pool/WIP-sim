//! Selected S9 pool lifecycle. Target policy stays outside the generic broker.
use super::{
    shared_manifest::{self, Manifest},
    worker_broker::bounded,
};
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::{LockedRun, WorkerTransport};
use md_sim_runtime::{
    isolation::save_json,
    store::{hash_file, ObjectId},
    worker::{self, Request},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

pub const ENVIRONMENT: &str = "MD_SIM_SHARED_POOL";
pub const SELECTION: &str = "/shared-selection.json";
pub const SERVER_SECONDS: u64 = 1200;
pub const LEASE: &str = "/work/broker/shared-lease.json";
pub const READY: &str = "/work/shared-ready.json";

pub fn enabled() -> bool {
    std::env::var(ENVIRONMENT).as_deref() == Ok("1")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub schema_version: String,
    pub locked_run_hash: String,
    pub process: ObjectId,
    pub manifest: ObjectId,
    pub pool_input: ObjectId,
    pub module: ObjectId,
}

pub fn selected() -> Result<(Selection, Manifest)> {
    let selected: Selection = serde_json::from_slice(&bounded(Path::new(SELECTION), 65536)?)?;
    ensure!(
        selected.schema_version == "md-sim-shared-pool-selection/1",
        "shared selection schema differs"
    );
    for object in [
        &selected.process,
        &selected.manifest,
        &selected.pool_input,
        &selected.module,
    ] {
        object.validate()?;
    }
    ensure!(
        selected.locked_run_hash.len() == 64
            && selected
                .locked_run_hash
                .bytes()
                .all(|b| b.is_ascii_hexdigit()),
        "shared locked hash differs"
    );
    ensure!(
        hash_file(Path::new("/shared-pool.json"))? == selected.manifest
            && hash_file(Path::new("/private-pool.json"))? == selected.pool_input,
        "selected pool input changed"
    );
    let (manifest, _) = shared_manifest::read(Path::new("/shared-pool.json"))?;
    ensure!(
        hash_file(Path::new("/pool-guest"))? == manifest.pool_verifier,
        "captured pool verifier differs"
    );
    Ok((selected, manifest))
}

pub struct Support<'a> {
    pub relay: &'a Path,
    pub veth: &'a Path,
    pub pool: &'a Path,
    pub module: &'a Path,
    pub browser_payload: Option<&'a Path>,
}

pub fn prepare(out: &Path, lock: &Path, objects: &Path, support: Support<'_>) -> Result<()> {
    let Support {
        relay,
        veth,
        pool,
        module,
        browser_payload,
    } = support;
    let run: LockedRun = serde_json::from_slice(&bounded(lock, 4 << 20)?)?;
    run.validate()?;
    let process = run.process.as_ref().context("shared process absent")?;
    let input = |name: &str| -> Result<ObjectId> {
        Ok(process
            .inputs
            .iter()
            .find(|mount| mount.guest_path == name)
            .context("required shared process mount absent")?
            .object
            .clone())
    };
    let manifest_id = input("/shared-pool.json")?;
    let pool_input = input("/private-pool.json")?;
    let manifest_path = objects.join(&manifest_id.sha256);
    let input_path = objects.join(&pool_input.sha256);
    ensure!(
        hash_file(&manifest_path)? == manifest_id && hash_file(&input_path)? == pool_input,
        "shared process input hash differs"
    );
    let (manifest, _) = shared_manifest::read(&manifest_path)?;
    let pool_policy = super::pool_input::read(&input_path)?;
    manifest.observation_seconds(pool_policy.seconds, process.limits.wall_seconds)?;
    ensure!(
        serde_json::to_value(&process.private_channel)?
            == serde_json::to_value(Some(&manifest.carrier))?
            && process
                .environment
                .get(shared_manifest::ENVIRONMENT)
                .map(String::as_str)
                == Some("1"),
        "selected process lacks exact shared carrier mode"
    );
    ensure!(
        hash_file(pool)? == manifest.pool_verifier,
        "pool verifier build differs from shared manifest"
    );
    let selected = Selection {
        schema_version: "md-sim-shared-pool-selection/1".into(),
        locked_run_hash: run.hash()?,
        process: worker::process_identity(process)?,
        manifest: manifest_id,
        pool_input,
        module: hash_file(module)?,
    };
    let selection_path = out.with_extension("shared-selection.json");
    ensure!(
        !selection_path.exists(),
        "shared selection output already exists"
    );
    super::worker_broker::prepare(out, lock, objects, relay, veth)?;
    save_json(&selection_path, &selected)?;
    let mut scenario: Value = serde_json::from_slice(&bounded(out, 4 << 20)?)?;
    scenario["environment"][ENVIRONMENT] = json!("1");
    scenario["programs"]["/pool-guest"] = json!(pool.canonicalize()?);
    for (guest, host) in [
        (SELECTION, selection_path),
        ("/shared-pool.json", manifest_path),
        ("/private-pool.json", input_path),
        ("/e1000.ko.zst", module.canonicalize()?),
    ] {
        scenario["files"][guest] = json!({"path":host.canonicalize()?,"mode":292});
    }
    super::carrier_dashboard::prepare(&mut scenario, lock, objects, &run, relay)?;
    if let Some(payload) = browser_payload {
        super::board_browser::prepare_carrier(&mut scenario, payload, relay)?;
    }
    save_json(out, &scenario)
}

pub fn role_mounts(command: &mut Command, role: &str) {
    command.args(["--setenv", ENVIRONMENT, "1"]);
    if super::carrier_dashboard::enabled() {
        super::carrier_dashboard::role_mounts(command);
    }
    if role == "server" {
        for path in [
            SELECTION,
            "/shared-pool.json",
            "/private-pool.json",
            "/pool-guest",
        ] {
            command.args(["--ro-bind", path, path]);
        }
    }
}

pub fn configure_network(events: &mut Vec<Value>) -> Result<()> {
    let (selected, _) = selected()?;
    ensure!(
        hash_file(Path::new("/e1000.ko.zst"))? == selected.module,
        "selected Ethernet module differs"
    );
    super::invoke(
        "/usr/bin/zstd",
        &["--decompress", "/e1000.ko.zst", "-o", "/work/e1000.ko"],
    )?;
    super::invoke("/usr/bin/insmod", &["/work/e1000.ko"])?;
    let interfaces = super::invoke("/usr/bin/ip", &["-json", "link", "show"])?;
    let links: Vec<Value> = serde_json::from_slice(&interfaces)?;
    ensure!(
        links.iter().filter(|link| link["ifname"] == "eth0").count() == 1,
        "actual shared Ethernet NIC absent"
    );
    ensure!(
        fs::read_to_string("/sys/class/net/eth0/device/vendor")?.trim() == "0x8086"
            && fs::read_to_string("/sys/class/net/eth0/device/device")?.trim() == "0x100e",
        "shared NIC is not the declared e1000"
    );
    super::invoke(
        "/usr/bin/ip",
        &["link", "set", "eth0", "netns", "md-worker"],
    )?;
    super::invoke(
        "/usr/bin/ip",
        &[
            "-n",
            "md-worker",
            "address",
            "add",
            "10.0.2.100/24",
            "dev",
            "eth0",
        ],
    )?;
    super::invoke(
        "/usr/bin/ip",
        &["-n", "md-worker", "link", "set", "eth0", "up"],
    )?;
    let checks = super::invoke(
        "/usr/bin/ip",
        &[
            "netns",
            "exec",
            "md-worker",
            "/namespace-check",
            "check-shared-network",
        ],
    )?;
    let checks: Value = serde_json::from_slice(&checks)?;
    events.push(json!({"shared_network":checks,"module":selected.module,"decoded_module":hash_file(Path::new("/work/e1000.ko"))?}));
    Ok(())
}

pub fn network_check() -> Result<()> {
    ensure!(
        enabled()
            && unsafe { libc::geteuid() } == 0
            && fs::read_to_string("/proc/cmdline")?.contains("md_sim_private_guest=1"),
        "shared network check requires guest root"
    );
    let mut forwarding = serde_json::Map::new();
    for path in [
        "/proc/sys/net/ipv4/ip_forward",
        "/proc/sys/net/ipv4/conf/all/forwarding",
        "/proc/sys/net/ipv6/conf/all/forwarding",
        "/proc/sys/net/ipv6/conf/default/forwarding",
    ] {
        if Path::new(path).exists() {
            fs::write(path, b"0\n")?;
            ensure!(
                fs::read_to_string(path)?.trim() == "0",
                "shared network forwarding remains enabled"
            );
            forwarding.insert(path.into(), json!(0));
        } else {
            ensure!(
                path.contains("ipv6") && !Path::new("/proc/net/if_inet6").exists(),
                "required forwarding policy absent"
            );
            forwarding.insert(path.into(), json!("IPv6 subsystem absent"));
        }
    }
    let routes = String::from_utf8(super::invoke("/usr/bin/ip", &["route", "show"])?)?;
    ensure!(
        !routes.lines().any(|line| line.starts_with("default")),
        "shared server has a default route"
    );
    let addresses: Value = serde_json::from_slice(&super::invoke(
        "/usr/bin/ip",
        &["-json", "address", "show"],
    )?)?;
    let mut names = addresses
        .as_array()
        .context("interface list absent")?
        .iter()
        .map(|value| value["ifname"].as_str().context("interface name absent"))
        .collect::<Result<Vec<_>>>()?;
    names.sort_unstable();
    ensure!(
        names == ["eth0", "lo", "worker0"],
        "unexpected shared server interface"
    );
    println!(
        "{}",
        json!({"interfaces":addresses,"routes":routes,"forwarding":forwarding,"private_miner_network":"10.0.2.0/24","private_ssh_network":"192.0.2.0/29"})
    );
    Ok(())
}

pub fn monotonic_ns() -> Result<u64> {
    let mut value: libc::timespec = unsafe { std::mem::zeroed() };
    ensure!(
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut value) } == 0,
        "monotonic clock read failed"
    );
    let seconds: u64 = value.tv_sec.try_into()?;
    let nanos: u64 = value.tv_nsec.try_into()?;
    seconds
        .checked_mul(1_000_000_000)
        .and_then(|v| v.checked_add(nanos))
        .context("monotonic clock overflow")
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lease {
    pub schema_version: String,
    pub token: String,
    pub process: ObjectId,
    pub locked_run_hash: String,
    pub manifest: ObjectId,
    pub pool_input: ObjectId,
    pub pool_verifier: ObjectId,
    pub request_monotonic_ns: u64,
    pub ready_monotonic_ns: Option<u64>,
    pub execute_monotonic_ns: Option<u64>,
    pub ready: Option<Value>,
    pub run_id: Option<String>,
}

pub struct Gate<T> {
    inner: T,
    selected: Selection,
    manifest: Manifest,
    lease: Option<Lease>,
}
impl<T: WorkerTransport> Gate<T> {
    pub fn new(inner: T) -> Result<Self> {
        let (selected, manifest) = selected()?;
        Ok(Self {
            inner,
            selected,
            manifest,
            lease: None,
        })
    }
    fn prepare(
        &mut self,
        token: &str,
        spec: &md_sim_runtime::isolation::ProcessSpec,
    ) -> Result<()> {
        ensure!(
            token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()),
            "shared execution token differs"
        );
        ensure!(
            worker::process_identity(spec)? == self.selected.process,
            "shared execution process differs"
        );
        if let Some(lease) = &self.lease {
            ensure!(
                lease.token == token,
                "shared carrier already belongs to another token"
            );
            ensure!(
                lease.ready.is_some(),
                "selected pool readiness previously failed"
            );
            return Ok(());
        }
        let lease = Lease {
            schema_version: "md-sim-shared-pool-lease/1".into(),
            token: token.into(),
            process: self.selected.process.clone(),
            locked_run_hash: self.selected.locked_run_hash.clone(),
            manifest: self.selected.manifest.clone(),
            pool_input: self.selected.pool_input.clone(),
            pool_verifier: self.manifest.pool_verifier.clone(),
            request_monotonic_ns: monotonic_ns()?,
            ready_monotonic_ns: None,
            execute_monotonic_ns: None,
            ready: None,
            run_id: None,
        };
        save_json(Path::new(LEASE), &lease)?;
        self.lease = Some(lease);
        let deadline = Instant::now() + Duration::from_secs(self.manifest.readiness_seconds);
        loop {
            if Path::new(READY).exists() {
                let file = OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                    .open(READY)?;
                let metadata = file.metadata()?;
                ensure!(
                    metadata.is_file()
                        && metadata.uid() == 0
                        && metadata.nlink() == 1
                        && metadata.mode() & 0o022 == 0
                        && metadata.len() <= 65536,
                    "pool readiness must be a bounded root-owned regular file"
                );
                let mut bytes = Vec::new();
                file.take(65537).read_to_end(&mut bytes)?;
                ensure!(bytes.len() <= 65536, "pool readiness grew beyond its bound");
                let ready: Value = serde_json::from_slice(&bytes)?;
                ensure!(
                    ready["error"].is_null(),
                    "selected pool startup failed: {}",
                    ready["error"]
                );
                ensure!(
                    ready["token"] == token
                        && ready["process"] == serde_json::to_value(&self.selected.process)?,
                    "pool readiness belongs to another execution"
                );
                let record = &ready["ready"];
                ensure!(
                    record["endpoint"] == self.manifest.pool_endpoint
                        && record["input"] == serde_json::to_value(&self.selected.pool_input)?
                        && record["verifier"]
                            == serde_json::to_value(&self.manifest.pool_verifier)?,
                    "actual pool readiness identity differs"
                );
                let lease = self.lease.as_mut().context("shared lease absent")?;
                lease.ready = Some(record.clone());
                lease.ready_monotonic_ns = Some(monotonic_ns()?);
                lease.execute_monotonic_ns = Some(monotonic_ns()?);
                save_json(Path::new(LEASE), lease)?;
                return Ok(());
            }
            ensure!(
                Instant::now() < deadline,
                "selected pool readiness deadline"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
fn rejected(error: anyhow::Error) -> Result<Value> {
    let mut bytes = serde_json::to_vec(
        &json!({"protocol":worker::PROTOCOL_VERSION,"ok":false,"error":format!("shared pool gate: {error:#}")}),
    )?;
    bytes.push(b'\n');
    worker::decode_response(&bytes)
}
impl<T: WorkerTransport> WorkerTransport for Gate<T> {
    fn request(&mut self, request: &Request) -> Result<Value> {
        match request {
            Request::Execute { .. } => {
                return rejected(anyhow::anyhow!("shared execution requires its token"))
            }
            Request::ExecuteToken { token, spec } => {
                if let Err(error) = self.prepare(token, spec) {
                    return rejected(error);
                }
            }
            _ => {}
        }
        let response = self.inner.request(request)?;
        if matches!(request, Request::ExecuteToken { .. }) {
            let id = response["run_id"]
                .as_str()
                .context("selected native run ID absent")?;
            let lease = self.lease.as_mut().context("selected lease absent")?;
            ensure!(
                lease.run_id.as_ref().is_none_or(|old| old == id),
                "same token returned a different run"
            );
            lease.run_id = Some(id.into());
            save_json(Path::new(LEASE), lease)?;
        }
        if let Request::Stop { run_id } = request {
            if let Some(lease) = &self.lease {
                if lease.run_id.as_deref() == Some(run_id.as_str()) {
                    save_json(
                        Path::new("/work/broker/pool-cancel-request.json"),
                        &json!({"token":lease.token,"run_id":run_id}),
                    )?;
                }
            }
        }
        Ok(response)
    }
    fn description(&self) -> String {
        format!(
            "{}; exact selected shared-pool readiness gate",
            self.inner.description()
        )
    }
}

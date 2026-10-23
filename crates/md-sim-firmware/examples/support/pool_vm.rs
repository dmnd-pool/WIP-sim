//! Captured Linux pool VM with one private inherited Ethernet endpoint.
use super::pool_input;
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{save_json, Mount},
    store::{hash_file, Store},
};
use serde_json::{json, Value};
use std::{
    fs::File,
    io::Read,
    os::{
        fd::AsRawFd,
        unix::{
            net::UnixStream,
            process::{CommandExt, ExitStatusExt},
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
pub const KERNEL: &str = "/usr/lib/modules/7.2.9-arch1-1/vmlinuz";
const KERNEL_HASH: &str = "d02afb36d5f99390fdd8d04dedf8894354f4bbdc5e7d44afa5b897639bb3707c";
const DRIVER: &str =
    "/usr/lib/modules/7.2.9-arch1-1/kernel/drivers/net/ethernet/intel/e1000/e1000.ko.zst";
const DRIVER_HASH: &str = "6542f4d97c2e1424dd65b8ce6aa667bdc1a25d542c6dce521f5593ba69f2f210";
pub const MINER_NIC: &str = "socket,model=cadence_gem,fd=3,x-md-sim-private-stream=on";
pub const STARTUP_ENVIRONMENT: &str = "MD_SIM_S9_POOL_STARTUP_SECONDS";

pub fn startup_seconds(browser: bool, explicit: Option<&str>) -> Result<u64> {
    if let Some(value) = explicit {
        ensure!(!browser, "browser pool keeps its separate startup budget");
        let seconds: u64 = value.parse()?;
        ensure!(
            (20..=120).contains(&seconds),
            "pool startup deadline must be20..120 seconds"
        );
        Ok(seconds)
    } else {
        Ok(if browser {
            browser_budget::STARTUP_SECONDS
        } else {
            20
        })
    }
}
#[allow(dead_code)]
#[path = "s9_browser_budget.rs"]
pub mod browser_budget;
use super::pool_report as mining_report;
use md_sim_firmware::s9_dashboard_state as dashboard_state;
pub use md_sim_firmware::s9_update_validation as update_validation;
#[allow(dead_code)]
#[path = "s9_wall_clock.rs"]
pub mod wall_clock;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DashboardSuite<'a> {
    None,
    Basic,
    Luci,
    UpdateValidation,
    UpdateInstall,
    UpdateRejected(&'a Path),
}

pub fn prepare(
    out: &Path,
    input: &Path,
    suite: DashboardSuite<'_>,
    state: Option<&Path>,
    browser_payload: Option<&Path>,
    clock_epoch: Option<u64>,
) -> Result<()> {
    let dashboard = suite != DashboardSuite::None;
    ensure!(
        state.is_none() || dashboard,
        "dashboard state needs its probe"
    );
    ensure!(
        hash_file(Path::new(KERNEL))?.sha256 == KERNEL_HASH,
        "pool kernel changed"
    );
    ensure!(
        hash_file(Path::new(DRIVER))?.sha256 == DRIVER_HASH,
        "pool Ethernet driver changed"
    );
    let directory = std::env::current_exe()?
        .parent()
        .context("tool directory missing")?
        .to_owned();
    for name in ["isolated_network_vm", "s9_pool_guest"] {
        let path = directory.join(name);
        let metadata = std::fs::metadata(&path)
            .with_context(|| format!("required pool preparation tool: {}", path.display()))?;
        ensure!(
            metadata.is_file(),
            "pool preparation tool is not a file: {}",
            path.display()
        );
    }
    let scenario = out.with_extension("scenario.json");
    ensure!(
        !out.exists() && !scenario.exists(),
        "pool preparation output exists"
    );
    let mut configuration = json!({
        "programs":{
            "/pool-guest":directory.join("s9_pool_guest"),
            "/usr/bin/zstd":"/usr/bin/zstd"
        },
        "files":{
            "/e1000.ko.zst":{"path":DRIVER,"mode":420},
            "/private-pool.json":{"path":input.canonicalize()?,"mode":420}
        },
        "argv":["/pool-guest"],"environment":{}
    });
    if dashboard {
        let contract = out.with_extension("dashboard-contract");
        std::fs::write(&contract, b"md-sim-original-s9-dashboard-probe/1\n")?;
        configuration["programs"]["/usr/bin/curl"] = json!("/usr/bin/curl");
        configuration["files"]["/dashboard-probe"] =
            json!({"path":contract.canonicalize()?,"mode":420});
    }
    if matches!(
        suite,
        DashboardSuite::Luci
            | DashboardSuite::UpdateValidation
            | DashboardSuite::UpdateInstall
            | DashboardSuite::UpdateRejected(_)
    ) {
        let contract = out.with_extension("luci-contract");
        std::fs::write(&contract, b"md-sim-original-s9-luci-probe/1\n")?;
        configuration["files"]["/dashboard-luci"] =
            json!({"path":contract.canonicalize()?,"mode":420});
    }
    if matches!(
        suite,
        DashboardSuite::UpdateValidation
            | DashboardSuite::UpdateInstall
            | DashboardSuite::UpdateRejected(_)
    ) {
        let (action, path) = match suite {
            DashboardSuite::UpdateRejected(path) => {
                (update_validation::Action::RejectTampered, path)
            }
            DashboardSuite::UpdateInstall => (
                update_validation::Action::InstallKeep,
                Path::new(update_validation::ARCHIVE_IMAGE),
            ),
            _ => (
                update_validation::Action::ValidateCancel,
                Path::new(update_validation::ARCHIVE_IMAGE),
            ),
        };
        ensure!(
            browser_payload.is_none(),
            "update validation uses a separate HTTP suite"
        );
        let object = action.checked_image(path)?;
        let parent = update_validation::checked_image(Path::new(update_validation::ARCHIVE_IMAGE))?;
        let contract = out.with_extension("update-validation.json");
        save_json(
            &contract,
            &json!({"schema":action.schema(),
            "artifact_id":update_validation::ARTIFACT,"object":object,
            "parent":parent,
            "fault":(action == update_validation::Action::RejectTampered).then(||json!({"archive_offset":update_validation::FAULT_OFFSET,"xor_mask":1})),
            "release":"22.02.4-plus","mode":"sd","platform":"am1-s9",
            "client_operation_seconds":update_validation::CLIENT_SECONDS,
            "pool_seconds":update_validation::POOL_SECONDS,
            "aggregate_trace_bytes":update_validation::TRACE_BYTES,
            "maintenance":action.maintenance(),
            "restart_observation_seconds":if action == update_validation::Action::InstallKeep {Some(update_validation::RESTART_SECONDS)} else {None},
            "release_index":hash_file(Path::new("catalog/parts/bitmain/braiins-release-index.jsonl"))?,
            "operation":action.operation()}),
        )?;
        configuration["files"][update_validation::GUEST_IMAGE] =
            json!({"path":path.canonicalize()?,"mode":420});
        configuration["files"]["/dashboard-update.json"] =
            json!({"path":contract.canonicalize()?,"mode":420});
    }
    if let Some(path) = state {
        let mut bytes = Vec::new();
        File::open(path)?.take(4097).read_to_end(&mut bytes)?;
        dashboard_state::parse(&bytes)?;
        let captured = out.with_extension("dashboard-state.json");
        std::fs::write(&captured, bytes)?;
        configuration["files"]["/dashboard-state.json"] =
            json!({"path":captured.canonicalize()?,"mode":420});
    }
    if let Some(payload) = browser_payload {
        ensure!(dashboard, "browser needs the original dashboard contract");
        configuration["programs"]["/boser-browser"] = json!(directory.join("s9_browser_check"));
        configuration["payload_manifest"] = json!(payload.canonicalize()?);
    }
    if let Some(epoch) = clock_epoch {
        wall_clock::validate(epoch)?;
        let contract = out.with_extension("clock-policy.json");
        save_json(
            &contract,
            &json!({"schema":"md-sim-private-wall-clock/1", "epoch_unix_seconds":epoch}),
        )?;
        configuration["files"]["/clock-policy.json"] =
            json!({"path":contract.canonicalize()?,"mode":420});
    }
    save_json(&scenario, &configuration)?;
    let began = Instant::now();
    let status = Command::new(directory.join("isolated_network_vm"))
        .arg(out)
        .arg(scenario)
        .arg("--prepare-only")
        .stdin(Stdio::null())
        .stdout(File::create(out.with_extension("prepare.stdout"))?)
        .stderr(File::create(out.with_extension("prepare.stderr"))?)
        .status()?;
    save_json(
        &out.with_extension("prepare-status.json"),
        &json!({"status":status.to_string(),"exit_code":status.code(),
            "signal":status.signal(),"elapsed_milliseconds":began.elapsed().as_millis(),
            "stdout":hash_file(&out.with_extension("prepare.stdout"))?,
            "stderr":hash_file(&out.with_extension("prepare.stderr"))?}),
    )?;
    ensure!(
        status.success(),
        "pool VM image preparation failed: {status}"
    );
    Ok(())
}

pub fn capture(
    store: &Store,
    inputs: &mut Vec<Mount>,
    prepared: &Path,
    backend: &Path,
    browser: bool,
) -> Result<()> {
    let report: Value = serde_json::from_reader(File::open(prepared.join("prepared-image.json"))?)?;
    ensure!(
        report["schema"] == "md-sim-private-vm-preparation/1",
        "unknown prepared pool image"
    );
    let image = prepared.join(if browser {
        "combined-initramfs.cpio.gz"
    } else {
        "initramfs.cpio.gz"
    });
    ensure!(
        report["image"] == serde_json::to_value(hash_file(&image)?)?,
        "prepared pool image changed"
    );
    ensure!(
        report["root_manifest"]
            == serde_json::to_value(hash_file(&prepared.join("root-manifest.json"))?)?,
        "prepared pool input manifest changed"
    );
    ensure!(
        hash_file(Path::new(KERNEL))?.sha256 == KERNEL_HASH,
        "pool kernel changed"
    );
    let build: Value = serde_json::from_reader(File::open(backend.join("build-report.json"))?)?;
    ensure!(
        build["target"] == "x86_64-private-pool-support"
            && build["private_stream"]["contract"] == "md-sim-private-inherited-ethernet/1"
            && build["pool_machine_smoke"]["code"] == 0
            && build["undefined_behavior_sanitizer"] == true,
        "pool backend lacks required private stream build/smoke"
    );
    let binary = backend.join("qemu-system-x86_64");
    ensure!(
        build["binary"] == serde_json::to_value(hash_file(&binary)?)?,
        "pool backend identity differs"
    );
    for entry in md_sim_firmware::lab::elf_closure(store, vec![("/pool-qemu".into(), binary)])? {
        if let Some(old) = inputs.iter().find(|i| i.guest_path == entry.guest_path) {
            ensure!(
                old.object == entry.object,
                "conflicting pool runtime dependency"
            );
        } else {
            inputs.push(entry);
        }
    }
    let files = [
        (PathBuf::from(KERNEL), "/pool-kernel".to_owned()),
        (image, "/pool-initramfs".to_owned()),
        (
            prepared.join("prepared-image.json"),
            "/pool-image-identity.json".to_owned(),
        ),
        (
            prepared.join("root-manifest.json"),
            "/pool-root-manifest.json".to_owned(),
        ),
        (
            backend.join("build-report.json"),
            "/pool-backend.json".to_owned(),
        ),
    ]
    .into_iter()
    .chain(
        ["bios-256k.bin", "linuxboot_dma.bin", "kvmvapic.bin"]
            .into_iter()
            .map(|name| {
                (
                    Path::new("/usr/share/qemu").join(name),
                    format!("/pool-qemu-data/{name}"),
                )
            }),
    );
    for (path, guest_path) in files {
        ensure!(
            !inputs.iter().any(|i| i.guest_path == guest_path),
            "duplicate pool input"
        );
        let object = hash_file(&path)?;
        store.import(&path, &object)?;
        inputs.push(Mount { guest_path, object });
    }
    Ok(())
}

/// The fd is private to this child. The opposite socket remains close-on-exec.
pub fn attach(command: &mut Command, endpoint: &UnixStream) {
    let fd = endpoint.as_raw_fd();
    let parent = std::process::id() as libc::pid_t;
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                || libc::getppid() != parent
                || libc::dup2(fd, 3) < 0
                || libc::fcntl(3, libc::F_SETFD, 0) < 0
            {
                return Err(std::io::Error::last_os_error());
            }
            if fd != 3 {
                libc::close(fd);
            }
            Ok(())
        });
    }
}

pub struct Machine {
    pub child: Child,
    pub stdout: PathBuf,
    pub stderr: PathBuf,
    pub packets: PathBuf,
    packet_limit: usize,
}
impl Drop for Machine {
    fn drop(&mut self) {
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
pub fn start(endpoint: UnixStream, client: bool) -> Result<Machine> {
    start_with_browser(endpoint, client, false, None)
}
fn start_with_browser(
    endpoint: UnixStream,
    client: bool,
    browser: bool,
    rtc: Option<&str>,
) -> Result<Machine> {
    ensure!(
        !client || !browser,
        "transport client cannot select a browser"
    );
    let role = if client { "client" } else { "server" };
    let stdout = PathBuf::from(format!("/work/pool-{role}.log"));
    let stderr = PathBuf::from(format!("/work/pool-{role}.stderr"));
    let packets = PathBuf::from(format!("/work/pool-{role}.pcap"));
    let mut args = vec![
        "-machine",
        "pc",
        "-accel",
        if browser {
            "tcg,thread=multi,tb-size=256"
        } else {
            "tcg,thread=single"
        },
        "-cpu",
        "max",
        "-m",
        if browser { "2048M" } else { "256M" },
        "-smp",
        if browser { "4" } else { "1" },
        "-nodefaults",
        "-no-user-config",
        "-display",
        "none",
        "-monitor",
        "none",
        "-serial",
        "stdio",
        "-no-reboot",
        "-L",
        "/pool-qemu-data",
        "-bios",
        "/pool-qemu-data/bios-256k.bin",
        "-kernel",
        "/pool-kernel",
        "-initrd",
        "/pool-initramfs",
        "-netdev",
        "socket,id=private,fd=3,x-md-sim-private-stream=on",
        "-device",
        if client {
            "e1000,netdev=private,mac=52:54:00:12:34:15,romfile="
        } else {
            "e1000,netdev=private,mac=52:54:00:12:34:64,romfile="
        },
        "-append",
        if client {
            "console=ttyS0 rdinit=/init panic=-1 md_sim_private_guest=1 md_sim_pool_role=client"
        } else if browser {
            "console=ttyS0 rdinit=/init panic=-1 rootfstype=ramfs md_sim_private_guest=1 md_sim_pool_role=server"
        } else {
            "console=ttyS0 rdinit=/init panic=-1 md_sim_private_guest=1 md_sim_pool_role=server"
        },
    ];
    if let Some(rtc) = rtc {
        args.extend(["-rtc", rtc]);
    }
    println!(
        "MD_SIM_POOL_VM_COMMAND {}",
        json!({"role":role,"argv":args,"network":"inherited Unix socket pair; two QEMU Ethernet endpoints; no host listener"})
    );
    let mut command = Command::new("/pool-qemu");
    command
        .args(&args)
        .args([
            "-object",
            &format!(
                "filter-dump,id=capture,netdev=private,file={}",
                packets.display()
            ),
        ])
        .stdin(Stdio::null())
        .stdout(File::create(&stdout)?)
        .stderr(File::create(&stderr)?);
    attach(&mut command, &endpoint);
    let child = command.spawn()?;
    Ok(Machine {
        child,
        stdout,
        stderr,
        packets,
        packet_limit: if update_validation::selected_environment()? {
            ensure!(
                !browser && !client,
                "update packet allowance needs its selected server suite"
            );
            update_validation::PACKET_BYTES
        } else if browser {
            browser_budget::PACKET_BYTES
        } else {
            1024 * 1024
        },
    })
}

pub fn read_bounded(path: &Path, maximum: usize) -> Result<String> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= maximum, "pool VM output exceeded bound");
    Ok(String::from_utf8(bytes)?)
}
pub fn markers(text: &str, marker: &str) -> Result<Vec<Value>> {
    text.lines()
        .filter_map(|line| line.trim_end_matches('\r').strip_prefix(marker))
        .map(|s| Ok(serde_json::from_str(s)?))
        .collect()
}

pub fn require_guest_success(log: &str, stderr: &str) -> Result<()> {
    ensure!(stderr.is_empty(), "pool VM backend emitted an error");
    for marker in [
        "MD_SIM_VM_SCENARIO selected guest command exited 0",
        "MD_SIM_VM_COMPLETE isolated guest loopback probe passed",
    ] {
        ensure!(
            log.lines()
                .filter(|line| line.trim_end_matches('\r') == marker)
                .count()
                == 1,
            "pool VM did not complete its real guest scenario"
        );
    }
    Ok(())
}
pub fn export(machine: &Machine) -> Result<Value> {
    let packets = packet_bytes(machine)?;
    Ok(json!({
        "stdout":read_bounded(&machine.stdout,8*1024*1024)?,
        "stderr":read_bounded(&machine.stderr,65536)?,
        "stdout_identity":hash_file(&machine.stdout)?,"stderr_identity":hash_file(&machine.stderr)?,
        "pcap_hex":hex::encode(packets),"pcap_identity":hash_file(&machine.packets)?
    }))
}

fn packet_bytes(machine: &Machine) -> Result<Vec<u8>> {
    let mut packets = Vec::new();
    File::open(&machine.packets)?
        .take(machine.packet_limit as u64 + 1)
        .read_to_end(&mut packets)?;
    ensure!(
        packets.len() <= machine.packet_limit,
        "pool packet evidence bound"
    );
    Ok(packets)
}

/// Retain actual captured Ethernet bytes after the VM has stopped. A failed
/// run without a completion record has no complete packet capture to export.
pub fn retain_miner_packets(out: &Path, text: &str, browser: bool, update: bool) -> Result<()> {
    let records = markers(text, "MD_SIM_POOL_VM_FINISHED ")?;
    ensure!(records.len() <= 1, "ambiguous pool VM completion");
    if let Some(record) = records.first() {
        let encoded = record["pcap_hex"]
            .as_str()
            .context("pool packet bytes absent")?;
        let limit = if update {
            update_validation::PACKET_BYTES
        } else if browser {
            browser_budget::PACKET_BYTES
        } else {
            1024 * 1024
        };
        ensure!(encoded.len() <= 2 * limit, "pool packet evidence bound");
        let packets = hex::decode(encoded)?;
        let expected: md_sim_runtime::store::ObjectId =
            serde_json::from_value(record["packets"].clone())?;
        ensure!(
            expected.size == packets.len() as u64
                && expected.sha256 == md_sim_firmware::prepare::hash_bytes(&packets),
            "pool packet identity differs"
        );
        use std::io::Write;
        File::create_new(out.join("pool.pcap"))?.write_all(&packets)?;
    }
    Ok(())
}

/// Retain pool output while the miner runs, including when its outer budget
/// ends before the console script returns.
pub struct MinerPool {
    machine: Machine,
    stop: Arc<AtomicBool>,
    pump: Option<JoinHandle<Result<()>>>,
    ready_at: Instant,
}
fn pump_output(path: PathBuf, stop: Arc<AtomicBool>) -> Result<()> {
    let mut file = File::open(path)?;
    let mut pending = Vec::new();
    let mut total = 0usize;
    loop {
        let mut bytes = [0u8; 8192];
        let count = file.read(&mut bytes)?;
        total += count;
        ensure!(total <= 8 * 1024 * 1024, "pool serial output budget");
        pending.extend_from_slice(&bytes[..count]);
        while let Some(end) = pending.iter().position(|b| *b == b'\n') {
            let bytes: Vec<_> = pending.drain(..=end).collect();
            let line = std::str::from_utf8(&bytes)?.trim_end_matches(['\r', '\n']);
            if line.starts_with("MD_SIM_PRIVATE_POOL_RESULT ")
                || line.starts_with("MD_SIM_PRIVATE_POOL_COLLECTION ")
                || line.starts_with("MD_SIM_POOL_READY ")
                || line.starts_with("MD_SIM_DASHBOARD_PROBE ")
                || line.starts_with("MD_SIM_BOSER_BROWSER_")
            {
                println!("\n{line}");
            } else {
                println!("\nMD_SIM_POOL_VM_LOG {}", json!({"line":line}));
            }
        }
        ensure!(pending.len() <= 2 * 1024 * 1024, "pool serial line budget");
        if count == 0 {
            if stop.load(Ordering::Acquire) {
                ensure!(pending.is_empty(), "pool output ended with a partial line");
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
impl MinerPool {
    /// Preserve the running miner until its independent pool has retained all
    /// sessions. A short console script must not terminate mining evidence.
    /// This does not extend the existing pool or outer execution budgets.
    pub fn wait_mining_window(&mut self) -> Result<()> {
        let input = pool_input::read(Path::new("/private-pool.json"))?;
        let seconds = input.seconds;
        let session_limit = input.session_limit()?;
        ensure!((20..=300).contains(&seconds), "pool window bound");
        let deadline = self.ready_at + Duration::from_secs(seconds + 5);
        loop {
            let log = read_bounded(&self.machine.stdout, 8 * 1024 * 1024)?;
            let complete = log.rsplit_once('\n').map(|(text, _)| text).unwrap_or("");
            if mining_window_completed(complete, session_limit)? {
                return Ok(());
            }
            ensure!(
                self.machine.child.try_wait()?.is_none(),
                "support VM exited before mining collection"
            );
            ensure!(Instant::now() < deadline, "mining collection deadline");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    /// Finish native HTTP writes and logout before a requested storage checkpoint.
    pub fn wait_dashboard(&mut self) -> Result<()> {
        let deadline = self.ready_at + Duration::from_secs(230);
        loop {
            let log = read_bounded(&self.machine.stdout, 8 * 1024 * 1024)?;
            let complete = log.rsplit_once('\n').map(|(text, _)| text).unwrap_or("");
            if dashboard_completed(complete)? {
                return Ok(());
            }
            ensure!(
                self.machine.child.try_wait()?.is_none(),
                "support VM exited before native HTTP completion"
            );
            ensure!(Instant::now() < deadline, "native HTTP completion deadline");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    /// An upload can outlast the console script. Verify the selected native
    /// operation. Install mode still requires a separately observed reboot.
    pub fn wait_update(&mut self, action: update_validation::Action) -> Result<()> {
        let deadline = self.ready_at + Duration::from_secs(update_validation::POOL_SECONDS);
        loop {
            let log = read_bounded(&self.machine.stdout, 8 * 1024 * 1024)?;
            let complete = log.rsplit_once('\n').map(|(text, _)| text).unwrap_or("");
            if update_completed(complete, action)? {
                return Ok(());
            }
            ensure!(
                self.machine.child.try_wait()?.is_none(),
                "support VM exited before native upload validation"
            );
            ensure!(
                Instant::now() < deadline,
                "native upload validation deadline"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub fn start(browser: bool, rtc: Option<&str>) -> Result<(Self, UnixStream)> {
        let (server, miner) = UnixStream::pair()?;
        let machine = start_with_browser(server, false, browser, rtc)?;
        let stop = Arc::new(AtomicBool::new(false));
        let path = machine.stdout.clone();
        let signal = stop.clone();
        let pump = std::thread::spawn(move || pump_output(path, signal));
        let mut pool = Self {
            machine,
            stop,
            pump: Some(pump),
            ready_at: Instant::now(),
        };
        let seconds = startup_seconds(browser, std::env::var(STARTUP_ENVIRONMENT).ok().as_deref())?;
        let began = Instant::now();
        let deadline = began + Duration::from_secs(seconds);
        println!(
            "MD_SIM_POOL_VM_STARTUP_POLICY {}",
            json!({"deadline_seconds":seconds,"scope":"Support VM readiness only. Firmware clocks, hardware timing and mining assertions are unchanged."})
        );
        let ready = (|| -> Result<()> {
            loop {
                let log = read_bounded(&pool.machine.stdout, 1024 * 1024)?;
                let complete = log
                    .rsplit_once('\n')
                    .map(|(complete, _)| complete)
                    .unwrap_or("");
                if !markers(complete, "MD_SIM_POOL_READY ")?.is_empty() {
                    break;
                }
                ensure!(
                    pool.machine.child.try_wait()?.is_none(),
                    "pool VM exited before ready: {}",
                    read_bounded(&pool.machine.stderr, 65536)?
                );
                ensure!(Instant::now() < deadline, "pool VM readiness deadline");
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(())
        })();
        if let Err(error) = ready {
            // Stop only this owned support VM before reading its final logs.
            // Failed startup must retain the real kernel/backend diagnostics.
            if !matches!(pool.machine.child.try_wait(), Ok(Some(_))) {
                let _ = pool.machine.child.kill();
                let _ = pool.machine.child.wait();
            }
            let log = read_bounded(&pool.machine.stdout, 1024 * 1024);
            let stderr = read_bounded(&pool.machine.stderr, 65536);
            println!(
                "MD_SIM_POOL_VM_STARTUP_FAILED {}",
                json!({"error":format!("{error:#}"),"elapsed_ms":began.elapsed().as_millis(),"deadline_seconds":seconds,"stdout":log.as_ref().ok(),"stderr":stderr.as_ref().ok(),"stdout_error":log.as_ref().err().map(|e|format!("{e:#}")),"stderr_error":stderr.as_ref().err().map(|e|format!("{e:#}")),"firmware_started":false})
            );
            return Err(error);
        }
        println!(
            "MD_SIM_POOL_VM_STARTUP_COMPLETE {}",
            json!({"elapsed_ms":began.elapsed().as_millis(),"deadline_seconds":seconds,"scope":"Actual support guest ready marker observed; no firmware boot claim."})
        );
        pool.ready_at = Instant::now();
        Ok((pool, miner))
    }
    /// Keep the selected firmware process alive until its browser finishes.
    /// This wait stays inside the unchanged outer miner CPU and wall bounds.
    pub fn wait_browser(&mut self) -> Result<()> {
        let deadline = self.ready_at + Duration::from_secs(browser_budget::COMPLETION_SECONDS);
        loop {
            let log = read_bounded(&self.machine.stdout, 8 * 1024 * 1024)?;
            let complete = log.rsplit_once('\n').map(|(text, _)| text).unwrap_or("");
            if browser_completed(complete)? {
                return Ok(());
            }
            ensure!(
                self.machine.child.try_wait()?.is_none(),
                "support VM exited before browser completion"
            );
            ensure!(
                Instant::now() < deadline,
                "native browser completion deadline"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    pub fn finish(mut self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(240);
        let status = loop {
            if let Some(status) = self.machine.child.try_wait()? {
                break status;
            }
            ensure!(Instant::now() < deadline, "pool VM completion deadline");
            std::thread::sleep(Duration::from_millis(20));
        };
        self.stop.store(true, Ordering::Release);
        self.pump
            .take()
            .context("pool pump missing")?
            .join()
            .map_err(|_| anyhow::anyhow!("pool output thread panicked"))??;
        println!(
            "\nMD_SIM_POOL_VM_FINISHED {}",
            json!({
                "status":status.to_string(),"stdout":hash_file(&self.machine.stdout)?,
                "stderr":read_bounded(&self.machine.stderr,65536)?,"packets":hash_file(&self.machine.packets)?,
                "pcap_hex":hex::encode(packet_bytes(&self.machine)?)
            })
        );
        ensure!(status.success(), "pool VM failed");
        require_guest_success(
            &read_bounded(&self.machine.stdout, 8 * 1024 * 1024)?,
            &read_bounded(&self.machine.stderr, 65536)?,
        )?;
        Ok(())
    }
}

fn mining_window_completed(log: &str, session_limit: usize) -> Result<bool> {
    let collections = markers(log, "MD_SIM_PRIVATE_POOL_COLLECTION ")?;
    ensure!(collections.len() <= 1, "ambiguous mining collection");
    let Some(collection) = collections.first() else {
        return Ok(false);
    };
    let sessions = markers(log, mining_report::MARKER)?;
    mining_report::require_completed(&sessions, collection, session_limit)?;
    Ok(true)
}

fn update_completed(log: &str, action: update_validation::Action) -> Result<bool> {
    let records = markers(log, "MD_SIM_DASHBOARD_PROBE ")?;
    if records.is_empty() {
        return Ok(false);
    }
    ensure!(records.len() == 1, "duplicate native HTTP completion");
    update_validation::verify_completion(&records[0], action)?;
    Ok(true)
}

fn dashboard_completed(log: &str) -> Result<bool> {
    let records = markers(log, "MD_SIM_DASHBOARD_PROBE ")?;
    ensure!(records.len() <= 1, "ambiguous native HTTP completion");
    let Some(record) = records.first() else {
        return Ok(false);
    };
    ensure!(
        record["schema"] == "md-sim-original-s9-dashboard-probe/1"
            && record["origin"] == "http://10.0.2.15"
            && record["passed"] == true
            && record.get("error") == Some(&Value::Null)
            && record["result"].is_object(),
        "native HTTP checks did not finish successfully before storage checkpoint"
    );
    Ok(true)
}

fn browser_completed(log: &str) -> Result<bool> {
    ensure!(
        markers(log, "MD_SIM_BOSER_BROWSER_FAILURE ")?.is_empty(),
        "native browser reported failure"
    );
    let records = markers(log, "MD_SIM_BOSER_BROWSER_RESULT ")?;
    ensure!(records.len() <= 1, "ambiguous native browser completion");
    let Some(record) = records.first() else {
        return Ok(false);
    };
    ensure!(
        record["origin"] == "http://10.0.2.15" && record["passed"] == true,
        "native browser did not pass for selected origin: {}",
        record["error"]
    );
    Ok(true)
}
impl Drop for MinerPool {
    fn drop(&mut self) {
        if !matches!(self.machine.child.try_wait(), Ok(Some(_))) {
            let _ = self.machine.child.kill();
            let _ = self.machine.child.wait();
        }
        self.stop.store(true, Ordering::Release);
        if let Some(pump) = self.pump.take() {
            if let Ok(Err(error)) = pump.join() {
                eprintln!("pool VM output retention failed: {error:#}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn storage_wait_keeps_all_sessions_and_rejects_a_later_success_after_failure() {
        use serde_json::json;
        let trace = concat!(
            "{\"direction\":\"firmware-to-pool\",\"message\":{\"method\":\"mining.authorize\",\"params\":[\"simulator\",\"x\"]}}\n",
            "{\"direction\":\"pool-to-firmware\",\"message\":{\"method\":\"mining.notify\"}}\n"
        );
        let session = |id, passed| {
            format!(
                "MD_SIM_PRIVATE_POOL_RESULT {}\n",
                json!({"session_id":id,"trace":trace,"report":{"completed":passed,
                    "stopped":false,"timed_out":false,"budget_exhausted":false,"failure":null}})
            )
        };
        let collection = |count| {
            format!(
                "MD_SIM_PRIVATE_POOL_COLLECTION {}\n",
                json!({"sessions_completed":count,"incomplete_sessions":[],"max_sessions":16})
            )
        };
        assert!(!super::mining_window_completed(&session(0, true), 16).unwrap());
        assert!(super::mining_window_completed(
            &format!("{}{}", session(0, true), collection(1)),
            16
        )
        .unwrap());
        assert!(super::mining_window_completed(
            &format!("{}{}{}", session(1, true), session(0, false), collection(2)),
            16
        )
        .is_err());
        assert!(super::mining_window_completed(&collection(0), 16).is_err());
        assert!(super::mining_window_completed(
            &format!("{}{}", session(0, true), collection(2)),
            16
        )
        .is_err());
        assert!(super::mining_window_completed(
            &format!("{}{}{}", session(0, true), collection(1), collection(1)),
            16
        )
        .is_err());
    }
    #[test]
    fn support_startup_budget_is_explicit_bounded_and_separate_from_browser() {
        assert_eq!(super::startup_seconds(false, None).unwrap(), 20);
        assert_eq!(super::startup_seconds(false, Some("120")).unwrap(), 120);
        for invalid in ["0", "19", "121", "-1", "x"] {
            assert!(super::startup_seconds(false, Some(invalid)).is_err());
        }
        assert!(super::startup_seconds(true, Some("120")).is_err());
        assert_eq!(
            super::startup_seconds(true, None).unwrap(),
            super::browser_budget::STARTUP_SECONDS
        );
    }
    use super::*;
    #[test]
    fn storage_checkpoint_wait_requires_this_native_http_result() {
        assert!(!dashboard_completed("").unwrap());
        let valid = json!({"schema":"md-sim-original-s9-dashboard-probe/1", "origin":"http://10.0.2.15", "passed":true,"error":null,"result":{}});
        let record = |value: &Value| format!("MD_SIM_DASHBOARD_PROBE {value}\n");
        assert!(dashboard_completed(&record(&valid)).unwrap());
        assert!(dashboard_completed(&format!("{}{}", record(&valid), record(&valid))).is_err());
        for (name, bad) in [
            ("schema", json!("other")),
            ("origin", json!("http://10.0.2.16")),
            ("passed", json!(false)),
            ("error", json!("failed")),
            ("result", Value::Null),
        ] {
            let mut changed = valid.clone();
            changed[name] = bad;
            assert!(dashboard_completed(&record(&changed)).is_err());
        }
    }

    #[test]
    fn update_wait_requires_native_validation_and_cancellation() {
        let check = |log: &str| update_completed(log, update_validation::Action::ValidateCancel);
        assert!(!check("MD_SIM_POOL_READY {}\n").unwrap());
        let record = json!({"origin":"http://10.0.2.15","passed":true,
            "result":{"luci":{"valid_update":{"cancelled":true,"commit_requested":false,
            "contract":{"schema":update_validation::SCHEMA,"maintenance":update_validation::MAINTENANCE},
            "maintenance":{"stop":{"port":{"connection_refused":true}},"start":{"port":{"listening":true}}}}}}});
        let line = format!("MD_SIM_DASHBOARD_PROBE {record}\n");
        assert!(check(&line).unwrap());
        assert!(check(&format!("{line}{line}")).is_err());
        for (path, changed) in [
            ("/passed", json!(false)),
            ("/origin", json!("http://10.0.2.16")),
            ("/result/luci/valid_update/cancelled", json!(false)),
            ("/result/luci/valid_update/commit_requested", json!(true)),
            (
                "/result/luci/valid_update/contract/schema",
                json!("md-sim-s9-sd-update-validation/2"),
            ),
            (
                "/result/luci/valid_update/maintenance/stop/port/connection_refused",
                json!(false),
            ),
            ("/result/luci/valid_update/maintenance/start", Value::Null),
        ] {
            let mut bad = record.clone();
            *bad.pointer_mut(path).unwrap() = changed;
            assert!(check(&format!("MD_SIM_DASHBOARD_PROBE {bad}\n")).is_err());
        }
    }

    #[test]
    fn damaged_update_requires_its_exact_input_and_native_rejection() {
        let action = update_validation::Action::RejectTampered;
        let record = json!({"origin":"http://10.0.2.15","passed":true,
            "result":{"luci":{"rejected_update":{"rejected":true,"commit_requested":false,
            "install_controls_present":false,"native_rejection":{"size":123},
            "contract":{"schema":action.schema(),"maintenance":action.maintenance(),
                "object":{"sha256":update_validation::FAULT_SHA256,"size":update_validation::BYTES},
                "parent":{"sha256":update_validation::SHA256,"size":update_validation::BYTES},
                "fault":{"archive_offset":update_validation::FAULT_OFFSET,"xor_mask":1}},
            "maintenance":{"stop":{"port":{"connection_refused":true}},"start":{"port":{"listening":true}}}}}}});
        let check =
            |value: &Value| update_completed(&format!("MD_SIM_DASHBOARD_PROBE {value}\n"), action);
        assert!(check(&record).unwrap());
        for (path, bad) in [
            ("/passed", json!(false)),
            ("/result/luci/rejected_update/rejected", json!(false)),
            ("/result/luci/rejected_update/commit_requested", json!(true)),
            (
                "/result/luci/rejected_update/install_controls_present",
                json!(true),
            ),
            (
                "/result/luci/rejected_update/native_rejection/size",
                json!(0),
            ),
            (
                "/result/luci/rejected_update/contract/object/sha256",
                json!(update_validation::SHA256),
            ),
            (
                "/result/luci/rejected_update/contract/parent/sha256",
                json!(update_validation::FAULT_SHA256),
            ),
            (
                "/result/luci/rejected_update/contract/fault/xor_mask",
                json!(2),
            ),
            (
                "/result/luci/rejected_update/maintenance/start/port/listening",
                json!(false),
            ),
        ] {
            let mut changed = record.clone();
            *changed.pointer_mut(path).unwrap() = bad;
            assert!(check(&changed).is_err());
        }
        assert!(update_completed(
            &format!("MD_SIM_DASHBOARD_PROBE {record}\n"),
            update_validation::Action::ValidateCancel
        )
        .is_err());
    }

    #[test]
    fn install_wait_cannot_accept_cancel_or_incomplete_commit_evidence() {
        let action = update_validation::Action::InstallKeep;
        let record = json!({"origin":"http://10.0.2.15","passed":true,
            "result":{"luci":{"valid_update":{"cancelled":false,"commit_requested":true,
            "keep_settings":true,"step":2,"reported_sha256":update_validation::SHA256,
            "native_commit_response":{"size":123},
            "contract":{"schema":action.schema(),"maintenance":action.maintenance(),
                "restart_observation_seconds":update_validation::RESTART_SECONDS},
            "maintenance":{"stop":{"port":{"connection_refused":true}}}}}}});
        let line = format!("MD_SIM_DASHBOARD_PROBE {record}\n");
        assert!(update_completed(&line, action).unwrap());
        assert!(update_completed(&line, update_validation::Action::ValidateCancel).is_err());
        for (path, value) in [
            ("/passed", json!(false)),
            ("/result/luci/valid_update/commit_requested", json!(false)),
            ("/result/luci/valid_update/cancelled", json!(true)),
            ("/result/luci/valid_update/keep_settings", json!(false)),
            ("/result/luci/valid_update/step", json!(1)),
            (
                "/result/luci/valid_update/reported_sha256",
                json!("00".repeat(32)),
            ),
            (
                "/result/luci/valid_update/native_commit_response/size",
                json!(0),
            ),
            (
                "/result/luci/valid_update/contract/schema",
                json!(update_validation::SCHEMA),
            ),
            (
                "/result/luci/valid_update/contract/restart_observation_seconds",
                json!(121),
            ),
        ] {
            let mut changed = record.clone();
            *changed.pointer_mut(path).unwrap() = value;
            assert!(
                update_completed(&format!("MD_SIM_DASHBOARD_PROBE {changed}\n"), action).is_err()
            );
        }
    }

    #[test]
    fn browser_wait_requires_its_selected_service_result() {
        assert!(!browser_completed(
            "MD_SIM_POOL_READY {}\nMD_SIM_VM_COMPLETE isolated guest loopback probe passed\n"
        )
        .unwrap());
        let pass =
            "MD_SIM_BOSER_BROWSER_RESULT {\"origin\":\"http://10.0.2.15\",\"passed\":true}\n";
        assert!(browser_completed(pass).unwrap());
        assert!(browser_completed(&format!("{pass}{pass}")).is_err());
        assert!(browser_completed(&pass.replace("true", "false")).is_err());
        assert!(browser_completed(&pass.replace("10.0.2.15", "10.0.2.16")).is_err());
        assert!(browser_completed(&format!("{pass}MD_SIM_BOSER_BROWSER_FAILURE {{}}\n")).is_err());
    }

    #[test]
    fn qemu_shutdown_alone_does_not_establish_guest_success() {
        let command = "MD_SIM_VM_SCENARIO selected guest command exited 0\n";
        let complete = "MD_SIM_VM_COMPLETE isolated guest loopback probe passed\n";
        require_guest_success(&format!("{command}{complete}"), "").unwrap();
        assert!(require_guest_success(
            "Error: guest scenario failed: exit status: 1\nreboot: Power down\n",
            ""
        )
        .is_err());
        assert!(require_guest_success(command, "").is_err());
        assert!(require_guest_success(complete, "").is_err());
        assert!(require_guest_success(&format!("{command}{command}{complete}"), "").is_err());
        assert!(require_guest_success(&format!("{command}{complete}"), "backend failed").is_err());
    }
}

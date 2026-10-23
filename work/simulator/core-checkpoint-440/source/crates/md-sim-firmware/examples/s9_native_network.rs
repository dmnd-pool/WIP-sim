//! Nested ARM firmware service inside the captured, disconnected x86 test VM.
//! This is an adapted native service diagnostic, not an original-board boot.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{isolation::save_json, store::hash_file};
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::process::{CommandExt, ExitStatusExt},
    },
    path::Path,
    process::{Child, ChildStdin, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
static STOP: AtomicBool = AtomicBool::new(false);
#[allow(dead_code)] // This executable uses only the native-service CPU budget.
#[path = "support/native_browser_budget.rs"]
mod native_browser_budget;
#[path = "support/s9_sd_image.rs"]
mod sd;
extern "C" fn stop(_: libc::c_int) {
    STOP.store(true, Ordering::Release);
}
const SCRIPT: &[u8] = include_bytes!("../../../simulator/firmware/s9-native-network-start.sh");
const DISK: &str = "/worker/native/persistent.img";
const INPUTS: &[(&str, &str)] = &[
    (
        "kernel",
        "f85eba975b58b4333e99a656ad661667cbedaf3111af3deb8572b902b88cd276",
    ),
    (
        "ramdisk",
        "2bd49c70b915b7e25f6c01013d95a5ffea8d499b73c6c6ec5539fda4247154b3",
    ),
    (
        "board.dtb",
        "550c87818cb0db41c00a8351dc4ac7b5dce5bf6d5336539c6951163ebf3cb62a",
    ),
];
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
fn exit_evidence(status: Option<ExitStatus>) -> Result<serde_json::Value> {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    ensure!(
        unsafe { libc::getrusage(libc::RUSAGE_CHILDREN, &mut usage) } == 0,
        "cannot read native child resource usage"
    );
    let evidence = json!({"exit_code":status.and_then(|s| s.code()),"signal":status.and_then(|s| s.signal()),
        "observed_exit":status.is_some(),"reaped_children_user_us":usage.ru_utime.tv_sec * 1_000_000 + usage.ru_utime.tv_usec,
        "reaped_children_system_us":usage.ru_stime.tv_sec * 1_000_000 + usage.ru_stime.tv_usec});
    save_json(Path::new("/worker/native/exit.json"), &evidence)?;
    Ok(evidence)
}
fn startup_observation(pid: u32, elapsed: Duration, ready: bool) -> serde_json::Value {
    let mut records = serde_json::Map::new();
    for path in [
        "/proc/meminfo".to_string(),
        "/proc/pressure/memory".to_string(),
        format!("/proc/{pid}/stat"),
        format!("/proc/{pid}/status"),
        format!("/proc/{pid}/wchan"),
        format!("/proc/{pid}/schedstat"),
    ] {
        let result = (|| -> std::io::Result<Vec<u8>> {
            let mut bytes = Vec::new();
            fs::File::open(&path)?.take(8193).read_to_end(&mut bytes)?;
            Ok(bytes)
        })();
        records.insert(path, match result {
            Ok(bytes) => json!({"hex":hex::encode(&bytes[..bytes.len().min(8192)]),"truncated":bytes.len()>8192}),
            Err(error) => json!({"error":error.to_string()}),
        });
    }
    json!({"elapsed_ms":elapsed.as_millis(),"native_qemu_pid":pid,"actual_readiness_marker":ready,"observations":records})
}
fn confinement() -> Result<serde_json::Value> {
    ensure!(
        fs::read_to_string("/proc/cmdline")?
            .split_whitespace()
            .any(|w| w == "md_sim_private_guest=1"),
        "requires captured private VM kernel"
    );
    ensure!(
        unsafe { libc::geteuid() } == 1000,
        "native VM diagnostic requires dedicated UID1000"
    );
    let status = fs::read_to_string("/proc/self/status")?;
    ensure!(
        status.lines().any(|l| l == "NoNewPrivs:\t1")
            && status.lines().any(|l| l == "CapEff:\t0000000000000000"),
        "native VM diagnostic requires no capabilities and no new privileges"
    );
    for line in fs::read_to_string("/proc/net/dev")?.lines().skip(2) {
        ensure!(
            matches!(
                line.split(':').next().map(str::trim),
                Some("lo" | "worker0")
            ),
            "unexpected worker network interface"
        );
    }
    for line in fs::read_to_string("/proc/net/route")?.lines().skip(1) {
        ensure!(
            line.split_whitespace().nth(1) != Some("00000000"),
            "worker has an external default route"
        );
    }
    let mut limits = serde_json::Map::new();
    let cpu_seconds = if std::env::var("MD_SIM_NATIVE_BROWSER").as_deref() == Ok("1") {
        native_browser_budget::SERVICE_CPU_SECONDS
    } else {
        420
    };
    for (name, resource, max) in [
        ("cpu_seconds", libc::RLIMIT_CPU, cpu_seconds),
        ("address_bytes", libc::RLIMIT_AS, 3 * 1024 * 1024 * 1024),
        ("file_bytes", libc::RLIMIT_FSIZE, 64 * 1024 * 1024),
        ("processes", libc::RLIMIT_NPROC, 128),
    ] {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        ensure!(
            unsafe { libc::getrlimit(resource, &mut limit) } == 0 && limit.rlim_max <= max,
            "native diagnostic resource bound missing"
        );
        limits.insert(
            name.into(),
            json!({"soft":limit.rlim_cur,"hard":limit.rlim_max}),
        );
    }
    for (name, expected) in INPUTS {
        ensure!(
            hash_file(&Path::new("/s9").join(name))?.sha256 == *expected,
            "native firmware component hash differs"
        );
    }
    Ok(limits.into())
}
fn main() -> Result<()> {
    let resource_limits = confinement()?;
    ensure!(
        !Path::new("/worker/native").exists(),
        "native output must be new"
    );
    fs::create_dir("/worker/native")?;
    let mut nonce = [0; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut nonce)?;
    let format = sd::format(
        Path::new(DISK),
        Path::new("/usr/bin/mke2fs"),
        &hex::encode(nonce),
    )?;
    let initial_storage = sd::capture(Path::new(DISK), "formatted")?;
    save_json(
        Path::new("/worker/native/storage-initial.json"),
        &initial_storage,
    )?;
    unsafe {
        libc::signal(libc::SIGINT, stop as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, stop as *const () as libc::sighandler_t);
    }
    let args=["-machine","xilinx-zynq-a9","-accel","tcg,thread=single","-m","1024M","-smp","1","-nodefaults","-nic","user,model=cadence_gem,restrict=on,ipv6=off,hostfwd=tcp:127.0.0.1:18080-10.0.2.15:8000","-display","none","-monitor","none","-serial","null","-serial","stdio","-drive","file=/worker/native/persistent.img,if=sd,format=raw","-kernel","/s9/kernel","-initrd","/s9/ramdisk","-dtb","/s9/board.dtb","-append","earlycon=cdns,mmio,0xe0001000 console=ttyPS0,115200n8 initcall_blacklist=xiic_i2c_driver_init root=/dev/ram0 init=/bin/sh","-sandbox","on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny"];
    let mut inputs = serde_json::Map::new();
    for (name, _) in INPUTS {
        inputs.insert(
            (*name).into(),
            serde_json::to_value(hash_file(&Path::new("/s9").join(name))?)?,
        );
    }
    let manifest = json!({"schema":"md-sim-native-network-diagnostic/1","firmware_artifact":"braiins-feed-25bba3b4a505ccbc0e54","archive_sha256":"469743ebcd5faf3445e4f4209ad8af58cc7adeff37c7df28af3c53a5b255b56e","inputs":inputs,"qemu":hash_file(Path::new("/usr/bin/qemu-system-arm"))?,"driver":hash_file(&std::env::current_exe()?)?,"script_sha256":md_sim_firmware::prepare::hash_bytes(SCRIPT),"argv":args,"storage":{"initial":initial_storage["raw"],"format":format,"kind":"new8MiB ext2 SD fixture"},"firmware_identity":"original-bytes","dashboard_source":"adapted-native","adaptations":["direct kernel loader","oneCPU","XIIC driver blacklist","init shell override","manual native service startup after each observed boot","run-owned SD /etc overlay","guest Ethernet static address","restricted user network and explicit loopback HTTP forward"],"isolation":"captured disconnected VM; separate worker network/mount/PID namespace; dedicated UID1000, no capabilities/no-new-privileges; bounded emulator; acquired code has no host filesystem or host network","full_native_dashboard_gate":"requires selected endpoint client results","original_board_gate":"incomplete","mining_gate":"not-run"});
    let mut manifest = manifest;
    manifest["resource_limits"] = resource_limits;
    save_json(Path::new("/worker/native/run.json"), &manifest)?;
    let mut command = Command::new("/usr/bin/qemu-system-arm");
    command
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(fs::File::create("/worker/native/qemu.log")?));
    let parent = std::process::id() as libc::pid_t;
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                || libc::getppid() != parent
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = Owned(command.spawn()?);
    let mut input = Some(child.0.stdin.take().context("QEMU stdin absent")?);
    let mut output = child.0.stdout.take().context("QEMU stdout absent")?;
    let mut serial = fs::File::create("/worker/native/serial.log")?;
    let mut startup = fs::File::create("/worker/native/startup.jsonl")?;
    let began = Instant::now();
    let mut next_observation = Duration::ZERO;
    let mut window = Vec::new();
    let mut sender = None;
    // Hold stdin after script transfer. QEMU's stdio backend must remain
    // connected while the guest can still write serial diagnostics.
    let mut held_input: Option<ChildStdin> = None;
    let mut ready = false;
    let mut resets = 0u32;
    let mut boots = 0u32;
    let emulator_pid = child.0.id();
    let mut buffer = [0; 4096];
    while !STOP.load(Ordering::Acquire) {
        if sender.as_ref().is_some_and(thread::JoinHandle::is_finished) {
            held_input = Some(
                sender
                    .take()
                    .unwrap()
                    .join()
                    .map_err(|_| anyhow::anyhow!("serial sender panicked"))??,
            );
        }
        if !ready && began.elapsed() >= next_observation {
            writeln!(
                startup,
                "{}",
                startup_observation(child.0.id(), began.elapsed(), false)
            )?;
            startup.flush()?;
            next_observation = began.elapsed() + Duration::from_secs(20);
        }
        if let Some(status) = child.0.try_wait()? {
            anyhow::bail!(
                "native emulator exited before stop: {}",
                exit_evidence(Some(status))?
            );
        }
        let mut poll = libc::pollfd {
            fd: output.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut poll, 1, 100) };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        if result == 0 {
            continue;
        }
        let count = output.read(&mut buffer)?;
        if count == 0 {
            let deadline = Instant::now() + Duration::from_secs(1);
            let mut status = child.0.try_wait()?;
            while status.is_none() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
                status = child.0.try_wait()?;
            }
            anyhow::bail!(
                "native serial closed; emulator exit evidence: {}",
                exit_evidence(status)?
            );
        }
        serial.write_all(&buffer[..count])?;
        window.extend_from_slice(&buffer[..count]);
        let reset_marker = b"reboot: Restarting system";
        if let Some(offset) = window
            .windows(reset_marker.len())
            .position(|w| w == reset_marker)
        {
            ensure!(ready && resets == 0, "unexpected native guest reset");
            ensure!(sender.is_none(), "guest reset during startup transfer");
            input = Some(
                held_input
                    .take()
                    .context("native serial input absent at reset")?,
            );
            ready = false;
            resets += 1;
            // Ignore old prompts and readiness text before the real reset.
            window.drain(..offset + reset_marker.len());
            writeln!(
                startup,
                "{}",
                json!({"observed_kernel_reset":resets,
                "native_qemu_pid":emulator_pid,"elapsed_ms":began.elapsed().as_millis()})
            )?;
            startup.flush()?;
        }
        if input.is_some() && window.windows(4).any(|w| w == b"/ # ") {
            let mut input = input.take().unwrap();
            window.clear();
            sender = Some(thread::spawn(move || -> std::io::Result<ChildStdin> {
                for chunk in SCRIPT.chunks(32) {
                    input.write_all(chunk)?;
                    input.flush()?;
                    thread::sleep(Duration::from_millis(40));
                }
                Ok(input)
            }));
        }
        if !ready
            && window
                .windows(b"\r\nMD_SIM_NATIVE_NETWORK_READY\r\n".len())
                .any(|w| w == b"\r\nMD_SIM_NATIVE_NETWORK_READY\r\n")
        {
            ready = true;
            boots += 1;
            writeln!(
                startup,
                "{}",
                startup_observation(child.0.id(), began.elapsed(), true)
            )?;
            startup.sync_all()?;
            serial.sync_all()?;
            save_json(
                Path::new("/worker/native/ready.json"),
                &json!({"actual_guest_login_probe":403,"boot_number":boots,"elapsed_ms":began.elapsed().as_millis(),"startup_observations":hash_file(Path::new("/worker/native/startup.jsonl"))?,"run_manifest":hash_file(Path::new("/worker/native/run.json"))?,"upstream":"http://127.0.0.1:18080/","native_dashboard_gate":"requires independent client test"}),
            )?;
            window.clear();
        }
        if window.len() > 8192 {
            window.drain(..window.len() - 8192);
        }
    }
    drop(child);
    if let Some(sender) = sender {
        sender
            .join()
            .map_err(|_| anyhow::anyhow!("serial sender panicked"))??;
    }
    drop(held_input);
    serial.sync_all()?;
    ensure!(ready, "native service never became ready");
    save_json(
        Path::new("/worker/native/storage-final.json"),
        &sd::capture(Path::new(DISK), "stopped")?,
    )?;
    save_json(
        Path::new("/worker/native/lifecycle.json"),
        &json!({"native_qemu_pid":emulator_pid,"observed_kernel_resets":resets,
            "ready_boots":boots,"run_manifest":hash_file(Path::new("/worker/native/run.json"))?,
            "serial":hash_file(Path::new("/worker/native/serial.log"))?,
            "startup":hash_file(Path::new("/worker/native/startup.jsonl"))?}),
    )?;
    Ok(())
}

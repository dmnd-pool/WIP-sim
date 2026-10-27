//! Exercise a native worker beside an isolated client VM. Selected modes test
//! direct or SSH transport. Native dashboard acceptance remains separate.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{
        execute_with_trace, save_json, Limits, Mount, ProcessSpec, StdinStream, TraceFiles,
    },
    store::{hash_file, ObjectId, Store},
    worker::{request_device, Request},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::{net::UnixStream, process::CommandExt},
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
};

#[allow(dead_code)]
#[path = "support/s9_boot_storage.rs"]
mod boot_storage;
#[path = "support/worker_channel_artifacts.rs"]
mod channel_artifacts;
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "support/worker_device_transport.rs"]
mod device_transport;
#[path = "support/worker_channel_execution.rs"]
mod execution;
#[path = "support/s9_execution_report.rs"]
mod execution_report;
#[allow(dead_code)]
#[path = "support/s9_pool_input.rs"]
mod pool_input;
#[path = "support/s9_pool_report.rs"]
mod pool_report;
#[allow(dead_code)]
#[path = "support/pool_vm.rs"]
mod pool_vm;
#[path = "support/prepared_board.rs"]
mod prepared;
#[path = "support/prepared_vm_image.rs"]
mod prepared_vm_image;
#[path = "support/worker_channel_retention.rs"]
mod retention;
#[path = "support/worker_retention_frame.rs"]
mod retention_frame;
#[path = "support/worker_shared_host.rs"]
mod shared_host;
#[allow(dead_code)]
#[path = "support/s9_shared_pool.rs"]
mod shared_pool;
use shared_pool as shared_manifest;
#[allow(dead_code)]
#[path = "support/worker_shared_retention.rs"]
mod shared_retention;
#[path = "support/worker_channel_ssh.rs"]
mod ssh;
#[path = "support/worker_virtual_port.rs"]
mod virtual_port;
const KERNEL: &str = "/usr/lib/modules/7.2.9-arch1-1/vmlinuz";
const MARKER: &str = "MD_SIM_VIRTUAL_WORKER_CHANNEL ";
const DISCONNECT: &str = "MD_SIM_WORKER_CLIENT_DISCONNECT ";
const STALL: &str = "MD_SIM_WORKER_CLIENT_STALL ";
const HOST: &str = "MD_SIM_VIRTUAL_WORKER_HOST ";

struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn guard_child(command: &mut Command) {
    let parent = std::process::id() as libc::pid_t;
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                return Err(std::io::Error::from_raw_os_error(libc::ECANCELED));
            }
            Ok(())
        });
    }
}

fn attach(
    command: &mut Command,
    stream: &UnixStream,
    additional: Option<&UnixStream>,
) -> Result<()> {
    guard_child(command);
    let reserve = |stream: &UnixStream| -> Result<OwnedFd> {
        let fd = unsafe { libc::fcntl(stream.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 64) };
        ensure!(
            fd >= 0,
            "reserve private endpoint: {}",
            std::io::Error::last_os_error()
        );
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    };
    let control = reserve(stream)?;
    let extra = additional.map(reserve).transpose()?;
    unsafe {
        command.pre_exec(move || {
            if libc::syscall(libc::SYS_close_range, 3u32, u32::MAX, 4u32) != 0
                || libc::dup2(control.as_raw_fd(), 3) != 3
                || libc::fcntl(3, libc::F_SETFD, 0) < 0
            {
                return Err(std::io::Error::last_os_error());
            }
            if let Some(extra) = &extra {
                if libc::dup2(extra.as_raw_fd(), 4) != 4 || libc::fcntl(4, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    Ok(())
}

fn guest(disconnect: bool, stall: bool) -> Result<()> {
    ensure!(
        fs::read_to_string("/proc/cmdline")?.contains("md_sim_private_guest=1"),
        "private client VM marker absent"
    );
    let mut device = virtual_port::open("md-sim.worker")?;
    let worker: ObjectId = serde_json::from_slice(&fs::read("/worker-identity.json")?)?;
    let probe = request_device(&mut device, &Request::Probe)?;
    ensure!(
        probe["worker_build"] == serde_json::to_value(&worker)?,
        "wrong worker"
    );
    if stall {
        println!(
            "{STALL}{}",
            json!({"worker":worker,"scope":"Client stopped after Probe; outer wall limit must retain this real guest output"})
        );
        std::io::stdout().flush()?;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }
    let bytes = fs::read("/payload.bin")?;
    ensure!(bytes.len() == 1024 * 1024, "probe payload bound");
    let object = hash_file(Path::new("/payload.bin"))?;
    let start = request_device(
        &mut device,
        &Request::Begin {
            object: object.clone(),
        },
    )?;
    ensure!(start["offset"] == 0, "unexpected existing transfer");
    for (index, chunk) in bytes.chunks(65536).enumerate() {
        let reply = request_device(
            &mut device,
            &Request::Chunk {
                object: object.clone(),
                offset: (index * 65536) as u64,
                hex: hex::encode(chunk),
            },
        )?;
        ensure!(
            reply["offset"] == (index + 1) * 65536,
            "transfer acknowledgment"
        );
        if disconnect {
            println!(
                "{DISCONNECT}{}",
                json!({"object":object,"acknowledged_bytes":65536,
                "scope":"Injected client failure before Finish; no object publication or process execution expected"})
            );
            anyhow::bail!("injected client disconnect after first acknowledged chunk");
        }
    }
    let finished = request_device(
        &mut device,
        &Request::Finish {
            object: object.clone(),
        },
    )?;
    ensure!(
        finished["verified"] == serde_json::to_value(&object)?,
        "destination hash"
    );
    let mut returned = Vec::new();
    for offset in (0..bytes.len()).step_by(65536) {
        let reply = request_device(
            &mut device,
            &Request::Get {
                object: object.clone(),
                offset: offset as u64,
                count: 65536,
            },
        )?;
        returned.extend(hex::decode(
            reply["hex"].as_str().context("returned bytes")?,
        )?);
    }
    ensure!(returned == bytes, "returned payload differs");
    let execution = execution::run(device, virtual_port::open("md-sim.retention")?)?;
    println!(
        "{MARKER}{}",
        json!({"passed":true,"worker":worker,"object":object,
        "client_kernel":fs::read_to_string("/proc/sys/kernel/osrelease")?.trim(),
        "uploaded_bytes":bytes.len(),"downloaded_bytes":returned.len(),"destination_verified":true,
        "process_execution":execution,
        "scope":"Actual local native worker and private client VM over virtio serial. No SSH, firmware, dashboard or external physical host claim."})
    );
    Ok(())
}

fn inside(disconnect: bool, firmware: bool, stall: bool, bad_ack: bool, ssh: bool) -> Result<()> {
    let shared = firmware && ssh;
    let kvm = match std::env::var("MD_SIM_SUPPORT_KVM") {
        Err(std::env::VarError::NotPresent) => false,
        Ok(value) if value == "1" => true,
        _ => anyhow::bail!("invalid support virtualization selection"),
    };
    let browser = match std::env::var("MD_SIM_CARRIER_BROWSER_SUPPORT") {
        Ok(value) if value == "1" => true,
        Err(std::env::VarError::NotPresent) => false,
        _ => anyhow::bail!("invalid carrier browser support selection"),
    };
    ensure!(
        !browser || shared,
        "browser support requires selected shared firmware"
    );
    ensure!(
        !kvm || (browser && shared),
        "KVM requires the selected browser support profile"
    );
    if kvm {
        confinement::require_native_browser_kvm_host()?;
    } else if browser {
        confinement::require_native_firmware_browser_worker_host()?;
    } else if firmware {
        confinement::require_native_firmware_worker_host()?;
    } else {
        confinement::require_native_worker_host()?;
    }
    let (worker_end, vm_end) = UnixStream::pair()?;
    let carrier = shared.then(UnixStream::pair).transpose()?;
    let mut command = Command::new("/worker");
    command.args(["/work/worker", "3"]).stdin(Stdio::null());
    if shared {
        command.args(["--private-carrier", "/provider.json", "4"]);
    }
    attach(
        &mut command,
        &worker_end,
        carrier.as_ref().map(|(worker, _)| worker),
    )?;
    let mut worker = Owned(command.spawn()?);
    drop(command);
    drop(worker_end);
    let mut command = Command::new("/qemu");
    command.args([
        "-machine",
        "pc",
        "-accel",
        if kvm {
            "kvm"
        } else if browser {
            // Captured selected-client comparison458b starts both Firefox
            // profiles within their existing budgets with this cache size.
            "tcg,thread=multi,tb-size=256"
        } else {
            "tcg,thread=single"
        },
        "-cpu",
        // Versioned QEMU model with the captured userspace's x86-64-v2
        // instructions. Refuse unavailable features instead of filtering them.
        if kvm { "Nehalem-v1,enforce" } else { "max" },
        "-m",
        if browser {
            "4096M"
        } else if firmware {
            "1536M"
        } else if ssh {
            "512M"
        } else {
            "256M"
        },
        "-nodefaults",
        "-no-user-config",
        "-nic",
        "none",
        "-display",
        "none",
        "-monitor",
        "none",
        "-serial",
        "file:/work/client-console.log",
        "-no-reboot",
        "-L",
        "/qemu-data",
        "-bios",
        "/qemu-data/bios-256k.bin",
        "-kernel",
        "/vmlinuz",
        "-initrd",
        "/initramfs.cpio.gz",
        "-append",
        if browser {
            "console=ttyS0 rdinit=/init panic=-1 rootfstype=ramfs md_sim_private_guest=1 md_sim_trace_port=1"
        } else {
            "console=ttyS0 rdinit=/init panic=-1 md_sim_private_guest=1 md_sim_trace_port=1"
        },
        "-chardev",
        "stdio,id=worker,signal=off",
        "-device",
        "virtio-serial-pci,id=serial",
        "-device",
        "virtserialport,bus=serial.0,chardev=worker,name=md-sim.worker",
        "-chardev",
        "file,id=trace,path=/work/client-userspace.log",
        "-device",
        "virtserialport,bus=serial.0,chardev=trace,name=md-sim.trace",
    ]);
    if browser {
        command.args(["-smp", "2"]);
    }
    if !disconnect && !stall && (!ssh || shared) {
        retention::start_relay()?;
        command.args([
            "-chardev",
            "pipe,id=retention,path=/work/retention",
            "-device",
            "virtserialport,bus=serial.0,chardev=retention,name=md-sim.retention",
        ]);
    }
    if let Some((worker_peer, vm_peer)) = carrier {
        command.args([
            "-netdev",
            "socket,id=private,fd=3,x-md-sim-private-stream=on",
            "-device",
            "e1000,netdev=private,mac=52:54:00:12:34:64,romfile=",
        ]);
        attach(&mut command, &vm_peer, None)?;
        drop(worker_peer);
        drop(vm_peer);
    }
    // This pinned minimal backend has no optional libseccomp support. The
    // enclosing ProcessSpec supplies namespaces, a closed root, dropped
    // capabilities, no-new-privileges and resource limits. Neither process
    // receives a host network interface. No extra seccomp layer is claimed.
    // QEMU's ordinary stdio character backend uses read/write on this private
    // stream. Its socket character backend needs host-denied getsockopt calls.
    // Guest protocol output uses a dedicated virtual port. Kernel console
    // bytes stay in a separate file and cannot interrupt a JSON frame.
    let input: OwnedFd = vm_end.try_clone()?.into();
    let output: OwnedFd = vm_end.into();
    command
        .stdin(Stdio::from(input))
        .stdout(Stdio::from(output));
    let support_vm = json!({"browser_requested":browser,
        "argv":command.get_args().map(|arg|arg.to_string_lossy().into_owned()).collect::<Vec<_>>(),
        "scope":"Support VM resources only; selected native job specification unchanged"});
    guard_child(&mut command);
    let mut vm = Owned(command.spawn()?);
    // Command retains the configured standard descriptors after spawn.
    // Release them so the worker sees EOF if the VM fails before Shutdown.
    drop(command);
    // Retain both streams while the VM runs. A wall-limit kill must not
    // discard all guest evidence with the private temporary filesystem.
    let mut userspace = None;
    let mut userspace_bytes = 0usize;
    let mut console = None;
    let mut console_bytes = 0usize;
    let status = loop {
        // Read the process status before draining the file. If the process
        // exited, all bytes it wrote are present before the final drain.
        let status = vm.0.try_wait()?;
        relay_vm_trace(
            Path::new("/work/client-userspace.log"),
            &mut userspace,
            &mut userspace_bytes,
            (if firmware {
                120
            } else if ssh {
                8
            } else {
                4
            }) << 20,
            &mut std::io::stdout(),
        )?;
        relay_vm_trace(
            Path::new("/work/client-console.log"),
            &mut console,
            &mut console_bytes,
            4 << 20,
            &mut std::io::stderr(),
        )?;
        if let Some(status) = status {
            break status;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    ensure!(status.success(), "client VM failed");
    ensure!(worker.0.wait()?.success(), "worker failed");
    if disconnect {
        ensure!(
            fs::read_dir("/work/worker/store/objects")?.next().is_none(),
            "interrupted object was published"
        );
        ensure!(
            fs::read_dir("/work/worker/runs")?.next().is_none(),
            "interrupted client started a process"
        );
    }
    let unacknowledged_result = if bad_ack {
        let runs = fs::read_dir("/work/worker/runs")?.collect::<std::io::Result<Vec<_>>>()?;
        ensure!(
            runs.len() == 1 && runs[0].file_type()?.is_dir(),
            "wrong-receipt run absent"
        );
        let path = runs[0].path();
        ensure!(
            !path.join("cleanup.json").exists(),
            "worker cleaned an unacknowledged result"
        );
        let run_id = runs[0]
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("worker run ID is not UTF-8"))?;
        Some(json!({"run_id":run_id,"result":hash_file(&path.join("result.json"))?}))
    } else {
        None
    };
    println!(
        "{HOST}{}",
        json!({"worker_build":hash_file(Path::new("/worker"))?,
        "executor_kernel":fs::read_to_string("/proc/sys/kernel/osrelease")?.trim(),
        "executor_uid":unsafe {libc::geteuid()},"executor_gid":unsafe {libc::getegid()},
        "uid_map":fs::read_to_string("/proc/self/uid_map")?,
        "disconnect_unpublished_and_no_run":disconnect,
        "unacknowledged_result":unacknowledged_result,
        "support_vm":support_vm,
        "shared_carrier":if shared {Some(serde_json::from_slice::<Value>(&fs::read("/work/worker/private-carrier.json")?)?)} else {None},
        "transport":"inherited connected Unix socket; no listener or host directory shared with VM"})
    );
    Ok(())
}

fn relay_vm_trace(
    path: &Path,
    reader: &mut Option<File>,
    bytes: &mut usize,
    limit: usize,
    output: &mut impl Write,
) -> Result<()> {
    if reader.is_none() {
        match File::open(path) {
            Ok(file) => *reader = Some(file),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    }
    let file = reader.as_mut().context("VM trace file absent")?;
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        *bytes = bytes
            .checked_add(count)
            .context("VM trace count overflow")?;
        ensure!(
            *bytes <= limit,
            "VM trace exceeds bound: {}",
            path.display()
        );
        output.write_all(&buffer[..count])?;
    }
    output.flush()?;
    Ok(())
}

#[cfg(test)]
mod trace_tests {
    use super::*;

    #[test]
    fn partial_json_and_kernel_noise_remain_separate_across_live_polls() -> Result<()> {
        let root = tempfile::tempdir()?;
        let user = root.path().join("userspace");
        let kernel = root.path().join("kernel");
        let payload = "a".repeat(70_000);
        fs::write(&user, format!("{{\"payload\":\"{payload}"))?;
        fs::write(&kernel, b"labelled kernel noise\n")?;
        let (mut user_reader, mut kernel_reader) = (None, None);
        let (mut user_count, mut kernel_count) = (0, 0);
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        relay_vm_trace(
            &user,
            &mut user_reader,
            &mut user_count,
            100_000,
            &mut stdout,
        )?;
        relay_vm_trace(
            &kernel,
            &mut kernel_reader,
            &mut kernel_count,
            4096,
            &mut stderr,
        )?;
        fs::OpenOptions::new()
            .append(true)
            .open(&user)?
            .write_all(b"\"}\n")?;
        relay_vm_trace(
            &user,
            &mut user_reader,
            &mut user_count,
            100_000,
            &mut stdout,
        )?;
        assert_eq!(
            serde_json::from_slice::<Value>(&stdout)?["payload"],
            payload
        );
        assert_eq!(stderr, b"labelled kernel noise\n");
        assert_eq!(user_count, stdout.len());
        assert_eq!(kernel_count, stderr.len());
        Ok(())
    }

    #[test]
    fn delayed_trace_creation_and_growth_keep_the_original_byte_bound() -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("trace");
        let (mut reader, mut count, mut output) = (None, 0, Vec::new());
        relay_vm_trace(&path, &mut reader, &mut count, 3, &mut output)?;
        assert_eq!(count, 0);
        fs::write(&path, b"abc")?;
        relay_vm_trace(&path, &mut reader, &mut count, 3, &mut output)?;
        assert_eq!(output, b"abc");
        fs::OpenOptions::new()
            .append(true)
            .open(&path)?
            .write_all(b"d")?;
        assert!(relay_vm_trace(&path, &mut reader, &mut count, 3, &mut output).is_err());
        assert_eq!(output, b"abc");
        Ok(())
    }
}

fn add(store: &Store, inputs: &mut Vec<Mount>, path: &Path, guest: &str) -> Result<()> {
    let object = hash_file(path)?;
    store.import(path, &object)?;
    inputs.push(Mount {
        object,
        guest_path: guest.into(),
    });
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args
        .get(1)
        .is_some_and(|s| s == "__execution" || s == "__execution_long")
    {
        println!(
            "MD_SIM_CHANNEL_PROCESS_START {}",
            json!({"uid_map":fs::read_to_string("/proc/self/uid_map")?,
            "kernel":fs::read_to_string("/proc/sys/kernel/osrelease")?})
        );
        confinement::require(false)?;
        let long_running = args[1] == "__execution_long";
        for sequence in 0..if long_running { 600 } else { 1 } {
            println!(
                "MD_SIM_CHANNEL_PROCESS {}",
                json!({"challenge":hash_file(Path::new("/challenge.bin"))?,"sequence":sequence})
            );
            std::io::stdout().flush()?;
            if long_running {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        return Ok(());
    }
    if args
        .get(1)
        .is_some_and(|s| s == "__guest" || s == "__guest_disconnect" || s == "__guest_stall")
    {
        return guest(args[1] == "__guest_disconnect", args[1] == "__guest_stall");
    }
    if args.get(1).is_some_and(|s| {
        s == "__inside"
            || s == "__inside_disconnect"
            || s == "__firmware_inside"
            || s == "__inside_stall"
            || s == "__inside_bad_ack"
            || s == "__inside_ssh"
            || s == "__firmware_ssh_inside"
    }) {
        return inside(
            args[1] == "__inside_disconnect",
            args[1] == "__firmware_inside" || args[1] == "__firmware_ssh_inside",
            args[1] == "__inside_stall",
            args[1] == "__inside_bad_ack",
            args[1] == "__inside_ssh" || args[1] == "__firmware_ssh_inside",
        );
    }
    ensure!((4..=8).contains(&args.len()),
        "usage: worker_channel_probe WORKER_IPC QEMU_BUILD NEW_OUTPUT [prepared=DIRECTORY] [ssh-helper=REMOTE_NAMESPACE_CHECK] [ssh-browser-payload=MANIFEST] [support-accel=kvm] [fault=client-disconnect|client-stall|wrong-receipt]");
    let mut prepared_source = None;
    let mut ssh_helper = None;
    let mut browser_payload = None;
    let mut fault = None;
    let mut support_kvm = false;
    for option in &args[4..] {
        let (key, value) = option
            .to_str()
            .and_then(|s| s.split_once('='))
            .context("invalid channel option")?;
        ensure!(!value.is_empty(), "empty channel option");
        match key {
            "support-accel" if value == "kvm" => {
                ensure!(!support_kvm, "duplicate support accelerator");
                support_kvm = true;
            }
            "prepared" => ensure!(
                prepared_source.replace(Path::new(value)).is_none(),
                "duplicate prepared source"
            ),
            "ssh-helper" => ensure!(
                ssh_helper.replace(Path::new(value)).is_none(),
                "duplicate SSH helper"
            ),
            "ssh-browser-payload" => ensure!(
                browser_payload.replace(Path::new(value)).is_none(),
                "duplicate SSH browser payload"
            ),
            "fault"
                if matches!(
                    value,
                    "client-disconnect" | "client-stall" | "wrong-receipt"
                ) =>
            {
                ensure!(fault.replace(value).is_none(), "duplicate fault")
            }
            _ => anyhow::bail!("unknown channel option"),
        }
    }
    ensure!(
        fault.is_none() || (prepared_source.is_none() && ssh_helper.is_none()),
        "fault injection requires the plain support process"
    );
    let disconnect = fault == Some("client-disconnect");
    let stall = fault == Some("client-stall");
    let bad_ack = fault == Some("wrong-receipt");
    let firmware = prepared_source.is_some();
    let ssh = ssh_helper.is_some();
    let shared = firmware && ssh;
    let browser_identity = browser_payload.map(hash_file).transpose()?;
    ensure!(
        browser_payload.is_none() || (shared && fault.is_none()),
        "browser capture requires selected shared firmware execution"
    );
    ensure!(
        !support_kvm || (shared && browser_payload.is_some() && fault.is_none()),
        "KVM requires selected shared browser support without fault injection"
    );
    let worker = Path::new(&args[1]).canonicalize()?;
    let backend = Path::new(&args[2]);
    let backend_report = backend.join("build-report.json");
    ensure!(
        fs::metadata(&backend_report)?.len() <= 16 << 20,
        "backend report bound"
    );
    let build: Value = serde_json::from_slice(&fs::read(&backend_report)?)?;
    let qemu = backend.join("qemu-system-x86_64");
    ensure!(
        build["source"] == "2a40e6705e59fd8807012631bc6b2c0877f10481b12768f78e4f9d7b88376cce"
            && build["build"]["code"] == 0
            && build["binary"] == serde_json::to_value(hash_file(&qemu)?)?,
        "pinned x86 backend differs"
    );
    let out = PathBuf::from(&args[3]);
    ensure!(!out.exists(), "probe output must be new");
    fs::create_dir_all(&out)?;
    let executable = std::env::current_exe()?;
    let mut payload = vec![0; 1024 * 1024];
    File::open("/dev/urandom")?.read_exact(&mut payload)?;
    fs::write(out.join("payload.bin"), payload)?;
    save_json(&out.join("worker-identity.json"), &hash_file(&worker)?)?;
    let execution_out = out.join("execution");
    let execution_store = Store::open(&execution_out.join("input-store"), 512 << 20)?;
    let mut files = execution::stage(
        &execution_store,
        &executable,
        &out.join("payload.bin"),
        &execution_out,
        prepared_source,
        ssh,
    )?;
    drop(execution_store);
    files.insert(
        "/payload.bin".into(),
        json!({"path":out.join("payload.bin").canonicalize()?,"mode":420}),
    );
    files.insert(
        "/worker-identity.json".into(),
        json!({"path":out.join("worker-identity.json").canonicalize()?,"mode":420}),
    );
    if let Some(helper) = ssh_helper {
        ssh::prepare(helper, &out, shared.then_some(backend), browser_payload)?;
    } else {
        save_json(
            &out.join("scenario.json"),
            &json!({"programs":{"/channel-check":executable},
            "files":files,
            "argv":["/channel-check",if disconnect {"__guest_disconnect"} else if stall {"__guest_stall"} else {"__guest"}],"environment":{}}),
        )?;
    }
    let preparation = out.join("client-image");
    let scenario: Value =
        serde_json::from_slice(&prepared::bounded(&out.join("scenario.json"), 4 << 20)?)?;
    if let Some(expected) = &browser_identity {
        let selected = scenario["payload_manifest"]
            .as_str()
            .context("browser payload path absent")?;
        ensure!(
            hash_file(Path::new(selected))? == *expected,
            "scenario payload differs from the original browser request"
        );
    }
    let access = scenario["environment"]["MD_SIM_CARRIER_DASHBOARD"] == "1";
    let result = Command::new(
        executable
            .parent()
            .context("example directory")?
            .join("isolated_network_vm"),
    )
    .arg(&preparation)
    .arg(out.join("scenario.json"))
    .arg("--prepare-only")
    .stdout(File::create(out.join("preparation.stdout"))?)
    .stderr(File::create(out.join("preparation.stderr"))?)
    .status()?;
    ensure!(result.success(), "client image preparation failed");
    let image = prepared_vm_image::select(
        &preparation,
        &scenario,
        &hash_file(Path::new(KERNEL))?,
        browser_identity.as_ref(),
    )?;
    let store = Store::open(&out.join("input-store"), 512 << 20)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/probe".into(), executable),
            ("/worker".into(), worker),
            ("/qemu".into(), qemu),
            ("/usr/bin/unshare".into(), PathBuf::from("/usr/bin/unshare")),
            ("/usr/bin/prlimit".into(), PathBuf::from("/usr/bin/prlimit")),
            ("/usr/bin/bwrap".into(), PathBuf::from("/usr/bin/bwrap")),
        ],
    )?;
    add(&store, &mut inputs, Path::new(KERNEL), "/vmlinuz")?;
    add(&store, &mut inputs, &backend_report, "/backend-build.json")?;
    store.import(&image.path, &image.object)?;
    inputs.push(Mount {
        guest_path: "/initramfs.cpio.gz".into(),
        object: image.object,
    });
    for name in ["bios-256k.bin", "linuxboot_dma.bin", "kvmvapic.bin"] {
        add(
            &store,
            &mut inputs,
            &Path::new("/usr/share/qemu").join(name),
            &format!("/qemu-data/{name}"),
        )?;
    }
    if shared {
        add(
            &store,
            &mut inputs,
            &out.join("provider.json"),
            "/provider.json",
        )?;
    }
    let spec = ProcessSpec {
        support_virtualization: support_kvm
            .then_some(md_sim_runtime::isolation::SupportVirtualization::Kvm),
        private_channel: None,
        stdin_stream: if disconnect || stall || (ssh && !shared) {
            None
        } else {
            Some(StdinStream {
                protocol: if access {
                    shared_retention::ACCESS_SCHEMA
                } else if shared {
                    shared_retention::SCHEMA
                } else {
                    retention::PROTOCOL
                }
                .into(),
                max_bytes: 4096,
            })
        },
        namespace_uid: 1000,
        schema_version: "2.0".into(),
        argv: vec![
            "/probe".into(),
            if disconnect {
                "__inside_disconnect"
            } else if stall {
                "__inside_stall"
            } else if bad_ack {
                "__inside_bad_ack"
            } else if shared {
                "__firmware_ssh_inside"
            } else if firmware {
                "__firmware_inside"
            } else if ssh {
                "__inside_ssh"
            } else {
                "__inside"
            }
            .into(),
        ],
        inputs,
        stdin_object: None,
        environment: {
            let mut environment = BTreeMap::new();
            if browser_payload.is_some() {
                environment.insert("MD_SIM_CARRIER_BROWSER_SUPPORT".into(), "1".into());
            }
            if support_kvm {
                environment.insert("MD_SIM_SUPPORT_KVM".into(), "1".into());
            }
            environment
        },
        dashboard: None,
        loopback: false,
        limits: Limits {
            wall_seconds: if browser_payload.is_some() {
                1200
            } else if stall {
                15
            } else if disconnect {
                30
            } else if firmware {
                900
            } else if ssh {
                360
            } else {
                180
            },
            cpu_seconds: if browser_payload.is_some() {
                2000
            } else if disconnect {
                20
            } else if firmware {
                850
            } else {
                110
            },
            memory_bytes: if browser_payload.is_some() {
                6
            } else if firmware {
                4
            } else {
                3
            } << 30,
            processes: 64,
            file_bytes: if firmware { 256 } else { 64 } << 20,
            trace_bytes: if firmware {
                128
            } else if ssh {
                8
            } else {
                4
            } << 20,
            tmp_bytes: if firmware { 1024 } else { 128 } << 20,
        },
    };
    save_json(&out.join("process-spec.json"), &spec)?;
    if shared {
        ssh::support_proof(&out, &preparation)?;
    }
    let result = if shared {
        shared_host::execute(&store, &spec, &out, &execution_out)?
    } else if disconnect || stall || ssh {
        execute_with_trace(
            &store,
            &spec,
            Arc::new(AtomicBool::new(false)),
            Some(TraceFiles {
                stdout: File::create(out.join("guest.log"))?,
                stderr: File::create(out.join("backend.log"))?,
            }),
        )?
    } else {
        retention::execute(&store, &spec, &out, &execution_out, bad_ack)?
    };
    save_json(&out.join("result.json"), &result)?;
    if shared {
        save_json(
            &execution_out.join("shared-pool/outer-result.json"),
            &json!({
                "process":hash_file(&out.join("process-spec.json"))?,
                "result":hash_file(&out.join("result.json"))?,
                "parent_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("worker_channel_probe.rs")),
                "scope":"Exact outer execution result; success requires the separate retained evidence assessment."
            }),
        )?;
    }
    let text = String::from_utf8(hex::decode(&result.stdout)?)?;
    let records: Vec<Value> = text
        .lines()
        .filter_map(|s| {
            s.strip_prefix(if shared {
                shared_host::MARKER
            } else if ssh {
                ssh::MARKER
            } else {
                MARKER
            })
        })
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    let exports: Vec<Value> = text
        .lines()
        .filter_map(|line| line.strip_prefix(execution::ARTIFACTS))
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    let host_records: Vec<Value> = text
        .lines()
        .filter_map(|s| s.strip_prefix(HOST))
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    let provider_observation = if shared {
        if host_records.len() == 1 {
            shared_host::host_receipt(&host_records[0], &execution_out)
        } else {
            Err(anyhow::anyhow!(
                "native provider host receipt absent or duplicated"
            ))
        }
    } else {
        Ok(())
    };
    let observation = (|| -> Result<()> {
        if ssh {
            ensure!(
                records.len() == 1 && exports.is_empty(),
                "SSH result export absent or duplicated"
            );
            return if shared {
                shared_host::restore(&records[0], &execution_out)
            } else {
                ssh::restore(&records[0], &execution_out, &out.join("payload.bin"))
            };
        }
        ensure!(exports.len() <= 1, "duplicate artifact export");
        if exports.is_empty() {
            ensure!(disconnect || stall, "complete result export absent");
            return Ok(());
        }
        let result = execution::restore(&exports[0], &execution_out)?
            .context("complete worker result absent")?;
        ensure!(
            hash_file(&execution_out.join("host-retention.json"))?
                == hash_file(&execution_out.join("worker-retention.json"))?,
            "host and worker retention receipts differ"
        );
        ensure!(
            !execution_out.join("host-retention-error.json").exists(),
            "host retention failed"
        );
        if firmware {
            let process: ProcessSpec = serde_json::from_slice(&prepared::bounded(
                &execution_out.join("process-spec.json"),
                4 << 20,
            )?)?;
            execution_report::retain(&execution_out, &process, &result)?;
        }
        Ok(())
    })();
    let passed = result.status == "exited"
        && result.exit_code == Some(0)
        && !result.trace_truncated
        && observation.is_ok()
        && provider_observation.is_ok()
        && records.len() == 1
        && host_records.len() == 1
        && host_records[0]["support_vm"]["browser_requested"] == browser_payload.is_some()
        && host_records[0]["executor_uid"] == 1000
        && host_records[0]["worker_build"]
            == serde_json::to_value(hash_file(Path::new(&args[1]))?)?
        && records[0]["passed"] == true
        && (ssh
            || records[0]["object"] == serde_json::to_value(hash_file(&out.join("payload.bin"))?)?);
    let fault_records: Vec<Value> = text
        .lines()
        .filter_map(|s| s.strip_prefix(if stall { STALL } else { DISCONNECT }))
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    let disconnect_passed = disconnect
        && result.status == "exited"
        && result.exit_code == Some(0)
        && !result.trace_truncated
        && records.is_empty()
        && fault_records.len() == 1
        && host_records.len() == 1
        && fault_records[0]["acknowledged_bytes"] == 65536
        && fault_records[0]["object"]
            == serde_json::to_value(hash_file(&out.join("payload.bin"))?)?
        && host_records[0]["disconnect_unpublished_and_no_run"] == true;
    let stall_passed = stall
        && result.status == "timed-out"
        && !result.trace_truncated
        && records.is_empty()
        && exports.is_empty()
        && host_records.is_empty()
        && fault_records.len() == 1
        && fault_records[0]["worker"] == serde_json::to_value(hash_file(Path::new(&args[1]))?)?;
    let wrong_receipt_passed = if bad_ack
        && result.status == "exited"
        && result.exit_code == Some(0)
        && !result.trace_truncated
        && records.is_empty()
        && host_records.len() == 1
    {
        let expected: Value = serde_json::from_slice(&prepared::bounded(
            &execution_out.join("host-retention.json"),
            4096,
        )?)?;
        let error: Value = serde_json::from_slice(&prepared::bounded(
            &execution_out.join("worker-error.json"),
            65536,
        )?)?;
        host_records[0]["unacknowledged_result"]["result"] == expected["result"]
            && host_records[0]["unacknowledged_result"]["run_id"] == expected["run_id"]
            && error["error"]
                .as_str()
                .is_some_and(|e| e.contains("outer receipt identity differs"))
            && !execution_out.join("worker-cleanup.json").exists()
            && !execution_out.join("worker-retention.json").exists()
    } else {
        false
    };
    let fault_passed = disconnect_passed || stall_passed || wrong_receipt_passed;
    save_json(
        &out.join("assessment.json"),
        &json!({"passed":passed,"observations":records,
        "observation_error":observation.as_ref().err().map(|e|format!("{e:#}")),
        "provider_observation_error":provider_observation.as_ref().err().map(|e|format!("{e:#}")),
        "firmware_reference":firmware,"ssh_client_recovery":ssh && !shared,"shared_pool":shared,
        "carrier_browser_requested":browser_payload.is_some(),
        "complete_result_exported":(exports.len() == 1 || (ssh && records.len() == 1)) && observation.is_ok(),
        "fault":if disconnect {Some("client-disconnect")} else if stall {Some("client-stall")} else if bad_ack {Some("wrong-receipt")} else {None},
        "fault_gate_passed":fault_passed,"fault_observations":fault_records,"host_observations":host_records,
        "process_specification":hash_file(&out.join("process-spec.json"))?,"execution":hash_file(&out.join("result.json"))?,
        "source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("worker_channel_probe.rs")),
        "execution_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/worker_channel_execution.rs")),
        "confinement_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("../src/confinement.rs")),
        "worker_client_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("../../md-sim-runtime/src/worker.rs")),
        "runtime_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("../../md-sim-runtime/src/isolation.rs")),
        "artifact_codec_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/worker_channel_artifacts.rs")),
        "device_transport_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/worker_device_transport.rs")),
        "retention_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/worker_channel_retention.rs")),
        "virtual_port_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/worker_virtual_port.rs")),
        "ssh_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/worker_channel_ssh.rs")),
        "shared_host_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/worker_shared_host.rs")),
        "shared_retention_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/worker_shared_retention.rs")),
        "coordinator_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("../../md-sim-coordinator/src/executor.rs")),
        "preparation_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/prepared_board.rs")),
        "scope":if shared {"Actual archived firmware in a native worker, controlled by SSH in a disconnected support VM. Independent pool has a private Ethernet carrier in the server namespace. Adapted reference execution; mining/Core/dashboard require their separate evidence. No external physical host or exact-board pass."} else if ssh {"Actual SSH between isolated client/server namespaces in a disconnected VM, controlling a native worker outside it. Support-process client-loss/recovery/cancellation only. No firmware, dashboard, external physical host or whole-VM-loss pass."} else {"Transport component only. Local native worker plus isolated client VM. No SSH, firmware, dashboard or external physical host pass."}}),
    )?;
    ensure!(
        if disconnect || stall || bad_ack {
            fault_passed && !passed
        } else {
            passed
        },
        "private virtual worker channel failed; retained logs"
    );
    Ok(())
}

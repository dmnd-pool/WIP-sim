//! A local full-system Linux guest. It has no host network attachment.
//! Guest loopback tests are not an external host or simulator target claim.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, execute_with_trace, save_json, Limits, Mount, ProcessSpec, TraceFiles},
    store::{hash_file, Store},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::CString,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::fd::AsFd,
    os::unix::{fs::PermissionsExt, process::ExitStatusExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
};
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "isolated_network_vm/payload.rs"]
mod payload;
#[path = "isolated_network_vm/trace_port.rs"]
mod trace_port;
#[path = "support/worker_virtual_port.rs"]
mod virtual_port;
const KERNEL: &str = "/usr/lib/modules/7.2.9-arch1-1/vmlinuz";
const KERNEL_SHA: &str = "d02afb36d5f99390fdd8d04dedf8894354f4bbdc5e7d44afa5b897639bb3707c";
const EXPORT_HALF: usize = 64 * 1024 * 1024;
const EXPORT_PART2: &[u8] = b"MD_SIM_INITRAMFS_PART2_V2\n";
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GuestFile {
    path: PathBuf,
    mode: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    programs: BTreeMap<String, PathBuf>,
    files: BTreeMap<String, GuestFile>,
    argv: Vec<String>,
    environment: BTreeMap<String, String>,
    #[serde(default)]
    payload_manifest: Option<PathBuf>,
}
fn mount(source: &str, target: &str, kind: &str) -> Result<()> {
    let (s, t, k) = (
        CString::new(source)?,
        CString::new(target)?,
        CString::new(kind)?,
    );
    ensure!(
        unsafe { libc::mount(s.as_ptr(), t.as_ptr(), k.as_ptr(), 0, std::ptr::null()) } == 0,
        "guest mount {target}: {}",
        std::io::Error::last_os_error()
    );
    Ok(())
}
fn guest() -> Result<()> {
    ensure!(std::process::id() == 1, "guest init requires PID 1");
    mount("devtmpfs", "/dev", "devtmpfs")?;
    let console = CString::new("/dev/console")?;
    let fd = unsafe { libc::open(console.as_ptr(), libc::O_RDWR) };
    ensure!(fd >= 0, "guest console absent");
    for destination in 0..3 {
        ensure!(
            unsafe { libc::dup2(fd, destination) } >= 0,
            "console duplication failed"
        );
    }
    if fd > 2 {
        unsafe {
            libc::close(fd);
        }
    }
    mount("proc", "/proc", "proc")?;
    mount("sysfs", "/sys", "sysfs")?;
    let cmdline = fs::read_to_string("/proc/cmdline")?;
    ensure!(
        trace_port::selected(&cmdline, "md_sim_private_guest")?,
        "guest kernel marker missing"
    );
    let trace_selected = trace_port::selected(&cmdline, "md_sim_trace_port")?;
    let trace_stress = trace_port::selected(&cmdline, "md_sim_trace_stress")?;
    ensure!(
        !trace_stress || trace_selected,
        "trace stress requires the private port"
    );
    if trace_selected {
        trace_port::redirect()?;
    }
    println!("MD_SIM_VM_INIT actual Rust PID 1 started");
    println!(
        "MD_SIM_VM_KERNEL {}",
        fs::read_to_string("/proc/sys/kernel/osrelease")?.trim()
    );
    let names = fs::read_dir("/sys/class/net")?
        .map(|entry| Ok(entry?.file_name().to_string_lossy().into_owned()))
        .collect::<Result<BTreeSet<_>>>()?;
    ensure!(
        names == BTreeSet::from(["lo".into()]),
        "guest has an unexpected network device"
    );
    let socket = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) };
    ensure!(socket >= 0, "guest AF_INET denied");
    let mut request: libc::ifreq = unsafe { std::mem::zeroed() };
    request.ifr_name[0] = b'l' as i8;
    request.ifr_name[1] = b'o' as i8;
    ensure!(
        unsafe { libc::ioctl(socket, libc::SIOCGIFFLAGS, &mut request) } == 0,
        "loopback flag read failed"
    );
    unsafe {
        request.ifr_ifru.ifru_flags |= libc::IFF_UP as i16;
    }
    ensure!(
        unsafe { libc::ioctl(socket, libc::SIOCSIFFLAGS, &request) } == 0,
        "loopback setup failed"
    );
    unsafe {
        libc::close(socket);
    }
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let mut challenge = [0_u8; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut challenge)?;
    let child = std::thread::spawn(move || -> Result<()> {
        let mut client = TcpStream::connect(address)?;
        client.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
        client.write_all(&challenge)?;
        let mut response = [0_u8; 32];
        client.read_exact(&mut response)?;
        ensure!(response == challenge, "guest TCP response differs");
        Ok(())
    });
    let (mut connection, peer) = listener.accept()?;
    connection.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    let mut received = [0_u8; 32];
    connection.read_exact(&mut received)?;
    ensure!(received == challenge, "guest TCP request differs");
    connection.write_all(&received)?;
    child
        .join()
        .map_err(|_| anyhow::anyhow!("guest client panicked"))??;
    println!(
        "MD_SIM_VM_TCP {}",
        json!({"listener":address,"client":peer,"challenge_hex":hex::encode(challenge),"actual_roundtrip":true,"interfaces":names,"external_network":false})
    );
    if trace_stress {
        ensure!(
            !Path::new("/scenario.json").exists(),
            "trace prerequisite cannot execute a scenario"
        );
        trace_port::guest_stress()?;
    }
    if Path::new("/scenario.json").exists() {
        let scenario: Scenario = serde_json::from_slice(&fs::read("/scenario.json")?)?;
        ensure!(!scenario.argv.is_empty(), "scenario command is missing");
        for path in ["/work", "/run", "/var", "/var/empty"] {
            fs::create_dir_all(path)?;
        }
        let status = Command::new(&scenario.argv[0])
            .args(&scenario.argv[1..])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .envs(&scenario.environment)
            .current_dir("/work")
            .stdin(Stdio::null())
            .status()?;
        ensure!(status.success(), "guest scenario failed: {status}");
        println!("MD_SIM_VM_SCENARIO selected guest command exited 0");
    }
    println!("MD_SIM_VM_COMPLETE isolated guest loopback probe passed");
    std::io::stdout().flush()?;
    unsafe {
        libc::sync();
        libc::reboot(libc::LINUX_REBOOT_CMD_POWER_OFF);
    }
    anyhow::bail!("guest poweroff returned")
}
fn prepare_inside() -> Result<()> {
    confinement::require_private_vm_preparation()?;
    let manifest: Vec<Mount> = serde_json::from_slice(&fs::read("/root-manifest.json")?)?;
    fs::create_dir("/work/root")?;
    let mut names = BTreeSet::from([
        ".".to_string(),
        "dev".into(),
        "proc".into(),
        "sys".into(),
        "tmp".into(),
    ]);
    for name in ["dev", "proc", "sys", "tmp"] {
        fs::create_dir(Path::new("/work/root").join(name))?;
    }
    for input in manifest {
        ensure!(
            hash_file(Path::new(&input.guest_path))? == input.object,
            "initramfs input changed"
        );
        let relative = input
            .guest_path
            .strip_prefix('/')
            .context("nonabsolute input")?;
        ensure!(
            !relative
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
                && !relative.contains('\n'),
            "unsafe initramfs path"
        );
        let target = Path::new("/work/root").join(relative);
        fs::create_dir_all(target.parent().context("no file parent")?)?;
        fs::copy(&input.guest_path, &target)?;
        fs::set_permissions(&target, fs::Permissions::from_mode(0o555))?;
        names.insert(relative.into());
        for ancestor in Path::new(relative).ancestors().skip(1) {
            if !ancestor.as_os_str().is_empty() {
                names.insert(ancestor.to_string_lossy().into_owned());
            }
        }
    }
    if Path::new("/scenario.json").exists() {
        let scenario: Scenario = serde_json::from_slice(&fs::read("/scenario.json")?)?;
        for (name, file) in scenario.files {
            ensure!(file.mode & !0o777 == 0, "invalid guest file mode");
            fs::set_permissions(
                Path::new("/work/root").join(name.trim_start_matches('/')),
                fs::Permissions::from_mode(file.mode),
            )?;
        }
    }
    let mut cpio = Command::new("/tools/cpio")
        .args(["-o", "--format=newc", "--owner=0:0", "--quiet"])
        .current_dir("/work/root")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    cpio.stdin
        .take()
        .context("cpio stdin absent")?
        .write_all(format!("{}\n", names.into_iter().collect::<Vec<_>>().join("\n")).as_bytes())?;
    let output = cpio.wait_with_output()?;
    ensure!(
        output.status.success()
            && output.stdout.len() < 256 * 1024 * 1024
            && output.stderr.len() < 65536,
        "cpio preparation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gzip.write_all(&output.stdout)?;
    let compressed = gzip.finish()?;
    ensure!(
        compressed.len() <= 2 * EXPORT_HALF - EXPORT_PART2.len(),
        "compressed initramfs has {} bytes; exceeds bounded export of {} bytes",
        compressed.len(),
        2 * EXPORT_HALF - EXPORT_PART2.len()
    );
    let first = compressed.len().min(EXPORT_HALF);
    std::io::stdout().lock().write_all(&compressed[..first])?;
    if first < compressed.len() {
        let mut stderr = std::io::stderr().lock();
        stderr.write_all(EXPORT_PART2)?;
        stderr.write_all(&compressed[first..])?;
    }
    Ok(())
}
fn decode_export(first: &[u8], second: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        first.len() <= EXPORT_HALF && second.len() <= EXPORT_HALF,
        "export stream exceeds bound"
    );
    let mut bytes = first.to_vec();
    if !second.is_empty() {
        ensure!(
            first.len() == EXPORT_HALF,
            "unexpected second export stream"
        );
        bytes.extend_from_slice(
            second
                .strip_prefix(EXPORT_PART2)
                .context("invalid part2 export marker")?,
        );
    }
    let mut decoder = flate2::bufread::GzDecoder::new(bytes.as_slice());
    let count = std::io::copy(
        &mut decoder.by_ref().take(256 * 1024 * 1024 + 1),
        &mut std::io::sink(),
    )?;
    ensure!(
        count < 256 * 1024 * 1024 && decoder.get_ref().is_empty(),
        "invalid bounded initramfs gzip stream"
    );
    Ok(bytes)
}
fn add(store: &Store, inputs: &mut Vec<Mount>, source: &Path, guest: &str) -> Result<()> {
    let object = hash_file(source)?;
    store.import(source, &object)?;
    inputs.push(Mount {
        object,
        guest_path: guest.into(),
    });
    Ok(())
}
fn main() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.first().is_some_and(|s| s == "/init") {
        return guest();
    }
    if args.get(1).is_some_and(|s| s == "__prepare") {
        return prepare_inside();
    }
    if args.get(1).is_some_and(|s| {
        s == "__qemu"
            || s == "__nested_qemu"
            || s == "__native_qemu"
            || s == "__browser_qemu"
            || s == "__native_browser_qemu"
            || s == "__process_qemu"
            || s == "__extended_process_qemu"
            || s == "__long_process_qemu"
            || s == "__slow_process_qemu"
            || s == "__board_browser_qemu"
            || s == "__long_boot_browser_qemu"
            || s == "__long_boot_process_qemu"
    }) {
        let kvm = match std::env::var("MD_SIM_SUPPORT_KVM") {
            Err(std::env::VarError::NotPresent) => false,
            Ok(value) if value == "1" => true,
            _ => anyhow::bail!("invalid support virtualization selection"),
        };
        if kvm {
            ensure!(args[1] == "__qemu", "KVM prerequisite profile differs");
            confinement::require_kvm_prerequisite()?;
        } else if args[1] == "__long_boot_browser_qemu" || args[1] == "__long_boot_process_qemu" {
            confinement::require_long_boot_process_vm(args[1] == "__long_boot_browser_qemu")?;
        } else if args[1] == "__board_browser_qemu" {
            confinement::require_board_browser_vm()?;
        } else if args[1] == "__slow_process_qemu" {
            confinement::require_slow_process_vm()?;
        } else if args[1] == "__long_process_qemu" {
            confinement::require_long_process_vm()?;
        } else if args[1] == "__extended_process_qemu" {
            confinement::require_extended_process_vm()?;
        } else if args[1] == "__process_qemu" {
            confinement::require_process_vm()?;
        } else if args[1] == "__native_browser_qemu" {
            confinement::require_native_browser_vm()?;
        } else if args[1] == "__browser_qemu" {
            confinement::require_browser_vm()?;
        } else if args[1] == "__native_qemu" {
            confinement::require_native_dashboard_vm()?;
        } else if args[1] == "__nested_qemu" {
            confinement::require_nested_board()?;
        } else {
            confinement::require_board()?;
        }
        let trace_stress = match std::env::var("MD_SIM_TRACE_STRESS") {
            Err(std::env::VarError::NotPresent) => false,
            Ok(value) if value == "1" => true,
            _ => anyhow::bail!("invalid trace prerequisite selection"),
        };
        ensure!(
            !trace_stress || kvm,
            "trace prerequisite requires explicit KVM mode"
        );
        let began = std::time::Instant::now();
        let mut command = Command::new("/qemu");
        command.args(&args[2..]).stdin(Stdio::null());
        if trace_stress {
            command.stdout(Stdio::from(std::io::stderr().as_fd().try_clone_to_owned()?));
        }
        let mut child = command.spawn()?;
        let pid = child.id();
        let status = child.wait()?;
        if trace_stress {
            let mut trace = fs::File::open("/work/guest-trace.log")?;
            ensure!(
                trace.metadata()?.len() <= 16 * 1024 * 1024,
                "guest trace prerequisite file bound"
            );
            let copied = std::io::copy(
                &mut Read::by_ref(&mut trace).take(16 * 1024 * 1024 + 1),
                &mut std::io::stdout().lock(),
            )?;
            ensure!(
                copied <= 16 * 1024 * 1024,
                "guest trace prerequisite grew beyond bound"
            );
        }
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        ensure!(
            unsafe { libc::getrusage(libc::RUSAGE_CHILDREN, &mut usage) } == 0,
            "cannot retain outer QEMU resource usage"
        );
        // This isolated wrapper has exactly one child. Retain measurements
        // after wait even if the backend was killed before emitting evidence.
        println!(
            "MD_SIM_VM_RESOURCES {}",
            json!({"pid":pid,"exit_code":status.code(),"signal":status.signal(),
                "wall_ms":began.elapsed().as_millis(),
                "user_us":usage.ru_utime.tv_sec * 1_000_000 + usage.ru_utime.tv_usec,
                "system_us":usage.ru_stime.tv_sec * 1_000_000 + usage.ru_stime.tv_usec,
                "max_rss_kib":usage.ru_maxrss})
        );
        std::io::stdout().flush()?;
        ensure!(status.success(), "QEMU guest failed: {status}");
        return Ok(());
    }
    let trace_stress = args.len() == 3 && args[2] == "--kvm-trace-stress";
    let kvm = args.len() == 3 && (args[2] == "--kvm" || trace_stress);
    ensure!(
        (2..=3).contains(&args.len()) || (args.len() == 4 && args[3] == "--prepare-only"),
        "usage: isolated_network_vm NEW_OUTPUT [SCENARIO_JSON [--prepare-only] | --kvm | --kvm-trace-stress]"
    );
    let out = PathBuf::from(&args[1]);
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(&out)?;
    let result = run(
        &out,
        if kvm {
            None
        } else {
            args.get(2).map(Path::new)
        },
        args.len() == 4,
        kvm,
        trace_stress,
    );
    if let Err(error) = &result {
        let full = format!("{error:#}");
        let bounded: String = full.chars().take(8192).collect();
        save_json(
            &out.join("failure.json"),
            &json!({"error":bounded,"error_truncated":bounded.len()<full.len(),"guest_boot_and_tcp_probe":false}),
        )?;
        save_json(
            &out.join("assessment.json"),
            &json!({"guest_boot_and_tcp_probe":false,"actual_firmware_gate":false,"worker_transport_gate":false,"external_host_gate":false,"ssh_tls_gate":false,"failure":"failure.json"}),
        )?;
    }
    result
}
fn run(
    out: &Path,
    scenario_path: Option<&Path>,
    prepare_only: bool,
    kvm: bool,
    trace_stress: bool,
) -> Result<()> {
    let description: Option<Scenario> = scenario_path
        .map(|path| -> Result<_> {
            ensure!(
                fs::metadata(path)?.len() <= 1024 * 1024,
                "scenario exceeds metadata bound"
            );
            Ok(serde_json::from_slice(&fs::read(path)?)?)
        })
        .transpose()?;
    let browser = description
        .as_ref()
        .is_some_and(|s| s.payload_manifest.is_some());
    // The only input above 512MiB is a bounded combined browser initramfs.
    let store = Store::open(
        &out.join("input-store"),
        if browser {
            2 * 1024 * 1024 * 1024
        } else {
            512 * 1024 * 1024
        },
    )?;
    let mut init = md_sim_firmware::lab::elf_closure(
        &store,
        vec![("/init".into(), std::env::current_exe()?)],
    )?;
    let scenario = if let Some(scenario) = description {
        ensure!(
            !scenario.argv.is_empty() && scenario.programs.contains_key(&scenario.argv[0]),
            "scenario command must be an explicit supplied program"
        );
        ensure!(
            scenario.programs.len() <= 64 && scenario.files.len() <= 128,
            "scenario input count exceeds bound"
        );
        let extra = md_sim_firmware::lab::elf_closure(
            &store,
            scenario
                .programs
                .iter()
                .map(|(guest, host)| (guest.clone(), host.clone()))
                .collect(),
        )?;
        for input in extra {
            if let Some(old) = init.iter().find(|i| i.guest_path == input.guest_path) {
                ensure!(old.object == input.object, "conflicting guest input");
            } else {
                init.push(input);
            }
        }
        for (guest, file) in &scenario.files {
            ensure!(
                guest.starts_with('/')
                    && !guest
                        .split('/')
                        .skip(1)
                        .any(|s| s.is_empty() || s == "." || s == "..")
                    && !guest.contains('\n')
                    && !init.iter().any(|i| &i.guest_path == guest),
                "unsafe or duplicate scenario file"
            );
            add(&store, &mut init, &file.path, guest)?;
        }
        let saved = out.join("scenario.json");
        save_json(&saved, &scenario)?;
        add(&store, &mut init, &saved, "/scenario.json")?;
        Some(scenario)
    } else {
        None
    };
    if let Some(path) = scenario
        .as_ref()
        .and_then(|s| s.payload_manifest.as_deref())
    {
        let modes = scenario
            .as_ref()
            .context("payload scenario absent")?
            .files
            .iter()
            .map(|(path, file)| (path.clone(), file.mode))
            .collect();
        let aliases = payload::reconcile_base_links(&mut init, &modes, path)?;
        save_json(
            &out.join("payload-base-aliases.json"),
            &json!({
                "schema":"md-sim-payload-base-aliases/1",
                "manifest":hash_file(path)?,
                "base_regular_files_replaced_by_identical_payload_links":aliases
            }),
        )?;
    }
    let manifest = out.join("root-manifest.json");
    ensure!(
        init.iter().all(|input| input.object.size <= 128 << 20)
            && init.iter().map(|input| input.object.size).sum::<u64>() < 255 << 20,
        "private base image exceeds the bounded preparation profile"
    );
    save_json(&manifest, &init)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/prepare".into(), std::env::current_exe()?),
            ("/tools/cpio".into(), PathBuf::from("/usr/bin/cpio")),
        ],
    )?;
    for input in &init {
        if !inputs.iter().any(|i| i.guest_path == input.guest_path) {
            inputs.push(input.clone());
        }
    }
    add(&store, &mut inputs, &manifest, "/root-manifest.json")?;
    let prep = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        stdin_object: None,
        argv: vec!["/prepare".into(), "__prepare".into()],
        inputs,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 30,
            cpu_seconds: 20,
            memory_bytes: 2 * 1024 * 1024 * 1024,
            processes: 32,
            file_bytes: 128 * 1024 * 1024,
            trace_bytes: 128 * 1024 * 1024,
            tmp_bytes: 256 * 1024 * 1024,
        },
    };
    save_json(&out.join("preparation-spec.json"), &prep)?;
    let prepared = execute(&store, &prep, Arc::new(AtomicBool::new(false)))?;
    let first = hex::decode(&prepared.stdout)?;
    let second = hex::decode(&prepared.stderr)?;
    fs::write(out.join("preparation-stderr.log"), &second)?;
    let complete = prepared.exit_code == Some(0) && !prepared.trace_truncated;
    let image = out.join(if complete {
        "initramfs.cpio.gz"
    } else {
        "partial-initramfs.cpio.gz"
    });
    // Save the first stream before decoding so malformed exports retain evidence.
    fs::write(&image, &first)?;
    save_json(
        &out.join("preparation-result.json"),
        &json!({"status":prepared.status,"exit_code":prepared.exit_code,"trace_truncated":prepared.trace_truncated,"tools":prepared.tools,"wall_milliseconds":prepared.wall_milliseconds,"first_stream_identity":hash_file(&image)?,"export_format":"gzip or gzip split at64MiB with exact stderr V2 part2 marker; complete image identity in initramfs-export.json","first_stream_bytes":first.len(),"second_stream_bytes":second.len()}),
    )?;
    ensure!(
        prepared.exit_code == Some(0) && !prepared.trace_truncated,
        "initramfs preparation failed"
    );
    fs::write(&image, decode_export(&first, &second)?)?;
    save_json(
        &out.join("initramfs-export.json"),
        &json!({"complete_image":hash_file(&image)?,"first_stream_bytes":first.len(),"second_stream_bytes":second.len(),"gzip_integrity_checked":true,"uncompressed_limit":256*1024*1024}),
    )?;
    let image = if let Some(path) = scenario
        .as_ref()
        .and_then(|s| s.payload_manifest.as_deref())
    {
        let modes = scenario
            .as_ref()
            .context("payload scenario absent")?
            .files
            .iter()
            .map(|(path, file)| (path.clone(), file.mode))
            .collect();
        let (combined, extra) = payload::append(&store, &init, &modes, path, &image, out)?;
        // Preserve the exact isolated base preparation manifest. The combined
        // root manifest adds regular payload bytes; symlinks have a separate manifest.
        fs::copy(&manifest, out.join("base-root-manifest.json"))?;
        init.extend(extra);
        save_json(&manifest, &init)?;
        combined
    } else {
        image
    };
    ensure!(
        hash_file(Path::new(KERNEL))?.sha256 == KERNEL_SHA,
        "installed kernel identity changed"
    );
    if prepare_only {
        save_json(
            &out.join("prepared-image.json"),
            &json!({
                "schema":"md-sim-private-vm-preparation/1",
                "image":hash_file(&image)?,
                "kernel":hash_file(Path::new(KERNEL))?,
                "root_manifest":hash_file(&manifest)?,
                "builder":hash_file(&std::env::current_exe()?)?,
                "scope":"Prepared inputs only. No guest, TCP or firmware execution is inferred."
            }),
        )?;
        return Ok(());
    }
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/qemu".into(), PathBuf::from("/usr/bin/qemu-system-x86_64")),
            ("/runner".into(), std::env::current_exe()?),
        ],
    )?;
    add(&store, &mut inputs, Path::new(KERNEL), "/vmlinuz")?;
    add(&store, &mut inputs, &image, "/initramfs.cpio.gz")?;
    for name in ["bios-256k.bin", "linuxboot_dma.bin", "kvmvapic.bin"] {
        add(
            &store,
            &mut inputs,
            &Path::new("/usr/share/qemu").join(name),
            &format!("/qemu-data/{name}"),
        )?;
    }
    let nested = scenario
        .as_ref()
        .is_some_and(|s| s.environment.get("MD_SIM_NATIVE").map(String::as_str) == Some("1"));
    let captured_process = scenario
        .as_ref()
        .is_some_and(|s| s.environment.get("MD_SIM_PROCESS_VM").map(String::as_str) == Some("1"));
    let board_process = scenario.as_ref().is_some_and(|s| {
        s.environment
            .get("MD_SIM_BOARD_PROCESS")
            .map(String::as_str)
            == Some("1")
    });
    let extended_process = scenario.as_ref().is_some_and(|s| {
        s.environment
            .get("MD_SIM_BOARD_EXTENDED")
            .map(String::as_str)
            == Some("1")
    });
    let selected_browser = scenario.as_ref().is_some_and(|s| {
        s.environment
            .get("MD_SIM_BOARD_BROWSER")
            .map(String::as_str)
            == Some("1")
    });
    ensure!(
        !selected_browser || board_process,
        "selected native browser requires the captured board process"
    );
    ensure!(
        !extended_process || board_process,
        "extended process budget requires the captured board contract"
    );
    let long_process = scenario
        .as_ref()
        .is_some_and(|s| s.environment.get("MD_SIM_BOARD_LONG").map(String::as_str) == Some("1"));
    let slow_process = scenario
        .as_ref()
        .is_some_and(|s| s.environment.get("MD_SIM_BOARD_SLOW").map(String::as_str) == Some("1"));
    let long_boot = scenario.as_ref().is_some_and(|s| {
        s.environment
            .get("MD_SIM_BOARD_LONG_BOOT")
            .map(String::as_str)
            == Some("1")
    });
    ensure!(
        !long_boot || slow_process,
        "long boot budget requires slow process mode"
    );
    ensure!(
        !slow_process || long_process,
        "slow board budget requires long process mode"
    );
    ensure!(
        !long_process || extended_process,
        "long budget requires the extended board contract"
    );
    ensure!(
        !board_process || (captured_process && browser && !nested),
        "board transport requires captured process VM and payload"
    );
    let mut argv: Vec<String> = [
        "/runner",
        if selected_browser && long_boot {
            "__long_boot_browser_qemu"
        } else if long_boot {
            "__long_boot_process_qemu"
        } else if selected_browser {
            "__board_browser_qemu"
        } else if slow_process {
            "__slow_process_qemu"
        } else if long_process {
            "__long_process_qemu"
        } else if extended_process {
            "__extended_process_qemu"
        } else if captured_process {
            "__process_qemu"
        } else if nested && browser {
            "__native_browser_qemu"
        } else if nested {
            "__native_qemu"
        } else if browser {
            "__browser_qemu"
        } else {
            "__qemu"
        },
        "-machine",
        "pc",
        "-accel",
        if kvm {
            "kvm"
        } else if captured_process || (browser && nested) {
            "tcg,thread=multi,tb-size=256"
        } else if browser {
            "tcg,thread=single,tb-size=64"
        } else {
            "tcg,thread=single"
        },
        "-cpu",
        // Versioned QEMU model with the captured userspace's x86-64-v2
        // instructions. Refuse unavailable features instead of filtering them.
        if kvm { "Nehalem-v1,enforce" } else { "max" },
        "-m",
        if selected_browser {
            // Captured archive and worker/client stores, nested1GiB firmware,
            // and two Firefox profiles exceeded3GiB in SSH163. This separate
            // support profile remains bounded by8GiB outer address space.
            "6144M"
        } else if captured_process {
            "3072M"
        } else if browser && nested {
            // Captured root files (~916MiB), nested guest (1GiB), browser
            // leaf (768MiB), and both kernels need separate resident capacity.
            "4096M"
        } else if browser {
            "2048M"
        } else if nested {
            "1536M"
        } else if scenario.is_some() {
            "768M"
        } else {
            "256M"
        },
        "-smp",
        // The client browser and nested firmware emulator need independent
        // execution capacity. This changes only the support VM topology.
        if selected_browser {
            "4"
        } else if captured_process || (browser && nested) {
            "2"
        } else {
            "1"
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
        "stdio",
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
        if trace_stress {
            "console=ttyS0 rdinit=/init panic=-1 md_sim_private_guest=1 md_sim_trace_port=1 md_sim_trace_stress=1"
        } else if browser {
            // The captured browser runtime exceeds the default tmpfs rootfs size.
            // Its files are read-only; guest RAM and outer process limits still bind it.
            "console=ttyS0 rdinit=/init panic=-1 md_sim_private_guest=1 rootfstype=ramfs"
        } else {
            "console=ttyS0 rdinit=/init panic=-1 md_sim_private_guest=1"
        },
        "-sandbox",
        "on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    if trace_stress {
        argv.extend(
            [
                "-device",
                "virtio-serial-pci,id=trace-bus",
                "-chardev",
                "file,id=trace,path=/work/guest-trace.log",
                "-device",
                "virtserialport,bus=trace-bus.0,chardev=trace,name=md-sim.trace",
            ]
            .into_iter()
            .map(str::to_string),
        );
    }
    let spec = ProcessSpec {
        support_virtualization: kvm
            .then_some(md_sim_runtime::isolation::SupportVirtualization::Kvm),
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        stdin_object: None,
        argv,
        inputs,
        environment: if kvm {
            let mut environment = BTreeMap::from([("MD_SIM_SUPPORT_KVM".into(), "1".into())]);
            if trace_stress {
                environment.insert("MD_SIM_TRACE_STRESS".into(), "1".into());
            }
            environment
        } else {
            BTreeMap::new()
        },
        limits: Limits {
            wall_seconds: if long_boot {
                5400
            } else if selected_browser || slow_process {
                3000
            } else if long_process {
                2400
            } else if extended_process {
                1800
            } else if captured_process {
                900
            } else if nested && browser {
                confinement::NATIVE_BROWSER_VM_WALL_SECONDS
            } else if nested {
                660
            } else if browser {
                900
            } else if scenario.is_some() {
                300
            } else {
                120
            },
            // QEMU's two vCPUs can consume two CPU-seconds per wall second.
            // SSH board158 reached the old3600 CPU cap before its3000s wall
            // deadline. Match each bounded support-VM window; the selected
            // inner firmware process keeps its separate original budget.
            cpu_seconds: if selected_browser && long_boot {
                21600
            } else if long_boot {
                10800
            } else if selected_browser {
                12000
            } else if slow_process {
                6000
            } else if long_process {
                4800
            } else if extended_process {
                3600
            } else if captured_process {
                1440
            } else if nested && browser {
                confinement::NATIVE_BROWSER_VM_CPU_SECONDS
            } else if nested {
                480
            } else if browser {
                720
            } else if scenario.is_some() {
                120
            } else {
                90
            },
            memory_bytes: if selected_browser {
                8 * 1024 * 1024 * 1024
            } else if captured_process {
                5 * 1024 * 1024 * 1024
            } else if nested && browser {
                confinement::NATIVE_BROWSER_VM_ADDRESS_BYTES
            } else {
                3 * 1024 * 1024 * 1024
            },
            processes: 64,
            file_bytes: 64 * 1024 * 1024,
            // Retain both SD snapshots and all HTTP/reset evidence.
            trace_bytes: if trace_stress {
                16
            } else if board_process {
                128
            } else if captured_process {
                64
            } else if nested {
                8
            } else {
                4
            } * 1024
                * 1024,
            tmp_bytes: 128 * 1024 * 1024,
        },
    };
    save_json(&out.join("process-spec.json"), &spec)?;
    save_json(
        &out.join("topology.json"),
        &json!({"host":"local restricted environment","backend":"QEMU x86 system under strict ProcessSpec isolation","guest_kernel":hash_file(Path::new(KERNEL))?,"guest_initramfs":hash_file(&image)?,"root_filesystem":if browser {"explicit ramfs for captured browser payload; bounded by the declared guest RAM and outer process limits"} else {"kernel default rootfs"},"guest_ram":spec.argv.windows(2).find(|pair|pair[0]=="-m").context("guest RAM argument absent")?[1],"guest_cpus":spec.argv.windows(2).find(|pair|pair[0]=="-smp").context("guest CPU argument absent")?[1],"accelerator":spec.argv.windows(2).find(|pair|pair[0]=="-accel").context("guest accelerator argument absent")?[1],"process_limits":spec.limits,"trace_transport":if trace_stress {json!({"guest_userspace":"md-sim.trace virtserialport","kernel_console":"UART to backend.log","stress_bytes":trace_port::PAYLOAD_BYTES,"source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("isolated_network_vm/trace_port.rs")),"port_opener_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/worker_virtual_port.rs"))})} else {json!({"guest_userspace":"UART","kernel_console":"UART"})},"network":"outer VM starts with loopback only; QEMU has no virtual network device and no host network backend; scenario may add internal guest namespaces","client":"Rust guest TCP prerequisite; scenario report identifies further clients","worker":"Rust guest TCP prerequisite; scenario report identifies further workers","external_physical_host":false,"simulator_worker_exercised":false,"ssh_tls_exercised":false,"source":{"sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("isolated_network_vm.rs")),"size":include_bytes!("isolated_network_vm.rs").len(),"binding":"compiled include_bytes source"}}),
    )?;
    let traces = TraceFiles {
        stdout: fs::File::create(out.join("guest.log"))?,
        stderr: fs::File::create(out.join("backend.log"))?,
    };
    let execute_vm = if kvm {
        md_sim_runtime::isolation::execute_support_vm
    } else {
        execute_with_trace
    };
    let result = execute_vm(
        &store,
        &spec,
        Arc::new(AtomicBool::new(false)),
        Some(traces),
    )?;
    let stdout = hex::decode(&result.stdout)?;
    let stderr = hex::decode(&result.stderr)?;
    save_json(&out.join("result.json"), &result)?;
    ensure!(
        result.status == "exited"
            && result.exit_code == Some(0)
            && !result.trace_truncated
            && (trace_stress || result.stderr.is_empty()),
        "guest did not exit cleanly with complete traces"
    );
    let tcp = verify_guest_log(&stdout)?;
    let trace_stress_evidence = if trace_stress {
        Some(trace_port::verify(&stdout, &stderr)?)
    } else {
        None
    };
    if scenario.is_some() {
        ensure!(
            std::str::from_utf8(&stdout)?
                .lines()
                .filter(|s| s.trim_end_matches('\r')
                    == "MD_SIM_VM_SCENARIO selected guest command exited 0")
                .count()
                == 1,
            "scenario did not report one successful exit"
        );
    }
    save_json(
        &out.join("assessment.json"),
        &json!({"guest_boot_and_tcp_probe":true,"actual_tcp_record":tcp,"trace_stress":trace_stress_evidence,"actual_firmware_gate":false,"worker_transport_gate":false,"external_host_gate":false,"ssh_tls_gate":false,"guest_log":hash_file(&out.join("guest.log"))?,"backend_log":hash_file(&out.join("backend.log"))?,"process_spec":hash_file(&out.join("process-spec.json"))?,"process_result":hash_file(&out.join("result.json"))?}),
    )?;
    println!(
        "{}",
        json!({"output":out,"guest_boot_and_tcp_probe":true,"external_host":false})
    );
    Ok(())
}

fn verify_guest_log(bytes: &[u8]) -> Result<serde_json::Value> {
    let text = std::str::from_utf8(bytes)?;
    let lines = text
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .collect::<Vec<_>>();
    let marker = |wanted: &str| -> Result<usize> {
        let matches = lines
            .iter()
            .enumerate()
            .filter(|(_, line)| **line == wanted)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        ensure!(
            matches.len() == 1,
            "missing or duplicate exact guest marker: {wanted}"
        );
        Ok(matches[0])
    };
    let start = marker("MD_SIM_VM_INIT actual Rust PID 1 started")?;
    let kernel = marker("MD_SIM_VM_KERNEL 7.2.9-arch1-1")?;
    let finish = marker("MD_SIM_VM_COMPLETE isolated guest loopback probe passed")?;
    let records = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            line.strip_prefix("MD_SIM_VM_TCP ")
                .map(|data| (index, data))
        })
        .collect::<Vec<_>>();
    ensure!(
        records.len() == 1 && start < kernel && kernel < records[0].0 && records[0].0 < finish,
        "guest evidence order or TCP record count differs"
    );
    let record: serde_json::Value = serde_json::from_str(records[0].1)?;
    ensure!(
        record["actual_roundtrip"] == true
            && record["external_network"] == false
            && record["interfaces"] == json!(["lo"]),
        "guest topology or roundtrip differs"
    );
    ensure!(
        hex::decode(
            record["challenge_hex"]
                .as_str()
                .context("missing challenge")?
        )?
        .len()
            == 32,
        "guest challenge length differs"
    );
    for field in ["listener", "client"] {
        let address: std::net::SocketAddr = record[field]
            .as_str()
            .context("missing TCP address")?
            .parse()?;
        ensure!(
            address.ip() == std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
                && address.port() > 0,
            "TCP endpoint is not guest loopback"
        );
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evidence_requires_ordered_unique_markers_and_real_tcp_fields() {
        let record = json!({"actual_roundtrip":true,"external_network":false,"interfaces":["lo"],"challenge_hex":"01".repeat(32),"listener":"127.0.0.1:10000","client":"127.0.0.1:10001"});
        let log=format!("MD_SIM_VM_INIT actual Rust PID 1 started\r\nMD_SIM_VM_KERNEL 7.2.9-arch1-1\r\nMD_SIM_VM_TCP {record}\r\nMD_SIM_VM_COMPLETE isolated guest loopback probe passed\r\n");
        assert!(verify_guest_log(log.as_bytes()).is_ok());
        assert!(verify_guest_log(
            log.replace(
                "MD_SIM_VM_INIT actual Rust PID 1 started\r\nMD_SIM_VM_KERNEL 7.2.9-arch1-1",
                "MD_SIM_VM_KERNEL 7.2.9-arch1-1\r\nMD_SIM_VM_INIT actual Rust PID 1 started"
            )
            .as_bytes()
        )
        .is_err());
        assert!(verify_guest_log(format!("{log}MD_SIM_VM_TCP {record}\n").as_bytes()).is_err());
        assert!(
            verify_guest_log(log.replace("127.0.0.1:10000", "192.0.2.1:10000").as_bytes()).is_err()
        );
        assert!(verify_guest_log(log.replace("[\"lo\"]", "[\"lo\",\"eth0\"]").as_bytes()).is_err());
        assert!(verify_guest_log(
            log.replace(
                "isolated guest loopback probe passed",
                "isolated guest loopback probe passed extra"
            )
            .as_bytes()
        )
        .is_err());
    }
    #[test]
    fn two_stream_export_checks_framing_crc_and_complete_gzip() {
        let mut bytes = vec![0u8; EXPORT_HALF];
        let mut state = 0x1324587au32;
        for byte in &mut bytes {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *byte = state as u8;
        }
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        gzip.write_all(&bytes).unwrap();
        let compressed = gzip.finish().unwrap();
        assert!(compressed.len() > EXPORT_HALF);
        let mut second = EXPORT_PART2.to_vec();
        second.extend_from_slice(&compressed[EXPORT_HALF..]);
        assert_eq!(
            decode_export(&compressed[..EXPORT_HALF], &second).unwrap(),
            compressed
        );
        assert!(decode_export(&compressed[..EXPORT_HALF], &second[1..]).is_err());
        assert!(decode_export(&compressed[..EXPORT_HALF], &[]).is_err());
        let last = second.len() - 1;
        second[last] ^= 1;
        assert!(decode_export(&compressed[..EXPORT_HALF], &second).is_err());
        second[last] ^= 1;
        second.push(0);
        assert!(decode_export(&compressed[..EXPORT_HALF], &second).is_err());
    }
}

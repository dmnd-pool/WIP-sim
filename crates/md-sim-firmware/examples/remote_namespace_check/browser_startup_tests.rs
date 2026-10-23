//! Captured selected-client Firefox startup. The worker, firmware and native
//! dashboard are absent. The production client role builds the isolation.
use super::*;
use std::process::Stdio;

fn require_guest() -> Result<()> {
    ensure!(
        unsafe { libc::geteuid() } == 0
            && fs::read_to_string("/proc/cmdline")?
                .split_whitespace()
                .any(|word| word == "md_sim_private_guest=1"),
        "private startup VM required"
    );
    Ok(())
}

#[test]
#[ignore = "requires captured Firefox, production helper and private Linux VM"]
fn actual_selected_client_starts_two_fresh_profiles() -> Result<()> {
    require_guest()?;
    ensure!(
        board_browser::enabled()
            && board_browser::carrier_enabled()
            && worker_broker::enabled()
            && carrier_dashboard::enabled()
            && !shared_pool::enabled()
            && !board_mode()
            && !native_mode(),
        "explicit standalone startup fixture selection required"
    );
    for path in [
        "/run/netns",
        "/work/keys",
        "/work/client-root",
        "/broker-inputs",
    ] {
        fs::create_dir_all(path)?;
    }
    fs::write("/etc/passwd", "root:x:0:0:root:/work:/bin/false\n")?;
    fs::write("/etc/group", "root:x:0:\n")?;
    fs::write(
        "/etc/nsswitch.conf",
        "passwd: files\ngroup: files\nhosts: files\n",
    )?;
    fs::write("/etc/hosts", "127.0.0.1 localhost\n")?;
    if !Path::new("/var/run").exists() {
        std::os::unix::fs::symlink("/run", "/var/run")?;
    }
    let spec = fs::read("/execution-spec.json")?;
    fs::write("/work/execution-spec.json", &spec)?;
    fs::set_permissions(
        "/work/execution-spec.json",
        fs::Permissions::from_mode(0o444),
    )?;
    let context = hash_file(Path::new("/work/execution-spec.json"))?;
    let credentials = credentials::create(
        Path::new("/usr/bin/openssl"),
        Path::new("/work/credentials"),
        WORKER.parse()?,
        &context.sha256,
    )?;
    // Role mount inputs are deliberately inert. No SSH service or worker is
    // started, and these bytes cannot be mistaken for successful SSH evidence.
    for name in ["clientkey", "clientkey.pub", "known_hosts"] {
        fs::write(
            format!("/work/keys/{name}"),
            b"startup fixture; not an SSH key\n",
        )?;
    }
    invoke(
        "/usr/bin/zstd",
        &["--decompress", "/veth.ko.zst", "-o", "/work/veth.ko"],
    )?;
    invoke("/usr/bin/insmod", &["/work/veth.ko"])?;
    invoke("/usr/bin/ip", &["netns", "add", "md-client"])?;
    invoke(
        "/usr/bin/ip",
        &[
            "link", "add", "worker0", "type", "veth", "peer", "name", "client0",
        ],
    )?;
    invoke(
        "/usr/bin/ip",
        &["link", "set", "client0", "netns", "md-client"],
    )?;
    invoke(
        "/usr/bin/ip",
        &[
            "-n",
            "md-client",
            "address",
            "add",
            "192.0.2.2/29",
            "dev",
            "client0",
        ],
    )?;
    invoke(
        "/usr/bin/ip",
        &["-n", "md-client", "link", "set", "client0", "up"],
    )?;
    invoke(
        "/usr/bin/ip",
        &["-n", "md-client", "link", "set", "lo", "up"],
    )?;
    let role = role_command("client")?;
    let args = role.get_args().collect::<Vec<_>>();
    ensure!(
        args.ends_with(&[
            std::ffi::OsStr::new("/namespace-check"),
            std::ffi::OsStr::new("client")
        ]),
        "production client role command changed"
    );
    // Keep every production role option and mount. Only replace its final
    // program entry with this test binary's explicit guest-only helper.
    let mut command = Command::new(role.get_program());
    command
        .args(&args[..args.len() - 2])
        .args([
            "/namespace-check",
            "browser_startup_tests::selected_startup_role",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .stdin(Stdio::null())
        .stdout(fs::File::create("/work/role.stdout")?)
        .stderr(fs::File::create("/work/role.stderr")?);
    let mut child = Owned(command.spawn()?);
    let deadline = Instant::now() + Duration::from_secs(650);
    let status = loop {
        if let Some(status) = child.0.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.0.kill()?;
            child.0.wait()?;
            break None;
        }
        thread::sleep(Duration::from_millis(100));
    };
    for name in ["role.stdout", "role.stderr"] {
        let bytes = read_evidence(&Path::new("/work").join(name), 4 << 20)?;
        println!(
            "MD_SIM_STARTUP_ROLE_LOG {}",
            json!({"name":name,
            "object":hash_file(&Path::new("/work").join(name))?,"hex":hex::encode(bytes)})
        );
    }
    invoke("/usr/bin/ip", &["netns", "delete", "md-client"])?;
    let report_path = Path::new("/work/client-root/browser/startup-result.json");
    let report = if report_path.exists() {
        serde_json::from_slice::<Value>(&read_evidence(report_path, 4 << 20)?)?
    } else {
        Value::Null
    };
    println!(
        "MD_SIM_BROWSER_STARTUP_COMPONENT {}",
        json!({
        "schema":"md-sim-selected-browser-startup-component/1",
        "passed":status.is_some_and(|s|s.success()) && report["passed"] == true,
        "firmware_gate":false,"native_dashboard_gate":false,"credentials":credentials,
        "status":status.and_then(|s|s.code()),"helper":hash_file(Path::new("/s9-browser-check"))?,
        "namespace_helper":hash_file(&std::env::current_exe()?)?,"report":report})
    );
    ensure!(
        status.is_some_and(|s| s.success()) && report["passed"] == true,
        "actual selected-client startup failed"
    );
    ensure!(
        report["firmware_gate"] == false && report["native_dashboard_gate"] == false,
        "startup fixture granted an unrelated gate"
    );
    Ok(())
}

#[test]
#[ignore = "guest-only child entry of the actual selected-client startup fixture"]
fn selected_startup_role() -> Result<()> {
    ensure!(
        unsafe { libc::geteuid() } == 0 && !Path::new("/proc/self").exists(),
        "production role must supply a new empty proc mount point"
    );
    ensure!(
        unsafe {
            libc::mount(
                c"proc".as_ptr(),
                c"/proc".as_ptr(),
                c"proc".as_ptr(),
                libc::MS_NOSUID | libc::MS_NOEXEC | libc::MS_NODEV,
                std::ptr::null(),
            )
        } == 0,
        "startup private proc mount failed: {}",
        std::io::Error::last_os_error()
    );
    require_guest()?;
    let isolation = namespace_evidence("startup client")?;
    ensure!(
        !Path::new("/credentials/server.key").exists()
            && !Path::new("/work/credentials/ca.key").exists()
            && !Path::new("/work/client-root").exists(),
        "startup role has parent credentials or work"
    );
    println!("MD_SIM_STARTUP_CLIENT_ISOLATION {isolation}");
    let status = Command::new("/s9-browser-check")
        .arg("--selected-startup")
        .env_clear()
        .env("PATH", "/usr/bin")
        .stdin(Stdio::null())
        .status()?;
    ensure!(
        status.success(),
        "production selected startup helper failed: {status}"
    );
    Ok(())
}

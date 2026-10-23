//! Real delayed nginx and owned-child readiness checks in a captured VM.
//! No firmware or selected native dashboard pass follows from this fixture.
use super::*;
use std::{
    net::TcpListener,
    os::unix::process::{CommandExt, ExitStatusExt},
};

fn private_vm(uid: u32) -> Result<()> {
    ensure!(
        unsafe { libc::geteuid() } == uid
            && fs::read_to_string("/proc/cmdline")?
                .split_whitespace()
                .any(|value| value == "md_sim_private_guest=1"),
        "private readiness VM required"
    );
    Ok(())
}

#[test]
#[ignore = "owned child entry, executed only by the VM readiness regression"]
fn vm_nginx_child_entry() -> Result<()> {
    private_vm(1000)?;
    ensure!(
        unsafe { libc::getegid() } == 1000,
        "actual fixture GID differs"
    );
    let mode = std::env::var("MD_SIM_READINESS_CHILD")?;
    let root = PathBuf::from(std::env::var("MD_SIM_READINESS_ROOT")?);
    fs::write(root.join("child.pid.tmp"), std::process::id().to_string())?;
    fs::rename(root.join("child.pid.tmp"), root.join("child.pid"))?;
    match mode.as_str() {
        "nginx" => {
            let deadline = Instant::now() + Duration::from_secs(15);
            while !root.join("release-nginx").exists() {
                ensure!(
                    Instant::now() < deadline,
                    "delayed nginx fixture gate deadline"
                );
                std::thread::sleep(Duration::from_millis(5));
            }
            std::thread::sleep(Duration::from_millis(250));
            Err(Command::new("/usr/bin/nginx")
                .args(["-p", root.to_str().context("fixture root path")?, "-c"])
                .arg(root.join("nginx.conf"))
                .exec()
                .into())
        }
        "no-listener" => {
            let mut byte = [0];
            ensure!(
                std::io::stdin().read(&mut byte)? == 0,
                "fixture expected owned stdin closure"
            );
            Ok(())
        }
        "exit" => Ok(()),
        _ => anyhow::bail!("unknown readiness child fixture"),
    }
}

fn child(root: &Path, mode: &str) -> Result<OwnedSshChild> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args([
            "remote_carrier::readiness_tests::vm_nginx_child_entry",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env_clear()
        .env("MD_SIM_READINESS_CHILD", mode)
        .env("MD_SIM_READINESS_ROOT", root)
        .uid(1000)
        .gid(1000)
        .stdin(Stdio::piped())
        .stdout(File::create_new(root.join("child.stdout"))?)
        .stderr(File::create_new(root.join("child.stderr"))?);
    OwnedSshChild::spawn_service(&mut command, 20, 1 << 20)
}

fn read_json(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&read(path, 1 << 20)?)?)
}

fn child_pid(root: &Path) -> Result<i32> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !root.join("child.pid").exists() {
        ensure!(Instant::now() < deadline, "actual child startup deadline");
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(fs::read_to_string(root.join("child.pid"))?.parse()?)
}

fn absent(pid: i32) -> bool {
    (unsafe { libc::kill(pid, 0) }) == -1
        && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
}

struct FailureLogs {
    root: PathBuf,
    passed: bool,
}
impl Drop for FailureLogs {
    fn drop(&mut self) {
        if self.passed {
            return;
        }
        let mut logs = BTreeMap::new();
        for directory in ["nginx", "refused", "exited"] {
            for name in [
                "child.stdout",
                "child.stderr",
                "error.log",
                "listener-readiness.json",
            ] {
                let relative = format!("{directory}/{name}");
                let path = self.root.join(&relative);
                if path.exists() {
                    let bytes = read(&path, 1 << 20);
                    logs.insert(
                        relative,
                        match bytes {
                            Ok(bytes) => {
                                json!({"bytes":bytes.len(),"text":String::from_utf8_lossy(&bytes)})
                            }
                            Err(error) => json!({"read_error":format!("{error:#}")}),
                        },
                    );
                }
            }
        }
        println!(
            "MD_SIM_TLS_READINESS_FIXTURE_FAILURE {}",
            json!({"logs":logs,
            "scope":"Bounded component fixture failure diagnostics; no readiness pass"})
        );
    }
}

fn worker_owner(path: &Path) -> Result<()> {
    std::os::unix::fs::chown(path, Some(1000), Some(1000))?;
    Ok(())
}

#[test]
#[ignore = "requires real nginx, OpenSSL, curl and sockets in the isolated VM"]
fn vm_delayed_nginx_and_failed_listener_lifecycle() -> Result<()> {
    private_vm(0)?;
    let root = tempfile::tempdir_in("/work")?;
    let mut failure_logs = FailureLogs {
        root: root.path().into(),
        passed: false,
    };
    worker_owner(root.path())?;
    let credentials = root.path().join("credentials");
    let ip = "127.0.0.1".parse()?;
    crate::credentials::create(
        Path::new("/usr/bin/openssl"),
        &credentials,
        ip,
        &"a".repeat(64),
    )?;
    worker_owner(&credentials)?;
    for entry in fs::read_dir(&credentials)? {
        let path = entry?.path();
        ensure!(path.is_file(), "unexpected credential fixture entry");
        worker_owner(&path)?;
    }
    let reserved = TcpListener::bind((ip, 0))?;
    let address = reserved.local_addr()?;
    let nginx = root.path().join("nginx");
    fs::create_dir(&nginx)?;
    worker_owner(&nginx)?;
    fs::write(nginx.join("nginx.conf"),format!(
        "daemon off; master_process off; worker_processes 1;\npid {dir}/nginx.pid;\nerror_log {dir}/error.log warn;\nevents {{ worker_connections 16; }}\nhttp {{ access_log {dir}/access.log; client_body_temp_path {dir}/body; proxy_temp_path {dir}/proxy; fastcgi_temp_path {dir}/fastcgi; uwsgi_temp_path {dir}/uwsgi; scgi_temp_path {dir}/scgi; server {{ listen {address} ssl; ssl_certificate {cert}/server.crt; ssl_certificate_key {cert}/server.key; ssl_client_certificate {cert}/ca.crt; ssl_verify_client on; ssl_protocols TLSv1.2 TLSv1.3; location / {{ return 200 'MD_SIM_TLS_READINESS_COMPONENT'; }} }} }}\n",
        dir=nginx.display(),cert=credentials.display()))?;
    drop(reserved);
    let mut delayed = child(&nginx, "nginx")?;
    let nginx_pid = child_pid(&nginx)?;
    let premature = TcpStream::connect_timeout(&address, Duration::from_millis(100));
    ensure!(
        premature.is_err(),
        "one-shot fixture unexpectedly reached gated nginx"
    );
    fs::write(nginx.join("release-nginx"), b"start actual nginx")?;
    let started = Instant::now();
    let ready = wait_listener(
        &delayed,
        address,
        started,
        started + Duration::from_secs(10),
        &nginx.join("listener-readiness.json"),
    );
    ready?;
    let readiness = read_json(&nginx.join("listener-readiness.json"))?;
    let attempts = readiness["attempts"]
        .as_array()
        .context("readiness attempts absent")?;
    ensure!(
        attempts.len() > 1
            && attempts
                .iter()
                .any(|a| a["raw_os_error"] == libc::ECONNREFUSED)
            && attempts.last().context("last readiness attempt")?["connected"] == true,
        "delayed actual listener did not exercise refusal then success"
    );
    let mut curl = Command::new("/usr/bin/curl");
    curl.env_clear()
        .args([
            "--noproxy",
            "*",
            "--silent",
            "--show-error",
            "--fail",
            "--proto",
            "=https",
            "--connect-timeout",
            "2",
            "--max-time",
            "5",
            "--cacert",
        ])
        .arg(credentials.join("ca.crt"))
        .arg("--cert")
        .arg(credentials.join("client.crt"))
        .arg("--key")
        .arg(credentials.join("client.key"))
        .arg(format!("https://{address}/"));
    let tls = curl.output()?;
    ensure!(
        tls.status.success() && tls.stdout == b"MD_SIM_TLS_READINESS_COMPONENT",
        "real nginx authenticated TLS failed: {}",
        String::from_utf8_lossy(&tls.stderr)
    );
    delayed.finish()?;
    ensure!(
        absent(nginx_pid)
            && TcpStream::connect_timeout(&address, Duration::from_millis(100)).is_err(),
        "owned nginx or listener survived cleanup"
    );

    let refused = root.path().join("refused");
    fs::create_dir(&refused)?;
    worker_owner(&refused)?;
    let refusing = child(&refused, "no-listener")?;
    let refused_pid = child_pid(&refused)?;
    // The publication phase has already spent29.75s. Listener readiness must
    // use the remaining250ms, not create a second30s startup budget.
    let began = Instant::now();
    let started = began - Duration::from_millis(29_750);
    let failure = wait_listener(
        &refusing,
        address,
        started,
        started + Duration::from_secs(30),
        &refused.join("listener-readiness.json"),
    );
    ensure!(
        failure.is_err()
            && began.elapsed() >= Duration::from_millis(250)
            && began.elapsed() < Duration::from_secs(2),
        "listener reset or exceeded the remaining startup budget"
    );
    let refusal = read_json(&refused.join("listener-readiness.json"))?;
    ensure!(
        refusal["reachable"] == false
            && refusal["startup_budget_milliseconds"] == 30_000
            && refusal["attempts"]
                .as_array()
                .context("refusal attempts")?
                .iter()
                .all(|a| a["raw_os_error"] == libc::ECONNREFUSED),
        "exact refusal evidence differs"
    );
    let (_, _, mut spec) = crate::carrier_gateway::tests::gateway_fixture()?;
    spec.listen = address;
    let service = RemoteCarrierGateway {
        child: refusing,
        directory: refused.clone(),
        spec,
        manifest: Value::Null,
        stopped: false,
    };
    drop(service); // Exercise the same RAII cleanup as start()'s failure path.
    ensure!(
        absent(refused_pid) && read_json(&refused.join("stopped.json"))?["acknowledged"] == true,
        "failed-start Drop did not close stdin and reap its owned child"
    );

    let exited = root.path().join("exited");
    fs::create_dir(&exited)?;
    worker_owner(&exited)?;
    let mut ended = child(&exited, "exit")?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !ended.exited()? {
        ensure!(Instant::now() < deadline, "child exit fixture deadline");
        std::thread::sleep(Duration::from_millis(10));
    }
    let occupied = TcpListener::bind(address)?;
    let started = Instant::now();
    ensure!(
        wait_listener(
            &ended,
            address,
            started,
            started + Duration::from_secs(2),
            &exited.join("listener-readiness.json")
        )
        .is_err(),
        "unrelated reachable port hid the owned child's exit"
    );
    let dead = read_json(&exited.join("listener-readiness.json"))?;
    ensure!(
        dead["attempts"] == json!([])
            && dead["error"]
                .as_str()
                .is_some_and(|s| s.contains("child exited")),
        "dead child readiness evidence differs"
    );
    ensure!(
        ended.finish()?.signal().is_none(),
        "already exited fixture child was signalled"
    );
    drop(occupied);
    println!(
        "MD_SIM_TLS_READINESS_FIXTURE {}",
        json!({
        "scope":"Actual delayed nginx and authenticated component TLS; no firmware or native dashboard claim",
        "delayed":readiness,"remaining_budget_refusal":refusal,"exited_child":dead,
        "tls_body":String::from_utf8(tls.stdout)?,"failed_start_drop_reaped":true,
        "nginx":md_sim_runtime::store::hash_file(Path::new("/usr/bin/nginx"))?,
        "test_helper":md_sim_runtime::store::hash_file(&std::env::current_exe()?)?})
    );
    failure_logs.passed = true;
    Ok(())
}

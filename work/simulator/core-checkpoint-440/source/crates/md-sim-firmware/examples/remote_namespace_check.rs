//! Real remote transport in private guest namespaces. Explicit modes select
//! protocol fixtures, captured native services, or an outside-VM native worker
//! with an independent pool. The HTTP fixture is never a miner dashboard.
use anyhow::{bail, ensure, Context, Result};
use md_sim_access::{
    credentials,
    gateway::{BrowserPolicy, Gateway, GatewaySpec},
};
use md_sim_coordinator::ssh_transport::{SshConfig, SshTunnel};
use md_sim_runtime::store::{hash_file, ObjectId};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::{fs::PermissionsExt, io::AsRawFd, process::CommandExt},
    path::Path,
    process::{Child, Command, Stdio},
    sync::OnceLock,
    thread,
    time::{Duration, Instant},
};

const WORKER: &str = "192.0.2.1";
const CLIENT: &str = "192.0.2.2";
#[path = "remote_namespace_check/board.rs"]
mod board;
#[path = "remote_namespace_check/board_browser.rs"]
mod board_browser;
#[allow(dead_code)] // Browser child uses the target response checks.
#[path = "support/s9_browser_target.rs"]
mod board_browser_target;
#[path = "remote_namespace_check/board_dashboard.rs"]
mod board_dashboard;
#[allow(dead_code)] // This client reuses the selected-endpoint native probe.
#[path = "support/s9_dashboard_probe.rs"]
mod board_dashboard_probe;
#[allow(dead_code)]
#[path = "support/s9_boot_storage.rs"]
mod boot_storage;
#[path = "remote_namespace_check/browser.rs"]
mod browser;
#[path = "remote_namespace_check/carrier_artifacts.rs"]
mod carrier_artifacts;
#[path = "remote_namespace_check/carrier_dashboard.rs"]
mod carrier_dashboard;
#[path = "remote_namespace_check/carrier_dashboard_client.rs"]
mod carrier_dashboard_client;
#[allow(dead_code)] // This guest exports; the outer receiver imports the same codec.
#[path = "support/worker_channel_artifacts.rs"]
mod channel_artifacts;
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "support/dashboard_transport_report.rs"]
mod dashboard_transport_report;
#[path = "support/worker_device_broker.rs"]
mod device_broker;
#[path = "support/worker_device_transport.rs"]
mod device_transport;
#[path = "support/s9_execution_report.rs"]
mod execution_report;
#[path = "remote_namespace_check/native.rs"]
mod native;
#[path = "support/native_browser_budget.rs"]
mod native_browser_budget;
#[allow(dead_code)]
#[path = "support/s9_pool_input.rs"]
mod pool_input;
#[path = "support/s9_pool_report.rs"]
mod pool_report;
#[path = "remote_namespace_check/pool_service.rs"]
mod pool_service;
#[allow(dead_code)]
#[path = "support/pool_vm.rs"]
mod pool_vm;
#[path = "support/worker_retention_frame.rs"]
mod retention_frame;
#[allow(dead_code)] // Namespace integration reads the common shared manifest.
#[path = "support/s9_shared_pool.rs"]
mod shared_manifest;
#[path = "remote_namespace_check/shared_pool.rs"]
mod shared_pool;
#[path = "remote_namespace_check/shared_progress.rs"]
mod shared_progress;
#[allow(dead_code)] // Both host import and guest receipt paths share these types.
#[path = "support/worker_shared_retention.rs"]
mod shared_retention;
#[path = "remote_namespace_check/shared_run.rs"]
mod shared_run;
#[allow(dead_code)]
#[path = "support/ssh_artifact_ack_loss.rs"]
mod ssh_artifact_ack_loss;
#[path = "support/ssh_one_reply.rs"]
mod ssh_one_reply;
#[path = "support/worker_virtual_port.rs"]
mod virtual_port;
#[path = "remote_namespace_check/worker_broker.rs"]
mod worker_broker;
fn board_mode() -> bool {
    std::env::var("MD_SIM_BOARD_PROCESS").as_deref() == Ok("1")
}
fn native_mode() -> bool {
    std::env::var("MD_SIM_NATIVE").as_deref() == Ok("1")
}
fn browser_mode() -> bool {
    std::env::var("MD_SIM_NATIVE_BROWSER").as_deref() == Ok("1")
}
static EXECUTION: OnceLock<ObjectId> = OnceLock::new();
fn run_id() -> &'static str {
    &EXECUTION
        .get()
        .expect("initialized component execution identity")
        .sha256
}
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn invoke(program: &str, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()?;
    ensure!(
        output.status.success(),
        "{program} {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}
fn start(program: &str, args: &[&str], log: &str) -> Result<Owned> {
    let file = fs::File::create(log)?;
    Ok(Owned(
        Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(file.try_clone()?)
            .stderr(file)
            .spawn()?,
    ))
}
fn namespace_evidence(role: &str) -> Result<Value> {
    let observed: u32 = fs::read_to_string("/proc/self/stat")?
        .split_whitespace()
        .next()
        .context("proc PID absent")?
        .parse()?;
    ensure!(observed == std::process::id(), "role proc PID differs");
    if role != "VM orchestrator" {
        for process in fs::read_dir("/proc")? {
            let process = process?;
            if !process
                .file_name()
                .as_encoded_bytes()
                .iter()
                .all(u8::is_ascii_digit)
            {
                continue;
            }
            for outside in [
                "work/keys/clientkey",
                "work/server-root",
                "work/client-root",
            ] {
                ensure!(
                    !process.path().join("root").join(outside).exists(),
                    "role can reach the orchestrator through proc"
                );
            }
        }
    }
    let mut namespaces = serde_json::Map::new();
    for name in ["net", "mnt", "pid", "uts", "ipc"] {
        namespaces.insert(
            name.into(),
            json!(fs::read_link(format!("/proc/self/ns/{name}"))?),
        );
    }
    let interfaces: Value =
        serde_json::from_slice(&invoke("/usr/bin/ip", &["-json", "link", "show"])?)?;
    let routes = String::from_utf8(invoke("/usr/bin/ip", &["route", "show"])?)?;
    ensure!(
        !routes.contains("default"),
        "environment has a default route"
    );
    Ok(
        json!({"role":role,"namespaces":namespaces,"interfaces":interfaces,"routes":routes,"uid":unsafe{libc::geteuid()},"root_has_only_captured_inputs":true,"external_route":false,"proc_pid_matches":true}),
    )
}
fn serve_one(mut socket: TcpStream) -> Result<()> {
    let run = run_id();
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    socket.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut input = Vec::new();
    let header_end = loop {
        let mut chunk = [0u8; 4096];
        let count = socket.read(&mut chunk)?;
        if count == 0 {
            return Ok(());
        }
        input.extend_from_slice(&chunk[..count]);
        ensure!(input.len() <= 128 * 1024, "fixture request exceeds bound");
        if let Some(at) = input.windows(4).position(|b| b == b"\r\n\r\n") {
            break at + 4;
        }
    };
    let header = String::from_utf8(input[..header_end].to_vec())?;
    let path = header
        .split_whitespace()
        .nth(1)
        .context("fixture request path missing")?;
    let length = header
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length:")
                .map(|v| v.trim().parse::<usize>())
        })
        .transpose()?
        .unwrap_or(0);
    ensure!(length <= 64 * 1024, "fixture upload too large");
    while input.len() < header_end + length {
        let mut chunk = [0; 4096];
        let n = socket.read(&mut chunk)?;
        ensure!(n != 0, "short fixture upload");
        input.extend_from_slice(&chunk[..n]);
    }
    if path == "/stream" || path == "/events" {
        let (kind, parts) = if path == "/events" {
            (
                "text/event-stream",
                ["data: first\n\n", "data: second\n\n", "data: third\n\n"],
            )
        } else {
            ("text/plain", ["first\n", "second\n", "third\n"])
        };
        write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n")?;
        for part in parts {
            write!(socket, "{:x}\r\n{part}\r\n", part.len())?;
            socket.flush()?;
            thread::sleep(Duration::from_millis(100));
        }
        socket.write_all(b"0\r\n\r\n")?;
        return Ok(());
    }
    if path == "/ws" {
        ensure!(
            header
                .lines()
                .any(|l| l.eq_ignore_ascii_case("Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==")),
            "fixture websocket challenge differs from RFC6455 vector"
        );
        socket.write_all(b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n")?;
        socket.write_all(&[0x81, run.len() as u8])?;
        socket.write_all(run.as_bytes())?;
        let mut frame = [0u8; 2];
        socket.read_exact(&mut frame)?;
        ensure!(
            frame[0] == 0x81 && frame[1] & 0x80 != 0 && frame[1] & 0x7f <= 125,
            "invalid fixture websocket client frame"
        );
        let mut mask = [0u8; 4];
        socket.read_exact(&mut mask)?;
        let mut payload = vec![0; usize::from(frame[1] & 0x7f)];
        socket.read_exact(&mut payload)?;
        for (n, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[n % 4];
        }
        socket.write_all(&[0x81, payload.len() as u8])?;
        socket.write_all(&payload)?;
        socket.write_all(&[0x88, 0])?;
        return Ok(());
    }
    let (status,extra,body)=match path {
        "/identity" => ("200 OK",String::new(),json!({"run_id":run,"service":"simulator-protocol-fixture","native_dashboard":false,"execution_identity":identity()}).to_string()),
        "/redirect" => ("302 Found","Location: http://127.0.0.1:18080/identity\r\n".into(),String::new()),
        "/login" => ("200 OK",format!("Set-Cookie: fixture_session={run}; Path=/; HttpOnly; SameSite=Strict\r\n"),"fixture cookie set".into()),
        "/cookie" => ("200 OK",String::new(),json!({"cookie_present":header.lines().any(|line|line.to_ascii_lowercase().starts_with("cookie:")&&line.contains(run))}).to_string()),
        "/upload" => ("200 OK",String::new(),json!({"bytes":length,"sha256":hex::encode(Sha256::digest(&input[header_end..header_end+length]))}).to_string()),
        _ => ("404 Not Found",String::new(),"unknown fixture endpoint".into())
    };
    write!(socket,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nX-Md-Sim-Run: deliberately-conflicting-upstream\r\n{extra}\r\n{body}",body.len())?;
    Ok(())
}
fn identity() -> ObjectId {
    EXECUTION
        .get()
        .expect("initialized component execution identity")
        .clone()
}
fn capture_inputs(
    path: &Path,
    files: &mut BTreeMap<String, ObjectId>,
    links: &mut BTreeMap<String, String>,
    browser: bool,
) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        files.len() + links.len() < if browser { 8192 } else { 1024 },
        "component input count exceeds bound"
    );
    if metadata.file_type().is_symlink() {
        ensure!(browser, "unexpected captured input symlink");
        let target = fs::read_link(path)?;
        let resolved = path.canonicalize()?;
        ensure!(
            target.is_absolute()
                && target == resolved
                && (resolved.starts_with("/usr") || resolved.starts_with("/etc")),
            "browser link differs from the canonical captured target"
        );
        links.insert(
            path.to_string_lossy().into_owned(),
            target.to_string_lossy().into_owned(),
        );
    } else if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            capture_inputs(&entry?.path(), files, links, browser)?;
        }
    } else {
        ensure!(
            metadata.is_file() && metadata.len() <= if browser { 512 } else { 256 } * 1024 * 1024,
            "component input exceeds bound"
        );
        files.insert(path.to_string_lossy().into_owned(), hash_file(path)?);
    }
    Ok(())
}
fn initialize_identity(role: &str) -> Result<()> {
    let file = if role == "orchestrator" {
        let mut files = BTreeMap::new();
        let mut links = BTreeMap::new();
        let paths = if worker_broker::enabled() {
            vec![
                "/usr",
                "/lib64",
                "/init",
                "/namespace-check",
                "/md-sim-worker-relay",
                "/worker-identity.json",
                "/broker-inputs",
                "/veth.ko.zst",
                "/scenario.json",
            ]
        } else {
            vec![
                "/usr",
                "/lib64",
                "/init",
                "/namespace-check",
                "/check",
                "/md-sim",
                "/md-sim-worker-relay",
                "/fixture-inputs.json",
                "/veth.ko.zst",
                "/scenario.json",
            ]
        };
        for path in paths {
            capture_inputs(
                Path::new(path),
                &mut files,
                &mut links,
                browser_mode() || board_browser::enabled(),
            )?;
        }
        if shared_pool::enabled() {
            for path in [
                "/shared-selection.json",
                "/shared-pool.json",
                "/private-pool.json",
                "/pool-guest",
                "/e1000.ko.zst",
            ] {
                capture_inputs(Path::new(path), &mut files, &mut links, false)?;
            }
            if carrier_dashboard::enabled() {
                for path in [
                    carrier_dashboard::LOCK,
                    carrier_dashboard::POLICY,
                    carrier_dashboard::PROVIDER,
                    "/md-sim-access",
                    "/usr/bin/curl",
                    "/usr/bin/openssl",
                    "/usr/bin/nginx",
                ] {
                    capture_inputs(Path::new(path), &mut files, &mut links, false)?;
                }
            }
        }
        if native_mode() {
            for path in ["/s9-native-network", "/s9"] {
                capture_inputs(
                    Path::new(path),
                    &mut files,
                    &mut links,
                    browser_mode() || board_browser::enabled(),
                )?;
            }
        }
        if board_mode() {
            ensure!(
                !native_mode() && !browser_mode(),
                "board process has its own declared suite"
            );
            for path in ["/s9-execute-prepared", "/etc/md-sim-board"] {
                capture_inputs(
                    Path::new(path),
                    &mut files,
                    &mut links,
                    board_browser::enabled(),
                )?;
            }
        }
        if browser_mode() {
            ensure!(
                native_mode(),
                "native browser requires the native service path"
            );
            for path in ["/native-browser-check", "/etc"] {
                capture_inputs(Path::new(path), &mut files, &mut links, true)?;
            }
            ensure!(
                files.values().map(|object| object.size).sum::<u64>() <= 1280 * 1024 * 1024,
                "browser and base inputs exceed aggregate byte bound"
            );
        }
        if board_browser::enabled() {
            ensure!(
                (board_mode()
                    || (shared_pool::enabled()
                        && carrier_dashboard::enabled()
                        && board_browser::carrier_enabled()))
                    && !browser_mode(),
                "selected browser requires the board suite"
            );
            for path in ["/s9-browser-check", "/etc"] {
                capture_inputs(Path::new(path), &mut files, &mut links, true)?;
            }
            if board_browser::carrier_enabled() {
                capture_inputs(
                    Path::new("/carrier-browser-payload.json"),
                    &mut files,
                    &mut links,
                    true,
                )?;
            }
            ensure!(
                files.values().map(|object| object.size).sum::<u64>() <= 1280 * 1024 * 1024,
                "selected browser and board inputs exceed aggregate byte bound"
            );
        }
        let mut nonce = [0u8; 32];
        fs::File::open("/dev/urandom")?.read_exact(&mut nonce)?;
        let spec = json!({"schema":"md-sim-private-network-component-run/1","inputs":files,"symlinks":links,"native_browser":browser_mode(),"board_browser":board_browser::enabled(),"board_process":board_mode(),"board_local":board::local_mode(),"fresh_run_nonce":hex::encode(nonce),"kernel_release":fs::read_to_string("/proc/sys/kernel/osrelease")?,"kernel_command_line":fs::read_to_string("/proc/cmdline")?,"fixture_only":!native_mode() && !board_mode() && !worker_broker::enabled(),"worker_broker":worker_broker::enabled(),"shared_pool":if shared_pool::enabled() { shared_run::policy() } else { Value::Null },"selected_locked_run":if worker_broker::enabled() { worker_broker::selected_identity()? } else { Value::Null },"native_diagnostic":native_mode(),"firmware_target":null,"network":{"worker":"192.0.2.1/29","client":"192.0.2.2/29","denied_client":"192.0.2.3/29","default_route":false,"host_nic":false},"worker_uid":1000,"suite_binary":"/namespace-check","outer_vm_inputs":"linked separately by host process-spec and root-manifest"});
        fs::write(
            "/work/execution-spec.json",
            serde_json::to_vec_pretty(&spec)?,
        )?;
        fs::set_permissions(
            "/work/execution-spec.json",
            fs::Permissions::from_mode(0o444),
        )?;
        "/work/execution-spec.json"
    } else {
        "/execution-spec.json"
    };
    let object = hash_file(Path::new(file))?;
    ensure!(
        object.size
            <= if browser_mode() || board_browser::enabled() {
                4
            } else {
                1
            } * 1024
                * 1024,
        "component execution spec too large"
    );
    let _: Value = serde_json::from_slice(&fs::read(file)?)?;
    EXECUTION
        .set(object)
        .map_err(|_| anyhow::anyhow!("execution identity already set"))?;
    Ok(())
}

fn server(events: &mut Vec<Value>) -> Result<()> {
    if worker_broker::enabled() {
        return worker_broker::server(events);
    }
    let run = run_id();
    let observed = namespace_evidence("worker")?;
    fs::write(
        "/work/namespace.json",
        serde_json::to_vec_pretty(&observed)?,
    )?;
    events.push(observed);
    if board_mode() {
        ensure!(
            fs::read_dir("/etc/md-sim-board")?.next().is_none(),
            "worker sees preloaded board inputs"
        );
        ensure!(
            !Path::new("/s9-execute-prepared").exists(),
            "worker sees client board helper"
        );
        events.push(json!({"board_inputs_visible_before_transfer":false,"client_board_helper_visible":false}));
    }
    ensure!(
        !Path::new("/work/clientkey").exists() && !Path::new("/credentials/client.key").exists(),
        "worker sees client secrets"
    );
    ensure!(
        unsafe { libc::chown(c"/worker".as_ptr(), 1000, 1000) } == 0,
        "cannot prepare nonzero worker UID storage"
    );
    let mut native = if native_mode() {
        Some(native::launch(events)?)
    } else {
        None
    };
    {
        let prefix = if native_mode() {
            "/work/native-credentials"
        } else {
            "/credentials"
        };
        fs::create_dir("/worker/access-credentials")?;
        fs::set_permissions(
            "/worker/access-credentials",
            fs::Permissions::from_mode(0o700),
        )?;
        std::os::unix::fs::chown("/worker/access-credentials", Some(1000), Some(1000))?;
        for name in ["ca.crt", "server.crt", "server.key"] {
            let destination = format!("/worker/access-credentials/{name}");
            fs::copy(format!("{prefix}/{name}"), &destination)?;
            fs::set_permissions(&destination, fs::Permissions::from_mode(0o600))?;
            std::os::unix::fs::chown(&destination, Some(1000), Some(1000))?;
        }
    }
    let _http = if native.is_none() {
        let listener = TcpListener::bind("127.0.0.1:18080")?;
        Some(thread::spawn(move || {
            for socket in listener.incoming() {
                match socket {
                    Ok(s) => {
                        if let Err(e) = serve_one(s) {
                            eprintln!("fixture HTTP error: {e:#}");
                        }
                    }
                    Err(_) => break,
                }
            }
        }))
    } else {
        None
    };
    let mut spec = GatewaySpec {
        run_id: run.into(),
        execution_identity: identity(),
        dashboard_source: "simulator-console".into(),
        listen: format!("{WORKER}:8443").parse()?,
        upstream: "127.0.0.1:18080".parse()?,
        upstream_unix: None,
        upstream_tls: None,
        browser_policy: if native_mode() {
            BrowserPolicy::Preserve
        } else {
            BrowserPolicy::UpgradeInsecureRequests
        },
        certificate: "/credentials/server.crt".into(),
        private_key: "/credentials/server.key".into(),
        client_ca: "/credentials/ca.crt".into(),
        allow_clients: vec![format!("{CLIENT}/32")],
        browser_context_id: "78".repeat(32),
        nginx: "/usr/bin/nginx".into(),
        openssl: "/usr/bin/openssl".into(),
    };
    if let Some(selected) = &native {
        selected.configure_gateway(&mut spec)?;
    }
    let mut gateway = Gateway::start(&spec, Path::new("/work/gateway"))?;
    events.push(gateway.manifest()?);
    if let Some(selected) = &native {
        selected.publish_endpoint(&gateway)?;
    }
    ensure!(
        unsafe { libc::chown(c"/worker".as_ptr(), 1000, 1000) } == 0,
        "cannot prepare nonzero worker UID storage"
    );
    let worker_log = fs::File::create("/work/worker.log")?;
    let mut worker = Owned(
        Command::new("/md-sim")
            .args([
                "--state-dir",
                "/worker",
                "worker",
                "serve",
                "--socket",
                "/worker/worker.sock",
            ])
            .uid(1000)
            .gid(1000)
            .stdin(Stdio::null())
            .stdout(worker_log.try_clone()?)
            .stderr(worker_log)
            .spawn()?,
    );
    events.push(json!({"worker_uid":1000,"worker_socket":"/worker/worker.sock","firmware_guard_uid_mapping":"nested runtime maps uid0 to this guest uid1000"}));
    let mut sshd = start(
        "/usr/bin/sshd",
        &["-D", "-e", "-f", "/work/sshd_config"],
        "/work/sshd.log",
    )?;
    let deadline = Instant::now()
        + Duration::from_secs(if board_mode() {
            board::server_seconds()
        } else if browser_mode() {
            native_browser_budget::POST_READY_SECONDS
        } else if native_mode() {
            360
        } else {
            200
        });
    loop {
        ensure!(
            gateway.is_running()?,
            "TLS gateway exited; retained gateway logs identify the cause"
        );
        if let Some(status) = worker.0.try_wait()? {
            ensure!(status.success(), "worker exited with {status}");
            break;
        }
        ensure!(sshd.0.try_wait()?.is_none(), "SSH daemon exited");
        ensure!(Instant::now() < deadline, "worker scenario deadline");
        thread::sleep(Duration::from_millis(25));
    }
    gateway.stop()?;
    if let Some(selected) = &mut native {
        selected.stop(events)?;
    }
    events.push(json!({"worker_shutdown":true}));
    Ok(())
}
fn ssh_config() -> SshConfig {
    SshConfig {
        host: WORKER.into(),
        user: "worker".into(),
        port: 2222,
        known_hosts: "/work/known_hosts".into(),
        identity_file: "/work/clientkey".into(),
        remote_relay: "/md-sim-worker-relay".into(),
        worker_socket: "/worker/worker.sock".into(),
        ssh_program: "/usr/bin/ssh".into(),
        timeout_seconds: 30,
    }
}
fn curl_command(path: &str, extra: &[&str], auth: bool) -> Command {
    let mut command = Command::new("/usr/bin/curl");
    command.env_clear().args([
        "--noproxy",
        "*",
        "--silent",
        "--show-error",
        "--max-time",
        "10",
        "--cacert",
        "/credentials/ca.crt",
    ]);
    if auth {
        command.args([
            "--cert",
            "/credentials/client.crt",
            "--key",
            "/credentials/client.key",
        ]);
    }
    command
        .args(extra)
        .arg(format!("https://{WORKER}:8443{path}"))
        .stdin(Stdio::null());
    command
}
fn curl(path: &str, extra: &[&str], auth: bool) -> Result<std::process::Output> {
    Ok(curl_command(path, extra, auth).output()?)
}
fn stream_check(path: &str, expected: &[u8]) -> Result<Value> {
    let mut child = Owned(
        curl_command(path, &["--fail", "--no-buffer"], true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let mut output = BufReader::new(child.0.stdout.take().context("stream stdout missing")?);
    let mut first = Vec::new();
    ensure!(
        output.read_until(b'\n', &mut first)? > 0,
        "stream first line absent"
    );
    let first_at = Instant::now();
    let mut rest = Vec::new();
    output.take(4096).read_to_end(&mut rest)?;
    let delivery_gap = first_at.elapsed();
    first.extend(rest);
    ensure!(
        child.0.wait()?.success() && first == expected,
        "stream output differs"
    );
    ensure!(
        delivery_gap >= Duration::from_millis(150),
        "proxy buffered all stream data until completion"
    );
    Ok(
        json!({"path":path,"received_bytes":first.len(),"first_line_before_completion_ms":delivery_gap.as_millis(),"early_delivery":true}),
    )
}
fn pipe_read_exact(
    reader: &mut std::process::ChildStdout,
    out: &mut [u8],
    deadline: Instant,
) -> Result<()> {
    let mut used = 0;
    while used < out.len() {
        ensure!(Instant::now() < deadline, "TLS websocket read deadline");
        let mut pfd = libc::pollfd {
            fd: reader.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let polled = unsafe { libc::poll(&mut pfd, 1, 100) };
        if polled < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(err.into());
        }
        if polled == 0 {
            continue;
        }
        let count = reader.read(&mut out[used..])?;
        ensure!(count != 0, "TLS websocket ended early");
        used += count;
    }
    Ok(())
}
fn require_upgrade_policy(headers: &str) -> Result<()> {
    let policies: Vec<_> = headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.eq_ignore_ascii_case("content-security-policy"))
        .map(|(_, value)| value.trim())
        .collect();
    ensure!(
        policies == ["upgrade-insecure-requests"],
        "selected gateway browser policy is missing or differs"
    );
    Ok(())
}

fn websocket_check() -> Result<Value> {
    let run = run_id();
    let mut child = Owned(
        Command::new("/usr/bin/openssl")
            .env_clear()
            .env("OPENSSL_CONF", "/dev/null")
            .args([
                "s_client",
                "-quiet",
                "-connect",
                &format!("{WORKER}:8443"),
                "-CAfile",
                "/credentials/ca.crt",
                "-cert",
                "/credentials/client.crt",
                "-key",
                "/credentials/client.key",
                "-verify_return_error",
                "-verify_ip",
                WORKER,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(fs::File::create("/work/websocket-tls.log")?)
            .spawn()?,
    );
    let mut input = child.0.stdin.take().context("TLS websocket stdin absent")?;
    let mut output = child
        .0
        .stdout
        .take()
        .context("TLS websocket stdout absent")?;
    write!(input,"GET /ws HTTP/1.1\r\nHost: {WORKER}:8443\r\nOrigin: https://{WORKER}:8443\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n")?;
    input.flush()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        ensure!(header.len() < 8192, "websocket headers exceed bound");
        let mut byte = [0];
        pipe_read_exact(&mut output, &mut byte, deadline)?;
        header.push(byte[0]);
    }
    let header = String::from_utf8(header)?;
    require_upgrade_policy(&header)?;
    ensure!(
        header.starts_with("HTTP/1.1 101 ")
            && header
                .lines()
                .any(|l| l
                    .eq_ignore_ascii_case("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=")),
        "websocket upgrade failed"
    );
    let mut prefix = [0u8; 2];
    pipe_read_exact(&mut output, &mut prefix, deadline)?;
    ensure!(prefix == [0x81, 64], "websocket run frame differs");
    let mut selected = [0u8; 64];
    pipe_read_exact(&mut output, &mut selected, deadline)?;
    ensure!(selected == run.as_bytes(), "websocket reached another run");
    let payload = b"real bidirectional TLS websocket";
    let mut mask = [0u8; 4];
    fs::File::open("/dev/urandom")?.read_exact(&mut mask)?;
    input.write_all(&[0x81, 0x80 | payload.len() as u8])?;
    input.write_all(&mask)?;
    input.write_all(
        &payload
            .iter()
            .enumerate()
            .map(|(n, b)| b ^ mask[n % 4])
            .collect::<Vec<_>>(),
    )?;
    input.flush()?;
    pipe_read_exact(&mut output, &mut prefix, deadline)?;
    ensure!(
        prefix == [0x81, payload.len() as u8],
        "websocket echo frame differs"
    );
    let mut echoed = vec![0; payload.len()];
    pipe_read_exact(&mut output, &mut echoed, deadline)?;
    ensure!(echoed == payload, "websocket echo data differs");
    pipe_read_exact(&mut output, &mut prefix, deadline)?;
    ensure!(prefix == [0x88, 0], "websocket close frame absent");
    Ok(
        json!({"actual_websocket_upgrade":true,"bidirectional_payload_sha256":hex::encode(Sha256::digest(payload)),"selected_run":run,"tls_certificate_verification":true,"headers":header,"fixture_only":true}),
    )
}
fn body(path: &str, extra: &[&str]) -> Result<Vec<u8>> {
    let out = curl(path, extra, true)?;
    ensure!(
        out.status.success(),
        "authenticated request failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out.stdout)
}
fn client(events: &mut Vec<Value>) -> Result<()> {
    if worker_broker::enabled() {
        return worker_broker::client(events);
    }
    let run = run_id();
    let observed = namespace_evidence("client")?;
    fs::write(
        "/work/namespace.json",
        serde_json::to_vec_pretty(&observed)?,
    )?;
    events.push(observed);
    if native_mode() {
        record_progress("native-http-rpc")?;
        native::client_checks(events)?;
        if browser_mode() {
            record_progress("native-browser")?;
            browser::client_checks(events)?;
        }
        record_progress("ssh-worker")?;
        ssh_read_reconnect(events)?;
        ssh_worker_checks()?;
        record_progress("client-checks-ended")?;
        return Ok(());
    }
    ensure!(
        !Path::new("/worker.sock").exists()
            && !Path::new("/credentials/server.key").exists()
            && !Path::new("/work/hostkey").exists(),
        "client sees worker secrets/socket"
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    while TcpStream::connect(format!("{WORKER}:2222")).is_err()
        || TcpStream::connect(format!("{WORKER}:8443")).is_err()
    {
        ensure!(Instant::now() < deadline, "SSH server readiness deadline");
        thread::sleep(Duration::from_millis(50));
    }
    let mut tunnel = SshTunnel::start(
        ssh_config(),
        "127.0.0.1:18081".parse()?,
        "127.0.0.1:18080".parse()?,
        Path::new("/work/tunnel.log"),
    )?;
    let out = invoke(
        "/usr/bin/curl",
        &[
            "--noproxy",
            "*",
            "--silent",
            "--show-error",
            "--max-time",
            "10",
            "http://127.0.0.1:18081/identity",
        ],
    )?;
    let id: Value = serde_json::from_slice(&out)?;
    ensure!(
        id["run_id"] == run && id["execution_identity"] == serde_json::to_value(identity())?,
        "tunnel reached wrong run"
    );
    events.push(json!({"actual_ssh_tunnel":true,"response":id,"evidence":tunnel.evidence()}));
    tunnel.stop()?;
    ensure!(
        TcpStream::connect("127.0.0.1:18081").is_err(),
        "tunnel listener remains after stop"
    );
    let unauth = curl("/identity", &["--fail"], false)?;
    ensure!(
        !unauth.status.success(),
        "gateway accepted client without certificate"
    );
    let id: Value = serde_json::from_slice(&body(
        "/identity",
        &["--fail", "--dump-header", "/work/identity.headers"],
    )?)?;
    ensure!(
        id["run_id"] == run && id["execution_identity"] == serde_json::to_value(identity())?,
        "TLS reached wrong run"
    );
    let headers = fs::read_to_string("/work/identity.headers")?;
    require_upgrade_policy(&headers)?;
    let ids = headers
        .lines()
        .filter(|l| l.to_ascii_lowercase().starts_with("x-md-sim-run:"))
        .collect::<Vec<_>>();
    ensure!(
        ids.len() == 1 && ids[0].split_whitespace().nth(1) == Some(run),
        "conflicting gateway run header"
    );
    let wrong_host = curl("/identity", &["--fail", "-H", "Host: wrong.invalid"], true)?;
    ensure!(!wrong_host.status.success(), "gateway accepted wrong Host");
    let wrong_origin = curl(
        "/identity",
        &["--fail", "-H", "Origin: https://wrong.invalid"],
        true,
    )?;
    ensure!(
        !wrong_origin.status.success(),
        "gateway accepted unrelated browser origin"
    );
    let denied = curl(
        "/identity",
        &["--interface", "192.0.2.3", "--write-out", "%{http_code}"],
        true,
    )?;
    ensure!(
        denied.status.success() && denied.stdout.ends_with(b"403"),
        "gateway did not deny a valid TLS client outside its selected CIDR"
    );
    let redirected: Value = serde_json::from_slice(&body("/redirect", &["--fail", "--location"])?)?;
    ensure!(
        redirected["run_id"] == run,
        "redirect escaped selected gateway"
    );
    fs::create_dir("/work/browser-a")?;
    fs::create_dir("/work/browser-b")?;
    body(
        "/login",
        &["--fail", "--cookie-jar", "/work/browser-a/cookies"],
    )?;
    let a: Value = serde_json::from_slice(&body(
        "/cookie",
        &["--fail", "--cookie", "/work/browser-a/cookies"],
    )?)?;
    let b: Value = serde_json::from_slice(&body(
        "/cookie",
        &["--fail", "--cookie", "/work/browser-b/cookies"],
    )?)?;
    ensure!(
        a["cookie_present"] == true && b["cookie_present"] == false,
        "separate client cookie stores failed"
    );
    let upload = b"actual bounded fixture upload over authenticated TLS\n";
    fs::write("/work/upload", upload)?;
    let got: Value = serde_json::from_slice(&body(
        "/upload",
        &["--fail", "--data-binary", "@/work/upload"],
    )?)?;
    ensure!(
        got["sha256"] == hex::encode(Sha256::digest(upload)) && got["bytes"] == upload.len(),
        "proxy changed upload body"
    );
    events.push(stream_check("/stream", b"first\nsecond\nthird\n")?);
    events.push(stream_check(
        "/events",
        b"data: first\n\ndata: second\n\ndata: third\n\n",
    )?);
    events.push(websocket_check()?);
    events.push(json!({"actual_mutual_tls":true,"unauthenticated_rejected":true,"wrong_host_rejected":true,"wrong_origin_rejected":true,"wrong_client_cidr_rejected":true,"selected_run":run,"selected_ip":WORKER,"tls_response":id,"headers":headers,"redirect":redirected,"separate_cookie_jars":{"a":a,"b":b},"upload":got,"stream_content_preserved":true,"native_dashboard":false,"fixture_only":true}));
    let board_result = if board_mode() {
        board::client(events)
    } else {
        Ok(())
    };
    // The fixture suite stops the worker even when the board process failed.
    ssh_read_reconnect(events)?;
    let transport_result = ssh_worker_checks();
    board_result?;
    transport_result
}

fn record_progress(stage: &str) -> Result<()> {
    fs::write("/work/stage.tmp", stage)?;
    fs::rename("/work/stage.tmp", "/work/stage")?;
    Ok(())
}
fn ssh_worker_checks() -> Result<()> {
    let status = Command::new("/check")
        .arg("client")
        .env("MD_SIM_WORKER_IP", WORKER)
        .env("MD_SIM_WORKER_USER", "worker")
        .env("MD_SIM_WORKER_SOCKET", "/worker/worker.sock")
        .status()?;
    ensure!(status.success(), "SSH worker test failed");
    Ok(())
}

fn ssh_read_reconnect(events: &mut Vec<Value>) -> Result<()> {
    use md_sim_coordinator::{ssh_transport::SshTransport, Executor, WorkerExecutor};
    let mut config = ssh_config();
    config.remote_relay = "/one-shot-relay".into();
    let mut worker = WorkerExecutor {
        transport: SshTransport::new_streaming(config)?,
    };
    let first = serde_json::to_value(worker.identity()?)?;
    let second = serde_json::to_value(worker.identity()?)?;
    let passed = first == second && worker.transport.reconnect_count() == 1;
    events.push(json!({"actual_ssh_read_reconnect":passed,
        "reconnects":worker.transport.reconnect_count(),
        "forced_server_channel_close_after_each_reply":true,
        "worker":{"protocol":first["policy"]["protocol"],"identity_before":first,"identity_after":second},
        "transport":worker.transport.evidence(),"scope":"Read-only SSH transport component; canonical executor identity excludes transient doctor observations; no firmware gate"}));
    ensure!(
        passed,
        "actual SSH read reconnect did not retain the worker identity"
    );
    Ok(())
}
fn role_command(role: &str) -> Result<Command> {
    let mut command = Command::new("/usr/bin/ip");
    command.args([
        "netns",
        "exec",
        if role == "server" {
            "md-worker"
        } else {
            "md-client"
        },
        "/usr/bin/bwrap",
        "--unshare-pid",
        "--unshare-ipc",
        "--unshare-uts",
        "--die-with-parent",
        "--new-session",
        "--cap-add",
        "ALL",
        "--tmpfs",
        "/",
        "--ro-bind",
        "/usr",
        "/usr",
        "--ro-bind",
        "/lib64",
        "/lib64",
        "--ro-bind",
        "/etc",
        "/etc",
        // The trusted VM role mounts its own proc after Bubblewrap creates
        // its PID namespace and root. Bubblewrap's read-only child mounts
        // prevent an unprivileged nested worker from creating a new proc.
        // The firmware runtime still uses its full protected proc policy.
        "--dir",
        "/proc",
        "--dev",
        "/dev",
        "--ro-bind",
        "/sys",
        "/sys",
        "--perms",
        "01777",
        "--tmpfs",
        "/tmp",
        "--tmpfs",
        "/run",
        "--dir",
        "/var",
        "--dir",
        "/var/empty",
        "--dir",
        "/credentials",
        "--dir",
        "/client-store",
        "--dir",
        "/worker",
    ]);
    command.args(["--bind", &format!("/work/{role}-root"), "/work"]);
    if worker_broker::enabled() {
        command.args(["--setenv", "MD_SIM_WORKER_BROKER", "1"]);
        if shared_pool::enabled() {
            shared_pool::role_mounts(&mut command, role);
        }
        if role == "server" {
            command.args([
                "--dir",
                "/broker-inputs",
                "--ro-bind",
                "/worker-identity.json",
                "/worker-identity.json",
            ]);
            worker_broker::inherit_device(&mut command)?;
        } else {
            command.args(["--ro-bind", "/broker-inputs", "/broker-inputs"]);
        }
    }
    command.args([
        "--ro-bind",
        "/work/execution-spec.json",
        "/execution-spec.json",
    ]);
    if board_mode() {
        command.args(["--setenv", "MD_SIM_BOARD_PROCESS", "1"]);
        if board::local_mode() {
            command.args(["--setenv", "MD_SIM_BOARD_LOCAL", "1"]);
        }
        if board::extended_mode() {
            command.args(["--setenv", "MD_SIM_BOARD_EXTENDED", "1"]);
        }
        if board::long_mode() {
            command.args(["--setenv", "MD_SIM_BOARD_LONG", "1"]);
        }
        if board::slow_mode() {
            command.args(["--setenv", "MD_SIM_BOARD_SLOW", "1"]);
        }
        if board::long_boot_mode() {
            command.args(["--setenv", "MD_SIM_BOARD_LONG_BOOT", "1"]);
        }
        if role == "server" {
            // The worker must receive each artifact through authenticated SSH.
            // Its mount and PID namespaces cannot reach the client's payload.
            command.args(["--tmpfs", "/etc/md-sim-board"]);
        } else {
            command.args(["--ro-bind", "/s9-execute-prepared", "/s9-execute-prepared"]);
        }
    }
    if board_browser::enabled() {
        command.args(["--setenv", "MD_SIM_BOARD_BROWSER", "1"]);
        if board_browser::carrier_enabled() {
            command.args(["--setenv", "MD_SIM_CARRIER_BROWSER", "1"]);
        }
        if role == "client" {
            command.args(["--ro-bind", "/s9-browser-check", "/s9-browser-check"]);
        }
    }
    if native_mode() {
        command.args(["--setenv", "MD_SIM_NATIVE", "1"]);
        if role == "server" {
            command.args([
                "--ro-bind",
                "/s9-native-network",
                "/s9-native-network",
                "--ro-bind",
                "/s9",
                "/s9",
                "--bind",
                "/work/server-state",
                "/worker",
            ]);
        } else {
            command.args([
                "--ro-bind",
                "/work/server-root/native-endpoint.json",
                "/native-endpoint.json",
            ]);
        }
    }
    if browser_mode() {
        command.args(["--setenv", "MD_SIM_NATIVE_BROWSER", "1"]);
        if role == "client" {
            command.args([
                "--ro-bind",
                "/native-browser-check",
                "/native-browser-check",
            ]);
        }
    }
    let programs = if worker_broker::enabled() {
        if carrier_dashboard::enabled() {
            vec!["/namespace-check", "/md-sim-worker-relay", "/md-sim-access"]
        } else {
            vec!["/namespace-check", "/md-sim-worker-relay"]
        }
    } else {
        vec![
            "/check",
            "/namespace-check",
            "/md-sim",
            "/md-sim-worker-relay",
            "/md-sim-access",
            "/fixture-inputs.json",
        ]
    };
    for program in programs {
        command.args(["--ro-bind", program, program]);
    }
    if role == "server" && !worker_broker::enabled() {
        command.args(["--ro-bind", "/namespace-check", "/one-shot-relay"]);
        for operation in ["chunk", "finish"] {
            command.args([
                "--ro-bind",
                "/check",
                &format!("/drop-{operation}-ack-relay"),
            ]);
        }
    }
    let keys = if role == "server" {
        vec!["hostkey", "sshd_config", "clientkey.pub"]
    } else {
        vec!["clientkey", "clientkey.pub", "known_hosts"]
    };
    for key in keys {
        command.args([
            "--ro-bind",
            &format!("/work/keys/{key}"),
            &format!("/work/{key}"),
        ]);
    }
    let certs = if worker_broker::enabled() && !carrier_dashboard::enabled() {
        vec![]
    } else if role == "server" {
        vec!["ca.crt", "server.crt", "server.key"]
    } else {
        vec!["ca.crt", "client.crt", "client.key"]
    };
    for name in certs {
        if native_mode() && role == "server" {
            continue;
        }
        let prefix = if native_mode() {
            "/work/server-root/native-credentials"
        } else {
            "/work/credentials"
        };
        command.args([
            "--ro-bind",
            &format!("{prefix}/{name}"),
            &format!("/credentials/{name}"),
        ]);
    }
    command.args(["--chdir", "/work", "--", "/namespace-check", role]);
    Ok(command)
}
fn orchestrate(events: &mut Vec<Value>) -> Result<()> {
    let run = run_id();
    for path in [
        "/etc",
        "/run/netns",
        "/usr/share/empty.sshd",
        "/work/keys",
        "/work/server-root",
        "/work/client-root",
        "/work/server-state",
    ] {
        fs::create_dir_all(path)?;
    }
    std::os::unix::fs::symlink("/run", "/var/run")?;
    fs::write("/etc/passwd","root::0:0:VM test root:/work:/usr/bin/sh\nworker::1000:1000:VM worker:/worker:/usr/bin/sh\nhttp:x:33:33:HTTP service:/var/empty:/usr/bin/sh\nnobody:x:65534:65534:SSH privilege separation:/var/empty:/usr/bin/sh\n")?;
    fs::write(
        "/etc/group",
        "root:x:0:\nworker:x:1000:\nhttp:x:33:\nnobody:x:65534:\n",
    )?;
    fs::write(
        "/etc/nsswitch.conf",
        "passwd: files\ngroup: files\nhosts: files\n",
    )?;
    fs::write("/etc/hosts", "127.0.0.1 localhost\n")?;
    invoke(
        "/usr/bin/zstd",
        &["--decompress", "/veth.ko.zst", "-o", "/work/veth.ko"],
    )?;
    invoke("/usr/bin/insmod", &["/work/veth.ko"])?;
    for ns in ["md-worker", "md-client"] {
        invoke("/usr/bin/ip", &["netns", "add", ns])?;
    }
    invoke(
        "/usr/bin/ip",
        &[
            "link", "add", "worker0", "type", "veth", "peer", "name", "client0",
        ],
    )?;
    for (ns, interface, address) in [
        ("md-worker", "worker0", "192.0.2.1/29"),
        ("md-client", "client0", "192.0.2.2/29"),
    ] {
        invoke("/usr/bin/ip", &["link", "set", interface, "netns", ns])?;
        invoke(
            "/usr/bin/ip",
            &["-n", ns, "address", "add", address, "dev", interface],
        )?;
        invoke("/usr/bin/ip", &["-n", ns, "link", "set", interface, "up"])?;
        invoke("/usr/bin/ip", &["-n", ns, "link", "set", "lo", "up"])?;
    }
    invoke(
        "/usr/bin/ip",
        &[
            "-n",
            "md-client",
            "address",
            "add",
            "192.0.2.3/29",
            "dev",
            "client0",
        ],
    )?;
    events.push(namespace_evidence("VM orchestrator")?);
    for key in ["hostkey", "clientkey"] {
        invoke(
            "/usr/bin/ssh-keygen",
            &[
                "-q",
                "-t",
                "ed25519",
                "-N",
                "",
                "-f",
                &format!("/work/keys/{key}"),
            ],
        )?;
        fs::set_permissions(
            format!("/work/keys/{key}"),
            fs::Permissions::from_mode(0o600),
        )?;
    }
    let public = fs::read_to_string("/work/keys/hostkey.pub")?;
    let pair = public
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");
    fs::write(
        "/work/keys/known_hosts",
        format!("[{WORKER}]:2222 {pair}\n"),
    )?;
    fs::write("/work/keys/sshd_config",format!("Port 2222\nListenAddress {WORKER}\nHostKey /work/hostkey\nPidFile /work/sshd.pid\nAuthorizedKeysFile /work/clientkey.pub\nStrictModes yes\nPasswordAuthentication no\nKbdInteractiveAuthentication no\nPubkeyAuthentication yes\nPermitRootLogin prohibit-password\nUsePAM no\nUseDNS no\nPrintMotd no\nLogLevel VERBOSE\nAllowUsers worker\n"))?;
    if worker_broker::enabled() {
        if shared_pool::enabled() {
            shared_pool::configure_network(events)?;
        }
        if carrier_dashboard::enabled() {
            events.push(credentials::create(
                Path::new("/usr/bin/openssl"),
                Path::new("/work/credentials"),
                WORKER.parse()?,
                run,
            )?);
        }
        return worker_broker::orchestrate_roles(events);
    }
    if !native_mode() {
        events.push(credentials::create(
            Path::new("/usr/bin/openssl"),
            Path::new("/work/credentials"),
            WORKER.parse()?,
            run,
        )?);
    }
    let mut server = Owned(
        role_command("server")?
            .stdin(Stdio::null())
            .stdout(fs::File::create("/work/server.stdout")?)
            .stderr(fs::File::create("/work/server.stderr")?)
            .spawn()?,
    );
    if native_mode() {
        let deadline = Instant::now() + Duration::from_secs(210);
        while !Path::new("/work/server-root/native-endpoint.json").exists() {
            ensure!(
                server.0.try_wait()?.is_none(),
                "native worker exited before endpoint readiness"
            );
            ensure!(
                Instant::now() < deadline,
                "native endpoint readiness deadline"
            );
            thread::sleep(Duration::from_millis(100));
        }
    }
    let mut client = Owned(
        role_command("client")?
            .stdin(Stdio::null())
            .stdout(fs::File::create("/work/client.stdout")?)
            .stderr(fs::File::create("/work/client.stderr")?)
            .spawn()?,
    );
    let began = Instant::now();
    let mut last_stage = Vec::new();
    let mut last_sample = Vec::new();
    let status = loop {
        if (native_mode() || board_mode()) && Path::new("/work/client-root/stage").exists() {
            let stage = read_evidence(Path::new("/work/client-root/stage"), 128)?;
            if stage != last_stage {
                println!(
                    "MD_SIM_NAMESPACE_PROGRESS {}",
                    json!({
                        "client_stage":std::str::from_utf8(&stage)?,
                        "elapsed_ms":began.elapsed().as_millis(),
                        "diagnostic_only":true
                    })
                );
                std::io::stdout().flush()?;
                last_stage = stage;
            }
        }
        if board_mode() && Path::new("/work/client-root/board-live.json").exists() {
            let sample = read_evidence(Path::new("/work/client-root/board-live.json"), 16 << 10)?;
            if sample != last_sample {
                let record: Value = serde_json::from_slice(&sample)?;
                ensure!(
                    record["diagnostic_only"] == true && record["provisional"] == true,
                    "live diagnostic scope differs"
                );
                println!("MD_SIM_BOARD_LIVE {record}");
                std::io::stdout().flush()?;
                last_sample = sample;
            }
        }
        if let Some(status) = client.0.try_wait()? {
            break status;
        }
        thread::sleep(Duration::from_millis(200));
    };
    ensure!(status.success(), "isolated client failed");
    ensure!(server.0.wait()?.success(), "isolated worker failed");
    for role in ["server", "client"] {
        let saved: Value = serde_json::from_slice(&fs::read(format!(
            "/work/{role}-root/scenario-result.json"
        ))?)?;
        ensure!(
            saved["role"] == role && saved["passed"] == true,
            "saved {role} scenario did not pass"
        );
    }
    let stdout = fs::read_to_string("/work/client.stdout")?;
    let records = stdout
        .lines()
        .filter_map(|l| l.strip_prefix("MD_SIM_SSH_EVIDENCE "))
        .collect::<Vec<_>>();
    ensure!(
        records.len() == 1,
        "missing or duplicate complete SSH report"
    );
    let ssh: Value = serde_json::from_str(records[0])?;
    ensure!(ssh["passed"] == true, "SSH report did not pass");
    events.push(json!({"actual_ssh_worker_report":ssh}));
    let worker: Value = serde_json::from_slice(&fs::read("/work/server-root/namespace.json")?)?;
    let client: Value = serde_json::from_slice(&fs::read("/work/client-root/namespace.json")?)?;
    for namespace in ["net", "mnt", "pid", "uts", "ipc"] {
        ensure!(
            worker["namespaces"][namespace] != client["namespaces"][namespace],
            "client and worker share {namespace} namespace"
        );
    }
    events.push(json!({"verified_distinct_namespaces":{"worker":worker,"client":client}}));
    for ns in ["md-worker", "md-client"] {
        invoke("/usr/bin/ip", &["netns", "delete", ns])?;
    }
    events.push(json!({"separate_network_mount_pid_ipc_uts_environments":true,"link":"private in-guest veth, 192.0.2.0/29, no default route","host_network_backend":false,"external_physical_host":false,"fixture_only":!native_mode() && !board_mode(),"native_diagnostic":native_mode(),"board_process":board_mode()}));
    Ok(())
}
fn main() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.len() == 7 && args[1] == "prepare-worker-broker" {
        return worker_broker::prepare(
            Path::new(&args[2]),
            Path::new(&args[3]),
            Path::new(&args[4]),
            Path::new(&args[5]),
            Path::new(&args[6]),
        );
    }
    if (args.len() == 9 || args.len() == 10) && args[1] == "prepare-shared-pool" {
        return shared_pool::prepare(
            Path::new(&args[2]),
            Path::new(&args[3]),
            Path::new(&args[4]),
            shared_pool::Support {
                relay: Path::new(&args[5]),
                veth: Path::new(&args[6]),
                pool: Path::new(&args[7]),
                module: Path::new(&args[8]),
                browser_payload: args.get(9).map(Path::new),
            },
        );
    }
    if args.len() == 2 && args[1] == "check-shared-network" {
        return shared_pool::network_check();
    }
    if args.len() == 2 && args[1] == "broker-service" {
        return worker_broker::service();
    }
    if args.len() == 2 && args[1] == "carrier-dashboard-service" {
        return carrier_dashboard::service();
    }
    if args.first().is_some_and(|arg| arg == "/one-shot-relay") {
        ensure!(
            args.len() == 3 && args[1] == "/worker/worker.sock" && args[2] == "--stream",
            "one-shot relay requires the isolated test VM and exact worker socket"
        );
        return ssh_one_reply::run(Path::new(&args[1]));
    }
    if (args.len() == 6 || args.len() == 7)
        && (args[1] == "prepare-board" || args[1] == "prepare-board-local")
    {
        return board::prepare(
            Path::new(&args[2]),
            Path::new(&args[3]),
            Path::new(&args[4]),
            Path::new(&args[5]),
            args[1] == "prepare-board-local",
            args.get(6).map(Path::new),
        );
    }
    if args.len() == 3 && args[1] == "report" {
        return host_report(Path::new(&args[2]));
    }
    if args.len() == 3 && args[1] == "extract-board-failure" {
        return board::extract_failure(Path::new(&args[2]));
    }
    if args.len() == 2 && (args[1] == "server" || args[1] == "client") {
        // Only this captured infrastructure program runs here. It is root
        // inside the disconnected VM, before the worker drops to UID1000.
        // Firmware never inherits this root or mount namespace.
        ensure!(
            !Path::new("/proc/self").exists() && unsafe { libc::geteuid() } == 0,
            "role requires an empty proc mount point and VM root"
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
            "private role proc mount failed: {}",
            std::io::Error::last_os_error()
        );
    }
    ensure!(
        fs::read_to_string("/proc/cmdline")?.contains("md_sim_private_guest=1"),
        "requires private guest"
    );
    let role = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "orchestrator".into());
    initialize_identity(&role)?;
    let mut events = vec![
        json!({"component_execution_identity":identity(),"run_id":run_id(),"scope":if worker_broker::enabled() {"Actual selected LockedRun through native outside-VM worker and SSH; claims remain those of the recorded selected scope"} else if board_mode() && board::local_mode() {"Archived board reference process through the local executor; separate SSH/TLS protocol fixtures; not a resolved physical target"} else if board_mode() {"Archived board reference process over SSH; separate gateway protocol fixture; not a resolved physical target"} else {"transport protocol fixture; not a resolved firmware target"}}),
    ];
    let mut result = match role.as_str() {
        "orchestrator" => orchestrate(&mut events),
        "server" => server(&mut events),
        "client" => client(&mut events),
        _ => bail!("unknown role"),
    };
    if role == "client" && native_mode() {
        if let Err(error) = native::retain_http_evidence(&mut events) {
            events.push(json!({"native_http_evidence_error":format!("{error:#}")}));
            if result.is_ok() {
                result = Err(error);
            }
        }
    }
    if role == "orchestrator" && native_mode() && result.is_err() {
        if let Err(error) = native::retain_failure_storage(&mut events) {
            events.push(json!({"native_failure_storage_error":format!("{error:#}")}));
        }
    }
    for path in [
        "/work/worker.log",
        "/work/sshd.log",
        "/work/tunnel.log",
        "/work/websocket-tls.log",
        "/work/gateway/startup.log",
        "/work/gateway/error.log",
        "/work/gateway/access.log",
        "/work/server-root/worker.log",
        "/work/server-root/sshd.log",
        "/work/server-root/gateway/startup.log",
        "/work/server-root/gateway/error.log",
        "/work/client-root/tunnel.log",
        "/work/native-driver.log",
        "/work/server-root/native-driver.log",
        "/worker/native/serial.log",
        "/worker/native/qemu.log",
        "/worker/native/startup.jsonl",
        "/work/server-state/native/serial.log",
        "/work/server-state/native/qemu.log",
        "/work/server-state/native/startup.jsonl",
        "/work/server.stderr",
        "/work/client.stderr",
        "/work/client.stdout",
        "/work/client-first.stdout",
        "/work/client-first.stderr",
        "/work/server-root/broker.stdout",
        "/work/server-root/broker.stderr",
        "/work/server-root/broker/broker.jsonl",
        "/work/server-root/pool.stdout",
        "/work/server-root/pool.stderr",
        "/work/server-root/pool-outcome.json",
        "/work/server-root/broker/shared-lease.json",
        "/work/server-root/shared-ready.json",
        "/work/server-root/pool-start.json",
        "/work/server-root/carrier.stdout",
        "/work/server-root/carrier.stderr",
        "/work/server-root/carrier-launch.json",
        "/work/server-root/carrier-start.json",
        "/work/server-root/carrier-outcome.json",
        "/work/server-root/access/service-failure.json",
        "/work/server-root/access/service-outcome.json",
    ] {
        if let Ok(bytes) = fs::read(path) {
            events.push(json!({"log":path,"hex":hex::encode(&bytes[..bytes.len().min(256*1024)]),"truncated":bytes.len()>256*1024}));
        }
    }
    if role == "orchestrator" {
        events.push(json!({"component_execution_spec":serde_json::from_slice::<Value>(&fs::read("/work/execution-spec.json")?)?}));
        for selected in ["server", "client"] {
            if let Ok(bytes) = fs::read(format!("/work/{selected}-root/scenario-result.json")) {
                events.push(json!({"role_report":serde_json::from_slice::<Value>(&bytes)?}));
            }
        }
    }
    let report = json!({"role":role,"passed":result.is_ok(),"error":result.as_ref().err().map(|e|format!("{e:#}")),"events":events});
    if role == "orchestrator" {
        println!("MD_SIM_NAMESPACE_EVIDENCE {report}");
    } else {
        fs::write(
            "/work/scenario-result.json",
            serde_json::to_vec_pretty(&report)?,
        )?;
    }
    result
}

fn host_report(directory: &Path) -> Result<()> {
    ensure!(
        !directory.join("remote-report.json").exists(),
        "remote report already exists"
    );
    let assessment: Value =
        serde_json::from_slice(&read_evidence(&directory.join("assessment.json"), 65536)?)?;
    ensure!(
        assessment["guest_boot_and_tcp_probe"] == true,
        "outer guest prerequisite failed"
    );
    verify_assessment(directory, &assessment)?;
    let scenario: Value =
        serde_json::from_slice(&read_evidence(&directory.join("scenario.json"), 65536)?)?;
    let browser = scenario["environment"]["MD_SIM_NATIVE_BROWSER"] == "1";
    let board_browser = scenario["environment"]["MD_SIM_BOARD_BROWSER"] == "1";
    let native_run = scenario["environment"]["MD_SIM_NATIVE"] == "1";
    let board_run = scenario["environment"]["MD_SIM_BOARD_PROCESS"] == "1";
    let board_local = scenario["environment"]["MD_SIM_BOARD_LOCAL"] == "1";
    ensure!(
        !board_local || board_run,
        "local board executor requires board process scenario"
    );
    let process: md_sim_runtime::isolation::ProcessResult =
        serde_json::from_slice(&read_evidence(
            &directory.join("result.json"),
            if board_run {
                257
            } else if native_run {
                17
            } else {
                9
            } << 20,
        )?)?;
    ensure!(
        process.exit_code == Some(0) && !process.trace_truncated && process.stderr.is_empty(),
        "outer VM process did not pass completely"
    );
    let raw = read_evidence(
        &directory.join("guest.log"),
        if board_run {
            128
        } else if native_run {
            8
        } else {
            4
        } << 20,
    )?;
    ensure!(
        hex::decode(&process.stdout)? == raw,
        "guest log differs from captured process output"
    );
    let log = std::str::from_utf8(&raw)?;
    let records = log
        .lines()
        .filter_map(|line| line.strip_prefix("MD_SIM_NAMESPACE_EVIDENCE "))
        .collect::<Vec<_>>();
    ensure!(
        records.len() == 1,
        "missing or duplicate complete namespace record"
    );
    let report: Value = serde_json::from_str(records[0])?;
    ensure!(
        report["role"] == "orchestrator" && report["passed"] == true && report["error"].is_null(),
        "guest namespace suite failed"
    );
    let events = report["events"]
        .as_array()
        .context("missing guest events")?;
    let unique = |key: &str| -> Result<&Value> {
        let matches = events
            .iter()
            .filter(|event| !event[key].is_null())
            .collect::<Vec<_>>();
        ensure!(matches.len() == 1, "missing or duplicate {key}");
        Ok(&matches[0][key])
    };
    let identity: ObjectId =
        serde_json::from_value(unique("component_execution_identity")?.clone())?;
    let spec = unique("component_execution_spec")?;
    ensure!(
        spec["native_browser"] == browser
            && spec["board_process"] == board_run
            && (spec["board_browser"] == true) == board_browser,
        "browser scenario and captured identity differ"
    );
    ensure!(
        (spec["board_local"] == true) == board_local,
        "local board scenario differs from captured identity"
    );
    let bytes = serde_json::to_vec_pretty(spec)?;
    ensure!(
        identity.sha256 == hex::encode(Sha256::digest(&bytes))
            && identity.size == bytes.len() as u64,
        "component execution spec identity mismatch"
    );
    let manifest: Vec<md_sim_runtime::isolation::Mount> = serde_json::from_slice(&read_evidence(
        &directory.join("root-manifest.json"),
        if browser || board_browser { 4 } else { 1 } << 20,
    )?)?;
    let outer: md_sim_runtime::isolation::ProcessSpec = serde_json::from_slice(&read_evidence(
        &directory.join("process-spec.json"),
        1 << 20,
    )?)?;
    outer.validate()?;
    for input in manifest.iter().chain(&outer.inputs) {
        ensure!(
            input.object.sha256.len() == 64
                && input
                    .object
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid captured input hash"
        );
        ensure!(
            hash_file(
                &directory
                    .join("input-store/objects")
                    .join(&input.object.sha256)
            )? == input.object,
            "captured input bytes changed: {}",
            input.guest_path
        );
    }
    let reporter = hash_file(&std::env::current_exe()?)?;
    ensure!(
        manifest
            .iter()
            .filter(|input| input.guest_path == "/namespace-check" && input.object == reporter)
            .count()
            == 1,
        "reporter differs from actual captured guest verifier"
    );
    for (path, object) in spec["inputs"]
        .as_object()
        .context("component inputs absent")?
    {
        let object: ObjectId = serde_json::from_value(object.clone())?;
        ensure!(
            manifest
                .iter()
                .any(|input| input.guest_path == *path && input.object == object),
            "component input {path} is not the captured guest input"
        );
    }
    if browser || board_browser {
        browser::verify_captured_inputs(directory, spec, &manifest, &outer)?;
    }
    let namespaces = unique("verified_distinct_namespaces")?;
    for name in ["net", "mnt", "pid", "ipc", "uts"] {
        let worker = namespaces["worker"]["namespaces"][name]
            .as_str()
            .context("missing worker namespace")?;
        let client = namespaces["client"]["namespaces"][name]
            .as_str()
            .context("missing client namespace")?;
        ensure!(worker != client, "same client and worker {name} namespace");
    }
    dashboard_transport_report::verify_report(unique("actual_ssh_worker_report")?)?;
    let roles = events
        .iter()
        .filter_map(|event| event.get("role_report"))
        .collect::<Vec<_>>();
    ensure!(
        roles.len() == 2
            && roles.iter().all(|role| role["passed"] == true)
            && roles.iter().filter(|role| role["role"] == "client").count() == 1
            && roles.iter().filter(|role| role["role"] == "server").count() == 1,
        "role results incomplete or duplicate"
    );
    let client = roles
        .iter()
        .find(|role| role["role"] == "client")
        .context("client report missing")?;
    let client_events = client["events"]
        .as_array()
        .context("client events missing")?;
    verify_read_reconnect(client_events)?;
    let native = spec["native_diagnostic"] == true;
    let selected = if native {
        let server = roles
            .iter()
            .find(|role| role["role"] == "server")
            .context("server report missing")?;
        native::verify_report(
            client_events,
            server["events"]
                .as_array()
                .context("server events missing")?,
            spec,
            &identity,
        )?
    } else {
        verify_fixture_report(client_events, &identity)?;
        if board_run {
            let server = roles
                .iter()
                .find(|role| role["role"] == "server")
                .context("server report missing")?;
            ensure!(
                server["events"]
                    .as_array()
                    .context("server events absent")?
                    .iter()
                    .any(
                        |event| event["board_inputs_visible_before_transfer"] == false
                            && event["client_board_helper_visible"] == false
                    ),
                "worker input isolation not verified"
            );
            board::restore(directory, client_events, spec)?
        } else {
            identity.clone()
        }
    };
    if browser {
        ensure!(native, "browser suite did not execute a native dashboard");
        let server = roles
            .iter()
            .find(|role| role["role"] == "server")
            .context("server report missing")?;
        browser::verify_report(
            client_events,
            server["events"]
                .as_array()
                .context("server events missing")?,
            spec,
            &selected,
        )?;
    }
    let mut inputs = BTreeMap::new();
    for file in [
        "assessment.json",
        "process-spec.json",
        "result.json",
        "root-manifest.json",
        "preparation-spec.json",
        "preparation-result.json",
        "scenario.json",
        "topology.json",
        "guest.log",
        "backend.log",
    ] {
        inputs.insert(file, hash_file(&directory.join(file))?);
    }
    if browser || board_run {
        for file in ["payload-manifest.json", "payload-report.json"] {
            inputs.insert(file, hash_file(&directory.join(file))?);
        }
    }
    let source = include_bytes!("remote_namespace_check.rs");
    let native_source = include_bytes!("remote_namespace_check/native.rs");
    let scope = if board_local {
        "Actual archived board reference process through the local executor in a disconnected local VM. Separate SSH/TLS protocol fixtures do not establish remote execution or dashboard access for this board."
    } else if board_run {
        "Actual archived board reference process over SSH between separate client/worker environments inside one local VM. TLS checks use a separate protocol fixture; selected board dashboard access is untested."
    } else if native {
        "actual adapted native S9 services; separate client/worker environments inside one local full-system VM"
    } else {
        "actual separate client/worker environments inside one local full-system VM; transport protocol fixture only"
    };
    let browser_source = include_bytes!("remote_namespace_check/browser.rs");
    let output = json!({"schema":"md-sim-isolated-remote-component-report/1","passed":true,"scope":scope,"external_physical_host":false,"native_dashboard":native,"firmware_execution_gate":false,"original_board_boot_gate":false,"mining_gate":false,"browser_javascript_execution":browser,"full_guest_reboot_persistence":native,"exact_hardware_matrix_gate":false,"component_execution_identity":identity,"selected_execution_identity":selected,"run_id":selected.sha256,"vm_evidence":inputs,"reporter_binary":reporter,"reporter_source":{"sha256":hex::encode(Sha256::digest(source)),"size":source.len(),"binding":"compiled include_bytes source"},"native_reporter_source":{"sha256":hex::encode(Sha256::digest(native_source)),"size":native_source.len(),"binding":"compiled include_bytes source"},"browser_reporter_source":{"sha256":hex::encode(Sha256::digest(browser_source)),"size":browser_source.len(),"binding":"compiled include_bytes source"},"observations":report});
    let mut output = output;
    if board_run {
        output["board_executor"] = json!(if board_local { "local" } else { "ssh" });
        if !board_local {
            let selected: Value =
                serde_json::from_slice(&fs::read(directory.join("board-result/worker-run.json"))?)?;
            output["selected_worker_run_id"] = selected["run_id"].clone();
        }
        output["board_process_execution"] = json!(true);
        output["board_dashboard_access_gate"] = json!(false);
    }
    fs::write(
        directory.join("remote-report.json"),
        serde_json::to_vec_pretty(&output)?,
    )?;
    let mut checksums = BTreeMap::new();
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if fs::symlink_metadata(&path)?.is_file() {
            checksums.insert(
                path.file_name()
                    .context("filename absent")?
                    .to_string_lossy()
                    .into_owned(),
                hash_file(&path)?,
            );
        }
    }
    fs::write(
        directory.join("remote-evidence-checksums.json"),
        serde_json::to_vec_pretty(&checksums)?,
    )?;
    println!(
        "{}",
        json!({"passed":true,"output":directory.join("remote-report.json"),"identity":selected,"scope":scope})
    );
    Ok(())
}

fn read_evidence(path: &Path, cap: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(cap + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= cap,
        "evidence exceeds bound: {}",
        path.display()
    );
    Ok(bytes)
}

fn verify_assessment(directory: &Path, assessment: &Value) -> Result<()> {
    for (field, file) in [
        ("guest_log", "guest.log"),
        ("process_result", "result.json"),
        ("process_spec", "process-spec.json"),
    ] {
        ensure!(
            assessment[field] == serde_json::to_value(hash_file(&directory.join(file))?)?,
            "outer assessment refers to other {file} bytes"
        );
    }
    Ok(())
}

#[cfg(test)]
mod report_tests {
    use super::*;
    #[test]
    fn reconnect_report_needs_actual_bounded_recovery_and_worker_identity() -> Result<()> {
        let record = json!({"actual_ssh_read_reconnect":true,"reconnects":1,
            "forced_server_channel_close_after_each_reply":true,
            "worker":{"protocol":"md-sim-worker/2","identity_before":{"build":"same"},
                "identity_after":{"build":"same"}},"transport":{"read_only_reconnects":1}});
        verify_read_reconnect(std::slice::from_ref(&record))?;
        assert!(verify_read_reconnect(&[]).is_err());
        assert!(verify_read_reconnect(&[record.clone(), record.clone()]).is_err());
        for field in [
            "actual_ssh_read_reconnect",
            "reconnects",
            "forced_server_channel_close_after_each_reply",
            "worker",
            "transport",
        ] {
            let mut changed = record.clone();
            changed.as_object_mut().unwrap().remove(field);
            assert!(verify_read_reconnect(&[changed]).is_err());
        }
        let mut changed = record.clone();
        changed["reconnects"] = json!(2);
        assert!(verify_read_reconnect(&[changed]).is_err());
        let mut changed = record;
        changed["worker"]["identity_after"]["build"] = json!("different");
        assert!(verify_read_reconnect(&[changed]).is_err());
        Ok(())
    }
    #[test]
    fn gateway_policy_evidence_rejects_missing_duplicate_and_different_headers() {
        let header = "Content-Security-Policy: upgrade-insecure-requests\r\n";
        assert!(require_upgrade_policy(header).is_ok());
        assert!(require_upgrade_policy(&header.to_ascii_lowercase()).is_ok());
        for changed in [
            String::new(),
            header.repeat(2),
            "X-Content-Security-Policy: upgrade-insecure-requests\r\n".into(),
            "Content-Security-Policy: default-src *\r\n".into(),
            "Content-Security-Policy-Report-Only: upgrade-insecure-requests\r\n".into(),
        ] {
            assert!(require_upgrade_policy(&changed).is_err());
        }
    }
    #[test]
    fn changed_outer_evidence_and_oversized_records_are_rejected() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut assessment = json!({});
        for (field, file) in [
            ("guest_log", "guest.log"),
            ("process_result", "result.json"),
            ("process_spec", "process-spec.json"),
        ] {
            fs::write(directory.path().join(file), file)?;
            assessment[field] = serde_json::to_value(hash_file(&directory.path().join(file))?)?;
        }
        verify_assessment(directory.path(), &assessment)?;
        for file in ["guest.log", "result.json", "process-spec.json"] {
            fs::write(directory.path().join(file), "changed")?;
            assert!(verify_assessment(directory.path(), &assessment).is_err());
            fs::write(directory.path().join(file), file)?;
        }
        assert!(read_evidence(&directory.path().join("guest.log"), 4).is_err());
        Ok(())
    }
}

fn verify_read_reconnect(events: &[Value]) -> Result<()> {
    let records: Vec<_> = events
        .iter()
        .filter(|e| e.get("actual_ssh_read_reconnect").is_some())
        .collect();
    ensure!(
        records.len() == 1,
        "one actual SSH read reconnect record is required"
    );
    let record = records[0];
    ensure!(
        record["actual_ssh_read_reconnect"] == true
            && record["reconnects"] == 1
            && record["forced_server_channel_close_after_each_reply"] == true
            && record["worker"]["protocol"] == "md-sim-worker/2"
            && record["worker"]["identity_before"].is_object()
            && record["worker"]["identity_before"] == record["worker"]["identity_after"]
            && record["transport"]["read_only_reconnects"] == 1,
        "actual SSH read reconnect evidence incomplete"
    );
    Ok(())
}

fn verify_fixture_report(client_events: &[Value], identity: &ObjectId) -> Result<()> {
    for key in [
        "actual_ssh_tunnel",
        "actual_mutual_tls",
        "actual_websocket_upgrade",
    ] {
        ensure!(
            client_events
                .iter()
                .filter(|event| event[key] == true)
                .count()
                == 1,
            "{key} evidence missing or duplicate"
        );
    }
    let tunnel = client_events
        .iter()
        .find(|event| event["actual_ssh_tunnel"] == true)
        .context("tunnel observation absent")?;
    let tls = client_events
        .iter()
        .find(|event| event["actual_mutual_tls"] == true)
        .context("TLS observation absent")?;
    let ws = client_events
        .iter()
        .find(|event| event["actual_websocket_upgrade"] == true)
        .context("websocket observation absent")?;
    for response in [&tunnel["response"], &tls["tls_response"], &tls["redirect"]] {
        ensure!(
            response["run_id"] == identity.sha256
                && response["execution_identity"] == serde_json::to_value(identity)?,
            "endpoint response refers to another execution"
        );
    }
    for event in [tls, ws] {
        ensure!(
            event["selected_run"] == identity.sha256,
            "selected endpoint run differs"
        );
        let headers = event["headers"]
            .as_str()
            .context("endpoint headers missing")?;
        require_upgrade_policy(headers)?;
        let ids = headers
            .lines()
            .filter(|line| line.to_ascii_lowercase().starts_with("x-md-sim-run:"))
            .collect::<Vec<_>>();
        ensure!(
            ids.len() == 1 && ids[0].split_whitespace().nth(1) == Some(identity.sha256.as_str()),
            "endpoint run header differs"
        );
    }
    ensure!(
        tls["selected_ip"] == WORKER,
        "selected publication IP differs"
    );
    for field in [
        "unauthenticated_rejected",
        "wrong_host_rejected",
        "wrong_origin_rejected",
        "wrong_client_cidr_rejected",
    ] {
        ensure!(tls[field] == true, "required rejection {field} absent");
    }
    for path in ["/stream", "/events"] {
        ensure!(
            client_events.iter().any(|event| event["path"] == path
                && event["early_delivery"] == true
                && event["first_line_before_completion_ms"]
                    .as_u64()
                    .is_some_and(|ms| ms >= 150)),
            "{path} early delivery was not observed"
        );
    }
    Ok(())
}

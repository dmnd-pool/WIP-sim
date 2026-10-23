//! Authenticated TLS to the actual selected process, through its unique socket.
//! This is an HTTP protocol fixture, not archived firmware acceptance.
use super::*;
use md_sim_access::gateway::{BrowserPolicy, GatewaySpec};
use md_sim_access::remote::{RemoteGateway, RemotePaths};
use md_sim_coordinator::{ExecutionScope, LockedRun, SuiteIdentity};
use md_sim_runtime::dashboard::{Agent, Binding, Source, Spec};

pub(super) fn probe() -> Result<()> {
    confinement::require_mining_execution()?;
    let listener = TcpListener::bind("127.0.0.1:18080")?;
    let _agent = Agent::start_from_environment()?.context("HTTP bridge absent")?;
    let (mut socket, peer) = listener.accept()?;
    ensure!(peer.ip().is_loopback(), "HTTP fixture peer differs");
    socket.set_read_timeout(Some(Duration::from_secs(10)))?;
    socket.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        ensure!(header.len() < 8192, "HTTP fixture header bound");
        let mut byte = [0];
        socket.read_exact(&mut byte)?;
        header.push(byte[0]);
    }
    let header = String::from_utf8(header)?;
    ensure!(
        header.starts_with("POST /proof HTTP/1.1\r\n"),
        "HTTP fixture request differs"
    );
    let lengths: Vec<_> = header
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
        .collect();
    ensure!(
        lengths.len() == 1,
        "HTTP fixture body length missing or duplicated"
    );
    let size: usize = lengths[0].1.trim().parse()?;
    ensure!(size == 512, "HTTP fixture body bound");
    let mut body = vec![0; size];
    socket.read_exact(&mut body)?;
    let mut fresh = [0; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut fresh)?;
    let proof = json!({"request_sha256":hex::encode(Sha256::digest(&body)),"guest_challenge":hex::encode(fresh)});
    let bytes = serde_json::to_vec(&proof)?;
    write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nX-Md-Sim-Run: forged-fixture-value\r\nConnection: close\r\n\r\n", bytes.len())?;
    socket.write_all(&bytes)?;
    socket.shutdown(std::net::Shutdown::Both)?;
    println!("MD_SIM_SELECTED_HTTP {proof}");
    std::io::stdout().flush()?;
    // The owner stops this actual process after checking TLS and correlation.
    loop {
        thread::sleep(Duration::from_millis(100));
    }
}

pub(super) fn check(
    base: &ProcessSpec,
    transport: &mut SshTransport,
    events: &mut Vec<Value>,
) -> Result<()> {
    let mut spec = base.clone();
    for path in ["/check", "/usr/lib/libgcc_s.so.1"] {
        spec.inputs.push(Mount {
            object: upload(transport, Path::new(path), events)?,
            guest_path: path.into(),
        });
    }
    spec.loopback = true;
    spec.dashboard = Some(Spec {
        source: Source::SimulatorConsole,
        guest_port: 18080,
        listen_port: 0,
    });
    spec.argv = vec!["/check".into(), "__selected-http".into()];
    spec.limits.memory_bytes = 512 << 20;
    spec.limits.wall_seconds = 120;
    spec.limits.trace_bytes = 65536;
    let started = request(
        transport,
        Request::ExecuteToken {
            token: "7b".repeat(32),
            spec: spec.clone(),
        },
        events,
    )?;
    let id = started["run_id"]
        .as_str()
        .context("selected HTTP run absent")?;
    let deadline = Instant::now() + Duration::from_secs(10);
    let binding: Binding = loop {
        let status = request(transport, Request::Status { run_id: id.into() }, events)?;
        if !status["dashboard"].is_null() {
            break serde_json::from_value(status["dashboard"].clone())?;
        }
        ensure!(
            Instant::now() < deadline && status["state"]["state"] != "failed",
            "selected HTTP did not start"
        );
        thread::sleep(Duration::from_millis(20));
    };
    binding.verify(
        spec.dashboard.as_ref().context("HTTP policy absent")?,
        &spec,
    )?;
    let reporter = hash_file(Path::new("/check"))?;
    let run = LockedRun {
        name: "selected-tls-transport-fixture".into(),
        scope: ExecutionScope::ProcessPrimitive {
            description: "Actual HTTP byte transport fixture; no firmware or mining gate.".into(),
        },
        process: Some(spec.clone()),
        suite: SuiteIdentity {
            id: "selected-tls".into(),
            version: "1".into(),
            suite: reporter.clone(),
            verifier: reporter,
            required_gates: ["actual-selected-tls".into()].into(),
        },
        inputs: spec
            .inputs
            .iter()
            .map(|input| (input.guest_path.clone(), input.object.clone()))
            .collect(),
        backend_tools: md_sim_coordinator::executor::local_tools()?,
        policies: json!({"fresh_request":true,"fixture_only":true}),
        capability_limits: vec!["Protocol fixture only; no native dashboard acceptance.".into()],
    };
    let gateway = GatewaySpec {
        run_id: id.into(),
        execution_identity: run.object()?,
        dashboard_source: "simulator-console".into(),
        listen: format!("{}:9443", config().host).parse()?,
        upstream: binding.listen,
        upstream_unix: binding.local_socket.clone(),
        upstream_tls: None,
        browser_policy: BrowserPolicy::Preserve,
        certificate: "/worker/access-credentials/server.crt".into(),
        private_key: "/worker/access-credentials/server.key".into(),
        client_ca: "/worker/access-credentials/ca.crt".into(),
        allow_clients: vec!["192.0.2.2/32".into()],
        browser_context_id: "7c".repeat(32),
        nginx: "/usr/bin/nginx".into(),
        openssl: "/usr/bin/openssl".into(),
    };
    // Keep the same firmware input bytes, but claim a different executable
    // command. The worker must reject that complete execution identity.
    let mut other = run.clone();
    other
        .process
        .as_mut()
        .context("fixture process absent")?
        .argv[1] = "__isolated-dashboard".into();
    let mut other_gateway = gateway.clone();
    other_gateway.execution_identity = other.object()?;
    let rejected = RemoteGateway::start(
        transport,
        &other,
        &other_gateway,
        RemotePaths {
            access_program: "/md-sim-access",
            store: Path::new("/worker/worker/store"),
            socket: Path::new("/worker/worker.sock"),
            output: Path::new("/worker/selected-gateway-wrong-execution"),
        },
        Path::new("/work/selected-gateway-wrong-execution"),
    );
    ensure!(
        rejected.is_err() && TcpStream::connect(gateway.listen).is_err(),
        "publication accepted another execution with shared input bytes"
    );
    let rejection = fs::read_to_string("/work/selected-gateway-wrong-execution/stderr.log")?;
    ensure!(
        rejection.contains("dashboard process identity differs"),
        "wrong-execution probe did not reach the worker's identity check"
    );
    let mut service = RemoteGateway::start(
        transport,
        &run,
        &gateway,
        RemotePaths {
            access_program: "/md-sim-access",
            store: Path::new("/worker/worker/store"),
            socket: Path::new("/worker/worker.sock"),
            output: Path::new("/worker/selected-gateway-test"),
        },
        Path::new("/work/selected-gateway"),
    )?;
    // Closing only the held SSH input must stop publication, while the
    // selected worker and its unique upstream remain available.
    let first_publication = service.manifest().clone();
    service.stop()?;
    let active = request(transport, Request::Status { run_id: id.into() }, events)?;
    ensure!(
        active["state"]["state"] == "booting"
            && active["dashboard"] == serde_json::to_value(&binding)?,
        "client disconnect stopped or changed the selected worker"
    );
    let mut service = RemoteGateway::start(
        transport,
        &run,
        &gateway,
        RemotePaths {
            access_program: "/md-sim-access",
            store: Path::new("/worker/worker/store"),
            socket: Path::new("/worker/worker.sock"),
            output: Path::new("/worker/selected-gateway-reconnect"),
        },
        Path::new("/work/selected-gateway-reconnect"),
    )?;
    let mut fresh = [0; 512];
    fs::File::open("/dev/urandom")?.read_exact(&mut fresh)?;
    fs::write("/work/selected-http-request", fresh)?;
    let curl = |auth: bool| -> Result<std::process::Output> {
        let mut command = Command::new("/usr/bin/curl");
        command.env_clear().args([
            "--noproxy",
            "*",
            "--silent",
            "--show-error",
            "--fail",
            "--max-time",
            "10",
            "--cacert",
            "/credentials/ca.crt",
            "--dump-header",
            "/work/selected-http.headers",
            "--data-binary",
            "@/work/selected-http-request",
        ]);
        if auth {
            command.args([
                "--cert",
                "/credentials/client.crt",
                "--key",
                "/credentials/client.key",
            ]);
        }
        Ok(command
            .arg(format!("{}proof", gateway.public_url()))
            .output()?)
    };
    let denied = curl(false)?;
    ensure!(
        !denied.status.success(),
        "selected gateway accepted client without certificate"
    );
    let response = curl(true)?;
    ensure!(
        response.status.success(),
        "selected authenticated HTTP failed: {}",
        String::from_utf8_lossy(&response.stderr)
    );
    let proof: Value = serde_json::from_slice(&response.stdout)?;
    ensure!(
        proof["request_sha256"] == hex::encode(Sha256::digest(fresh)),
        "selected HTTP body changed"
    );
    let headers = fs::read_to_string("/work/selected-http.headers")?;
    let runs: Vec<_> = headers
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(key, _)| key.eq_ignore_ascii_case("x-md-sim-run"))
        .map(|(_, value)| value.trim())
        .collect();
    ensure!(
        runs == [id],
        "selected gateway trusted forged firmware identity header"
    );
    request(transport, Request::Stop { run_id: id.into() }, events)?;
    let result = collect(transport, id, events)?;
    ensure!(
        result.status == "cancelled" && !result.trace_truncated,
        "selected HTTP process did not stop"
    );
    let log = String::from_utf8(hex::decode(&result.stdout)?)?;
    let records: Vec<_> = log
        .lines()
        .filter_map(|line| line.strip_prefix("MD_SIM_SELECTED_HTTP "))
        .collect();
    ensure!(
        records.len() == 1 && serde_json::from_str::<Value>(records[0])? == proof,
        "TLS reached another process"
    );
    service.wait_stopped()?;
    let manifest = service.manifest();
    ensure!(
        manifest["run_id"] == id
            && manifest["locked_run"] == serde_json::to_value(run.object()?)?
            && manifest["binding"] == serde_json::to_value(&binding)?,
        "publication identity differs"
    );
    events.push(json!({"selected_gateway_tls":{"selected_run":id,"locked_run":run,"binding":binding,"publication":manifest,
        "result":result,"proof":proof,"headers":headers,"no_certificate_rejected":true,"automatic_stop":true,
        "unique_unix_upstream":true,"native_firmware_gate":false,
        "client_disconnect_stopped_publication":true,"first_publication":first_publication,
        "wrong_execution_rejected_by_worker":rejection}}));
    Ok(())
}

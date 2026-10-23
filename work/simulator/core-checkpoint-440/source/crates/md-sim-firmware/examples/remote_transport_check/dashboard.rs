//! Actual selected-process byte forwarding, including a client SSH tunnel.
//! This transport fixture is not firmware or a native dashboard acceptance test.
use super::*;
use md_sim_coordinator::ssh_transport::SshTunnel;
use md_sim_runtime::dashboard::{Agent, Binding, Source, Spec};
use std::net::Shutdown;
use std::os::{
    fd::{AsRawFd, OwnedFd},
    unix::net::UnixStream,
};

pub(super) fn probe(cancel: bool) -> Result<()> {
    loopback::probe()?;
    ensure!(
        md_sim_runtime::dashboard::guest_mount_expected()?,
        "runtime socket mount absent"
    );
    if cancel {
        // This separate SSH run executes as worker UID1000 and therefore
        // checks the full firmware guard. The generic local transport fixture
        // runs as the VM root and has no firmware-isolation acceptance claim.
        confinement::require_mining_execution()?;
    }
    let listener = TcpListener::bind("127.0.0.1:18080")?;
    listener.set_nonblocking(true)?;
    let agent = Agent::start_from_environment()?.context("dashboard adapter absent")?;
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut socket = loop {
        match listener.accept() {
            Ok((socket, peer)) => {
                ensure!(peer.ip().is_loopback(), "private peer differs");
                break socket;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                ensure!(
                    Instant::now() < deadline,
                    "dashboard fixture accept deadline"
                );
                thread::sleep(Duration::from_millis(10));
            }
            Err(e) => return Err(e.into()),
        }
    };
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    socket.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut prefix = [0; 32];
    socket.read_exact(&mut prefix)?;
    socket.write_all(&Sha256::digest(prefix))?;
    if cancel {
        println!(
            "MD_SIM_DASHBOARD_OPEN {}",
            hex::encode(Sha256::digest(prefix))
        );
        std::io::stdout().flush()?;
        // Keep the actual upstream connection open until the selected worker
        // is stopped. The client must first receive its independent digest.
        let mut byte = [0];
        socket.set_read_timeout(Some(Duration::from_secs(20)))?;
        let _ = socket.read(&mut byte)?;
        anyhow::bail!("cancellation fixture was not stopped with its open stream");
    }
    let mut bytes = prefix.to_vec();
    std::io::Read::by_ref(&mut socket)
        .take(65537)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() == 65568, "dashboard fixture body length");
    let digest = Sha256::digest(&bytes);
    let mut challenge = [0; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut challenge)?;
    socket.write_all(&digest)?;
    socket.write_all(&challenge)?;
    socket.shutdown(Shutdown::Write)?;
    drop(socket);
    while agent.completed_connections() != 1 {
        ensure!(
            Instant::now() < deadline,
            "dashboard transport did not complete"
        );
        thread::sleep(Duration::from_millis(10));
    }
    println!(
        "MD_SIM_DASHBOARD_STREAM {}",
        json!({"request_sha256":hex::encode(digest),"guest_challenge":hex::encode(challenge),"request_bytes":bytes.len(),"early_response_before_request_eof":true,"completed_connections":agent.completed_connections()})
    );
    Ok(())
}

fn exchange(address: std::net::SocketAddr) -> Result<Value> {
    let client = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
    client.set_read_timeout(Some(Duration::from_secs(5)))?;
    client.set_write_timeout(Some(Duration::from_secs(5)))?;
    exchange_fd(client.into())
}
fn exchange_unix(path: &Path) -> Result<Value> {
    let client = UnixStream::connect(path)?;
    client.set_read_timeout(Some(Duration::from_secs(5)))?;
    client.set_write_timeout(Some(Duration::from_secs(5)))?;
    exchange_fd(client.into())
}
fn exchange_fd(fd: OwnedFd) -> Result<Value> {
    let mut bytes = vec![0u8; 65568];
    fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    let mut client = fs::File::from(fd);
    client.write_all(&bytes[..32])?;
    let mut early = [0; 32];
    client.read_exact(&mut early)?;
    ensure!(
        early[..] == Sha256::digest(&bytes[..32])[..],
        "early dashboard response differs"
    );
    client.write_all(&bytes[32..])?;
    ensure!(
        unsafe { libc::shutdown(client.as_raw_fd(), libc::SHUT_WR) } == 0,
        "request half-close failed"
    );
    let mut result = Vec::new();
    client.take(65).read_to_end(&mut result)?;
    ensure!(
        result.len() == 64 && result[..32] == Sha256::digest(&bytes)[..],
        "dashboard body or half-close differs"
    );
    Ok(
        json!({"request_sha256":hex::encode(Sha256::digest(&bytes)),"guest_challenge":hex::encode(&result[32..]),"request_bytes":bytes.len(),"early_response_before_request_eof":true,"completed_connections":1}),
    )
}

pub(super) fn check(
    store: &Store,
    base: &ProcessSpec,
    transport: &mut SshTransport,
    events: &mut Vec<Value>,
) -> Result<()> {
    let mut spec = base.clone();
    for path in ["/check", "/usr/lib/libgcc_s.so.1"] {
        let object = upload(transport, Path::new(path), events)?;
        store.import(Path::new(path), &object)?;
        spec.inputs.push(Mount {
            object,
            guest_path: path.into(),
        });
    }
    spec.loopback = true;
    spec.dashboard = Some(Spec {
        source: Source::SimulatorConsole,
        guest_port: 18080,
        listen_port: 0,
    });
    spec.argv = vec!["/check".into(), "__isolated-dashboard".into()];
    spec.limits.wall_seconds = 30;
    spec.limits.memory_bytes = 512 << 20;
    spec.limits.trace_bytes = 65536;
    for (unix, name) in [
        (false, "dashboard_stream_local"),
        (true, "dashboard_stream_unix"),
    ] {
        let receipt_path = format!("/work/{name}.json");
        let receipt = Path::new(&receipt_path);
        let cancel = Arc::new(AtomicBool::new(false));
        let local = thread::scope(|scope| -> Result<(ProcessResult, Result<Value>, Binding)> {
            let join = scope.spawn(|| {
                isolation::execute_dashboard_recorded(
                    store,
                    &spec,
                    cancel.clone(),
                    None,
                    None,
                    None,
                    Some(receipt),
                )
            });
            let deadline = Instant::now() + Duration::from_secs(5);
            while !receipt.exists() {
                ensure!(
                    !join.is_finished() && Instant::now() < deadline,
                    "local dashboard binding absent"
                );
                thread::sleep(Duration::from_millis(10));
            }
            let binding: Binding = serde_json::from_slice(&fs::read(receipt)?)?;
            binding.verify(
                spec.dashboard.as_ref().context("dashboard policy absent")?,
                &spec,
            )?;
            let expected = if unix {
                exchange_unix(
                    binding
                        .local_socket
                        .as_deref()
                        .context("run Unix socket absent")?,
                )
            } else {
                exchange(binding.listen)
            };
            if expected.is_err() {
                cancel.store(true, std::sync::atomic::Ordering::Release);
            }
            let result = join
                .join()
                .map_err(|_| anyhow::anyhow!("local dashboard runner panicked"))??;
            Ok((result, expected, binding))
        })?;
        if let Err(error) = &local.1 {
            events.push(json!({"dashboard_local_failure":{"result":local.0,"binding":local.2,"error":format!("{error:#}")}}));
        }
        let expected = local.1?;
        dashboard_transport_report::verify_stream(&local.0, &expected)?;
        ensure!(
            TcpStream::connect(local.2.listen).is_err(),
            "local listener survived completion"
        );
        let socket = local
            .2
            .local_socket
            .as_deref()
            .context("run Unix socket absent")?;
        ensure!(
            !socket.exists() && UnixStream::connect(socket).is_err(),
            "run Unix socket survived completion"
        );
        events.push(json!({name:{"result":local.0,"observation":expected,"binding":local.2}}));
    }

    let started = request(
        transport,
        Request::ExecuteToken {
            token: "79".repeat(32),
            spec: spec.clone(),
        },
        events,
    )?;
    let id = started["run_id"]
        .as_str()
        .context("dashboard worker run absent")?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let binding: Binding = loop {
        let status = request(transport, Request::Status { run_id: id.into() }, events)?;
        if !status["dashboard"].is_null() {
            break serde_json::from_value(status["dashboard"].clone())?;
        }
        if status["state"]["state"] == "failed" {
            let failure = request(transport, Request::Result { run_id: id.into() }, events)?;
            anyhow::bail!("worker dashboard startup failed: {}", failure["result"]);
        }
        ensure!(Instant::now() < deadline, "worker dashboard binding absent");
        thread::sleep(Duration::from_millis(20));
    };
    binding.verify(
        spec.dashboard.as_ref().context("dashboard policy absent")?,
        &spec,
    )?;
    let listen = "127.0.0.1:18089".parse()?;
    let mut tunnel = SshTunnel::start(
        config(),
        listen,
        binding.listen,
        Path::new("/work/selected-dashboard-tunnel.log"),
    )?;
    let expected = exchange(listen)?;
    let remote = collect(transport, id, events)?;
    dashboard_transport_report::verify_stream(&remote, &expected)?;
    let tunnel_evidence = tunnel.evidence();
    tunnel.stop()?;
    ensure!(
        TcpStream::connect(listen).is_err(),
        "selected dashboard tunnel survived stop"
    );
    events.push(json!({"dashboard_stream_ssh":{"selected_run":id,"binding":binding,"result":remote,"observation":expected,"tunnel":tunnel_evidence,"same_process_specification":true,"native_firmware_gate":false}}));
    cancel_stream(&spec, transport, events)?;
    Ok(())
}

fn cancel_stream(
    base: &ProcessSpec,
    transport: &mut SshTransport,
    events: &mut Vec<Value>,
) -> Result<()> {
    let mut spec = base.clone();
    spec.argv[1] = "__isolated-dashboard-cancel".into();
    let started = request(
        transport,
        Request::ExecuteToken {
            token: "7a".repeat(32),
            spec: spec.clone(),
        },
        events,
    )?;
    let id = started["run_id"]
        .as_str()
        .context("cancellation run absent")?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let binding: Binding = loop {
        let status = request(transport, Request::Status { run_id: id.into() }, events)?;
        if !status["dashboard"].is_null() {
            break serde_json::from_value(status["dashboard"].clone())?;
        }
        ensure!(
            status["state"]["state"] != "failed" && Instant::now() < deadline,
            "cancellation dashboard did not start"
        );
        thread::sleep(Duration::from_millis(20));
    };
    binding.verify(
        spec.dashboard.as_ref().context("dashboard policy absent")?,
        &spec,
    )?;
    let listen = "127.0.0.1:18089".parse()?;
    let mut tunnel = SshTunnel::start(
        config(),
        listen,
        binding.listen,
        Path::new("/work/cancel-dashboard-tunnel.log"),
    )?;
    let mut socket = TcpStream::connect_timeout(&listen, Duration::from_secs(5))?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    socket.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut challenge = [0; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut challenge)?;
    socket.write_all(&challenge)?;
    let mut response = [0; 32];
    socket.read_exact(&mut response)?;
    ensure!(
        response[..] == Sha256::digest(challenge)[..],
        "cancellation stream did not reach selected guest"
    );
    let began = Instant::now();
    request(transport, Request::Stop { run_id: id.into() }, events)?;
    let result = collect(transport, id, events)?;
    ensure!(
        result.status == "cancelled" && !result.trace_truncated,
        "dashboard worker was not cancelled"
    );
    let dashboard = result
        .dashboard
        .as_ref()
        .context("cancelled dashboard receipt absent")?;
    ensure!(
        dashboard["listener_closed"] == true && dashboard["forwarded_bytes"] == 64,
        "cancelled dashboard lifecycle differs"
    );
    match socket.read(&mut [0]) {
        Ok(0) => {}
        Err(e)
            if matches!(
                e.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
            ) => {}
        other => anyhow::bail!("dashboard stream survived cancellation: {other:?}"),
    }
    let tunnel_evidence = tunnel.evidence();
    tunnel.stop()?;
    ensure!(
        began.elapsed() < Duration::from_secs(10) && TcpStream::connect(listen).is_err(),
        "dashboard cancellation or tunnel cleanup exceeded bound"
    );
    events.push(json!({"dashboard_cancel_ssh":{"selected_run":id,"binding":binding,"result":result,"challenge_sha256":hex::encode(response),"open_stream_reached_guest":true,"stream_closed_after_stop":true,"elapsed_ms":began.elapsed().as_millis(),"tunnel":tunnel_evidence,"native_firmware_gate":false}}));
    Ok(())
}

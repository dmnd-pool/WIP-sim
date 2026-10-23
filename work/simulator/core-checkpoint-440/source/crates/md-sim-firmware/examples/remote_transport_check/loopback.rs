//! Actual TCP inside the process sandbox, reached locally and through SSH.
use super::*;

pub(super) fn probe() -> Result<()> {
    let pid: u32 = fs::read_to_string("/proc/self/stat")?
        .split_whitespace()
        .next()
        .context("proc PID absent")?
        .parse()?;
    ensure!(pid == std::process::id(), "proc PID mismatch");
    let interfaces: Vec<String> = fs::read_to_string("/proc/net/dev")?
        .lines()
        .skip(2)
        .map(|line| line.split(':').next().unwrap_or("").trim().to_owned())
        .collect();
    ensure!(interfaces == ["lo"], "non-loopback interface in sandbox");
    for line in fs::read_to_string("/proc/net/route")?.lines().skip(1) {
        let fields: Vec<_> = line.split_whitespace().collect();
        ensure!(
            fields.get(1) != Some(&"00000000"),
            "default route in sandbox"
        );
    }
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let address = listener.local_addr()?;
    let mut challenge = [0; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut challenge)?;
    let server = thread::spawn(move || -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, peer)) => {
                    ensure!(peer.ip().is_loopback(), "non-loopback peer");
                    break socket;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    ensure!(Instant::now() < deadline, "private accept deadline");
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => return Err(e.into()),
            }
        };
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.set_write_timeout(Some(Duration::from_secs(5)))?;
        let mut bytes = [0; 32];
        socket.read_exact(&mut bytes)?;
        socket.write_all(&bytes)?;
        Ok(())
    });
    let exchange = (|| -> Result<()> {
        let mut socket = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.set_write_timeout(Some(Duration::from_secs(5)))?;
        socket.write_all(&challenge)?;
        let mut observed = [0; 32];
        socket.read_exact(&mut observed)?;
        ensure!(observed == challenge, "private TCP bytes differ");
        Ok(())
    })();
    server
        .join()
        .map_err(|_| anyhow::anyhow!("private server panic"))??;
    exchange?;
    println!(
        "{}",
        json!({"actual_roundtrip":true,"challenge_hex":hex::encode(challenge),"listener":address,"interfaces":interfaces,"proc_pid":pid,"net_namespace":fs::read_link("/proc/self/ns/net")?})
    );
    Ok(())
}

fn verify(result: &ProcessResult) -> Result<Value> {
    ensure!(
        result.status == "exited"
            && result.exit_code == Some(0)
            && !result.trace_truncated
            && result.stderr.is_empty(),
        "private loopback process failed: {}",
        String::from_utf8_lossy(&hex::decode(&result.stderr)?)
    );
    let record: Value = serde_json::from_slice(&hex::decode(&result.stdout)?)?;
    ensure!(
        record["actual_roundtrip"] == true
            && record["interfaces"] == json!(["lo"])
            && record["proc_pid"].as_u64().is_some_and(|n| n > 0)
            && record["challenge_hex"]
                .as_str()
                .is_some_and(|s| hex::decode(s).is_ok_and(|b| b.len() == 32)),
        "private TCP evidence differs"
    );
    Ok(record)
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
        ensure!(
            !spec.inputs.iter().any(|input| input.guest_path == path),
            "duplicate loopback input"
        );
        spec.inputs.push(Mount {
            object,
            guest_path: path.into(),
        });
    }
    spec.argv = vec!["/check".into(), "__isolated-loopback".into()];
    let disabled = isolation::execute(store, &spec, Arc::new(AtomicBool::new(false)))?;
    let error = String::from_utf8(hex::decode(&disabled.stderr)?)?;
    ensure!(
        disabled.status == "exited"
            && disabled.exit_code == Some(1)
            && !disabled.trace_truncated
            && error.contains("private accept deadline"),
        "disabled loopback did not fail at the actual TCP listener: {error}"
    );
    events.push(json!({"disabled_loopback_expected_failure":disabled}));
    spec.loopback = true;
    let local = isolation::execute(store, &spec, Arc::new(AtomicBool::new(false)))?;
    events.push(json!({"private_loopback_local_result":local,"process_spec":spec}));
    let local_record = verify(&local)?;
    ensure!(
        local_record["net_namespace"] != json!(fs::read_link("/proc/self/ns/net")?),
        "local network namespace is shared"
    );
    let started = request(
        transport,
        Request::ExecuteToken {
            token: "78".repeat(32),
            spec,
        },
        events,
    )?;
    let id = started["run_id"].as_str().context("loopback run absent")?;
    let remote = collect(transport, id, events)?;
    let remote_record = verify(&remote)?;
    events.push(json!({"private_loopback":{"local":local_record,"ssh":remote_record,"same_process_spec":true,"worker_run":id,"firmware_gate":false}}));
    Ok(())
}

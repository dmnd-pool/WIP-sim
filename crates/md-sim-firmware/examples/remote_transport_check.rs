//! Real OpenSSH transport integration, executed only inside the private Linux VM.
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::{
    ssh_transport::{SshConfig, SshTransport},
    WorkerTransport,
};
use md_sim_runtime::{
    isolation::{self, Limits, Mount, ProcessResult, ProcessSpec},
    store::{hash_file, ObjectId, Store, MAX_CHUNK},
    worker::{LogStream, Request},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
    thread,
    time::{Duration, Instant},
};
#[path = "remote_transport_check/artifact.rs"]
mod artifact;
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "remote_transport_check/dashboard.rs"]
mod dashboard;
#[path = "support/dashboard_transport_report.rs"]
mod dashboard_transport_report;
#[path = "remote_transport_check/live.rs"]
mod live;
#[path = "remote_transport_check/loopback.rs"]
mod loopback;
#[path = "remote_transport_check/selected_gateway.rs"]
mod selected_gateway;
#[path = "support/ssh_artifact_ack_loss.rs"]
mod ssh_artifact_ack_loss;
#[path = "support/ssh_one_reply.rs"]
mod ssh_one_reply;
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn run_command(program: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()?;
    ensure!(
        output.status.success(),
        "{program} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
fn start(program: &str, args: &[&str], log: &str) -> Result<Owned> {
    let output = fs::File::create(log)?;
    Ok(Owned(
        Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(output.try_clone()?)
            .stderr(output)
            .spawn()?,
    ))
}
fn config() -> SshConfig {
    SshConfig {
        host: std::env::var("MD_SIM_WORKER_IP").unwrap_or_else(|_| "127.0.0.1".into()),
        user: std::env::var("MD_SIM_WORKER_USER").unwrap_or_else(|_| "root".into()),
        port: 2222,
        known_hosts: "/work/known_hosts".into(),
        identity_file: "/work/clientkey".into(),
        remote_relay: "/md-sim-worker-relay".into(),
        worker_socket: std::env::var("MD_SIM_WORKER_SOCKET")
            .unwrap_or_else(|_| "/worker.sock".into()),
        ssh_program: "/usr/bin/ssh".into(),
        timeout_seconds: 30,
    }
}
fn request(transport: &mut SshTransport, req: Request, events: &mut Vec<Value>) -> Result<Value> {
    let response = transport.request(&req)?;
    let operation = serde_json::to_value(&req)?["operation"].clone();
    events.push(json!({"operation":operation,"response":response}));
    Ok(response)
}
fn upload(transport: &mut SshTransport, path: &Path, events: &mut Vec<Value>) -> Result<ObjectId> {
    let object = hash_file(path)?;
    let begin = request(
        transport,
        Request::Begin {
            object: object.clone(),
        },
        events,
    )?;
    let mut offset = begin["offset"].as_u64().context("missing offset")?;
    let bytes = fs::read(path)?;
    ensure!(offset <= bytes.len() as u64, "remote offset exceeds input");
    while offset < object.size {
        let end = (offset as usize + MAX_CHUNK).min(bytes.len());
        let ack = transport.request(&Request::Chunk {
            object: object.clone(),
            offset,
            hex: hex::encode(&bytes[offset as usize..end]),
        })?;
        ensure!(
            ack["offset"].as_u64() == Some(end as u64),
            "remote chunk acknowledgement differs"
        );
        offset = end as u64;
    }
    let done = request(
        transport,
        Request::Finish {
            object: object.clone(),
        },
        events,
    )?;
    ensure!(
        serde_json::from_value::<ObjectId>(done["verified"].clone())? == object,
        "destination digest differs"
    );
    Ok(object)
}
fn collect(
    transport: &mut SshTransport,
    id: &str,
    events: &mut Vec<Value>,
) -> Result<ProcessResult> {
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        let state = request(transport, Request::Status { run_id: id.into() }, events)?;
        if state["state"]["state"] != "booting" {
            break;
        }
        ensure!(Instant::now() < deadline, "worker completion deadline");
        thread::sleep(Duration::from_millis(20));
    }
    let summary = request(transport, Request::Result { run_id: id.into() }, events)?;
    let identity: ObjectId = serde_json::from_value(summary["result_identity"].clone())?;
    ensure!(
        identity.size < 1024 * 1024,
        "worker result exceeds test bound"
    );
    let mut bytes = Vec::new();
    while bytes.len() < identity.size as usize {
        let part = transport.request(&Request::ResultData {
            run_id: id.into(),
            result: identity.clone(),
            offset: bytes.len() as u64,
            count: MAX_CHUNK,
        })?;
        let data = hex::decode(part["hex"].as_str().context("missing result bytes")?)?;
        ensure!(
            !data.is_empty()
                && data.len() <= MAX_CHUNK
                && bytes.len() + data.len() <= identity.size as usize,
            "invalid result chunk"
        );
        bytes.extend(data);
    }
    ensure!(
        hex::encode(Sha256::digest(&bytes)) == identity.sha256,
        "result digest mismatch"
    );
    let path = Path::new("/work").join(format!("{id}-result.json"));
    let mut file = fs::File::create(&path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    fs::File::open("/work")?.sync_all()?;
    let result: ProcessResult = serde_json::from_slice(&bytes)?;
    events.push(json!({"run_id":id,"saved_result":identity,"actual_process":result}));
    let clean = request(
        transport,
        Request::Cleanup {
            run_id: id.into(),
            result: identity,
        },
        events,
    )?;
    ensure!(clean["removed"] == true, "worker cleanup failed");
    Ok(result)
}
fn test(events: &mut Vec<Value>) -> Result<()> {
    ensure!(
        fs::read_to_string("/proc/cmdline")?.contains("md_sim_private_guest=1"),
        "test requires private VM kernel marker"
    );
    ensure!(
        fs::read_to_string("/proc/sys/kernel/osrelease")?.trim() == "7.2.9-arch1-1",
        "unexpected guest kernel"
    );
    for directory in [
        "/etc",
        "/run",
        "/var/empty",
        "/usr/share/empty.sshd",
        "/work",
    ] {
        fs::create_dir_all(directory)?;
    }
    for operation in ["chunk", "finish"] {
        fs::copy(
            std::env::current_exe()?,
            format!("/drop-{operation}-ack-relay"),
        )?;
    }
    fs::write("/etc/passwd","root::0:0:VM test root:/work:/usr/bin/sh\nsshd:x:74:74:SSH privilege separation:/var/empty:/usr/bin/sh\nnobody:x:65534:65534:SSH privilege separation:/var/empty:/usr/bin/sh\n")?;
    fs::write("/etc/group", "root:x:0:\nsshd:x:74:\nnobody:x:65534:\n")?;
    fs::write(
        "/etc/nsswitch.conf",
        "passwd: files\ngroup: files\nhosts: files\n",
    )?;
    fs::write("/etc/hosts", "127.0.0.1 localhost\n")?;
    run_command(
        "/usr/bin/ssh-keygen",
        &["-q", "-t", "ed25519", "-N", "", "-f", "/work/hostkey"],
    )?;
    run_command(
        "/usr/bin/ssh-keygen",
        &["-q", "-t", "ed25519", "-N", "", "-f", "/work/clientkey"],
    )?;
    let hostkey = fs::read_to_string("/work/hostkey.pub")?;
    let mut fields = hostkey.split_whitespace();
    let kind = fields.next().context("host key kind missing")?;
    let key = fields.next().context("host key absent")?;
    fs::write(
        "/work/known_hosts",
        format!("[127.0.0.1]:2222 {kind} {key}\n"),
    )?;
    fs::set_permissions("/work/clientkey", fs::Permissions::from_mode(0o600))?;
    fs::write("/work/sshd_config","Port 2222\nListenAddress 127.0.0.1\nHostKey /work/hostkey\nPidFile /work/sshd.pid\nAuthorizedKeysFile /work/clientkey.pub\nStrictModes yes\nPasswordAuthentication no\nKbdInteractiveAuthentication no\nPubkeyAuthentication yes\nPermitRootLogin prohibit-password\nUsePAM no\nUseDNS no\nPrintMotd no\nLogLevel VERBOSE\nAllowUsers root\n")?;
    let mut worker = start(
        "/md-sim",
        &[
            "--state-dir",
            "/worker",
            "worker",
            "serve",
            "--socket",
            "/worker.sock",
        ],
        "/work/worker.log",
    )?;
    let mut daemon = start(
        "/usr/bin/sshd",
        &["-D", "-e", "-f", "/work/sshd_config"],
        "/work/sshd.log",
    )?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if Path::new("/worker.sock").exists() && TcpStream::connect("127.0.0.1:2222").is_ok() {
            break;
        }
        ensure!(
            worker.0.try_wait()?.is_none() && daemon.0.try_wait()?.is_none(),
            "worker or SSH daemon exited before readiness"
        );
        ensure!(
            Instant::now() < deadline,
            "worker or SSH daemon readiness deadline"
        );
        thread::sleep(Duration::from_millis(50));
    }
    client_test(events)?;
    ensure!(worker.0.wait()?.success(), "worker shutdown failed");
    Ok(())
}
fn client_test(events: &mut Vec<Value>) -> Result<()> {
    ensure!(
        fs::read_to_string("/proc/cmdline")?.contains("md_sim_private_guest=1"),
        "client test requires the private guest"
    );
    let mut transport = SshTransport::new_streaming(config())?;
    events.push(json!({"ssh_channel":transport.evidence()}));
    let client_public = fs::read_to_string("/work/clientkey.pub")?;
    let fields = client_public.split_whitespace().take(2).collect::<Vec<_>>();
    fs::write(
        "/work/wrong_known_hosts",
        format!("[{}]:2222 {} {}\n", config().host, fields[0], fields[1]),
    )?;
    let mut wrong = config();
    wrong.known_hosts = "/work/wrong_known_hosts".into();
    let error = SshTransport::new(wrong)?
        .request(&Request::Probe)
        .expect_err("incorrect SSH host key was accepted");
    events.push(json!({"host_key_mismatch_rejected":true,"error":error.to_string()}));
    let stalled = TcpListener::bind("127.0.0.1:0")?;
    let port = stalled.local_addr()?.port();
    let hold = thread::spawn(move || -> Result<()> {
        let (_stream, _) = stalled.accept()?;
        thread::sleep(Duration::from_secs(3));
        Ok(())
    });
    let mut timeout = config();
    timeout.host = "127.0.0.1".into();
    timeout.port = port;
    timeout.timeout_seconds = 1;
    let started = Instant::now();
    ensure!(
        SshTransport::new(timeout)?
            .request(&Request::Probe)
            .is_err(),
        "stalled SSH handshake did not fail"
    );
    ensure!(
        started.elapsed() < Duration::from_secs(3),
        "stalled SSH handshake exceeded bounded deadline"
    );
    hold.join()
        .map_err(|_| anyhow::anyhow!("stalled peer thread panicked"))??;
    events.push(json!({"unresponsive_ssh_handshake_rejected":true,"no_execute_request_sent":true}));
    let probe = request(&mut transport, Request::Probe, events)?;
    ensure!(
        probe["doctor"]["tcp_loopback"]["available"] == true
            && probe["doctor"]["namespaces"]["available"] == true,
        "guest worker prerequisites missing"
    );
    events.push(transport.evidence());
    artifact::check(events)?;
    // A new SSH connection resumes the same unfinished artifact by exact hash.
    let mut fresh = vec![0_u8; 8192];
    fs::File::open("/dev/urandom")?.read_exact(&mut fresh)?;
    let object = ObjectId {
        sha256: hex::encode(Sha256::digest(&fresh)),
        size: fresh.len() as u64,
    };
    request(
        &mut transport,
        Request::Begin {
            object: object.clone(),
        },
        events,
    )?;
    request(
        &mut transport,
        Request::Chunk {
            object: object.clone(),
            offset: 0,
            hex: hex::encode(&fresh[..4096]),
        },
        events,
    )?;
    transport = SshTransport::new_streaming(config())?;
    ensure!(
        request(
            &mut transport,
            Request::Begin {
                object: object.clone()
            },
            events
        )?["offset"]
            == 4096,
        "artifact resume offset differs"
    );
    request(
        &mut transport,
        Request::Chunk {
            object: object.clone(),
            offset: 4096,
            hex: hex::encode(&fresh[4096..]),
        },
        events,
    )?;
    ensure!(
        request(
            &mut transport,
            Request::Finish {
                object: object.clone()
            },
            events
        )?["verified"]
            == serde_json::to_value(&object)?,
        "resumed artifact hash differs"
    );
    let got = request(
        &mut transport,
        Request::Get {
            object: object.clone(),
            offset: 0,
            count: fresh.len(),
        },
        events,
    )?;
    ensure!(
        hex::decode(got["hex"].as_str().context("missing artifact")?)? == fresh,
        "retrieved artifact differs"
    );
    let local = Store::open(Path::new("/client-store"), 32 * 1024 * 1024)?;
    let mut inputs = Vec::new();
    let closure: Vec<Mount> = serde_json::from_slice(&fs::read("/fixture-inputs.json")?)?;
    for input in closure {
        let path = Path::new(&input.guest_path);
        ensure!(
            hash_file(path)? == input.object,
            "fixture dependency differs from prepared closure"
        );
        let object = upload(&mut transport, path, events)?;
        local.import(path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: input.guest_path,
        });
    }
    let mut spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None, stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        stdin_object: None,
        argv: vec![
            "/usr/bin/sh".into(),
            "-c".into(),
            "read proc_pid rest < /proc/self/stat; test \"$$\" = \"$proc_pid\" && test ! -e /work/clientkey && printf 'actual SSH worker process\\n'".into(),
        ],
        inputs,
        environment: BTreeMap::new(),
        limits: Limits {
            trace_bytes: 4096,
            ..Limits::default()
        },
    };
    let local_result = isolation::execute(&local, &spec, Arc::new(AtomicBool::new(false)))?;
    events.push(json!({"local_baseline":local_result,"exact_process_spec":spec}));
    ensure!(
        local_result.exit_code == Some(0),
        "local baseline process failed"
    );
    let token = "12".repeat(32);
    let started = request(
        &mut transport,
        Request::ExecuteToken {
            token: token.clone(),
            spec: spec.clone(),
        },
        events,
    )?;
    let id = started["run_id"]
        .as_str()
        .context("missing run id")?
        .to_string();
    transport = SshTransport::new_streaming(config())?;
    ensure!(
        request(
            &mut transport,
            Request::LookupToken {
                token: token.clone()
            },
            events
        )?["run_id"]
            == id,
        "reconnected worker identity differs"
    );
    let remote = collect(&mut transport, &id, events)?;
    ensure!(
        remote.exit_code == Some(0)
            && remote.status == "exited"
            && !remote.trace_truncated
            && remote.stdout == local_result.stdout
            && remote.stderr == local_result.stderr,
        "local/SSH process results differ"
    );
    ensure!(
        request(&mut transport, Request::LookupToken { token }, events)?["state"] == "cleaned",
        "cleanup receipt missing"
    );
    // Capture the actual nested worker mounts. A successful shell alone cannot
    // establish that a firmware guard accepts this kernel's proc protections.
    let mut mount_probe = spec.clone();
    mount_probe.argv[2] =
        "while IFS= read -r line; do printf '%s\\n' \"$line\"; done < /proc/self/mountinfo".into();
    mount_probe.limits.trace_bytes = 65536;
    let started = request(
        &mut transport,
        Request::ExecuteToken {
            token: "56".repeat(32),
            spec: mount_probe,
        },
        events,
    )?;
    let mount_run = started["run_id"]
        .as_str()
        .context("mount probe run absent")?;
    let mounts = collect(&mut transport, mount_run, events)?;
    ensure!(
        mounts.exit_code == Some(0) && !mounts.trace_truncated && mounts.stderr.is_empty(),
        "mount probe incomplete"
    );
    events.push(json!({"worker_mount_probe":{"run_id":mount_run,"mountinfo":String::from_utf8(hex::decode(&mounts.stdout)?)?,"scope":"Actual nested process mounts; no firmware gate."}}));

    loopback::check(&local, &spec, &mut transport, events)?;
    dashboard::check(&local, &spec, &mut transport, events)?;
    selected_gateway::check(&spec, &mut transport, events)?;
    live::check(&local, &spec, events)?;
    spec.argv[2] = "printf 'actual cancellation process\\n'; while :; do :; done".into();
    let started = request(
        &mut transport,
        Request::ExecuteToken {
            token: "34".repeat(32),
            spec,
        },
        events,
    )?;
    let id = started["run_id"]
        .as_str()
        .context("missing cancellation run")?
        .to_string();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let logs = request(
            &mut transport,
            Request::Logs {
                run_id: id.clone(),
                stream: LogStream::Stdout,
                offset: 0,
                count: 4096,
            },
            events,
        )?;
        if hex::decode(logs["hex"].as_str().context("missing live log")?)?
            == b"actual cancellation process\n"
        {
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "actual cancellation process did not start"
        );
    }
    request(&mut transport, Request::Stop { run_id: id.clone() }, events)?;
    ensure!(
        collect(&mut transport, &id, events)?.status == "cancelled",
        "worker cancellation was not observed"
    );
    request(&mut transport, Request::Shutdown, events)?;
    events.push(json!({"topology":"actual SSH client and worker processes; outer harness records namespace/network topology","external_physical_host":false,"firmware_gate":false,"actual_ssh_transport_suite":true,"local_remote_same_process_spec":true,"proc_view_matches_execution_pid_namespace":true}));
    dashboard_transport_report::verify_report(&json!({"passed":true,"events":events}))?;
    Ok(())
}
fn main() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    for operation in ["chunk", "finish"] {
        if args
            .first()
            .is_some_and(|arg| arg == format!("/drop-{operation}-ack-relay").as_str())
        {
            ensure!(
                args.len() == 3 && args[2] == "--stream",
                "artifact reply-loss relay arguments"
            );
            return ssh_artifact_ack_loss::run(Path::new(&args[1]), operation);
        }
    }
    if args.len() == 3 && args[2] == "--stream" {
        return ssh_one_reply::run(Path::new(&args[1]));
    }
    if args.len() == 2 && args[1] == "__selected-http" {
        return selected_gateway::probe();
    }
    if args.len() == 2 && args[1] == "__isolated-dashboard" {
        return dashboard::probe(false);
    }
    if args.len() == 2 && args[1] == "__isolated-dashboard-cancel" {
        return dashboard::probe(true);
    }
    if args.len() == 2 && args[1] == "__isolated-loopback" {
        return loopback::probe();
    }
    if args.len() == 3 && args[1] == "prepare" {
        let out = Path::new(&args[2]);
        ensure!(!out.exists(), "fixture output must be new");
        fs::create_dir_all(out)?;
        let store = Store::open(&out.join("input-store"), 16 * 1024 * 1024)?;
        let inputs = md_sim_firmware::lab::elf_closure(
            &store,
            vec![("/usr/bin/sh".into(), "/usr/bin/sh".into())],
        )?;
        isolation::save_json(&out.join("fixture-inputs.json"), &inputs)?;
        println!(
            "{}",
            json!({"fixture":hash_file(&out.join("fixture-inputs.json"))?,"output":out})
        );
        return Ok(());
    }
    let mut events = Vec::new();
    let result = if args.len() == 2 && args[1] == "client" {
        client_test(&mut events)
    } else {
        test(&mut events)
    };
    for file in [
        "/work/worker.log",
        "/work/sshd.log",
        "/work/coordinator-live/live-reconnect.json",
        "/work/artifact-ack-loss.json",
        "/work/selected-gateway-wrong-execution/stdout.json",
        "/work/selected-gateway-wrong-execution/stderr.log",
        "/work/selected-gateway-wrong-execution/transfer.json",
        "/work/selected-gateway/stdout.json",
        "/work/selected-gateway/stderr.log",
        "/work/selected-gateway/stopped.json",
        "/work/selected-gateway/transfer.json",
        "/work/selected-gateway-reconnect/stdout.json",
        "/work/selected-gateway-reconnect/stderr.log",
        "/work/selected-gateway-reconnect/stopped.json",
        "/work/selected-gateway-reconnect/transfer.json",
    ] {
        if let Ok(bytes) = fs::read(file) {
            events.push(json!({"saved_log":file,"hex":hex::encode(&bytes[..bytes.len().min(256*1024)]),"truncated":bytes.len()>256*1024}));
        }
    }
    println!(
        "MD_SIM_SSH_EVIDENCE {}",
        json!({"passed":result.is_ok(),"events":events,"error":result.as_ref().err().map(|e|format!("{e:#}"))})
    );
    result
}

//! Actual worker, different client processes, and native reply loss in a VM.
//! No SSH, firmware, outside-VM deployment or exact-target pass is inferred.
use super::broker;
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::{
    executor::{local_tools, PrivateWorker, SocketTransport},
    ExecutionScope, Executor, LockedRun, SuiteIdentity, WorkerExecutor, WorkerTransport,
};
use md_sim_runtime::{
    isolation::{save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, Store},
    worker::{self, Request},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::OwnedFd,
        unix::{fs::PermissionsExt, net::UnixStream, process::CommandExt},
    },
    path::Path,
    process::{Child, Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};

const ROOT: &str = "/work/broker-regression";
const SOCKET: &str = "/work/broker-regression/endpoint/control.sock";
const ATTEMPT: &str = "/work/broker-regression/client/attempt";
const WORKER: &str = "/work/broker-regression/worker";

struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
struct ActualWorker(PrivateWorker);
impl WorkerTransport for ActualWorker {
    fn request(&mut self, request: &Request) -> Result<Value> {
        self.0.executor.transport.request(request)
    }
    fn description(&self) -> String {
        self.0.executor.transport.description()
    }
}

pub fn prepare(out: &Path, worker: &Path) -> Result<()> {
    ensure!(!out.exists(), "broker test output must be new");
    fs::create_dir_all(out)?;
    let out = out.canonicalize()?;
    let store = Store::open(&out.join("inputs"), 32 << 20)?;
    let executable = std::env::current_exe()?;
    let mut inputs =
        md_sim_firmware::lab::elf_closure(&store, vec![("/job".into(), executable.clone())])?;
    let payload = out.join("challenge.bin");
    let mut bytes = vec![0; 1024];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    fs::write(&payload, bytes)?;
    let challenge = hash_file(&payload)?;
    store.import(&payload, &challenge)?;
    inputs.push(Mount {
        guest_path: "/challenge.bin".into(),
        object: challenge.clone(),
    });
    let process = ProcessSpec {
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        schema_version: "2.0".into(),
        argv: vec!["/job".into(), "__job".into()],
        stdin_object: None,
        environment: BTreeMap::new(),
        loopback: false,
        dashboard: None,
        inputs,
        limits: Limits {
            wall_seconds: 25,
            cpu_seconds: 5,
            memory_bytes: 256 << 20,
            processes: 32,
            file_bytes: 8 << 20,
            trace_bytes: 128 << 10,
            tmp_bytes: 16 << 20,
        },
    };
    let reporter = hash_file(&executable)?;
    let inputs = process
        .inputs
        .iter()
        .map(|m| (m.guest_path.clone(), m.object.clone()))
        .collect();
    let run = LockedRun {
        name: "worker-broker-client-loss".into(),
        scope: ExecutionScope::ProcessPrimitive {
            description: "Actual support process; no miner claim".into(),
        },
        suite: SuiteIdentity {
            id: "worker-broker-client-loss".into(),
            version: "1".into(),
            suite: reporter.clone(),
            verifier: reporter,
            required_gates: ["client-loss-survival".into()].into(),
        },
        process: Some(process),
        inputs,
        backend_tools: local_tools()?,
        policies: json!({"broker":"owner-only Unix socket","challenge":challenge}),
        capability_limits: vec![
            "Local VM support-process test; no SSH, firmware or external host".into(),
        ],
    };
    run.validate()?;
    save_json(&out.join("fixture.json"), &run)?;
    let mut files = BTreeMap::from([(
        "/fixture.json".to_owned(),
        json!({"path":out.join("fixture.json"),"mode":420}),
    )]);
    for object in run.inputs.values() {
        files.insert(
            format!("/fixture-objects/{}", object.sha256),
            json!({"path":store.verified_path(object)?.canonicalize()?,"mode":420}),
        );
    }
    save_json(
        &out.join("scenario.json"),
        &json!({"programs":{
        "/broker-check":executable,"/md-sim-worker-ipc":worker.canonicalize()?,
        "/usr/bin/bwrap":"/usr/bin/bwrap","/usr/bin/unshare":"/usr/bin/unshare"},
        "files":files,"argv":["/broker-check","__test"],"environment":{}}),
    )?;
    Ok(())
}

fn run() -> Result<LockedRun> {
    let bytes = fs::read("/fixture.json")?;
    ensure!(bytes.len() <= 65536, "fixture bound");
    let run: LockedRun = serde_json::from_slice(&bytes)?;
    run.validate()?;
    Ok(run)
}
fn client() -> WorkerExecutor<SocketTransport> {
    WorkerExecutor {
        transport: SocketTransport(SOCKET.into()),
    }
}
fn selected() -> Result<String> {
    let bytes = fs::read(Path::new(ATTEMPT).join("worker-run.json"))?;
    ensure!(bytes.len() <= 65536, "run record bound");
    Ok(serde_json::from_slice::<Value>(&bytes)?["run_id"]
        .as_str()
        .context("run ID absent")?
        .into())
}
fn spawn(mode: &str) -> Result<Owned> {
    let log = File::create(Path::new(ROOT).join(format!("{mode}.log")))?;
    Ok(Owned(
        Command::new("/broker-check")
            .arg(mode)
            .uid(1000)
            .gid(1000)
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    ))
}
fn wait(child: &mut Owned) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.0.try_wait()? {
            ensure!(status.success(), "broker test child failed: {status}");
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "broker test child deadline");
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn guest(mode: &str) -> Result<()> {
    if mode == "__test" {
        let result = orchestrate();
        if let Err(error) = &result {
            let mut diagnostics = BTreeMap::new();
            for name in [
                "__test-server.log",
                "__test-client.log",
                "__test-recover.log",
                "broker.jsonl",
            ] {
                let path = Path::new(ROOT).join(name);
                if let Ok(file) = File::open(path) {
                    let mut bytes = Vec::new();
                    file.take(64 << 10).read_to_end(&mut bytes)?;
                    diagnostics.insert(name, String::from_utf8_lossy(&bytes).into_owned());
                }
            }
            eprintln!(
                "MD_SIM_WORKER_BROKER_FAILURE {}",
                json!({"error":format!("{error:#}"),"diagnostics":diagnostics})
            );
        }
        return result;
    }
    ensure!(
        unsafe { libc::geteuid() } == 1000,
        "test worker/client must use UID1000"
    );
    match mode {
        "__test-server" => {
            let worker = ActualWorker(PrivateWorker::spawn(
                Path::new("/md-sim-worker-ipc"),
                Path::new(WORKER),
            )?);
            let report = broker::serve(
                Path::new(SOCKET),
                worker,
                Arc::new(AtomicBool::new(false)),
                File::create(Path::new(ROOT).join("broker.jsonl"))?,
            )?;
            save_json(&Path::new(ROOT).join("broker-report.json"), &report)?;
        }
        "__test-client" => {
            let run = run()?;
            let store = Store::open(&Path::new(ROOT).join("client/inputs"), 32 << 20)?;
            for object in run.inputs.values() {
                store.import(&Path::new("/fixture-objects").join(&object.sha256), object)?;
            }
            fs::create_dir(ATTEMPT)?;
            let result = client().run(
                &store,
                &run,
                Arc::new(AtomicBool::new(false)),
                Path::new(ATTEMPT),
            )?;
            save_json(
                &Path::new(ROOT).join("unexpected-client-completion.json"),
                &result,
            )?;
            anyhow::bail!("first client must be terminated during the actual job");
        }
        "__test-drop" => {
            let error = worker::request(
                Path::new(SOCKET),
                &Request::Status {
                    run_id: "run-absent-for-broker-regression".into(),
                },
            )
            .expect_err("absent run unexpectedly exists");
            ensure!(
                error
                    .downcast_ref::<worker::CompletedWorkerError>()
                    .is_some(),
                "broker did not retain a complete worker error"
            );
            let stream = UnixStream::connect(SOCKET)?;
            let descriptor: OwnedFd = stream.into();
            let mut stream = File::from(descriptor);
            serde_json::to_writer(
                &mut stream,
                &Request::Status {
                    run_id: selected()?,
                },
            )?;
            stream.write_all(b"\n")?;
            // Drop the actual connected client without reading its response.
        }
        "__test-recover" => {
            let mut worker = client();
            let id = selected()?;
            let before = worker
                .transport
                .request(&Request::Status { run_id: id.clone() })?;
            ensure!(
                before["state"]["state"] == "booting",
                "selected job did not survive client loss"
            );
            worker.recover(Path::new(ATTEMPT))?;
            let bytes = fs::read(Path::new(ATTEMPT).join("worker-result.json"))?;
            ensure!(bytes.len() <= 512 << 10, "fixture result bound");
            let result: Value = serde_json::from_slice(&bytes)?;
            ensure!(
                result["status"] == "cancelled",
                "recovery did not stop the same running job"
            );
            let identity = hash_file(&Path::new(ATTEMPT).join("worker-result.json"))?;
            let cleanup: Value =
                serde_json::from_slice(&fs::read(Path::new(ATTEMPT).join("worker-cleanup.json"))?)?;
            ensure!(
                cleanup["acknowledged_result"] == serde_json::to_value(&identity)?,
                "cleanup identity differs"
            );
            save_json(
                &Path::new(ROOT).join("recovery.json"),
                &json!({"run_id":id,"status_before":before,
                "result":identity,"exact_result_hex":hex::encode(bytes),"cleanup":cleanup}),
            )?;
            ensure!(
                worker.transport.request(&Request::Shutdown)?["stopping"] == true,
                "shutdown acknowledgment absent"
            );
        }
        _ => anyhow::bail!("unknown broker test mode"),
    }
    Ok(())
}

fn orchestrate() -> Result<()> {
    ensure!(
        unsafe { libc::geteuid() } == 0,
        "VM test orchestrator needs guest root"
    );
    ensure!(
        !Path::new(ROOT).exists(),
        "broker regression root already exists"
    );
    for suffix in ["", "endpoint", "worker", "client"] {
        let path = Path::new(ROOT).join(suffix);
        fs::create_dir_all(&path)?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        std::os::unix::fs::chown(&path, Some(1000), Some(1000))?;
    }
    let mut server = spawn("__test-server")?;
    let deadline = Instant::now() + Duration::from_secs(15);
    while !Path::new(SOCKET).exists() {
        ensure!(
            server.0.try_wait()?.is_none() && Instant::now() < deadline,
            "broker startup failed"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    ensure!(
        worker::request(Path::new(SOCKET), &Request::Probe).is_err(),
        "root peer bypassed owner check"
    );
    let mut first = spawn("__test-client")?;
    let deadline = Instant::now() + Duration::from_secs(90);
    let (id, log, before) = loop {
        ensure!(
            first.0.try_wait()?.is_none() && server.0.try_wait()?.is_none(),
            "client/server stopped before actual job"
        );
        if let Ok(id) = selected() {
            let log = Path::new(WORKER).join("runs").join(&id).join("stdout.log");
            if let Ok(bytes) = fs::read(&log) {
                if bytes
                    .windows(b"MD_SIM_BROKER_JOB ".len())
                    .any(|b| b == b"MD_SIM_BROKER_JOB ")
                {
                    break (id, log, bytes);
                }
            }
        }
        ensure!(
            Instant::now() < deadline,
            "actual broker job start deadline"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let first_pid = first.0.id();
    let mut lost = spawn("__test-drop")?;
    wait(&mut lost)?;
    first.0.kill()?;
    first.0.wait()?;
    std::thread::sleep(Duration::from_millis(500));
    let after = fs::read(&log)?;
    ensure!(
        after.len() > before.len() && after.starts_with(&before),
        "actual job output did not continue after client EOF"
    );
    ensure!(
        !Path::new(ROOT)
            .join("unexpected-client-completion.json")
            .exists(),
        "client completed before injected loss"
    );
    let mut second = spawn("__test-recover")?;
    let second_pid = second.0.id();
    wait(&mut second)?;
    wait(&mut server)?;
    ensure!(
        first_pid != second_pid && !Path::new(SOCKET).exists(),
        "new client or owned socket cleanup missing"
    );
    let broker: Value =
        serde_json::from_slice(&fs::read(Path::new(ROOT).join("broker-report.json"))?)?;
    ensure!(
        broker["rejected_peers"].as_u64().is_some_and(|v| v >= 1)
            && broker["undelivered_replies"]
                .as_u64()
                .is_some_and(|v| v >= 1)
            && broker["worker_shutdown_acknowledged"] == true,
        "broker peer/lost-reply/shutdown evidence absent"
    );
    let recovery: Value =
        serde_json::from_slice(&fs::read(Path::new(ROOT).join("recovery.json"))?)?;
    ensure!(recovery["run_id"] == id, "recovery selected another job");
    let audit = fs::read_to_string(Path::new(ROOT).join("broker.jsonl"))?;
    ensure!(audit.len() <= 1 << 20, "test audit bound");
    let record = json!({"passed":true,"first_client_pid":first_pid,"second_client_pid":second_pid,
        "run_id":id,"output_before_loss":before.len(),"output_after_loss":after.len(),
        "broker":broker,"recovery":recovery,"audit":audit,
        "scope":"Actual worker and separate client processes inside one isolated VM. No SSH, firmware, outside-VM deployment or exact-target pass."});
    println!("MD_SIM_WORKER_BROKER_REGRESSION {record}");
    Ok(())
}

//! SSH control for a real native worker outside the support VM. The VM owns
//! only a checked virtual device channel and separate SSH client/server roles.
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::{
    ssh_transport::SshTransport, Executor, LockedRun, WorkerExecutor, WorkerTransport,
};
use md_sim_runtime::{
    isolation::{save_json, ProcessResult},
    store::{hash_file, ObjectId, Store},
    worker::{LogStream, Request},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            fs::{FileTypeExt, MetadataExt, PermissionsExt},
            process::CommandExt,
        },
    },
    path::Path,
    process::{Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};

const INPUTS: &str = "/broker-inputs";
const ATTEMPT: &str = "/work/attempt";
pub const RECORD: &str = "MD_SIM_NATIVE_SSH_BROKER ";

pub fn enabled() -> bool {
    std::env::var("MD_SIM_WORKER_BROKER").as_deref() == Ok("1")
}

pub fn prepare(out: &Path, lock: &Path, objects: &Path, relay: &Path, veth: &Path) -> Result<()> {
    ensure!(!out.exists(), "SSH broker scenario output already exists");
    let run: LockedRun = serde_json::from_slice(&bounded(lock, 4 << 20)?)?;
    run.validate()?;
    ensure!(run.process.is_some(), "SSH broker requires a real process");
    let mut programs = BTreeMap::from([
        ("/namespace-check".to_owned(), std::env::current_exe()?),
        ("/md-sim-worker-relay".to_owned(), relay.canonicalize()?),
    ]);
    for program in [
        "/usr/bin/ssh",
        "/usr/bin/sshd",
        "/usr/bin/ssh-keygen",
        "/usr/lib/ssh/sshd-session",
        "/usr/lib/ssh/sshd-auth",
        "/usr/bin/sh",
        "/usr/bin/true",
        "/usr/bin/bwrap",
        "/usr/bin/unshare",
        "/usr/bin/ip",
        "/usr/bin/insmod",
        "/usr/bin/zstd",
    ] {
        programs.insert(program.to_owned(), Path::new(program).canonicalize()?);
    }
    let mut files = BTreeMap::from([
        (
            format!("{INPUTS}/locked-run.json"),
            json!({"path":lock.canonicalize()?,"mode":292}),
        ),
        (
            "/veth.ko.zst".into(),
            json!({"path":veth.canonicalize()?,"mode":292}),
        ),
    ]);
    for object in run.inputs.values() {
        let path = objects.join(&object.sha256);
        ensure!(hash_file(&path)? == *object, "locked SSH input changed");
        files.insert(
            format!("{INPUTS}/objects/{}", object.sha256),
            json!({"path":path.canonicalize()?,"mode":292}),
        );
    }
    save_json(
        out,
        &json!({"programs":programs,"files":files,
        "argv":["/namespace-check","orchestrator"],
        "environment":{"MD_SIM_WORKER_BROKER":"1"}}),
    )
}

pub(super) fn bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "broker evidence bound exceeded"
    );
    Ok(bytes)
}

pub(super) fn run() -> Result<LockedRun> {
    let run: LockedRun = serde_json::from_slice(&bounded(
        &Path::new(INPUTS).join("locked-run.json"),
        4 << 20,
    )?)?;
    run.validate()?;
    Ok(run)
}

pub fn selected_identity() -> Result<Value> {
    let run = run()?;
    Ok(
        json!({"locked_run_hash":run.hash()?,"lock_file":hash_file(&Path::new(INPUTS).join("locked-run.json"))?,
        "name":run.name,"scope":run.scope,"required_gates":run.suite.required_gates}),
    )
}

fn pass_device(command: &mut Command, device: File) {
    // SAFETY: the owned descriptor remains live in this closure until exec.
    // This performs only async-signal-safe descriptor operations after fork.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(device.as_raw_fd(), 3) != 3
                || libc::fcntl(3, libc::F_SETFD, 0) != 0
                || libc::syscall(libc::SYS_close_range, 4u32, u32::MAX, 4u32) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

pub fn inherit_device(command: &mut Command) -> Result<()> {
    let device = super::virtual_port::open("md-sim.worker")?;
    let opened = File::open(Path::new(INPUTS).join("locked-run.json"))?;
    let sentinel = opened.try_clone()?;
    drop(opened);
    ensure!(
        sentinel.as_raw_fd() >= 4,
        "descriptor isolation sentinel overlaps the worker channel"
    );
    command.env(
        "MD_SIM_BROKER_SENTINEL_FD",
        sentinel.as_raw_fd().to_string(),
    );
    // The regression deliberately makes one unrelated descriptor inheritable.
    // pass_device must remove it at exec while retaining only the worker FD3.
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(sentinel.as_raw_fd(), libc::F_SETFD, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    pass_device(command, device);
    Ok(())
}

fn take_device() -> Result<File> {
    ensure!(
        unsafe { libc::fcntl(3, libc::F_GETFD) } >= 0,
        "inherited worker FD absent"
    );
    // SAFETY: the trusted parent transfers this descriptor once to this mode.
    let file = unsafe { File::from_raw_fd(3) };
    ensure!(
        file.metadata()?.file_type().is_char_device(),
        "inherited worker FD is not a device"
    );
    ensure!(
        unsafe { libc::fcntl(3, libc::F_SETFD, libc::FD_CLOEXEC) } == 0,
        "cannot protect worker FD"
    );
    Ok(file)
}

pub fn service() -> Result<()> {
    ensure!(
        enabled()
            && unsafe { libc::geteuid() } == 1000
            && fs::read_to_string("/proc/cmdline")?.contains("md_sim_private_guest=1"),
        "broker service requires its private guest account"
    );
    let mut transport = super::device_transport::DeviceTransport::new(take_device()?)?;
    let identity: ObjectId =
        serde_json::from_slice(&bounded(Path::new("/worker-identity.json"), 4096)?)?;
    ensure!(
        transport.request(&Request::Probe)?["worker_build"] == serde_json::to_value(identity)?,
        "native worker build differs"
    );
    let log = File::create_new("/work/broker/broker.jsonl")?;
    let report = if super::shared_pool::enabled() {
        super::device_broker::serve(
            Path::new("/worker/worker.sock"),
            super::shared_pool::Gate::new(transport)?,
            Arc::new(AtomicBool::new(false)),
            log,
        )?
    } else {
        super::device_broker::serve(
            Path::new("/worker/worker.sock"),
            transport,
            Arc::new(AtomicBool::new(false)),
            log,
        )?
    };
    save_json(Path::new("/work/broker/broker-report.json"), &report)
}

pub fn server(events: &mut Vec<Value>) -> Result<()> {
    let device = take_device()?;
    let sentinel: i32 = std::env::var("MD_SIM_BROKER_SENTINEL_FD")?.parse()?;
    ensure!(
        sentinel >= 4 && unsafe { libc::fcntl(sentinel, libc::F_GETFD) } == -1,
        "server inherited the client-input sentinel descriptor"
    );
    for descriptor in 4..=64 {
        ensure!(
            unsafe { libc::fcntl(descriptor, libc::F_GETFD) } == -1,
            "server inherited unrelated descriptor {descriptor}"
        );
    }
    events.push(json!({"inherited_worker_fd":3,"unrelated_inheritable_descriptor_rejected":true}));
    ensure!(
        fs::read_dir(INPUTS)?.next().is_none(),
        "SSH server sees client inputs"
    );
    ensure!(
        !Path::new("/work/clientkey").exists() && !Path::new("/credentials/client.key").exists(),
        "SSH server sees client key"
    );
    let namespaces = super::namespace_evidence("worker")?;
    save_json(Path::new("/work/namespace.json"), &namespaces)?;
    events.push(namespaces);
    ensure!(
        fs::symlink_metadata("/work")?.uid() == 0,
        "server work root must retain root ownership"
    );
    fs::set_permissions("/work", fs::Permissions::from_mode(0o711))?;
    fs::create_dir("/work/broker")?;
    for path in ["/worker", "/work/broker"] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        std::os::unix::fs::chown(path, Some(1000), Some(1000))?;
    }
    if super::carrier_dashboard::enabled() {
        fs::create_dir("/worker/access-credentials")?;
        fs::set_permissions(
            "/worker/access-credentials",
            fs::Permissions::from_mode(0o700),
        )?;
        std::os::unix::fs::chown("/worker/access-credentials", Some(1000), Some(1000))?;
        for name in ["ca.crt", "server.crt", "server.key"] {
            let destination = format!("/worker/access-credentials/{name}");
            fs::copy(format!("/credentials/{name}"), &destination)?;
            fs::set_permissions(&destination, fs::Permissions::from_mode(0o600))?;
            std::os::unix::fs::chown(destination, Some(1000), Some(1000))?;
        }
    }
    let mut pool = if super::shared_pool::enabled() {
        Some(super::pool_service::Service::new()?)
    } else {
        None
    };
    let mut carrier = if super::carrier_dashboard::enabled() {
        Some(super::carrier_dashboard::Supervisor::new()?)
    } else {
        None
    };
    let mut command = Command::new("/namespace-check");
    command
        .arg("broker-service")
        .uid(1000)
        .gid(1000)
        .stdin(Stdio::null())
        .stdout(File::create("/work/broker.stdout")?)
        .stderr(File::create("/work/broker.stderr")?);
    pass_device(&mut command, device);
    let mut broker = super::Owned(command.spawn()?);
    drop(command);
    let mut sshd = super::start(
        "/usr/bin/sshd",
        &["-D", "-e", "-f", "/work/sshd_config"],
        "/work/sshd.log",
    )?;
    let deadline = Instant::now()
        + Duration::from_secs(if super::shared_pool::enabled() {
            super::shared_pool::SERVER_SECONDS
        } else {
            240
        });
    while !Path::new("/worker/worker.sock").exists() {
        ensure!(
            broker.0.try_wait()?.is_none()
                && sshd.0.try_wait()?.is_none()
                && Instant::now() < deadline,
            "native broker startup failed"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    save_json(
        Path::new("/work/broker-ready.json"),
        &json!({"native_worker_outside_vm":true,"client_inputs_visible":false,"worker_uid":1000}),
    )?;
    loop {
        if let Some(pool) = &mut pool {
            pool.poll()?;
        }
        if let Some(carrier) = &mut carrier {
            carrier.poll()?;
        }
        if let Some(status) = broker.0.try_wait()? {
            ensure!(status.success(), "native broker failed: {status}");
            break;
        }
        ensure!(
            sshd.0.try_wait()?.is_none() && Instant::now() < deadline,
            "SSH server deadline or daemon exit"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    if let Some(pool) = &mut pool {
        pool.finish()?;
    }
    if let Some(carrier) = &mut carrier {
        carrier.finish()?;
    }
    events.push(serde_json::from_slice::<Value>(&bounded(
        Path::new("/work/broker/broker-report.json"),
        65536,
    )?)?);
    Ok(())
}

fn executor() -> Result<WorkerExecutor<SshTransport>> {
    Ok(WorkerExecutor {
        transport: SshTransport::new_streaming(super::ssh_config())?,
    })
}

fn selected() -> Result<String> {
    let selected: Value = serde_json::from_slice(&bounded(
        &Path::new(ATTEMPT).join("worker-run.json"),
        65536,
    )?)?;
    Ok(selected["run_id"]
        .as_str()
        .context("selected run absent")?
        .into())
}

pub fn client(events: &mut Vec<Value>) -> Result<()> {
    ensure!(
        unsafe { libc::fcntl(3, libc::F_GETFD) } == -1,
        "SSH client inherited server descriptor3"
    );
    ensure!(
        !Path::new("/worker-identity.json").exists(),
        "client sees server-only identity file"
    );
    let namespaces = super::namespace_evidence("client")?;
    save_json(Path::new("/work/namespace.json"), &namespaces)?;
    events.push(namespaces);
    if super::shared_pool::enabled() {
        return super::shared_run::client(events);
    }
    let mut executor = executor()?;
    events.push(executor.transport.evidence());
    let phase = std::env::var("MD_SIM_BROKER_PHASE")?;
    if phase == "run" {
        let run = run()?;
        let store = Store::open(Path::new("/work/inputs"), 32 << 20)?;
        for object in run.inputs.values() {
            store.import(
                &Path::new(INPUTS).join("objects").join(&object.sha256),
                object,
            )?;
        }
        fs::create_dir(ATTEMPT)?;
        save_json(
            &Path::new(ATTEMPT).join("executor.json"),
            &executor.identity()?,
        )?;
        executor.run(
            &store,
            &run,
            Arc::new(AtomicBool::new(false)),
            Path::new(ATTEMPT),
        )?;
        anyhow::bail!("first SSH client completed before injected loss");
    }
    ensure!(phase == "recover", "unknown SSH broker client phase");
    let id = selected()?;
    let before: Value = serde_json::from_slice(&bounded(Path::new("/work/loss.json"), 65536)?)?;
    ensure!(before["run_id"] == id, "recovery selected a different run");
    let connected = executor
        .transport
        .request(&Request::Status { run_id: id.clone() })?;
    ensure!(
        connected["state"]["state"] == "booting",
        "job was not active at fresh SSH reconnect"
    );
    let at_reconnect = connected["saved_logs"]["stdout"]
        .as_u64()
        .context("reconnect stdout length absent")?;
    std::thread::sleep(Duration::from_millis(500));
    let status = executor
        .transport
        .request(&Request::Status { run_id: id.clone() })?;
    ensure!(
        status["state"]["state"] == "booting",
        "job did not survive SSH/client namespace loss"
    );
    let saved = status["saved_logs"]["stdout"]
        .as_u64()
        .context("saved stdout length absent")?;
    let prior = before["stdout_bytes"]
        .as_u64()
        .context("prior stdout length absent")?;
    ensure!(
        saved > at_reconnect && at_reconnect >= prior,
        "native job did not produce output after client loss"
    );
    let log = executor.transport.request(&Request::Logs {
        run_id: id.clone(),
        stream: LogStream::Stdout,
        offset: 0,
        count: saved.min(65536) as usize,
    })?;
    let bytes = hex::decode(log["hex"].as_str().context("log bytes absent")?)?;
    let prefix = hex::decode(
        before["stdout_hex"]
            .as_str()
            .context("prior log bytes absent")?,
    )?;
    ensure!(
        bytes.starts_with(&prefix),
        "native log changed after client loss"
    );
    executor.recover(Path::new(ATTEMPT))?;
    let result: ProcessResult = serde_json::from_slice(&bounded(
        &Path::new(ATTEMPT).join("worker-result.json"),
        2 << 20,
    )?)?;
    ensure!(
        result.status == "cancelled" && !result.trace_truncated,
        "recovery result status or trace differs"
    );
    save_json(
        Path::new("/work/recovery.json"),
        &json!({"run_id":id,"status_at_reconnect":connected,"status_before_recovery":status,
        "stdout_at_reconnect":at_reconnect,
        "stdout_before_loss":prior,"stdout_after_loss":saved,"result":hash_file(&Path::new(ATTEMPT).join("worker-result.json"))?,
        "cleanup":serde_json::from_slice::<Value>(&bounded(&Path::new(ATTEMPT).join("worker-cleanup.json"),65536)?)?}),
    )?;
    ensure!(
        executor.transport.request(&Request::Shutdown)?["stopping"] == true,
        "worker shutdown acknowledgment absent"
    );
    Ok(())
}

fn role(role: &str, phase: Option<&str>, suffix: &str) -> Result<super::Owned> {
    let mut command = super::role_command(role)?;
    if let Some(phase) = phase {
        command.env("MD_SIM_BROKER_PHASE", phase);
    }
    Ok(super::Owned(
        command
            .stdin(Stdio::null())
            .stdout(File::create(format!("/work/{suffix}.stdout"))?)
            .stderr(File::create(format!("/work/{suffix}.stderr"))?)
            .spawn()?,
    ))
}

fn wait(child: &mut super::Owned, deadline: Instant) -> Result<()> {
    loop {
        if let Some(status) = child.0.try_wait()? {
            ensure!(status.success(), "SSH broker role failed: {status}");
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "SSH broker role deadline");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn namespace_members(namespace: &Value) -> Result<Vec<Value>> {
    let expected = namespace["namespaces"]["pid"]
        .as_str()
        .context("client PID namespace absent")?;
    let mut members = Vec::new();
    for (index, entry) in fs::read_dir("/proc")?.enumerate() {
        ensure!(index < 4096, "guest process listing exceeds bound");
        let entry = entry?;
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if fs::read_link(entry.path().join("ns/pid")).is_ok_and(|path| path == Path::new(expected))
        {
            let bytes = match bounded(&entry.path().join("status"), 16384) {
                Ok(bytes) => bytes,
                Err(error)
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound) =>
                {
                    continue
                }
                Err(error) => return Err(error),
            };
            let status = String::from_utf8(bytes)?;
            let field = |key: &str| -> Result<&str> {
                status
                    .lines()
                    .find_map(|line| line.strip_prefix(key))
                    .map(str::trim)
                    .context("process status field absent")
            };
            members.push(json!({"pid":pid,"comm":field("Name:")?,"state":field("State:")?,
                "parent_pid":field("PPid:")?.parse::<u32>()?,
                "namespace_pids":field("NSpid:")?.split_whitespace().map(str::parse::<u32>).collect::<std::result::Result<Vec<_>,_>>()?}));
        }
    }
    Ok(members)
}

fn pin_namespaces(namespace: &Value, members: &[Value]) -> Result<Vec<File>> {
    let process = members
        .iter()
        .find(|entry| entry["comm"] == "namespace-check")
        .context("client process is absent while pinning namespaces")?;
    let pid = process["pid"].as_u64().context("client PID absent")?;
    let mut handles = Vec::new();
    for name in ["mnt", "pid", "uts", "ipc"] {
        let expected = namespace["namespaces"][name]
            .as_str()
            .context("client namespace identity absent")?;
        let inode: u64 = expected
            .strip_prefix(&format!("{name}:["))
            .and_then(|value| value.strip_suffix(']'))
            .context("client namespace identity format differs")?
            .parse()?;
        let handle = File::open(format!("/proc/{pid}/ns/{name}"))?;
        ensure!(
            handle.metadata()?.ino() == inode,
            "client namespace changed before pinning"
        );
        handles.push(handle);
    }
    Ok(handles)
}

fn artifacts() -> Result<Value> {
    let run = run()?;
    let attempt = Path::new("/work/client-root/attempt");
    export_attempt(
        attempt,
        &run,
        true,
        attempt.join("worker-cleanup.json").exists(),
        (2 << 20, 1 << 20),
    )
}

pub(super) fn export_attempt(
    attempt: &Path,
    run: &LockedRun,
    include_result: bool,
    complete_trace: bool,
    result_bounds: (u64, u64),
) -> Result<Value> {
    let mut files = BTreeMap::new();
    for name in [
        "executor.json",
        "artifact-transfer.json",
        "worker-request.json",
        "worker-run.json",
        "worker-cleanup-response.json",
        "worker-cleanup.json",
        "worker-error.json",
        "worker-retention.json",
        "worker-result.json",
    ] {
        if name == "worker-result.json" && !include_result {
            continue;
        }
        if attempt.join(name).exists() {
            files.insert(
                name,
                super::channel_artifacts::export_file(
                    &attempt.join(name),
                    if name == "worker-result.json" {
                        result_bounds.0
                    } else {
                        1 << 20
                    },
                    if name == "worker-result.json" {
                        result_bounds.1
                    } else {
                        1 << 20
                    },
                )?,
            );
        }
    }
    let mut traces = BTreeMap::new();
    // Partial live logs do not claim equality with a complete result.
    if complete_trace {
        for name in ["worker-stdout.log", "worker-stderr.log"] {
            traces.insert(name, hash_file(&attempt.join(name))?);
        }
    }
    Ok(
        json!({"locked_run_hash":run.hash()?,"files":files,"traces":traces,
        "retention_contract":"VM-local durable result before cleanup; exact export after cleanup. No outer-loss retention claim."}),
    )
}

pub fn orchestrate_roles(events: &mut Vec<Value>) -> Result<()> {
    if super::shared_pool::enabled() {
        return super::shared_run::orchestrate(events);
    }
    let result = orchestrate_inner(events);
    if let Err(error) = &result {
        match artifacts() {
            Ok(artifacts) => {
                let report = json!({"passed":false,"error":format!("{error:#}"),"artifacts":artifacts,
                    "scope":"Failed native SSH broker integration. Retained partial or complete artifacts do not count as a pass."});
                ensure!(
                    serde_json::to_vec(&report)?.len() <= 4 << 20,
                    "SSH failure evidence envelope too large"
                );
                println!("{RECORD}{report}");
                std::io::stdout().flush()?;
            }
            Err(export) => events.push(json!({"artifact_export_error":format!("{export:#}")})),
        }
    }
    result
}

fn orchestrate_inner(events: &mut Vec<Value>) -> Result<()> {
    // Adopt and reap only observed descendants of the killed client namespace.
    // The VM init waits for this scenario, so it cannot reap them on our behalf.
    ensure!(
        unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } == 0,
        "cannot own orphaned client descendants"
    );
    let mut server = role("server", None, "server")?;
    let deadline = Instant::now() + Duration::from_secs(240);
    while !Path::new("/work/server-root/broker-ready.json").exists() {
        ensure!(
            server.0.try_wait()?.is_none() && Instant::now() < deadline,
            "SSH broker server startup deadline"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut first = role("client", Some("run"), "client-first")?;
    let (id, prefix) = loop {
        ensure!(
            first.0.try_wait()?.is_none()
                && server.0.try_wait()?.is_none()
                && Instant::now() < deadline,
            "SSH broker job start failed"
        );
        if let (Ok(run), Ok(bytes)) = (
            bounded(
                Path::new("/work/client-root/attempt/worker-run.json"),
                65536,
            ),
            bounded(
                Path::new("/work/client-root/attempt/worker-stdout.log"),
                65536,
            ),
        ) {
            if !bytes.is_empty() {
                let value: Value = serde_json::from_slice(&run)?;
                break (
                    value["run_id"]
                        .as_str()
                        .context("run ID absent")?
                        .to_owned(),
                    bytes,
                );
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let first_namespace: Value = serde_json::from_slice(&bounded(
        Path::new("/work/client-root/namespace.json"),
        65536,
    )?)?;
    let first_members = namespace_members(&first_namespace)?;
    let namespace_handles = pin_namespaces(&first_namespace, &first_members)?;
    ensure!(
        first_members.iter().any(|entry| entry["comm"] == "ssh"),
        "streaming OpenSSH process was not observed"
    );
    let first_pid = first.0.id();
    events.push(json!({"client_before_loss":first_members,"namespace":first_namespace}));
    first.0.kill()?;
    first.0.wait()?;
    let reap_deadline = Instant::now() + Duration::from_secs(5);
    let mut reaped = Vec::new();
    let mut last_members = Value::Null;
    loop {
        let remaining = namespace_members(&first_namespace)?;
        if remaining.is_empty() {
            break;
        }
        let observed = serde_json::to_value(&remaining)?;
        if observed != last_members {
            ensure!(events.len() < 128, "client-loss process evidence bound");
            events.push(json!({"client_after_kill":observed}));
            last_members = observed;
        }
        for member in &remaining {
            if member["parent_pid"].as_u64() == Some(u64::from(std::process::id())) {
                let pid: libc::pid_t = member["pid"]
                    .as_u64()
                    .context("owned process ID absent")?
                    .try_into()?;
                let mut status = 0;
                let result = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
                ensure!(
                    result >= 0,
                    "cannot reap owned client descendant: {}",
                    std::io::Error::last_os_error()
                );
                if result == pid {
                    reaped.push(json!({"process":member,"wait_status":status}));
                }
            }
        }
        ensure!(
            Instant::now() < reap_deadline,
            "old client or SSH process survived client namespace termination"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    save_json(
        Path::new("/work/client-root/loss.json"),
        &json!({"run_id":id,"stdout_bytes":prefix.len(),"stdout_hex":hex::encode(&prefix)}),
    )?;
    std::thread::sleep(Duration::from_millis(500));
    let mut second = role("client", Some("recover"), "client")?;
    let second_pid = second.0.id();
    wait(&mut second, deadline)?;
    wait(&mut server, deadline)?;
    let second_namespace: Value = serde_json::from_slice(&bounded(
        Path::new("/work/client-root/namespace.json"),
        65536,
    )?)?;
    let server_namespace: Value = serde_json::from_slice(&bounded(
        Path::new("/work/server-root/namespace.json"),
        65536,
    )?)?;
    for ns in ["net", "mnt", "pid", "uts", "ipc"] {
        ensure!(
            server_namespace["namespaces"][ns] != second_namespace["namespaces"][ns],
            "server/client namespace is shared"
        );
    }
    for ns in ["mnt", "pid", "uts", "ipc"] {
        ensure!(
            first_namespace["namespaces"][ns] != second_namespace["namespaces"][ns],
            "replacement client namespace did not change"
        );
    }
    drop(namespace_handles);
    ensure!(first_pid != second_pid, "client process was reused");
    let report = json!({"passed":true,"first_client_pid":first_pid,"second_client_pid":second_pid,
        "namespace_references_held_during_replacement":["mnt","pid","uts","ipc"],
        "first_client_processes":first_members,"first_client_processes_after_kill":0,"reaped_client_descendants":reaped,
        "first_client_namespace":first_namespace,"second_client_namespace":second_namespace,"server_namespace":server_namespace,
        "recovery":serde_json::from_slice::<Value>(&bounded(Path::new("/work/client-root/recovery.json"),65536)?)?,
        "broker":serde_json::from_slice::<Value>(&bounded(Path::new("/work/server-root/broker/broker-report.json"),65536)?)?,
        "broker_audit":String::from_utf8(bounded(Path::new("/work/server-root/broker/broker.jsonl"),1 << 20)?)?,
        "artifacts":artifacts()?,
        "scope":"Native worker outside the disconnected VM. Actual OpenSSH between separate guest client/server namespaces. Client namespace loss only; no full-VM survival or external physical host claim."});
    ensure!(
        serde_json::to_vec(&report)?.len() <= 4 << 20,
        "SSH broker evidence envelope too large"
    );
    for ns in ["md-worker", "md-client"] {
        super::invoke("/usr/bin/ip", &["netns", "delete", ns])?;
    }
    println!("{RECORD}{report}");
    std::io::stdout().flush()?;
    events.push(json!({"native_ssh_broker":true,"same_run_survived_client_namespace_loss":true,"outer_retention_receipt":false}));
    Ok(())
}

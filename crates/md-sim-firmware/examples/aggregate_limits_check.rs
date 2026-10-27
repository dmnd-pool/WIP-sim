//! Real cgroup-v2 fault checks inside the disconnected Linux test VM.
//! This executes test workloads, not firmware, and has no mining/board claim.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute_recorded, Limits, Mount, ProcessSpec, TraceFiles},
    store::{hash_file, Store},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

struct Children(Vec<Child>);
impl Drop for Children {
    fn drop(&mut self) {
        for child in &mut self.0 {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
fn workload(kind: &str) -> Result<()> {
    ensure!(
        fs::read_to_string("/proc/cmdline")?
            .split_whitespace()
            .any(|w| w == "md_sim_private_guest=1"),
        "test workload requires private VM"
    );
    ensure!(
        fs::read_to_string("/proc/self/status")?.contains("NoNewPrivs:\t1"),
        "test workload needs no-new-privileges"
    );
    match kind {
        "ok" => println!("actual bounded guest execution"),
        "cpu-child" => loop {
            std::hint::black_box((0..10_000_u64).fold(1u64, u64::wrapping_add));
        },
        "memory-child" => {
            let mut bytes = vec![0u8; 32 * 1024 * 1024];
            for byte in bytes.iter_mut().step_by(4096) {
                *byte = 0x5a;
            }
            std::hint::black_box(&bytes);
            std::thread::sleep(Duration::from_secs(20));
        }
        "sleep-child" => {
            println!("actual sleeping workload started");
            std::thread::sleep(Duration::from_secs(20));
        }
        "cpu" | "memory" | "pids" | "sleep" => {
            let sub = match kind {
                "cpu" => "cpu-child",
                "memory" => "memory-child",
                _ => "sleep-child",
            };
            let mut children = Children(Vec::new());
            for _ in 0..if kind == "pids" { 32 } else { 4 } {
                match Command::new("/probe").args(["workload", sub]).spawn() {
                    Ok(child) => children.0.push(child),
                    Err(error) if kind == "pids" && error.raw_os_error() == Some(libc::EAGAIN) => {
                        println!(
                            "{}",
                            json!({"process_creation_bounded":true,"created":children.0.len(),"errno":libc::EAGAIN})
                        );
                        return Ok(());
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            ensure!(
                kind != "pids",
                "process creation unexpectedly exceeded selected bound"
            );
            std::thread::sleep(Duration::from_secs(20));
            anyhow::bail!("aggregate fault did not stop workload");
        }
        _ => anyhow::bail!("unknown workload"),
    }
    Ok(())
}
fn chown(path: &Path) -> Result<()> {
    let name = CString::new(path.as_os_str().as_encoded_bytes())?;
    ensure!(
        unsafe { libc::chown(name.as_ptr(), 1000, 1000) } == 0,
        "delegation ownership failed: {}",
        std::io::Error::last_os_error()
    );
    Ok(())
}
fn guest() -> Result<()> {
    ensure!(
        unsafe { libc::geteuid() } == 0
            && fs::read_to_string("/proc/cmdline")?
                .split_whitespace()
                .any(|w| w == "md_sim_private_guest=1"),
        "requires private VM setup role"
    );
    ensure!(
        fs::read_dir("/sys/class/net")?.count() == 1,
        "unexpected VM network device"
    );
    fs::create_dir_all("/sys/fs/cgroup")?;
    ensure!(
        unsafe {
            libc::mount(
                c"cgroup2".as_ptr(),
                c"/sys/fs/cgroup".as_ptr(),
                c"cgroup2".as_ptr(),
                0,
                std::ptr::null(),
            )
        } == 0,
        "private VM cgroup mount failed: {}",
        std::io::Error::last_os_error()
    );
    fs::write(
        "/sys/fs/cgroup/cgroup.subtree_control",
        "+cpu +memory +pids",
    )?;
    let parent = Path::new("/sys/fs/cgroup/md-sim-check");
    fs::create_dir(parent)?;
    fs::write(parent.join("cgroup.subtree_control"), "+cpu +memory +pids")?;
    fs::create_dir(parent.join("supervisor"))?;
    fs::create_dir(parent.join("runs"))?;
    fs::write(
        parent.join("runs/cgroup.subtree_control"),
        "+cpu +memory +pids",
    )?;
    fs::write(
        parent.join("supervisor/cgroup.procs"),
        std::process::id().to_string(),
    )?;
    for path in [
        parent.to_path_buf(),
        parent.join("cgroup.procs"),
        parent.join("runs"),
        parent.join("runs/cgroup.procs"),
        parent.join("runs/cgroup.subtree_control"),
    ] {
        chown(&path)?;
    }
    fs::create_dir("/work/aggregate")?;
    chown(Path::new("/work/aggregate"))?;
    ensure!(
        unsafe { libc::setgroups(0, std::ptr::null()) } == 0
            && unsafe { libc::setgid(1000) } == 0
            && unsafe { libc::setuid(1000) } == 0,
        "dedicated test UID setup failed"
    );
    let store = Store::open(Path::new("/work/aggregate/store"), 64 * 1024 * 1024)?;
    let inputs: Vec<Mount> = serde_json::from_slice(&fs::read("/aggregate-inputs.json")?)?;
    for input in &inputs {
        store.import(Path::new(&input.guest_path), &input.object)?;
    }
    let mut events = Vec::new();
    for kind in ["ok", "cpu", "memory", "pids", "cancel"] {
        let spec = ProcessSpec {
            support_virtualization: None,
            private_channel: None,
            stdin_stream: None,
            namespace_uid: 0,
            dashboard: None,
            loopback: false,
            schema_version: "2.0".into(),
            argv: vec![
                "/probe".into(),
                "workload".into(),
                if kind == "cancel" { "sleep" } else { kind }.into(),
            ],
            inputs: inputs.clone(),
            environment: BTreeMap::new(),
            stdin_object: None,
            limits: Limits {
                wall_seconds: 12,
                cpu_seconds: if kind == "cpu" { 2 } else { 8 },
                memory_bytes: 80 * 1024 * 1024,
                processes: if kind == "pids" { 10 } else { 32 },
                trace_bytes: 64 * 1024,
                ..Limits::default()
            },
        };
        let start = Instant::now();
        let cancel = Arc::new(AtomicBool::new(false));
        let stdout_path = Path::new("/work/aggregate").join(format!("{kind}-stdout.log"));
        let trace = TraceFiles {
            stdout: fs::File::create(&stdout_path)?,
            stderr: fs::File::create(
                Path::new("/work/aggregate").join(format!("{kind}-stderr.log")),
            )?,
        };
        let cancel_task = if kind == "cancel" {
            let cancel = cancel.clone();
            Some(std::thread::spawn(move || -> Result<serde_json::Value> {
                let start = Instant::now();
                loop {
                    let bytes = fs::read(&stdout_path)?;
                    let text = std::str::from_utf8(&bytes)?;
                    if text
                        .lines()
                        .filter(|l| *l == "actual sleeping workload started")
                        .count()
                        == 4
                    {
                        let record: serde_json::Value = serde_json::from_slice(&fs::read(
                            "/work/aggregate/cancel-cgroup.json",
                        )?)?;
                        let group = Path::new("/sys/fs/cgroup/md-sim-check/runs")
                            .join(record["name"].as_str().context("group absent")?);
                        let processes = fs::read_to_string(group.join("cgroup.procs"))?;
                        ensure!(
                            processes.lines().count() >= 4,
                            "running descendants absent at cancellation"
                        );
                        cancel.store(true, Ordering::Release);
                        return Ok(
                            json!({"readiness_stdout":hex::encode(bytes),"populated_processes":processes,
                            "trigger_milliseconds":start.elapsed().as_millis(),"receipt":record}),
                        );
                    }
                    if start.elapsed() > Duration::from_secs(5) {
                        cancel.store(true, Ordering::Release);
                        anyhow::bail!("running cancellation readiness deadline");
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
            }))
        } else {
            None
        };
        let result = execute_recorded(
            &store,
            &spec,
            cancel,
            Some(trace),
            Some(&parent.join("runs")),
            Some(&Path::new("/work/aggregate").join(format!("{kind}-cgroup.json"))),
        );
        let cancel_trigger = cancel_task
            .map(|task| {
                task.join()
                    .map_err(|_| anyhow::anyhow!("cancellation task panicked"))?
            })
            .transpose()?;
        let result = result?;
        let evidence = result
            .aggregate_limits
            .as_ref()
            .context("aggregate evidence absent")?;
        ensure!(
            evidence["empty_before_removal"] == true
                && fs::read_dir(parent.join("runs"))?
                    .all(|e| e.is_ok_and(|e| !e.file_type().is_ok_and(|t| t.is_dir()))),
            "run-owned group survived cleanup"
        );
        match kind {
            "ok" => ensure!(
                result.exit_code == Some(0)
                    && hex::decode(&result.stdout)? == b"actual bounded guest execution\n",
                "normal aggregate run failed"
            ),
            "cpu" => ensure!(
                result.status == "cpu-limited"
                    && evidence["cpu"]["usage_usec"]
                        .as_u64()
                        .is_some_and(|n| (2_000_000..2_500_000).contains(&n)),
                "aggregate CPU budget was not enforced"
            ),
            "memory" => ensure!(
                result.status == "memory-limited"
                    && evidence["memory_events"]["oom_kill"]
                        .as_u64()
                        .is_some_and(|n| n > 0),
                "aggregate memory limit was not enforced"
            ),
            "pids" => ensure!(
                result.exit_code == Some(0)
                    && String::from_utf8(hex::decode(&result.stdout)?)?
                        .contains("process_creation_bounded"),
                "process creation was not bounded"
            ),
            "cancel" => ensure!(
                result.status == "cancelled" && start.elapsed() < Duration::from_secs(3),
                "aggregate cancellation failed"
            ),
            _ => unreachable!(),
        }
        events
            .push(json!({"case":kind,"spec":spec,"result":result,"cancel_trigger":cancel_trigger}));
    }
    events.push(worker_loss(&inputs, &parent.join("runs"))?);
    println!(
        "MD_SIM_AGGREGATE_EVIDENCE {}",
        json!({"passed":true,"scope":"real test workloads in one disconnected local VM; no firmware claim","uid":unsafe{libc::geteuid()},"driver":hash_file(&std::env::current_exe()?)?,"events":events})
    );
    Ok(())
}

fn worker_loss(inputs: &[Mount], parent: &Path) -> Result<serde_json::Value> {
    use md_sim_runtime::worker::{self, Request};
    let root = Path::new("/work/aggregate/worker");
    let store = Store::open(&root.join("store"), 8 * 1024 * 1024 * 1024)?;
    for input in inputs {
        store.import(Path::new(&input.guest_path), &input.object)?;
    }
    drop(store);
    let socket = Path::new("/work/aggregate/worker.sock");
    let start_worker = || -> Result<Child> {
        let child = Command::new("/aggregate-check").arg("worker").spawn()?;
        let start = Instant::now();
        while !socket.exists() {
            ensure!(
                start.elapsed() < Duration::from_secs(5),
                "worker startup deadline"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(child)
    };
    let mut child = Children(vec![start_worker()?]);
    let token = "c3".repeat(32);
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec!["/probe".into(), "workload".into(), "sleep-child".into()],
        inputs: inputs.to_vec(),
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits::default(),
    };
    let started = worker::request(
        socket,
        &Request::ExecuteToken {
            token: token.clone(),
            spec,
        },
    )?;
    let run_id = started["run_id"].as_str().context("worker run ID absent")?;
    let run = root.join("runs").join(run_id);
    let start = Instant::now();
    loop {
        if fs::read_to_string(run.join("stdout.log"))
            .unwrap_or_default()
            .contains("actual sleeping workload started")
        {
            break;
        }
        ensure!(
            start.elapsed() < Duration::from_secs(5),
            "actual workload readiness deadline"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let receipt: serde_json::Value = serde_json::from_slice(&fs::read(run.join("cgroup.json"))?)?;
    ensure!(
        receipt["state"] == "created",
        "worker did not commit cgroup ownership before execution"
    );
    let group = parent.join(receipt["name"].as_str().context("group name missing")?);
    let populated_processes = fs::read_to_string(group.join("cgroup.procs"))?;
    let readiness_stdout = fs::read(run.join("stdout.log"))?;
    ensure!(
        !populated_processes.trim().is_empty(),
        "worker group was not populated"
    );
    child.0[0].kill()?;
    child.0[0].wait()?;
    // Restart must recover the exact recorded group and the listener lease.
    child.0.push(start_worker()?);
    let start = Instant::now();
    let found = loop {
        match worker::request(
            socket,
            &Request::LookupToken {
                token: token.clone(),
            },
        ) {
            Ok(found) => break found,
            Err(_) if start.elapsed() < Duration::from_secs(5) => {
                std::thread::sleep(Duration::from_millis(20))
            }
            Err(error) => return Err(error),
        }
    };
    ensure!(
        found["state"] == "lost" && !group.exists(),
        "worker restart did not retain lost state and clean owned group"
    );
    let after: serde_json::Value = serde_json::from_slice(&fs::read(run.join("cgroup.json"))?)?;
    ensure!(
        after["state"] == "removed",
        "cgroup recovery receipt not durable"
    );
    worker::request(socket, &Request::Shutdown)?;
    ensure!(
        child.0[1].wait()?.success(),
        "restarted worker shutdown failed"
    );
    Ok(
        json!({"case":"worker-loss","actual_sigkill":true,"same_socket_restart":true,"before":receipt,"after":after,"lost_run":found,
            "readiness_stdout":hex::encode(readiness_stdout),"populated_processes":populated_processes,"unrelated_parent_preserved":parent.exists()}),
    )
}
fn prepare(directory: &Path) -> Result<()> {
    ensure!(!directory.exists(), "output must be new");
    fs::create_dir_all(directory)?;
    let directory = directory.canonicalize()?;
    let executable = std::env::current_exe()?.canonicalize()?;
    fs::copy(&executable, directory.join("driver"))?;
    fs::set_permissions(directory.join("driver"), fs::Permissions::from_mode(0o555))?;
    let store = Store::open(&directory.join("input-store"), 64 * 1024 * 1024)?;
    let inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![("/probe".into(), directory.join("driver"))],
    )?;
    let mut files = BTreeMap::<String, serde_json::Value>::new();
    fs::write(
        directory.join("inputs.json"),
        serde_json::to_vec_pretty(&inputs)?,
    )?;
    files.insert(
        "/aggregate-inputs.json".into(),
        json!({"path":directory.join("inputs.json"),"mode":292}),
    );
    let scenario = json!({"programs":{"/aggregate-check":directory.join("driver"),"/probe":directory.join("driver"),"/usr/bin/bwrap":"/usr/bin/bwrap","/usr/bin/unshare":"/usr/bin/unshare","/usr/bin/prlimit":"/usr/bin/prlimit"},"files":files,"argv":["/aggregate-check","guest"],"environment":{}});
    fs::write(
        directory.join("scenario.json"),
        serde_json::to_vec_pretty(&scenario)?,
    )?;
    println!(
        "{}",
        json!({"scenario":directory.join("scenario.json"),"driver":hash_file(&directory.join("driver"))?,"scope":"prepared only; real VM test still required"})
    );
    Ok(())
}
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    match args.get(1).map(String::as_str) {
        Some("workload") if args.len() == 3 => workload(&args[2]),
        Some("prepare") if args.len() == 3 => prepare(&PathBuf::from(&args[2])),
        Some("guest") if args.len() == 2 => guest(),
        Some("worker") if args.len() == 2 => md_sim_runtime::worker::serve_with_limits(
            Path::new("/work/aggregate/worker"),
            Path::new("/work/aggregate/worker.sock"),
            md_sim_runtime::worker::WorkerLimits {
                cgroup_parent: Some(PathBuf::from("/sys/fs/cgroup/md-sim-check/runs")),
                ..Default::default()
            },
        ),
        _ => anyhow::bail!("usage: aggregate_limits_check prepare NEW_DIR | guest | workload KIND"),
    }
}

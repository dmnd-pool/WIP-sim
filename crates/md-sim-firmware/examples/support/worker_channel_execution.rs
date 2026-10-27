//! Execute the same locked process through a real native worker and a VM client.
use super::{channel_artifacts, device_transport::DeviceTransport, prepared, retention};
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::{
    executor::local_tools, ExecutionScope, Executor, LockedRun, RetentionRequest, SuiteIdentity,
    WorkerExecutor, WorkerTransport,
};
use md_sim_runtime::{
    isolation::{save_json, Limits, Mount, ProcessResult, ProcessSpec},
    store::{hash_file, ObjectId, Store},
    worker::Request,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

pub const ARTIFACTS: &str = "MD_SIM_VIRTUAL_WORKER_ARTIFACTS ";
const RESULT_BOUND: u64 = (192 << 20) + (1 << 20);
const COMPRESSED_BOUND: u64 = 48 << 20;
const META_FILES: [&str; 8] = [
    "executor.json",
    "artifact-transfer.json",
    "worker-request.json",
    "worker-run.json",
    "worker-cleanup-response.json",
    "worker-cleanup.json",
    "worker-error.json",
    "worker-retention.json",
];

fn fixture(
    store: &Store,
    executable: &Path,
    payload: &Path,
    long_running: bool,
) -> Result<LockedRun> {
    let mut mounts = md_sim_firmware::lab::elf_closure(
        store,
        vec![("/execution-check".into(), executable.into())],
    )?;
    let challenge = hash_file(payload)?;
    store.import(payload, &challenge)?;
    mounts.push(Mount {
        object: challenge,
        guest_path: "/challenge.bin".into(),
    });
    let process = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        schema_version: "2.0".into(),
        argv: vec![
            "/execution-check".into(),
            if long_running {
                "__execution_long"
            } else {
                "__execution"
            }
            .into(),
        ],
        inputs: mounts,
        stdin_object: None,
        environment: BTreeMap::new(),
        loopback: false,
        dashboard: None,
        limits: Limits {
            wall_seconds: if long_running { 70 } else { 15 },
            cpu_seconds: 5,
            memory_bytes: 256 << 20,
            processes: 32,
            file_bytes: 8 << 20,
            trace_bytes: 64 << 10,
            tmp_bytes: 16 << 20,
        },
    };
    let reporter = hash_file(executable)?;
    let inputs = process
        .inputs
        .iter()
        .map(|mount| {
            (
                format!("runtime:{}", mount.guest_path),
                mount.object.clone(),
            )
        })
        .collect();
    Ok(LockedRun {
        name: "native-worker-channel-challenge".into(),
        scope: ExecutionScope::ProcessPrimitive {
            description: "Actual isolated support process; no firmware capability claim".into(),
        },
        suite: SuiteIdentity {
            id: "native-worker-channel-challenge".into(),
            version: "1".into(),
            suite: reporter.clone(),
            verifier: reporter,
            required_gates: ["isolated-process-exit".into()].into(),
        },
        process: Some(process),
        inputs,
        backend_tools: local_tools()?,
        policies: json!({"challenge":"fresh captured payload","transport":"private virtual device"}),
        capability_limits: vec!["No firmware, dashboard, SSH or physical-host pass".into()],
    })
}

pub fn stage(
    store: &Store,
    executable: &Path,
    payload: &Path,
    out: &Path,
    source: Option<&Path>,
    ssh: bool,
) -> Result<BTreeMap<String, Value>> {
    let shared = source.is_some() && ssh;
    fs::create_dir_all(out)?;
    let run = if let Some(source) = source {
        prepared::freeze(source, out, store)?
    } else {
        let run = fixture(store, executable, payload, ssh)?;
        save_json(
            &out.join("process-spec.json"),
            run.process.as_ref().context("process absent")?,
        )?;
        run
    };
    run.validate()?;
    let process = run.process.as_ref().context("process absent")?;
    ensure!(
        process.namespace_uid == 0
            && !process.loopback
            && process.dashboard.is_none()
            && process.stdin_stream.is_none()
            && process.private_channel.is_some() == shared,
        "channel process requires the normal root mapping and no host network bridge"
    );
    ensure!(
        process.limits.wall_seconds <= 360
            && process.limits.cpu_seconds <= 330
            && process.limits.memory_bytes <= 3 << 30
            && process.limits.processes <= 64
            && process.limits.file_bytes <= 64 << 20
            && process.limits.trace_bytes <= 64 << 20
            && process.limits.tmp_bytes <= 128 << 20,
        "selected process exceeds the declared channel profile"
    );
    let mut unique = BTreeMap::new();
    for object in run.inputs.values() {
        unique.insert(&object.sha256, object);
    }
    ensure!(
        unique.len() <= 128 && unique.values().map(|o| o.size).sum::<u64>() <= 384 << 20,
        "channel input closure exceeds support budget"
    );
    let lock = out.join("locked-run.json");
    save_json(&lock, &run)?;
    let mode = out.join("channel-mode.json");
    save_json(
        &mode,
        &json!({"firmware_reference":source.is_some(),
        "locked_run":hash_file(&lock)?,"locked_run_hash":run.hash()?}),
    )?;
    let mut files = BTreeMap::from([
        (
            "/execution-lock.json".into(),
            json!({"path":lock.canonicalize()?,"mode":420}),
        ),
        (
            "/execution-mode.json".into(),
            json!({"path":mode.canonicalize()?,"mode":420}),
        ),
    ]);
    for object in unique.values() {
        files.insert(
            format!("/execution-objects/{}", object.sha256),
            json!({"path":store.verified_path(object)?.canonicalize()?,"mode":420}),
        );
    }
    Ok(files)
}

fn artifacts(out: &Path, run: &LockedRun, include_result: bool) -> Result<Value> {
    let mut files = BTreeMap::new();
    for name in META_FILES {
        let path = out.join(name);
        if path.exists() {
            files.insert(
                name,
                channel_artifacts::export_file(&path, 1 << 20, 1 << 20)?,
            );
        }
    }
    let path = out.join("worker-result.json");
    if include_result && path.exists() {
        files.insert(
            "worker-result.json",
            channel_artifacts::export_file(&path, RESULT_BOUND, COMPRESSED_BOUND)?,
        );
    }
    let mut traces = BTreeMap::new();
    for name in ["worker-stdout.log", "worker-stderr.log"] {
        let path = out.join(name);
        if path.exists() {
            traces.insert(name, hash_file(&path)?);
        }
    }
    Ok(
        json!({"locked_run_hash":run.hash()?,"files":files,"traces":traces,
        "retention_contract":"Cleanup requires the outer host's exact durable result receipt. Active-job survival across client or worker loss is a separate gate."}),
    )
}

pub fn run(device: File, mut receipt: File) -> Result<Value> {
    let mode: Value = serde_json::from_slice(&prepared::bounded(
        Path::new("/execution-mode.json"),
        65536,
    )?)?;
    let bytes = prepared::bounded(Path::new("/execution-lock.json"), 4 << 20)?;
    ensure!(
        mode["locked_run"] == serde_json::to_value(hash_file(Path::new("/execution-lock.json"))?)?,
        "guest locked run identity differs"
    );
    let run: LockedRun = serde_json::from_slice(&bytes)?;
    run.validate()?;
    ensure!(
        mode["locked_run_hash"] == run.hash()?,
        "guest run hash differs"
    );
    let firmware = mode["firmware_reference"]
        .as_bool()
        .context("execution mode absent")?;
    let store = Store::open(Path::new("/work/execution-inputs"), 512 << 20)?;
    println!("MD_SIM_WORKER_CHANNEL_PHASE importing-captured-inputs");
    for object in run.inputs.values() {
        store.import(
            &Path::new("/execution-objects").join(&object.sha256),
            object,
        )?;
    }
    let out = Path::new("/work/channel-result");
    fs::create_dir(out)?;
    let mut worker = WorkerExecutor {
        transport: DeviceTransport::new(device)?,
    };
    let before = worker.identity()?;
    let expected: ObjectId = serde_json::from_slice(&prepared::bounded(
        Path::new("/worker-identity.json"),
        65536,
    )?)?;
    ensure!(before.build == expected, "wrong worker execution build");
    save_json(&out.join("executor.json"), &before)?;
    println!("MD_SIM_WORKER_CHANNEL_PHASE transferring-and-executing-locked-run");
    let mut exported = false;
    let mut retain = |request: &RetentionRequest| {
        println!(
            "{}{}",
            retention::REQUEST,
            json!({"identity":request.identity,
            "artifacts":artifacts(out,&run,true)?})
        );
        exported = true;
        retention::read_ack(&mut receipt, &request.identity)
    };
    let result = match worker.run_with_retention(
        &store,
        &run,
        Arc::new(AtomicBool::new(false)),
        out,
        &mut retain,
    ) {
        Ok(result) => result,
        Err(error) => {
            save_json(
                &out.join("worker-error.json"),
                &json!({"error":format!("{error:#}")}),
            )?;
            println!("{ARTIFACTS}{}", artifacts(out, &run, !exported)?);
            return Err(error);
        }
    };
    // Retain the received bytes before any later identity or observation
    // check can fail. Those failures must not discard the completed result.
    println!("{ARTIFACTS}{}", artifacts(out, &run, false)?);
    let after = worker.identity()?;
    ensure!(
        md_sim_coordinator::identity(&before)? == md_sim_coordinator::identity(&after)?,
        "worker execution identity changed"
    );
    let mut passed =
        result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated;
    if !firmware {
        let stdout = String::from_utf8(hex::decode(&result.stdout)?)?;
        let observations: Vec<Value> = stdout
            .lines()
            .filter_map(|line| line.strip_prefix("MD_SIM_CHANNEL_PROCESS "))
            .map(serde_json::from_str)
            .collect::<std::result::Result<_, _>>()?;
        passed &= observations.len() == 1
            && observations[0]["challenge"]
                == serde_json::to_value(hash_file(Path::new("/payload.bin"))?)?;
    }
    // The final observation contains only identities, not another export.
    ensure!(
        worker.transport.request(&Request::Shutdown)?["stopping"] == true,
        "worker shutdown acknowledgment absent"
    );
    let evidence = json!({"passed":passed,"locked_run_hash":run.hash()?,
        "result_identity":hash_file(&out.join("worker-result.json"))?,
        "cleanup_identity":hash_file(&out.join("worker-cleanup.json"))?,
        "firmware_reference":firmware,
        "scope":"Actual locked process transport; firmware boot, mining, Core and dashboard gates require separate observation and verification",
        "retention_limit":"Exact result retained by the outer host before cleanup; active-job reconnect and worker-root loss remain separate gates"});
    println!("MD_SIM_VIRTUAL_WORKER_EXECUTION {evidence}");
    ensure!(
        passed,
        "native worker process failed; exact available artifacts exported"
    );
    Ok(evidence)
}

/// Restore only fixed, run-owned filenames, and verify bytes before observation.
pub fn restore(record: &Value, out: &Path) -> Result<Option<ProcessResult>> {
    let run: LockedRun =
        serde_json::from_slice(&prepared::bounded(&out.join("locked-run.json"), 4 << 20)?)?;
    ensure!(
        record["locked_run_hash"] == run.hash()?,
        "exported result belongs to another run"
    );
    let files = record["files"]
        .as_object()
        .context("exported files absent")?;
    ensure!(
        files.len() <= META_FILES.len() + 1,
        "exported file count exceeds bound"
    );
    for (name, value) in files {
        ensure!(
            META_FILES.contains(&name.as_str()) || name == "worker-result.json",
            "unexpected exported filename"
        );
        let export: channel_artifacts::Export = serde_json::from_value(value.clone())?;
        let result = name == "worker-result.json";
        if out.join(name).exists() {
            ensure!(
                hash_file(&out.join(name))? == export.object,
                "repeated export changed an existing artifact"
            );
        } else {
            channel_artifacts::import_file(
                &export,
                &out.join(name),
                if result { RESULT_BOUND } else { 1 << 20 },
                if result { COMPRESSED_BOUND } else { 1 << 20 },
            )?;
        }
    }
    let result_path = out.join("worker-result.json");
    if !result_path.exists() {
        return Ok(None);
    }
    let result: ProcessResult =
        serde_json::from_slice(&prepared::bounded(&result_path, RESULT_BOUND)?)?;
    fs::write(out.join("guest.log"), hex::decode(&result.stdout)?)?;
    fs::write(out.join("backend.log"), hex::decode(&result.stderr)?)?;
    for (wire, local) in [
        ("worker-stdout.log", "guest.log"),
        ("worker-stderr.log", "backend.log"),
    ] {
        if let Some(identity) = record["traces"].get(wire) {
            ensure!(
                *identity == serde_json::to_value(hash_file(&out.join(local))?)?,
                "restored trace differs from the client's saved trace"
            );
        }
    }
    ensure!(
        result.tools == run.backend_tools,
        "restored isolation tool identities differ"
    );
    // Preserve the original layout whose hash the client acknowledged.
    if out.join("result.json").exists() {
        ensure!(
            hash_file(&out.join("result.json"))? == hash_file(&result_path)?,
            "existing execution result differs"
        );
    } else {
        fs::copy(result_path, out.join("result.json"))?;
    }
    Ok(Some(result))
}

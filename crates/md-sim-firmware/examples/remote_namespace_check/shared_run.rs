//! Full selected firmware execution and joined native/pool retention over SSH.
use super::{
    shared_retention::{self, Receipt},
    worker_broker::{self, bounded},
};
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::{
    ssh_transport::SshTransport, Executor, RetentionIdentity, RetentionRequest, WorkerExecutor,
    WorkerTransport,
};
use md_sim_runtime::{
    isolation::save_json,
    store::{hash_file, Store},
    worker::Request,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::Write,
    path::Path,
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};

pub const MARKER: &str = "MD_SIM_NATIVE_SHARED_POOL ";
const RESULT_BYTES: u64 = (192 << 20) + (1 << 20);
const RESULT_COMPRESSED: u64 = 48 << 20;
const NATIVE_REQUEST: &str = "native-retention-request.json";
const EXPECTED: &str = "shared-retention-expected.json";
const ACK: &str = "shared-retention-ack.json";
const ERROR: &str = "shared-retention-error.json";

pub fn policy() -> Value {
    json!({"mode":"selected-shared-pool","support_seconds":super::shared_pool::SERVER_SECONDS,
        "ssh_request_seconds":60,"client_store_bytes":512u64<<20,"result_bytes":RESULT_BYTES,"result_compressed_bytes":RESULT_COMPRESSED,
        "pool_bundle_bytes":shared_retention::POOL_BUNDLE_BYTES,"pool_compressed_bytes":shared_retention::POOL_COMPRESSED_BYTES,
        "request_bytes":shared_retention::REQUEST_BYTES,"ack_bytes":shared_retention::ACK_BYTES,"ack_seconds":shared_retention::ACK_SECONDS,
        "retention_protocol":if super::carrier_dashboard::enabled() {shared_retention::ACCESS_SCHEMA} else {shared_retention::SCHEMA},
        "carrier_access":super::carrier_dashboard::enabled(),
        "carrier_browser":super::board_browser::enabled(),
        "access_bundle_bytes":if super::carrier_dashboard::enabled() {Some(shared_retention::ACCESS_BUNDLE_BYTES)} else {None},
        "access_compressed_bytes":if super::carrier_dashboard::enabled() {Some(shared_retention::ACCESS_COMPRESSED_BYTES)} else {None},
        "native_process_limits":"unchanged selected ProcessSpec"})
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    identity: RetentionIdentity,
    native_artifacts: Value,
}

fn artifacts(path: &Path, include_result: bool, complete: bool) -> Result<Value> {
    let run = worker_broker::run()?;
    let mut artifact = worker_broker::export_attempt(
        path,
        &run,
        include_result,
        complete,
        (RESULT_BYTES, RESULT_COMPRESSED),
    )?;
    artifact["retention_contract"]=json!("Exact native result, independent pool bundle and any selected access bundle must be retained before the joined outer receipt permits cleanup.");
    Ok(artifact)
}

fn retain(request: &RetentionRequest) -> Result<RetentionIdentity> {
    let out = Path::new("/work/attempt");
    ensure!(
        request.result_path == out.join("worker-result.json")
            && hash_file(&request.result_path)? == request.identity.result,
        "shared native retention path or identity differs"
    );
    let pending = Pending {
        identity: request.identity.clone(),
        native_artifacts: artifacts(out, true, true)?,
    };
    ensure!(
        serde_json::to_vec_pretty(&pending)?.len() < shared_retention::REQUEST_BYTES,
        "native retention request exceeds declared bound"
    );
    save_json(&Path::new("/work").join(NATIVE_REQUEST), &pending)?;
    let deadline = Instant::now() + Duration::from_secs(super::shared_pool::SERVER_SECONDS);
    loop {
        if Path::new("/work").join(ERROR).exists() {
            let error: Value =
                serde_json::from_slice(&bounded(&Path::new("/work").join(ERROR), 65536)?)?;
            anyhow::bail!("joined retention failed: {}", error["error"]);
        }
        if Path::new("/work").join(ACK).exists() {
            let expected: Receipt =
                serde_json::from_slice(&bounded(&Path::new("/work").join(EXPECTED), 4096)?)?;
            let actual: Receipt =
                serde_json::from_slice(&bounded(&Path::new("/work").join(ACK), 4096)?)?;
            ensure!(
                expected.native == request.identity && actual == expected,
                "joined retention receipt differs"
            );
            save_json(Path::new("/work/shared-retention.json"), &actual)?;
            return Ok(actual.native);
        }
        ensure!(
            Instant::now() < deadline,
            "joined retention receipt deadline"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn client(events: &mut Vec<Value>) -> Result<()> {
    let namespace: Value =
        serde_json::from_slice(&bounded(Path::new("/work/namespace.json"), 65536)?)?;
    let mut interfaces = namespace["interfaces"]
        .as_array()
        .context("client interfaces absent")?
        .iter()
        .map(|interface| {
            interface["ifname"]
                .as_str()
                .context("client interface name absent")
        })
        .collect::<Result<Vec<_>>>()?;
    interfaces.sort_unstable();
    ensure!(
        interfaces == ["client0", "lo"],
        "client has an unexpected miner interface"
    );
    ensure!(
        !namespace["routes"]
            .as_str()
            .context("client routes absent")?
            .contains("10.0.2."),
        "client has a miner-network route"
    );
    let run = worker_broker::run()?;
    ensure!(
        run.process
            .as_ref()
            .is_some_and(|process| process.private_channel.is_some()),
        "full shared process lacks declared carrier"
    );
    let store = Store::open(Path::new("/work/inputs"), 512 << 20)?;
    for object in run.inputs.values() {
        store.import(
            &Path::new("/broker-inputs/objects").join(&object.sha256),
            object,
        )?;
    }
    let mut config = super::ssh_config();
    config.timeout_seconds = 60;
    let mut executor = WorkerExecutor {
        transport: SshTransport::new_streaming(config)?,
    };
    events.push(executor.transport.evidence());
    fs::create_dir("/work/attempt")?;
    save_json(
        Path::new("/work/attempt/executor.json"),
        &executor.identity()?,
    )?;
    let mut observer = super::carrier_dashboard_client::Observer::start(&run)?;
    let mut access = None;
    let result = executor.run_with_retention(
        &store,
        &run,
        Arc::new(AtomicBool::new(false)),
        Path::new("/work/attempt"),
        &mut |request| {
            if let Some(mut observer) = observer.take() {
                let observed = observer
                    .finish()
                    .unwrap_or_else(|error| json!({"passed":false,"error":format!("{error:#}")}));
                save_json(Path::new("/work/carrier-observer.json"), &observed)?;
                access = Some(observed);
            }
            retain(request)
        },
    )?;
    save_json(
        Path::new("/work/shared-result.json"),
        &json!({"status":result.status,"exit_code":result.exit_code,"trace_truncated":result.trace_truncated,
        "result":hash_file(Path::new("/work/attempt/worker-result.json"))?,"locked_run_hash":run.hash()?}),
    )?;
    ensure!(
        executor.transport.request(&Request::Shutdown)?["stopping"] == true,
        "native worker shutdown acknowledgment absent"
    );
    if run.policies.get("carrier_dashboard").is_some() {
        ensure!(
            access.is_some_and(|report| report["passed"] == true),
            "selected carrier dashboard client did not pass; inspect retained access evidence"
        );
    }
    Ok(())
}

fn role(role: &str) -> Result<super::Owned> {
    Ok(super::Owned(
        super::role_command(role)?
            .stdin(std::process::Stdio::null())
            .stdout(File::create(format!("/work/{role}.stdout"))?)
            .stderr(File::create(format!("/work/{role}.stderr"))?)
            .spawn()?,
    ))
}

fn joined_request(pending: Pending) -> Result<shared_retention::Request> {
    let access = super::carrier_artifacts::export(&pending.identity)?;
    let bundle = super::pool_service::bundle(Path::new("/work/server-root"), &pending.identity)?;
    let path = Path::new("/work/shared-pool-bundle.json");
    ensure!(
        (serde_json::to_vec_pretty(&bundle)?.len() as u64) < shared_retention::POOL_BUNDLE_BYTES,
        "pool bundle exceeds declared bound"
    );
    save_json(path, &bundle)?;
    let export = super::channel_artifacts::export_file(
        path,
        shared_retention::POOL_BUNDLE_BYTES,
        shared_retention::POOL_COMPRESSED_BYTES,
    )?;
    let request = shared_retention::Request {
        schema_version: if access.is_some() {
            shared_retention::ACCESS_SCHEMA
        } else {
            shared_retention::SCHEMA
        }
        .into(),
        identity: Receipt {
            native: pending.identity,
            pool_bundle: export.object.clone(),
            access_bundle: access.as_ref().map(|bundle| bundle.object.clone()),
        },
        native_artifacts: pending.native_artifacts,
        pool_bundle: export,
        access_bundle: access,
    };
    request.validate()?;
    ensure!(
        serde_json::to_vec(&request)?.len() + shared_retention::REQUEST.len()
            < shared_retention::REQUEST_BYTES,
        "joined retention request exceeds declared bound"
    );
    Ok(request)
}

fn supervise(events: &mut Vec<Value>) -> Result<()> {
    let mut receipt = super::virtual_port::open("md-sim.retention")?;
    let mut server = role("server")?;
    let mut progress = super::shared_progress::Progress::new();
    let deadline = Instant::now() + Duration::from_secs(super::shared_pool::SERVER_SECONDS);
    while !Path::new("/work/server-root/broker-ready.json").exists() {
        progress.emit("server_start", &mut server.0, None);
        ensure!(
            server.0.try_wait()?.is_none() && Instant::now() < deadline,
            "shared server startup failed"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut client = role("client")?;
    let mut acknowledged = false;
    loop {
        progress.emit("execution", &mut server.0, Some(&mut client.0));
        if !acknowledged
            && Path::new("/work/client-root").join(NATIVE_REQUEST).exists()
            && Path::new("/work/server-root/pool-outcome.json").exists()
            && (!super::carrier_dashboard::enabled()
                || Path::new("/work/server-root/carrier-outcome.json").exists())
        {
            let pending: Pending = serde_json::from_slice(&bounded(
                &Path::new("/work/client-root").join(NATIVE_REQUEST),
                shared_retention::REQUEST_BYTES as u64,
            )?)?;
            ensure!(
                pending.identity.locked_run_hash == worker_broker::run()?.hash()?,
                "native pending retention selected a different lock"
            );
            let request = joined_request(pending)?;
            save_json(
                &Path::new("/work/client-root").join(EXPECTED),
                &request.identity,
            )?;
            println!(
                "{}{}",
                shared_retention::REQUEST,
                serde_json::to_string(&request)?
            );
            std::io::stdout().flush()?;
            match shared_retention::read_ack(&mut receipt, &request.identity) {
                Ok(ack) => {
                    save_json(&Path::new("/work/client-root").join(ACK), &ack)?;
                    acknowledged = true;
                }
                Err(error) => {
                    save_json(
                        &Path::new("/work/client-root").join(ERROR),
                        &json!({"error":format!("{error:#}")}),
                    )?;
                    return Err(error);
                }
            }
        }
        if let Some(status) = client.0.try_wait()? {
            ensure!(
                status.success(),
                "shared full-execution client failed: {status}"
            );
            break;
        }
        ensure!(
            server.0.try_wait()?.is_none() && Instant::now() < deadline,
            "shared execution server exit or deadline"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    ensure!(
        acknowledged,
        "client ended without joined retention acknowledgment"
    );
    loop {
        progress.emit("server_stop", &mut server.0, Some(&mut client.0));
        if let Some(status) = server.0.try_wait()? {
            ensure!(status.success(), "shared server failed: {status}");
            break;
        }
        ensure!(Instant::now() < deadline, "shared server stop deadline");
        std::thread::sleep(Duration::from_millis(20));
    }
    let server_ns: Value = serde_json::from_slice(&bounded(
        Path::new("/work/server-root/namespace.json"),
        65536,
    )?)?;
    let client_ns: Value = serde_json::from_slice(&bounded(
        Path::new("/work/client-root/namespace.json"),
        65536,
    )?)?;
    for name in ["net", "mnt", "pid", "uts", "ipc"] {
        ensure!(
            server_ns["namespaces"][name] != client_ns["namespaces"][name],
            "shared client/server namespace is not distinct"
        );
    }
    events.push(json!({"shared_server_namespace":server_ns,"shared_client_namespace":client_ns,"joined_retention_acknowledged":true}));
    for ns in ["md-worker", "md-client"] {
        super::invoke("/usr/bin/ip", &["netns", "delete", ns])?;
    }
    Ok(())
}

pub fn orchestrate(events: &mut Vec<Value>) -> Result<()> {
    let execution = supervise(events);
    let out = Path::new("/work/client-root/attempt");
    let exported = artifacts(
        out,
        execution.is_err(),
        out.join("worker-cleanup.json").exists(),
    );
    let receipt = bounded(Path::new("/work/client-root/shared-retention.json"), 4096)
        .ok()
        .map(|bytes| serde_json::from_slice::<Receipt>(&bytes))
        .transpose()?;
    let result = bounded(Path::new("/work/client-root/shared-result.json"), 65536)
        .ok()
        .map(|bytes| serde_json::from_slice::<Value>(&bytes))
        .transpose()?
        .unwrap_or(Value::Null);
    let outcome = bounded(Path::new("/work/server-root/pool-outcome.json"), 65536)
        .ok()
        .map(|bytes| serde_json::from_slice::<Value>(&bytes))
        .transpose()?
        .unwrap_or(Value::Null);
    let passed = execution.is_ok()
        && exported.is_ok()
        && receipt.is_some()
        && result["status"] == "exited"
        && result["exit_code"] == 0
        && result["trace_truncated"] == false
        && outcome["complete"] == true
        && outcome["exit_code"] == 0
        && outcome["signal"].is_null()
        && outcome["cancelled"] == false;
    let error = execution
        .as_ref()
        .err()
        .map(|error| format!("{error:#}"))
        .or_else(|| exported.as_ref().err().map(|error| format!("{error:#}")));
    let report = json!({"passed":passed,"shared_receipt":receipt,"artifacts":exported.ok(),"native_outcome":result,"pool_outcome":outcome,"error":error,
        "scope":"Actual selected native firmware process, private external pool, and SSH. Joined artifact retention and process outcomes only; independent mining audits, Core and exact-target gates remain separate."});
    ensure!(
        serde_json::to_vec(&report)?.len() + MARKER.len() < shared_retention::REQUEST_BYTES,
        "shared final report exceeds declared bound"
    );
    println!("{MARKER}{report}");
    std::io::stdout().flush()?;
    execution?;
    ensure!(
        passed,
        "shared process or independent pool did not complete successfully"
    );
    Ok(())
}

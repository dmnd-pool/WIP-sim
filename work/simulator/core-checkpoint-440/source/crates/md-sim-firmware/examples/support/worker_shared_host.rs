//! Retain the exact native result and its independently produced pool evidence
//! before returning the joined cleanup receipt.
use super::{
    channel_artifacts, execution, execution_report, prepared, retention, shared_pool,
    shared_retention as wire,
};
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::LockedRun;
use md_sim_runtime::{
    isolation::{self, ProcessResult, ProcessSpec},
    store::{hash_file, ObjectId, Store},
    worker::process_identity,
};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

#[path = "worker_shared_access.rs"]
mod access;

pub const MARKER: &str = "MD_SIM_NATIVE_SHARED_POOL ";

fn mount<'a>(process: &'a ProcessSpec, path: &str) -> Result<&'a ObjectId> {
    process
        .inputs
        .iter()
        .find(|entry| entry.guest_path == path)
        .map(|entry| &entry.object)
        .context("shared process input absent")
}

fn retain_bytes(value: &wire::Bytes, path: &Path, maximum: u64) -> Result<()> {
    value.object.validate()?;
    ensure!(
        value.object.size <= maximum && value.hex.len() as u64 == value.object.size * 2,
        "pool log exceeds its declared size or bound"
    );
    let bytes = hex::decode(&value.hex)?;
    ensure!(
        md_sim_firmware::prepare::hash_bytes(&bytes) == value.object.sha256,
        "independent pool log hash differs"
    );
    if path.exists() {
        ensure!(
            hash_file(path)? == value.object,
            "existing pool log differs"
        );
        return Ok(());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn receive(bytes: &[u8], out: &Path) -> Result<Vec<u8>> {
    ensure!(
        bytes.len() <= wire::REQUEST_BYTES,
        "shared retention request too large"
    );
    let request: wire::Request = serde_json::from_slice(bytes)?;
    request.validate()?;
    // The common importer validates the exact native bytes/token/run/hash.
    // Its native-only receipt is insufficient for this mode; no wire
    // acknowledgment is returned until the independent pool is also durable.
    let _native_ack = retention::receive(
        &json!({"identity":request.identity.native,
        "artifacts":request.native_artifacts}),
        out,
    )?;
    let pool = out.join("shared-pool");
    fs::create_dir_all(pool.join("sessions"))?;
    let bundle_path = pool.join("pool-bundle.json");
    channel_artifacts::import_file(
        &request.pool_bundle,
        &bundle_path,
        wire::POOL_BUNDLE_BYTES,
        wire::POOL_COMPRESSED_BYTES,
    )?;
    let bundle: wire::PoolBundle =
        serde_json::from_slice(&prepared::bounded(&bundle_path, wire::POOL_BUNDLE_BYTES)?)?;
    let run: LockedRun =
        serde_json::from_slice(&prepared::bounded(&out.join("locked-run.json"), 4 << 20)?)?;
    run.validate()?;
    let process = run.process.as_ref().context("shared process absent")?;
    ensure!(
        process.private_channel.is_some(),
        "shared run has no private carrier"
    );
    ensure!(
        bundle.schema_version == wire::POOL_SCHEMA
            && bundle.locked_run_hash == run.hash()?
            && bundle.locked_run_hash == request.identity.native.locked_run_hash
            && bundle.token == request.identity.native.token
            && bundle.run_id == request.identity.native.run_id
            && bundle.process == process_identity(process)?
            && &bundle.manifest == mount(process, "/shared-pool.json")?
            && &bundle.pool_input == mount(process, "/private-pool.json")?,
        "pool bundle belongs to another selected execution"
    );
    let manifest_path = out.join("shared-pool.json");
    ensure!(
        hash_file(&manifest_path)? == bundle.manifest
            && hash_file(&out.join("pool-input.json"))? == bundle.pool_input,
        "retained shared inputs differ"
    );
    let (manifest, _) = shared_pool::read(&manifest_path)?;
    ensure!(
        manifest.pool_verifier == bundle.pool_verifier
            && manifest.support_runner == bundle.support_runner
            && manifest.support_backend == bundle.support_backend
            && manifest.support_kernel == bundle.support_kernel
            && serde_json::to_value(&manifest.carrier)?
                == serde_json::to_value(&process.private_channel)?,
        "pool support identities or carrier policy differ"
    );
    let proof: Value =
        serde_json::from_slice(&prepared::bounded(&pool.join("support-proof.json"), 65536)?)?;
    for (name, expected) in [
        ("pool_verifier", &bundle.pool_verifier),
        ("support_runner", &bundle.support_runner),
        ("support_backend", &bundle.support_backend),
        ("support_kernel", &bundle.support_kernel),
        ("manifest", &bundle.manifest),
        ("pool_input", &bundle.pool_input),
    ] {
        ensure!(
            proof[name] == serde_json::to_value(expected)?,
            "actual outer support input differs from pool bundle"
        );
    }
    access::retain(&request, &run, &proof, out)?;
    retain_bytes(&bundle.stdout, &pool.join("pool.stdout"), 8 << 20)?;
    retain_bytes(&bundle.stderr, &pool.join("pool.stderr"), 2 << 20)?;
    ensure!(
        bundle.sessions.len() <= 128,
        "too many pool session artifacts"
    );
    for (name, bytes) in &bundle.sessions {
        let index: u32 = name
            .strip_prefix("session-")
            .and_then(|name| name.strip_suffix(".jsonl"))
            .context("invalid pool session filename")?
            .parse()?;
        ensure!(
            index < 128 && *name == format!("session-{index}.jsonl"),
            "unsafe pool session filename"
        );
        retain_bytes(bytes, &pool.join("sessions").join(name), 512 << 10)?;
    }
    File::open(pool.join("sessions"))?.sync_all()?;
    isolation::save_json(&pool.join("retention-receipt.json"), &request.identity)?;
    File::open(&pool)?.sync_all()?;
    File::open(out)?.sync_all()?;
    let mut ack = serde_json::to_vec(&request.identity)?;
    ack.push(b'\n');
    ensure!(
        ack.len() <= wire::ACK_BYTES,
        "joined acknowledgment exceeds bound"
    );
    Ok(ack)
}

pub fn execute(
    store: &Store,
    spec: &ProcessSpec,
    out: &Path,
    execution_out: &Path,
) -> Result<ProcessResult> {
    retention::execute_with_receiver(
        store,
        spec,
        out,
        execution_out,
        wire::REQUEST,
        wire::REQUEST_BYTES,
        |bytes| receive(bytes, execution_out),
    )
}

pub fn restore(record: &Value, out: &Path) -> Result<()> {
    let result = execution::restore(&record["artifacts"], out)?;
    ensure!(
        record["passed"] == true,
        "shared SSH execution failed: {}",
        record["error"]
    );
    let result = result.context("retained native result absent")?;
    let receipt: wire::Receipt = serde_json::from_slice(&prepared::bounded(
        &out.join("shared-pool/retention-receipt.json"),
        wire::ACK_BYTES as u64,
    )?)?;
    ensure!(
        record["shared_receipt"] == serde_json::to_value(&receipt)?,
        "guest and host joined retention receipts differ"
    );
    let worker_receipt: md_sim_coordinator::RetentionIdentity = serde_json::from_slice(
        &prepared::bounded(&out.join("worker-retention.json"), wire::ACK_BYTES as u64)?,
    )?;
    let host_receipt: md_sim_coordinator::RetentionIdentity = serde_json::from_slice(
        &prepared::bounded(&out.join("host-retention.json"), wire::ACK_BYTES as u64)?,
    )?;
    ensure!(
        worker_receipt == receipt.native
            && host_receipt == receipt.native
            && !out.join("host-retention-error.json").exists(),
        "native retention evidence differs"
    );
    let cleanup: Value =
        serde_json::from_slice(&prepared::bounded(&out.join("worker-cleanup.json"), 65536)?)?;
    ensure!(
        cleanup["removed"] == true
            && cleanup["run_id"] == receipt.native.run_id
            && cleanup["acknowledged_result"] == serde_json::to_value(&receipt.native.result)?,
        "shared native cleanup differs from the exact retained result"
    );
    ensure!(
        result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
        "native shared process did not complete successfully"
    );
    let channel = result
        .private_channel
        .as_ref()
        .context("native carrier evidence absent")?;
    ensure!(
        channel.error.is_none()
            && channel.guest_to_peer.forwarded_packets > 0
            && channel.peer_to_guest.forwarded_packets > 0,
        "native Ethernet exchange absent or failed"
    );
    let process: ProcessSpec =
        serde_json::from_slice(&prepared::bounded(&out.join("process-spec.json"), 4 << 20)?)?;
    ensure!(
        serde_json::to_value(&channel.policy)? == serde_json::to_value(&process.private_channel)?,
        "observed carrier policy differs"
    );
    access::check(out, &receipt)?;
    execution_report::retain(out, &process, &result)
}

pub fn host_receipt(record: &Value, out: &Path) -> Result<()> {
    let process: ProcessSpec =
        serde_json::from_slice(&prepared::bounded(&out.join("process-spec.json"), 4 << 20)?)?;
    let proof: Value = serde_json::from_slice(&prepared::bounded(
        &out.join("shared-pool/support-proof.json"),
        65536,
    )?)?;
    let carrier = &record["shared_carrier"];
    ensure!(
        record["executor_uid"] == 1000
            && record["executor_gid"] == 1000
            && carrier["configuration"] == proof["provider_config"]
            && carrier["process"] == serde_json::to_value(process_identity(&process)?)?
            && carrier["support_identity"] == proof["manifest"],
        "native provider belongs to another configuration or selected process"
    );
    let receipt: md_sim_runtime::private_channel::Receipt =
        serde_json::from_value(carrier["receipt"].clone())?;
    ensure!(
        receipt.error.is_none()
            && receipt.end != "timed-out"
            && receipt.guest_to_peer.forwarded_packets > 0
            && receipt.peer_to_guest.forwarded_packets > 0
            && serde_json::to_value(&receipt.policy)?
                == serde_json::to_value(&process.private_channel)?,
        "native provider carrier policy or exchange failed"
    );
    isolation::save_json(&out.join("shared-pool/provider-receipt.json"), record)
}

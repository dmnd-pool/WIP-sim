//! Join immutable native worker output to the separately retained pool process.
//! The join verifies provenance; the submission audit independently verifies work.
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::{LockedRun, RetentionIdentity};
use md_sim_runtime::{
    isolation::{Mount, ProcessSpec},
    store::{hash_file, ObjectId},
    worker::process_identity,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

use crate::{pool_input, pool_report, shared_manifest as contract, shared_retention as retention};

const META_BYTES: u64 = 4 << 20;
const LOG_BYTES: u64 = 8 << 20;
const RESULT_BYTES: u64 = md_sim_runtime::isolation::MAX_TRACE_BYTES * 2 + (1 << 20);

fn bytes(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    ensure!(file.metadata()?.is_file(), "evidence is not a regular file");
    let mut data = Vec::new();
    file.take(limit + 1).read_to_end(&mut data)?;
    ensure!(data.len() as u64 <= limit, "shared evidence size bound");
    Ok(data)
}

fn read<T: serde::de::DeserializeOwned>(path: &Path, limit: u64) -> Result<T> {
    Ok(serde_json::from_slice(&bytes(path, limit)?)?)
}

fn object(bytes: &[u8]) -> ObjectId {
    ObjectId {
        sha256: md_sim_firmware::prepare::hash_bytes(bytes),
        size: bytes.len() as u64,
    }
}

fn id(value: &Value, name: &str) -> Result<ObjectId> {
    let object: ObjectId = serde_json::from_value(value[name].clone())?;
    object.validate()?;
    Ok(object)
}

fn retained(raw: &retention::Bytes, path: &Path, limit: u64) -> Result<Vec<u8>> {
    raw.object.validate()?;
    ensure!(
        raw.object.size <= limit && raw.hex.len() as u64 == raw.object.size * 2,
        "shared byte envelope exceeds its bound"
    );
    let data = hex::decode(&raw.hex)?;
    ensure!(
        object(&data) == raw.object && bytes(path, limit)? == data,
        "retained shared bytes differ"
    );
    Ok(data)
}

fn require_binding(
    bundle: &retention::PoolBundle,
    native: &RetentionIdentity,
    process: &ObjectId,
) -> Result<()> {
    ObjectId {
        sha256: native.token.clone(),
        size: 0,
    }
    .validate()?;
    ObjectId {
        sha256: native.locked_run_hash.clone(),
        size: 0,
    }
    .validate()?;
    ensure!(
        native.run_id == format!("run-{}", native.token)
            && bundle.token == native.token
            && bundle.run_id == native.run_id
            && bundle.locked_run_hash == native.locked_run_hash
            && bundle.process == *process,
        "pool bundle belongs to another native run"
    );
    Ok(())
}

fn markers(text: &str, prefix: &str) -> Result<Vec<Value>> {
    text.lines()
        .filter_map(|line| line.strip_prefix(prefix))
        .map(|line| Ok(serde_json::from_str(line)?))
        .collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lease {
    schema_version: String,
    token: String,
    process: ObjectId,
    locked_run_hash: String,
    manifest: ObjectId,
    pool_input: ObjectId,
    pool_verifier: ObjectId,
    request_monotonic_ns: u64,
    ready_monotonic_ns: u64,
    execute_monotonic_ns: u64,
    ready: Value,
    run_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    schema_version: String,
    pool_pid: u32,
    started_monotonic_ns: u64,
    finished_monotonic_ns: u64,
    exit_code: Option<i32>,
    signal: Option<i32>,
    cancelled: bool,
    complete: bool,
}

pub fn selected(spec: &ProcessSpec) -> Result<bool> {
    match spec
        .environment
        .get(contract::ENVIRONMENT)
        .map(String::as_str)
    {
        None => Ok(false),
        Some("1") => Ok(true),
        _ => anyhow::bail!("invalid external pool flag"),
    }
}

fn mount(spec: &ProcessSpec, path: &str, expected: &ObjectId) -> Result<()> {
    mounted(&spec.inputs, path, expected)
}

fn mounted(inputs: &[Mount], path: &str, expected: &ObjectId) -> Result<()> {
    let entries: Vec<_> = inputs
        .iter()
        .filter(|input| input.guest_path == path)
        .collect();
    ensure!(
        entries.len() == 1 && entries[0].object == *expected,
        "external pool input differs from executed mount"
    );
    Ok(())
}

fn successful_outer(result: &md_sim_runtime::isolation::ProcessResult) -> Result<()> {
    ensure!(
        result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
        "outer support execution failed or is incomplete"
    );
    Ok(())
}

fn provider_evidence(
    run: &Path,
    proof: &Value,
    bundle: &retention::PoolBundle,
    spec: &ProcessSpec,
    outer_spec: &ProcessSpec,
    require_success: bool,
) -> Result<ObjectId> {
    let outer = run.parent().context("outer evidence directory absent")?;
    let path = outer.join("result.json");
    let metadata = std::fs::symlink_metadata(&path)?;
    ensure!(
        metadata.is_file() && metadata.len() <= RESULT_BYTES,
        "outer result extent or type differs"
    );
    let result: md_sim_runtime::isolation::ProcessResult =
        serde_json::from_reader(File::open(&path)?.take(RESULT_BYTES + 1))?;
    if require_success {
        successful_outer(&result)?;
    }
    let stdout = hex::decode(&result.stdout)?;
    let actual = markers(std::str::from_utf8(&stdout)?, "MD_SIM_VIRTUAL_WORKER_HOST ")?;
    let retained_path = run.join("shared-pool/provider-receipt.json");
    let host: Value = read(&retained_path, 65536)?;
    ensure!(
        actual.len() == 1 && actual[0] == host,
        "provider receipt differs from actual outer output"
    );
    let request: Value = read(&run.join("worker-request.json"), 65536)?;
    ensure!(
        host["executor_uid"] == 1000
            && host["executor_gid"] == 1000
            && host["worker_build"] == request["identity"]["build"],
        "actual native worker identity differs"
    );
    mount(outer_spec, "/worker", &id(&host, "worker_build")?)?;
    let provider = &host["shared_carrier"];
    ensure!(
        provider["schema"] == "md-sim-worker-private-carrier-result/1"
            && id(provider, "configuration")? == id(proof, "provider_config")?
            && id(provider, "process")? == bundle.process
            && id(provider, "support_identity")? == bundle.manifest,
        "provider belongs to another selected execution"
    );
    ensure!(
        id(proof, "provider_config")? == hash_file(&outer.join("provider.json"))?,
        "actual provider configuration differs"
    );
    mount(outer_spec, "/provider.json", &id(proof, "provider_config")?)?;
    let config: Value = read(&outer.join("provider.json"), META_BYTES)?;
    let configured: ProcessSpec = serde_json::from_value(config["process"].clone())?;
    configured.validate()?;
    ensure!(
        config["schema"] == "md-sim-worker-private-carrier/1"
            && process_identity(&configured)? == bundle.process
            && id(&config, "support_identity")? == bundle.manifest
            && provider["handoff_seconds"] == config["handoff_seconds"]
            && provider["relay_wall_seconds"] == config["wall_seconds"],
        "provider configuration contract differs"
    );
    if require_success {
        let receipt: md_sim_runtime::private_channel::Receipt =
            serde_json::from_value(provider["receipt"].clone())?;
        ensure!(
            receipt.error.is_none()
                && ["guest-eof", "peer-eof", "supervisor-closed", "cancelled"]
                    .contains(&receipt.end.as_str())
                && serde_json::to_value(&receipt.policy)?
                    == serde_json::to_value(&spec.private_channel)?,
            "support carrier failed or policy differs"
        );
        ensure!(
            receipt.guest_to_peer.forwarded_packets > 0
                && receipt.peer_to_guest.forwarded_packets > 0,
            "support carrier has no complete bidirectional packets"
        );
    }
    hash_file(&retained_path)
}

/// Retention may preserve failed executions. Only the independent mining audit
/// requests complete outer execution and a successful pool outcome.
pub fn verify(run: &Path, require_success: bool) -> Result<Value> {
    let directory = run.join("shared-pool");
    let spec: ProcessSpec = read(&run.join("process-spec.json"), META_BYTES)?;
    spec.validate()?;
    ensure!(
        selected(&spec)?
            && spec
                .environment
                .get("MD_SIM_S9_LOCAL_POOL")
                .is_some_and(|s| s == "1")
            && spec
                .environment
                .get("MD_SIM_S9_POOL_VM")
                .is_some_and(|s| s == "0"),
        "run does not declare the external pool mode"
    );
    let policy = spec
        .private_channel
        .as_ref()
        .context("native carrier policy absent")?;
    let (manifest, _) = contract::read(&run.join("shared-pool.json"))?;
    ensure!(
        serde_json::to_value(policy)? == serde_json::to_value(&manifest.carrier)?,
        "manifest carrier policy differs"
    );
    let bundle_path = directory.join("pool-bundle.json");
    let bundle: retention::PoolBundle = read(&bundle_path, retention::POOL_BUNDLE_BYTES)?;
    ensure!(
        bundle.schema_version == retention::POOL_SCHEMA,
        "external pool bundle schema differs"
    );
    let receipt: retention::Receipt = read(&directory.join("retention-receipt.json"), 4096)?;
    ensure!(
        receipt.pool_bundle == hash_file(&bundle_path)?,
        "pool bundle differs from acknowledged bytes"
    );
    let native: RetentionIdentity = read(&run.join("worker-retention.json"), 4096)?;
    let host: RetentionIdentity = read(&run.join("host-retention.json"), 4096)?;
    ensure!(
        receipt.native == native && native == host,
        "native retention receipts differ"
    );
    ensure!(
        native.result.size <= RESULT_BYTES,
        "native result size bound"
    );
    for name in ["worker-result.json", "result.json"] {
        let metadata = std::fs::symlink_metadata(run.join(name))?;
        ensure!(
            metadata.is_file() && metadata.len() == native.result.size,
            "native result extent or type differs"
        );
    }
    ensure!(
        native.result == hash_file(&run.join("worker-result.json"))?
            && native.result == hash_file(&run.join("result.json"))?,
        "native result bytes differ from receipt"
    );
    let locked: LockedRun = read(&run.join("locked-run.json"), META_BYTES)?;
    locked.validate()?;
    ensure!(
        locked.hash()? == native.locked_run_hash
            && locked.process.as_ref().map(process_identity).transpose()?
                == Some(process_identity(&spec)?),
        "retained locked process differs"
    );
    let request: Value = read(&run.join("worker-request.json"), 65536)?;
    let started: Value = read(&run.join("worker-run.json"), 65536)?;
    ensure!(
        request["external_retention_required"] == true
            && request["token"] == native.token
            && request["locked_run_hash"] == native.locked_run_hash
            && started["run_id"] == native.run_id
            && native.run_id == format!("run-{}", native.token),
        "selected worker request or run differs"
    );
    require_binding(&bundle, &native, &process_identity(&spec)?)?;
    ensure!(
        bundle.manifest == hash_file(&run.join("shared-pool.json"))?
            && bundle.pool_input == hash_file(&run.join("pool-input.json"))?,
        "pool manifest or input differs"
    );
    mount(&spec, contract::GUEST_MANIFEST, &bundle.manifest)?;
    mount(&spec, "/private-pool.json", &bundle.pool_input)?;
    ensure!(
        bundle.pool_verifier == manifest.pool_verifier
            && bundle.support_runner == manifest.support_runner
            && bundle.support_backend == manifest.support_backend
            && bundle.support_kernel == manifest.support_kernel,
        "pool helper identities differ from selected manifest"
    );
    let proof_path = directory.join("support-proof.json");
    let proof: Value = read(&proof_path, META_BYTES)?;
    for (name, expected) in [
        ("manifest", &bundle.manifest),
        ("pool_input", &bundle.pool_input),
        ("pool_verifier", &bundle.pool_verifier),
        ("support_runner", &bundle.support_runner),
        ("support_backend", &bundle.support_backend),
        ("support_kernel", &bundle.support_kernel),
    ] {
        ensure!(
            id(&proof, name)? == *expected,
            "actual support {name} differs"
        );
    }
    let outer = run.parent().context("outer evidence directory absent")?;
    ensure!(
        id(&proof, "outer_process")? == hash_file(&outer.join("process-spec.json"))?,
        "outer process differs from support proof"
    );
    for (name, path) in [
        ("scenario", outer.join("scenario.json")),
        ("initramfs", outer.join("client-image/initramfs.cpio.gz")),
        (
            "root_manifest",
            outer.join("client-image/root-manifest.json"),
        ),
    ] {
        ensure!(
            id(&proof, name)? == hash_file(&path)?,
            "actual support {name} bytes differ"
        );
    }
    let outer_spec: ProcessSpec = read(&outer.join("process-spec.json"), META_BYTES)?;
    outer_spec.validate()?;
    mount(&outer_spec, "/qemu", &bundle.support_backend)?;
    mount(&outer_spec, "/vmlinuz", &bundle.support_kernel)?;
    mount(&outer_spec, "/initramfs.cpio.gz", &id(&proof, "initramfs")?)?;
    let root: Vec<Mount> = read(&outer.join("client-image/root-manifest.json"), META_BYTES)?;
    mounted(&root, "/namespace-check", &bundle.support_runner)?;
    mounted(&root, "/pool-guest", &bundle.pool_verifier)?;
    mounted(&root, "/shared-pool.json", &bundle.manifest)?;
    mounted(&root, "/private-pool.json", &bundle.pool_input)?;
    let provider_receipt =
        provider_evidence(run, &proof, &bundle, &spec, &outer_spec, require_success)?;
    let stdout = retained(&bundle.stdout, &directory.join("pool.stdout"), LOG_BYTES)?;
    retained(&bundle.stderr, &directory.join("pool.stderr"), LOG_BYTES)?;
    let text = std::str::from_utf8(&stdout)?;
    let network = markers(text, "MD_SIM_SHARED_POOL_NETWORK ")?;
    ensure!(
        network.len() == 1
            && network[0]["forwarding"] == false
            && id(&network[0], "manifest")? == bundle.manifest
            && id(&network[0], "input")? == bundle.pool_input
            && id(&network[0], "verifier")? == bundle.pool_verifier,
        "actual shared pool network receipt differs"
    );
    let ready = markers(text, "MD_SIM_POOL_READY ")?;
    let lease: Lease = serde_json::from_value(bundle.lease.clone())?;
    ensure!(
        lease.schema_version == "md-sim-shared-pool-lease/1"
            && lease.token == bundle.token
            && lease.run_id == bundle.run_id
            && lease.process == bundle.process
            && lease.locked_run_hash == bundle.locked_run_hash
            && lease.manifest == bundle.manifest
            && lease.pool_input == bundle.pool_input
            && lease.pool_verifier == bundle.pool_verifier,
        "pool lease belongs to another execution"
    );
    ensure!(
        ready.len() == 1
            && ready[0] == lease.ready
            && ready[0]["endpoint"] == manifest.pool_endpoint
            && id(&ready[0], "input")? == bundle.pool_input
            && id(&ready[0], "verifier")? == bundle.pool_verifier,
        "actual pool readiness differs"
    );
    ensure!(
        lease.request_monotonic_ns <= lease.ready_monotonic_ns
            && lease.ready_monotonic_ns <= lease.execute_monotonic_ns
            && lease.ready_monotonic_ns - lease.request_monotonic_ns
                <= manifest.readiness_seconds * 1_000_000_000,
        "pool readiness ordering or budget differs"
    );
    let outcome: Outcome = serde_json::from_value(bundle.outcome.clone())?;
    ensure!(
        outcome.schema_version == "md-sim-shared-pool-outcome/1"
            && outcome.pool_pid > 0
            && lease.request_monotonic_ns <= outcome.started_monotonic_ns
            && outcome.started_monotonic_ns <= lease.ready_monotonic_ns
            && outcome.finished_monotonic_ns >= lease.execute_monotonic_ns,
        "pool lifetime ordering differs"
    );
    let sessions = markers(text, pool_report::MARKER)?;
    let collections = markers(text, "MD_SIM_PRIVATE_POOL_COLLECTION ")?;
    let input = pool_input::read(&run.join("pool-input.json"))?;
    ensure!(
        sessions.len() <= input.session_limit()?
            && bundle.sessions.len() <= input.session_limit()?,
        "pool session count bound"
    );
    let mut names = BTreeSet::new();
    for session in &sessions {
        let index = session["session_id"]
            .as_u64()
            .context("session ID absent")?;
        ensure!(
            index < input.session_limit()? as u64,
            "session ID out of range"
        );
        let name = format!("session-{index}.jsonl");
        ensure!(names.insert(name.clone()), "duplicate pool session");
        let raw = bundle
            .sessions
            .get(&name)
            .context("pool session bytes absent")?;
        let trace = retained(raw, &directory.join("sessions").join(&name), 2 << 20)?;
        ensure!(
            session["trace"]
                .as_str()
                .context("session trace absent")?
                .as_bytes()
                == trace,
            "pool result differs from actual retained session bytes"
        );
    }
    for (name, raw) in &bundle.sessions {
        let index: usize = name
            .strip_prefix("session-")
            .and_then(|s| s.strip_suffix(".jsonl"))
            .context("noncanonical session name")?
            .parse()?;
        ensure!(
            index < input.session_limit()? && *name == format!("session-{index}.jsonl"),
            "invalid session name"
        );
        if !names.contains(name) {
            retained(raw, &directory.join("sessions").join(name), 2 << 20)?;
        }
    }
    let mut outer_result = Value::Null;
    if require_success {
        ensure!(
            outcome.complete
                && !outcome.cancelled
                && outcome.exit_code == Some(0)
                && outcome.signal.is_none(),
            "external pool process failed or is incomplete"
        );
        ensure!(
            collections.len() == 1 && names.len() == bundle.sessions.len(),
            "external collection missing or session trace omitted"
        );
        ensure!(
            collections[0]["cancelled"] == false,
            "external pool collection was cancelled"
        );
        pool_report::require_completed(&sessions, &collections[0], input.session_limit()?)?;
        let record_path = directory.join("outer-result.json");
        let record: Value = read(&record_path, 65536)?;
        ensure!(
            id(&record, "process")? == id(&proof, "outer_process")?
                && id(&record, "result")? == hash_file(&outer.join("result.json"))?,
            "outer result identity differs"
        );
        outer_result = json!({"record":hash_file(&record_path)?,"process":record["process"],"result":record["result"]});
        let result: md_sim_runtime::isolation::ProcessResult =
            serde_json::from_reader(File::open(run.join("result.json"))?.take(RESULT_BYTES + 1))?;
        ensure!(
            result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
            "native execution incomplete"
        );
        let carrier = result
            .private_channel
            .context("actual carrier receipt absent")?;
        ensure!(
            carrier.error.is_none()
                && serde_json::to_value(carrier.policy)? == serde_json::to_value(policy)?,
            "actual native carrier failed or policy differs"
        );
    }
    let selected = pool_report::select_session(&sessions)?;
    Ok(json!({
        "runner_completed_pool_report":false,"external_completed_pool_report":selected.is_some(),
        "execution_result":native.result,"process_specification":hash_file(&run.join("process-spec.json"))?,
        "adaptation":hash_file(&run.join("adaptation.json"))?,"pool_input":bundle.pool_input,
        "pool":selected,"pool_sessions":sessions,"pool_collection":collections,
        "external_pool":{
            "schema_version":"md-sim-s9-external-pool-join/1","bundle":receipt.pool_bundle,
            "receipt":hash_file(&directory.join("retention-receipt.json"))?,"support_proof":hash_file(&proof_path)?,
            "provider_receipt":provider_receipt,
            "native":native,"pool_stdout":bundle.stdout.object,"pool_stderr":bundle.stderr.object,
            "manifest":bundle.manifest,"process":bundle.process,"lease":bundle.lease,"outcome":bundle.outcome,
            "outer_result":outer_result,"verifier_source":object(include_bytes!("s9_external_pool.rs"))
        },
        "pool_selection":"First authorized V1 session in allocation order. All retained sessions remain distinct; selection never depends on pass/fail.",
        "full_board_gate":"incomplete",
        "scope":"Exact external pool evidence joined to unchanged native worker result bytes. Independent header/hardware correlation and Core acceptance require separate checks."
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Value {
        serde_json::from_str(include_str!(
            "../../tests/fixtures/s9-pool-354-session0.json"
        ))
        .unwrap()
    }

    #[test]
    fn shared_success_rejects_failed_or_truncated_outer_execution() {
        let baseline = json!({"tools":{},"limits_scope":"test","isolation":"test",
            "trace_truncated":false,"stderr":"","stdout":"","wall_milliseconds":1,
            "exit_code":0,"status":"exited"});
        let good = serde_json::from_value(baseline.clone()).unwrap();
        successful_outer(&good).unwrap();
        for (field, value) in [
            ("status", json!("cancelled")),
            ("exit_code", json!(1)),
            ("exit_code", Value::Null),
            ("trace_truncated", json!(true)),
        ] {
            let mut changed = baseline.clone();
            changed[field] = value;
            assert!(successful_outer(&serde_json::from_value(changed).unwrap()).is_err());
        }
    }

    #[test]
    fn historical_pool_bytes_reject_changed_digest_or_retained_trace() {
        let fixture = fixture();
        let trace = fixture["pool"]["trace"].as_str().unwrap().as_bytes();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("session-0.jsonl");
        std::fs::write(&path, trace).unwrap();
        let raw = retention::Bytes {
            object: object(trace),
            hex: hex::encode(trace),
        };
        assert_eq!(retained(&raw, &path, 2 << 20).unwrap(), trace);
        let mut changed = raw.clone();
        changed.object.sha256 = "00".repeat(32);
        assert!(retained(&changed, &path, 2 << 20).is_err());
        changed = raw.clone();
        changed.hex.replace_range(..2, "00");
        assert!(retained(&changed, &path, 2 << 20).is_err());
        std::fs::write(&path, b"changed pool evidence").unwrap();
        assert!(retained(&raw, &path, 2 << 20).is_err());
    }

    #[test]
    fn historical_retention_identity_rejects_rebound_pool_bundle() {
        let fixture = fixture();
        let native: RetentionIdentity = serde_json::from_value(fixture["native"].clone()).unwrap();
        let input: ObjectId = serde_json::from_value(fixture["pool_input"].clone()).unwrap();
        // This is a contract regression. It does not claim that run354 had a
        // shared carrier or these support components.
        let process = object(b"labelled historical binding regression");
        let empty = retention::Bytes {
            object: object(b""),
            hex: String::new(),
        };
        let bundle = retention::PoolBundle {
            schema_version: retention::POOL_SCHEMA.into(),
            locked_run_hash: native.locked_run_hash.clone(),
            token: native.token.clone(),
            run_id: native.run_id.clone(),
            process: process.clone(),
            manifest: input.clone(),
            pool_input: input.clone(),
            pool_verifier: input.clone(),
            support_runner: input.clone(),
            support_backend: input.clone(),
            support_kernel: input,
            lease: Value::Null,
            outcome: Value::Null,
            stdout: empty.clone(),
            stderr: empty,
            sessions: Default::default(),
        };
        require_binding(&bundle, &native, &process).unwrap();
        for field in ["token", "run_id", "locked_run_hash", "process"] {
            let mut bad = bundle.clone();
            match field {
                "token" => bad.token = "ab".repeat(32),
                "run_id" => bad.run_id = format!("run-{}", "ab".repeat(32)),
                "locked_run_hash" => bad.locked_run_hash = "ab".repeat(32),
                "process" => bad.process.sha256 = "ab".repeat(32),
                _ => unreachable!(),
            }
            assert!(
                require_binding(&bad, &native, &process).is_err(),
                "accepted altered {field}"
            );
        }
    }
}

//! A bounded, explicit derivation of one prepared process. Only pool seed and
//! job time can change. The original process and every other input stay pinned.
use super::{freeze, snapshot_object};
use crate::pool_input;
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::LockedRun;
use md_sim_runtime::{
    isolation::ProcessSpec,
    store::{hash_file, ObjectId, Store},
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::CString,
    fs,
    io::Read,
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
    },
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub const RECEIPT: &str = "fresh-work.json";
const PARENT_LOCK: &str = "fresh-work-parent-lock.json";
const PARENT_PROCESS: &str = "fresh-work-parent-process.json";
const PARENT_POOL: &str = "fresh-work-parent-pool.json";
const PARENTS: [&str; 3] = [PARENT_LOCK, PARENT_PROCESS, PARENT_POOL];
const POOL: &str = "/private-pool.json";
const SCHEMA: &str = "md-sim-s9-fresh-preparation/1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema: String,
    pub parent_lock: ObjectId,
    pub parent_run_hash: String,
    pub parent_process: ObjectId,
    pub parent_pool: ObjectId,
    pub child_process: ObjectId,
    pub child_pool: ObjectId,
    pub helper: ObjectId,
    pub entropy_source: String,
    pub entropy_bytes: u32,
    pub time_before: u32,
    pub time_after: u32,
}

fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn no_derivation(source: &Path) -> Result<()> {
    for name in PARENTS.into_iter().chain([RECEIPT]) {
        ensure!(
            !exists(&source.join(name))?,
            "fresh preparation requires an original prepared input, not a derivative"
        );
    }
    Ok(())
}

pub(super) fn check_pool_mount(spec: &ProcessSpec, pool: &ObjectId) -> Result<()> {
    let mounts: Vec<_> = spec
        .inputs
        .iter()
        .filter(|m| m.guest_path == POOL)
        .collect();
    ensure!(
        mounts.len() == 1 && mounts[0].object == *pool,
        "pool sidecar differs from the sole executed pool mount"
    );
    Ok(())
}

fn supported_pool(path: &Path) -> Result<pool_input::PoolInput> {
    supported_pool_bytes(&strict_bytes(path, 4096)?)
}

fn supported_pool_bytes(bytes: &[u8]) -> Result<pool_input::PoolInput> {
    let input = pool_input::parse(bytes)?;
    ensure!(
        matches!(
            input.schema.as_str(),
            "md-sim-s9-pool-input/7" | "md-sim-s9-pool-input/8"
        ),
        "fresh preparation requires pool schema7 or schema8"
    );
    Ok(input)
}

fn replace_pool(spec: &mut ProcessSpec, pool: &ObjectId) -> Result<()> {
    ensure!(
        spec.inputs.iter().filter(|m| m.guest_path == POOL).count() == 1,
        "fresh process needs exactly one pool mount"
    );
    spec.inputs
        .iter_mut()
        .find(|m| m.guest_path == POOL)
        .unwrap()
        .object = pool.clone();
    Ok(())
}

fn check_derivation(
    parent: &ProcessSpec,
    child: &ProcessSpec,
    old_pool: &pool_input::PoolInput,
    new_pool: &pool_input::PoolInput,
    receipt: &Receipt,
) -> Result<()> {
    ensure!(
        receipt.schema == SCHEMA
            && receipt.entropy_source == "/dev/urandom"
            && receipt.entropy_bytes == 32,
        "unknown fresh preparation policy"
    );
    ensure!(
        old_pool.seed != new_pool.seed && receipt.parent_pool != receipt.child_pool,
        "fresh pool seed did not change"
    );
    ensure!(
        receipt.time_before <= receipt.time_after
            && (receipt.time_before..=receipt.time_after).contains(&new_pool.time),
        "fresh pool time is outside its recorded sampling interval"
    );
    let mut restored_pool = new_pool.clone();
    restored_pool.seed = old_pool.seed;
    restored_pool.time = old_pool.time;
    ensure!(
        serde_json::to_value(restored_pool)? == serde_json::to_value(old_pool)?,
        "fresh preparation changed pool policy"
    );
    check_pool_mount(parent, &receipt.parent_pool)?;
    check_pool_mount(child, &receipt.child_pool)?;
    let mut restored = child.clone();
    replace_pool(&mut restored, &receipt.parent_pool)?;
    ensure!(
        serde_json::to_value(restored)? == serde_json::to_value(parent)?,
        "fresh preparation changed a non-pool process input or policy"
    );
    Ok(())
}

fn retained_object(source: &Path, name: &str, expected: &ObjectId) -> Result<Vec<u8>> {
    expected.validate()?;
    let path = source.join(name);
    let bytes = strict_bytes(&path, 4 * 1024 * 1024)?;
    ensure!(
        md_sim_firmware::prepare::hash_bytes(&bytes) == expected.sha256
            && bytes.len() as u64 == expected.size,
        "fresh preparation metadata identity differs: {name}"
    );
    Ok(bytes)
}

fn strict_bytes(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.nlink() == 1 && metadata.len() <= limit,
        "invalid fresh preparation metadata file"
    );
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "fresh metadata grew beyond its bound"
    );
    Ok(bytes)
}

fn validated_receipt(source: &Path, base: &LockedRun) -> Result<(Receipt, ObjectId)> {
    let bytes = strict_bytes(&source.join(RECEIPT), 8192)?;
    let receipt_object = ObjectId {
        sha256: md_sim_firmware::prepare::hash_bytes(&bytes),
        size: bytes.len() as u64,
    };
    let receipt: Receipt = serde_json::from_slice(&bytes)?;
    let parent: LockedRun =
        serde_json::from_slice(&retained_object(source, PARENT_LOCK, &receipt.parent_lock)?)?;
    parent.validate()?;
    ensure!(
        parent.hash()? == receipt.parent_run_hash,
        "fresh parent run hash differs"
    );
    let parent_process: ProcessSpec = serde_json::from_slice(&retained_object(
        source,
        PARENT_PROCESS,
        &receipt.parent_process,
    )?)?;
    parent_process.validate()?;
    let old_pool = retained_object(source, PARENT_POOL, &receipt.parent_pool)?;
    let new_pool = retained_object(source, "pool-input.json", &receipt.child_pool)?;
    ensure!(
        base.inputs.get("process-specification") == Some(&receipt.child_process)
            && base.inputs.get("evidence:pool-input.json") == Some(&receipt.child_pool),
        "fresh receipt does not bind the executed process and pool"
    );
    check_derivation(
        &parent_process,
        base.process
            .as_ref()
            .context("fresh child process absent")?,
        &supported_pool_bytes(&old_pool)?,
        &supported_pool_bytes(&new_pool)?,
        &receipt,
    )?;
    // Reconstruct the complete parent contract from the current, validated
    // contract. Do not accept arbitrary policies or unchanged metadata merely
    // because an untrusted parent lock claims them. This narrow derivation also
    // requires identical pinned preparation/coordinator isolation tools.
    let mut expected = base.clone();
    expected.process = Some(parent_process);
    for (key, object) in [
        ("process-specification", &receipt.parent_process),
        ("evidence:pool-input.json", &receipt.parent_pool),
        ("runtime:/private-pool.json", &receipt.parent_pool),
        ("execution-and-report-build", &receipt.helper),
    ] {
        expected.inputs.insert(key.into(), object.clone());
    }
    expected.suite.suite = receipt.helper.clone();
    expected.suite.verifier = receipt.helper.clone();
    ensure!(
        serde_json::to_value(expected)? == serde_json::to_value(parent)?,
        "fresh parent contract differs from the verified unchanged execution closure"
    );
    Ok((receipt, receipt_object))
}

pub(super) fn bind(source: &Path, out: &Path, store: &Store, run: &mut LockedRun) -> Result<()> {
    if !exists(&source.join(RECEIPT))? {
        no_derivation(source)?;
        return Ok(());
    }
    let (receipt, receipt_object) = validated_receipt(source, run)?;
    for (name, expected) in [
        (PARENT_LOCK, &receipt.parent_lock),
        (PARENT_PROCESS, &receipt.parent_process),
        (PARENT_POOL, &receipt.parent_pool),
        (RECEIPT, &receipt_object),
    ] {
        let bytes = retained_object(source, name, expected)?;
        let destination = out.join(name);
        fs::write(&destination, bytes)?;
        store.import(&destination, expected)?;
        run.inputs
            .insert(format!("evidence:{name}"), expected.clone());
    }
    store.import(&snapshot_object(source, &receipt.helper)?, &receipt.helper)?;
    run.inputs
        .insert("fresh-preparation-build".into(), receipt.helper);
    run.policies["fresh_preparation"] =
        serde_json::to_value(&run.inputs["evidence:fresh-work.json"])?;
    // The new seed is fresh for this derivation. Replaying it is still the same
    // frozen identity, as the existing fresh_work policy states.
    Ok(())
}

#[allow(dead_code)] // Only the explicit preparation CLI creates a derivative.
pub fn prepare(source: &Path, output: &Path) -> Result<Receipt> {
    ensure!(!exists(output)?, "fresh output must be new");
    no_derivation(source)?;
    let directory = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    ensure!(directory.is_dir(), "fresh output parent must exist");
    let staging = tempfile::Builder::new()
        .prefix(".s9-fresh-")
        .tempdir_in(directory)?;
    let out = staging.path();
    let store = Store::open(&out.join("input-store"), 512 * 1024 * 1024)?;
    let parent = freeze(source, out, &store)?;
    let parent_process = parent.process.as_ref().context("prepared process absent")?;
    let old_pool = supported_pool(&out.join("pool-input.json"))?;
    fs::copy(out.join("process-spec.json"), out.join(PARENT_PROCESS))?;
    fs::copy(out.join("pool-input.json"), out.join(PARENT_POOL))?;
    fs::write(out.join(PARENT_LOCK), serde_json::to_vec_pretty(&parent)?)?;
    let time_before = epoch()?;
    let generated: pool_input::PoolInput = serde_json::from_value(pool_input::prepare(
        old_pool.seconds,
        old_pool.difficulty_exponents,
        old_pool.regtest_anchor.clone(),
    )?)?;
    let time_after = epoch()?;
    let mut new_pool = old_pool.clone();
    new_pool.seed = generated.seed;
    new_pool.time = generated.time;
    fs::write(
        out.join("pool-input.json"),
        serde_json::to_vec_pretty(&new_pool)?,
    )?;
    let child_pool = hash_file(&out.join("pool-input.json"))?;
    store.import(&out.join("pool-input.json"), &child_pool)?;
    let mut child_process = parent_process.clone();
    replace_pool(&mut child_process, &child_pool)?;
    fs::write(
        out.join("process-spec.json"),
        serde_json::to_vec_pretty(&child_process)?,
    )?;
    let child_process_object = hash_file(&out.join("process-spec.json"))?;
    store.import(&out.join("process-spec.json"), &child_process_object)?;
    let receipt = Receipt {
        schema: SCHEMA.into(),
        parent_lock: hash_file(&out.join(PARENT_LOCK))?,
        parent_run_hash: parent.hash()?,
        parent_process: parent.inputs["process-specification"].clone(),
        parent_pool: parent.inputs["evidence:pool-input.json"].clone(),
        child_process: child_process_object.clone(),
        child_pool: child_pool.clone(),
        helper: parent.inputs["execution-and-report-build"].clone(),
        entropy_source: "/dev/urandom".into(),
        entropy_bytes: 32,
        time_before,
        time_after,
    };
    check_derivation(
        parent_process,
        &child_process,
        &old_pool,
        &new_pool,
        &receipt,
    )?;
    let mut base = parent.clone();
    base.process = Some(child_process);
    base.inputs
        .insert("process-specification".into(), child_process_object);
    base.inputs
        .insert("runtime:/private-pool.json".into(), child_pool.clone());
    base.inputs
        .insert("evidence:pool-input.json".into(), child_pool);
    // Write the completion marker only after the full copy and derivation pass.
    fs::write(out.join(RECEIPT), serde_json::to_vec_pretty(&receipt)?)?;
    bind(out, out, &store, &mut base)?;
    base.validate()?;
    drop(store);
    publish(out, output)?;
    Ok(receipt)
}

fn epoch() -> Result<u32> {
    Ok(u32::try_from(
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
    )?)
}

fn publish(source: &Path, destination: &Path) -> Result<()> {
    let from = CString::new(source.as_os_str().as_bytes())?;
    let to = CString::new(destination.as_os_str().as_bytes())?;
    // Linux is already required by the execution environment. Atomic no-replace
    // publication prevents another path from becoming an overwrite target.
    let rc = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    ensure!(
        rc == 0,
        "publish fresh preparation: {}",
        std::io::Error::last_os_error()
    );
    Ok(())
}

#[cfg(test)]
#[path = "fresh_prepared_tests.rs"]
mod tests;

//! Separate-output recovery of an acknowledged shared-pool proof graph.
//! The existing strict mining and Core verifiers remain the acceptance path.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{isolation::ProcessSpec, store::ObjectId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{DirBuilderExt, OpenOptionsExt},
    },
    path::{Component, Path, PathBuf},
};

use crate::{external_pool, prepared_vm_image, shared_retention as retention};

const META: u64 = 4 << 20;
const RESULT: u64 = md_sim_runtime::isolation::MAX_TRACE_BYTES * 2 + (1 << 20);
const LOG: u64 = md_sim_runtime::isolation::MAX_TRACE_BYTES;
const IMAGE: u64 = 1 << 30;
const TOTAL: u64 = 2 << 30;
const FILES: usize = 256;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Entry {
    size: u64,
    limit: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Plan {
    schema: String,
    source_execution: PathBuf,
    files: BTreeMap<String, Entry>,
    projected_copy_bytes: u64,
    maximum_copy_bytes: u64,
    maximum_files: usize,
    scope: String,
}

fn relative(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty()
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "unsafe evidence path"
    );
    Ok(())
}

fn original(root: &Path, path: &str) -> Result<PathBuf> {
    relative(path)?;
    let mut checked = root.to_path_buf();
    for part in Path::new(path).components() {
        checked.push(part);
        ensure!(
            !fs::symlink_metadata(&checked)?.file_type().is_symlink(),
            "evidence path contains a symlink"
        );
    }
    Ok(checked)
}

fn open(path: &Path, limit: u64) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let meta = file.metadata()?;
    ensure!(
        meta.is_file() && meta.len() <= limit,
        "evidence file type or extent exceeds bound"
    );
    Ok(file)
}

fn read<T: serde::de::DeserializeOwned>(path: &Path, limit: u64) -> Result<T> {
    let mut bytes = Vec::new();
    open(path, limit)?.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "evidence grew beyond read bound"
    );
    Ok(serde_json::from_slice(&bytes)?)
}

fn fingerprint(path: &Path, limit: u64) -> Result<ObjectId> {
    let mut file = open(path, limit)?.take(limit + 1);
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .context("evidence size overflow")?;
        ensure!(size <= limit, "evidence grew beyond hash bound");
        hash.update(&buffer[..count]);
    }
    Ok(ObjectId {
        sha256: hex::encode(hash.finalize()),
        size,
    })
}

fn save_new(path: &Path, value: &impl Serialize, limit: u64) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    ensure!(bytes.len() as u64 <= limit, "derived report exceeds bound");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}

fn plan(source: &Path) -> Result<Plan> {
    let source = source.canonicalize()?;
    ensure!(
        source.file_name().is_some_and(|name| name == "execution"),
        "shared source must be the retained execution directory"
    );
    let outer = source.parent().context("outer evidence root absent")?;
    let spec: ProcessSpec = read(&original(outer, "process-spec.json")?, META)?;
    spec.validate()?;
    let proof: Value = read(
        &original(outer, "execution/shared-pool/support-proof.json")?,
        META,
    )?;
    let browser =
        prepared_vm_image::browser_request(&spec.environment, &proof["carrier_dashboard"])?;
    let pool: retention::PoolBundle = read(
        &original(outer, "execution/shared-pool/pool-bundle.json")?,
        retention::POOL_BUNDLE_BYTES,
    )?;
    let receipt: retention::Receipt = read(
        &original(outer, "execution/shared-pool/retention-receipt.json")?,
        4096,
    )?;
    let mut paths = BTreeMap::new();
    for (path, limit) in [
        ("process-spec.json", META),
        ("result.json", RESULT),
        ("assessment.json", 64 << 20),
        ("provider.json", META),
        ("scenario.json", META),
        ("client-image/prepared-image.json", META),
        ("client-image/root-manifest.json", META),
        ("execution/process-spec.json", META),
        ("execution/adaptation.json", META),
        ("execution/locked-run.json", META),
        ("execution/worker-request.json", META),
        ("execution/worker-run.json", META),
        ("execution/worker-retention.json", 4096),
        ("execution/host-retention.json", 4096),
        ("execution/result.json", RESULT),
        ("execution/worker-result.json", RESULT),
        ("execution/backend.log", LOG),
        ("execution/guest.log", LOG),
        ("execution/pool-input.json", META),
        ("execution/shared-pool.json", META),
        (
            "execution/shared-pool/pool-bundle.json",
            retention::POOL_BUNDLE_BYTES,
        ),
        ("execution/shared-pool/retention-receipt.json", 4096),
        ("execution/shared-pool/support-proof.json", META),
        ("execution/shared-pool/provider-receipt.json", 65536),
        ("execution/shared-pool/outer-result.json", 65536),
        ("execution/shared-pool/pool.stdout", 8 << 20),
        ("execution/shared-pool/pool.stderr", 8 << 20),
    ] {
        paths.insert(path.to_owned(), limit);
    }
    if browser.is_some() {
        paths.insert("client-image/combined-initramfs.cpio.gz".into(), IMAGE);
        paths.insert("client-image/payload-report.json".into(), META);
        paths.insert("client-image/payload-manifest.json".into(), META);
    } else {
        paths.insert("client-image/initramfs.cpio.gz".into(), IMAGE);
    }
    ensure!(
        pool.sessions.len() <= 128,
        "pool session count exceeds recovery bound"
    );
    for name in pool.sessions.keys() {
        let index: usize = name
            .strip_prefix("session-")
            .and_then(|s| s.strip_suffix(".jsonl"))
            .context("session name absent")?
            .parse()?;
        ensure!(
            index < 128 && *name == format!("session-{index}.jsonl"),
            "noncanonical pool session name"
        );
        paths.insert(format!("execution/shared-pool/sessions/{name}"), 2 << 20);
    }
    ensure!(
        receipt.access_bundle.is_some(),
        "this recovery path requires acknowledged access evidence"
    );
    for (name, limit) in [
        ("access-bundle.json", retention::ACCESS_BUNDLE_BYTES),
        ("retention-receipt.json", 4096),
        ("client/report.json", META),
    ] {
        paths.insert(format!("execution/shared-access/{name}"), limit);
    }
    ensure!(paths.len() <= FILES, "recovery file count exceeds bound");
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    for (name, limit) in paths {
        let path = original(outer, &name)?;
        let metadata = fs::symlink_metadata(path)?;
        ensure!(
            metadata.is_file() && metadata.len() <= limit,
            "planned file type or extent differs: {name}"
        );
        total = total
            .checked_add(metadata.len())
            .context("copy total overflow")?;
        ensure!(
            total <= TOTAL,
            "projected recovery exceeds aggregate byte bound"
        );
        files.insert(
            name,
            Entry {
                size: metadata.len(),
                limit,
            },
        );
    }
    Ok(Plan {
        schema: "md-sim-shared-evidence-recovery-plan/1".into(), source_execution: source,
        files, projected_copy_bytes: total, maximum_copy_bytes: TOTAL, maximum_files: FILES,
        scope: "Metadata-only copy plan. It establishes no provenance, mining, access or target pass. No archive or input-store tree is copied.".into(),
    })
}

pub fn save_plan(source: &Path, output: &Path) -> Result<()> {
    save_new(output, &plan(source)?, META)
}

fn copy(source: &Path, target: &Path, expected: &ObjectId) -> Result<&'static str> {
    expected.validate()?;
    ensure!(
        expected.size <= IMAGE,
        "copy object exceeds hard per-file bound"
    );
    let mut input = open(source, expected.size)?;
    ensure!(
        input.metadata()?.len() == expected.size,
        "source extent changed before copy"
    );
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(target)?;
    // FICLONE makes a distinct inode with copy-on-write data. A hard link is
    // never used. If this filesystem cannot clone, use one bounded open input.
    let cloned = unsafe { libc::ioctl(output.as_raw_fd(), libc::FICLONE, input.as_raw_fd()) } == 0;
    let method = if cloned {
        "copy-on-write-clone"
    } else {
        output.set_len(0)?;
        output.seek(SeekFrom::Start(0))?;
        input.seek(SeekFrom::Start(0))?;
        let copied = std::io::copy(&mut input.take(expected.size + 1), &mut output)?;
        ensure!(
            copied == expected.size,
            "source changed during bounded copy"
        );
        "bounded-stream-copy"
    };
    output.sync_all()?;
    ensure!(
        fingerprint(source, expected.size)? == *expected
            && fingerprint(target, expected.size)? == *expected,
        "copied bytes differ from source identity"
    );
    Ok(method)
}

fn access_verdict(root: &Path, inputs: &BTreeMap<String, ObjectId>) -> Result<Value> {
    let receipt: retention::Receipt = read(
        &root.join("execution/shared-pool/retention-receipt.json"),
        4096,
    )?;
    let access_receipt: retention::Receipt = read(
        &root.join("execution/shared-access/retention-receipt.json"),
        4096,
    )?;
    ensure!(
        receipt == access_receipt,
        "access and pool acknowledgment differ"
    );
    ensure!(
        receipt.access_bundle.as_ref() == inputs.get("execution/shared-access/access-bundle.json"),
        "access bundle differs from acknowledgment"
    );
    let bundle: retention::AccessBundle = read(
        &root.join("execution/shared-access/access-bundle.json"),
        retention::ACCESS_BUNDLE_BYTES,
    )?;
    ensure!(
        bundle.schema == "md-sim-shared-carrier-artifacts/1" && bundle.native == receipt.native,
        "access bundle belongs to another native execution"
    );
    let report = bundle
        .files
        .get("client/report.json")
        .context("original access verdict absent")?;
    ensure!(
        Some(&report.object) == inputs.get("execution/shared-access/client/report.json"),
        "access verdict differs from acknowledged bundle"
    );
    ensure!(
        report.object.size <= META && report.hex.len() as u64 == report.object.size * 2,
        "access verdict envelope exceeds bound"
    );
    let bytes = hex::decode(&report.hex)?;
    ensure!(
        bytes.len() as u64 == report.object.size
            && hex::encode(Sha256::digest(&bytes)) == report.object.sha256,
        "access verdict bytes differ"
    );
    let verdict: Value = serde_json::from_slice(&bytes)?;
    let assessment: Value = read(&root.join("assessment.json"), 64 << 20)?;
    ensure!(
        assessment["execution"] == serde_json::to_value(&inputs["result.json"])?
            && assessment["process_specification"]
                == serde_json::to_value(&inputs["process-spec.json"])?
            && assessment["passed"] == false
            && verdict["passed"] == false,
        "failed access assessment is absent or belongs to another outer execution"
    );
    Ok(
        json!({"original_overall_passed":false,"original_access_passed":false,
        "assessment":inputs["assessment.json"],"access_bundle":receipt.access_bundle,
        "access_verdict":report.object,"observation_error":assessment["observation_error"],
        "scope":"Original failed verdicts remain unchanged. This is recovery of existing execution evidence, not another firmware execution or an access pass."}),
    )
}

pub fn retain(source: &Path, output: &Path) -> Result<()> {
    let plan = plan(source)?;
    ensure!(!output.try_exists()?, "new recovery root required");
    let outer = plan
        .source_execution
        .parent()
        .context("source parent absent")?;
    let parent = output
        .parent()
        .context("output parent absent")?
        .canonicalize()?;
    let target = parent.join(output.file_name().context("output name absent")?);
    ensure!(
        !target.starts_with(outer),
        "recovery output must be outside the original tree"
    );
    let joined = external_pool::verify(&plan.source_execution, false)?;
    let mut inputs = BTreeMap::new();
    for (name, entry) in &plan.files {
        let id = fingerprint(&original(outer, name)?, entry.limit)?;
        ensure!(
            id.size == entry.size,
            "source extent changed after planning"
        );
        inputs.insert(name.clone(), id);
    }
    let verdict = access_verdict(outer, &inputs)?;
    fs::DirBuilder::new().mode(0o700).create(&target)?;
    save_new(&target.join("recovery-plan.json"), &plan, META)?;
    let result = (|| -> Result<()> {
        let mut methods = BTreeMap::new();
        for (name, id) in &inputs {
            let path = target.join(name);
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(path.parent().context("copy parent absent")?)?;
            methods.insert(name.clone(), copy(&original(outer, name)?, &path, id)?);
        }
        let copied = external_pool::verify(&target.join("execution"), false)?;
        ensure!(
            copied == joined && external_pool::verify(&plan.source_execution, false)? == joined,
            "source and copied pool joins differ"
        );
        ensure!(
            access_verdict(&target, &inputs)? == verdict,
            "copied failed access verdict differs"
        );
        for (name, id) in &inputs {
            ensure!(
                fingerprint(&original(outer, name)?, id.size)? == *id
                    && fingerprint(&target.join(name), id.size)? == *id,
                "evidence changed before derived publication"
            );
        }
        save_new(
            &target.join("execution/pool-result.json"),
            &copied,
            retention::POOL_BUNDLE_BYTES,
        )?;
        let pool_result = fingerprint(
            &target.join("execution/pool-result.json"),
            retention::POOL_BUNDLE_BYTES,
        )?;
        save_new(
            &target.join("recovery.json"),
            &json!({
            "schema":"md-sim-shared-evidence-recovery/1","source_execution":plan.source_execution,
            "derived_execution":target.join("execution"),"inputs":inputs,"copy_methods":methods,
            "copied_bytes":plan.projected_copy_bytes,"pool_result":pool_result,"original_verdict":verdict,
            "driver":md_sim_runtime::store::hash_file(&std::env::current_exe()?)?,
            "source_files":{
                "s9_retain_shared_evidence.rs":md_sim_firmware::prepare::hash_bytes(include_bytes!("../s9_retain_shared_evidence.rs")),
                "support/s9_shared_recovery.rs":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_shared_recovery.rs"))},
            "external_verifier_source":copied["external_pool"]["verifier_source"],
            "image_verifier_source":copied["external_pool"]["image_verifier_source"],
            "observations_retained":true,"mining_acceptance":"not-run","core_acceptance":"not-run",
            "overall_access_gate":"failed; original verdict preserved",
            "scope":"Derived observations from unchanged acknowledged bytes. No new firmware execution. Existing strict regtest-shared and independent Core tests remain required. No original-board or complete-target pass."}),
            META,
        )?;
        File::open(&target)?.sync_all()?;
        Ok(())
    })();
    if let Err(error) = &result {
        save_new(
            &target.join("recovery-failure.json"),
            &json!({
            "error":format!("{error:#}"),"source_execution":plan.source_execution,
            "scope":"Recovery did not complete. Original evidence is unchanged; partial derived files grant no acceptance."}),
            META,
        )?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};

    #[test]
    fn copied_bytes_are_independent_and_changed_identity_is_rejected() -> Result<()> {
        let root = tempfile::tempdir()?;
        let source = root.path().join("source");
        let target = root.path().join("target");
        fs::write(&source, b"original evidence\n")?;
        let id = fingerprint(&source, 64)?;
        copy(&source, &target, &id)?;
        assert_eq!(fs::read(&target)?, b"original evidence\n");
        assert_ne!(fs::metadata(&source)?.ino(), fs::metadata(&target)?.ino());
        assert_eq!(fs::metadata(&target)?.permissions().mode() & 0o777, 0o600);
        fs::write(&target, b"changed derived copy")?;
        assert_eq!(fs::read(&source)?, b"original evidence\n");
        assert!(copy(&source, &target, &id).is_err());
        fs::write(&source, b"changed original\n")?;
        assert!(copy(&source, &root.path().join("bad"), &id).is_err());
        Ok(())
    }

    #[test]
    fn evidence_paths_types_and_extent_are_bounded() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::write(root.path().join("file"), b"five!")?;
        assert!(fingerprint(&root.path().join("file"), 4).is_err());
        assert!(fingerprint(root.path(), 1024).is_err());
        symlink("file", root.path().join("link"))?;
        assert!(original(root.path(), "link").is_err());
        assert!(fingerprint(&root.path().join("link"), 1024).is_err());
        for path in ["", "/file", "../file", "a/../file", "./file"] {
            assert!(relative(path).is_err(), "accepted {path}");
        }
        assert!(read::<Value>(&root.path().join("file"), 4).is_err());
        Ok(())
    }

    #[test]
    fn failed_access_is_bound_to_acknowledged_native_and_outer_results() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::create_dir_all(root.path().join("execution/shared-pool"))?;
        fs::create_dir_all(root.path().join("execution/shared-access/client"))?;
        let raw = br#"{"passed":false,"error":"fixture browser startup failure"}"#;
        let id = |bytes: &[u8]| ObjectId {
            sha256: hex::encode(Sha256::digest(bytes)),
            size: bytes.len() as u64,
        };
        let native = md_sim_coordinator::RetentionIdentity {
            locked_run_hash: "11".repeat(32),
            token: "22".repeat(32),
            run_id: format!("run-{}", "22".repeat(32)),
            result: id(b"fixture native result"),
        };
        let bundle = retention::AccessBundle {
            schema: "md-sim-shared-carrier-artifacts/1".into(),
            native: native.clone(),
            policy: id(b"fixture policy"),
            provider: id(b"fixture provider"),
            browser_requested: true,
            browser_driver: Some(id(b"fixture driver")),
            files: BTreeMap::from([(
                "client/report.json".into(),
                retention::Bytes {
                    object: id(raw),
                    hex: hex::encode(raw),
                },
            )]),
        };
        save_new(
            &root
                .path()
                .join("execution/shared-access/access-bundle.json"),
            &bundle,
            META,
        )?;
        fs::write(
            root.path()
                .join("execution/shared-access/client/report.json"),
            raw,
        )?;
        let access = fingerprint(
            &root
                .path()
                .join("execution/shared-access/access-bundle.json"),
            META,
        )?;
        let receipt = retention::Receipt {
            native,
            pool_bundle: id(b"fixture pool"),
            access_bundle: Some(access.clone()),
        };
        save_new(
            &root
                .path()
                .join("execution/shared-pool/retention-receipt.json"),
            &receipt,
            META,
        )?;
        save_new(
            &root
                .path()
                .join("execution/shared-access/retention-receipt.json"),
            &receipt,
            META,
        )?;
        let mut inputs = BTreeMap::from([
            ("execution/shared-access/access-bundle.json".into(), access),
            ("execution/shared-access/client/report.json".into(), id(raw)),
            ("result.json".into(), id(b"fixture outer result")),
            ("process-spec.json".into(), id(b"fixture outer process")),
        ]);
        let assessment = json!({"passed":false,"execution":inputs["result.json"],
            "process_specification":inputs["process-spec.json"],"observation_error":"fixture failure"});
        fs::write(
            root.path().join("assessment.json"),
            serde_json::to_vec(&assessment)?,
        )?;
        inputs.insert(
            "assessment.json".into(),
            fingerprint(&root.path().join("assessment.json"), META)?,
        );
        assert_eq!(
            access_verdict(root.path(), &inputs)?["original_access_passed"],
            false
        );
        let mut changed = inputs.clone();
        changed.insert("result.json".into(), id(b"another outer result"));
        assert!(access_verdict(root.path(), &changed).is_err());
        changed = inputs.clone();
        changed.insert(
            "execution/shared-access/access-bundle.json".into(),
            id(b"another access bundle"),
        );
        assert!(access_verdict(root.path(), &changed).is_err());
        let mut success = assessment;
        success["passed"] = json!(true);
        fs::write(
            root.path().join("assessment.json"),
            serde_json::to_vec(&success)?,
        )?;
        assert!(access_verdict(root.path(), &inputs).is_err());
        Ok(())
    }

    #[test]
    #[ignore = "set MD_SIM_SHARED_RECOVERY_SOURCE to exact retained453 execution; metadata-only"]
    fn retained_failed453_copy_plan_has_exact_bounded_volume() -> Result<()> {
        let source =
            std::env::var_os("MD_SIM_SHARED_RECOVERY_SOURCE").context("source not selected")?;
        let plan = plan(Path::new(&source))?;
        assert_eq!(plan.files.len(), 43);
        assert_eq!(plan.projected_copy_bytes, 1_038_030_428);
        assert!(plan.projected_copy_bytes < plan.maximum_copy_bytes);
        assert!(!plan.files.keys().any(|name| name.contains("input-store/")));
        assert!(!plan.source_execution.join("pool-result.json").exists());
        let pool = fingerprint(
            &plan.source_execution.join("shared-pool/pool-bundle.json"),
            retention::POOL_BUNDLE_BYTES,
        )?;
        assert_eq!(
            pool.sha256,
            "a86232130a35647f6abe6bc593ed22fd34940fd50e47232301ab24ecb66891f7"
        );
        assert_eq!(pool.size, 102679);
        Ok(())
    }
}

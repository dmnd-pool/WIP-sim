//! Only the reviewed502c/d/e failed HDL tool captures and successful502f restore
//! store. This validates cleanup evidence, not HDL or firmware acceptance.
use super::{component_bytes, real_directories};
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{ProcessResult, ProcessSpec},
    store::{ObjectId, Store, StoreLimits},
};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

const SOURCES: &[(&str, &str, &str)] = &[
    (
        "/toolchain.tar.gz",
        "compiler_archive",
        "cc98e84002c7b7fefb0128128e4cf4546d333e780715da16d048e17b73dafaa5",
    ),
    (
        "/compiler-manifest.json",
        "compiler_manifest",
        "e2eabd7bd16c22397c37cfbcfd0115223406f7850b5ad8f0733de6ac79e4d12a",
    ),
    (
        "/gperf.tar.gz",
        "gperf",
        "588546b945bba4b70b6a3a616e80b4ab466e3f33024a352fc2198112cdbb3ae2",
    ),
    (
        "/iverilog.tar.gz",
        "iverilog",
        "690f02e6fd0e0c05e4768bcb65a2c7f1b64d20289344d1ad3ccfb69bae8bd55b",
    ),
];
const DRIVER: &str = "crates/md-sim-firmware/examples/build_iverilog_tool.rs";

pub(super) fn selected<'a>(path: &'a Path, repository: &Path) -> Option<&'a Path> {
    let run = path.parent()?;
    (path.file_name()? == "inputs"
        && run.parent()? == repository.join("work/simulator/qemu-s9")
        && matches!(
            run.file_name()?.to_str()?,
            "iverilog-build-502c"
                | "iverilog-build-502d"
                | "iverilog-build-502e"
                | "iverilog-build-502f"
        ))
    .then_some(run)
}
pub(super) fn canonical(repository: &Path) -> PathBuf {
    repository.join("work/simulator/qemu-s9/iverilog-build-502f/inputs")
}
pub(super) fn directories(path: &Path, repository: &Path) -> Result<()> {
    ensure!(
        selected(path, repository).is_some(),
        "HDL store outside reviewed cohort"
    );
    real_directories(path, repository)?;
    real_directories(&path.join("objects"), repository)?;
    real_directories(&path.join("incoming"), repository)
}
fn source_pin(run: &Path) -> Result<(&'static str, &'static str)> {
    Ok(
        match run
            .file_name()
            .and_then(|n| n.to_str())
            .context("HDL run name")?
        {
            "iverilog-build-502c" => (
                "502c",
                "6f2559a92d863e82b8756c3cb89964998fdd79d89f254cd46cd6178546d092f7",
            ),
            "iverilog-build-502d" => (
                "502d",
                "03943c66e21a89c9b22371db793847ae665e75b5ef6701bf5625fbe53349f24d",
            ),
            "iverilog-build-502e" => (
                "502e",
                "7e87c0d6468d337b6e06772d32c9d2d6fb771693bee909741f135d18c8d61132",
            ),
            "iverilog-build-502f" => (
                "502f",
                "7b2a9ece9e5f910245a4cc89c49ab64e2e799d0e238abd1184d2d35f1ebcbf32",
            ),
            _ => anyhow::bail!("HDL run outside reviewed cohort"),
        },
    )
}
fn object(v: &Value) -> Result<ObjectId> {
    let id: ObjectId = serde_json::from_value(v.clone())?;
    id.validate()?;
    Ok(id)
}
fn json_file(path: &Path, bound: u64) -> Result<(Value, ObjectId)> {
    let (bytes, id) = component_bytes(path, bound)?;
    Ok((serde_json::from_slice(&bytes)?, id))
}
fn contract(spec: &ProcessSpec, ids: &Value, freeze: &Value) -> Result<BTreeMap<String, ObjectId>> {
    spec.validate()?;
    ensure!(
        spec.argv == ["/guard", "__inside"]
            && spec.namespace_uid == 0
            && spec.support_virtualization.is_none()
            && spec.private_channel.is_none()
            && spec.stdin_stream.is_none()
            && spec.stdin_object.is_none()
            && spec.dashboard.is_none()
            && !spec.loopback,
        "HDL execution role differs"
    );
    let mut roles = BTreeMap::new();
    for &(guest, field, sha) in SOURCES {
        let mount = spec
            .inputs
            .iter()
            .find(|m| m.guest_path == guest)
            .context("HDL source role absent")?;
        ensure!(
            mount.object.sha256 == sha && ids[field] == sha,
            "HDL source role identity differs"
        );
        roles.insert(guest.into(), mount.object.clone());
    }
    let guard = spec
        .inputs
        .iter()
        .find(|m| m.guest_path == "/guard")
        .context("HDL guard absent")?;
    ensure!(
        guard.object == object(&ids["recipe"])? && guard.object == object(&freeze["helper"])?,
        "HDL guard source binding differs"
    );
    roles.insert("/guard".into(), guard.object.clone());
    Ok(roles)
}
fn terminal(result: &ProcessResult, report: &Value, successful_restore: bool) -> Result<()> {
    ensure!(
        result.status == "exited" && result.exit_code.is_some() && !result.trace_truncated,
        "HDL execution is not a complete terminal result"
    );
    let passed = result.exit_code == Some(0);
    ensure!(
        report["success"].as_bool() == Some(passed) && passed == successful_restore,
        "HDL verdict differs from the reviewed failed/source or successful/restore role"
    );
    Ok(())
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct Evidence {
    pub run: PathBuf,
    files: BTreeMap<String, ObjectId>,
    roles: BTreeMap<String, ObjectId>,
    terminal_status: String,
    exit_code: i32,
    original_success: bool,
}
pub(super) fn finished(path: &Path, repository: &Path) -> Result<Option<Evidence>> {
    let Some(run) = selected(path, repository) else {
        return Ok(None);
    };
    directories(path, repository)?;
    let (_, policy_id) = policy(path)?;
    let (spec_value, spec_id) = json_file(&run.join("spec.json"), 1 << 20)?;
    let spec: ProcessSpec = serde_json::from_value(spec_value)?;
    let (ids, ids_id) = json_file(&run.join("input-identities.json"), 1 << 20)?;
    let (result_bytes, result_id) = component_bytes(&run.join("result.json"), 32 << 20)?;
    let result: ProcessResult = serde_json::from_slice(&result_bytes)?;
    let (report, report_id) = json_file(&run.join("build-report.json"), 1 << 20)?;
    terminal(&result, &report, path == canonical(repository))?;
    let (envelope, envelope_id) = component_bytes(&run.join("build-envelope.json"), 16 << 20)?;
    ensure!(
        hex::decode(&result.stdout)? == envelope,
        "HDL raw stdout differs from retained envelope"
    );
    let mut full: Value = serde_json::from_slice(&envelope)?;
    let archive_hex = full
        .as_object_mut()
        .context("HDL envelope object")?
        .remove("tools_hex");
    ensure!(full == report, "HDL compact report differs from raw result");
    let (stderr, stderr_id) = component_bytes(&run.join("stderr.log"), 1 << 20)?;
    ensure!(hex::decode(&result.stderr)? == stderr, "HDL stderr differs");
    let (checkpoint, expected_freeze) = source_pin(run)?;
    let source = repository
        .join("work/simulator/qemu-s9")
        .join(format!("iverilog-driver-{checkpoint}"));
    real_directories(&source, repository)?;
    let (freeze, freeze_id) = json_file(&source.join("manifest.json"), 1 << 20)?;
    ensure!(
        freeze_id.sha256 == expected_freeze
            && freeze["schema"] == "md-sim-hdl-tool-source-freeze/1",
        "HDL exact source freeze differs"
    );
    let roles = contract(&spec, &ids, &freeze)?;
    let (_, helper) = component_bytes(&source.join("build_iverilog_tool"), 8 << 20)?;
    ensure!(helper == roles["/guard"], "HDL retained helper differs");
    real_directories(
        &source.join("source/crates/md-sim-firmware/examples"),
        repository,
    )?;
    let (_, driver) = component_bytes(&source.join("source").join(DRIVER), 1 << 20)?;
    ensure!(
        driver == object(&freeze["sources"][DRIVER])?,
        "HDL frozen driver differs"
    );
    let mut files = BTreeMap::from([
        ("inputs/limits.json".into(), policy_id),
        ("spec.json".into(), spec_id),
        ("input-identities.json".into(), ids_id),
        ("result.json".into(), result_id),
        ("build-report.json".into(), report_id),
        ("build-envelope.json".into(), envelope_id),
        ("stderr.log".into(), stderr_id),
        (
            source.join("manifest.json").to_string_lossy().into_owned(),
            freeze_id,
        ),
        (
            source
                .join("build_iverilog_tool")
                .to_string_lossy()
                .into_owned(),
            helper,
        ),
        (
            source
                .join("source")
                .join(DRIVER)
                .to_string_lossy()
                .into_owned(),
            driver,
        ),
    ]);
    if path == canonical(repository) {
        let encoded = archive_hex.context("successful HDL restore archive absent")?;
        let bytes = hex::decode(encoded.as_str().context("HDL archive encoding")?)?;
        let (saved, id) = component_bytes(&run.join("iverilog-tools.tar.gz"), 24 << 20)?;
        ensure!(
            bytes == saved && id == object(&report["tools"])?,
            "HDL retained restore output differs"
        );
        files.insert("iverilog-tools.tar.gz".into(), id);
    } else {
        ensure!(
            archive_hex.is_none(),
            "failed HDL run unexpectedly claims an exported archive"
        );
    }
    Ok(Some(Evidence {
        run: run.to_path_buf(),
        files,
        roles,
        terminal_status: result.status,
        exit_code: result.exit_code.unwrap(),
        original_success: report["success"] == true,
    }))
}
fn policy(path: &Path) -> Result<(StoreLimits, ObjectId)> {
    let (bytes, identity) = component_bytes(&path.join("limits.json"), 4096)?;
    let limits: StoreLimits = serde_json::from_slice(&bytes)?;
    limits.validate()?;
    ensure!(
        limits.max_object_bytes == 512 << 20
            && limits.max_total_bytes == md_sim_runtime::store::DEFAULT_TOTAL_BYTES
            && limits.max_objects == md_sim_runtime::store::DEFAULT_MAX_OBJECTS,
        "HDL store policy differs from the reviewed captures"
    );
    Ok((limits, identity))
}
pub(super) fn open(path: &Path, repository: &Path) -> Result<Store> {
    directories(path, repository)?;
    ensure!(
        !path.join("limits.pending.json").exists(),
        "HDL store policy transition pending"
    );
    ensure!(
        fs::read_dir(path.join("incoming"))?.next().is_none(),
        "HDL incomplete transfer remains"
    );
    let (limits, _) = policy(path)?;
    Store::open_with_limits(path, limits)
}
pub(super) fn locked(path: &Path, repository: &Path, store: &Store) -> Result<Option<Evidence>> {
    let evidence = finished(path, repository)?;
    if let Some(value) = &evidence {
        for id in value.roles.values() {
            store.verified_path(id)?;
        }
    }
    Ok(evidence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use md_sim_runtime::isolation::{Limits, Mount};
    fn role_fixture() -> (ProcessSpec, Value, Value) {
        let mut ids = serde_json::json!({});
        let mut inputs = vec![];
        for &(guest, field, sha) in SOURCES {
            ids[field] = Value::String(sha.into());
            inputs.push(Mount {
                guest_path: guest.into(),
                object: ObjectId {
                    sha256: sha.into(),
                    size: 1,
                },
            });
        }
        let guard = ObjectId {
            sha256: "00".repeat(32),
            size: 1,
        };
        ids["recipe"] = serde_json::to_value(&guard).unwrap();
        inputs.push(Mount {
            guest_path: "/guard".into(),
            object: guard.clone(),
        });
        (
            ProcessSpec {
                schema_version: "2.0".into(),
                argv: vec!["/guard".into(), "__inside".into()],
                inputs,
                limits: Limits {
                    wall_seconds: 1200,
                    cpu_seconds: 600,
                    memory_bytes: 3 << 30,
                    processes: 128,
                    file_bytes: 128 << 20,
                    trace_bytes: 64 << 20,
                    tmp_bytes: 2 << 30,
                },
                environment: BTreeMap::new(),
                stdin_object: None,
                support_virtualization: None,
                private_channel: None,
                stdin_stream: None,
                namespace_uid: 0,
                dashboard: None,
                loopback: false,
            },
            ids,
            serde_json::json!({"helper":guard}),
        )
    }
    #[test]
    fn rejects_prelaunch_other_cohort_and_alias_shapes() {
        let repo = Path::new("/repo");
        for name in [
            "iverilog-build-502b",
            "iverilog-build-502g",
            "backend-build-502-release",
            "other/iverilog-build-502c",
        ] {
            assert!(selected(
                &repo
                    .join("work/simulator/qemu-s9")
                    .join(name)
                    .join("inputs"),
                repo
            )
            .is_none());
        }
        assert!(selected(
            &repo.join("work/simulator/qemu-s9/iverilog-build-502c/inputs"),
            repo
        )
        .is_some());
    }
    #[test]
    fn rejects_wrong_executable_and_source_roles() {
        let (mut spec, mut ids, freeze) = role_fixture();
        contract(&spec, &ids, &freeze).unwrap();
        spec.argv[0] = "/not-the-guard".into();
        assert!(contract(&spec, &ids, &freeze).is_err());
        spec.argv[0] = "/guard".into();
        ids["gperf"] = Value::String("00".repeat(32));
        assert!(contract(&spec, &ids, &freeze).is_err());
    }
    #[test]
    fn rejects_nonterminal_and_changed_verdict() {
        let mut result: ProcessResult=serde_json::from_value(serde_json::json!({"status":"exited","exit_code":1,"wall_milliseconds":1,"stdout":"","stderr":"","trace_truncated":false,"isolation":"fixture","limits_scope":"fixture","tools":{}})).unwrap();
        let report = serde_json::json!({"success":false});
        terminal(&result, &report, false).unwrap();
        result.status = "running".into();
        assert!(terminal(&result, &report, false).is_err());
        result.status = "exited".into();
        assert!(terminal(&result, &report, true).is_err());
        result.trace_truncated = true;
        assert!(terminal(&result, &report, false).is_err());
    }
    #[test]
    fn active_store_and_pending_transfers_are_rejected() {
        let repo = tempfile::tempdir().unwrap();
        let path = repo
            .path()
            .join("work/simulator/qemu-s9/iverilog-build-502c/inputs");
        let store = Store::open(&path, 512 << 20).unwrap();
        assert!(open(&path, repo.path()).is_err());
        drop(store);
        drop(open(&path, repo.path()).unwrap());
        fs::write(path.join("incoming/unfinished"), b"partial").unwrap();
        assert!(open(&path, repo.path()).is_err());
    }
    #[test]
    fn rejects_malformed_changed_and_pending_store_policy() {
        let repo = tempfile::tempdir().unwrap();
        let path = repo
            .path()
            .join("work/simulator/qemu-s9/iverilog-build-502c/inputs");
        drop(Store::open(&path, 512 << 20).unwrap());
        let original = fs::read(path.join("limits.json")).unwrap();
        fs::write(path.join("limits.json"), b"{}").unwrap();
        assert!(open(&path, repo.path()).is_err());
        let mut changed: StoreLimits = serde_json::from_slice(&original).unwrap();
        changed.max_objects += 1;
        fs::write(
            path.join("limits.json"),
            serde_json::to_vec(&changed).unwrap(),
        )
        .unwrap();
        assert!(open(&path, repo.path()).is_err());
        fs::write(path.join("limits.json"), &original).unwrap();
        fs::write(path.join("limits.pending.json"), &original).unwrap();
        assert!(open(&path, repo.path()).is_err());
        assert_eq!(fs::read(path.join("limits.json")).unwrap(), original);
    }
    #[test]
    fn retained_journal_restores_bytes_and_changed_restore_prevents_removal() {
        let d = tempfile::tempdir().unwrap();
        let target = d.path().join("duplicate");
        let restore = d.path().join("keeper");
        let journal = d.path().join("journal.json");
        fs::write(&target, b"retained original fixture").unwrap();
        fs::copy(&target, &restore).unwrap();
        let id = md_sim_runtime::store::hash_file(&target).unwrap();
        let record = serde_json::json!({"path":target,"restore_copy_from":restore,"sha256":id.sha256,"size":id.size});
        fs::write(&journal, serde_json::to_vec(&record).unwrap()).unwrap();
        fs::write(&restore, b"changed").unwrap();
        assert!(super::super::reclaim_object(&record).is_err());
        assert!(target.exists());
        fs::copy(&target, &restore).unwrap();
        super::super::reclaim_object(&record).unwrap();
        assert!(!target.exists());
        let recorded: Value = serde_json::from_slice(&fs::read(journal).unwrap()).unwrap();
        fs::copy(
            recorded["restore_copy_from"].as_str().unwrap(),
            recorded["path"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(md_sim_runtime::store::hash_file(&target).unwrap(), id);
    }
}

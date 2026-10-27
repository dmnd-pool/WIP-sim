//! Freeze a prepared board process without changing its source inputs.
use anyhow::{ensure, Result};
use md_sim_coordinator::{executor::local_tools, ExecutionScope, LockedRun, SuiteIdentity};
use md_sim_runtime::{
    isolation::ProcessSpec,
    store::{hash_file, ObjectId, Store},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
#[path = "s9_carrier_dashboard.rs"]
mod carrier_dashboard;
#[path = "fresh_prepared.rs"]
pub mod fresh;

pub fn bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "input metadata bound");
    Ok(bytes)
}

fn retain_metadata(source: &Path, out: &Path, store: &Store, name: &str) -> Result<ObjectId> {
    let bytes = bounded(&source.join(name), 4 * 1024 * 1024)?;
    let _: serde_json::Value = serde_json::from_slice(&bytes)?;
    let destination = out.join(name);
    fs::write(&destination, bytes)?;
    let object = hash_file(&destination)?;
    ensure!(
        hash_file(&source.join(name))? == object,
        "prepared metadata changed"
    );
    store.import(&destination, &object)?;
    Ok(object)
}

fn snapshot_object(source: &Path, object: &ObjectId) -> Result<PathBuf> {
    object.validate()?;
    let root = source.join("input-store");
    let objects = root.join("objects");
    ensure!(
        fs::symlink_metadata(&root)?.is_dir() && fs::symlink_metadata(&objects)?.is_dir(),
        "prepared object directories must be real directories"
    );
    let path = objects.join(&object.sha256);
    let metadata = fs::symlink_metadata(&path)?;
    ensure!(
        metadata.is_file() && metadata.nlink() == 1 && metadata.len() == object.size,
        "prepared object type, links or size differ"
    );
    // Store::import verifies the complete source hash, copies into the new
    // bounded store, and independently checks the completed destination.
    // Do not open the original store for writing or change its permissions.
    Ok(path)
}

fn shared_metadata(
    source: &Path,
    out: &Path,
    store: &Store,
    spec: &ProcessSpec,
) -> Result<Option<ObjectId>> {
    match spec
        .environment
        .get("MD_SIM_S9_SHARED_POOL")
        .map(String::as_str)
    {
        None => {
            ensure!(spec.private_channel.is_none(), "undeclared shared carrier");
            Ok(None)
        }
        Some("1") => {
            let carrier = spec
                .private_channel
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("shared carrier policy absent"))?;
            let object = retain_metadata(source, out, store, "shared-pool.json")?;
            let mounts: Vec<_> = spec
                .inputs
                .iter()
                .filter(|mount| mount.guest_path == "/shared-pool.json")
                .collect();
            ensure!(
                mounts.len() == 1 && mounts[0].object == object,
                "shared manifest differs from executed input"
            );
            let manifest: serde_json::Value =
                serde_json::from_slice(&bounded(&out.join("shared-pool.json"), 8192)?)?;
            ensure!(
                manifest["schema_version"] == "md-sim-s9-shared-pool/1"
                    && manifest["carrier"] == serde_json::to_value(carrier)?,
                "shared manifest carrier differs"
            );
            Ok(Some(object))
        }
        _ => anyhow::bail!("invalid shared pool preparation flag"),
    }
}

pub fn freeze(source: &Path, out: &Path, store: &Store) -> Result<LockedRun> {
    let mut inputs = BTreeMap::new();
    inputs.insert(
        "process-specification".into(),
        retain_metadata(source, out, store, "process-spec.json")?,
    );
    let spec: ProcessSpec =
        serde_json::from_slice(&bounded(&out.join("process-spec.json"), 4 * 1024 * 1024)?)?;
    spec.validate()?;
    ensure!(
        spec.argv.first().is_some_and(|s| s == "/boot-runner")
            && spec.argv.get(1).is_some_and(|s| s == "__boot-console"),
        "expected the prepared board console process"
    );
    for mount in &spec.inputs {
        store.import(&snapshot_object(source, &mount.object)?, &mount.object)?;
        inputs.insert(
            format!("runtime:{}", mount.guest_path),
            mount.object.clone(),
        );
    }
    if let Some(object) = &spec.stdin_object {
        store.import(&snapshot_object(source, object)?, object)?;
        inputs.insert("stdin".into(), object.clone());
    }
    for name in ["adaptation.json", "console.json", "pool-input.json"] {
        inputs.insert(
            format!("evidence:{name}"),
            retain_metadata(source, out, store, name)?,
        );
    }
    fresh::check_pool_mount(&spec, &inputs["evidence:pool-input.json"])?;
    let shared = shared_metadata(source, out, store, &spec)?;
    if let Some(object) = &shared {
        inputs.insert("evidence:shared-pool.json".into(), object.clone());
    }
    let dashboard = dashboard_metadata(source, out, store, &spec, shared.as_ref())?;
    if let Some(object) = &dashboard {
        inputs.insert(
            format!("evidence:{}", carrier_dashboard::FILE),
            object.clone(),
        );
    }
    let binary = std::env::current_exe()?;
    let reporter = hash_file(&binary)?;
    store.import(&binary, &reporter)?;
    inputs.insert("execution-and-report-build".into(), reporter.clone());
    let mut run = LockedRun {
        name: "archived-s9-reference-process".into(),
        scope: ExecutionScope::ProcessPrimitive { description: "Actual archived firmware on the declared adapted reference board. This contract tests execution transport only. Physical target, original board, mining and dashboard gates require their separate evidence.".into() },
        process: Some(spec),
        suite: SuiteIdentity {
            id: "archived-s9-reference-process-transport".into(), version: "1".into(),
            suite: reporter.clone(), verifier: reporter,
            required_gates: ["isolated-process-exit".into()].into(),
        },
        inputs, backend_tools: local_tools()?,
        policies: json!({"scope":"Replay the entire frozen process, including its firmware, devices, pool seeds, clock, adaptations, budgets and support image. No seed or target substitution.",
            "fresh_work":"Job seeds are frozen by preparation. Repeating this prepared input is the same execution identity, not a new random-seed test.",
            "result":"Native output is retained by the same report code as s9_boot_bundle; independent mining/Core checks remain mandatory."}),
        capability_limits: vec!["Adapted reference board; exact physical target and original board gate incomplete.".into(), "An IPC worker is a separate local process, not an external physical host or SSH test.".into()],
    };
    if let Some(object) = dashboard {
        run.policies["carrier_dashboard"] = serde_json::to_value(object)?;
    }
    fresh::bind(source, out, store, &mut run)?;
    run.validate()?;
    Ok(run)
}

fn dashboard_metadata(
    source: &Path,
    out: &Path,
    store: &Store,
    spec: &ProcessSpec,
    shared: Option<&ObjectId>,
) -> Result<Option<ObjectId>> {
    let mounts: Vec<_> = spec
        .inputs
        .iter()
        .filter(|mount| mount.guest_path == carrier_dashboard::GUEST)
        .collect();
    let path = source.join(carrier_dashboard::FILE);
    if mounts.is_empty() {
        ensure!(!path.exists(), "undeclared carrier dashboard metadata");
        return Ok(None);
    }
    ensure!(mounts.len() == 1, "duplicate carrier dashboard mount");
    let support =
        shared.ok_or_else(|| anyhow::anyhow!("carrier dashboard has no selected shared pool"))?;
    let (_, bytes) = carrier_dashboard::read(&path, support, spec.limits.wall_seconds)?;
    let object = ObjectId {
        sha256: md_sim_firmware::prepare::hash_bytes(&bytes),
        size: bytes.len() as u64,
    };
    ensure!(
        mounts[0].object == object,
        "dashboard policy differs from executed input"
    );
    let destination = out.join(carrier_dashboard::FILE);
    let mut retained = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&destination)?;
    retained.write_all(&bytes)?;
    retained.sync_all()?;
    store.import(&destination, &object)?;
    Ok(Some(object))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};

    #[test]
    fn dashboard_metadata_requires_the_executed_policy_and_shared_support() -> Result<()> {
        let source = tempfile::tempdir()?;
        let out = tempfile::tempdir()?;
        let objects = tempfile::tempdir()?;
        let store = Store::open(objects.path(), 65536)?;
        let support = ObjectId {
            sha256: "a".repeat(64),
            size: 930,
        };
        let mut spec: ProcessSpec = serde_json::from_value(json!({
            "schema_version":"2.0","argv":["/boot-runner","__boot-console"],"inputs":[],"environment":{},
            "limits":{"wall_seconds":360,"cpu_seconds":330,"memory_bytes":67108864,
                "processes":16,"file_bytes":1048576,"trace_bytes":65536,"tmp_bytes":1048576}
        }))?;
        assert!(
            dashboard_metadata(source.path(), out.path(), &store, &spec, Some(&support))?.is_none()
        );
        let policy = json!({
            "schema":"md-sim-carrier-dashboard-policy/1", "source":"adapted-native",
            "support_identity":support, "destination":"10.0.2.15:80", "listen_port":0,
            "connection_limit":8, "connection_attempt_limit":1024,
            "connection_bytes":67108864, "run_bytes":268435456,
            "connect_milliseconds":1000, "idle_seconds":60, "wall_seconds":300,
            "lease_milliseconds":10000
        });
        let path = source.path().join(carrier_dashboard::FILE);
        fs::write(&path, serde_json::to_vec(&policy)?)?;
        assert!(
            dashboard_metadata(source.path(), out.path(), &store, &spec, Some(&support)).is_err()
        );
        let object = hash_file(&path)?;
        spec.inputs.push(md_sim_runtime::isolation::Mount {
            guest_path: carrier_dashboard::GUEST.into(),
            object: object.clone(),
        });
        assert!(dashboard_metadata(source.path(), out.path(), &store, &spec, None).is_err());
        assert_eq!(
            dashboard_metadata(source.path(), out.path(), &store, &spec, Some(&support))?,
            Some(object)
        );
        spec.inputs[0].object.sha256 = "b".repeat(64);
        assert!(
            dashboard_metadata(source.path(), out.path(), &store, &spec, Some(&support)).is_err()
        );
        Ok(())
    }

    #[test]
    fn shared_metadata_must_match_both_executed_mount_and_carrier() {
        let source = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let objects = tempfile::tempdir().unwrap();
        let store = Store::open(objects.path(), 65536).unwrap();
        let carrier = json!({"protocol":"qemu-private-ethernet-v1","guest_fd":3,
            "max_frame_bytes":1500,"max_wire_bytes":65536,"max_packets":100});
        let raw = serde_json::to_vec(
            &json!({"schema_version":"md-sim-s9-shared-pool/1","carrier":carrier}),
        )
        .unwrap();
        fs::write(source.path().join("shared-pool.json"), &raw).unwrap();
        let object = hash_file(&source.path().join("shared-pool.json")).unwrap();
        let mut spec: ProcessSpec = serde_json::from_value(json!({
            "schema_version":"2.0","argv":["/boot-runner","__boot-console"],
            "inputs":[{"guest_path":"/shared-pool.json","object":object}],
            "environment":{"MD_SIM_S9_SHARED_POOL":"1"},"private_channel":carrier,
            "limits":{"wall_seconds":30,"cpu_seconds":20,"memory_bytes":67108864,
                "processes":16,"file_bytes":1048576,"trace_bytes":65536,"tmp_bytes":1048576}
        }))
        .unwrap();
        assert_eq!(
            shared_metadata(source.path(), out.path(), &store, &spec).unwrap(),
            Some(object)
        );
        assert_eq!(fs::read(out.path().join("shared-pool.json")).unwrap(), raw);
        spec.private_channel.as_mut().unwrap().max_packets += 1;
        assert!(shared_metadata(source.path(), out.path(), &store, &spec).is_err());
        spec.private_channel.as_mut().unwrap().max_packets -= 1;
        spec.inputs[0].object.sha256 = "ab".repeat(32);
        assert!(shared_metadata(source.path(), out.path(), &store, &spec).is_err());
        spec.environment.clear();
        assert!(shared_metadata(source.path(), out.path(), &store, &spec).is_err());
        spec.private_channel = None;
        assert!(shared_metadata(source.path(), out.path(), &store, &spec)
            .unwrap()
            .is_none());
    }

    #[test]
    fn prepared_inputs_remain_read_only_and_copies_are_verified() {
        let source = tempfile::tempdir().unwrap();
        let objects = source.path().join("input-store/objects");
        fs::create_dir_all(&objects).unwrap();
        let id = ObjectId {
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
            size: 3,
        };
        let input = objects.join(&id.sha256);
        fs::write(&input, b"abc").unwrap();
        fs::set_permissions(&input, fs::Permissions::from_mode(0o444)).unwrap();
        fs::set_permissions(objects.parent().unwrap(), fs::Permissions::from_mode(0o555)).unwrap();
        let output = tempfile::tempdir().unwrap();
        let store = Store::open(output.path(), 1024).unwrap();
        store
            .import(&snapshot_object(source.path(), &id).unwrap(), &id)
            .unwrap();
        assert_eq!(fs::read(store.verified_path(&id).unwrap()).unwrap(), b"abc");
        assert_eq!(
            fs::metadata(objects.parent().unwrap()).unwrap().mode() & 0o777,
            0o555
        );
        assert_eq!(fs::metadata(&input).unwrap().mode() & 0o777, 0o444);
        let alias = source.path().join("alias");
        fs::hard_link(&input, &alias).unwrap();
        assert!(snapshot_object(source.path(), &id).is_err());
        fs::remove_file(&alias).unwrap();
        fs::remove_file(&input).unwrap();
        fs::write(&alias, b"abc").unwrap();
        symlink(&alias, &input).unwrap();
        assert!(snapshot_object(source.path(), &id).is_err());
        fs::remove_file(&input).unwrap();
        fs::write(&input, b"abd").unwrap();
        assert!(store
            .import(&snapshot_object(source.path(), &id).unwrap(), &id)
            .is_err());
        fs::set_permissions(objects.parent().unwrap(), fs::Permissions::from_mode(0o755)).unwrap();
    }
}

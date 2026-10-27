use super::*;
use md_sim_runtime::{
    isolation::{Limits, Mount},
    store::Store,
};
use serde_json::{json, Value};
use std::{os::unix::fs::symlink, process::Command};

struct Fixture {
    root: tempfile::TempDir,
    inventory: PathBuf,
    target: PathBuf,
    restore: PathBuf,
    unique: PathBuf,
    object: ObjectId,
}
fn publish(store: &Store, bytes: &[u8]) -> ObjectId {
    let object = ObjectId {
        sha256: hex::encode(Sha256::digest(bytes)),
        size: bytes.len() as u64,
    };
    store.begin(&object).unwrap();
    store.chunk(&object, 0, bytes).unwrap();
    store.finish(&object).unwrap();
    object
}
fn save(path: &Path, value: &impl Serialize) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let target_store = Store::open(&root.path().join(TARGETS[1]), 1 << 20).unwrap();
        let restore_store = Store::open(&root.path().join(KEEPERS[0]), 1 << 20).unwrap();
        let object = publish(&target_store, b"duplicate fixture bytes; never executed");
        assert_eq!(
            publish(&restore_store, b"duplicate fixture bytes; never executed"),
            object
        );
        let unique_object = publish(&target_store, b"unique fixture must survive cleanup");
        let target = Path::new(TARGETS[1]).join("objects").join(&object.sha256);
        let restore = Path::new(KEEPERS[0]).join("objects").join(&object.sha256);
        let unique = Path::new(TARGETS[1])
            .join("objects")
            .join(unique_object.sha256);
        let run = root.path().join(TARGETS[1]).parent().unwrap().to_owned();
        let spec = ProcessSpec {
            support_virtualization: None,
            private_channel: None,
            stdin_stream: None,
            namespace_uid: 0,
            schema_version: "2.0".into(),
            argv: vec!["/fixture".into()],
            inputs: vec![Mount {
                object: object.clone(),
                guest_path: "/fixture".into(),
            }],
            limits: Limits::default(),
            environment: BTreeMap::new(),
            loopback: false,
            dashboard: None,
            stdin_object: None,
        };
        save(&run.join("process-spec.json"), &spec);
        save(
            &run.join("result.json"),
            &json!({"status":"exited","exit_code":0,"stdout_hex":"fixture only"}),
        );
        save(
            &run.join("assessment.json"),
            &json!({"passed":false,"error":"retained suite failure"}),
        );
        fs::write(
            run.join("guest.log"),
            b"labelled test fixture, no firmware execution claim",
        )
        .unwrap();
        let metadata = fs::symlink_metadata(root.path().join(&target)).unwrap();
        let inventory = root.path().join("inventory.json");
        save(
            &inventory,
            &json!({"schema":"md-sim-readonly-duplicate-store-inventory/1", "candidates":[{
                "path":target,"restore_copy_from":restore,"sha256_name":object.sha256,"size":object.size,
                "allocated_bytes":metadata.blocks()*512,"device":metadata.dev(),"inode":metadata.ino(),"links":metadata.nlink()
            }]}),
        );
        drop(target_store);
        drop(restore_store);
        Self {
            root,
            inventory,
            target,
            restore,
            unique,
            object,
        }
    }
    fn edit(&self, change: impl FnOnce(&mut Value)) {
        let mut input: Value = serde_json::from_slice(&fs::read(&self.inventory).unwrap()).unwrap();
        change(&mut input);
        save(&self.inventory, &input);
    }
    fn reject(&self) {
        assert!(run(
            self.root.path(),
            &self.inventory,
            &self.root.path().join("reject.json"),
            false
        )
        .is_err());
        assert!(self
            .root
            .path()
            .join(&self.target)
            .symlink_metadata()
            .is_ok());
        assert!(self.root.path().join(&self.unique).is_file());
    }
}

#[test]
fn plan_reclaim_restore_preserves_unique_objects_and_failed_verdict() {
    let fixture = Fixture::new();
    let root = fixture.root.path();
    let retained = hash_owned(&root.join(&fixture.unique), OBJECT_LIMIT)
        .unwrap()
        .0;
    let plan = run(root, &fixture.inventory, &root.join("plan.json"), true).unwrap();
    assert!(plan.plan_only && !plan.completed);
    assert_eq!(plan.removed_bytes, 0);
    assert_eq!(plan.objects.len(), 1);
    assert!(root.join(&fixture.target).is_file());
    let result = run(root, &fixture.inventory, &root.join("reclaim.json"), false).unwrap();
    assert!(result.completed);
    assert_eq!(result.removed_bytes, fixture.object.size);
    assert_eq!(result.original_run_assessment_passed, Some(false));
    assert!(!root.join(&fixture.target).exists());
    assert_eq!(
        hash_owned(&root.join(&fixture.unique), OBJECT_LIMIT)
            .unwrap()
            .0,
        retained
    );
    for (path, identity) in &plan.evidence {
        assert_eq!(
            hash_owned(&root.join(path), OBJECT_LIMIT).unwrap().0,
            *identity
        );
    }
    let complete: Value =
        serde_json::from_slice(&fs::read(root.join("reclaim.completed.json")).unwrap()).unwrap();
    assert_eq!(complete["completed"], true);
    assert_eq!(complete["original_run_assessment_passed"], false);
    fs::copy(root.join(&fixture.restore), root.join(&fixture.target)).unwrap();
    assert_eq!(
        hash_owned(&root.join(&fixture.target), OBJECT_LIMIT)
            .unwrap()
            .0,
        fixture.object
    );
    assert_eq!(fs::metadata(root.join(&fixture.target)).unwrap().nlink(), 1);
}

#[test]
fn superseded_preparation_keeps_unexecuted_status_without_a_run_result() {
    let mut fixture = Fixture::new();
    let root = fixture.root.path();
    let old_run = root.join(TARGETS[1]).parent().unwrap().to_owned();
    let prepared = root.join(TARGETS[0]).parent().unwrap().to_owned();
    fs::create_dir_all(&prepared).unwrap();
    fs::copy(
        old_run.join("process-spec.json"),
        prepared.join("process-spec.json"),
    )
    .unwrap();
    fs::rename(root.join(TARGETS[1]), root.join(TARGETS[0])).unwrap();
    fixture.target = Path::new(TARGETS[0])
        .join("objects")
        .join(&fixture.object.sha256);
    fixture.unique = Path::new(TARGETS[0])
        .join("objects")
        .join(fixture.unique.file_name().unwrap());
    fixture.edit(|input| input["candidates"][0]["path"] = json!(fixture.target));
    let report = run(root, &fixture.inventory, &root.join("prepared.json"), false).unwrap();
    assert!(report.completed);
    assert_eq!(report.original_run_assessment_passed, None);
    assert!(!prepared.join("result.json").exists());
    assert!(!prepared.join("assessment.json").exists());
    assert!(root.join(&fixture.unique).is_file());
    assert!(old_run.join("assessment.json").is_file());
}

#[test]
fn rejects_active_store_and_incomplete_transfer_before_mutation() {
    let fixture = Fixture::new();
    let active = Store::open(&fixture.root.path().join(TARGETS[1]), 1 << 20).unwrap();
    fixture.reject();
    drop(active);
    fs::write(
        fixture
            .root
            .path()
            .join(TARGETS[1])
            .join("incoming/incomplete"),
        b"partial",
    )
    .unwrap();
    fixture.reject();
}

#[test]
fn rejects_malformed_or_pending_store_policy() {
    let fixture = Fixture::new();
    let policy = fixture.root.path().join(TARGETS[1]).join("limits.json");
    fs::remove_file(&policy).unwrap();
    save(
        &policy,
        &json!({"max_object_bytes":0,"max_total_bytes":0,"max_objects":0}),
    );
    fixture.reject();
    let fixture = Fixture::new();
    let pending = fixture
        .root
        .path()
        .join(KEEPERS[0])
        .join("limits.pending.json");
    fs::write(pending, b"pending policy").unwrap();
    fixture.reject();
    let fixture = Fixture::new();
    let pending = fixture
        .root
        .path()
        .join(KEEPERS[0])
        .join("limits.pending.json");
    symlink("absent-policy", pending).unwrap();
    fixture.reject();
}

#[test]
fn rejects_changed_identity_unsafe_paths_and_nonterminal_run() {
    let fixture = Fixture::new();
    fixture.edit(|v| v["candidates"][0]["inode"] = json!(0));
    fixture.reject();
    let fixture = Fixture::new();
    fixture.edit(|v| v["candidates"][0]["path"] = json!("artifacts/files/source/objects/a"));
    fixture.reject();
    let fixture = Fixture::new();
    fixture.edit(|v| {
        v["candidates"][0]["restore_copy_from"] = json!("work/simulator/../archive/objects/a")
    });
    fixture.reject();
    let fixture = Fixture::new();
    let result = fixture
        .root
        .path()
        .join(TARGETS[1])
        .parent()
        .unwrap()
        .join("result.json");
    save(&result, &json!({"status":"running","exit_code":null}));
    fixture.reject();
}

#[test]
fn rejects_corrupt_or_aliased_restore_and_output_collisions() {
    let fixture = Fixture::new();
    let restore = fixture.root.path().join(&fixture.restore);
    fs::remove_file(&restore).unwrap();
    fs::write(restore, vec![0; fixture.object.size as usize]).unwrap();
    fixture.reject();
    let fixture = Fixture::new();
    let restore = fixture.root.path().join(&fixture.restore);
    fs::remove_file(&restore).unwrap();
    symlink(fixture.root.path().join(&fixture.target), &restore).unwrap();
    fixture.reject();
    let fixture = Fixture::new();
    let restore = fixture.root.path().join(&fixture.restore);
    fs::remove_file(&restore).unwrap();
    fs::hard_link(fixture.root.path().join(&fixture.target), &restore).unwrap();
    fixture.reject();
    let fixture = Fixture::new();
    fs::write(
        fixture.root.path().join("reject.completed.json"),
        b"existing result",
    )
    .unwrap();
    fixture.reject();
    assert_eq!(
        fs::read(fixture.root.path().join("reject.completed.json")).unwrap(),
        b"existing result"
    );
    assert!(!fixture.root.path().join("reject.json").exists());
}

#[test]
fn large_ignored_fields_are_hashed_and_invalid_trailers_are_rejected() {
    let fixture = Fixture::new();
    let root = fixture.root.path();
    let result = root.join(TARGETS[1]).parent().unwrap().join("result.json");
    save(
        &result,
        &json!({"status":"exited","exit_code":0,"stdout_hex":"a".repeat(2 << 20)}),
    );
    let plan = run(root, &fixture.inventory, &root.join("plan.json"), true).unwrap();
    assert_eq!(
        plan.evidence[Path::new("work/simulator/network-vm/native-browser-469/result.json")],
        md_sim_runtime::store::hash_file(&result).unwrap()
    );
    OpenOptions::new()
        .append(true)
        .open(&result)
        .unwrap()
        .write_all(b" trailing junk")
        .unwrap();
    fixture.reject();
    let mut input = HashedRead {
        file: File::open(result).unwrap(),
        digest: Sha256::new(),
        bytes: 0,
        limit: 32,
    };
    assert_eq!(input.read(&mut []).unwrap(), 0);
    assert!(std::io::copy(&mut input, &mut std::io::sink()).is_err());
}

#[test]
#[ignore = "explicit frozen release binary: MD_SIM_BROWSER_RECLAIM_TOOL"]
fn real_cli_plan_reclaim_and_restore() {
    let tool =
        std::env::var_os("MD_SIM_BROWSER_RECLAIM_TOOL").expect("frozen release binary is required");
    let fixture = Fixture::new();
    let root = fixture.root.path();
    for (flag, journal) in [
        ("--browser-plan", "plan.json"),
        ("--browser-reclaim", "reclaim.json"),
    ] {
        let output = Command::new(&tool)
            .current_dir(root)
            .args([flag, "inventory.json", journal])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        if flag == "--browser-plan" {
            assert!(root.join(&fixture.target).is_file());
        }
    }
    assert!(!root.join(&fixture.target).exists());
    assert!(root.join(&fixture.unique).is_file());
    fs::copy(root.join(&fixture.restore), root.join(&fixture.target)).unwrap();
    assert_eq!(
        md_sim_runtime::store::hash_file(&root.join(&fixture.target)).unwrap(),
        fixture.object
    );
    let output = Command::new(tool)
        .current_dir(root)
        .args(["--browser-reclaim", "inventory.json", "reclaim.json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(root.join(&fixture.target).is_file());
}

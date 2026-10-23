use md_sim_catalog::*;
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};
use tempfile::TempDir;

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn records(root: &Path, name: &str, values: &[Value]) {
    let mut bytes = Vec::new();
    for value in values {
        bytes.extend(serde_json::to_vec(value).unwrap());
        bytes.push(b'\n');
    }
    write(&root.join(format!("catalog/{name}.jsonl")), &bytes);
}

fn repository() -> TempDir {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    for name in ["entities", "artifacts", "relationships", "sources", "gaps"] {
        records(root, name, &[]);
    }
    write(
        &root.join("catalog/storage.json"),
        br#"{"artifact_roots":[]}"#,
    );
    for path in [
        "reports/device-matrix.csv",
        "reports/coverage.md",
        "docs/simulator-specification.md",
    ] {
        write(&root.join(path), b"fixture\n");
    }
    records(
        root,
        "entities",
        &[
            json!({"id":"miner-a","kind":"device","scope":"bitcoin-asic","source_ids":["evidence"]}),
            json!({"id":"miner-b","kind":"device","scope":"bitcoin-asic"}),
            json!({"id":"firmware","kind":"firmware","artifact_ids":["payload","missing","pointer"],"source_ids":["evidence"]}),
            json!({"id":"tool","kind":"tool","scope":"supporting-tool"}),
        ],
    );
    records(
        root,
        "artifacts",
        &[
            json!({"id":"payload","kind":"firmware","status":"downloaded","sha256":hash_bytes(b"firmware bytes"),"size_bytes":14,"local_path":"artifacts/files/firmware/test/image.bin","model_labels":["miner-a"]}),
            json!({"id":"missing","kind":"firmware","status":"failed","error":"HTTP 404"}),
            json!({"id":"pointer","kind":"firmware","status":"downloaded","content_role":"git-lfs-pointer","resolved_artifact_id":"payload"}),
        ],
    );
    records(
        root,
        "relationships",
        &[
            json!({"id":"supports-family","from_id":"firmware","to_id":"miner-a","relation":"supports","status":"vendor-claimed","constraints":["controller rev B only"],"source_ids":["evidence"]}),
        ],
    );
    records(
        root,
        "sources",
        &[json!({"id":"evidence","url":"https://example.invalid/evidence"})],
    );
    write(
        &root.join("artifacts/files/firmware/test/image.bin"),
        b"firmware bytes",
    );
    write(&root.join("catalog/parts/family/evidence.json"), b"{}");
    write(&root.join("catalog/parts/family.jsonl"), b"{}\n");
    directory
}

fn catalog(directory: &TempDir) -> Catalog {
    let snapshot = import_catalog(directory.path(), &directory.path().join("snapshots")).unwrap();
    load_catalog(&snapshot.directory).unwrap()
}

#[test]
fn snapshot_preserves_bytes_and_rejects_tampering() {
    let directory = repository();
    let first = import_catalog(directory.path(), &directory.path().join("snapshots")).unwrap();
    let second = import_catalog(directory.path(), &directory.path().join("snapshots")).unwrap();
    assert_eq!(first.manifest.snapshot_hash, second.manifest.snapshot_hash);
    assert!(!first.manifest.artifact_payloads_verified);
    let original = fs::read(directory.path().join("catalog/entities.jsonl")).unwrap();
    assert_eq!(
        original,
        fs::read(first.directory.join("catalog/entities.jsonl")).unwrap()
    );
    write(
        &directory.path().join("catalog/entities.jsonl"),
        b"changed repository\n",
    );
    assert!(load_catalog(&first.directory).is_ok());
    let frozen = first.directory.join("catalog/entities.jsonl");
    fs::remove_file(&frozen).unwrap();
    write(&frozen, b"tampered\n");
    assert!(load_catalog(&first.directory).is_err());
}

#[test]
fn duplicate_ids_cannot_be_published() {
    let directory = repository();
    records(
        directory.path(),
        "entities",
        &[json!({"id":"same"}), json!({"id":"same"})],
    );
    assert!(import_catalog(directory.path(), &directory.path().join("snapshots")).is_err());
    assert_eq!(
        fs::read_dir(directory.path().join("snapshots"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn all_inventory_and_all_artifact_aliases_remain_accounted() {
    let directory = repository();
    let catalog = catalog(&directory);
    let plan = catalog.plan(Pairing::Relevant).unwrap();
    plan.validate().unwrap();
    assert_eq!(plan.inventory_entities, 4);
    assert_eq!(plan.inventory_artifacts, 3);
    assert_eq!(plan.rows.len(), 10);
    assert_eq!(
        plan.rows.iter().filter(|r| r.record_kind == "pair").count(),
        3
    );
    assert!(plan.rows.iter().all(|r| r.state != PlanState::Runnable));
    let payload = plan
        .rows
        .iter()
        .find(|r| r.id == "pair:supports-family:payload")
        .unwrap();
    assert_eq!(payload.state, PlanState::Unknown);
    assert!(payload
        .constraints
        .iter()
        .any(|c| c["constraints"] == json!(["controller rev B only"])));
    assert!(payload
        .constraints
        .iter()
        .any(|c| c["field"] == "model_labels"));
    for id in [
        "artifact:missing",
        "artifact:pointer",
        "pair:supports-family:missing",
    ] {
        assert_eq!(
            plan.rows.iter().find(|r| r.id == id).unwrap().state,
            PlanState::Blocked
        );
    }
    assert!(plan.require_complete().is_err());
    assert_eq!(plan.summary().executed_tests, 0);
    assert_eq!(plan.summary().passing_firmware_targets, 0);
}

#[test]
fn cartesian_rule_accounts_for_pairs_without_candidate_evidence() {
    let directory = repository();
    let plan = catalog(&directory).plan(Pairing::Cartesian).unwrap();
    let domain = plan.cartesian.as_ref().unwrap();
    assert_eq!(domain.total_pairs, 6);
    assert_eq!(domain.explicit_pairs, 3);
    assert_eq!(domain.default_state, PlanState::Unknown);
    assert_eq!(plan.summary().implicit_cartesian_pairs, 3);
    assert!(plan.require_complete().is_err());
}

#[test]
fn payload_checks_read_the_bytes_and_never_accept_lfs_pointers() {
    let directory = repository();
    let catalog = catalog(&directory);
    let verified = catalog
        .verify_artifact(directory.path(), "payload")
        .unwrap();
    assert_eq!(verified.sha256, hash_bytes(b"firmware bytes"));
    assert!(catalog
        .verify_artifact(directory.path(), "pointer")
        .is_err());
    assert!(catalog
        .verify_artifact(directory.path(), "missing")
        .is_err());
    write(
        &directory
            .path()
            .join("artifacts/files/firmware/test/image.bin"),
        b"modified bytes",
    );
    assert!(catalog
        .verify_artifact(directory.path(), "payload")
        .is_err());
}

#[cfg(unix)]
#[test]
fn archive_link_cannot_escape_frozen_storage_roots() {
    let directory = repository();
    let outside = tempfile::tempdir().unwrap();
    write(&outside.path().join("image.bin"), b"firmware bytes");
    let catalog = catalog(&directory);
    let path = directory
        .path()
        .join("artifacts/files/firmware/test/image.bin");
    fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(outside.path().join("image.bin"), &path).unwrap();
    assert!(catalog
        .verify_artifact(directory.path(), "payload")
        .is_err());
}

fn complete_hardware() -> HardwareProfileV2 {
    serde_json::from_value(json!({
        "schema_version":"2.0","id":"fixture-profile","revision":"test-v1","device_aliases":["miner-a"],"evidence_ids":["evidence"],
        "controller":{"board_id":"fixture-board","pcb_revision":"1","soc":"fixture-soc","cpu":{"type":"fixture-cpu","count":1},"isa":"fixture-isa","endian":"little","reset_entry":0,"address_map":[{"base":0,"size":4096}]},
        "memory":{"ram_layout":[{"base":0,"size":4096}],"rom_layout":[],"reserved_ranges":[]},
        "storage":{"bus":"spi","chip_id":"fixture-chip","size_bytes":4096,"geometry":{"erase_bytes":256},"ecc_oob":{"not_applicable":true},"partitions":[{"offset":0,"size":4096}],"boot_order":["spi"],"write_protection":false},
        "hashboards":{"slots":[{"board_id":"fixture-hashboard","revision":"1","asic_type":"fixture-asic","asic_revision":"1","chip_count":1,"chip_addresses":[0],"chain_links":[],"reset_domain":"board"}]},
        "auxiliary_controllers":[],"power":{"psu_id":"fixture-psu","revision":"1","control_interface":"gpio","rails":[{"name":"3v3","voltage_mv":3300}],"startup_sequence":["enable"]},
        "cooling":{"mode":"air","sensor_layout":[{"id":"asic-temperature","type":"temperature","location":"asic"}],"actuators":[{"id":"fan","type":"fan"}]},
        "interfaces":{"bindings":[{"id":"asic-spi","kind":"spi","endpoint":"spi0"}],"timing":{"clock_hz":1000000},"implementation_hashes":{"spi":hash_bytes(b"spi")}},"unknowns":[]
    })).unwrap()
}

fn target(catalog: &Catalog) -> TargetDefinitionV2 {
    let hash = hash_bytes(b"firmware bytes");
    TargetDefinitionV2 {
        schema_version: "2.0".into(),
        id: "fixture-target".into(),
        snapshot_hash: catalog.snapshot.manifest.catalogue_data_hash.clone(),
        device_id: "miner-a".into(),
        firmware_entity_id: "firmware".into(),
        hardware: complete_hardware(),
        firmware: FirmwareBundleV2 {
            schema_version: "2.0".into(),
            id: "bundle".into(),
            artifact_ids: vec!["payload".into()],
            components: vec![BundleComponent {
                id: "image".into(),
                parent_artifact_id: "payload".into(),
                parent_sha256: hash.clone(),
                sha256: hash,
                archive_member: None,
                byte_range: Some([0, 14]),
                role: "flash-image".into(),
                destination: json!({"offset":0}),
                preparation_tool_sha256: hash_bytes(b"copy"),
            }],
            installation_path: vec![json!({"type":"factory-flash"})],
            starting_state_sha256: hash_bytes(b"empty flash"),
            missing_inputs: vec![],
        },
        execution_mode: ExecutionMode::BoardEmulation,
        firmware_identity: FirmwareIdentity::OriginalBytes,
        environment_identity: vec![],
        boot_entry: BootEntry::Reset,
        adaptation_manifest: json!([]),
        backend_build_sha256: hash_bytes(b"backend"),
        required_models: BTreeMap::from([("spi".into(), hash_bytes(b"spi"))]),
        constraint_reviews: vec![ConstraintReview {
            artifact_id: "payload".into(),
            constraints_sha256: catalog
                .target_constraints_hash("miner-a", "firmware", "payload")
                .unwrap(),
            state: PlanState::Runnable,
            evidence_ids: vec!["evidence".into()],
            explanation: "Fixture explicitly names this test board.".into(),
        }],
    }
}

#[test]
fn zero_reset_address_is_valid_but_zero_chip_count_is_unknown() {
    let mut hardware = complete_hardware();
    assert!(hardware.unresolved_fields().unwrap().is_empty());
    hardware.hashboards["slots"][0]["chip_count"] = json!(0);
    assert!(hardware
        .unresolved_fields()
        .unwrap()
        .iter()
        .any(|field| field.contains("chip_count")));
}

#[test]
fn missing_implementation_is_blocked_not_incompatible() {
    let directory = repository();
    let catalog = catalog(&directory);
    let target = target(&catalog);
    let resolution = catalog
        .resolve_target(&target, &BackendRegistry::default(), &BTreeMap::new())
        .unwrap();
    assert_eq!(resolution.state, PlanState::Blocked);
    assert!(resolution
        .reasons
        .iter()
        .any(|r| r.code == "backend-not-implemented"));
    assert!(resolution
        .reasons
        .iter()
        .any(|r| r.code == "component-bytes-not-verified"));
}

#[test]
fn stale_constraint_review_and_wrong_mode_are_rejected() {
    let directory = repository();
    let catalog = catalog(&directory);
    let mut target = target(&catalog);
    target.constraint_reviews[0].constraints_sha256 = hash_bytes(b"other constraints");
    assert_eq!(
        catalog
            .resolve_target(&target, &BackendRegistry::default(), &BTreeMap::new())
            .unwrap()
            .state,
        PlanState::Unknown
    );
    target.execution_mode = ExecutionMode::ApiModel;
    assert!(catalog
        .resolve_target(&target, &BackendRegistry::default(), &BTreeMap::new())
        .is_err());
}

#[test]
fn resume_changes_with_every_required_execution_setting() {
    let mut specification = json!({
        "target_hash":hash_bytes(b"target"),"firmware_bundle_hash":hash_bytes(b"bundle"),"backend_build_hash":hash_bytes(b"backend"),"adaptation_hash":hash_bytes(b"adaptation"),"scenario_hash":hash_bytes(b"scenario"),
        "seed":0,"hashing":{"cap_hps_total":10000,"mode":"functional"},"low_rate":{"enabled":true},"clock":{"policy":"realtime","epoch":0},"network":{"egress":"deny"},"resources":{"memory_bytes":4096},"timeouts":{"boot_seconds":30},"persistent_state_fixture_hash":hash_bytes(b"fixture"),"dashboard":{"listen":"127.0.0.1:18080"}
    });
    let identity = ResumeIdentity {
        resolved_run_specification: specification.clone(),
        suite_sha256: hash_bytes(b"suite"),
        verifier_sha256: hash_bytes(b"verifier"),
        referenced_inputs: BTreeMap::from([("payload".into(), hash_bytes(b"payload"))]),
    };
    let original = identity.hash().unwrap();
    for field in specification.as_object().unwrap().keys() {
        let mut changed = identity.clone();
        changed.resolved_run_specification[field] = json!({"changed":true});
        assert_ne!(original, changed.hash().unwrap(), "{field}");
    }
    specification.as_object_mut().unwrap().remove("seed");
    assert!(ResumeIdentity {
        resolved_run_specification: specification,
        ..identity
    }
    .hash()
    .is_err());
}

#[test]
fn migration_never_promotes_a_legacy_validation_claim() {
    let migrated = migrate_v1_profile(
        &json!({"schema_version":"1.0","validation":{"level":"physical-calibrated"}}),
    )
    .unwrap();
    assert_eq!(migrated["capability_results"], json!({}));
    assert_eq!(
        migrated["legacy_profile"]["validation"]["level"],
        "physical-calibrated"
    );
}

#[test]
fn registry_freeze_does_not_make_target_catalogue_identity_circular() {
    let directory = repository();
    let first = import_catalog(directory.path(), &directory.path().join("snapshots")).unwrap();
    write(
        &directory.path().join("profiles/targets/example.json"),
        &serde_json::to_vec(&json!({"snapshot_hash":first.manifest.catalogue_data_hash})).unwrap(),
    );
    let second = import_catalog(directory.path(), &directory.path().join("snapshots")).unwrap();
    assert_eq!(
        first.manifest.catalogue_data_hash,
        second.manifest.catalogue_data_hash
    );
    assert_ne!(first.manifest.snapshot_hash, second.manifest.snapshot_hash);
}

#[test]
fn hardware_fields_have_types_and_addresses_have_unique_identities() {
    let mut hardware = complete_hardware();
    hardware.controller["soc"] = json!(true);
    hardware.controller["address_map"] = json!([]);
    hardware.hashboards["slots"][0]["chip_count"] = json!(2);
    hardware.hashboards["slots"][0]["chip_addresses"] = json!([0, 0]);
    let fields = hardware.unresolved_fields().unwrap();
    assert!(fields.iter().any(|f| f.contains("/controller/soc")));
    assert!(fields.iter().any(|f| f.contains("empty layout")));
    assert!(fields.iter().any(|f| f.contains("uniqueness mismatch")));
}

#[test]
fn nonexistent_evidence_and_duplicate_components_cannot_resolve() {
    let directory = repository();
    let catalog = catalog(&directory);
    let mut target = target(&catalog);
    target.hardware.evidence_ids = vec!["invented".into()];
    assert_eq!(
        catalog
            .resolve_target(&target, &BackendRegistry::default(), &BTreeMap::new())
            .unwrap()
            .state,
        PlanState::Unknown
    );
    target
        .firmware
        .components
        .push(target.firmware.components[0].clone());
    assert!(catalog
        .resolve_target(&target, &BackendRegistry::default(), &BTreeMap::new())
        .is_err());
}

#[test]
fn source_adapter_requires_real_entry_and_explicit_adaptation() {
    let directory = repository();
    let catalog = catalog(&directory);
    let mut target = target(&catalog);
    target.execution_mode = ExecutionMode::SourceAdapter;
    target.firmware_identity = FirmwareIdentity::RebuiltSource;
    target.boot_entry = BootEntry::None;
    assert!(catalog
        .resolve_target(&target, &BackendRegistry::default(), &BTreeMap::new())
        .is_err());
    target.boot_entry = BootEntry::Process;
    assert!(catalog
        .resolve_target(&target, &BackendRegistry::default(), &BTreeMap::new())
        .is_err());
    target.adaptation_manifest = json!({"changes":[],"reason":"Rebuild the unchanged fixture source for this host process."});
    assert_eq!(
        catalog
            .resolve_target(&target, &BackendRegistry::default(), &BTreeMap::new())
            .unwrap()
            .state,
        PlanState::Blocked
    );
}

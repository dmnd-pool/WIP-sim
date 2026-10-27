use super::*;
use md_sim_runtime::isolation::Mount;
use serde_json::json;

// Preparation tests use inert input bytes. They do not execute firmware or
// stand in for the separate fresh-work mining and native dashboard suites.
fn fixture(root: &Path) -> Result<()> {
    fs::create_dir_all(root)?;
    let store = Store::open(&root.join("input-store"), 64 * 1024 * 1024)?;
    fs::write(root.join("runner"), b"inert preparation fixture")?;
    let runner = hash_file(&root.join("runner"))?;
    store.import(&root.join("runner"), &runner)?;
    let seed = [11u8; 32];
    let pool = json!({"schema":"md-sim-s9-pool-input/8", "seed":seed,
        "time":1700000000,"seconds":40,"difficulty_exponents":[-29,-28],
        "regtest_anchor":null,"connection_limit":19});
    fs::write(
        root.join("pool-input.json"),
        serde_json::to_vec_pretty(&pool)?,
    )?;
    let pool = hash_file(&root.join("pool-input.json"))?;
    store.import(&root.join("pool-input.json"), &pool)?;
    fs::write(
        root.join("process-spec.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version":"2.0", "argv":["/boot-runner","__boot-console","untouched"],
            "environment":{"MD_SIM_FIXTURE":"original"},
            "inputs":[{"guest_path":"/boot-runner","object":runner},
                {"guest_path":"/private-pool.json","object":pool}],
            "limits":{"wall_seconds":60,"cpu_seconds":40,"memory_bytes":67108864,
                "processes":16,"file_bytes":1048576,"trace_bytes":65536,"tmp_bytes":1048576}
        }))?,
    )?;
    fs::write(
        root.join("adaptation.json"),
        br#"{"fixture":"unchanged adaptation"}"#,
    )?;
    fs::write(
        root.join("console.json"),
        br#"{"fixture":"unchanged console"}"#,
    )?;
    Ok(())
}

fn lock(source: &Path) -> Result<LockedRun> {
    let output = tempfile::tempdir()?;
    let store = Store::open(&output.path().join("input-store"), 64 * 1024 * 1024)?;
    freeze(source, output.path(), &store)
}

#[test]
fn fresh_preparation_preserves_policy_and_source_and_binds_two_distinct_runs() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fixture(&source)?;
    let names = [
        "process-spec.json",
        "pool-input.json",
        "adaptation.json",
        "console.json",
    ];
    let before: Vec<_> = names
        .iter()
        .map(|n| hash_file(&source.join(n)).unwrap())
        .collect();
    let a = temp.path().join("fresh-a");
    let b = temp.path().join("fresh-b");
    let ra = prepare(&source, &a)?;
    let rb = prepare(&source, &b)?;
    assert_ne!(ra.child_pool, rb.child_pool);
    assert_ne!(ra.child_process, rb.child_process);
    assert_eq!(ra.parent_pool, rb.parent_pool);
    assert_eq!(ra.parent_process, rb.parent_process);
    let la = lock(&a)?;
    let lb = lock(&b)?;
    assert_ne!(la.hash()?, lb.hash()?);
    assert_eq!(
        la.inputs["evidence:fresh-work.json"],
        hash_file(&a.join(RECEIPT))?
    );
    assert_eq!(la.inputs["fresh-preparation-build"], ra.helper);
    assert_eq!(
        la.policies["fresh_preparation"],
        serde_json::to_value(hash_file(&a.join(RECEIPT))?)?
    );
    let pool = supported_pool(&a.join("pool-input.json"))?;
    assert_eq!(pool.schema, "md-sim-s9-pool-input/8");
    assert_eq!(pool.seconds, 40);
    assert_eq!(pool.difficulty_exponents, [-29, -28]);
    assert_eq!(pool.connection_limit, Some(19));
    assert_eq!(
        pool.completion_policy()?,
        md_sim_mining::stratum::CompletionPolicy::KeepOpen
    );
    for (name, old) in names.into_iter().zip(before) {
        assert_eq!(hash_file(&source.join(name))?, old);
    }
    assert!(prepare(&source, &a).is_err());
    let derivative = temp.path().join("derivative");
    assert!(prepare(&a, &derivative).is_err());
    assert!(!derivative.exists());
    Ok(())
}

#[test]
fn derivation_rejects_changes_outside_entropy_and_time() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fixture(&source)?;
    let out = temp.path().join("fresh");
    let receipt = prepare(&source, &out)?;
    let parent: ProcessSpec = serde_json::from_slice(&fs::read(source.join("process-spec.json"))?)?;
    let child: ProcessSpec = serde_json::from_slice(&fs::read(out.join("process-spec.json"))?)?;
    let old = supported_pool(&source.join("pool-input.json"))?;
    let new = supported_pool(&out.join("pool-input.json"))?;
    for mutation in 0..7 {
        let mut changed = new.clone();
        let mut spec = child.clone();
        let mut r = receipt.clone();
        match mutation {
            0 => changed.seconds += 1,
            1 => changed.difficulty_exponents = [-31, -30],
            2 => changed.connection_limit = Some(20),
            3 => changed.schema = "md-sim-s9-pool-input/7".into(),
            4 => changed.seed = old.seed,
            5 => spec.limits.wall_seconds += 1,
            6 => r.time_after = r.time_before - 1,
            _ => unreachable!(),
        }
        assert!(
            check_derivation(&parent, &spec, &old, &changed, &r).is_err(),
            "mutation {mutation}"
        );
    }
    Ok(())
}

#[test]
fn full_parent_contract_and_receipt_fields_are_verified() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fixture(&source)?;
    let out = temp.path().join("fresh");
    let receipt = prepare(&source, &out)?;
    let original_receipt = fs::read(out.join(RECEIPT))?;
    let mut value: serde_json::Value = serde_json::from_slice(&original_receipt)?;
    value["unrecognized_claim"] = json!(true);
    fs::write(out.join(RECEIPT), serde_json::to_vec(&value)?)?;
    assert!(lock(&out).is_err());
    fs::write(out.join(RECEIPT), &original_receipt)?;
    // Re-hash a false parent claim as well: hashes alone cannot legitimize it.
    let mut parent: LockedRun = serde_json::from_slice(&fs::read(out.join(PARENT_LOCK))?)?;
    parent.policies["arbitrary_claim"] = json!("accepted");
    fs::write(out.join(PARENT_LOCK), serde_json::to_vec_pretty(&parent)?)?;
    let mut changed = receipt;
    changed.parent_lock = hash_file(&out.join(PARENT_LOCK))?;
    changed.parent_run_hash = parent.hash()?;
    fs::write(out.join(RECEIPT), serde_json::to_vec_pretty(&changed)?)?;
    let error = lock(&out).unwrap_err().to_string();
    assert!(
        error.contains("verified unchanged execution closure"),
        "{error}"
    );
    fs::remove_file(out.join(RECEIPT))?;
    assert!(lock(&out).is_err());
    Ok(())
}

#[test]
fn malformed_preparation_is_not_published_and_does_not_leave_staging() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    fixture(&source)?;
    let original = fs::read(source.join("process-spec.json"))?;
    let mut spec: ProcessSpec = serde_json::from_slice(&original)?;
    let pool = spec.inputs[1].object.clone();
    spec.inputs.push(Mount {
        guest_path: POOL.into(),
        object: pool.clone(),
    });
    fs::write(source.join("process-spec.json"), serde_json::to_vec(&spec)?)?;
    let out = temp.path().join("fresh");
    assert!(prepare(&source, &out).is_err());
    assert!(!out.exists());
    assert_eq!(fs::read_dir(temp.path())?.count(), 1);
    fs::write(source.join("process-spec.json"), &original)?;
    fs::write(source.join("pool-input.json"), b"{}")?;
    assert!(prepare(&source, &out).is_err());
    assert!(!out.exists());
    assert_eq!(fs::read_dir(temp.path())?.count(), 1);
    Ok(())
}

#[test]
fn atomic_publish_never_replaces_an_existing_destination() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("stage");
    let output = temp.path().join("other");
    fs::create_dir(&source)?;
    fs::create_dir(&output)?;
    fs::write(output.join("owned-by-someone-else"), b"preserve")?;
    assert!(publish(&source, &output).is_err());
    assert_eq!(fs::read(output.join("owned-by-someone-else"))?, b"preserve");
    assert!(source.is_dir());
    Ok(())
}

#[test]
fn fresh_metadata_rejects_symlinks_hardlinks_and_fifos() -> Result<()> {
    let temp = tempfile::tempdir()?;
    let real = temp.path().join("real");
    fs::write(&real, b"{}")?;
    let alias = temp.path().join("alias");
    std::os::unix::fs::symlink(&real, &alias)?;
    assert!(strict_bytes(&alias, 4096).is_err());
    fs::remove_file(&alias)?;
    fs::hard_link(&real, &alias)?;
    assert!(strict_bytes(&real, 4096).is_err());
    fs::remove_file(&alias)?;
    let fifo = CString::new(alias.as_os_str().as_bytes())?;
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    assert!(strict_bytes(&alias, 4096).is_err());
    assert_eq!(strict_bytes(&real, 4096)?, b"{}");
    Ok(())
}

//! Real-file regressions for one inventory with different per-path rules.
use super::*;
use md_sim_runtime::{isolation, store::Store};
use std::{
    os::unix::fs::symlink,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

fn populated(directory: &Path, count: usize) -> Result<()> {
    fs::create_dir(directory)?;
    for index in 0..count {
        fs::write(
            directory.join(format!("input-{index:05}")),
            index.to_le_bytes(),
        )?;
    }
    Ok(())
}

fn mixed_inventory(root: &Path) -> Result<()> {
    let payload = root.join("browser-payload");
    populated(&payload, 1200)?;
    let strict = root.join("shared-pool.json");
    fs::write(&strict, b"actual strict shared input\n")?;
    let mut files = BTreeMap::new();
    let mut links = BTreeMap::new();
    capture_inputs(&payload, &mut files, &mut links, 8192, true)?;
    capture_inputs(&strict, &mut files, &mut links, 8192, false)?;
    assert_eq!(files.len(), 1201);
    assert!(links.is_empty());
    assert_eq!(
        files[&strict.to_string_lossy().into_owned()],
        hash_file(&strict)?
    );

    let alias = root.join("shared-alias");
    symlink(&strict, &alias)?;
    let before = files.clone();
    let error = capture_inputs(&alias, &mut files, &mut links, 8192, false).unwrap_err();
    assert!(error
        .to_string()
        .contains("unexpected captured input symlink"));
    assert_eq!(files, before);
    assert!(links.is_empty());

    // Sparse length is checked before hashing, so this creates no large payload.
    let oversized = root.join("oversized-shared-input");
    fs::File::create(&oversized)?.set_len((256 << 20) + 1)?;
    let error = capture_inputs(&oversized, &mut files, &mut links, 8192, false).unwrap_err();
    assert!(error.to_string().contains("component input exceeds bound"));
    assert_eq!(files, before);
    Ok(())
}

fn exact_capacity(root: &Path, maximum: usize, browser: bool) -> Result<()> {
    let payload = root.join("payload");
    populated(&payload, maximum)?;
    let mut files = BTreeMap::new();
    let mut links = BTreeMap::new();
    capture_inputs(&payload, &mut files, &mut links, maximum, browser)?;
    assert_eq!(files.len(), maximum);

    // Shared tools can already occur inside /usr. Revalidation at the exact
    // aggregate cap must still apply strict rules without consuming capacity.
    capture_inputs(
        &payload.join("input-00000"),
        &mut files,
        &mut links,
        maximum,
        false,
    )?;
    let empty = root.join("empty");
    fs::create_dir(&empty)?;
    capture_inputs(&empty, &mut files, &mut links, maximum, false)?;
    let before = files.clone();
    let extra = root.join("extra");
    fs::write(&extra, b"must not enter the full inventory")?;
    let error = capture_inputs(&extra, &mut files, &mut links, maximum, false).unwrap_err();
    assert!(error
        .to_string()
        .contains("component input count exceeds bound"));
    assert_eq!(files, before);
    assert!(links.is_empty());
    Ok(())
}

#[test]
fn browser_inventory_then_strict_shared_input_preserves_both_policies() -> Result<()> {
    let directory = tempfile::tempdir()?;
    mixed_inventory(directory.path())
}

#[test]
fn basic_inventory_retains_1024_unique_entry_limit() -> Result<()> {
    let directory = tempfile::tempdir()?;
    exact_capacity(directory.path(), 1024, false)
}

#[test]
fn browser_inventory_retains_8192_unique_entry_limit() -> Result<()> {
    let directory = tempfile::tempdir()?;
    exact_capacity(directory.path(), 8192, true)
}

#[test]
fn strict_revisit_rejects_a_previously_allowed_browser_symlink() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let target = Path::new("/usr/lib").canonicalize()?;
    let alias = directory.path().join("browser-link");
    symlink(&target, &alias)?;
    let mut files = BTreeMap::new();
    let mut links = BTreeMap::new();
    capture_inputs(&alias, &mut files, &mut links, 1, true)?;
    assert_eq!(links.len(), 1);
    capture_inputs(&alias, &mut files, &mut links, 1, true)?;
    let before = links.clone();
    let error = capture_inputs(&alias, &mut files, &mut links, 1, false).unwrap_err();
    assert!(error
        .to_string()
        .contains("unexpected captured input symlink"));
    assert_eq!(links, before);
    assert!(files.is_empty());
    Ok(())
}

#[test]
#[ignore = "entry point executed by the real isolated regression"]
fn isolated_inventory_guest() -> Result<()> {
    ensure!(
        std::env::var("MD_SIM_INVENTORY_GUEST").as_deref() == Ok("1"),
        "isolated entry only"
    );
    ensure!(
        !Path::new(&std::env::var("MD_SIM_HOST_SENTINEL")?).exists(),
        "host path exposed"
    );
    let root = tempfile::tempdir()?;
    let mixed = root.path().join("mixed");
    fs::create_dir(&mixed)?;
    mixed_inventory(&mixed)?;
    let basic = root.path().join("basic");
    fs::create_dir(&basic)?;
    exact_capacity(&basic, 1024, false)?;
    let browser = root.path().join("browser");
    fs::create_dir(&browser)?;
    exact_capacity(&browser, 8192, true)?;
    println!("MD_SIM_INPUT_INVENTORY_ISOLATED_PASS mixed=1201 basic=1024 browser=8192 strict_symlink=rejected strict_oversize=rejected host_path=absent");
    Ok(())
}

#[test]
#[ignore = "run explicitly to retain the actual isolated process and input identities"]
fn actual_isolated_inventory_regression() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let retained = std::env::var_os("MD_SIM_INVENTORY_EVIDENCE").map(PathBuf::from);
    let root = retained.as_deref().unwrap_or(temporary.path());
    if retained.is_some() {
        fs::create_dir(root)?;
    }
    let sentinel = temporary.path().join("host-only-sentinel");
    fs::write(&sentinel, b"must remain outside the closed guest root")?;
    let store = Store::open(&root.join("inputs"), 64 << 20)?;
    let inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![("/inventory-check".into(), std::env::current_exe()?)],
    )?;
    let spec = isolation::ProcessSpec {
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        schema_version: "2.0".into(),
        argv: vec![
            "/inventory-check".into(),
            "input_inventory_tests::isolated_inventory_guest".into(),
            "--exact".into(),
            "--ignored".into(),
            "--nocapture".into(),
        ],
        inputs,
        limits: isolation::Limits {
            wall_seconds: 30,
            cpu_seconds: 20,
            memory_bytes: 256 << 20,
            processes: 32,
            // The sparse oversized-file negative uses length without payload.
            file_bytes: 257 << 20,
            trace_bytes: 1 << 20,
            tmp_bytes: 64 << 20,
        },
        environment: BTreeMap::from([
            ("MD_SIM_INVENTORY_GUEST".into(), "1".into()),
            (
                "MD_SIM_HOST_SENTINEL".into(),
                sentinel.to_string_lossy().into_owned(),
            ),
        ]),
        loopback: false,
        dashboard: None,
        stdin_object: None,
    };
    isolation::save_json(&root.join("process-spec.json"), &spec)?;
    let result = isolation::execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    isolation::save_json(&root.join("result.json"), &result)?;
    let stdout = hex::decode(&result.stdout)?;
    let stderr = hex::decode(&result.stderr)?;
    fs::write(root.join("guest.stdout"), &stdout)?;
    fs::write(root.join("guest.stderr"), &stderr)?;
    let text = std::str::from_utf8(&stdout)?;
    ensure!(
        result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
        "isolated inventory failed: status={}, exit={:?}, truncated={}, stderr={}",
        result.status,
        result.exit_code,
        result.trace_truncated,
        String::from_utf8_lossy(&stderr)
    );
    ensure!(
        text.contains("MD_SIM_INPUT_INVENTORY_ISOLATED_PASS")
            && text.contains("1 passed; 0 failed"),
        "isolated inventory evidence absent"
    );
    println!(
        "ISOLATED_INVENTORY_RECEIPT {}",
        json!({"specification":hash_file(&root.join("process-spec.json"))?,"result":hash_file(&root.join("result.json"))?,"stdout":hash_file(&root.join("guest.stdout"))?,"scope":"Actual isolated input inventory component; no firmware or browser execution"})
    );
    Ok(())
}

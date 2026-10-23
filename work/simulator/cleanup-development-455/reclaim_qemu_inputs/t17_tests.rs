use super::*;
use crate::{input_store_in, reclaim_object};
use md_sim_runtime::{
    isolation::{Limits, Mount},
    store::{hash_file, Store},
};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::{symlink, MetadataExt},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn object(bytes: &[u8]) -> ObjectId {
    ObjectId {
        sha256: md_sim_firmware::prepare::hash_bytes(bytes),
        size: bytes.len() as u64,
    }
}

fn publish(store: &Store, bytes: &[u8]) -> ObjectId {
    let id = object(bytes);
    store.begin(&id).unwrap();
    store.chunk(&id, 0, bytes).unwrap();
    store.finish(&id).unwrap();
    id
}

fn save(path: &Path, value: &impl Serialize) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn fixture(repository: &Path, number: &str, mode: Mode, legacy: bool, passed: bool) -> PathBuf {
    let prefix = if mode == Mode::Packing {
        "t17-core-oracle-"
    } else {
        "t17-readback-oracle-"
    };
    let run = repository
        .join("work/simulator/bitaxe")
        .join(format!("{prefix}{number}"));
    fs::create_dir_all(&run).unwrap();
    let store = Store::open(&run.join("inputs"), 1 << 30).unwrap();
    let mut inputs = Vec::new();
    for guest in [
        "/oracle-check",
        "/tools/clang",
        "/tools/qemu-arm",
        "/miner",
        "/oracle.c",
    ] {
        let bytes = format!("labelled cleanup fixture {number} {guest}");
        let id = publish(&store, bytes.as_bytes());
        inputs.push(Mount {
            object: id,
            guest_path: guest.into(),
        });
        if guest == "/oracle.c" {
            fs::write(run.join("oracle.c"), bytes).unwrap();
        }
    }
    if mode == Mode::Readback {
        for guest in ["/oracle.ld", "/oracle-abi.h"] {
            let bytes = format!("labelled cleanup fixture {number} {guest}");
            let id = publish(&store, bytes.as_bytes());
            inputs.push(Mount {
                object: id,
                guest_path: guest.into(),
            });
            fs::write(run.join(&guest[1..]), bytes).unwrap();
        }
        save(&run.join("compile-arguments.json"), &vec!["fixture-only"]);
    }
    let source = |name: &str| {
        inputs
            .iter()
            .find(|m| m.guest_path == name)
            .unwrap()
            .object
            .clone()
    };
    // A compact GNU sparse member exercises the format used by real446.
    let oracle = if mode == Mode::Readback {
        let mut bytes = vec![0; 4608];
        bytes[..512].fill(b'A');
        bytes[4096..].fill(b'B');
        bytes
    } else {
        b"labelled oracle fixture, not an executable".to_vec()
    };
    fs::write(run.join("oracle"), &oracle).unwrap();
    fs::write(run.join("execution.stdout"), b"labelled output fixture").unwrap();
    fs::write(run.join("execution.stderr"), b"").unwrap();
    let mut report = json!({"schema":if mode==Mode::Packing {"md-sim-vendor-core-packing-oracle/1"} else {"md-sim-vendor-core-readback-oracle/1"},
        "passed":passed,"error":if passed {None} else {Some("retained fixture failure")},
        "vendor_miner":source("/miner"),"wrapper":source("/oracle.c")});
    if !legacy {
        report["oracle_executable"] = json!(hash_file(&run.join("oracle")).unwrap());
        report["output_words"] = json!(hash_file(&run.join("execution.stdout")).unwrap());
    }
    if mode == Mode::Readback {
        report["link_script"] = json!(source("/oracle.ld"));
        report["abi_header"] = json!(source("/oracle-abi.h"));
        report["compile_arguments"] =
            json!(hash_file(&run.join("compile-arguments.json")).unwrap());
    }
    save(&run.join("assessment.json"), &report);
    let spec = ProcessSpec {
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        schema_version: "2.0".into(),
        argv: if mode == Mode::Packing {
            vec!["/oracle-check".into(), "__inside".into()]
        } else {
            vec!["/oracle-check".into(), "__inside".into(), "readback".into()]
        },
        inputs,
        limits: Limits::default(),
        environment: BTreeMap::new(),
        loopback: false,
        dashboard: None,
        stdin_object: None,
    };
    save(&run.join("spec.json"), &spec);
    drop(store);
    repack(&run, mode, passed);
    run
}

fn repack(run: &Path, mode: Mode, passed: bool) {
    let mut builder = tar::Builder::new(Vec::new());
    for name in MEMBERS {
        let p = run.join(name);
        if !p.exists() {
            continue;
        }
        let bytes = fs::read(p).unwrap();
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o600);
        if *name == "oracle" && mode == Mode::Readback {
            header.set_entry_type(tar::EntryType::GNUSparse);
            header.set_size(1024);
            let gnu = header.as_gnu_mut().unwrap();
            gnu.set_real_size(4608);
            gnu.sparse[0].set_offset(0);
            gnu.sparse[0].set_length(512);
            gnu.sparse[1].set_offset(4096);
            gnu.sparse[1].set_length(512);
            header.set_cksum();
            let stored = [&bytes[..512], &bytes[4096..]].concat();
            builder.append_data(&mut header, name, &stored[..]).unwrap();
        } else {
            header.set_size(bytes.len() as u64);
            header.set_cksum();
            builder.append_data(&mut header, name, &bytes[..]).unwrap();
        }
    }
    let bytes = builder.into_inner().unwrap();
    fs::write(run.join("evidence.tar"), &bytes).unwrap();
    fs::write(run.join("stderr.log"), b"").unwrap();
    let result = ProcessResult {
        private_channel: None,
        stdin_stream: None,
        dashboard: None,
        status: "exited".into(),
        exit_code: Some(if passed { 0 } else { 1 }),
        wall_milliseconds: 1,
        stdout: hex::encode(bytes),
        stderr: String::new(),
        trace_truncated: false,
        isolation: "labelled cleanup fixture; no process or firmware execution claim".into(),
        limits_scope: "test fixture".into(),
        tools: BTreeMap::new(),
        aggregate_limits: None,
    };
    save(&run.join("result.json"), &result);
}

#[test]
fn exact_path_and_mode_scope_excludes_other_finished_runs() {
    let root = Path::new("/repository");
    for s in [
        "t17-core-oracle-272",
        "t17-core-oracle-274",
        "t17-readback-oracle-446",
    ] {
        assert!(selected(
            &root.join("work/simulator/bitaxe").join(s).join("inputs"),
            root
        )
        .is_some());
    }
    for s in [
        "t17-core-oracle-27",
        "t17-core-oracle-0274",
        "t17-core-oracle-274-copy",
        "t17-readback-oracle-ABC",
        "t17-other-oracle-446",
    ] {
        assert!(selected(
            &root.join("work/simulator/bitaxe").join(s).join("inputs"),
            root
        )
        .is_none());
    }
    for s in [
        "work/simulator/network-vm/t17-core-oracle-274/inputs",
        "work/simulator/bitaxe/t17-core-oracle-274/input-store",
        "work/simulator/bitaxe/t17-core-oracle-274/nested/inputs",
        "artifacts/files/t17-core-oracle-274/inputs",
    ] {
        assert!(selected(&root.join(s), root).is_none());
    }
}

#[test]
fn regular_and_sparse_evidence_and_legacy_packing_remain_distinct() {
    let root = tempfile::tempdir().unwrap();
    for (number, mode, legacy, passed) in [
        ("272", Mode::Packing, true, true),
        ("274", Mode::Packing, false, true),
        ("446", Mode::Readback, false, true),
        ("448", Mode::Readback, false, false),
    ] {
        let run = fixture(root.path(), number, mode, legacy, passed);
        let evidence = finished(&run.join("inputs"), root.path()).unwrap().unwrap();
        assert_eq!(evidence.oracle_passed, passed);
        assert_eq!(evidence.legacy_packing_output_fields_absent, legacy);
        assert_eq!(
            evidence.members["oracle"],
            hash_file(&run.join("oracle")).unwrap()
        );
    }
    for number in ["274", "447"] {
        let later_root = tempfile::tempdir().unwrap();
        let later = fixture(later_root.path(), number, Mode::Packing, true, true);
        assert!(
            finished(&later.join("inputs"), later_root.path()).is_err(),
            "later packing records cannot omit output identities"
        );
    }
}

#[test]
fn changed_files_or_substituted_process_and_mode_fail_closed() {
    let root = tempfile::tempdir().unwrap();
    let run = fixture(root.path(), "446", Mode::Readback, false, true);
    for name in [
        "assessment.json",
        "oracle",
        "execution.stdout",
        "execution.stderr",
        "compile-arguments.json",
        "oracle.c",
        "oracle.ld",
        "oracle-abi.h",
        "stderr.log",
        "evidence.tar",
    ] {
        let path = run.join(name);
        let original = fs::read(&path).unwrap();
        fs::write(&path, b"substitution").unwrap();
        assert!(
            finished(&run.join("inputs"), root.path()).is_err(),
            "{name}"
        );
        fs::write(path, original).unwrap();
    }
    let p = run.join("spec.json");
    let original = fs::read(&p).unwrap();
    for key in ["argv", "inputs"] {
        let mut spec: Value = serde_json::from_slice(&original).unwrap();
        if key == "argv" {
            spec[key] = json!(["/oracle-check", "__inside"])
        } else {
            spec[key]
                .as_array_mut()
                .unwrap()
                .retain(|v| v["guest_path"] != "/oracle.ld");
        }
        save(&p, &spec);
        assert!(finished(&run.join("inputs"), root.path()).is_err());
    }
    fs::write(p, original).unwrap();
    let p = run.join("result.json");
    let original = fs::read(&p).unwrap();
    for (key, value) in [
        ("status", json!("running")),
        ("exit_code", json!(1)),
        ("trace_truncated", json!(true)),
        ("stdout", json!("00")),
    ] {
        let mut result: Value = serde_json::from_slice(&original).unwrap();
        result[key] = value;
        save(&p, &result);
        assert!(finished(&run.join("inputs"), root.path()).is_err(), "{key}");
    }
    fs::write(p, original).unwrap();
    finished(&run.join("inputs"), root.path()).unwrap();
}

#[test]
fn tar_aliases_traversal_links_extra_members_and_hidden_trailers_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let run = fixture(root.path(), "274", Mode::Packing, false, true);
    let bytes = fs::read(run.join("evidence.tar")).unwrap();
    let mut duplicate = bytes[..bytes.len() - 1024].to_vec();
    duplicate.extend_from_slice(&bytes);
    assert!(tar_members(&run, &duplicate).is_err());
    let mut trailer = bytes.clone();
    trailer.extend_from_slice(&[b'x'; 512]);
    assert!(tar_members(&run, &trailer).is_err());
    for (name, kind) in [
        ("unexpected", tar::EntryType::Regular),
        ("assessment.json", tar::EntryType::Symlink),
    ] {
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o600);
        header.set_entry_type(kind);
        header.set_size(0);
        header.set_cksum();
        builder
            .append_data(&mut header, name, std::io::empty())
            .unwrap();
        assert!(tar_members(&run, &builder.into_inner().unwrap()).is_err());
    }
    let mut absolute = bytes.clone();
    absolute[..100].fill(0);
    absolute[..16].copy_from_slice(b"/assessment.json");
    // Recompute the first checksum so rejection is about the path, not damage.
    let mut header = tar::Header::new_gnu();
    header.as_mut_bytes().copy_from_slice(&absolute[..512]);
    header.set_cksum();
    absolute[..512].copy_from_slice(header.as_bytes());
    assert!(tar_members(&run, &absolute).is_err());
}

#[test]
fn evidence_and_directory_links_fifos_and_growth_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let run = fixture(root.path(), "274", Mode::Packing, false, true);
    for name in [
        "assessment.json",
        "spec.json",
        "result.json",
        "evidence.tar",
        "stderr.log",
        "oracle.c",
    ] {
        let p = run.join(name);
        let saved = run.join(format!("saved-{name}"));
        fs::rename(&p, &saved).unwrap();
        symlink(&saved, &p).unwrap();
        assert!(finished(&run.join("inputs"), root.path()).is_err());
        fs::remove_file(&p).unwrap();
        fs::hard_link(&saved, &p).unwrap();
        assert!(finished(&run.join("inputs"), root.path()).is_err());
        fs::remove_file(&p).unwrap();
        let path = std::ffi::CString::new(p.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
        assert!(finished(&run.join("inputs"), root.path()).is_err());
        fs::remove_file(&p).unwrap();
        fs::rename(saved, p).unwrap();
    }
    let alias = run.with_file_name("t17-core-oracle-275");
    symlink(&run, &alias).unwrap();
    assert!(input_store_in(&alias.join("inputs"), root.path()).is_err());
    let p = run.join("stderr.log");
    fs::File::create(&p)
        .unwrap()
        .set_len(FILE_BYTES + 1)
        .unwrap();
    assert!(finished(&run.join("inputs"), root.path()).is_err());
}

#[test]
fn changed_after_locked_evidence_and_pinned_journal_copy_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let run = fixture(root.path(), "274", Mode::Packing, false, true);
    let owner = Store::open(&run.join("inputs"), 1 << 30).unwrap();
    assert!(Store::open(&run.join("inputs"), 1 << 30).is_err());
    let before = finished(&run.join("inputs"), root.path()).unwrap().unwrap();
    fs::write(
        run.join("execution.stdout"),
        b"different retained execution",
    )
    .unwrap();
    assert!(finished(&run.join("inputs"), root.path()).is_err());
    drop(owner);
    assert_eq!(before.mode, Mode::Packing);
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::write(&source, b"original").unwrap();
    fs::copy(&source, &target).unwrap();
    let id = hash_file(&source).unwrap();
    let record =
        json!({"path":target,"restore_copy_from":source,"sha256":id.sha256,"size":id.size});
    fs::write(&source, b"replaced").unwrap();
    fs::write(&target, b"replaced").unwrap();
    assert!(reclaim_object(&record).is_err());
    assert!(target.exists());
}

#[test]
#[ignore = "explicit real cleanup CLI scratch-store mutation and copy restoration"]
fn actual_cli_reclaims_only_pinned_duplicates_and_preserves_failed_evidence() {
    let executable = PathBuf::from(
        std::env::var_os("MD_SIM_RECLAIM_EXECUTABLE").expect("set explicit cleanup executable"),
    )
    .canonicalize()
    .unwrap();
    let repository = tempfile::tempdir().unwrap();
    let keeper_run = fixture(repository.path(), "274", Mode::Packing, false, true);
    let target_run = fixture(repository.path(), "448", Mode::Readback, false, false);
    let canonical = keeper_run.join("inputs");
    let target = target_run.join("inputs");
    let keeper = Store::open(&canonical, 1 << 30).unwrap();
    let owner = Store::open(&target, 1 << 30).unwrap();
    let duplicate = publish(&keeper, b"one common explicitly labelled fixture input");
    assert_eq!(
        publish(&owner, b"one common explicitly labelled fixture input"),
        duplicate
    );
    let unique = publish(&owner, b"unique input retained");
    drop(keeper);
    let duplicate_path = target.join("objects").join(&duplicate.sha256);
    let restore_path = canonical.join("objects").join(&duplicate.sha256);
    let unique_path = target.join("objects").join(&unique.sha256);
    let archive = repository.path().join("artifacts/files/original");
    fs::create_dir_all(archive.parent().unwrap()).unwrap();
    fs::write(&archive, b"archive must remain").unwrap();
    let mut preserved = vec![(archive.clone(), hash_file(&archive).unwrap())];
    for run in [&keeper_run, &target_run] {
        for name in ["overlay.tar.gz", "keycodes.tar.gz"] {
            fs::write(run.join(name), b"equal adjacent archive, retained").unwrap();
        }
        for entry in fs::read_dir(run).unwrap() {
            let p = entry.unwrap().path();
            if p.is_file() {
                preserved.push((p.clone(), hash_file(&p).unwrap()));
            }
        }
    }
    let journal = repository.path().join("restore.json");
    let invoke = |label: &str, from: &Path, to: &Path| {
        let out = repository.path().join(format!("{label}.out"));
        let err = repository.path().join(format!("{label}.err"));
        let mut child = Command::new(&executable)
            .current_dir(repository.path())
            .args([from, journal.as_path(), to])
            .stdin(Stdio::null())
            .stdout(fs::File::create(&out).unwrap())
            .stderr(fs::File::create(&err).unwrap())
            .spawn()
            .unwrap();
        let start = Instant::now();
        let status = loop {
            if let Some(s) = child.try_wait().unwrap() {
                break s;
            }
            if start.elapsed() > Duration::from_secs(5) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("cleanup CLI deadline")
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        (
            status,
            String::from_utf8(component_bytes(&out, 65536).unwrap().0).unwrap(),
            String::from_utf8(component_bytes(&err, 65536).unwrap().0).unwrap(),
        )
    };
    let (status, _, error) = invoke("active", &canonical, &target);
    assert!(!status.success());
    assert!(error.contains("store already has a writer"), "{error}");
    assert!(!journal.exists());
    assert!(duplicate_path.exists());
    drop(owner);
    let assessment = target_run.join("assessment.json");
    let saved = fs::read(&assessment).unwrap();
    fs::write(&assessment, b"substituted result").unwrap();
    let (status, _, _) = invoke("invalid-evidence", &canonical, &target);
    assert!(!status.success());
    assert!(!journal.exists());
    assert!(duplicate_path.exists());
    fs::write(&assessment, saved).unwrap();
    let (status, _, error) = invoke("protected-canonical", &target, &canonical);
    assert!(!status.success());
    assert!(error.contains("protected274"), "{error}");
    assert!(!journal.exists());
    assert_eq!(hash_file(&restore_path).unwrap(), duplicate);
    let expected = [
        finished(&canonical, repository.path()).unwrap().unwrap(),
        finished(&target, repository.path()).unwrap().unwrap(),
    ];
    let (status, output, error) = invoke("finished", &canonical, &target);
    assert!(status.success(), "{error}");
    assert!(output.contains("Reclaimed 1 duplicate inputs"), "{output}");
    assert!(!duplicate_path.exists());
    assert_eq!(hash_file(&restore_path).unwrap(), duplicate);
    assert_eq!(hash_file(&unique_path).unwrap(), unique);
    for (path, id) in preserved {
        assert_eq!(hash_file(&path).unwrap(), id, "{}", path.display());
    }
    let record: Value =
        serde_json::from_slice(&component_bytes(&journal, 1 << 20).unwrap().0).unwrap();
    assert_eq!(record["completed"], true);
    assert_eq!(record["removed_bytes"], duplicate.size);
    assert_eq!(record["objects"].as_array().unwrap().len(), 1);
    assert_eq!(
        record["oracle_cas_only_evidence"],
        serde_json::to_value(expected).unwrap()
    );
    assert_eq!(record["tool"], json!(hash_file(&executable).unwrap()));
    assert_eq!(
        record["objects"][0],
        json!({"path":duplicate_path,"restore_copy_from":restore_path,"sha256":duplicate.sha256,"size":duplicate.size})
    );
    fs::copy(&restore_path, &duplicate_path).unwrap();
    assert_eq!(hash_file(&duplicate_path).unwrap(), duplicate);
    assert_eq!(fs::metadata(duplicate_path).unwrap().nlink(), 1);
    println!("ACTUAL_T17_CLEANUP_CLI: active owner and changed evidence refused; one duplicate reclaimed and copy-restored; failed result, unique inputs, canonical store, adjacent archives and original archive unchanged");
}

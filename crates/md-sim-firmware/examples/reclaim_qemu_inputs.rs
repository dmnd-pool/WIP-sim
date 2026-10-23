//! Reclaim verified duplicate inputs from finished QEMU builds/runs and Core checks.
//! A journal gives a copy source for every removed byte. Unique inputs,
//! incomplete transfers, binaries, logs and the archive remain untouched.
use anyhow::{ensure, Result};
use md_sim_runtime::isolation::ProcessResult;
use md_sim_runtime::store::{hash_file, ObjectId, Store, StoreLimits};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
};

fn cycle_member(path: &Path, repository: &Path, core: bool) -> bool {
    let Some(run) = path.parent() else {
        return false;
    };
    let Some(cycle) = run.parent() else {
        return false;
    };
    if path.file_name().is_none_or(|name| name != "input-store")
        || cycle.parent() != Some(repository.join("work/simulator/qemu-s9").as_path())
    {
        return false;
    }
    let number = cycle
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| {
            name.strip_prefix("original-dashboard-cycle-")
                .or_else(|| name.strip_prefix("pcap-dashboard-cycle-"))
        });
    let numbered = number.is_some_and(|s| s.len() == 3 && s.bytes().all(|b| b.is_ascii_digit()));
    let member = run.file_name().and_then(|name| name.to_str()).unwrap_or("");
    numbered
        && if core {
            matches!(
                member,
                "core-provision-0"
                    | "core-provision-1"
                    | "core-0-0"
                    | "core-0-1"
                    | "core-1-0"
                    | "core-1-1"
                    | "core-2-0"
                    | "core-2-1"
            )
        } else {
            matches!(member, "provision" | "run-0" | "run-1" | "run-2")
        }
}

fn core_store(path: &Path, repository: &Path) -> bool {
    cycle_member(path, repository, true)
        || (path.parent().and_then(Path::parent)
            == Some(repository.join("work/simulator/bitcoin-core").as_path())
            && path.file_name().is_some_and(|n| n == "input-store")
            && path
                .parent()
                .and_then(Path::file_name)
                .and_then(|n| n.to_str())
                .is_some_and(|name| {
                    name.strip_prefix("original-s9-")
                        .and_then(|name| name.split_once("-candidate-"))
                        .is_some_and(|(run, candidate)| {
                            checkpoint(run) && matches!(candidate, "0" | "1")
                        })
                        || ["pcap", "channel"].iter().any(|prefix| {
                            name.strip_prefix(prefix)
                                .and_then(|name| name.split_once("-candidate"))
                                .is_some_and(|(run, tail)| {
                                    checkpoint(run)
                                        && tail.split_once('-').is_some_and(|(candidate, check)| {
                                            matches!(candidate, "0" | "1") && checkpoint(check)
                                        })
                                })
                        })
                }))
}

fn checkpoint(number: &str) -> bool {
    number.len() == 3 && number.bytes().all(|byte| byte.is_ascii_digit())
}

fn original_browser_run(path: &Path) -> bool {
    path.parent() == Some(Path::new("/tmp"))
        && path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("md-sim-original-boser-"))
            .is_some_and(|n| n.len() == 3 && n.bytes().all(|b| b.is_ascii_digit()))
}

fn coordinator_store(path: &Path, repository: &Path) -> Option<bool> {
    let parent = path.parent()?;
    if parent.parent() != Some(repository.join("work/simulator/coordinator").as_path()) {
        return None;
    }
    let name = parent.file_name()?.to_str()?;
    for (prefix, leaf, worker) in [("queue-", "input-store", false), ("worker-", "store", true)] {
        if path.file_name().is_some_and(|name| name == leaf)
            && name.strip_prefix(prefix).is_some_and(|number| {
                number.len() == 3 && number.bytes().all(|byte| byte.is_ascii_digit())
            })
        {
            return Some(worker);
        }
    }
    None
}

fn browser_capture(path: &Path, repository: &Path) -> bool {
    path.parent().and_then(Path::parent)
        == Some(repository.join("work/simulator/browser").as_path())
        && path.file_name().is_some_and(|name| name == "input-store")
        && path
            .parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix("captured-"))
            .is_some_and(|n| n.len() == 3 && n.bytes().all(|b| b.is_ascii_digit()))
}

fn pool_preparation(path: &Path, repository: &Path) -> bool {
    path.file_name().is_some_and(|name| name == "input-store")
        && path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "pool-vm-image")
        && path.parent().and_then(Path::parent).is_some_and(|run| {
            (run.parent() == Some(repository.join("work/simulator/qemu-s9").as_path())
                && run
                    .file_name()
                    .and_then(|name| name.to_str())
                    .and_then(|name| name.strip_prefix("sd2022-private-pool-"))
                    .is_some_and(|n| n.len() == 3 && n.bytes().all(|b| b.is_ascii_digit())))
                || (run.parent() == Some(Path::new("/tmp"))
                    && run
                        .file_name()
                        .and_then(|name| name.to_str())
                        .and_then(|name| name.strip_prefix("md-sim-original-boser-"))
                        .is_some_and(|n| n.len() == 3 && n.bytes().all(|b| b.is_ascii_digit())))
        })
}

fn channel_store<'a>(path: &'a Path, repository: &Path) -> Option<&'a Path> {
    if path.file_name()? != "input-store" {
        return None;
    }
    let parent = path.parent()?;
    let run = if parent
        .file_name()
        .is_some_and(|name| name == "client-image" || name == "execution")
    {
        parent.parent()?
    } else {
        parent
    };
    if run.parent()? != repository.join("work/simulator/network-vm") {
        return None;
    }
    let name = run.file_name()?.to_str()?;
    [
        "native-worker-channel-",
        "native-worker-disconnect-",
        "native-worker-stall-",
        "native-worker-firmware-",
        "native-worker-retention-",
        "native-ssh-channel-",
    ]
    .iter()
    .any(|prefix| {
        name.strip_prefix(prefix)
            .is_some_and(|n| n.len() == 3 && n.bytes().all(|b| b.is_ascii_digit()))
    })
    .then_some(run)
}

fn component_store<'a>(path: &'a Path, repository: &Path) -> Option<&'a Path> {
    let run = path.parent()?;
    if path.file_name()? != "input-store"
        || run.parent()? != repository.join("work/simulator/network-vm")
    {
        return None;
    }
    let name = run.file_name()?.to_str()?;
    ["byte-stream-", "worker-timeout-", "tls-readiness-"]
        .iter()
        .any(|prefix| name.strip_prefix(prefix).is_some_and(checkpoint))
        .then_some(run)
}

fn component_directories(path: &Path, repository: &Path) -> Result<()> {
    ensure!(
        component_store(path, repository).is_some()
            && !path.components().any(|part| part == Component::ParentDir),
        "component cleanup path is outside the declared scope"
    );
    let mut directory = repository.to_path_buf();
    ensure!(
        fs::symlink_metadata(&directory)?.is_dir(),
        "component repository must be a real directory"
    );
    for part in path.strip_prefix(repository)?.components() {
        directory.push(part);
        ensure!(
            fs::symlink_metadata(&directory)?.is_dir(),
            "component cleanup boundary contains a link or non-directory"
        );
    }
    Ok(())
}

fn component_bytes(path: &Path, limit: u64) -> Result<(Vec<u8>, ObjectId)> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.nlink() == 1 && metadata.len() <= limit,
        "component cleanup evidence must be a bounded regular file with one link"
    );
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == metadata.len() && bytes.len() as u64 <= limit,
        "component cleanup evidence changed size during its bounded read"
    );
    let identity = ObjectId {
        sha256: hex::encode(Sha256::digest(&bytes)),
        size: bytes.len() as u64,
    };
    Ok((bytes, identity))
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct ComponentEvidence {
    run: PathBuf,
    assessment: ObjectId,
    process_result: ObjectId,
    process_spec: ObjectId,
    guest_log: ObjectId,
    terminal_status: String,
}

fn require_finished_component(run: &Path) -> Result<ComponentEvidence> {
    let (assessment_bytes, assessment) = component_bytes(&run.join("assessment.json"), 64 * 1024)?;
    let report: serde_json::Value = serde_json::from_slice(&assessment_bytes)?;
    let (result_bytes, process_result) =
        component_bytes(&run.join("result.json"), 16 * 1024 * 1024)?;
    let (_, process_spec) = component_bytes(&run.join("process-spec.json"), 64 * 1024)?;
    let (_, guest_log) = component_bytes(&run.join("guest.log"), 8 * 1024 * 1024)?;
    for (field, identity) in [
        ("process_result", &process_result),
        ("process_spec", &process_spec),
        ("guest_log", &guest_log),
    ] {
        let expected: ObjectId = serde_json::from_value(report[field].clone())?;
        expected.validate()?;
        ensure!(
            expected == *identity,
            "component assessment {field} differs from the retained file"
        );
    }
    let result: ProcessResult = serde_json::from_slice(&result_bytes)?;
    ensure!(
        matches!(
            result.status.as_str(),
            "exited"
                | "cancelled"
                | "timed-out"
                | "cpu-limited"
                | "memory-limited"
                | "channel-failed"
                | "input-failed"
                | "dashboard-failed"
        ),
        "component execution is not terminal"
    );
    // Success is not required for duplicate reclamation. The complete failed
    // result and its exact assessment, specification and log remain untouched.
    Ok(ComponentEvidence {
        run: run.to_path_buf(),
        assessment,
        process_result,
        process_spec,
        guest_log,
        terminal_status: result.status,
    })
}

fn bounded_json(path: &Path) -> Result<serde_json::Value> {
    ensure!(fs::metadata(path)?.len() <= 65536, "cleanup metadata bound");
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn require_finished_channel(run: &Path) -> Result<()> {
    if run.join("assessment.json").exists() {
        let report = bounded_json(&run.join("assessment.json"))?;
        ensure!(
            report["execution"] == serde_json::to_value(hash_file(&run.join("result.json"))?)?
                && report["process_specification"]
                    == serde_json::to_value(hash_file(&run.join("process-spec.json"))?)?,
            "finished channel assessment must match its exact execution and process specification"
        );
    } else {
        ensure!(
            !run.join("process-spec.json").exists(),
            "outer channel execution may still be active"
        );
        let preparation = run.join("client-image");
        let failure = bounded_json(&preparation.join("failure.json"))?;
        let result = bounded_json(&preparation.join("preparation-result.json"))?;
        ensure!(
            failure["error"]
                .as_str()
                .is_some_and(|message| !message.is_empty())
                && (matches!(
                    result["status"].as_str(),
                    Some("timed-out" | "cancelled" | "cpu-limited" | "memory-limited")
                ) || (result["status"] == "exited"
                    && result["exit_code"].as_i64().is_some_and(|code| code != 0))
                    || result["trace_truncated"] == true),
            "client preparation has no terminal failure"
        );
    }
    Ok(())
}

fn require_finished_coordinator(path: &Path, worker: bool) -> Result<()> {
    let parent = path.parent().unwrap();
    let runs = parent.join(if worker { "runs" } else { "jobs" });
    ensure!(runs.canonicalize()? == runs, "run directory is a link");
    for entry in fs::read_dir(&runs)? {
        let entry = entry?;
        ensure!(entry.file_type()?.is_dir(), "unexpected run entry");
        if worker {
            ensure!(
                entry.path().join("cleanup.json").is_file(),
                "worker cleanup absent"
            );
        } else {
            for attempt in fs::read_dir(entry.path())? {
                let attempt = attempt?;
                ensure!(attempt.file_type()?.is_dir(), "unexpected attempt entry");
                let result = attempt.path().join("result.json");
                ensure!(
                    fs::metadata(&result)?.len() < 1024 * 1024,
                    "queue result bound"
                );
                let result: serde_json::Value = serde_json::from_slice(&fs::read(result)?)?;
                ensure!(
                    matches!(
                        result["status"].as_str(),
                        Some("pass" | "fail" | "blocked" | "cancelled" | "interrupted")
                    ),
                    "queue attempt is not terminal"
                );
            }
        }
    }
    Ok(())
}

fn eligible(path: &Path, repository: &Path) -> bool {
    let build = path.parent().and_then(Path::parent) == Some(Path::new("/tmp"))
        && path.file_name().is_some_and(|n| n == "inputs")
        && path
            .parent()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .is_some_and(|n| {
                n.starts_with("md-sim-qemu-backend-") || n.starts_with("md-sim-qemu-pool-")
            });
    let repository_build = path.parent().and_then(Path::parent)
        == Some(repository.join("work/simulator/qemu-s9").as_path())
        && path.file_name().is_some_and(|n| n == "inputs")
        && path
            .parent()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .and_then(|name| {
                name.strip_prefix("backend-build-")
                    .or_else(|| name.strip_prefix("pool-backend-build-"))
            })
            .and_then(|suffix| suffix.split_once('-'))
            .is_some_and(|(number, profile)| {
                number.len() == 3
                    && number.bytes().all(|b| b.is_ascii_digit())
                    && matches!(profile, "release" | "ubsan" | "pool")
            });
    let run = path.parent().and_then(Path::parent)
        == Some(repository.join("work/simulator/qemu-s9").as_path())
        && path.file_name().is_some_and(|n| n == "input-store")
        && path
            .parent()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .is_some_and(|n| {
                n.starts_with("sd2022-private-pool-")
                    || n.strip_prefix("native-pcap-").is_some_and(checkpoint)
            });
    build
        || repository_build
        || run
        || cycle_member(path, repository, false)
        || core_store(path, repository)
        || coordinator_store(path, repository).is_some()
        || browser_capture(path, repository)
        || pool_preparation(path, repository)
        || channel_store(path, repository).is_some()
        || component_store(path, repository).is_some()
}

fn input_store(path: &Path) -> Result<PathBuf> {
    let repository = std::env::current_dir()?.canonicalize()?;
    input_store_in(path, &repository)
}

fn input_store_in(path: &Path, repository: &Path) -> Result<PathBuf> {
    let supplied = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repository.join(path)
    };
    let path = supplied.canonicalize()?;
    if component_store(&supplied, repository).is_some()
        || component_store(&path, repository).is_some()
    {
        ensure!(
            supplied == path,
            "component store path must not be an alias"
        );
        component_directories(&supplied, repository)?;
    }
    ensure!(
        eligible(&path, repository),
        "only declared local QEMU or original-S9 Core input stores are eligible"
    );
    let parent = path.parent().unwrap();
    if cycle_member(&path, repository, false) || cycle_member(&path, repository, true) {
        ensure!(
            parent.parent().unwrap().join("assessment.json").is_file(),
            "the complete cycle must have stopped before cleanup"
        );
    }
    if let Some(run) = component_store(&path, repository) {
        require_finished_component(run)?;
    } else if let Some(run) = channel_store(&path, repository) {
        require_finished_channel(run)?;
    } else if browser_capture(&path, repository) {
        let capture = bounded_json(&parent.join("capture.json"))?;
        for (field, file) in [("inventory", "inventory.json"), ("payload", "payload.json")] {
            ensure!(
                capture[field] == serde_json::to_value(hash_file(&parent.join(file))?)?,
                "browser capture manifest differs"
            );
        }
    } else if pool_preparation(&path, repository) {
        let preparation = bounded_json(&parent.join("preparation-result.json"))?;
        ensure!(
            preparation["status"] == "exited"
                && preparation["exit_code"] == 0
                && preparation["trace_truncated"] == false,
            "pool preparation did not complete"
        );
        let identity = bounded_json(&parent.join("prepared-image.json"))?;
        ensure!(
            identity["schema"] == "md-sim-private-vm-preparation/1"
                && identity["root_manifest"]
                    == serde_json::to_value(hash_file(&parent.join("root-manifest.json"))?)?,
            "pool preparation identity differs"
        );
        let run = parent.parent().unwrap();
        ensure!(
            run.join("result.json").is_file()
                || bounded_json(&run.join("preparation-failure.json"))?["terminal_status"]
                    == "preparation-failed",
            "parent run is not terminal"
        );
    } else if let Some(worker) = coordinator_store(&path, repository) {
        require_finished_coordinator(&path, worker)?;
    } else if core_store(&path, repository) {
        let report: serde_json::Value =
            serde_json::from_reader(fs::File::open(parent.join("block-acceptance.json"))?)?;
        ensure!(
            report["accepted"] == true
                && report["persisted_after_restart"] == true
                && parent
                    .join("regtest-offline-import-startup-result.json")
                    .is_file(),
            "finished Core acceptance and persistence required"
        );
    } else {
        ensure!(
            parent.join("result.json").is_file(),
            "finished execution result required"
        );
    }
    Ok(path)
}

fn locked_component_evidence(path: &Path, repository: &Path) -> Result<Option<ComponentEvidence>> {
    component_store(path, repository)
        .map(|run| {
            component_directories(path, repository)?;
            require_finished_component(run)
        })
        .transpose()
}

fn reclaim_object(object: &serde_json::Value) -> Result<()> {
    let path = Path::new(object["path"].as_str().unwrap());
    let restore = Path::new(object["restore_copy_from"].as_str().unwrap());
    let expected = ObjectId {
        sha256: object["sha256"].as_str().unwrap().to_owned(),
        size: object["size"].as_u64().unwrap(),
    };
    ensure!(
        hash_file(path)? == expected && hash_file(restore)? == expected,
        "input or restore copy changed after the journal was written"
    );
    fs::remove_file(path)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() >= 4,
        "usage: reclaim_qemu_inputs CANONICAL_STORE NEW_JOURNAL FINISHED_STORE..."
    );
    let canonical = input_store(Path::new(&args[1]))?;
    let repository = std::env::current_dir()?.canonicalize()?;
    let journal = Path::new(&args[2]);
    ensure!(!journal.exists(), "journal must be new");
    // Store handles hold exclusive file locks. An active owner prevents cleanup.
    let keeper = open_store(&canonical)?;
    let mut component_evidence = Vec::new();
    if let Some(evidence) = locked_component_evidence(&canonical, &repository)? {
        component_evidence.push(evidence);
    }
    let mut locks = Vec::new();
    let mut objects = Vec::new();
    let mut total = 0u64;
    for path in &args[3..] {
        let root = input_store(Path::new(path))?;
        ensure!(root != canonical, "canonical store cannot be reclaimed");
        locks.push(open_store(&root)?);
        let component = locked_component_evidence(&root, &repository)?;
        let component_only = component.is_some();
        if let Some(evidence) = component {
            component_evidence.push(evidence);
        }
        for entry in fs::read_dir(root.join("objects"))? {
            let entry = entry?;
            let restore = canonical.join("objects").join(entry.file_name());
            if !restore.exists() {
                continue;
            }
            let object = hash_file(&entry.path())?;
            ensure!(
                entry.file_name() == object.sha256.as_str(),
                "published input hash mismatch"
            );
            let verified = keeper.verified_path(&object)?;
            ensure!(verified == restore, "canonical path differs");
            objects.push(json!({"path":entry.path(),"restore_copy_from":restore,
                "sha256":object.sha256,"size":object.size}));
            total += object.size;
        }
        // These are derived build inputs, not downloaded archive originals.
        // Keep the canonical copy and journal the exact restoration path.
        for name in ["overlay.tar.gz", "keycodes.tar.gz"] {
            if component_only {
                break;
            }
            let path = root.parent().unwrap().join(name);
            let restore = canonical.parent().unwrap().join(name);
            if !path.is_file() || !restore.is_file() {
                continue;
            }
            let object = hash_file(&path)?;
            if hash_file(&restore)? != object {
                continue;
            }
            objects.push(json!({"path":path,"restore_copy_from":restore,
                "sha256":object.sha256,"size":object.size}));
            total += object.size;
        }
        if core_store(&root, &std::env::current_dir()?.canonicalize()?) {
            let parent = root.parent().unwrap();
            let copies = parent.join("source-evidence");
            ensure!(
                copies.canonicalize()? == copies,
                "Core copy directory is a link"
            );
            let link: serde_json::Value =
                serde_json::from_reader(fs::File::open(parent.join("source-run-link.json"))?)?;
            let source = Path::new(
                link["source_run"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Core source run path absent"))?,
            )
            .canonicalize()?;
            let repository = std::env::current_dir()?.canonicalize()?;
            let channel_input = source.join("input-store");
            let channel = channel_store(&channel_input, &repository).filter(|run| {
                source.file_name().is_some_and(|name| name == "execution")
                    && run
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.starts_with("native-worker-firmware-"))
            });
            if let Some(run) = channel {
                require_finished_channel(run)?;
            }
            ensure!(
                ((eligible(&source.join("input-store"), &repository)
                    && source.starts_with(repository.join("work/simulator/qemu-s9")))
                    || original_browser_run(&source)
                    || channel.is_some())
                    && source.join("result.json").is_file(),
                "Core source must be a declared local firmware run"
            );
            for name in [
                "process-spec.json",
                "adaptation.json",
                "pool-result.json",
                "backend.log",
                "guest.log",
                "pool-input.json",
                "result.json",
            ] {
                let path = copies.join(name);
                if !path.try_exists()? {
                    continue;
                }
                ensure!(
                    !path.symlink_metadata()?.file_type().is_symlink(),
                    "source copy is a link"
                );
                let expected: ObjectId = serde_json::from_value(link["files"][name].clone())?;
                let restore = source.join(name);
                ensure!(
                    hash_file(&path)? == expected && hash_file(&restore)? == expected,
                    "Core evidence copy or original run changed"
                );
                objects.push(json!({"path":path,"restore_copy_from":restore,
                    "sha256":expected.sha256,"size":expected.size}));
                total += expected.size;
            }
        }
    }
    // Keep all Store locks while checking the evidence again and recording the
    // exact authorization. New component modes reclaim CAS duplicates only.
    for evidence in &component_evidence {
        component_directories(&evidence.run.join("input-store"), &repository)?;
        ensure!(
            require_finished_component(&evidence.run)? == *evidence,
            "component evidence changed while preparing cleanup"
        );
    }
    let mut report = json!({"scope":"Finished build/run/Core-check CAS, derived build archives and Core evidence copies. Original run evidence is retained. Restore with copies, never hard links",
        "objects":objects,"removed_bytes":total,"completed":false,
        "component_cas_only_evidence":component_evidence,
        "tool":hash_file(Path::new("/proc/self/exe"))?});
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(journal)?;
    file.write_all(&serde_json::to_vec_pretty(&report)?)?;
    file.sync_all()?;
    let journal_parent = journal
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::File::open(journal_parent)?.sync_all()?;
    for object in &objects {
        reclaim_object(object)?;
    }
    report["completed"] = json!(true);
    let completed = journal.with_extension("completed.tmp");
    ensure!(!completed.exists(), "completion path must be new");
    let mut completed_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&completed)?;
    completed_file.write_all(&serde_json::to_vec_pretty(&report)?)?;
    completed_file.sync_all()?;
    fs::rename(&completed, journal)?;
    fs::File::open(journal_parent)?.sync_all()?;
    println!(
        "Reclaimed {} duplicate inputs, {} bytes",
        objects.len(),
        total
    );
    Ok(())
}

fn open_store(path: &Path) -> Result<Store> {
    let repository = std::env::current_dir()?.canonicalize()?;
    if coordinator_store(path, &repository).is_some()
        || browser_capture(path, &repository)
        || pool_preparation(path, &repository)
    {
        let file = path.join("limits.json");
        ensure!(fs::metadata(&file)?.len() < 4096, "store policy bound");
        let limits: StoreLimits = serde_json::from_slice(&fs::read(file)?)?;
        return Store::open_with_limits(path, limits);
    }
    let limit = if path.file_name().is_some_and(|name| name == "input-store") {
        512 * 1024 * 1024
    } else {
        2 * 1024 * 1024 * 1024
    };
    Store::open(path, limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component_fixture() -> (tempfile::TempDir, PathBuf) {
        let repository = tempfile::tempdir().unwrap();
        let run = repository
            .path()
            .join("work/simulator/network-vm/tls-readiness-438");
        fs::create_dir_all(run.join("input-store")).unwrap();
        fs::write(run.join("process-spec.json"), b"{\"fixture\":true}").unwrap();
        fs::write(run.join("guest.log"), b"actual retained fixture log\n").unwrap();
        write_component_result(&run, "exited", Some(0));
        (repository, run)
    }

    fn write_component_result(run: &Path, status: &str, code: Option<i32>) {
        fs::write(
            run.join("result.json"),
            serde_json::to_vec(&json!({
                "status":status,"exit_code":code,"wall_milliseconds":20,
                "stdout":"fixture","stderr":"","trace_truncated":false,
                "isolation":"test fixture","limits_scope":"test fixture","tools":{}
            }))
            .unwrap(),
        )
        .unwrap();
        write_component_assessment(run);
    }

    fn write_component_assessment(run: &Path) {
        fs::write(
            run.join("assessment.json"),
            serde_json::to_vec(&json!({
                "process_result":hash_file(&run.join("result.json")).unwrap(),
                "process_spec":hash_file(&run.join("process-spec.json")).unwrap(),
                "guest_log":hash_file(&run.join("guest.log")).unwrap()
            }))
            .unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn component_cleanup_requires_exact_three_digit_direct_run_names() {
        let repository = Path::new("/repository");
        for prefix in ["byte-stream", "worker-timeout", "tls-readiness"] {
            let path = repository.join(format!(
                "work/simulator/network-vm/{prefix}-438/input-store"
            ));
            assert!(component_store(&path, repository).is_some());
            assert!(eligible(&path, repository));
        }
        for suffix in [
            "tls-readiness-38/input-store",
            "tls-readiness-0438/input-store",
            "tls-readiness-abc/input-store",
            "tls-readiness-438-extra/input-store",
            "tls-readiness-438/client-image/input-store",
            "tls-readiness-438/execution/input-store",
            "other/tls-readiness-438/input-store",
            "tls-readiness-438/objects",
            "tls-readiness-438/../input-store",
            "unrelated-438/input-store",
        ] {
            let path = repository.join("work/simulator/network-vm").join(suffix);
            assert!(
                component_store(&path, repository).is_none(),
                "{}",
                path.display()
            );
            assert!(!eligible(&path, repository), "{}", path.display());
        }
        for base in ["/archive", "/repository/artifacts/files", "/tmp"] {
            let path =
                Path::new(base).join("work/simulator/network-vm/tls-readiness-438/input-store");
            assert!(component_store(&path, repository).is_none());
            assert!(!eligible(&path, repository));
        }
    }

    #[test]
    fn exact_terminal_failures_qualify_but_active_or_partial_runs_do_not() {
        let (repository, run) = component_fixture();
        let store = run.join("input-store");
        input_store_in(&store, repository.path()).unwrap();
        for (status, code) in [
            ("exited", Some(153)),
            ("timed-out", None),
            ("cancelled", None),
            ("channel-failed", None),
        ] {
            write_component_result(&run, status, code);
            assert_eq!(
                require_finished_component(&run).unwrap().terminal_status,
                status
            );
        }
        for status in ["running", "booting", "preparing", "unknown", ""] {
            write_component_result(&run, status, None);
            assert!(input_store_in(&store, repository.path()).is_err());
        }
        write_component_result(&run, "exited", Some(0));
        fs::write(
            run.join("assessment.json"),
            b"{\"failure\":\"failure.json\"}",
        )
        .unwrap();
        assert!(require_finished_component(&run).is_err());
        fs::remove_file(run.join("assessment.json")).unwrap();
        assert!(require_finished_component(&run).is_err());
    }

    #[test]
    fn component_identity_checks_reject_substituted_result_specification_and_log() {
        let (_repository, run) = component_fixture();
        for name in ["result.json", "process-spec.json", "guest.log"] {
            let path = run.join(name);
            let original = fs::read(&path).unwrap();
            let mut substituted = original.clone();
            substituted[0] ^= 1;
            fs::write(&path, substituted).unwrap();
            assert!(require_finished_component(&run).is_err(), "{name}");
            fs::write(&path, original).unwrap();
            require_finished_component(&run).unwrap();
        }
        for field in ["process_result", "process_spec", "guest_log"] {
            let path = run.join("assessment.json");
            let original = fs::read(&path).unwrap();
            let mut value: serde_json::Value = serde_json::from_slice(&original).unwrap();
            value[field]["size"] = json!(value[field]["size"].as_u64().unwrap() + 1);
            fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
            assert!(require_finished_component(&run).is_err(), "{field}");
            fs::write(&path, original).unwrap();
        }
        fs::write(
            run.join("result.json"),
            br#"{"status":"exited","exit_code":0}"#,
        )
        .unwrap();
        write_component_assessment(&run);
        assert!(
            require_finished_component(&run).is_err(),
            "partial terminal result"
        );
    }

    #[test]
    fn component_paths_and_evidence_reject_links_fifos_and_oversize() {
        use std::os::unix::fs::symlink;
        let (repository, run) = component_fixture();
        let store = run.join("input-store");
        let alias = repository.path().join("alias");
        symlink(&store, &alias).unwrap();
        assert!(input_store_in(&alias, repository.path()).is_err());
        let named_alias = run.with_file_name("tls-readiness-439");
        symlink(&run, &named_alias).unwrap();
        assert!(input_store_in(&named_alias.join("input-store"), repository.path()).is_err());
        assert!(
            component_directories(&named_alias.join("input-store"), repository.path()).is_err()
        );
        fs::rename(&store, run.join("saved-store")).unwrap();
        symlink(run.join("saved-store"), &store).unwrap();
        assert!(input_store_in(&store, repository.path()).is_err());
        fs::remove_file(&store).unwrap();
        fs::rename(run.join("saved-store"), &store).unwrap();
        for name in [
            "assessment.json",
            "result.json",
            "process-spec.json",
            "guest.log",
        ] {
            let path = run.join(name);
            let saved = run.join("saved-record");
            fs::rename(&path, &saved).unwrap();
            symlink(&saved, &path).unwrap();
            assert!(require_finished_component(&run).is_err(), "symlink {name}");
            fs::remove_file(&path).unwrap();
            fs::hard_link(&saved, &path).unwrap();
            assert!(
                require_finished_component(&run).is_err(),
                "hard link {name}"
            );
            fs::remove_file(&path).unwrap();
            let fifo = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
            assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
            assert!(require_finished_component(&run).is_err(), "FIFO {name}");
            fs::remove_file(&path).unwrap();
            fs::rename(saved, path).unwrap();
        }
        let oversized = run.join("oversized");
        fs::write(&oversized, b"12345").unwrap();
        assert!(component_bytes(&oversized, 4).is_err());
        require_finished_component(&run).unwrap();
    }

    #[test]
    fn active_component_store_lock_and_post_lock_changed_evidence_are_rejected() {
        let (repository, run) = component_fixture();
        let path = run.join("input-store");
        let owner = open_store(&path).unwrap();
        assert!(open_store(&path).is_err());
        let before = locked_component_evidence(&path, repository.path())
            .unwrap()
            .unwrap();
        fs::write(run.join("guest.log"), b"substituted while lock held").unwrap();
        assert!(locked_component_evidence(&path, repository.path()).is_err());
        write_component_assessment(&run);
        assert_ne!(
            locked_component_evidence(&path, repository.path())
                .unwrap()
                .unwrap(),
            before
        );
        drop(owner);
        open_store(&path).unwrap();
    }

    #[test]
    fn journal_identity_prevents_deleting_two_identically_substituted_copies() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("duplicate");
        let restore = root.path().join("canonical");
        let unique = root.path().join("unique");
        for file in [&path, &restore] {
            fs::write(file, b"original").unwrap();
        }
        fs::write(&unique, b"unique").unwrap();
        let identity = hash_file(&path).unwrap();
        let record = json!({"path":path,"restore_copy_from":restore,
            "sha256":identity.sha256,"size":identity.size});
        for file in [&path, &restore] {
            fs::write(file, b"tampered").unwrap();
        }
        assert!(reclaim_object(&record).is_err());
        assert!(path.is_file() && restore.is_file() && unique.is_file());
        for file in [&path, &restore] {
            fs::write(file, b"original").unwrap();
        }
        reclaim_object(&record).unwrap();
        assert!(!path.exists());
        assert_eq!(hash_file(&restore).unwrap(), identity);
        assert_eq!(fs::read(unique).unwrap(), b"unique");
    }

    #[test]
    #[ignore = "requires MD_SIM_RECLAIM_EXECUTABLE pointing to the separately built example"]
    fn actual_cli_preserves_evidence_and_unique_inputs_and_restores_the_duplicate() {
        use std::process::{Command, Stdio};
        use std::time::{Duration, Instant};

        fn publish(store: &Store, bytes: &[u8]) -> ObjectId {
            let identity = ObjectId {
                sha256: hex::encode(Sha256::digest(bytes)),
                size: bytes.len() as u64,
            };
            store.begin(&identity).unwrap();
            store.chunk(&identity, 0, bytes).unwrap();
            store.finish(&identity).unwrap();
            identity
        }

        let executable = PathBuf::from(
            std::env::var_os("MD_SIM_RECLAIM_EXECUTABLE")
                .expect("set MD_SIM_RECLAIM_EXECUTABLE to the separately built example"),
        )
        .canonicalize()
        .unwrap();
        let (repository, canonical_run) = component_fixture();
        let target_run = canonical_run.with_file_name("tls-readiness-439");
        fs::create_dir_all(target_run.join("input-store")).unwrap();
        for name in ["process-spec.json", "guest.log"] {
            fs::copy(canonical_run.join(name), target_run.join(name)).unwrap();
        }
        write_component_result(&target_run, "timed-out", None);
        let canonical = canonical_run.join("input-store");
        let target = target_run.join("input-store");
        let keeper = open_store(&canonical).unwrap();
        let owner = open_store(&target).unwrap();
        let duplicate = publish(&keeper, b"exact duplicate input");
        assert_eq!(publish(&owner, b"exact duplicate input"), duplicate);
        let unique = publish(&owner, b"unique input retained");
        drop(keeper);
        let duplicate_path = target.join("objects").join(&duplicate.sha256);
        let restore_path = canonical.join("objects").join(&duplicate.sha256);
        let unique_path = target.join("objects").join(&unique.sha256);
        let mut preserved = Vec::new();
        for run in [&canonical_run, &target_run] {
            for name in ["overlay.tar.gz", "keycodes.tar.gz"] {
                fs::write(run.join(name), b"equal derived archive retained").unwrap();
            }
            for name in [
                "assessment.json",
                "result.json",
                "process-spec.json",
                "guest.log",
                "overlay.tar.gz",
                "keycodes.tar.gz",
            ] {
                let path = run.join(name);
                preserved.push((path.clone(), hash_file(&path).unwrap()));
            }
        }
        let journal = repository.path().join("restore-journal.json");
        let invoke = |attempt: &str| {
            let out = repository.path().join(format!("{attempt}.stdout"));
            let err = repository.path().join(format!("{attempt}.stderr"));
            let mut child = Command::new(&executable)
                .current_dir(repository.path())
                .args([&canonical, &journal, &target])
                .stdin(Stdio::null())
                .stdout(fs::File::create(&out).unwrap())
                .stderr(fs::File::create(&err).unwrap())
                .spawn()
                .unwrap();
            let started = Instant::now();
            let status = loop {
                if let Some(status) = child.try_wait().unwrap() {
                    break status;
                }
                if started.elapsed() >= Duration::from_secs(5) {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("cleanup CLI exceeded its five-second scratch-fixture budget");
                }
                std::thread::sleep(Duration::from_millis(10));
            };
            let (stdout, _) = component_bytes(&out, 65536).unwrap();
            let (stderr, _) = component_bytes(&err, 65536).unwrap();
            (
                status,
                String::from_utf8(stdout).unwrap(),
                String::from_utf8(stderr).unwrap(),
            )
        };
        let (status, _, error) = invoke("active-lock");
        assert!(!status.success());
        assert!(error.contains("store already has a writer"), "{error}");
        assert!(!journal.exists());
        assert_eq!(hash_file(&duplicate_path).unwrap(), duplicate);
        assert_eq!(hash_file(&unique_path).unwrap(), unique);
        drop(owner);

        let (status, output, error) = invoke("finished");
        assert!(status.success(), "{error}");
        assert!(output.contains("Reclaimed 1 duplicate inputs"), "{output}");
        assert!(!duplicate_path.exists());
        assert_eq!(hash_file(&restore_path).unwrap(), duplicate);
        assert_eq!(hash_file(&unique_path).unwrap(), unique);
        for (path, identity) in preserved {
            assert_eq!(hash_file(&path).unwrap(), identity, "{}", path.display());
        }
        let report = bounded_json(&journal).unwrap();
        assert_eq!(report["completed"], true);
        assert_eq!(report["removed_bytes"], duplicate.size);
        assert_eq!(report["objects"].as_array().unwrap().len(), 1);
        let object = &report["objects"][0];
        assert_eq!(object["path"], json!(duplicate_path));
        assert_eq!(object["restore_copy_from"], json!(restore_path));
        assert_eq!(object["sha256"], duplicate.sha256);
        assert_eq!(object["size"], duplicate.size);
        assert_eq!(
            report["tool"],
            serde_json::to_value(hash_file(&executable).unwrap()).unwrap()
        );
        let expected = [
            require_finished_component(&canonical_run).unwrap(),
            require_finished_component(&target_run).unwrap(),
        ];
        assert_eq!(
            report["component_cas_only_evidence"],
            serde_json::to_value(expected).unwrap()
        );
        assert!(!journal.with_extension("completed.tmp").exists());
        fs::copy(
            object["restore_copy_from"].as_str().unwrap(),
            object["path"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(hash_file(&duplicate_path).unwrap(), duplicate);
        assert_eq!(fs::metadata(&duplicate_path).unwrap().nlink(), 1);
        println!("Actual cleanup CLI: active lock refused; one exact duplicate reclaimed and restored; unique inputs, adjacent archives and all evidence retained.");
    }

    #[test]
    fn failed_preparation_cannot_authorize_cleanup_of_a_started_or_successful_run() {
        let root = tempfile::tempdir().unwrap();
        let run = root.path();
        let prep = run.join("client-image");
        fs::create_dir(&prep).unwrap();
        assert!(require_finished_channel(run).is_err());
        fs::write(
            prep.join("failure.json"),
            r#"{"error":"preparation failed"}"#,
        )
        .unwrap();
        fs::write(
            prep.join("preparation-result.json"),
            r#"{"status":"exited","exit_code":0,"trace_truncated":false}"#,
        )
        .unwrap();
        assert!(require_finished_channel(run).is_err());
        fs::write(
            prep.join("preparation-result.json"),
            r#"{"status":"exited","exit_code":153,"trace_truncated":false}"#,
        )
        .unwrap();
        require_finished_channel(run).unwrap();
        fs::write(run.join("process-spec.json"), b"{}").unwrap();
        assert!(require_finished_channel(run).is_err());
    }
    #[test]
    fn original_browser_evidence_requires_the_exact_local_run_path() {
        assert!(original_browser_run(Path::new(
            "/tmp/md-sim-original-boser-048"
        )));
        for name in [
            "/tmp/md-sim-original-boser-48",
            "/tmp/md-sim-original-boser-abc",
            "/tmp/other/md-sim-original-boser-048",
            "/archive/md-sim-original-boser-048",
            "/tmp/md-sim-original-boser-048/source-evidence",
        ] {
            assert!(!original_browser_run(Path::new(name)));
        }
    }
    #[test]
    fn only_explicit_build_and_run_store_paths_are_eligible() {
        let repo = Path::new("/repository");
        for path in [
            "/repository/work/simulator/qemu-s9/backend-build-296-release/inputs",
            "/repository/work/simulator/qemu-s9/pool-backend-build-296-ubsan/inputs",
            "/repository/work/simulator/qemu-s9/backend-build-328-pool/inputs",
            "/tmp/md-sim-qemu-backend-066-ubsan/inputs",
            "/tmp/md-sim-qemu-pool-003-ubsan/inputs",
            "/repository/work/simulator/qemu-s9/sd2022-private-pool-028/input-store",
            "/repository/work/simulator/qemu-s9/original-dashboard-cycle-003/provision/input-store",
            "/repository/work/simulator/qemu-s9/original-dashboard-cycle-003/run-0/input-store",
            "/repository/work/simulator/qemu-s9/original-dashboard-cycle-002/core-0-1/input-store",
            "/repository/work/simulator/qemu-s9/pcap-dashboard-cycle-307/provision/input-store",
            "/repository/work/simulator/qemu-s9/pcap-dashboard-cycle-307/run-1/input-store",
            "/repository/work/simulator/qemu-s9/pcap-dashboard-cycle-307/core-provision-0/input-store",
            "/repository/work/simulator/qemu-s9/pcap-dashboard-cycle-307/core-2-1/input-store",
            "/repository/work/simulator/coordinator/queue-007/input-store",
            "/repository/work/simulator/coordinator/worker-007/store",
            "/repository/work/simulator/browser/captured-003/input-store",
            "/repository/work/simulator/qemu-s9/sd2022-private-pool-043/pool-vm-image/input-store",
            "/tmp/md-sim-original-boser-044/pool-vm-image/input-store",
            "/repository/work/simulator/network-vm/native-worker-channel-342/input-store",
            "/repository/work/simulator/network-vm/native-ssh-channel-375/input-store",
            "/repository/work/simulator/network-vm/native-ssh-channel-375/client-image/input-store",
            "/repository/work/simulator/network-vm/native-worker-channel-342/execution/input-store",
            "/repository/work/simulator/network-vm/native-worker-stall-342/client-image/input-store",
            "/repository/work/simulator/qemu-s9/native-pcap-349/input-store",
            "/repository/work/simulator/bitcoin-core/pcap349-candidate0-351/input-store",
            "/repository/work/simulator/bitcoin-core/channel354-candidate1-357/input-store",
        ] {
            assert!(eligible(Path::new(path), repo));
        }
        for path in [
            "/repository/artifacts/files/backend-build-296-release/inputs",
            "/repository/work/simulator/qemu-s9/backend-build-296-release/input-store",
            "/repository/work/simulator/qemu-s9/backend-build-26-release/inputs",
            "/repository/work/simulator/qemu-s9/backend-build-296-debug/inputs",
            "/repository/work/simulator/qemu-s9/backend-build-296-release/other/inputs",
            "/repository/artifacts/files/inputs",
            "/tmp/unrelated/inputs",
            "/repository/work/simulator/qemu-s9/other/input-store",
            "/different/work/simulator/qemu-s9/sd2022-private-pool-028/input-store",
            "/tmp/md-sim-qemu-pool-003-ubsan/inputs/objects",
            "/repository/work/simulator/qemu-s9/original-dashboard-cycle-003/run-9/input-store",
            "/repository/work/simulator/qemu-s9/original-dashboard-cycle-003/core-9-0/input-store",
            "/repository/work/simulator/qemu-s9/original-dashboard-cycle-x/core-0-0/input-store",
            "/repository/work/simulator/qemu-s9/pcap-dashboard-cycle-37/run-1/input-store",
            "/repository/work/simulator/qemu-s9/pcap-dashboard-cycle-307/run-3/input-store",
            "/repository/work/simulator/qemu-s9/pcap-dashboard-cycle-307/core-provision-2/input-store",
            "/repository/work/simulator/qemu-s9/pcap-dashboard-cycle-307/not-a-run/input-store",
            "/repository/artifacts/files/pcap-dashboard-cycle-307/run-1/input-store",
            "/repository/work/simulator/coordinator/queue-007.inputs/input-store",
            "/repository/work/simulator/coordinator/worker-007/input-store",
            "/repository/work/simulator/coordinator/queue-7/input-store",
            "/repository/work/simulator/browser/captured-3/input-store",
            "/repository/work/simulator/qemu-s9/sd2022-private-pool-043/other/input-store",
            "/repository/work/simulator/qemu-s9/sd2022-private-pool-any/pool-vm-image/input-store",
            "/tmp/md-sim-original-boser-44/pool-vm-image/input-store",
            "/repository/work/simulator/network-vm/native-worker-channel-42/input-store",
            "/repository/work/simulator/network-vm/native-ssh-channel-75/input-store",
            "/repository/work/simulator/network-vm/native-ssh-channel-375/unrelated/input-store",
            "/repository/work/simulator/network-vm/native-worker-channel-342/arbitrary/input-store",
            "/repository/work/simulator/network-vm/native-worker-channel-342/execution/other/input-store",
            "/repository/artifacts/files/native-worker-channel-342/input-store",
            "/repository/work/simulator/qemu-s9/native-pcap-49/input-store",
            "/repository/work/simulator/bitcoin-core/pcap349-candidate2-351/input-store",
            "/repository/work/simulator/bitcoin-core/channel354-candidate1-57/input-store",
        ] {
            assert!(!eligible(Path::new(path), repo));
        }
    }
}

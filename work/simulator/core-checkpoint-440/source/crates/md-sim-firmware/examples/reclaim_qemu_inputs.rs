//! Reclaim verified duplicate inputs from finished QEMU builds/runs and Core checks.
//! A journal gives a copy source for every removed byte. Unique inputs,
//! incomplete transfers, binaries, logs and the archive remain untouched.
use anyhow::{ensure, Result};
use md_sim_runtime::store::{hash_file, ObjectId, Store, StoreLimits};
use serde_json::json;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
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
}

fn input_store(path: &Path) -> Result<PathBuf> {
    let path = path.canonicalize()?;
    let repository = std::env::current_dir()?.canonicalize()?;
    ensure!(
        eligible(&path, &repository),
        "only declared local QEMU or original-S9 Core input stores are eligible"
    );
    let parent = path.parent().unwrap();
    if cycle_member(&path, &repository, false) || cycle_member(&path, &repository, true) {
        ensure!(
            parent.parent().unwrap().join("assessment.json").is_file(),
            "the complete cycle must have stopped before cleanup"
        );
    }
    if let Some(run) = channel_store(&path, &repository) {
        require_finished_channel(run)?;
    } else if browser_capture(&path, &repository) {
        let capture = bounded_json(&parent.join("capture.json"))?;
        for (field, file) in [("inventory", "inventory.json"), ("payload", "payload.json")] {
            ensure!(
                capture[field] == serde_json::to_value(hash_file(&parent.join(file))?)?,
                "browser capture manifest differs"
            );
        }
    } else if pool_preparation(&path, &repository) {
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
    } else if let Some(worker) = coordinator_store(&path, &repository) {
        require_finished_coordinator(&path, worker)?;
    } else if core_store(&path, &repository) {
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

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() >= 4,
        "usage: reclaim_qemu_inputs CANONICAL_STORE NEW_JOURNAL FINISHED_STORE..."
    );
    let canonical = input_store(Path::new(&args[1]))?;
    let journal = Path::new(&args[2]);
    ensure!(!journal.exists(), "journal must be new");
    // Store handles hold exclusive file locks. An active owner prevents cleanup.
    let keeper = open_store(&canonical)?;
    let mut locks = Vec::new();
    let mut objects = Vec::new();
    let mut total = 0u64;
    for path in &args[3..] {
        let root = input_store(Path::new(path))?;
        ensure!(root != canonical, "canonical store cannot be reclaimed");
        locks.push(open_store(&root)?);
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
    let mut report = json!({"scope":"Finished build/run/Core-check CAS, derived build archives and Core evidence copies. Original run evidence is retained. Restore with copies, never hard links",
        "objects":objects,"removed_bytes":total,"completed":false,
        "tool":hash_file(Path::new("/proc/self/exe"))?});
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(journal)?;
    file.write_all(&serde_json::to_vec_pretty(&report)?)?;
    file.sync_all()?;
    for object in &objects {
        let path = Path::new(object["path"].as_str().unwrap());
        let restore = Path::new(object["restore_copy_from"].as_str().unwrap());
        ensure!(
            hash_file(path)? == hash_file(restore)?,
            "input changed before reclaim"
        );
        fs::remove_file(path)?;
    }
    report["completed"] = json!(true);
    let completed = journal.with_extension("completed.tmp");
    ensure!(!completed.exists(), "completion path must be new");
    fs::write(&completed, serde_json::to_vec_pretty(&report)?)?;
    fs::rename(&completed, journal)?;
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

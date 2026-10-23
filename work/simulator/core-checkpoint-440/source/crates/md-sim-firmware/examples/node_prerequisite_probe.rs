//! Inspect a Bitcoin daemon only inside the strict isolated worker.
//! Candidate checks require the pinned source build and linked firmware evidence.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[path = "../src/confinement.rs"]
mod confinement;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
};

const PHASES: [&str; 3] = [
    "md-sim Core phase: initialize fresh regtest genesis",
    "md-sim Core phase: import candidate after persisted genesis",
    "md-sim Core phase: restart after persisted candidate",
];
const GENESIS: &str = "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206";

// Persistent real mining can retain up to1024 submissions plus both block
// records. Thermal170 produced a valid3MiB report, above the old1MiB cap.
const BLOCK_EVIDENCE_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq)]
enum EvidenceMode {
    Runner,
    Shared,
}

fn candidate_input(args: &[OsString]) -> Result<Option<(PathBuf, usize, EvidenceMode)>> {
    if args.is_empty() || args == [OsString::from("offline")] {
        return Ok(None);
    }
    let (mode, args) = if args[0] == "shared" {
        (EvidenceMode::Shared, &args[1..])
    } else {
        (EvidenceMode::Runner, args)
    };
    ensure!(
        (1..=2).contains(&args.len()) && args[0] != "offline",
        "expected [offline|[shared] REGTEST_BLOCKS_JSON [CANDIDATE_INDEX]]"
    );
    let index = args
        .get(1)
        .map(|value| value.to_string_lossy().parse::<usize>())
        .transpose()?;
    Ok(Some((PathBuf::from(&args[0]), index.unwrap_or(0), mode)))
}

fn replay_command(evidence: &Value, mode: EvidenceMode) -> Result<&'static str> {
    let shared = evidence["result"].get("external_pool");
    ensure!(
        shared.is_some() == (mode == EvidenceMode::Shared),
        "pool provenance requires the matching explicit node evidence mode"
    );
    if let Some(shared) = shared {
        ensure!(
            shared["schema_version"] == "md-sim-s9-external-pool-join/1",
            "unknown shared pool evidence schema"
        );
        Ok("regtest-session-shared")
    } else {
        Ok("regtest-session")
    }
}

fn block_evidence(path: &Path) -> Result<Value> {
    let file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= BLOCK_EVIDENCE_BYTES,
        "block evidence type or size exceeds bound"
    );
    let mut bytes = Vec::new();
    file.take(BLOCK_EVIDENCE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= BLOCK_EVIDENCE_BYTES,
        "block evidence grew beyond bound"
    );
    Ok(serde_json::from_slice(&bytes)?)
}

fn copy_bound(source: &Path, target: &Path, expected: &ObjectId, limit: u64) -> Result<()> {
    expected.validate()?;
    ensure!(expected.size <= limit, "evidence exceeds copy bound");
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(source)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() == expected.size,
        "evidence type or extent changed"
    );
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(target)?;
    let copied = std::io::copy(&mut file.take(expected.size + 1), &mut output)?;
    ensure!(
        copied == expected.size,
        "evidence changed during bounded copy"
    );
    ensure!(
        hash_file(target)? == *expected,
        "copied evidence identity differs"
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "__three_phase") {
        confinement::require(true)?;
        let arguments = &args[2..];
        ensure!(
            !arguments
                .iter()
                .any(|arg| arg.to_string_lossy().starts_with("-loadblock")),
            "loadblock is controlled by the import phase"
        );
        for (phase, label) in PHASES.iter().enumerate() {
            println!("{label}");
            let mut command = Command::new("/daemon");
            command.args(arguments).stdin(Stdio::null());
            if phase == 1 {
                command.arg("-loadblock=/candidate.blk");
            }
            let status = command.status()?;
            ensure!(
                matches!(status.code(), Some(0) | Some(1)),
                "Core phase terminated unexpectedly: {status}"
            );
        }
        return Ok(());
    }
    ensure!(
        (3..=6).contains(&args.len()),
        "usage: node_prerequisite_probe DAEMON NEW_OUTPUT [offline|[shared] REGTEST_BLOCKS_JSON [CANDIDATE_INDEX]]"
    );
    let candidate = candidate_input(&args[3..])?;
    let daemon = PathBuf::from(&args[1]).canonicalize()?;
    let daemon_object = hash_file(&daemon)?;
    let out = PathBuf::from(&args[2]);
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(&out)?;
    save_json(
        &out.join("input-origin.json"),
        &json!({"path":daemon,"object":hash_file(&daemon)?,"source_identity":"unresolved until separately verified","stage":"before-execution"}),
    )?;
    let store = Store::open(&out.join("input-store"), 512 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/daemon".into(), daemon),
            ("/probe".into(), std::env::current_exe()?),
        ],
    )?;
    let expected_hash = if let Some((input, index, mode)) = candidate {
        let evidence = block_evidence(&input)?;
        let original = evidence.is_object();
        ensure!(
            original || mode == EvidenceMode::Runner,
            "shared mode requires original firmware evidence"
        );
        let records = candidate_records(&evidence)?;
        let record = records
            .get(index)
            .context("no firmware-produced block at selected index")?;
        let registry_path = Path::new("simulator/backends/bitcoin-core-619334569b44.json");
        let registry: Value =
            serde_json::from_slice(&fs::read(registry_path).context(
                "verified source-build backend registry is required for block acceptance",
            )?)?;
        ensure!(
            registry["source_archive_sha256"]
                == "776f4a708eb1a461872a6f1dba28218f0ed09af04239c29115412eecd3de1bc8"
                && registry["source_commit"] == "619334569b44b4278d0155bb8daf4c8535a47367",
            "backend registry has a different source identity"
        );
        ensure!(
            registry["binary"] == serde_json::to_value(&daemon_object)?,
            "daemon is not the source-verified backend in the repository registry"
        );
        save_json(
            &out.join("core-backend-identity.json"),
            &json!({"registry":registry,"registry_file":hash_file(registry_path)?}),
        )?;
        if original {
            link_original_run(&input, &evidence, record, &out, mode)?;
        } else {
            link_source_run(&input, record, &out)?;
        }
        let (bytes, expected_hash) = validate_block(record)?;
        save_json(&out.join("firmware-block-record.json"), record)?;
        save_json(
            &out.join("firmware-block-origin.json"),
            &json!({"path":input,"object":hash_file(&input)?}),
        )?;
        let path = out.join("candidate.blk");
        fs::write(&path, bytes)?;
        let object = hash_file(&path)?;
        store.import(&path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: "/candidate.blk".into(),
        });
        Some(expected_hash)
    } else {
        None
    };
    for (name, mut arguments) in [
        (
            "version",
            vec!["/daemon", "-datadir=/work", "-conf=/dev/null", "-version"],
        ),
        (
            "regtest-startup",
            vec![
                "/daemon",
                "-regtest",
                "-datadir=/work",
                "-conf=/dev/null",
                "-disablewallet",
                "-server=1",
                "-listen=0",
                "-dnsseed=0",
                "-connect=0",
                "-discover=0",
                "-maxconnections=0",
                "-dbcache=4",
                "-par=1",
                "-printtoconsole=1",
                "-rpcbind=127.0.0.1:18443",
                "-rpcallowip=127.0.0.1",
                "-rpcport=18443",
                "-rpcuser=md-sim-probe",
                "-rpcpassword=run-owned-probe-only",
            ],
        ),
        (
            "regtest-offline-import-startup",
            vec![
                "/daemon",
                "-regtest",
                "-datadir=/work",
                "-conf=/dev/null",
                "-disablewallet",
                "-server=0",
                "-listen=0",
                "-connect=0",
                "-dnsseed=0",
                "-fixedseeds=0",
                "-networkactive=0",
                "-discover=0",
                "-maxconnections=0",
                "-dbcache=4",
                "-par=1",
                "-printtoconsole=1",
                "-stopafterblockimport",
            ],
        ),
    ] {
        if args.len() >= 4 && name != "regtest-offline-import-startup" {
            continue;
        }
        let argv = if expected_hash.is_some() {
            let mut argv = vec!["/probe".to_string(), "__three_phase".to_string()];
            argv.extend(arguments.drain(1..).map(str::to_string));
            argv
        } else {
            arguments.into_iter().map(str::to_string).collect()
        };
        let spec = ProcessSpec {
            private_channel: None,
            stdin_stream: None,
            namespace_uid: 0,
            dashboard: None,
            loopback: false,
            stdin_object: None,
            schema_version: "2.0".into(),
            argv,
            inputs: inputs.clone(),
            environment: BTreeMap::new(),
            limits: Limits {
                wall_seconds: 15,
                cpu_seconds: 10,
                memory_bytes: 2 * 1024 * 1024 * 1024,
                processes: 32,
                file_bytes: 64 * 1024 * 1024,
                trace_bytes: 2 * 1024 * 1024,
                tmp_bytes: 256 * 1024 * 1024,
            },
        };
        save_json(&out.join(format!("{name}-spec.json")), &spec)?;
        let result = match execute(&store, &spec, Arc::new(AtomicBool::new(false))) {
            Ok(result) => result,
            Err(error) => {
                save_json(
                    &out.join(format!("{name}-infrastructure-error.json")),
                    &json!({"accepted":false,"error":format!("{error:#}"),"spec":hash_file(&out.join(format!("{name}-spec.json")))?}),
                )?;
                return Err(error);
            }
        };
        fs::write(
            out.join(format!("{name}-stdout.log")),
            hex::decode(&result.stdout)?,
        )?;
        fs::write(
            out.join(format!("{name}-stderr.log")),
            hex::decode(&result.stderr)?,
        )?;
        save_json(&out.join(format!("{name}-result.json")), &result)?;
        if let Some(expected_hash) = &expected_hash {
            let stdout = String::from_utf8(hex::decode(&result.stdout)?)?;
            let observed = analyze_phases(&stdout, expected_hash);
            let complete = observed.accepted
                && observed.genesis
                && observed.persisted
                && observed.clean_stop
                && result.status == "exited"
                && matches!(result.exit_code, Some(0) | Some(1))
                && !result.trace_truncated;
            save_json(
                &out.join("block-acceptance.json"),
                &json!({
                    "expected_block_hash":expected_hash,"observed_height_before":if observed.genesis {Some(0)}else{None},
                    "observed_height_after":if observed.accepted {Some(1)}else{None},"accepted":complete,
                    "persisted_after_restart":observed.persisted,"observed_height_after_restart":if observed.persisted {Some(1)}else{None},
                    "node_result_status":result.status,"node_exit_code":result.exit_code,"clean_stop_after_each_phase":observed.clean_stop,
                    "rpc_used":false,"node_generated_block":false,"original_board_gate":"not-established",
                    "topology":"three sequential original Core processes in one private namespace and run-owned temporary storage; initialize genesis, import candidate, then restart without loadblock",
                    "spec":hash_file(&out.join(format!("{name}-spec.json")))?,
                    "stdout":hash_file(&out.join(format!("{name}-stdout.log")))?,
                    "block_input":hash_file(&out.join("candidate.blk"))?,
                    "source_run_link":hash_file(&out.join("source-run-link.json"))?,
                    "core_backend_identity":hash_file(&out.join("core-backend-identity.json"))?,
                    "restart_oracle":"Pinned Core Loaded best chain: hashBestChain=<exact candidate> height=1 in the third process, with no loadblock argument. Original contract observed in source-build v10 logs for persisted genesis.",
                    "evidence_limit":"Checks explicit Core UpdateTip from fresh isolated chainstate and Loaded best chain after process restart; stop-after-import may return exit1 during initialization. Storage is private and temporary; no host-crash durability, RPC or full target gate claim."
                }),
            )?;
            ensure!(
                complete,
                "Core did not prove exact block acceptance, chain-state change and restart persistence"
            );
        }
        println!(
            "{}",
            json!({"probe":name,"status":result.status,"exit_code":result.exit_code,"regtest_block_acceptance":if expected_hash.is_some(){"passed"}else{"not-run"},"complete_target_gate":false})
        );
    }
    Ok(())
}

#[derive(Debug, Default, PartialEq)]
struct PhaseEvidence {
    genesis: bool,
    accepted: bool,
    persisted: bool,
    clean_stop: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_shared_arguments_preserve_default_and_offline_modes() -> Result<()> {
        let parse =
            |args: &[&str]| candidate_input(&args.iter().map(OsString::from).collect::<Vec<_>>());
        assert!(parse(&[])?.is_none());
        assert!(parse(&["offline"])?.is_none());
        assert_eq!(
            parse(&["audit.json"])?,
            Some(("audit.json".into(), 0, EvidenceMode::Runner))
        );
        assert_eq!(
            parse(&["audit.json", "1"])?,
            Some(("audit.json".into(), 1, EvidenceMode::Runner))
        );
        assert_eq!(
            parse(&["shared", "audit.json"])?,
            Some(("audit.json".into(), 0, EvidenceMode::Shared))
        );
        assert_eq!(
            parse(&["shared", "audit.json", "1"])?,
            Some(("audit.json".into(), 1, EvidenceMode::Shared))
        );
        for args in [
            vec!["shared"],
            vec!["offline", "1"],
            vec!["shared", "offline"],
            vec!["shared", "audit.json", "-1"],
            vec!["audit.json", "0", "extra"],
        ] {
            assert!(parse(&args).is_err(), "accepted {args:?}");
        }
        Ok(())
    }

    #[test]
    fn shared_node_replay_requires_explicit_provenance_mode() -> Result<()> {
        let original = json!({"result":{}});
        assert_eq!(
            replay_command(&original, EvidenceMode::Runner)?,
            "regtest-session"
        );
        assert!(replay_command(&original, EvidenceMode::Shared).is_err());
        let shared =
            json!({"result":{"external_pool":{"schema_version":"md-sim-s9-external-pool-join/1"}}});
        assert_eq!(
            replay_command(&shared, EvidenceMode::Shared)?,
            "regtest-session-shared"
        );
        assert!(replay_command(&shared, EvidenceMode::Runner).is_err());
        assert!(replay_command(
            &json!({"result":{"external_pool":null}}),
            EvidenceMode::Shared
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn shared_link_rejects_changed_pool_and_outer_identity() -> Result<()> {
        // Contract fixture only. No firmware, pool or Core result is claimed.
        let root = tempfile::tempdir()?;
        let source = root.path().join("execution");
        let target = root.path().join("copied");
        fs::create_dir_all(source.join("shared-pool"))?;
        let mut join = json!({});
        for (field, path) in shared_files() {
            let path = source.join(path);
            fs::write(&path, format!("fixture {field}"))?;
            join[field] = serde_json::to_value(hash_file(&path)?)?;
        }
        join["outer_result"] = json!({});
        for (field, path) in [
            ("result", root.path().join("result.json")),
            ("process", root.path().join("process-spec.json")),
            ("record", source.join("shared-pool/outer-result.json")),
        ] {
            fs::write(&path, format!("fixture {field}"))?;
            join["outer_result"][field] = serde_json::to_value(hash_file(&path)?)?;
        }
        let retained = retain_shared_link(&source, &target, &join)?;
        assert_eq!(retained["join"], join);
        assert_eq!(retained["copied_files"].as_object().unwrap().len(), 7);
        let mut changed = join.clone();
        changed["pool_stdout"]["sha256"] = json!("00".repeat(32));
        assert!(retain_shared_link(&source, &root.path().join("changed-pool"), &changed).is_err());
        fs::write(root.path().join("result.json"), b"changed outer result")?;
        assert!(retain_shared_link(&source, &root.path().join("changed-outer"), &join).is_err());
        Ok(())
    }

    #[test]
    fn bounded_copy_rejects_growth_symlink_and_existing_destination() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let source = directory.path().join("source");
        fs::write(&source, b"evidence")?;
        let expected = hash_file(&source)?;
        let target = directory.path().join("target");
        copy_bound(&source, &target, &expected, 16)?;
        assert_eq!(fs::read(&target)?, b"evidence");
        assert!(copy_bound(&source, &target, &expected, 16).is_err());
        assert_eq!(fs::read(&target)?, b"evidence");
        let link = directory.path().join("link");
        std::os::unix::fs::symlink(&source, &link)?;
        let rejected = directory.path().join("rejected");
        assert!(copy_bound(&link, &rejected, &expected, 16).is_err());
        assert!(!rejected.exists());
        fs::File::create(&source)?.set_len(1024 * 1024)?;
        assert!(copy_bound(&source, &rejected, &expected, 16).is_err());
        assert!(!rejected.exists());
        Ok(())
    }

    #[test]
    fn complete_submission_evidence_can_exceed_one_mib_but_remains_bounded() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("evidence.json");
        let value = json!({"trace":"x".repeat(1024 * 1024 + 1),"candidate":1});
        fs::write(&path, serde_json::to_vec(&value)?)?;
        assert_eq!(block_evidence(&path)?, value);
        fs::File::create(&path)?.set_len(BLOCK_EVIDENCE_BYTES + 1)?;
        assert!(block_evidence(&path).is_err());
        assert!(block_evidence(directory.path()).is_err());
        fs::write(&path, b"{broken")?;
        assert!(block_evidence(&path).is_err());
        Ok(())
    }

    #[test]
    fn candidate_contract_keeps_original_and_source_fixture_evidence_distinct() {
        let source = json!([{"source_submission_index":0}]);
        assert_eq!(candidate_records(&source).unwrap().len(), 1);
        let original = json!({"result":{"kind":"independent-original-firmware-submission-correlation/1", "regtest_blocks":[{"source_submission_index":1}]}});
        assert_eq!(
            candidate_records(&original).unwrap()[0]["source_submission_index"],
            1
        );
        for bad in [
            json!({"regtest_blocks":[]}),
            json!({"result":{"kind":"API-only","regtest_blocks":[]}}),
            json!({"result":{"kind":"independent-original-firmware-submission-correlation/1"}}),
        ] {
            assert!(candidate_records(&bad).is_err());
        }
    }

    fn transcript() -> String {
        format!(
            "{}\nT UpdateTip: new best={GENESIS} height=0\nT Stopping after block import\nT Shutdown done\n{}\nT Loaded best chain: hashBestChain={GENESIS} height=0\nT UpdateTip: new best=candidate height=1\nT Stopping after block import\nT Shutdown done\n{}\nT Loaded best chain: hashBestChain=candidate height=1\nT Stopping after block import\nT Shutdown done\n",
            PHASES[0], PHASES[1], PHASES[2]
        )
    }

    #[test]
    fn restart_requires_exact_tip_in_distinct_ordered_phase() {
        assert_eq!(
            analyze_phases(&transcript(), "candidate"),
            PhaseEvidence {
                genesis: true,
                accepted: true,
                persisted: true,
                clean_stop: true,
            }
        );
        let missing_restart = transcript().split(PHASES[2]).next().unwrap().to_string();
        assert_eq!(
            analyze_phases(&missing_restart, "candidate"),
            PhaseEvidence::default()
        );
        let reordered = transcript().replace(PHASES[1], PHASES[2]);
        assert_eq!(
            analyze_phases(&reordered, "candidate"),
            PhaseEvidence::default()
        );
        for invalid in [
            "candidate-extra height=1",
            "candidate height=11",
            "different height=1",
        ] {
            let modified = transcript().replace(
                "hashBestChain=candidate height=1",
                &format!("hashBestChain={invalid}"),
            );
            assert!(!analyze_phases(&modified, "candidate").persisted);
        }
    }

    #[test]
    fn each_phase_requires_its_own_clean_stop() {
        let modified = transcript().replacen("T Shutdown done\n", "", 1);
        assert!(!analyze_phases(&modified, "candidate").clean_stop);
        let modified =
            transcript().replacen("T Shutdown done\n", "T Shutdown done\nT Shutdown done\n", 1);
        assert!(!analyze_phases(&modified, "candidate").clean_stop);
    }
}

fn analyze_phases(stdout: &str, expected_hash: &str) -> PhaseEvidence {
    let mut sections: [Vec<&str>; 3] = Default::default();
    let mut phase_count = 0;
    for line in stdout.lines() {
        if line.starts_with("md-sim Core phase:") {
            if PHASES.get(phase_count).copied() != Some(line) {
                return PhaseEvidence::default();
            }
            phase_count += 1;
        } else if phase_count > 0 {
            sections[phase_count - 1].push(line);
        }
    }
    if phase_count != PHASES.len() {
        return PhaseEvidence::default();
    }
    let has_tip = |section: usize, prefix: &str, hash: &str, height: &str| {
        sections[section].iter().any(|line| {
            let Some((_, fields)) = line.split_once(prefix) else {
                return false;
            };
            let mut fields = fields.split_whitespace();
            fields.next() == Some(hash) && fields.any(|field| field == height)
        })
    };
    PhaseEvidence {
        genesis: has_tip(0, "UpdateTip: new best=", GENESIS, "height=0")
            && has_tip(1, "Loaded best chain: hashBestChain=", GENESIS, "height=0"),
        accepted: has_tip(1, "UpdateTip: new best=", expected_hash, "height=1"),
        persisted: has_tip(
            2,
            "Loaded best chain: hashBestChain=",
            expected_hash,
            "height=1",
        ),
        clean_stop: sections.iter().all(|section| {
            section
                .iter()
                .filter(|line| line.ends_with(" Stopping after block import"))
                .count()
                == 1
                && section
                    .iter()
                    .filter(|line| line.ends_with(" Shutdown done"))
                    .count()
                    == 1
        }),
    }
}

fn validate_block(record: &Value) -> Result<(Vec<u8>, String)> {
    let field = |name: &str| {
        record[name]
            .as_str()
            .with_context(|| format!("missing block field {name}"))
    };
    let imported = hex::decode(field("import_hex")?)?;
    let block = hex::decode(field("block_hex")?)?;
    let header = hex::decode(field("header_hex")?)?;
    let coinbase = hex::decode(field("coinbase_hex")?)?;
    ensure!(
        imported.len() >= 89 && imported.len() <= 4 * 1024 * 1024,
        "block import size invalid"
    );
    ensure!(
        imported[..4] == [0xfa, 0xbf, 0xb5, 0xda],
        "wrong network magic"
    );
    ensure!(
        u32::from_le_bytes(imported[4..8].try_into()?) as usize == imported.len() - 8
            && imported[8..] == block,
        "block frame length/content differs"
    );
    ensure!(
        header.len() == 80 && block[..80] == header && block[80] == 1 && block[81..] == coinbase,
        "one-coinbase block differs from recorded header/transaction"
    );
    let merkle = Sha256::digest(Sha256::digest(&coinbase));
    ensure!(header[36..68] == merkle[..], "coinbase merkle root differs");
    let mut digest = Sha256::digest(Sha256::digest(&header)).to_vec();
    digest.reverse();
    let expected_hash = hex::encode(digest);
    ensure!(
        expected_hash == field("block_hash_display")?,
        "block header hash differs"
    );
    Ok((imported, expected_hash))
}

fn candidate_records(evidence: &Value) -> Result<&Vec<Value>> {
    if let Some(records) = evidence.as_array() {
        return Ok(records);
    }
    ensure!(
        evidence["result"]["kind"] == "independent-original-firmware-submission-correlation/1",
        "unknown block evidence kind"
    );
    evidence["result"]["regtest_blocks"]
        .as_array()
        .context("original firmware evidence lacks verified blocks")
}

fn link_original_run(
    input: &Path,
    evidence: &Value,
    record: &Value,
    out: &Path,
    mode: EvidenceMode,
) -> Result<()> {
    let source = input
        .parent()
        .context("original run directory absent")?
        .canonicalize()?;
    let auditor = std::env::current_exe()?
        .parent()
        .context("probe directory absent")?
        .join("s9_asic_audit");
    let auditor_id = hash_file(&auditor)?;
    let replay_path = out.join("original-submission-replay.json");
    let session_id = evidence["result"]["selected_session_id"]
        .as_u64()
        .context("original evidence lacks selected pool session")?;
    // The fresh auditor also checks the exact limit in the frozen pool input.
    ensure!(session_id < 128, "original pool session ID exceeds bound");
    // This is our own Rust evidence tool. It only reads bounded run files and
    // writes a new report; it neither launches firmware nor searches nonces.
    let status = Command::new(&auditor)
        .arg(replay_command(evidence, mode)?)
        .arg(&source)
        .arg(session_id.to_string())
        .arg(&replay_path)
        .stdin(Stdio::null())
        .status()?;
    ensure!(
        status.success(),
        "original firmware hardware/submission replay failed"
    );
    ensure!(
        hash_file(&auditor)? == auditor_id,
        "auditor changed during replay"
    );
    let replay = block_evidence(&replay_path)?;
    ensure!(
        replay["audit_binary"] == serde_json::to_value(&auditor_id)?
            && replay["result"] == evidence["result"],
        "original evidence differs from fresh replay"
    );
    ensure!(
        candidate_records(&replay)?.contains(record),
        "candidate absent from original replay"
    );
    let target = out.join("source-evidence");
    fs::create_dir(&target)?;
    let mut identities = BTreeMap::new();
    for name in [
        "process-spec.json",
        "adaptation.json",
        "pool-input.json",
        "pool-result.json",
        "result.json",
        "backend.log",
        "guest.log",
    ] {
        let path = source.join(name);
        let id = hash_file(&path)?;
        ensure!(
            id.size <= md_sim_runtime::isolation::MAX_TRACE_BYTES * 2 + 1024 * 1024,
            "original source evidence exceeds bound"
        );
        let copy = target.join(name);
        copy_bound(
            &path,
            &copy,
            &id,
            md_sim_runtime::isolation::MAX_TRACE_BYTES * 2 + 1024 * 1024,
        )?;
        identities.insert(name, id);
    }
    for (field, name) in [
        ("execution_result", "result.json"),
        ("process_specification", "process-spec.json"),
        ("adaptation", "adaptation.json"),
        ("backend_log", "backend.log"),
        ("pool_result", "pool-result.json"),
        ("pool_input", "pool-input.json"),
    ] {
        ensure!(
            replay["result"][field] == serde_json::to_value(&identities[name])?,
            "original source changed after replay"
        );
    }
    let external = if mode == EvidenceMode::Shared {
        Some(retain_shared_link(
            &source,
            &target,
            &replay["result"]["external_pool"],
        )?)
    } else {
        None
    };
    let mut link = json!({
        "source_run":source,"files":identities,"source_submission_index":record["source_submission_index"],
        "selected_session_id":session_id,
        "submitted_header_hardware_replay":hash_file(&replay_path)?,"auditor":auditor_id,
        "independent_header_and_coinbase_reconstruction":true,"matching_firmware_submit_trace":true,
        "claim":"Original archived firmware in an explicitly adapted S9 reference device environment. Exact physical board and full-target gates remain open."
    });
    if let Some(external) = external {
        link["external_pool"] = external;
    }
    save_json(&out.join("source-run-link.json"), &link)?;
    Ok(())
}

/// The fresh shared auditor validates the complete source tree, including the
/// outer result and provider. Retain its exact bindings and small raw records.
/// The large outer result stays at its bound source path instead of being
/// copied again into each independent Core check.
fn retain_shared_link(source: &Path, target: &Path, join: &Value) -> Result<Value> {
    let mut files = BTreeMap::new();
    for (field, relative) in shared_files() {
        let expected: ObjectId = serde_json::from_value(join[field].clone())?;
        let path = source.join(relative);
        let copy = target.join(relative);
        fs::create_dir_all(copy.parent().context("shared copy parent absent")?)?;
        copy_bound(&path, &copy, &expected, BLOCK_EVIDENCE_BYTES)?;
        files.insert(relative, expected);
    }
    let outer = source.parent().context("outer execution absent")?;
    for (path, expected) in [
        (outer.join("result.json"), &join["outer_result"]["result"]),
        (
            outer.join("process-spec.json"),
            &join["outer_result"]["process"],
        ),
        (
            source.join("shared-pool/outer-result.json"),
            &join["outer_result"]["record"],
        ),
    ] {
        let expected: ObjectId = serde_json::from_value(expected.clone())?;
        expected.validate()?;
        ensure!(
            hash_file(&path)? == expected,
            "outer source changed after replay"
        );
    }
    Ok(json!({
        "schema_version":"md-sim-core-shared-pool-link/1", "join":join, "copied_files":files,
        "outer_result_path":outer.join("result.json"),
        "scope":"Fresh shared auditor verified the exact worker, carrier, pool, support and outer result before import. Source paths and identities remain required for a complete replay. Core does not generate a candidate."
    }))
}

fn shared_files() -> [(&'static str, &'static str); 7] {
    [
        ("bundle", "shared-pool/pool-bundle.json"),
        ("receipt", "shared-pool/retention-receipt.json"),
        ("support_proof", "shared-pool/support-proof.json"),
        ("provider_receipt", "shared-pool/provider-receipt.json"),
        ("pool_stdout", "shared-pool/pool.stdout"),
        ("pool_stderr", "shared-pool/pool.stderr"),
        ("manifest", "shared-pool.json"),
    ]
}

fn link_source_run(input: &Path, record: &Value, out: &Path) -> Result<()> {
    let source = input.parent().context("source run directory missing")?;
    let read = |name: &str| -> Result<Vec<u8>> {
        let path = source.join(name);
        ensure!(
            fs::metadata(&path)?.len() <= 64 * 1024 * 1024,
            "source evidence too large"
        );
        Ok(fs::read(path)?)
    };
    let checksums: Value = serde_json::from_slice(&read("evidence-checksums.json")?)?;
    let mut identities = BTreeMap::new();
    for name in [
        "pool-report.json",
        "run-spec.json",
        "execution-identity.json",
        "execution.json",
        "input-manifest.json",
        "run-inputs.json",
        "session.json",
        "guest.log",
        "pool-transactions.jsonl",
        "device-transactions.jsonl",
        "regtest-blocks.json",
    ] {
        let bytes = read(name)?;
        let object = ObjectId {
            sha256: hex::encode(Sha256::digest(&bytes)),
            size: bytes.len() as u64,
        };
        ensure!(
            checksums[name] == serde_json::to_value(&object)?,
            "source evidence checksum differs for {name}"
        );
        identities.insert(name, object);
        fs::create_dir_all(out.join("source-evidence"))?;
        fs::write(out.join("source-evidence").join(name), bytes)?;
    }
    let verified = |name: &str| fs::read(out.join("source-evidence").join(name));
    let report: md_sim_mining::stratum::PoolReport =
        serde_json::from_slice(&verified("pool-report.json")?)?;
    ensure!(report.completed, "firmware fresh-work suite is incomplete");
    let reconstructed =
        md_sim_mining::regtest::regtest_blocks(&report).map_err(anyhow::Error::msg)?;
    ensure!(
        reconstructed
            .iter()
            .any(|block| serde_json::to_value(block).ok().as_ref() == Some(record)),
        "candidate differs from independently reconstructed source submission"
    );
    let session: Value = serde_json::from_slice(&verified("session.json")?)?;
    ensure!(
        session["execution_mode"] == "source-adapter"
            && session["configured"] == true
            && session["computed_hashes"].as_u64().is_some_and(|n| n > 0),
        "source session lacks actual execution evidence"
    );
    let execution: Value = serde_json::from_slice(&verified("execution.json")?)?;
    ensure!(
        execution["status"] == "exited"
            && execution["exit_code"] == 0
            && execution["trace_truncated"] == false,
        "firmware execution did not complete"
    );
    let identity: Value = serde_json::from_slice(&verified("execution-identity.json")?)?;
    ensure!(
        identity["run_spec_file_sha256"] == identities["run-spec.json"].sha256,
        "source run identity differs"
    );
    let share: md_sim_mining::Share = serde_json::from_value(record["share"].clone())?;
    let trace = String::from_utf8(verified("pool-transactions.jsonl")?)?;
    let mut matched = false;
    for line in trace.lines() {
        let event: Value = serde_json::from_str(line)?;
        if event["direction"] != "firmware-to-pool" || event["message"]["method"] != "mining.submit"
        {
            continue;
        }
        let parameters = event["message"]["params"]
            .as_array()
            .context("invalid submission trace")?;
        if parameters.len() == 5
            && parameters[1] == share.job_id
            && parameters[2] == hex::encode(&share.extra_nonce2)
            && parameters[3]
                .as_str()
                .and_then(|s| u32::from_str_radix(s, 16).ok())
                == Some(share.time)
            && parameters[4]
                .as_str()
                .and_then(|s| u32::from_str_radix(s, 16).ok())
                == Some(share.nonce)
        {
            matched = true;
        }
    }
    ensure!(matched, "no exact firmware submission in the Stratum trace");
    save_json(
        &out.join("source-run-link.json"),
        &json!({"source_run":source.canonicalize()?,"files":identities,"source_submission_index":record["source_submission_index"],"independent_header_and_coinbase_reconstruction":true,"matching_firmware_submit_trace":true,"claim":"source-adapted fixture, not an exact original board"}),
    )?;
    Ok(())
}

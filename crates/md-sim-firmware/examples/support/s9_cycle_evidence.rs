//! Evidence checks shared by native firmware restart suites.
use anyhow::{ensure, Context, Result};
use md_sim_firmware::serial_console::{GuestInterruption, GuestRestart};
use md_sim_runtime::{
    isolation::save_json,
    store::{hash_file, ObjectId},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn read_json(path: &Path, bound: u64) -> Result<Value> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(bound + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= bound,
        "JSON evidence bound: {}",
        path.display()
    );
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn command(program: &Path, argv: &[OsString], log: &Path) -> Result<()> {
    save_json(
        &log.with_extension("command.json"),
        &json!({"program":hash_file(program)?,"argv":argv}),
    )?;
    let status = Command::new(program)
        .args(argv)
        .stdin(Stdio::null())
        .stdout(fs::File::create_new(log.with_extension("stdout"))?)
        .stderr(fs::File::create_new(log.with_extension("stderr"))?)
        .status()?;
    ensure!(
        status.success(),
        "command failed: {}; see {}.stderr",
        program.display(),
        log.display()
    );
    Ok(())
}

pub fn boot_evidence(
    run: &Path,
    previous: Option<&Path>,
    checkpoint_required: bool,
    interrupted: bool,
) -> Result<String> {
    ensure!(
        !(checkpoint_required && interrupted),
        "checkpoint cannot precede controlled interruption"
    );
    let result = read_json(
        &run.join("result.json"),
        2 * md_sim_runtime::isolation::MAX_TRACE_BYTES + 1024 * 1024,
    )?;
    ensure!(
        result["status"] == "exited"
            && result["exit_code"] == 0
            && result["trace_truncated"] == false,
        "firmware execution failed or trace truncated"
    );
    let snapshot = read_json(&run.join("sd-snapshot-0.json"), 16384)?;
    ensure!(
        snapshot["schema"]
            == if interrupted {
                "md-sim-original-boot-sd/4"
            } else {
                "md-sim-original-boot-sd/3"
            }
            && snapshot["guest_sync_completed"] == false,
        "cycle requires a native restart snapshot"
    );
    let console = read_json(&run.join("console.json"), 16384)?;
    ensure!(
        console["saved_sd_input"] == serde_json::to_value(previous.map(hash_file).transpose()?)?,
        "restart used a different SD state"
    );
    let stdout = hex::decode(result["stdout"].as_str().context("guest output absent")?)?;
    ensure!(
        stdout == fs::read(run.join("guest.log"))?,
        "retained guest output differs"
    );
    let guest = std::str::from_utf8(&stdout)?;
    if checkpoint_required {
        ensure!(
            console["sd_read_only_checkpoint"].is_object()
                && single_record(guest, "MD_SIM_NATIVE_SD_CHECKPOINT ", 4096)?
                    == console["sd_read_only_checkpoint"],
            "native read-only storage checkpoint absent or changed"
        );
    } else {
        ensure!(
            console["sd_read_only_checkpoint"].is_null(),
            "unexpected checkpoint before native installer restart"
        );
    }
    let restart = single_record(
        guest,
        if interrupted {
            "MD_SIM_GUEST_INTERRUPTION "
        } else {
            "MD_SIM_GUEST_RESTART "
        },
        2 * 1024 * 1024 + 4096,
    )?;
    let serial = hex::decode(
        restart["serial_hex"]
            .as_str()
            .context("restart serial absent")?,
    )?;
    if interrupted {
        let observation: GuestInterruption =
            serde_json::from_value(restart["observation"].clone())?;
        observation.verify_serial(&serial)?;
        ensure!(
            snapshot["guest_interruption"] == restart["observation"]
                && console["sd_controlled_interruption"]["signal"] == "SIGKILL"
                && !guest.contains("MD_SIM_GUEST_RESTART "),
            "snapshot interruption differs from actual output"
        );
    } else {
        let observation: GuestRestart = serde_json::from_value(restart["observation"].clone())?;
        observation.verify_serial(&serial)?;
        ensure!(
            snapshot["guest_restart"] == restart["observation"],
            "snapshot restart differs from actual output"
        );
    }
    for marker in [
        "U-Boot SPL 2016.03-g8728fee00a",
        "U-Boot 2016.03-gf30f5b605c",
        "Linux version 4.4.0-xilinx-g518c00cd977f-dirty",
    ] {
        ensure!(
            guest.matches(marker).count() == 1,
            "original boot stage absent or repeated: {marker}"
        );
    }
    Ok(guest.to_owned())
}

pub fn single_record(guest: &str, marker: &str, bound: usize) -> Result<Value> {
    let mut records = guest.lines().filter_map(|line| line.strip_prefix(marker));
    let text = records.next().context("required guest record absent")?;
    ensure!(
        text.len() <= bound && records.next().is_none(),
        "guest record bound or ambiguity"
    );
    Ok(serde_json::from_str(text)?)
}

pub fn verify_inputs(inputs: &BTreeMap<PathBuf, ObjectId>) -> Result<()> {
    for (path, object) in inputs {
        ensure!(
            hash_file(path)? == *object,
            "cycle input changed: {}",
            path.display()
        );
    }
    Ok(())
}

pub fn inspect_disk(
    bin: &Path,
    run: &Path,
    out: &Path,
    label: &str,
    replay_journal: bool,
    observe_recovery: bool,
) -> Result<Value> {
    ensure!(
        !observe_recovery || replay_journal,
        "recovery observation requires journal replay on the inspection copy"
    );
    let inspection = out.join(format!("disk-{label}"));
    let mut args = vec![
        run.join("sd-snapshot-0.json").into_os_string(),
        inspection.clone().into_os_string(),
    ];
    if observe_recovery {
        args.push("observe-recovery".into());
    } else if replay_journal {
        args.push("replay-journal".into());
    }
    command(
        &bin.join("s9_sd_snapshot_inspect"),
        &args,
        &out.join(format!("inspect-{label}")),
    )?;
    let report_path = inspection.join("inspection.json");
    let report = read_json(&report_path, 4 * 1024 * 1024)?;
    ensure!(
        report["snapshot"] == serde_json::to_value(hash_file(&run.join("sd-snapshot-0.json"))?)?,
        "disk inspection used a different snapshot"
    );
    ensure!(
        report["inspection"]["journal_replay_requested"] == replay_journal,
        "disk inspection used a different recovery policy"
    );
    ensure!(
        (report["inspection"]["recovery_observation_only"] == true) == observe_recovery,
        "inspection observation mode differs"
    );
    Ok(
        json!({"report":hash_file(&report_path)?,"partition":report["inspection"]["partition"],
        "raw_partition":report["inspection"]["raw_partition"],"journal_replay_on_inspection_copy":replay_journal,
        "pre_recovery_observation_only":observe_recovery,"read_only_full_check":report["inspection"]["read_only_full_check"]}),
    )
}

pub fn verify_mining(
    bin: &Path,
    run: &Path,
    out: &Path,
    label: &str,
    core: &Path,
) -> Result<Value> {
    let rate = run.join("rate-audit.json");
    command(
        &bin.join("s9_asic_audit"),
        &[
            "rates".into(),
            run.as_os_str().into(),
            rate.as_os_str().into(),
        ],
        &out.join(format!("rate-{label}")),
    )?;
    let audit = run.join("regtest-audit.json");
    command(
        &bin.join("s9_asic_audit"),
        &[
            "regtest".into(),
            run.as_os_str().into(),
            audit.as_os_str().into(),
        ],
        &out.join(format!("audit-{label}")),
    )?;
    let mut nodes = Vec::new();
    for candidate in 0..2 {
        let node = out.join(format!("core-{label}-{candidate}"));
        command(
            &bin.join("node_prerequisite_probe"),
            &[
                core.as_os_str().into(),
                node.as_os_str().into(),
                audit.as_os_str().into(),
                candidate.to_string().into(),
            ],
            &out.join(format!("node-{label}-{candidate}")),
        )?;
        let result_path = node.join("block-acceptance.json");
        let node_result = read_json(&result_path, 4 * 1024 * 1024)?;
        ensure!(
            node_result["accepted"] == true
                && node_result["persisted_after_restart"] == true
                && node_result["observed_height_before"] == 0
                && node_result["observed_height_after"] == 1
                && node_result["observed_height_after_restart"] == 1,
            "Core did not accept and retain the firmware block"
        );
        nodes.push(json!({"result":hash_file(&result_path)?,"block_hash":node_result["expected_block_hash"]}));
    }
    Ok(json!({"rate_audit":hash_file(&rate)?,"audit":hash_file(&audit)?,"core":nodes}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recorded_output_must_be_unique_bounded_and_valid() {
        assert_eq!(
            single_record("guest text\nMARK {\"passed\":true}\n", "MARK ", 32).unwrap(),
            json!({"passed":true})
        );
        for log in [
            "",
            "MARK not-json\n",
            "MARK {}\nMARK {}\n",
            "other MARK {}\n",
        ] {
            assert!(single_record(log, "MARK ", 32).is_err());
        }
        assert!(single_record("MARK {\"large\":true}", "MARK ", 2).is_err());
    }
    #[test]
    fn changed_frozen_input_stops_a_cycle() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("input");
        fs::write(&path, b"captured input").unwrap();
        let inputs = BTreeMap::from([(path.clone(), hash_file(&path).unwrap())]);
        verify_inputs(&inputs).unwrap();
        fs::write(&path, b"changed input").unwrap();
        assert!(verify_inputs(&inputs).is_err());
        fs::remove_file(&path).unwrap();
        assert!(verify_inputs(&inputs).is_err());
    }
}

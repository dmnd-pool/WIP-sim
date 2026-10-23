//! Three actual archived firmware boots with paired persistent media.
//! Each boot requires independent fresh-work verification and two Core nodes.
use anyhow::{ensure, Context, Result};
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
};

#[path = "support/s9_cycle_console.rs"]
mod console;
#[path = "support/s9_cycle_evidence.rs"]
mod evidence;
#[path = "support/s9_nand_scenario.rs"]
mod scenario;
#[allow(dead_code)]
#[path = "support/s9_boot_storage.rs"]
mod storage;

struct Inputs<'a> {
    bundle: &'a Path,
    backend: &'a Path,
    pool: &'a Path,
    core: &'a Path,
    monitor: &'a Path,
}

fn cycle(input: Inputs<'_>, out: &Path, interrupted: bool) -> Result<Value> {
    let bin = std::env::current_exe()?
        .parent()
        .context("driver directory")?
        .to_owned();
    let mut frozen = BTreeMap::<PathBuf, ObjectId>::new();
    for name in [
        "s9_boot_bundle",
        "s9_pool_guest",
        "isolated_network_vm",
        "s9_asic_audit",
        "node_prerequisite_probe",
        "s9_sd_snapshot_inspect",
    ] {
        let path = bin.join(name);
        frozen.insert(path.clone(), hash_file(&path)?);
    }
    for path in [
        input.bundle.join("boot/bundle.json"),
        input.backend.join("build-envelope.json"),
        input.backend.join("build-report.json"),
        input.backend.join("qemu-system-arm"),
        input.pool.join("build-envelope.json"),
        input.pool.join("build-report.json"),
        input.pool.join("qemu-system-x86_64"),
        input.core.to_owned(),
        input.monitor.to_owned(),
        PathBuf::from(console::CONSOLE),
    ] {
        frozen.insert(path.clone(), hash_file(&path)?);
    }
    let mut random = [0u8; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut random)?;
    let token = hex::encode(random);
    let data = scenario::pattern(&token)?;
    fs::write(out.join("expected-data.bin"), &data)?;
    frozen.insert(
        out.join("expected-data.bin"),
        hash_file(&out.join("expected-data.bin"))?,
    );
    let original = fs::read_to_string(console::CONSOLE)?;
    let resumed = console::persisted_pool_console(&original)?;
    let mut scripts = Vec::new();
    for (index, label) in scenario::PHASES.iter().enumerate() {
        // The outer errexit is required: a failing first subshell must not be
        // hidden by successful storage commands in the following subshell.
        let script = format!(
            "(\nset -eu\n{}\n{}\n)\n",
            if index == 0 { &original } else { &resumed },
            if interrupted && index == 0 {
                scenario::storage_commands_with_sync(index, &token, false)?
            } else {
                scenario::storage_commands(index, &token)?
            }
        );
        let path = out.join(format!("console-{label}.sh"));
        fs::write(&path, script)?;
        frozen.insert(path.clone(), hash_file(&path)?);
        scripts.push(path);
    }
    save_json(
        &out.join("driver-input.json"),
        &json!({
        "schema":"md-sim-native-nand-cycle/2", "driver":hash_file(&std::env::current_exe()?)?,
        "controlled_interruption_after_write":interrupted,
        "frozen_inputs":frozen, "pattern":hash_file(&out.join("expected-data.bin"))?,
        "pattern_bytes":scenario::PATTERN_BYTES,"hash_cap_hps":1000,
        "scope":"Three original firmware boots on the adapted S9 reference board. Native tools write fresh data across three NAND pages, compare it after restart, erase it, and verify erased data after another restart. Each boot uses new mining jobs and independent Core nodes. Not an exact physical target or full NAND conformance pass.",
        "adaptations":["Explicit serial console commands through original BusyBox tools; no new guest executable.",
            "Fan, power, temperature, filter and monitor adaptations are retained per boot.",
            "Each boot explicitly configures the private network; network persistence is not claimed.",
            if interrupted {"The first boot ends with intentional emulator SIGKILL and no final guest sync. The next boot uses its paired media. Later boots use a native read-only SD checkpoint and normal restart. No interrupted-write atomicity or physical power-loss claim."} else {"A native read-only SD checkpoint precedes normal restart. No power-loss recovery claim."},
            "Generated erased NAND backing and compressed private overlays; no physical factory dump."]}),
    )?;
    let mut previous: Option<PathBuf> = None;
    let mut phases = Vec::new();
    for (index, label) in scenario::PHASES.iter().enumerate() {
        evidence::verify_inputs(&frozen)?;
        let run = out.join(format!("run-{index}"));
        let mut argv: Vec<OsString> = vec![
            "boot".into(),
            input.bundle.as_os_str().into(),
            run.as_os_str().into(),
            "spl".into(),
            "sd2022-v1".into(),
            format!("backend={}", input.backend.display()).into(),
            "budget=360".into(),
            "batch-console".into(),
            "save-sd".into(),
            "nand-storage".into(),
            "presence-pin-fixture=e0".into(),
            "fan-fixture".into(),
            "pic-contract".into(),
            "asic-contract".into(),
            "asic-packets".into(),
            "local-pool".into(),
            format!("pool-vm={}", input.pool.display()).into(),
            "temperature-fixture".into(),
            "adapt-fixed-filter-bits=4".into(),
            "hash-cap-hps=1000".into(),
            "pool-difficulty=-28,-27".into(),
            "regtest-genesis".into(),
            format!("adapt-monitor-fan-source={}", input.monitor.display()).into(),
            format!("console={}", scripts[index].display()).into(),
        ];
        let interruption = interrupted && index == 0;
        if interruption {
            argv.push("sd-controlled-interruption".into());
        } else {
            argv.extend(["sd-native-reboot".into(), "sd-read-only-checkpoint".into()]);
        }
        if let Some(parent) = &previous {
            argv.extend([
                format!("sd-state={}", parent.join("sd-snapshot-0.json").display()).into(),
                format!(
                    "nand-state={}",
                    parent.join("nand-snapshot-0.json").display()
                )
                .into(),
            ]);
        }
        evidence::command(
            &bin.join("s9_boot_bundle"),
            &argv,
            &out.join(format!("boot-{index}")),
        )?;
        let previous_sd = previous.as_ref().map(|p| p.join("sd-snapshot-0.json"));
        let guest =
            evidence::boot_evidence(&run, previous_sd.as_deref(), !interruption, interruption)?;
        scenario::verify_native(&guest, index)?;
        let console_record = evidence::read_json(&run.join("console.json"), 65536)?;
        let previous_nand = previous.as_ref().map(|p| p.join("nand-snapshot-0.json"));
        ensure!(
            console_record["nand_storage"]["saved_input"]
                == serde_json::to_value(previous_nand.as_deref().map(hash_file).transpose()?)?,
            "restart used a different NAND state"
        );
        let sd_path = run.join("sd-snapshot-0.json");
        let nand_path = run.join("nand-snapshot-0.json");
        let sd = serde_json::from_value(evidence::read_json(&sd_path, 16384)?)?;
        let nand = serde_json::from_value(evidence::read_json(&nand_path, 16384)?)?;
        let pair = storage::verify_pair(&sd_path, &sd, &nand_path, &nand)?;
        let disk = evidence::inspect_disk(&bin, &run, out, label, interruption, interruption)?;
        let mining = evidence::verify_mining(&bin, &run, out, label, input.core)?;
        evidence::verify_inputs(&frozen)?;
        // Seal the parent bytes that the next boot will consume. Its loader
        // independently checks both exports against these execution records.
        for name in [
            "sd-snapshot-0.json",
            "nand-snapshot-0.json",
            "sd-snapshot-0.qcow2.gz",
            "nand-snapshot-0.qcow2.gz",
            "sd-storage.json",
            "nand-storage.json",
            "result.json",
            "process-spec.json",
        ] {
            let path = run.join(name);
            frozen.insert(path.clone(), hash_file(&path)?);
        }
        phases.push(json!({"phase":label,"controlled_interruption":interruption,"process_specification":hash_file(&run.join("process-spec.json"))?,
            "execution_result":hash_file(&run.join("result.json"))?,"sd":hash_file(&sd_path)?,"nand":hash_file(&nand_path)?,
            "paired_parent":pair,"disk_inspection":disk,"mining":mining}));
        save_json(
            &out.join("progress.json"),
            &json!({"completed_phases":phases,"required_phases":3}),
        )?;
        previous = Some(run);
    }
    evidence::verify_inputs(&frozen)?;
    save_json(&out.join("completed-inputs.json"), &frozen)?;
    Ok(
        json!({"phases":phases,"controlled_interruption_recovery_tested":interrupted,"original_board_gate":"incomplete","full_nand_gate":"incomplete",
        "limits":["Generated reference geometry; exact physical board remains unresolved.","Only the declared three-page native storage cycle was tested.","Full media, bad blocks, interrupted writes and power-loss recovery need separate tests."]}),
    )
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 7 || (args.len() == 8 && args[7] == "controlled-interruption"),
        "usage: s9_nand_cycle BUNDLE BACKEND POOL_BACKEND CORE ORIGINAL_MONITOR NEW_OUTPUT [controlled-interruption]"
    );
    let out = Path::new(&args[6]);
    ensure!(!out.exists(), "cycle output must be new");
    fs::create_dir_all(out)?;
    let binary = std::env::current_exe()?;
    fs::copy(&binary, out.join("driver-binary"))?;
    ensure!(
        hash_file(&binary)? == hash_file(&out.join("driver-binary"))?,
        "driver capture changed"
    );
    let result = cycle(
        Inputs {
            bundle: Path::new(&args[1]),
            backend: Path::new(&args[2]),
            pool: Path::new(&args[3]),
            core: Path::new(&args[4]),
            monitor: Path::new(&args[5]),
        },
        out,
        args.len() == 8,
    );
    save_json(
        &out.join("assessment.json"),
        &json!({"passed":result.is_ok(),"result":result.as_ref().ok(),"error":result.as_ref().err().map(|e|format!("{e:#}"))}),
    )?;
    result.map(|_| ())
}

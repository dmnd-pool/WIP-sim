//! Run the archived native watchdog API and verify a real IRQ fault/recovery.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{save_json, ProcessResult},
    store::hash_file,
};
use serde_json::json;
use std::{
    fs::{self, File},
    path::Path,
    process::{Command, Stdio},
};
#[path = "support/s9_watchdog_scenario.rs"]
mod scenario;

fn bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    ensure!(
        fs::metadata(path)?.len() <= limit,
        "evidence exceeds limit: {}",
        path.display()
    );
    Ok(fs::read(path)?)
}
fn run(bundle: &Path, backend: &Path, baseline: &Path, out: &Path) -> Result<serde_json::Value> {
    let executable = std::env::current_exe()?;
    let boot = executable
        .parent()
        .context("executable parent absent")?
        .join("s9_boot_bundle");
    let console = out.join("console.sh");
    fs::write(&console, scenario::CONSOLE)?;
    let run = out.join("run");
    let arguments = vec![
        "boot".to_owned(),
        bundle.canonicalize()?.display().to_string(),
        run.display().to_string(),
        "spl".into(),
        "sd2022-v1".into(),
        format!("backend={}", backend.canonicalize()?.display()),
        "budget=240".into(),
        "batch-console".into(),
        // Fixed instruction timing gives the short IRQ pulse a bounded
        // virtual-time observation. This is not physical clock calibration.
        "icount-shift=2".into(),
        format!("console={}", console.display()),
        format!("sd-state={}", baseline.canonicalize()?.display()),
        "presence-pin-fixture=e0".into(),
        "fan-fixture".into(),
        "pic-contract".into(),
        "asic-contract".into(),
        "asic-packets".into(),
        "temperature-fixture".into(),
        "hash-cap-hps=1000".into(),
    ];
    save_json(
        &out.join("driver-input.json"),
        &json!({"schema":"md-sim-s9-native-watchdog/1","driver":hash_file(&executable)?,"boot_runner":hash_file(&boot)?,"backend_report":hash_file(&backend.join("build-report.json"))?,"backend":hash_file(&backend.join("qemu-system-arm"))?,"baseline":hash_file(baseline)?,"console":hash_file(&console)?,"argv":arguments,"scope":"A watchdog fault and recovery test. Archived firmware executes in the existing reference board. No NIC, pool, exact-board or mining acceptance is claimed."}),
    )?;
    let status = Command::new(boot)
        .args(&arguments)
        .stdin(Stdio::null())
        .stdout(File::create(out.join("boot.stdout"))?)
        .stderr(File::create(out.join("boot.stderr"))?)
        .status()?;
    ensure!(status.success(), "board test runner exited {status}");
    let result: ProcessResult =
        serde_json::from_slice(&bounded(&run.join("result.json"), 256 << 20)?)?;
    ensure!(
        result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
        "watchdog firmware execution incomplete"
    );
    let guest = String::from_utf8(bounded(&run.join("guest.log"), 128 << 20)?)?;
    let trace = String::from_utf8(bounded(&run.join("backend.log"), 64 << 20)?)?;
    let evidence = scenario::assess(&guest, &trace)?;
    Ok(
        json!({"evidence":evidence,"driver_input":hash_file(&out.join("driver-input.json"))?,"process_specification":hash_file(&run.join("process-spec.json"))?,"adaptation":hash_file(&run.join("adaptation.json"))?,"execution_result":hash_file(&run.join("result.json"))?,"guest_log":hash_file(&run.join("guest.log"))?,"device_transactions":hash_file(&run.join("backend.log"))?,"original_board_gate":"incomplete","exact_target_gate":"incomplete"}),
    )
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 5,
        "usage: s9_watchdog_check BUNDLE BACKEND BASELINE_SD_MANIFEST NEW_OUTPUT"
    );
    let out = Path::new(&args[4]);
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(out)?;
    let out = out.canonicalize()?;
    save_json(
        &out.join("assessment.json"),
        &json!({"passed":false,"state":"running"}),
    )?;
    match run(
        Path::new(&args[1]),
        Path::new(&args[2]),
        Path::new(&args[3]),
        &out,
    ) {
        Ok(result) => save_json(
            &out.join("assessment.json"),
            &json!({"passed":true,"result":result}),
        ),
        Err(error) => {
            save_json(
                &out.join("assessment.json"),
                &json!({"passed":false,"error":format!("{error:#}"),"original_board_gate":"incomplete"}),
            )?;
            Err(error)
        }
    }
}

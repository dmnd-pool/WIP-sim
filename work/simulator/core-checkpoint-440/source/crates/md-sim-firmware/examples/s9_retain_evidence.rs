//! Regenerate native observations from a locked execution, even if its outer
//! dashboard suite failed. Keep all original evidence and verdicts unchanged.
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::LockedRun;
use md_sim_runtime::{
    isolation::{save_json, ProcessResult, ProcessSpec},
    store::hash_file,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, io::Read, path::Path};
#[allow(dead_code)]
#[path = "support/s9_boot_storage.rs"]
mod boot_storage;
#[allow(dead_code)] // The common pool bundle contract uses this artifact codec.
#[path = "support/worker_channel_artifacts.rs"]
mod channel_artifacts;
#[path = "support/s9_execution_report.rs"]
mod execution_report;
#[allow(dead_code)]
#[path = "support/s9_pool_input.rs"]
mod pool_input;
#[path = "support/s9_pool_report.rs"]
mod pool_report;
#[allow(dead_code)]
#[path = "support/pool_vm.rs"]
mod pool_vm;
#[allow(dead_code)]
#[path = "support/worker_retention_frame.rs"]
mod retention_frame;
#[allow(dead_code)]
#[path = "support/s9_shared_pool.rs"]
mod shared_manifest;
#[allow(dead_code)]
#[path = "support/worker_shared_retention.rs"]
mod shared_retention;

fn read(path: &Path, max: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(max + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= max,
        "evidence size bound: {}",
        path.display()
    );
    Ok(bytes)
}

fn main() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    ensure!(
        args.len() == 3,
        "usage: s9_retain_evidence EXTRACTED_LOCKED_RUN NEW_OUTPUT"
    );
    let source = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    ensure!(!out.exists(), "new evidence output required");
    let run: LockedRun = serde_json::from_slice(&read(&source.join("locked-run.json"), 4 << 20)?)?;
    run.validate()?;
    let spec = run.process.as_ref().context("locked process absent")?;
    let source_spec: ProcessSpec =
        serde_json::from_slice(&read(&source.join("process-spec.json"), 4 << 20)?)?;
    ensure!(
        serde_json::to_value(&source_spec)? == serde_json::to_value(spec)?,
        "process differs from locked run"
    );
    let mut frozen = BTreeMap::new();
    for (name, key) in [
        ("process-spec.json", "process-specification"),
        ("adaptation.json", "evidence:adaptation.json"),
        ("console.json", "evidence:console.json"),
        ("pool-input.json", "evidence:pool-input.json"),
    ] {
        let object = hash_file(&source.join(name))?;
        ensure!(
            run.inputs.get(key) == Some(&object),
            "locked evidence differs: {name}"
        );
        frozen.insert(name.to_owned(), object);
    }
    let identity: Value =
        serde_json::from_slice(&read(&source.join("execution-identity.json"), 16384)?)?;
    ensure!(
        identity["locked_run_hash"] == run.hash()?
            && identity["prepared_process"] == serde_json::to_value(&frozen["process-spec.json"])?,
        "selected execution identity differs"
    );
    let bytes = read(&source.join("result.json"), 257 << 20)?;
    let result: ProcessResult = serde_json::from_slice(&bytes)?;
    ensure!(
        result.tools == run.backend_tools,
        "executed isolation tools differ"
    );
    let exported: Value = serde_json::from_slice(&read(&source.join("export.json"), 16384)?)?;
    ensure!(
        exported["process_result"]
            == serde_json::to_value(hash_file(&source.join("result.json"))?)?,
        "exported process envelope differs"
    );
    for (file, stream) in [
        ("guest.log", &result.stdout),
        ("backend.log", &result.stderr),
    ] {
        let original = read(&source.join(file), 128 << 20)?;
        ensure!(
            hex::decode(stream)? == original,
            "retained stream differs: {file}"
        );
    }
    for name in [
        "locked-run.json",
        "execution-identity.json",
        "result.json",
        "guest.log",
        "backend.log",
        "export.json",
        "selected-dashboard.json",
        "transport-assessment.json",
    ] {
        frozen.insert(name.to_owned(), hash_file(&source.join(name))?);
    }
    if source.join("configuration-reference.bit").exists() {
        frozen.insert(
            "configuration-reference.bit".into(),
            hash_file(&source.join("configuration-reference.bit"))?,
        );
    }
    fs::create_dir_all(out)?;
    for (name, object) in &frozen {
        fs::copy(source.join(name), out.join(name))?;
        ensure!(
            hash_file(&out.join(name))? == *object && hash_file(&source.join(name))? == *object,
            "evidence changed during copy: {name}"
        );
    }
    let retained = execution_report::retain(out, spec, &result);
    for (name, object) in &frozen {
        ensure!(
            hash_file(&source.join(name))? == *object && hash_file(&out.join(name))? == *object,
            "original evidence changed during report retention: {name}"
        );
    }
    save_json(
        &out.join("retention.json"),
        &json!({"schema":"md-sim-derived-execution-observations/1","source":source,"inputs":frozen,"locked_run_hash":run.hash()?,"driver":hash_file(&std::env::current_exe()?)?,"source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_retain_evidence.rs")),"retained_report_ok":retained.is_ok(),"error":retained.as_ref().err().map(|e|format!("{e:#}")),"scope":"Regenerate observations from exact retained process bytes. Original dashboard, transport, execution and target verdicts remain unchanged. Independent mining and Core verification remain required."}),
    )?;
    retained
}

//! Inspect exact archived SPL bytes in the existing preparation namespace.
//! No firmware instructions or external analysis tools execute.
use anyhow::{ensure, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};

#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "support/s9_spl_contract.rs"]
mod contract;

fn inside() -> Result<()> {
    confinement::require(true)?;
    let inspected = (|| {
        ensure!(
            fs::metadata("/boot.bin")?.len() == contract::BOOT_SIZE as u64
                && fs::metadata("/spl-payload.bin")?.len() == contract::SPL_SIZE as u64,
            "captured firmware size differs"
        );
        contract::inspect(&fs::read("/boot.bin")?, &fs::read("/spl-payload.bin")?)
    })();
    let report = json!({"schema":"md-sim-s9-spl-inspection/1","passed":inspected.is_ok(),
        "error":inspected.as_ref().err().map(|e|format!("{e:#}")),"contract":inspected.as_ref().ok(),
        "inspector":hash_file(Path::new("/inspect"))?,
        "source_files":{
            "inspect_s9_spl.rs":contract::object(include_bytes!("inspect_s9_spl.rs")),
            "support/s9_spl_contract.rs":contract::object(include_bytes!("support/s9_spl_contract.rs"))},
        "scope":"Read-only bounded static inspection. No firmware instructions execute. No physical timing, FPGA equivalence, boot, mining or calibration pass."});
    let bytes = serde_json::to_vec_pretty(&report)?;
    ensure!(bytes.len() <= 1 << 20, "inspection output bound");
    use std::io::Write;
    std::io::stdout().write_all(&bytes)?;
    inspected.map(|_| ())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "__inside") {
        ensure!(args.len() == 2, "invalid inner invocation");
        return inside();
    }
    ensure!(
        args.len() == 4,
        "usage: inspect_s9_spl BOOT_BIN SPL_PAYLOAD NEW_OUTPUT"
    );
    let out = PathBuf::from(&args[3]);
    fs::create_dir(&out)?;
    let store = Store::open(&out.join("inputs"), 128 << 20)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![("/inspect".into(), std::env::current_exe()?)],
    )?;
    for (path, guest_path, sha, size) in [
        (
            Path::new(&args[1]),
            "/boot.bin",
            contract::BOOT_SHA,
            contract::BOOT_SIZE,
        ),
        (
            Path::new(&args[2]),
            "/spl-payload.bin",
            contract::SPL_SHA,
            contract::SPL_SIZE,
        ),
    ] {
        let metadata = fs::metadata(path)?;
        ensure!(
            metadata.is_file() && metadata.len() == size as u64,
            "selected firmware size/type differs"
        );
        let object = ObjectId {
            sha256: sha.into(),
            size: size as u64,
        };
        store.import(path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: guest_path.into(),
        });
    }
    let spec = ProcessSpec {
        schema_version: "2.0".into(),
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        argv: vec!["/inspect".into(), "__inside".into()],
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 15,
            cpu_seconds: 10,
            memory_bytes: 256 << 20,
            processes: 16,
            file_bytes: 2 << 20,
            trace_bytes: 1 << 20,
            tmp_bytes: 1 << 20,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    fs::write(out.join("contract.json"), hex::decode(&result.stdout)?)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    ensure!(
        result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
        "isolated SPL inspection failed; actual evidence retained"
    );
    println!("Exact selected SPL contract extracted. No firmware execution or physical calibration claimed.");
    Ok(())
}

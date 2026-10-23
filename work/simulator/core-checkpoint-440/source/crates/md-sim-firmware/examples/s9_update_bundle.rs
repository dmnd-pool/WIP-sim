//! Inspect the exact signed SD update before an installation test.
//! Acquired archive parsing occurs in the preparation namespace. No member runs.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute_with_trace, save_json, Limits, Mount, ProcessSpec, TraceFiles},
    store::{hash_file, Store},
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
use md_sim_firmware::s9_update_validation as update;

fn inspect() -> Result<()> {
    confinement::require(true)?;
    let parent = update::checked_image(Path::new("/update.tar"))?;
    ensure!(
        fs::metadata("/baseline.json")?.len() <= 65536,
        "baseline manifest bound"
    );
    let baseline: Value = serde_json::from_slice(&fs::read("/baseline.json")?)?;
    let names = ["COMMAND", "CONTROL", "fit", "fpga", "spl", "uboot"];
    let mut archive = tar::Archive::new(File::open("/update.tar")?);
    let mut seen = BTreeSet::new();
    let mut members = BTreeMap::new();
    let mut references = BTreeMap::new();
    for entry in archive.entries()? {
        let entry = entry?;
        ensure!(seen.len() < 7, "signed update member count differs");
        let path = entry.path()?.into_owned();
        let name = path.to_str().context("update path encoding")?;
        ensure!(seen.insert(name.to_owned()), "duplicate update member");
        if name.trim_end_matches('/') == "sysupgrade-am1-s9" {
            ensure!(
                entry.header().entry_type().is_dir() && entry.size() == 0,
                "update root differs"
            );
            continue;
        }
        let short = name
            .strip_prefix("sysupgrade-am1-s9/")
            .context("unexpected update root")?;
        ensure!(
            names.contains(&short) && entry.header().entry_type().is_file(),
            "unexpected update member: {name}"
        );
        ensure!(entry.size() <= update::BYTES, "update member size bound");
        let declared_size = entry.size();
        let mut bytes = Vec::new();
        entry.take(update::BYTES + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 == declared_size,
            "update member size differs"
        );
        let object =
            json!({"sha256":md_sim_firmware::prepare::hash_bytes(&bytes),"size":bytes.len()});
        members.insert(short.to_owned(), object);
        if matches!(short, "COMMAND" | "CONTROL") {
            ensure!(bytes.len() <= 32768, "update reference text bound");
            references.insert(short.to_owned(), String::from_utf8(bytes)?);
        }
    }
    ensure!(
        members.len() == names.len() && seen.len() == 7,
        "incomplete signed update"
    );
    let mut comparisons = BTreeMap::new();
    for (member, boot_file) in [
        ("spl", "boot.bin"),
        ("uboot", "u-boot.img"),
        ("fpga", "system.bit.gz"),
        ("fit", "fit.itb"),
    ] {
        ensure!(
            baseline["boot_files"][boot_file].is_object(),
            "baseline boot member absent"
        );
        comparisons.insert(
            member,
            json!({"baseline_file":boot_file,
            "baseline_object":baseline["boot_files"][boot_file],
            "identical":members[member] == baseline["boot_files"][boot_file]}),
        );
    }
    serde_json::to_writer(
        std::io::stdout(),
        &json!({
            "schema":"md-sim-s9-signed-update-inspection/1", "artifact_id":update::ARTIFACT,
            "archive":parent,"baseline_manifest":hash_file(Path::new("/baseline.json"))?,
            "members":members,"reference_text":references,"baseline_comparisons":comparisons,
            "scope":"Static inspection of the exact signed archive and comparison with the supplied baseline manifest. The original firmware must verify signature and format. No member was executed; no installation, recovery or exact-target pass."
        }),
    )?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|v| v == "__inspect") {
        ensure!(args.len() == 2, "unexpected inner arguments");
        return inspect();
    }
    ensure!(
        args.len() == 3,
        "usage: s9_update_bundle BASELINE_BUNDLE_JSON NEW_OUTPUT"
    );
    let baseline = Path::new(&args[1]);
    ensure!(
        fs::metadata(baseline)?.len() <= 65536,
        "baseline manifest bound"
    );
    let image = Path::new(update::ARCHIVE_IMAGE);
    update::checked_image(image)?;
    let out = Path::new(&args[2]);
    ensure!(!out.exists(), "inspection output must be new");
    fs::create_dir_all(out)?;
    let store = Store::open(&out.join("inputs"), 128 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![("/inspect-update".into(), std::env::current_exe()?)],
    )?;
    for (path, guest) in [(image, "/update.tar"), (baseline, "/baseline.json")] {
        let object = hash_file(path)?;
        store.import(path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: guest.into(),
        });
    }
    let spec = ProcessSpec {
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec!["/inspect-update".into(), "__inspect".into()],
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 30,
            cpu_seconds: 20,
            memory_bytes: 256 * 1024 * 1024,
            processes: 16,
            file_bytes: 1024 * 1024,
            trace_bytes: 1024 * 1024,
            tmp_bytes: 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute_with_trace(
        &store,
        &spec,
        Arc::new(AtomicBool::new(false)),
        Some(TraceFiles {
            stdout: File::create(out.join("inspection.json"))?,
            stderr: File::create(out.join("stderr.log"))?,
        }),
    )?;
    save_json(&out.join("result.json"), &result)?;
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated,
        "update inspection failed; retained evidence"
    );
    let report: Value = serde_json::from_slice(&fs::read(out.join("inspection.json"))?)?;
    for name in ["COMMAND", "CONTROL"] {
        let content = report["reference_text"][name]
            .as_str()
            .context("missing reference text")?;
        let path = out.join(name);
        fs::write(&path, content)?;
        ensure!(
            serde_json::to_value(hash_file(&path)?)? == report["members"][name],
            "exported reference identity differs"
        );
    }
    println!(
        "{}",
        json!({"output":out,"archive":report["archive"],"baseline_comparisons":report["baseline_comparisons"]})
    );
    Ok(())
}

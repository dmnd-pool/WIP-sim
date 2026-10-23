//! Derive one recorded payload-bit fault from the exact archived S9 update.
//! Archive parsing and mutation run only in the preparation namespace.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute_with_trace, save_json, Limits, Mount, ProcessSpec, TraceFiles},
    store::{hash_file, ObjectId, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
use md_sim_firmware::s9_update_validation as update;

const FIT_SHA: &str = "8b2a32020281aa67490c01633adf99c1a2ca6c9897e4c210bb6d0d001c15caca";
const FIT_BYTES: u64 = 18_333_708;

fn verify_one_bit(parent: &[u8], derived: &[u8], offset: usize) -> Result<()> {
    ensure!(
        parent.len() == derived.len() && offset < parent.len(),
        "fault image bounds differ"
    );
    ensure!(
        parent
            .iter()
            .zip(derived)
            .enumerate()
            .all(|(index, (before, after))| {
                before ^ after == if index == offset { 1 } else { 0 }
            }),
        "derived image is not the declared single-bit change"
    );
    Ok(())
}

fn derive() -> Result<()> {
    confinement::require(true)?;
    let parent = update::checked_image(Path::new("/original.tar"))?;
    let mut bytes = fs::read("/original.tar")?;
    ensure!(
        bytes.len() as u64 == update::BYTES,
        "archive changed after hash check"
    );
    let mut archive = tar::Archive::new(bytes.as_slice());
    let mut selected = None;
    let mut entries = 0;
    for entry in archive.entries()? {
        let mut entry = entry?;
        entries += 1;
        ensure!(entries <= 7, "unexpected update archive member count");
        if entry.path()?.as_ref() != Path::new("sysupgrade-am1-s9/fit") {
            continue;
        }
        ensure!(
            selected.is_none()
                && entry.header().entry_type().is_file()
                && entry.size() == FIT_BYTES,
            "ambiguous or wrong FIT member"
        );
        let start = entry.raw_file_position();
        let mut fit = Vec::new();
        (&mut entry).take(FIT_BYTES + 1).read_to_end(&mut fit)?;
        ensure!(
            fit.len() as u64 == FIT_BYTES && md_sim_firmware::prepare::hash_bytes(&fit) == FIT_SHA,
            "exact FIT bytes differ"
        );
        ensure!(
            start
                .checked_add(FIT_BYTES)
                .is_some_and(|end| end <= update::BYTES),
            "FIT member outside parent archive"
        );
        selected = Some((start, usize::try_from(start + FIT_BYTES / 2)?));
    }
    ensure!(entries == 7, "incomplete update archive");
    let (start, offset) = selected.context("exact FIT member absent")?;
    let before = bytes[offset];
    bytes[offset] ^= 1;
    let derived = ObjectId {
        sha256: md_sim_firmware::prepare::hash_bytes(&bytes),
        size: bytes.len() as u64,
    };
    std::io::stdout().write_all(&bytes)?;
    std::io::stdout().flush()?;
    serde_json::to_writer(
        std::io::stderr(),
        &json!({
        "schema":"md-sim-s9-update-payload-fault/1", "parent":parent,"derived":derived,
        "member":"sysupgrade-am1-s9/fit", "member_object":{"sha256":FIT_SHA,"size":FIT_BYTES},
        "member_data_offset":start,"archive_offset":offset,"before":before,"after":bytes[offset],"xor_mask":1,
        "scope":"Exactly one FIT payload bit changed. Tar headers and the original signature trailer are retained. This is a negative update input, never an executable firmware target or a signature rejection pass."}),
    )?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "__derive") {
        ensure!(args.len() == 2, "unexpected preparation arguments");
        return derive();
    }
    ensure!(args.len() == 2, "usage: s9_update_fault NEW_OUTPUT");
    let out = Path::new(&args[1]);
    ensure!(!out.exists(), "fault output must be new");
    let original = Path::new(update::ARCHIVE_IMAGE);
    let parent = update::checked_image(original)?;
    fs::create_dir_all(out)?;
    let store = Store::open(&out.join("inputs"), 128 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![("/derive-fault".into(), std::env::current_exe()?)],
    )?;
    store.import(original, &parent)?;
    inputs.push(Mount {
        object: parent.clone(),
        guest_path: "/original.tar".into(),
    });
    let spec = ProcessSpec {
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec!["/derive-fault".into(), "__derive".into()],
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 30,
            cpu_seconds: 20,
            memory_bytes: 256 * 1024 * 1024,
            processes: 16,
            file_bytes: 32 * 1024 * 1024,
            trace_bytes: update::BYTES + 65536,
            tmp_bytes: 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute_with_trace(
        &store,
        &spec,
        Arc::new(AtomicBool::new(false)),
        Some(TraceFiles {
            stdout: File::create_new(out.join("fault.tar"))?,
            stderr: File::create_new(out.join("derivation.json"))?,
        }),
    )?;
    save_json(&out.join("result.json"), &result)?;
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated,
        "isolated fault preparation failed; evidence retained"
    );
    ensure!(
        fs::metadata(out.join("derivation.json"))?.len() <= 4096,
        "fault manifest bound"
    );
    let record: Value = serde_json::from_slice(&fs::read(out.join("derivation.json"))?)?;
    let derived = hash_file(&out.join("fault.tar"))?;
    ensure!(
        record["parent"] == serde_json::to_value(&parent)?
            && record["derived"] == serde_json::to_value(&derived)?
            && parent.size == derived.size
            && parent.sha256 != derived.sha256,
        "fault provenance does not match actual files"
    );
    let original_bytes = fs::read(original)?;
    let derived_bytes = fs::read(out.join("fault.tar"))?;
    ensure!(
        md_sim_firmware::prepare::hash_bytes(&original_bytes) == parent.sha256,
        "original changed during preparation"
    );
    verify_one_bit(
        &original_bytes,
        &derived_bytes,
        usize::try_from(
            record["archive_offset"]
                .as_u64()
                .context("fault offset absent")?,
        )?,
    )?;
    update::Action::RejectTampered.checked_image(&out.join("fault.tar"))?;
    save_json(
        &out.join("manifest.json"),
        &json!({"derivation":record,
        "process_specification":hash_file(&out.join("spec.json"))?,
        "execution_result":hash_file(&out.join("result.json"))?,
        "independent_full_byte_comparison":"Exactly one XOR01 byte at the recorded offset; all other archive bytes unchanged",
        "firmware_rejection_gate":"untested"}),
    )?;
    println!("Prepared and checked the recorded single-bit negative update input");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comparison_rejects_extra_missing_or_out_of_bounds_changes() {
        verify_one_bit(&[0, 0xff, 2], &[0, 0xfe, 2], 1).unwrap();
        for (changed, offset) in [
            (&[0, 0xff, 2][..], 1),
            (&[0, 0xfc, 2][..], 1),
            (&[1, 0xfe, 2][..], 1),
            (&[0, 0xfe][..], 1),
            (&[0, 0xfe, 2][..], 3),
        ] {
            assert!(verify_one_bit(&[0, 0xff, 2], changed, offset).is_err());
        }
    }
}

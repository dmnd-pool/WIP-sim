//! Inspect actual selected FPGA predicates in a bounded isolated process.
use anyhow::{ensure, Result};
use md_sim_firmware::bitstream::{
    from_gzip,
    series7::{
        features::{decode, Decoding, FeatureDatabase},
        frames::Part,
        tilegrid::TileGrid,
    },
};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};

#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "support/fpga_feature_inputs.rs"]
mod inputs;

const REPORT_LIMIT: usize = 64 << 20;

#[derive(Serialize)]
struct Report<'a> {
    schema: &'static str,
    passed: bool,
    error: Option<String>,
    decoding: Option<&'a Decoding>,
    input_manifest: ObjectId,
    inspector: ObjectId,
    scope: &'static str,
}

struct BoundedWriter<W> {
    inner: W,
    remaining: usize,
}

impl<W: Write> Write for BoundedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(io::Error::other("feature inspection report byte bound"));
        }
        let written = self.inner.write(bytes)?;
        self.remaining -= written;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn inside() -> Result<()> {
    confinement::require(true)?;
    let (manifest, manifest_identity) = inputs::read(Path::new("/feature-inputs.json"))?;
    let inspected = (|| {
        let container = from_gzip(
            &inputs::checked(Path::new("/firmware.bit.gz"), &manifest.bitstream.object)?,
            4 << 20,
        )?;
        let part = Part::from_pinned_json(
            &inputs::checked(Path::new("/part.json"), &manifest.part.object)?,
            &manifest.part.object.sha256,
        )?;
        let grid = TileGrid::from_pinned_json(
            &inputs::checked(Path::new("/tilegrid.json"), &manifest.tilegrid.object)?,
            &manifest.tilegrid.object.sha256,
            &part,
        )?;
        let mut tables = Vec::new();
        for (index, table) in manifest.tables.iter().enumerate() {
            tables.push(FeatureDatabase::from_pinned_text(
                &inputs::checked(Path::new(&inputs::table_path(index)), &table.input.object)?,
                &table.input.object.sha256,
                &table.tile_type,
                &table.bus,
            )?);
        }
        decode(container.configuration(), &part, &grid, &tables)
    })();
    let report = Report {
        schema: "md-sim-fpga-feature-inspection/1",
        passed: inspected.is_ok(),
        error: inspected.as_ref().err().map(|e| format!("{e:#}")),
        decoding: inspected.as_ref().ok(),
        input_manifest: manifest_identity,
        inspector: hash_file(Path::new("/inspect"))?,
        scope: "Static archived configuration inspection. No guest firmware executes, no logic netlist or physical timing is established, and no exact-board gate passes.",
    };
    let mut writer = BoundedWriter {
        inner: BufWriter::new(io::stdout().lock()),
        remaining: REPORT_LIMIT,
    };
    serde_json::to_writer(&mut writer, &report)?;
    writer.flush()?;
    inspected.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_bound_preserves_exact_limit_and_rejects_extra_bytes() {
        let mut writer = BoundedWriter {
            inner: Vec::new(),
            remaining: 5,
        };
        writer.write_all(b"hello").unwrap();
        writer.write_all(b"").unwrap();
        assert!(writer.write_all(b"!").is_err());
        writer.flush().unwrap();
        assert_eq!(writer.inner, b"hello");
        assert_eq!(writer.remaining, 0);
    }
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|s| s == "__inside") {
        ensure!(args.len() == 2, "invalid feature inspector invocation");
        return inside();
    }
    ensure!(
        args.len() == 3,
        "usage: s9_feature_probe PINNED_INPUT_MANIFEST NEW_OUTPUT"
    );
    let input_path = Path::new(&args[1]);
    let (manifest, manifest_identity) = inputs::read(input_path)?;
    let out = PathBuf::from(&args[2]);
    fs::create_dir(&out)?;
    let store = Store::open(&out.join("inputs"), 128 << 20)?;
    let mut mounts = md_sim_firmware::lab::elf_closure(
        &store,
        vec![("/inspect".into(), std::env::current_exe()?)],
    )?;
    inputs::capture(
        &store,
        &mut mounts,
        input_path,
        &manifest,
        &manifest_identity,
    )?;
    let spec = ProcessSpec {
        support_virtualization: None,
        schema_version: "2.0".into(),
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        argv: vec!["/inspect".into(), "__inside".into()],
        inputs: mounts,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 30,
            cpu_seconds: 25,
            memory_bytes: 512 << 20,
            processes: 16,
            file_bytes: 1 << 20,
            trace_bytes: 64 << 20,
            tmp_bytes: 1 << 20,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    fs::write(out.join("features.json"), hex::decode(&result.stdout)?)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    ensure!(
        result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
        "isolated feature inspection failed; actual evidence retained"
    );
    println!("Feature predicates inspected. No firmware execution, complete netlist or exact-board acceptance claimed.");
    Ok(())
}

//! Exact, bounded source identities shared by FPGA inspection and its oracle.
use anyhow::{ensure, Result};
use md_sim_runtime::{
    isolation::Mount,
    store::{ObjectId, Store},
};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub const MANIFEST_LIMIT: u64 = 1 << 20;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub bitstream: Input,
    pub part: Input,
    pub tilegrid: Input,
    pub tables: Vec<Table>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub path: PathBuf,
    pub object: ObjectId,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    pub tile_type: String,
    pub bus: String,
    pub input: Input,
}

#[allow(dead_code)]
#[path = "bounded_file.rs"]
mod bounded_file;
pub use bounded_file::bounded;

pub fn read(path: &Path) -> Result<(Manifest, ObjectId)> {
    let bytes = bounded(path, MANIFEST_LIMIT)?;
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    ensure!(
        manifest.schema == "md-sim-fpga-feature-inputs/1",
        "feature input schema"
    );
    ensure!(
        !manifest.tables.is_empty() && manifest.tables.len() <= 64,
        "feature table count bound"
    );
    for (input, limit) in [
        (&manifest.bitstream, 4 << 20),
        (&manifest.part, 1 << 20),
        (&manifest.tilegrid, 32 << 20),
    ] {
        input.object.validate()?;
        ensure!(
            input.object.size > 0 && input.object.size <= limit,
            "feature source size bound"
        );
    }
    let mut keys = BTreeSet::new();
    let mut total = 0u64;
    for table in &manifest.tables {
        table.input.object.validate()?;
        ensure!(
            !table.tile_type.is_empty()
                && table.tile_type.len() <= 128
                && table
                    .tile_type
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_'),
            "feature table type"
        );
        ensure!(
            matches!(table.bus.as_str(), "CLB_IO_CLK" | "BLOCK_RAM" | "CFG_CLB")
                && keys.insert((&table.tile_type, &table.bus)),
            "feature table bus or duplicate identity"
        );
        ensure!(
            table.input.object.size > 0 && table.input.object.size <= 4 << 20,
            "feature table byte bound"
        );
        total += table.input.object.size;
    }
    ensure!(total <= 64 << 20, "aggregate feature table byte bound");
    Ok((
        manifest,
        ObjectId {
            sha256: md_sim_firmware::prepare::hash_bytes(&bytes),
            size: bytes.len() as u64,
        },
    ))
}

pub fn table_path(index: usize) -> String {
    format!("/feature-table-{index}.db")
}

pub fn capture(
    store: &Store,
    mounts: &mut Vec<Mount>,
    manifest_path: &Path,
    manifest: &Manifest,
    identity: &ObjectId,
) -> Result<()> {
    store.import(manifest_path, identity)?;
    mounts.push(Mount {
        object: identity.clone(),
        guest_path: "/feature-inputs.json".into(),
    });
    for (input, guest_path) in [
        (&manifest.bitstream, "/firmware.bit.gz"),
        (&manifest.part, "/part.json"),
        (&manifest.tilegrid, "/tilegrid.json"),
    ] {
        store.import(&input.path, &input.object)?;
        mounts.push(Mount {
            object: input.object.clone(),
            guest_path: guest_path.into(),
        });
    }
    for (index, table) in manifest.tables.iter().enumerate() {
        store.import(&table.input.path, &table.input.object)?;
        mounts.push(Mount {
            object: table.input.object.clone(),
            guest_path: table_path(index),
        });
    }
    Ok(())
}

pub fn checked(path: &Path, object: &ObjectId) -> Result<Vec<u8>> {
    let bytes = bounded(path, object.size)?;
    ensure!(
        bytes.len() as u64 == object.size
            && md_sim_firmware::prepare::hash_bytes(&bytes) == object.sha256,
        "captured FPGA input identity differs"
    );
    Ok(bytes)
}

//! Inspect an archived-derived Xilinx bitstream inside the worker boundary.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, Store},
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
fn main() -> Result<()> {
    let a: Vec<_> = std::env::args_os().collect();
    if a.get(1).is_some_and(|s| s == "__inside") {
        confinement::require(true)?;
        let input = fs::read("/bitstream.gz")?;
        let container = md_sim_firmware::bitstream::from_gzip(&input, 4 * 1024 * 1024)?;
        let bytes = container.bytes();
        let configuration = a
            .get(2)
            .map(|value| -> Result<_> {
                let idcode = u32::from_str_radix(value.to_str().context("IDCODE encoding")?, 16)?;
                md_sim_firmware::bitstream::series7::inspect(container.configuration(), idcode)
            })
            .transpose();
        let tiles = a
            .get(4)
            .map(|value| -> Result<_> {
                let part = md_sim_firmware::bitstream::series7::frames::Part::from_pinned_json(
                    &fs::read("/part.json")?,
                    a[3].to_str().context("part hash encoding")?,
                )?;
                let grid =
                    md_sim_firmware::bitstream::series7::tilegrid::TileGrid::from_pinned_json(
                        &fs::read("/tilegrid.json")?,
                        value.to_str().context("tile grid hash encoding")?,
                        &part,
                    )?;
                md_sim_firmware::bitstream::series7::tilegrid::map_tiles(
                    container.configuration(),
                    &part,
                    &grid,
                )
            })
            .transpose();
        let frames = a
            .get(3)
            .map(|value| -> Result<_> {
                let part = md_sim_firmware::bitstream::series7::frames::Part::from_pinned_json(
                    &fs::read("/part.json")?,
                    value.to_str().context("part hash encoding")?,
                )?;
                ensure!(
                    configuration
                        .as_ref()
                        .ok()
                        .and_then(|c| c.as_ref())
                        .is_some_and(|c| c.idcode == part.idcode()),
                    "part and requested silicon identities differ"
                );
                md_sim_firmware::bitstream::series7::frames::map_configuration(
                    container.configuration(),
                    &part,
                )
            })
            .transpose();
        let text: Vec<String> = bytes[..bytes.len().min(4096)]
            .split(|b| !b.is_ascii_graphic() && *b != b' ')
            .filter(|v| v.len() >= 4)
            .map(|v| String::from_utf8_lossy(v).into())
            .collect();
        serde_json::to_writer(
            std::io::stdout(),
            &serde_json::json!({"compressed":hash_file(Path::new("/bitstream.gz"))?,"expanded_sha256":md_sim_firmware::prepare::hash_bytes(bytes),"expanded_bytes":bytes.len(),"header_hex":hex::encode(&bytes[..bytes.len().min(256)]),"header_strings":text,"header":container.header(),"configuration":configuration.as_ref().ok(),"frames":frames.as_ref().ok(),"tiles":tiles.as_ref().ok(),"inspection_error":configuration.as_ref().err().or(frames.as_ref().err()).or(tiles.as_ref().err()).map(|e|format!("{e:#}")),"limit":"Container identity, packet/CRC checks, optional part-based frame mapping and pinned tile word ranges. No frame-to-logic reconstruction, firmware execution or exact physical board pass."}),
        )?;
        configuration?;
        frames?;
        return tiles.map(|_| ());
    }
    ensure!(
        a.len() == 3 || ((a.len() == 5 || ((a.len() == 9 || (a.len() == 13 && a[9] == "--tilegrid-json" && a[11] == "--tilegrid-sha256")) && a[5] == "--part-json" && a[7] == "--part-sha256")) && a[3] == "--series7-idcode"),
        "usage: s9_bitstream_probe BITSTREAM_GZ NEW_OUTPUT [--series7-idcode HEX [--part-json PATH --part-sha256 HASH [--tilegrid-json PATH --tilegrid-sha256 HASH]]]"
    );
    let mut argv = vec!["/probe".into(), "__inside".into()];
    if a.len() >= 5 {
        let value = a[4].to_str().context("IDCODE encoding")?;
        ensure!(
            value.len() == 8 && value.bytes().all(|v| v.is_ascii_hexdigit()),
            "IDCODE requires eight hex digits"
        );
        argv.push(value.into());
    }
    if a.len() >= 9 {
        let hash = a[8].to_str().context("part hash encoding")?;
        ensure!(
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v)),
            "part SHA256 requires lowercase hex"
        );
        argv.push(hash.into());
    }
    if a.len() == 13 {
        let hash = a[12].to_str().context("tile grid hash encoding")?;
        ensure!(
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v)),
            "tile grid SHA256 requires lowercase hex"
        );
        argv.push(hash.into());
    }
    let input = PathBuf::from(&a[1]);
    let out = PathBuf::from(&a[2]);
    ensure!(!out.exists(), "new output required");
    fs::create_dir_all(&out)?;
    let store = Store::open(&out.join("inputs"), 128 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![("/probe".into(), std::env::current_exe()?)],
    )?;
    let object = hash_file(&input)?;
    ensure!(object.size <= 4 * 1024 * 1024, "compressed bound");
    store.import(&input, &object)?;
    inputs.push(Mount {
        object,
        guest_path: "/bitstream.gz".into(),
    });
    if a.len() >= 9 {
        let path = Path::new(&a[6]);
        let object = hash_file(path)?;
        ensure!(
            object.size <= 1024 * 1024 && object.sha256 == a[8].to_str().unwrap(),
            "part database bound or hash mismatch"
        );
        store.import(path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: "/part.json".into(),
        });
    }
    if a.len() == 13 {
        let path = Path::new(&a[10]);
        let object = hash_file(path)?;
        ensure!(
            object.size <= md_sim_firmware::bitstream::series7::tilegrid::MAX_DATABASE_BYTES as u64
                && object.sha256 == a[12].to_str().unwrap(),
            "tile grid database bound or hash mismatch"
        );
        store.import(path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: "/tilegrid.json".into(),
        });
    }
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv,
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 15,
            cpu_seconds: 10,
            memory_bytes: 256 * 1024 * 1024,
            processes: 16,
            file_bytes: 1024 * 1024,
            trace_bytes: if a.len() == 13 {
                8 * 1024 * 1024
            } else {
                1024 * 1024
            },
            tmp_bytes: 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let r = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &r)?;
    fs::write(out.join("header.json"), hex::decode(r.stdout)?)?;
    ensure!(
        r.exit_code == Some(0) && !r.trace_truncated,
        "isolated bitstream inspection failed"
    );
    Ok(())
}

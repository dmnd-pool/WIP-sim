//! Static inspection of the exact T17 update cited by the BM1397 study.
//! No firmware program or archive-supplied command is executed.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute_with_trace, save_json, Limits, Mount, ProcessSpec, TraceFiles},
    store::{hash_file, ObjectId, Store},
};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path},
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};

const ARCHIVE: &str = "artifacts/files/firmware/bitmain-oem-7adde9c1d9605e5b0521/ANTMINER-T17-user-OM-202004231629-sig_5828.tar.gz";
const SHA: &str = "89677e26b8fb4b25cd8b3157b0a73b4df88f10f9994f0e7f8102af9d11f99772";
const SIZE: u64 = 17_123_204;
const BOUND: u64 = 128 << 20;

#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;

#[path = "support/arm_static.rs"]
mod arm_static;

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = flate2::Crc::new();
    crc.update(bytes);
    crc.sum()
}

fn object(bytes: &[u8]) -> ObjectId {
    ObjectId {
        sha256: md_sim_firmware::prepare::hash_bytes(bytes),
        size: bytes.len() as u64,
    }
}

fn bounded(reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "input exceeds inspection bound"
    );
    Ok(bytes)
}

fn tar_members(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    let stream: Box<dyn Read + '_> = if bytes.starts_with(&[0x1f, 0x8b]) {
        Box::new(flate2::read::GzDecoder::new(bytes))
    } else {
        Box::new(bytes)
    };
    let mut archive = tar::Archive::new(stream.take(BOUND + 1));
    let mut result = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut total = 0;
    for (i, entry) in archive.entries()?.enumerate() {
        ensure!(i < 256, "update member count bound");
        let entry = entry?;
        let path = entry.path()?;
        ensure!(
            path.components()
                .all(|p| matches!(p, Component::Normal(_) | Component::CurDir)),
            "unsafe update member path"
        );
        let name = path
            .components()
            .filter_map(|p| match p {
                Component::Normal(s) => Some(s.to_string_lossy()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/");
        ensure!(
            !name.is_empty() && seen.insert(name.clone()),
            "empty or duplicate member"
        );
        if entry.header().entry_type().is_dir() {
            continue;
        }
        ensure!(
            entry.header().entry_type().is_file(),
            "non-regular update member"
        );
        total += entry.size();
        ensure!(total <= BOUND, "update aggregate bound");
        result.insert(name, bounded(entry, BOUND)?);
    }
    Ok(result)
}

fn legacy_ramdisk(image: &[u8]) -> Result<&[u8]> {
    ensure!(
        image.len() >= 64 && image[..4] == [0x27, 0x05, 0x19, 0x56],
        "legacy image header absent"
    );
    let word = |at| u32::from_be_bytes(image[at..at + 4].try_into().unwrap());
    let mut header = image[..64].to_vec();
    header[4..8].fill(0);
    ensure!(crc32(&header) == word(4), "legacy header CRC differs");
    ensure!(
        image.len() - 64 == word(12) as usize,
        "legacy image length differs"
    );
    ensure!(
        image[28..32] == [5, 2, 3, 1],
        "expected Linux ARM gzip ramdisk"
    );
    let payload = &image[64..];
    ensure!(crc32(payload) == word(24), "legacy data CRC differs");
    Ok(payload)
}

fn inside() -> Result<()> {
    confinement::require_firmware_inspection()?;
    ensure!(
        hash_file(Path::new("/update"))?
            == ObjectId {
                sha256: SHA.into(),
                size: SIZE
            },
        "update identity differs"
    );
    let bytes = bounded(File::open("/update")?, SIZE)?;
    let outer = tar_members(&bytes)?;
    let mut report = json!({"schema":"md-sim-t17-static-inspection/1", "parent":object(&bytes), "firmware_executed":false,
        "outer":outer.iter().map(|(name,bytes)|json!({"name":name,"object":object(bytes)})).collect::<Vec<_>>()});
    let result = (|| -> Result<()> {
        let firmware = outer.get("fw.tar.gz").context("fw.tar.gz absent")?;
        let inner = tar_members(firmware)?;
        report["inner"] = json!(inner
            .iter()
            .map(|(name, bytes)| json!({"name":name,"object":object(bytes)}))
            .collect::<Vec<_>>());
        let image = inner
            .get("uramdisk.image.gz")
            .context("uramdisk.image.gz absent")?;
        let payload = legacy_ramdisk(image)?;
        let disk = bounded(flate2::read::GzDecoder::new(payload), BOUND)?;
        report["ramdisk"] =
            json!({"legacy_image":object(image),"gzip":object(payload),"filesystem":object(&disk)});
        fs::write("/work/ramdisk", disk)?;
        let dumped = Command::new("/usr/bin/debugfs")
            .args(["-R", "dump /usr/bin/bmminer /work/bmminer", "/work/ramdisk"])
            .output()?;
        fs::write("/work/debugfs.stdout", &dumped.stdout)?;
        fs::write("/work/debugfs.stderr", &dumped.stderr)?;
        ensure!(dumped.status.success(), "debugfs failed");
        let miner = bounded(File::open("/work/bmminer")?, 16 << 20)?;
        ensure!(
            miner.len() >= 52
                && miner[..7] == [0x7f, b'E', b'L', b'F', 1, 1, 1]
                && miner[18..20] == [40, 0],
            "extracted member is not an ARM little-endian ELF32"
        );
        report["miner"] = json!({"member":"/usr/bin/bmminer","object":object(&miner),"format":"ELF32 little-endian ARM"});
        report["disassembly"] = arm_static::inspect(&miner, File::create("/work/a32.txt")?)?;
        let symbols = Command::new("/usr/bin/readelf")
            .args(["--wide", "--symbols", "/work/bmminer"])
            .output()?;
        ensure!(
            symbols.stdout.len() <= 4 << 20 && symbols.stderr.len() <= 65536,
            "symbol output bound"
        );
        fs::write("/work/symbols.txt", symbols.stdout)?;
        fs::write("/work/readelf.stderr", symbols.stderr)?;
        ensure!(symbols.status.success(), "symbol inspection failed");
        Ok(())
    })();
    report["passed"] = json!(result.is_ok());
    report["error"] = json!(result.as_ref().err().map(|e| format!("{e:#}")));
    fs::write("/work/inspection.json", serde_json::to_vec_pretty(&report)?)?;
    let mut archive = tar::Builder::new(std::io::stdout());
    for name in [
        "inspection.json",
        "bmminer",
        "symbols.txt",
        "debugfs.stdout",
        "debugfs.stderr",
        "readelf.stderr",
        "a32.txt",
    ] {
        let path = Path::new("/work").join(name);
        if path.exists() {
            archive.append_path_with_name(path, name)?;
        }
    }
    archive.finish()?;
    std::io::stdout().flush()?;
    result
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|s| s == "__inside") {
        return inside();
    }
    ensure!(args.len() == 2, "usage: t17_source_inspect NEW_OUTPUT");
    let out = Path::new(&args[1]);
    fs::create_dir(out)?;
    let parent = hash_file(Path::new(ARCHIVE))?;
    ensure!(
        parent.sha256 == SHA && parent.size == SIZE,
        "archive differs from acquisition manifest"
    );
    let store = Store::open(&out.join("inputs"), 256 << 20)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/inspect".into(), std::env::current_exe()?),
            ("/usr/bin/debugfs".into(), "/usr/bin/debugfs".into()),
            ("/usr/bin/readelf".into(), "/usr/bin/readelf".into()),
            (
                "/usr/lib/libcapstone.so.5".into(),
                "/usr/lib/libcapstone.so.5".into(),
            ),
        ],
    )?;
    store.import(Path::new(ARCHIVE), &parent)?;
    inputs.push(Mount {
        object: parent,
        guest_path: "/update".into(),
    });
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        schema_version: "2.0".into(),
        dashboard: None,
        loopback: false,
        argv: vec!["/inspect".into(), "__inside".into()],
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 60,
            cpu_seconds: 30,
            memory_bytes: 512 << 20,
            processes: 16,
            file_bytes: BOUND,
            trace_bytes: 24 << 20,
            tmp_bytes: 192 << 20,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute_with_trace(
        &store,
        &spec,
        Arc::new(AtomicBool::new(false)),
        Some(TraceFiles {
            stdout: File::create_new(out.join("derived.tar"))?,
            stderr: File::create_new(out.join("stderr.log"))?,
        }),
    )?;
    save_json(&out.join("result.json"), &result)?;
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated,
        "isolated static inspection failed; retained result and derived.tar"
    );
    println!("Static inspection complete. Firmware execution remains untested.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_header_rejects_bit_damage_truncation_and_trailing_bytes() {
        // Empty gzip member and legacy header made with Python's independent
        // gzip/zlib code. Header CRCcf2984e0, payload CRCa90c1bcf.
        let image = hex::decode("27051956cf2984e000000000000000140000000000000000a90c1bcf0502030100000000000000000000000000000000000000000000000000000000000000001f8b08000000000002ff03000000000000000000").unwrap();
        assert_eq!(legacy_ramdisk(&image).unwrap().len(), 20);
        for bit in 0..image.len() * 8 {
            let mut damaged = image.clone();
            damaged[bit / 8] ^= 1 << (bit % 8);
            assert!(legacy_ramdisk(&damaged).is_err(), "bit{bit}");
        }
        assert!(legacy_ramdisk(&image[..image.len() - 1]).is_err());
        let mut extended = image;
        extended.push(0);
        assert!(legacy_ramdisk(&extended).is_err());
        assert_eq!(crc32(b"123456789"), 0xcbf43926);
    }

    #[test]
    fn archive_reader_rejects_duplicate_and_link_members() {
        let archive = |duplicate: bool, link: bool| {
            let mut tar = tar::Builder::new(Vec::new());
            for name in if duplicate {
                vec!["fw.tar.gz", "./fw.tar.gz"]
            } else {
                vec!["fw.tar.gz"]
            } {
                let mut header = tar::Header::new_gnu();
                header.set_size(1);
                header.set_mode(0o444);
                if link {
                    header.set_entry_type(tar::EntryType::Symlink);
                    header.set_link_name("/outside").unwrap();
                }
                header.set_cksum();
                tar.append_data(&mut header, name, &b"a"[..]).unwrap();
            }
            tar.into_inner().unwrap()
        };
        assert_eq!(
            tar_members(&archive(false, false)).unwrap()["fw.tar.gz"],
            b"a"
        );
        assert!(tar_members(&archive(true, false)).is_err());
        assert!(tar_members(&archive(false, true)).is_err());
        assert!(bounded(&b"abcd"[..], 3).is_err());
    }
}

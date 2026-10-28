//! Static A32 inspection of the exact extracted 2022 S9 Bosminer ELF.
//! The ARM program is data. Only the captured Capstone library executes.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute_with_trace, save_json, Limits, Mount, ProcessSpec, TraceFiles},
    store::{hash_file, ObjectId, Store},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

#[path = "support/arm_static.rs"]
mod arm_static;
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;

const ELF: &str = "work/simulator/qemu-s9/axi-bosminer-extract-511b/stdout.bin";
const ELF_SHA: &str = "4997a1c10a09f6881d6f5e0d4967eb31072ea9f1da7a80dac192ae1299d45960";
const ELF_SIZE: u64 = 12_699_280;
const ROOTFS_SHA: &str = "2bd49c70b915b7e25f6c01013d95a5ffea8d499b73c6c6ec5539fda4247154b3";
const OUTPUT_BOUND: u64 = 1 << 20;
// Intervals are the exact .ARM.exidx bounds enclosing the original
// PC-relative references to baud/version diagnostic string tables.
const RANGES: [(u32, u32); 3] = [
    (0x3c0680, 0x3c08a8),
    (0x47b0b0, 0x47b2e0),
    (0x47b5f4, 0x47bbc8),
];

fn expected_elf() -> ObjectId {
    ObjectId {
        sha256: ELF_SHA.into(),
        size: ELF_SIZE,
    }
}

struct LimitedWriter<W> {
    writer: W,
    remaining: u64,
}
impl<W: Write> Write for LimitedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() as u64 > self.remaining {
            return Err(std::io::Error::other("disassembly bound"));
        }
        let written = self.writer.write(bytes)?;
        self.remaining -= written as u64;
        Ok(written)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

fn read_elf(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(ELF_SIZE + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == ELF_SIZE && md_sim_firmware::prepare::hash_bytes(&bytes) == ELF_SHA,
        "selected Bosminer ELF identity differs"
    );
    Ok(bytes)
}

fn validate_unwind_ranges(bytes: &[u8]) -> Result<()> {
    ensure!(
        bytes.len() >= 52
            && bytes[..7] == [0x7f, b'E', b'L', b'F', 1, 1, 1]
            && bytes[18..20] == [40, 0],
        "not an A32 ELF"
    );
    let word = |at: usize| -> Result<u32> {
        Ok(u32::from_le_bytes(
            bytes
                .get(at..at + 4)
                .context("ELF word outside input")?
                .try_into()?,
        ))
    };
    let half = |at: usize| -> Result<u16> {
        Ok(u16::from_le_bytes(
            bytes
                .get(at..at + 2)
                .context("ELF halfword outside input")?
                .try_into()?,
        ))
    };
    let offset = word(32)? as usize;
    let count = half(48)? as usize;
    ensure!(
        half(46)? == 40 && (1..=256).contains(&count),
        "ELF section table bound"
    );
    let mut functions = Vec::new();
    let mut tables = 0;
    for index in 0..count {
        let at = offset
            .checked_add(index * 40)
            .context("section table overflow")?;
        if word(at + 4)? != 0x70000001 {
            continue;
        }
        tables += 1;
        let address = word(at + 12)?;
        let position = word(at + 16)? as usize;
        let size = word(at + 20)? as usize;
        ensure!(
            size > 0 && size <= 1 << 20 && size % 8 == 0,
            "ARM unwind table bound"
        );
        for delta in (0..size).step_by(8) {
            let raw = word(
                position
                    .checked_add(delta)
                    .context("unwind offset overflow")?,
            )?;
            // PREL31 is a signed31-bit displacement from the entry address.
            let signed = ((raw << 1) as i32) >> 1;
            let value = i64::from(address) + delta as i64 + i64::from(signed);
            let value = u32::try_from(value).context("unwind address overflow")?;
            ensure!(
                functions.last().is_none_or(|last| *last < value),
                "unordered unwind addresses"
            );
            functions.push(value);
        }
    }
    ensure!(tables == 1, "one ARM unwind table required");
    for &(start, end) in &RANGES {
        ensure!(
            functions.windows(2).any(|pair| pair == [start, end]),
            "selected interval differs from original unwind boundary"
        );
    }
    Ok(())
}

fn legacy_decoder_regression() -> Result<()> {
    // An independent literal MOV/BX vector in one ordinary executable ELF
    // section exercises both public entry points in the confined process.
    let mut elf = vec![0; 140];
    elf[..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 1, 1, 1]);
    elf[18..20].copy_from_slice(&40u16.to_le_bytes());
    elf[32..36].copy_from_slice(&52u32.to_le_bytes());
    elf[46..48].copy_from_slice(&40u16.to_le_bytes());
    elf[48..50].copy_from_slice(&2u16.to_le_bytes());
    for (at, value) in [(96, 1u32), (100, 4), (104, 0x1000), (108, 132), (112, 8)] {
        elf[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    elf[132..].copy_from_slice(&[1, 0, 0xa0, 0xe3, 0x1e, 0xff, 0x2f, 0xe1]);
    let mut complete = Vec::new();
    let mut selected = Vec::new();
    let old = arm_static::inspect(&elf, &mut complete)?;
    let new = arm_static::inspect_ranges(&elf, &[(0x1000, 0x1008)], &mut selected)?;
    ensure!(
        complete == b"00001000 mov        r0, #1\n00001004 bx         lr\n"
            && selected == complete
            && old == new,
        "legacy/static-range literal regression differs"
    );
    Ok(())
}

fn inside() -> Result<()> {
    confinement::require_firmware_inspection()?;
    let mut report = json!({
        "schema":"md-sim-s9-bosminer-static-inspection/1", "firmware_executed":false,
        "elf":expected_elf(), "parent_rootfs_sha256":ROOTFS_SHA,
        "member":"usr/bin/bosminer", "linear_decode_is_control_flow_proof":false,
        "output_bound":OUTPUT_BOUND, "requested_ranges":RANGES,
    });
    let result = (|| -> Result<()> {
        legacy_decoder_regression()?;
        report["legacy_api_regression_passed"] = json!(true);
        let bytes = read_elf(Path::new("/bosminer"))?;
        validate_unwind_ranges(&bytes)?;
        let output = LimitedWriter {
            writer: File::create_new("/work/a32.txt")?,
            remaining: OUTPUT_BOUND,
        };
        report["disassembly"] = arm_static::inspect_ranges(&bytes, &RANGES, output)?;
        report["disassembly_output"] = json!(hash_file(Path::new("/work/a32.txt"))?);
        Ok(())
    })();
    report["passed"] = json!(result.is_ok());
    report["error"] = json!(result.as_ref().err().map(|e| format!("{e:#}")));
    fs::write("/work/inspection.json", serde_json::to_vec_pretty(&report)?)?;
    let mut archive = tar::Builder::new(std::io::stdout());
    for name in ["inspection.json", "a32.txt"] {
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
        ensure!(args.len() == 2, "unexpected internal argument");
        return inside();
    }
    ensure!(args.len() == 2, "usage: s9_bosminer_inspect NEW_OUTPUT");
    let out = Path::new(&args[1]);
    fs::create_dir(out)?;
    let store = Store::open(&out.join("inputs"), 128 << 20)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/inspect".into(), std::env::current_exe()?),
            (
                "/usr/lib/libcapstone.so.5".into(),
                "/usr/lib/libcapstone.so.5".into(),
            ),
        ],
    )?;
    let expected = expected_elf();
    store.import(Path::new(ELF), &expected)?;
    inputs.push(Mount {
        object: expected,
        guest_path: "/bosminer".into(),
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
            file_bytes: 128 << 20,
            trace_bytes: 2 << 20,
            tmp_bytes: 4 << 20,
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
        result.exit_code == Some(0) && !result.trace_truncated && result.stderr.is_empty(),
        "static inspection failed; retained complete result and output"
    );
    let mut archive = tar::Archive::new(File::open(out.join("derived.tar"))?);
    let mut seen = std::collections::BTreeSet::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.to_string_lossy().into_owned();
        ensure!(
            ["inspection.json", "a32.txt"].contains(&name.as_str())
                && seen.insert(name.clone())
                && entry.header().entry_type().is_file()
                && entry.size() <= OUTPUT_BOUND,
            "unexpected inspection output member"
        );
        std::io::copy(&mut entry, &mut File::create_new(out.join(name))?)?;
    }
    ensure!(seen.len() == 2, "inspection output incomplete");
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(out.join("inspection.json"))?)?;
    ensure!(
        report["passed"] == true && report["elf"] == json!(expected_elf()),
        "inspection report differs"
    );
    let expected: ObjectId = serde_json::from_value(
        report
            .get("disassembly_output")
            .context("output identity absent")?
            .clone(),
    )?;
    ensure!(
        hash_file(&out.join("a32.txt"))? == expected,
        "disassembly output differs"
    );
    println!("Static A32 inspection passed. No firmware code executed.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unwind_intervals_reject_changed_boundaries_and_duplicate_entries() {
        let mut elf = vec![0; 180];
        elf[..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 1, 1, 1]);
        elf[18..20].copy_from_slice(&40u16.to_le_bytes());
        elf[32..36].copy_from_slice(&52u32.to_le_bytes());
        elf[46..48].copy_from_slice(&40u16.to_le_bytes());
        elf[48..50].copy_from_slice(&2u16.to_le_bytes());
        elf[96..100].copy_from_slice(&0x70000001u32.to_le_bytes());
        elf[104..108].copy_from_slice(&0x200u32.to_le_bytes());
        elf[108..112].copy_from_slice(&132u32.to_le_bytes());
        elf[112..116].copy_from_slice(&48u32.to_le_bytes());
        for (i, address) in RANGES.into_iter().flat_map(|(a, b)| [a, b]).enumerate() {
            let relative = address - (0x200 + i as u32 * 8);
            elf[132 + i * 8..136 + i * 8].copy_from_slice(&relative.to_le_bytes());
        }
        validate_unwind_ranges(&elf).unwrap();
        let mut changed = elf.clone();
        changed[132] ^= 4;
        assert!(validate_unwind_ranges(&changed).is_err());
        let first = u32::from_le_bytes(elf[132..136].try_into().unwrap());
        elf[140..144].copy_from_slice(&(first - 8).to_le_bytes());
        assert!(validate_unwind_ranges(&elf).is_err());
    }
    #[test]
    fn malformed_ranges_fail_before_foreign_decoder_loading() {
        for ranges in [
            vec![],
            vec![(4, 4)],
            vec![(1, 8)],
            vec![(4, 12), (8, 16)],
            vec![(4, 4 + (64 << 10) + 4)],
        ] {
            assert!(arm_static::inspect_ranges(&[], &ranges, Vec::new()).is_err());
        }
    }
    #[test]
    fn each_range_must_match_one_section_before_decoder_loading() {
        let mut elf = vec![0; 172];
        elf[..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 1, 1, 1]);
        elf[18..20].copy_from_slice(&40u16.to_le_bytes());
        elf[32..36].copy_from_slice(&52u32.to_le_bytes());
        elf[46..48].copy_from_slice(&40u16.to_le_bytes());
        elf[48..50].copy_from_slice(&3u16.to_le_bytes());
        for at in [92, 132] {
            elf[at + 4..at + 8].copy_from_slice(&1u32.to_le_bytes());
            elf[at + 8..at + 12].copy_from_slice(&4u32.to_le_bytes());
            elf[at + 12..at + 16].copy_from_slice(&0x1000u32.to_le_bytes());
            elf[at + 20..at + 24].copy_from_slice(&4u32.to_le_bytes());
        }
        let error =
            arm_static::inspect_ranges(&elf, &[(0x1000, 0x1004), (0x2000, 0x2004)], Vec::new())
                .unwrap_err();
        assert!(error.to_string().contains("unique executable section"));
        elf[136..140].copy_from_slice(&0u32.to_le_bytes());
        let error =
            arm_static::inspect_ranges(&elf, &[(0x1000, 0x1004), (0x2000, 0x2004)], Vec::new())
                .unwrap_err();
        assert!(error.to_string().contains("unique executable section"));
    }
    #[test]
    fn output_bound_rejects_overflow_without_partial_write() {
        let mut writer = LimitedWriter {
            writer: Vec::new(),
            remaining: 3,
        };
        writer.write_all(b"ab").unwrap();
        assert!(writer.write_all(b"cd").is_err());
        assert_eq!(writer.writer, b"ab");
        writer.write_all(b"c").unwrap();
        assert_eq!(writer.writer, b"abc");
    }
    #[test]
    fn changed_or_short_elf_cannot_enter_the_decoder() {
        let path = tempfile::NamedTempFile::new().unwrap();
        fs::write(path.path(), b"\x7fELF").unwrap();
        assert!(read_elf(path.path()).is_err());
        assert!(validate_unwind_ranges(b"short").is_err());
        fs::write(path.path(), vec![0; ELF_SIZE as usize]).unwrap();
        assert!(read_elf(path.path()).is_err());
    }
}

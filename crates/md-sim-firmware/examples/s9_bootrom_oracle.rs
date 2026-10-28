//! Run an unchanged public Zynq ROM read routine in a confined ARM ABI fixture.
//! The caller supplies a valid filesystem object and a cached final SD sector.
//! This does not execute the ROM reset path or establish selected S9 identity.
use anyhow::{ensure, Context, Result};
use md_sim_firmware::prepare::hash_bytes;
use md_sim_runtime::{
    isolation::{execute_with_trace, save_json, Limits, Mount, ProcessSpec, TraceFiles},
    store::{hash_file, ObjectId, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    os::unix::process::ExitStatusExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
};
#[path = "support/bounded_file.rs"]
mod bounded_file;
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "support/s9_fat16.rs"]
mod fat;
const ROM_SHA: &str = "c58e2bc1d837654457ed47064accb26c4fb48307737e60d6c2d05253e7ee3aea";
const SD_SHA: &str = "97a257571af4fb9e62f709e2d6b8f36a191fdd07f1cd12ae780bd1c73af7eaef";
const BOOT_SHA: &str = "950d19c6e5bfd9d8107def130b19fc9a5f64d824b26c0c493d5fdfe8503edac4";
const LINKER: &str = "/toolchain/lib/rustlib/x86_64-unknown-linux-gnu/bin/ld.lld";
const LINKER_SHA: &str = "4739514e3484915747c34b35007cd990235cc6b490b2d0525a61ac402ca16af9";
const LLVM_NAME: &str = "libLLVM.so.23.1-rust-1.99.0-stable";
const LLVM_SHA: &str = "7c2c7ebb79211f07e3b32e408a1a65b4d205656792f9c51bdbefcb21c0868cbd";
const DRIVER: &[u8] = include_bytes!("../../../simulator/firmware/fixtures/zynq-rom-eof-513.c");
const SCRIPT: &[u8] = include_bytes!("../../../simulator/firmware/fixtures/zynq-rom-eof-513.ld");
fn input(bytes: &[u8], sha: &str, size: usize) -> Result<()> {
    ensure!(
        bytes.len() == size && hash_bytes(bytes) == sha,
        "exact input identity differs"
    );
    Ok(())
}
fn word(b: &[u8], at: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        b.get(at..at.checked_add(4).context("word overflow")?)
            .context("short word")?
            .try_into()?,
    ))
}
fn object(b: &[u8]) -> ObjectId {
    ObjectId {
        sha256: hash_bytes(b),
        size: b.len() as u64,
    }
}

fn verify_rom_copy(elf: &[u8], rom: &[u8]) -> Result<Value> {
    ensure!(
        elf.get(..7) == Some(&[0x7f, b'E', b'L', b'F', 1, 1, 1])
            && elf.get(16..20) == Some(&[2, 0, 40, 0]),
        "static ARM ELF32 required"
    );
    let at = word(elf, 28)? as usize;
    let size = u16::from_le_bytes(elf.get(42..44).context("ELF header")?.try_into()?) as usize;
    let count = u16::from_le_bytes(elf.get(44..46).context("ELF header")?.try_into()?) as usize;
    ensure!(
        size == 32 && (1..=16).contains(&count),
        "ELF load table bound"
    );
    let mut matched = None;
    let mut ranges = Vec::new();
    let mut executable_ranges = Vec::new();
    for i in 0..count {
        let p = elf
            .get(at + i * size..at + (i + 1) * size)
            .context("ELF program header")?;
        ensure!(![2, 3].contains(&word(p, 0)?), "dynamic ELF is not allowed");
        if word(p, 0)? != 1 {
            continue;
        }
        let address = word(p, 8)?;
        let bytes = word(p, 16)? as usize;
        let memory = word(p, 20)?;
        let end = address
            .checked_add(memory)
            .context("ELF address overflow")?;
        ensure!(
            memory as usize >= bytes && end <= 0x2000000,
            "ELF load bounds"
        );
        for &(old_start, old_end) in &ranges {
            ensure!(
                end <= old_start || address >= old_end,
                "overlapping ELF load segments"
            );
        }
        ranges.push((address, end));
        if word(p, 24)? & 1 != 0 {
            executable_ranges.push((address, end));
        }
        if address == 0 {
            ensure!(
                matched.is_none()
                    && bytes == rom.len()
                    && word(p, 20)? as usize == bytes
                    && word(p, 24)? == 5,
                "ROM load segment differs"
            );
            let offset = word(p, 4)? as usize;
            ensure!(
                elf.get(
                    offset
                        ..offset
                            .checked_add(bytes)
                            .context("ROM ELF range overflow")?
                ) == Some(rom),
                "linked original ROM changed"
            );
            matched = Some(json!({"virtual_address":0,"file_offset":offset,"object":object(rom)}));
        }
    }
    let entry = word(elf, 24)?;
    ensure!(
        (0x1000000..0x2000000).contains(&entry)
            && ranges
                .iter()
                .any(|&(start, end)| start <= entry && entry < end),
        "entry is not the declared ABI harness"
    );
    ensure!(
        executable_ranges
            .iter()
            .any(|&(start, end)| start <= entry && entry < end),
        "harness entry is not executable"
    );
    matched.context("original ROM segment absent")
}
fn verify_output(output: &[u8], boot: &[u8]) -> Result<Value> {
    ensure!(output.len() == 16 + 28 * 540, "oracle record count/size");
    ensure!(boot.len() == 79263, "oracle reference file size");
    ensure!(
        [
            word(output, 0)?,
            word(output, 4)?,
            word(output, 8)?,
            word(output, 12)?
        ] == [0x454f4631, 1, 28, 540],
        "oracle header"
    );
    let mut cases = Vec::new();
    let positions = [78849u32, 79000, 79262, 79263];
    let requests = [0, 1, 2, 415, 79263, u32::MAX];
    for i in 0..28usize {
        let r = &output[16 + i * 540..16 + (i + 1) * 540];
        let (position, requested, fault) = if i < 24 {
            (positions[i / 6], requests[i % 6], 0)
        } else {
            (79262, 79263, (i - 23) as u32)
        };
        ensure!(
            [word(r, 0)?, word(r, 4)?, word(r, 8)?, word(r, 12)?]
                == [i as u32, position, requested, fault],
            "oracle input sequence"
        );
        let expected_status = match fault {
            0 => 0,
            1 => 9,
            2 => 7,
            3 => 2,
            4 => 3,
            _ => unreachable!(),
        };
        let count = if fault == 0 {
            requested.min(79263 - position)
        } else {
            0
        };
        ensure!(
            [word(r, 16)?, word(r, 20)?, word(r, 24)?]
                == [expected_status, count, position + count],
            "original EOF/status result differs at{i}"
        );
        let actual = &r[28..];
        ensure!(
            actual[..count as usize] == boot[position as usize..(position + count) as usize],
            "original read bytes differ at{i}"
        );
        ensure!(
            actual[count as usize..].iter().all(|&b| b == 0xa5),
            "bytes past returned count were changed at{i}"
        );
        cases.push(json!({"id":i,"position":position,"requested":requested,"fault":fault,"status":expected_status,"actual_bytes":count,"tail_sentinel_unchanged":true,"output":object(actual)}));
    }
    Ok(
        json!({"cases":cases,"passed":true,"scope":"Unchanged original f_read instructions. Explicit cached filesystem and sector inputs; no SD hardware calls or reset/board acceptance."}),
    )
}
fn run(program: &str, args: &[&str], name: &str) -> Result<()> {
    let stdout = PathBuf::from(format!("/work/{name}.stdout"));
    let stderr = PathBuf::from(format!("/work/{name}.stderr"));
    // Run-owned files obey the2MiB process FSIZE cap. The bounded reader
    // checks metadata before allocation; failed streams remain in the archive.
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(File::create_new(&stdout)?)
        .stderr(File::create_new(&stderr)?)
        .status()?;
    save_json(
        Path::new(&format!("/work/{name}.status.json")),
        &json!({"exit_code":status.code(),"signal":status.signal(),"success":status.success()}),
    )?;
    let out = bounded_file::capture(&stdout, 1 << 20)?;
    let err = bounded_file::capture(&stderr, 1 << 20)?;
    save_json(
        Path::new(&format!("/work/{name}.status.json")),
        &json!({"exit_code":status.code(),"signal":status.signal(),"stdout":out.object,"stderr":err.object}),
    )?;
    ensure!(
        status.code() == Some(0) && err.bytes.is_empty(),
        "tool failed or emitted diagnostics: {name}"
    );
    if name == "compile" {
        ensure!(out.bytes.is_empty(), "compiler emitted unexpected stdout");
    }
    Ok(())
}
fn inside() -> Result<()> {
    confinement::require_arm_protocol_oracle()?;
    let mut storage = Value::Null;
    let mut copy = Value::Null;
    let mut observations = Value::Null;
    let result = (|| -> Result<()> {
        let rom = fs::read("/rom.bin")?;
        input(&rom, ROM_SHA, 131072)?;
        let sd = fs::read("/sd.img")?;
        input(&sd, SD_SHA, 112197632)?;
        let boot = fs::read("/boot.bin")?;
        input(&boot, BOOT_SHA, 79263)?;
        ensure!(
            fs::read("/oracle.c")? == DRIVER && fs::read("/oracle.ld")? == SCRIPT,
            "fixture bytes differ"
        );
        let allocation = fat::inspect(&sd, b"BOOT    BIN")?;
        ensure!(
            allocation.logical == boot,
            "FAT reader differs from independently extracted7zip file"
        );
        fs::write("/work/last-sector.bin", &allocation.final_sector)?;
        storage = allocation.report;
        let offset = word(&boot, 0x30)?;
        let declared = word(&boot, 0x34)?;
        ensure!(
            offset == 2240 && declared == 79263 && word(&boot, 0x40)? == declared,
            "selected header differs"
        );
        storage["boot_header"] = json!({"source_offset":offset,"declared_length":declared,"available_logical_suffix":boot.len()-offset as usize,"transfer_beyond_logical_eof":offset as usize+declared as usize-boot.len(),"rom_copy_policy_inferred":false});
        drop(sd);
        run("/tools/clang", &["--version"], "compiler-version")?;
        run(LINKER, &["--version"], "linker-version")?;
        run("/tools/qemu-arm", &["--version"], "emulator-version")?;
        let args = [
            "--target=armv7a-linux-gnueabi",
            "-march=armv7-a",
            "-marm",
            "-O2",
            "-nostdlib",
            "-static",
            "-fno-pie",
            "-nostdinc",
            "-ffreestanding",
            "-fno-builtin",
            "-fno-stack-protector",
            "-fno-unwind-tables",
            "-fno-asynchronous-unwind-tables",
            "-fsanitize=undefined",
            "-fsanitize-trap=undefined",
            "-Wall",
            "-Wextra",
            "-Werror",
            "--ld-path=/toolchain/lib/rustlib/x86_64-unknown-linux-gnu/bin/ld.lld",
            "-Wl,-e,_start,--threads=1",
            "-Wl,-T,/oracle.ld,-z,max-page-size=4096",
            "/oracle.c",
            "-o",
            "/work/oracle",
        ];
        save_json(Path::new("/work/compile-arguments.json"), &args)?;
        run("/tools/clang", &args, "compile")?;
        copy = verify_rom_copy(&fs::read("/work/oracle")?, &rom)?;
        run(
            "/tools/qemu-arm",
            &["-s", "1048576", "/work/oracle"],
            "execution",
        )?;
        observations = verify_output(&fs::read("/work/execution.stdout")?, &boot)?;
        Ok(())
    })();
    let mut identities = BTreeMap::new();
    let names = [
        "oracle",
        "last-sector.bin",
        "compile-arguments.json",
        "compiler-version.stdout",
        "compiler-version.stderr",
        "compiler-version.status.json",
        "linker-version.stdout",
        "linker-version.stderr",
        "linker-version.status.json",
        "emulator-version.stdout",
        "emulator-version.stderr",
        "emulator-version.status.json",
        "compile.stdout",
        "compile.stderr",
        "compile.status.json",
        "execution.stdout",
        "execution.stderr",
        "execution.status.json",
    ];
    for name in names {
        let p = Path::new("/work").join(name);
        if p.is_file() {
            identities.insert(name, hash_file(&p)?);
        }
    }
    let report = json!({"schema":"md-sim-zynq-rom-eof-oracle/1","passed":result.is_ok(),"error":result.as_ref().err().map(|e|format!("{e:#}")),"storage":storage,"original_rom_copy":copy,"observations":observations,"outputs":identities,"scope":"Public XC7Z010-CLG400A ROM routine oracle in an adapted ARM ELF. Exact S9 silicon/ROM equivalence is unproved. No reset, complete SD transfer, firmware boot, original board, physical timing or mining pass.","original_board_gate":false,"reset_execution":false,"wrapper_ubsan":"undefined-behavior traps; original ROM instructions unchanged"});
    save_json(Path::new("/work/assessment.json"), &report)?;
    let mut archive = tar::Builder::new(std::io::stdout());
    archive.append_path_with_name("/work/assessment.json", "assessment.json")?;
    for name in names {
        let p = Path::new("/work").join(name);
        if p.is_file() {
            archive.append_path_with_name(p, name)?;
        }
    }
    archive.finish()?;
    result
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "__inside") {
        ensure!(args.len() == 2, "inner args");
        return inside();
    }
    ensure!(
        args.len() == 6,
        "usage: s9_bootrom_oracle ROM SD_IMAGE EXTRACTED_BOOT RUST_LLD NEW_OUTPUT"
    );
    let out = Path::new(&args[5]);
    fs::create_dir(out)?;
    fs::write(out.join("oracle.c"), DRIVER)?;
    fs::write(out.join("oracle.ld"), SCRIPT)?;
    let store = Store::open(&out.join("inputs"), 1 << 30)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/oracle-check".into(), std::env::current_exe()?),
            ("/tools/clang".into(), PathBuf::from("/usr/bin/clang")),
            ("/tools/qemu-arm".into(), PathBuf::from("/usr/bin/qemu-arm")),
            ("/usr/lib/librt.so.1".into(), "/usr/lib/librt.so.1".into()),
            (
                "/usr/lib/libpthread.so.0".into(),
                "/usr/lib/libpthread.so.0".into(),
            ),
            ("/usr/lib/libdl.so.2".into(), "/usr/lib/libdl.so.2".into()),
        ],
    )?;
    let linker = PathBuf::from(&args[4]).canonicalize()?;
    let llvm = linker
        .parent()
        .context("linker parent")?
        .join("../../..")
        .join(LLVM_NAME)
        .canonicalize()?;
    for (guest, path, sha) in [
        (LINKER.to_owned(), linker, LINKER_SHA),
        (format!("/toolchain/lib/{LLVM_NAME}"), llvm, LLVM_SHA),
    ] {
        let object = hash_file(&path)?;
        ensure!(object.sha256 == sha, "linker closure differs");
        store.import(&path, &object)?;
        inputs.push(Mount {
            guest_path: guest,
            object,
        });
    }
    for (guest, path, sha, size) in [
        ("/rom.bin", Path::new(&args[1]), ROM_SHA, 131072),
        ("/sd.img", Path::new(&args[2]), SD_SHA, 112197632),
        ("/boot.bin", Path::new(&args[3]), BOOT_SHA, 79263),
    ] {
        let object = ObjectId {
            sha256: sha.into(),
            size,
        };
        store.import(path, &object)?;
        inputs.push(Mount {
            guest_path: guest.into(),
            object,
        });
    }
    for name in ["oracle.c", "oracle.ld"] {
        let path = out.join(name);
        let object = hash_file(&path)?;
        store.import(&path, &object)?;
        inputs.push(Mount {
            guest_path: format!("/{name}"),
            object,
        });
    }
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        schema_version: "2.0".into(),
        dashboard: None,
        loopback: false,
        argv: vec!["/oracle-check".into(), "__inside".into()],
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 90,
            cpu_seconds: 60,
            memory_bytes: 6 << 30,
            processes: 16,
            file_bytes: 2 << 20,
            trace_bytes: 8 << 20,
            tmp_bytes: 32 << 20,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute_with_trace(
        &store,
        &spec,
        Arc::new(AtomicBool::new(false)),
        Some(TraceFiles {
            stdout: File::create_new(out.join("evidence.tar"))?,
            stderr: File::create_new(out.join("stderr.log"))?,
        }),
    )?;
    save_json(&out.join("result.json"), &result)?;
    ensure!(
        result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
        "original ROM oracle failed; evidence retained"
    );
    println!("Original ROM EOF routine and exact SD allocation checked. No reset or board pass.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn put32(b: &mut [u8], at: usize, x: u32) {
        b[at..at + 4].copy_from_slice(&x.to_le_bytes());
    }
    fn fixture_elf() -> Vec<u8> {
        let mut b = vec![0u8; 512];
        b[..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 1, 1, 1]);
        b[16..20].copy_from_slice(&[2, 0, 40, 0]);
        put32(&mut b, 24, 0x1000000);
        put32(&mut b, 28, 52);
        b[42..44].copy_from_slice(&32u16.to_le_bytes());
        b[44..46].copy_from_slice(&3u16.to_le_bytes());
        for (at, off, addr, size, flags) in [(52, 256, 0, 4, 5), (84, 260, 0x1000000, 4, 5)] {
            for (field, value) in [
                (0, 1),
                (4, off),
                (8, addr),
                (16, size),
                (20, size),
                (24, flags),
            ] {
                put32(&mut b, at + field, value);
            }
        }
        b[256..260].copy_from_slice(&[1, 2, 3, 4]);
        b
    }
    #[test]
    fn elf_rejects_dynamic_nonexecutable_overlap_and_changed_original() {
        let b = fixture_elf();
        verify_rom_copy(&b, &[1, 2, 3, 4]).unwrap();
        for kind in [2, 3] {
            let mut changed = b.clone();
            put32(&mut changed, 116, kind);
            assert!(verify_rom_copy(&changed, &[1, 2, 3, 4]).is_err());
        }
        let mut changed = b.clone();
        put32(&mut changed, 84 + 24, 4);
        assert!(verify_rom_copy(&changed, &[1, 2, 3, 4]).is_err());
        let mut changed = b.clone();
        put32(&mut changed, 84 + 8, 2);
        assert!(verify_rom_copy(&changed, &[1, 2, 3, 4]).is_err());
        assert!(verify_rom_copy(&b, &[1, 2, 3, 5]).is_err());
    }
    #[test]
    fn malformed_output_and_changed_inputs_reject() {
        assert!(verify_output(&[], &[]).is_err());
        assert!(verify_output(&vec![0; 16 + 28 * 540], &[]).is_err());
        assert!(input(&[0], ROM_SHA, 131072).is_err());
        assert!(verify_rom_copy(&[], &[]).is_err());
    }
}

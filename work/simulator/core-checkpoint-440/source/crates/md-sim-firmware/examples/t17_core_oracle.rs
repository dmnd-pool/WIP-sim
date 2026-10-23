//! Execute two exact vendor packing routines in isolated QEMU ARM user mode.
//! This is a protocol oracle experiment, not a miner or board execution gate.
use anyhow::{ensure, Result};
use md_sim_runtime::{
    isolation::{execute_with_trace, save_json, Limits, Mount, ProcessSpec, TraceFiles},
    store::{hash_file, ObjectId, Store},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};

#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
const MINER_SHA: &str = "ad090e7abbf525a7a8a23c7153529e45af94b864e1170e677e1f85b4c4091367";
const DRIVER: &[u8] = include_bytes!("../../../simulator/firmware/fixtures/bm1397-core-oracle.c");
const LINKER: &str = "/toolchain/lib/rustlib/x86_64-unknown-linux-gnu/bin/ld.lld";
const LINKER_SHA: &str = "4739514e3484915747c34b35007cd990235cc6b490b2d0525a61ac402ca16af9";
const LLVM_NAME: &str = "libLLVM.so.23.1-rust-1.99.0-stable";
const LLVM_SHA: &str = "7c2c7ebb79211f07e3b32e408a1a65b4d205656792f9c51bdbefcb21c0868cbd";

fn run(program: &str, args: &[&str], name: &str) -> Result<()> {
    let result = Command::new(program).args(args).output()?;
    ensure!(
        result.stdout.len() <= 1 << 20 && result.stderr.len() <= 1 << 20,
        "oracle tool output bound"
    );
    fs::write(format!("/work/{name}.stdout"), result.stdout)?;
    fs::write(format!("/work/{name}.stderr"), result.stderr)?;
    ensure!(
        result.status.success(),
        "oracle tool failed: {name}: {}",
        result.status
    );
    Ok(())
}

fn inside() -> Result<()> {
    confinement::require_arm_protocol_oracle()?;
    ensure!(
        hash_file(Path::new("/miner"))?
            == ObjectId {
                sha256: MINER_SHA.into(),
                size: 553247
            },
        "vendor executable differs"
    );
    ensure!(fs::read("/oracle.c")? == DRIVER, "oracle wrapper differs");
    let mut function_copies = Vec::new();
    let result = (|| -> Result<usize> {
        run("/tools/clang", &["--version"], "compiler-version")?;
        run(LINKER, &["--version"], "linker-version")?;
        run("/tools/qemu-arm", &["--version"], "emulator-version")?;
        run(
            "/tools/clang",
            &[
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
                "/oracle.c",
                "-o",
                "/work/oracle",
            ],
            "compile",
        )?;
        ensure!(
            fs::metadata("/work/compile.stderr")?.len() == 0,
            "compiler diagnostic output must be reviewed"
        );
        let executable = fs::read("/work/oracle")?;
        let original = fs::read("/miner")?;
        for (name, start) in [("read", 117936), ("write", 118000)] {
            let bytes = &original[start..start + 64];
            let offsets: Vec<_> = executable
                .windows(bytes.len())
                .enumerate()
                .filter_map(|(at, value)| (value == bytes).then_some(at))
                .collect();
            ensure!(
                offsets.len() == 1,
                "original {name} function is absent or duplicated in oracle ELF"
            );
            function_copies.push(json!({"operation":name,"source_file_offset":start,"oracle_file_offset":offsets[0],"size":64,"sha256":md_sim_firmware::prepare::hash_bytes(bytes)}));
        }
        ensure!(
            executable.len() >= 52
                && executable[..7] == [0x7f, b'E', b'L', b'F', 1, 1, 1]
                && executable[16..20] == [2, 0, 40, 0],
            "oracle must be a static ARM ELF32 executable"
        );
        let phoff = u32::from_le_bytes(executable[28..32].try_into()?) as usize;
        let phsize = u16::from_le_bytes(executable[42..44].try_into()?) as usize;
        let count = u16::from_le_bytes(executable[44..46].try_into()?) as usize;
        ensure!(
            phsize == 32
                && count <= 32
                && phoff
                    .checked_add(count * phsize)
                    .is_some_and(|end| end <= executable.len()),
            "oracle program-header bound"
        );
        for i in 0..count {
            let h = &executable[phoff + i * phsize..phoff + (i + 1) * phsize];
            let field = |n| u32::from_le_bytes(h[n..n + 4].try_into().unwrap());
            ensure!(
                !matches!(field(0), 2 | 3),
                "oracle cannot require a dynamic interpreter"
            );
            if field(0) == 1 {
                ensure!(
                    field(8)
                        .checked_add(field(20))
                        .is_some_and(|end| end < 268435456),
                    "oracle exceeds reserved guest address space"
                );
            }
        }
        // QEMU11.1.2 reserves the full ARM range, including its high helper
        // page. The separate6GiB address-space bound permits that reservation.
        run(
            "/tools/qemu-arm",
            &["-s", "1048576", "/work/oracle"],
            "execution",
        )?;
        let bytes = fs::read("/work/execution.stdout")?;
        ensure!(
            bytes.len() == (1024 * 32 + 32 * 512) * 4,
            "oracle output count differs"
        );
        let mut words = bytes
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        for core in 0..1024u32 {
            for reg in 0..32u32 {
                ensure!(
                    words.next() == Some(((core >> 1) & 0x1ff) << 16 | (reg & 15) << 8 | 255),
                    "read packing differs for core{core}, register{reg}"
                );
            }
        }
        for reg in 0..32u32 {
            for value in 0..512u32 {
                ensure!(
                    words.next() == Some(0x80008000 | (reg & 15) << 8 | (value & 255)),
                    "write packing differs for register{reg}, value{value}"
                );
            }
        }
        Ok(bytes.len() / 4)
    })();
    let optional_object = |name: &str| -> Result<Option<ObjectId>> {
        let path = Path::new("/work").join(name);
        if path.exists() {
            Ok(Some(hash_file(&path)?))
        } else {
            Ok(None)
        }
    };
    let report = json!({"schema":"md-sim-vendor-core-packing-oracle/1","passed":result.is_ok(),"error":result.as_ref().err().map(|e|format!("{e:#}")),"computed_words":result.as_ref().ok(),"original_function_copies":function_copies,"oracle_executable":optional_object("oracle")?,"output_words":optional_object("execution.stdout")?,"input_order":{"read":{"outer_core_range_exclusive":[0,1024],"inner_register_range_exclusive":[0,32]},"write":{"outer_register_range_exclusive":[0,32],"inner_value_range_exclusive":[0,512]},"word_encoding":"little-endian u32; all read cases followed by all write cases"},"wrapper_ubsan":"undefined-behavior traps; original instruction bytes are not instrumented","vendor_miner":hash_file(Path::new("/miner"))?,"wrapper":hash_file(Path::new("/oracle.c"))?,"scope":"Two original ARM routines execute under captured QEMU ARM in an adapted test ELF. No original-board boot, complete miner execution, ASIC behavior, nonce or mining pass is claimed."});
    fs::write("/work/assessment.json", serde_json::to_vec_pretty(&report)?)?;
    let mut archive = tar::Builder::new(std::io::stdout());
    for name in [
        "assessment.json",
        "oracle",
        "compiler-version.stdout",
        "compiler-version.stderr",
        "linker-version.stdout",
        "linker-version.stderr",
        "emulator-version.stdout",
        "emulator-version.stderr",
        "compile.stdout",
        "compile.stderr",
        "execution.stdout",
        "execution.stderr",
    ] {
        let path = Path::new("/work").join(name);
        if path.is_file() {
            archive.append_path_with_name(path, name)?;
        }
    }
    archive.finish()?;
    result.map(|_| ())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "__inside") {
        return inside();
    }
    ensure!(
        args.len() == 4,
        "usage: t17_core_oracle EXTRACTED_MINER RUST_LLD NEW_OUTPUT"
    );
    let out = Path::new(&args[3]);
    fs::create_dir(out)?;
    let miner = Path::new(&args[1]);
    let object = hash_file(miner)?;
    ensure!(
        object.sha256 == MINER_SHA && object.size == 553247,
        "vendor input differs"
    );
    fs::write(out.join("oracle.c"), DRIVER)?;
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
    // These two exact Rust1.99 toolchain files use relative RUNPATH entries.
    // Keep their original bytes and directory relationship in the empty
    // namespace. Their fixed dependencies are in the captured system closure.
    // Do not relax the general ELF closure's rejection of arbitrary RUNPATHs.
    let linker = PathBuf::from(&args[2]).canonicalize()?;
    let llvm = linker
        .parent()
        .unwrap()
        .join("../../..")
        .join(LLVM_NAME)
        .canonicalize()?;
    for (guest, path, sha) in [
        (LINKER.to_owned(), linker, LINKER_SHA),
        (format!("/toolchain/lib/{LLVM_NAME}"), llvm, LLVM_SHA),
    ] {
        let object = hash_file(&path)?;
        ensure!(object.sha256 == sha, "pinned Rust linker toolchain differs");
        store.import(&path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: guest,
        });
    }
    for (guest, path) in [
        ("/miner", miner.to_path_buf()),
        ("/oracle.c", out.join("oracle.c")),
    ] {
        let object = hash_file(&path)?;
        store.import(&path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: guest.into(),
        });
    }
    let spec = ProcessSpec {
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
            file_bytes: 64 << 20,
            trace_bytes: 4 << 20,
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
        result.exit_code == Some(0) && !result.trace_truncated,
        "isolated oracle failed; retained evidence"
    );
    println!("Original core packing routines passed the isolated protocol oracle.");
    Ok(())
}

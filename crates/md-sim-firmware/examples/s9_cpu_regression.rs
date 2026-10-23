//! Isolated assembly and execution of a project-owned CPU reset regression.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
fn text_section(bytes: &[u8]) -> Result<&[u8]> {
    ensure!(
        bytes.len() >= 52 && &bytes[..7] == b"\x7fELF\x01\x01\x01",
        "expected little-endian ELF32"
    );
    let u16_at = |n: usize| -> Result<usize> {
        Ok(u16::from_le_bytes(bytes.get(n..n + 2).context("ELF bounds")?.try_into()?) as usize)
    };
    let u32_at = |n: usize| -> Result<usize> {
        Ok(u32::from_le_bytes(bytes.get(n..n + 4).context("ELF bounds")?.try_into()?) as usize)
    };
    ensure!(
        u16_at(16)? == 1 && u16_at(18)? == 40,
        "expected ARM relocatable object"
    );
    let base = u32_at(32)?;
    let size = u16_at(46)?;
    let count = u16_at(48)?;
    ensure!(
        size == 40
            && count <= 128
            && base
                .checked_add(size * count)
                .is_some_and(|end| end <= bytes.len()),
        "ELF section bounds"
    );
    let mut result = None;
    for i in 0..count {
        let s = base + i * size;
        let kind = u32_at(s + 4)?;
        let flags = u32_at(s + 8)?;
        let len = u32_at(s + 20)?;
        ensure!(
            !matches!(kind, 4 | 9) || len == 0,
            "test object contains unresolved relocation"
        );
        if kind == 1 && flags & 4 != 0 {
            ensure!(result.is_none(), "multiple executable sections");
            let off = u32_at(s + 16)?;
            result = Some(
                bytes
                    .get(off..off.checked_add(len).context("section overflow")?)
                    .context("text bounds")?,
            );
        }
    }
    result.context("no executable text")
}
fn inner() -> Result<()> {
    confinement::require_board()?;
    let output = Command::new("/clang")
        .args([
            "-cc1as",
            "-triple",
            "armv7-none-eabi",
            "-target-cpu",
            "cortex-a9",
            "-filetype",
            "obj",
            "/source.S",
            "-o",
            "/work/cpu.o",
        ])
        .stdout(fs::File::create("/work/assembler.stdout")?)
        .stderr(fs::File::create("/work/assembler.stderr")?)
        .status()?;
    let mut report = json!({"code":output.code()});
    let mut success = output.success();
    for name in ["stdout", "stderr"] {
        let path = PathBuf::from(format!("/work/assembler.{name}"));
        let mut bytes = Vec::new();
        fs::File::open(&path)?.take(65536).read_to_end(&mut bytes)?;
        let object = hash_file(&path)?;
        report[name] = json!(String::from_utf8_lossy(&bytes));
        report[format!("{name}_identity")] = serde_json::to_value(&object)?;
        report[format!("{name}_truncated")] = json!(object.size > bytes.len() as u64);
        success &= object.size <= bytes.len() as u64;
    }
    if output.success() {
        ensure!(
            fs::metadata("/work/cpu.o")?.len() < 65536,
            "object size limit"
        );
        let bytes = fs::read("/work/cpu.o")?;
        report["object_hex"] = json!(hex::encode(&bytes));
        match text_section(&bytes) {
            Ok(text) => report["text_hex"] = json!(hex::encode(text)),
            Err(error) => {
                report["text_error"] = json!(format!("{error:#}"));
                success = false;
            }
        }
    }
    serde_json::to_writer(std::io::stdout(), &report)?;
    ensure!(success, "assembler or object validation failed");
    Ok(())
}
fn limits() -> Limits {
    Limits {
        wall_seconds: 20,
        cpu_seconds: 15,
        memory_bytes: 3 * 1024 * 1024 * 1024,
        processes: 64,
        file_bytes: 4 * 1024 * 1024,
        trace_bytes: 4 * 1024 * 1024,
        tmp_bytes: 16 * 1024 * 1024,
    }
}
fn run() -> Result<()> {
    let a: Vec<_> = std::env::args_os().collect();
    if a.get(1).is_some_and(|v| v == "__inside") {
        return inner();
    }
    ensure!(a.len()==4||a.len()==5,"usage: s9_cpu_regression prepare SOURCE NEW_OUTPUT | run BACKEND_BUILD PREPARATION NEW_OUTPUT");
    let prepare = a[1] == "prepare";
    ensure!(
        (prepare && a.len() == 4) || (a[1] == "run" && a.len() == 5),
        "invalid mode"
    );
    let out = PathBuf::from(a.last().unwrap());
    ensure!(!out.exists(), "new output required");
    fs::create_dir_all(&out)?;
    let store = Store::open(&out.join("inputs"), 768 * 1024 * 1024)?;
    let (inputs, argv, identity) = if prepare {
        let mut inputs = md_sim_firmware::lab::elf_closure(
            &store,
            vec![
                ("/probe".into(), std::env::current_exe()?),
                ("/clang".into(), PathBuf::from("/usr/bin/clang")),
            ],
        )?;
        let source = Path::new(&a[2]);
        ensure!(fs::metadata(source)?.len() < 65536, "source limit");
        let object = hash_file(source)?;
        store.import(source, &object)?;
        inputs.push(Mount {
            object,
            guest_path: "/source.S".into(),
        });
        (
            inputs,
            vec!["/probe".into(), "__inside".into()],
            json!({"kind":"project-owned test assembly; not firmware","recipe":hash_file(&std::env::current_exe()?)?}),
        )
    } else {
        let backend = Path::new(&a[2]);
        let preparation = Path::new(&a[3]);
        let report: Value = serde_json::from_slice(&fs::read(backend.join("build-report.json"))?)?;
        let binary = backend.join("qemu-system-arm");
        ensure!(
            report["build"]["code"] == 0
                && hash_file(&binary)?
                    == serde_json::from_value::<ObjectId>(report["binary"].clone())?,
            "backend differs"
        );
        let mut inputs = md_sim_firmware::lab::elf_closure(&store, vec![("/qemu".into(), binary)])?;
        let image = preparation.join("test.bin");
        let object = hash_file(&image)?;
        let prep: Value = serde_json::from_slice(&fs::read(preparation.join("assessment.json"))?)?;
        ensure!(
            prep["passed"] == true && serde_json::to_value(&object)? == prep["text"],
            "prepared regression differs"
        );
        store.import(&image, &object)?;
        inputs.push(Mount {
            object,
            guest_path: "/test.bin".into(),
        });
        let argv = [
            "/qemu",
            "-machine",
            "md-sim-s9-zynq-a9",
            "-accel",
            "tcg,thread=single",
            "-m",
            "128M",
            "-smp",
            "2",
            "-nodefaults",
            "-nic",
            "none",
            "-display",
            "none",
            "-monitor",
            "none",
            "-serial",
            "null",
            "-serial",
            "stdio",
            "-device",
            "loader,file=/test.bin,addr=0,cpu-num=0,force-raw=on",
            "-trace",
            "enable=md_sim_*",
        ];
        (
            inputs,
            argv.into_iter().map(str::to_string).collect(),
            json!({"kind":"project-owned CPU/device regression; not archived firmware","preparation":hash_file(&preparation.join("assessment.json"))?,"backend_build":hash_file(&backend.join("build-report.json"))?}),
        )
    };
    let spec = ProcessSpec {
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
        limits: limits(),
    };
    save_json(&out.join("spec.json"), &spec)?;
    save_json(&out.join("identity.json"), &identity)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    let bytes = hex::decode(&result.stdout)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    if prepare {
        fs::write(out.join("assembler-report.json"), &bytes)?;
        let r: Value = serde_json::from_slice(&bytes)?;
        ensure!(
            result.exit_code == Some(0) && !result.trace_truncated && r["code"] == 0,
            "preparation failed"
        );
        let object = hex::decode(r["object_hex"].as_str().context("object missing")?)?;
        let text = hex::decode(r["text_hex"].as_str().context("text missing")?)?;
        ensure!(text_section(&object)? == text, "object/text mismatch");
        fs::write(out.join("test.o"), object)?;
        fs::write(out.join("test.bin"), text)?;
        save_json(
            &out.join("assessment.json"),
            &json!({"passed":true,"text":hash_file(&out.join("test.bin"))?,"object":hash_file(&out.join("test.o"))?}),
        )?;
    } else {
        fs::write(out.join("guest.log"), &bytes)?;
        let passed = bytes == b"ABCDERP\n"
            && !result.trace_truncated
            && result.status == "timed-out"
            && result.exit_code.is_none();
        save_json(
            &out.join("assessment.json"),
            &json!({"passed":passed,"expected":"ABCDERP\n","actual":String::from_utf8_lossy(&bytes),"process_status":result.status,"exit_code":result.exit_code,"timeout_is_expected":true,"termination":"Both CPUs halt after the success markers. The supervisor deadline must end the stopped machine; a backend crash cannot pass.","scope":"Secondary reset/clock release, preserved registers across clock stop, machine reset and secondary re-release; project-owned regression only."}),
        )?;
        ensure!(passed, "CPU regression differs; retained logs are evidence");
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|v| v == "__inside") {
        return inner();
    }
    let out = args.last().map(PathBuf::from);
    let new = out.as_ref().is_some_and(|p| !p.exists());
    let result = run();
    if let (Err(error), Some(out)) = (&result, out) {
        if new && out.is_dir() {
            let path = out.join("assessment.json");
            let mut report = if path.is_file() && fs::metadata(&path)?.len() < 65536 {
                serde_json::from_slice(&fs::read(&path)?)?
            } else {
                json!({})
            };
            report["passed"] = json!(false);
            report["error"] = json!(format!("{error:#}"));
            save_json(&path, &report)?;
        }
    }
    result
}

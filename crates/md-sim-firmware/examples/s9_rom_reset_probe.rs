//! Execute a pinned public Zynq ROM from reset. Diagnostic, never a board pass.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute_with_trace, save_json, Limits, Mount, ProcessSpec, TraceFiles},
    store::{hash_file, ObjectId, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Write,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[allow(dead_code)]
#[path = "support/s9_boot_storage.rs"]
mod boot_storage;
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "support/rom_trace.rs"]
mod rom_trace;

const ROM_SHA: &str = "c58e2bc1d837654457ed47064accb26c4fb48307737e60d6c2d05253e7ee3aea";
const SD_SHA: &str = "97a257571af4fb9e62f709e2d6b8f36a191fdd07f1cd12ae780bd1c73af7eaef";

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "__inside") {
        confinement::require(false)?;
        ensure!(args.len() > 2, "emulator arguments absent");
        ensure!(
            hash_file(Path::new("/sd.img"))?.sha256 == SD_SHA,
            "captured original SD backing differs"
        );
        boot_storage::create_sd()?;
        // This is an explicit tool-preparation record, not guest serial output.
        println!(
            "MD_SIM_ROM_STORAGE_CREATED {}",
            json!({
            "overlay": hash_file(Path::new("/work/sd.qcow2"))?,
            "virtual_bytes":134217728,"backing_sha256":SD_SHA,
            "scope":"Run-owned copy-on-write storage; zero extension beyond the original image, no change to original filesystem bytes."})
        );
        std::io::stdout().flush()?;
        let error = Command::new("/qemu").args(&args[2..]).exec();
        return Err(error).context("execute captured QEMU");
    }
    ensure!(
        args.len() == 5,
        "usage: s9_rom_reset_probe BACKEND_BUILD ROM SD_IMAGE NEW_OUTPUT"
    );
    let build = Path::new(&args[1]);
    let report_path = build.join("build-report.json");
    ensure!(
        fs::metadata(&report_path)?.len() < 4 << 20,
        "backend report bound"
    );
    let report_bytes = fs::read(&report_path)?;
    let report: Value = serde_json::from_slice(&report_bytes)?;
    let binary = build.join("qemu-system-arm");
    let expected: ObjectId = serde_json::from_value(report["binary"].clone())?;
    ensure!(
        hash_file(&binary)? == expected
            && report["build"]["code"] == 0
            && report["rom_reset_patch"]["code"] == 0
            && report["rom_reset_reference"]["contract"] == "md-sim-public-zynq-rom-reset/1",
        "backend identity or ROM reset capability differs"
    );
    let out = Path::new(&args[4]);
    fs::create_dir(out)?;
    let store = Store::open(&out.join("inputs"), 256 << 20)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/qemu".into(), binary),
            ("/probe".into(), std::env::current_exe()?),
            ("/qemu-img".into(), PathBuf::from("/usr/bin/qemu-img")),
        ],
    )?;
    ensure!(
        inputs
            .iter()
            .find(|m| m.guest_path == "/qemu")
            .context("backend mount absent")?
            .object
            == expected,
        "captured backend changed after report check"
    );
    for (guest, path, sha, size) in [
        ("/rom.bin", Path::new(&args[2]), ROM_SHA, 131072),
        ("/sd.img", Path::new(&args[3]), SD_SHA, 112197632),
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
    let spec = ProcessSpec {
        schema_version: "2.0".into(),
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        argv: [
            "/probe",
            "__inside",
            "-machine",
            "md-sim-s9-zynq-a9,boot-mode=sd,bootrom-reference=on",
            "-accel",
            "tcg,thread=single,tb-size=64",
            "-m",
            "256M",
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
            "stdio",
            "-serial",
            "null",
            "-bios",
            "/rom.bin",
            "-drive",
            "file=/work/sd.qcow2,format=qcow2,if=sd,cache=writethrough",
            "-icount",
            "shift=0,align=off,sleep=off",
            "-d",
            "in_asm,guest_errors,unimp",
            "-dfilter",
            "0x0..0x20000,0xffff0000..0xffffffff",
            "-trace",
            "enable=md_sim_zynq_rom_map",
            "-trace",
            "enable=vm_state_notify",
        ]
        .map(String::from)
        .to_vec(),
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 10,
            cpu_seconds: 5,
            memory_bytes: 1 << 30,
            processes: 16,
            file_bytes: 4 << 20,
            trace_bytes: 8 << 20,
            tmp_bytes: 16 << 20,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    save_json(
        &out.join("input-identities.json"),
        &json!({
            "backend_report":{"sha256":md_sim_firmware::prepare::hash_bytes(&report_bytes),"size":report_bytes.len()},
            "backend":expected,"driver":hash_file(&std::env::current_exe()?)?,
            "rom":spec.inputs.iter().find(|m|m.guest_path=="/rom.bin").context("ROM mount")?.object,
            "sd":spec.inputs.iter().find(|m|m.guest_path=="/sd.img").context("SD mount")?.object,
            "storage":{"contract_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/s9_boot_storage.rs")),
                "qemu_img":spec.inputs.iter().find(|m|m.guest_path=="/qemu-img").context("storage tool mount")?.object,
                "virtual_bytes":134217728,"original_backing_read_only":true,
                "derived":"Run-owned qcow2 overlay; zero extension beyond original112197632bytes. No FAT file or allocation padding. No physical SD capacity claim."},
            "tcg_translation_cache_mib":64,
            "boot_mode":"Both CPUs start at reset0 in the original public ROM; no loader device or CPU entry override.",
            "scope":"Public-ROM reference. Exact S9 ROM/silicon equivalence and complete hardware support unresolved."
        }),
    )?;
    let trace_path = out.join("instruction-trace.log");
    let writer = File::create_new(&trace_path)?;
    let reader = File::open(&trace_path)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    let observer = {
        let cancelled = cancelled.clone();
        let done = done.clone();
        std::thread::spawn(move || {
            let result = rom_trace::watch(reader, cancelled.clone(), done);
            if result.is_err() {
                cancelled.store(true, Ordering::Release);
            }
            result
        })
    };
    let execution = execute_with_trace(
        &store,
        &spec,
        cancelled,
        Some(TraceFiles {
            stdout: File::create_new(out.join("serial.stdout"))?,
            stderr: writer,
        }),
    );
    done.store(true, Ordering::Release);
    let diagnostic = match observer.join() {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => Some(json!({"observer_error":format!("{error:#}")})),
        Err(_) => Some(json!({"observer_error":"trace observer panicked"})),
    };
    save_json(&out.join("first-diagnostic.json"), &diagnostic)?;
    let result = match execution {
        Ok(result) => result,
        Err(error) => {
            save_json(
                &out.join("execution-error.json"),
                &json!({"error":format!("{error:#}")}),
            )?;
            return Err(error);
        }
    };
    save_json(&out.join("result.json"), &result)?;
    let trace = fs::read(out.join("instruction-trace.log"))?;
    ensure!(
        trace.len() <= 8 << 20,
        "retained trace exceeds declared bound"
    );
    let text = String::from_utf8_lossy(&trace);
    let lines: Vec<_> = text.lines().filter(|line| line.starts_with("0x")).collect();
    let assessment = json!({"schema":"md-sim-public-rom-reset-probe/2",
        "process_status":result.status,"exit_code":result.exit_code,
        "trace_truncated":result.trace_truncated,"elapsed_ms":result.wall_milliseconds,
        "translated_block_address_headers":lines.len(),"first_diagnostic":diagnostic,
        "startup_diagnostics":rom_trace::startup_evidence(&trace),
        "trace":hash_file(&out.join("instruction-trace.log"))?,
        "serial":hash_file(&out.join("serial.stdout"))?,
        "result":hash_file(&out.join("result.json"))?,
        "original_board_gate":false,"firmware_boot_gate":false,"functional_mining_gate":false,
        "scope":"Bounded instruction diagnostic. Translation headers alone do not prove each block or instruction executed. Review the actual control flow and unsupported interfaces; no stage or watchdog success is synthesized."
    });
    save_json(&out.join("assessment.json"), &assessment)?;
    println!("Public ROM reset diagnostic retained; all board and mining gates remain open.");
    // A bounded diagnostic is not a successful boot. Preserve the observed
    // outcome and fail the command until a real boot acceptance exists.
    anyhow::bail!("reset boot acceptance is incomplete; inspect retained evidence")
}

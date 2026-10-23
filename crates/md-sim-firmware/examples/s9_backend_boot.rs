//! Execute original archived S9 components on the experimental pinned backend.
use anyhow::{ensure, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, Store},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};
const COMPONENTS: &[(&str, &str)] = &[
    (
        "kernel",
        "f85eba975b58b4333e99a656ad661667cbedaf3111af3deb8572b902b88cd276",
    ),
    (
        "ramdisk",
        "2bd49c70b915b7e25f6c01013d95a5ffea8d499b73c6c6ec5539fda4247154b3",
    ),
    (
        "board.dtb",
        "550c87818cb0db41c00a8351dc4ac7b5dce5bf6d5336539c6951163ebf3cb62a",
    ),
];
fn main() -> Result<()> {
    let a: Vec<_> = std::env::args_os().collect();
    ensure!(
        (4..=7).contains(&a.len()),
        "usage: s9_backend_boot REPO BACKEND_BUILD NEW_OUTPUT [trace] [single-cpu] [legacy-io]"
    );
    ensure!(
        a[4..]
            .iter()
            .all(|v| v == "trace" || v == "single-cpu" || v == "legacy-io"),
        "unknown diagnostic option"
    );
    let trace = a[4..].iter().any(|v| v == "trace");
    let single = a[4..].iter().any(|v| v == "single-cpu");
    let legacy = a[4..].iter().any(|v| v == "legacy-io");
    let repo = PathBuf::from(&a[1]);
    let build = PathBuf::from(&a[2]);
    let out = PathBuf::from(&a[3]);
    ensure!(!out.exists(), "new output required");
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(build.join("build-report.json"))?)?;
    let backend = build.join("qemu-system-arm");
    ensure!(
        serde_json::to_value(hash_file(&backend)?)? == report["binary"],
        "backend binary changed"
    );
    ensure!(report["build"]["code"] == 0, "backend build did not pass");
    let archive=repo.join("artifacts/files/firmware/braiins-feed-25bba3b4a505ccbc0e54/sd-firmware_2022-04-29-0-5eb2c5fe-22.02.4-plus_arm_cortex-a9_neon.tar");
    ensure!(
        hash_file(&archive)?.sha256
            == "469743ebcd5faf3445e4f4209ad8af58cc7adeff37c7df28af3c53a5b255b56e",
        "firmware archive changed"
    );
    fs::create_dir_all(&out)?;
    let store = Store::open(&out.join("inputs"), 512 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(&store, vec![("/qemu".into(), backend)])?;
    for (name, hash) in COMPONENTS {
        let path = repo
            .join("work/simulator/original-zynq/attempt-008/components")
            .join(name);
        let object = hash_file(&path)?;
        ensure!(object.sha256 == *hash, "component changed");
        store.import(&path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: format!("/{name}"),
        });
    }
    let argv=["/qemu","-machine","md-sim-s9-zynq-a9,boot-mode=sd","-accel","tcg,thread=single","-m","1024M","-smp","2","-nodefaults","-nic","none","-display","none","-monitor","none","-serial","null","-serial","stdio","-no-reboot","-kernel","/kernel","-initrd","/ramdisk","-dtb","/board.dtb","-append","earlycon=cdns,mmio,0xe0001000 console=ttyPS0,115200n8 ignore_loglevel loglevel=8 initcall_debug root=/dev/ram0","-trace","enable=md_sim_*"];
    let mut argv: Vec<String> = argv.into_iter().map(str::to_string).collect();
    if legacy {
        argv[2] = "md-sim-s9-legacy-io-development,boot-mode=sd".into();
        // The inherited blank NAND fixture produces a large scan trace.
        // This diagnostic records S9IO transactions, not a complete board trace.
        let filter = argv.iter().position(|v| v == "-trace").unwrap();
        argv[filter + 1] = "enable=md_sim_s9io_*".into();
    }
    if single {
        let smp = argv.iter().position(|v| v == "-smp").unwrap();
        argv[smp + 1] = "1".into();
    }
    if trace {
        argv.extend(["-d", "in_asm,int"].into_iter().map(str::to_string));
    }
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
        limits: Limits {
            wall_seconds: if trace { 5 } else { 60 },
            cpu_seconds: if trace { 4 } else { 45 },
            memory_bytes: 3 * 1024 * 1024 * 1024,
            processes: 64,
            file_bytes: 16 * 1024 * 1024,
            trace_bytes: if trace {
                32 * 1024 * 1024
            } else {
                8 * 1024 * 1024
            },
            tmp_bytes: 32 * 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    save_json(
        &out.join("execution-identity.json"),
        &json!({"build_spec":hash_file(&build.join("spec.json"))?,"build_report":hash_file(&build.join("build-report.json"))?,"run_spec":hash_file(&out.join("spec.json"))?,"firmware_archive":hash_file(&archive)?,"recipe":hash_file(&std::env::current_exe()?)?,"single_cpu_diagnostic":single,"instruction_trace_diagnostic":trace,"legacy_io_diagnostic":legacy,"device_trace_filter":if legacy {"md_sim_s9io_*"} else {"md_sim_*"},"legacy_io_scope":if legacy {Some("Archived legacy S9IO0.2 model with fixture build ID. Exact correspondence to selected2022 bitstream unresolved. Atomic MMIO; no AXI channel stalls. Inherited M69A development storage is not the resolved physical board. No ASIC chain.")} else {None},"adaptations":["Original component bytes; direct kernel entry bypasses archived bootloader/BootROM","Diagnostic UART output and ramdisk root selection","Secondary CPU parked at reset; kernel releases actual SLCR reset model","Experimental AXI IIC dynamic-master bus model; no invented I2C slaves"],"excluded_claims":["Exact physical PCB not resolved","No full original-board boot gate","No mining gate","No FPGA fabric equivalence or calibration"]}),
    )?;
    let r = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &r)?;
    let serial = hex::decode(&r.stdout)?;
    let log = hex::decode(&r.stderr)?;
    fs::write(out.join("guest.log"), &serial)?;
    fs::write(out.join("backend.log"), &log)?;
    let text = String::from_utf8_lossy(&serial);
    let backend_text = String::from_utf8_lossy(&log);
    let io_transactions: Vec<_> = backend_text
        .lines()
        .filter(|line| line.contains("md_sim_s9io_mmio "))
        .collect();
    save_json(
        &out.join("legacy-io-observations.json"),
        &json!({
            "selected":legacy,
            "transaction_count":io_transactions.len(),
            "first_transactions":io_transactions.iter().take(32).collect::<Vec<_>>(),
            "backend_log":hash_file(&out.join("backend.log"))?,
            "firmware_mining_gate":"not-passed",
            "scope":"Observed guest transactions only; no FPGA compatibility or hardware acceptance inferred."
        }),
    )?;
    let observations: Vec<_> = text
        .lines()
        .filter(|line| {
            line.contains("CPU")
                || line.contains("xiic")
                || line.contains("Linux version")
                || line.contains("Kernel panic")
        })
        .collect();
    save_json(
        &out.join("observations.json"),
        &json!({"guest_execution_lines":observations,"guest_log":hash_file(&out.join("guest.log"))?,"backend_log":hash_file(&out.join("backend.log"))?,"status":r.status,"exit_code":r.exit_code,"trace_truncated":r.trace_truncated,"acceptance":"Observations only. Complete board and mining gates remain unresolved."}),
    )?;
    ensure!(!r.trace_truncated, "trace budget exhausted");
    ensure!(
        text.contains("Linux version") || text.contains("console [ttyPS0] enabled"),
        "no kernel execution observed; inspect retained logs"
    );
    Ok(())
}

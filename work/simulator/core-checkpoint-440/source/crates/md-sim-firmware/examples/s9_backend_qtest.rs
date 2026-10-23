//! Exercise the compiled QEMU binding through private standard-input qtest IPC.
//! This is a device-model test. No guest firmware or board boot is claimed.
use anyhow::{ensure, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde_json::json;
#[path = "support/bm1387_qtest.rs"]
mod bm1387_qtest;
#[path = "support/cpu_timer_qtest.rs"]
mod cpu_timer_qtest;
#[path = "support/fabric_qtest.rs"]
mod fabric_qtest;
#[path = "support/pic16_qtest.rs"]
mod pic16_qtest;
#[path = "support/swdt_qtest.rs"]
mod swdt_qtest;
#[path = "support/ttc_decrement_qtest.rs"]
mod ttc_decrement_qtest;
#[path = "support/ttc_mode_qtest.rs"]
mod ttc_mode_qtest;
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc},
};

#[path = "support/fan_plan_qtest.rs"]
mod fan_plan_qtest;

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        (3..=4).contains(&args.len()),
        "usage: s9_backend_qtest BACKEND_BUILD NEW_OUTPUT [nand|nand-timing|nand-onfi|legacy-s9io|gpio|sd2022-s9io|s9io-v1|fan|fan-empty|fan-plan|pic16|cpu-timers|cpu-timers-clock-tree|ttc-modes|ttc-decrement|swdt|fabric|bm1387|bm1387-fabric|bm1387-burst|bm1387-temperature|bm1387-temperature-plan]"
    );
    ensure!(
        args.get(3).is_none_or(|v| v == "nand"
            || v == "nand-timing"
            || v == "nand-onfi"
            || v == "legacy-s9io"
            || v == "gpio"
            || v == "sd2022-s9io"
            || v == "s9io-v1"
            || v == "fan"
            || v == "fan-plan"
            || v == "pic16"
            || v == "cpu-timers"
            || v == "cpu-timers-clock-tree"
            || v == "ttc-modes"
            || v == "ttc-decrement"
            || v == "swdt"
            || v == "fabric"
            || v == "bm1387"
            || v == "bm1387-fabric"
            || v == "bm1387-burst"
            || v == "bm1387-temperature"
            || v == "bm1387-temperature-plan"
            || v == "fan-empty"),
        "unknown fixture"
    );
    let reference_v1 = args.get(3).is_some_and(|v| v == "s9io-v1");
    let sd2022 = reference_v1 || args.get(3).is_some_and(|v| v == "sd2022-s9io");
    let core_ns = if reference_v1 { 5 } else { 20 };
    let legacy = sd2022 || args.get(3).is_some_and(|v| v == "legacy-s9io");
    let gpio = args.get(3).is_some_and(|v| v == "gpio");
    let fan_empty = args.get(3).is_some_and(|v| v == "fan-empty");
    let fan_plan = args.get(3).is_some_and(|v| v == "fan-plan");
    let fan = fan_plan || fan_empty || args.get(3).is_some_and(|v| v == "fan");
    let pic16 = args.get(3).is_some_and(|v| v == "pic16");
    let explicit_clock_tree = args.get(3).is_some_and(|v| v == "cpu-timers-clock-tree");
    let ttc_modes = args.get(3).is_some_and(|v| v == "ttc-modes");
    let ttc_decrement = args.get(3).is_some_and(|v| v == "ttc-decrement");
    let fabric = args.get(3).is_some_and(|v| v == "fabric");
    let fabric_case = fabric_qtest::transactions();
    let swdt = args.get(3).is_some_and(|v| v == "swdt");
    let cpu_timers = fabric
        || swdt
        || ttc_decrement
        || ttc_modes
        || explicit_clock_tree
        || args.get(3).is_some_and(|v| v == "cpu-timers");
    let asic_plan = args.get(3).is_some_and(|v| v == "bm1387-temperature-plan");
    let asic_temperature = asic_plan || args.get(3).is_some_and(|v| v == "bm1387-temperature");
    let asic_burst = asic_temperature || args.get(3).is_some_and(|v| v == "bm1387-burst");
    let asic_fabric = args.get(3).is_some_and(|v| v == "bm1387-fabric");
    let bm1387 = asic_fabric || asic_burst || args.get(3).is_some_and(|v| v == "bm1387");
    let nand = args.len() == 4 && !legacy && !gpio && !fan && !pic16 && !cpu_timers && !bm1387;
    let timing = args.get(3).is_some_and(|v| v == "nand-timing");
    let onfi = args.get(3).is_some_and(|v| v == "nand-onfi");
    let build = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(&out)?;
    save_json(
        &out.join("assessment.json"),
        &json!({"passed":false,"state":"validating-inputs"}),
    )?;
    ensure!(
        fs::metadata(build.join("build-report.json"))?.len() < 16 * 1024 * 1024,
        "build report budget"
    );
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(build.join("build-report.json"))?)?;
    let binary = build.join("qemu-system-arm");
    ensure!(
        report["build"]["code"] == 0
            && hash_file(&binary)? == serde_json::from_value::<ObjectId>(report["binary"].clone())?,
        "backend identity or build differs"
    );
    let store = Store::open(&out.join("inputs"), 128 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(&store, vec![("/qemu".into(), binary)])?;
    if pic16 || bm1387 {
        let application = PathBuf::from("simulator/firmware/pic16/braiins-s9-app.txt");
        let object = hash_file(&application)?;
        ensure!(
            report["pic16_smoke"]["code"] == 0
                && object
                    == serde_json::from_value::<ObjectId>(
                        report["pic16_reference"]["application"].clone()
                    )?,
            "PIC backend or application differs"
        );
        store.import(&application, &object)?;
        inputs.push(md_sim_runtime::isolation::Mount {
            object,
            guest_path: "/pic16-app.txt".into(),
        });
    }
    if bm1387 {
        ensure!(report["bm1387_smoke"]["code"] == 0, "ASIC backend absent");
    }
    // Literal register transactions derive from the archived xiic_reinit
    // driver sequence and PG090 dynamic START. No I2C slave is attached.
    let iic_script = concat!(
        "readl 0xf8000530\n",
        "writel 0x41600040 0xa\n",
        "readb 0x41600104\n",
        "writeb 0x41600100 1\n",
        "writew 0x41600108 0x168\n",
        "readb 0x41600104\n",
        "clock_step 90000\n",
        "readb 0x41600104\n",
        "readl 0x41600020\n",
        "writel 0x41600040 0xa\n",
        "readl 0x41600020\n",
        "readb 0x41600104\n",
    );
    let (legacy_script, legacy_expected) = s9io_transactions(sd2022, reference_v1, core_ns);
    let legacy_expected: Vec<_> = legacy_expected.iter().map(String::as_str).collect();
    let pic_case = pic16_qtest::transactions();
    let pic_expected: Vec<_> = pic_case.responses.iter().map(String::as_str).collect();
    let timer_clock_tree = explicit_clock_tree || !report["ttc_clock_tree"].is_null();
    let (timer_script, timer_expected) = if fabric {
        (fabric_case.script.clone(), fabric_case.responses.clone())
    } else if swdt {
        swdt_qtest::transactions()
    } else if ttc_decrement {
        ttc_decrement_qtest::transactions()
    } else if ttc_modes {
        ttc_mode_qtest::transactions()
    } else {
        cpu_timer_qtest::transactions(timer_clock_tree)
    };
    let timer_expected: Vec<_> = timer_expected.iter().map(String::as_str).collect();
    let (asic_script, asic_expected) =
        bm1387_qtest::transactions(asic_burst, asic_temperature, asic_plan, asic_fabric);
    let asic_expected: Vec<_> = asic_expected.iter().map(String::as_str).collect();
    let script = if bm1387 {
        &asic_script
    } else if cpu_timers {
        &timer_script
    } else if pic16 {
        &pic_case.script
    } else if fan_plan {
        fan_plan_qtest::SCRIPT
    } else if fan {
        FAN_TRANSACTIONS
    } else if gpio {
        GPIO_TRANSACTIONS
    } else if legacy {
        &legacy_script
    } else if onfi {
        ONFI_TRANSACTIONS
    } else if timing {
        NAND_TIMING_TRANSACTIONS
    } else if nand {
        NAND_TRANSACTIONS
    } else {
        iic_script
    };
    let script = script.replace("PARAMETER_READS\n", &"readl 0xe1080000\n".repeat(192));
    let script_path = out.join("transactions.qtest");
    fs::write(&script_path, &script)?;
    let stdin = hash_file(&script_path)?;
    store.import(&script_path, &stdin)?;
    let argv = [
        "/qemu",
        "-machine",
        if bm1387 {
            if asic_fabric {
                "md-sim-s9-bm1387-development,fabric-clock-reference=on"
            } else {
                "md-sim-s9-bm1387-development"
            }
        } else if pic16 {
            "md-sim-s9-pic-development"
        } else if fan {
            "md-sim-s9-fan-development"
        } else if sd2022 {
            "md-sim-s9-sd2022-io-development"
        } else if gpio {
            "md-sim-s9-gpio-development"
        } else if legacy {
            "md-sim-s9-legacy-io-development"
        } else if onfi {
            "md-sim-s9-m69a-development"
        } else if nand {
            "md-sim-s9-nand-fixture"
        } else {
            "md-sim-s9-zynq-a9"
        },
        "-accel",
        "qtest",
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
        "null",
        "-qtest",
        "stdio",
        "-trace",
        "enable=md_sim_*",
    ];
    let mut argv: Vec<String> = argv.into_iter().map(str::to_string).collect();
    if asic_temperature {
        ensure!(
            report["bm1387_temperature_smoke"]["code"] == 0,
            "temperature fixture absent"
        );
        argv.extend(["-global".into(), "md-sim-s9io.tmp421-reference=on".into()]);
    }
    if asic_plan {
        ensure!(
            report["bm1387_temperature_plan_smoke"]["code"] == 0
                && report["bm1387_reference"]["temperature_input_plan_contract"]
                    == "md-sim-temperature-input-plan/1",
            "temperature input plan backend absent"
        );
        argv.extend([
            "-global".into(),
            format!(
                "md-sim-s9io.temperature-input-plan={}",
                bm1387_qtest::TEMPERATURE_PLAN
            ),
        ]);
    }
    if pic16 || bm1387 {
        argv.extend([
            "-global".into(),
            "md-sim-s9-pic16.application-text=/pic16-app.txt".into(),
        ]);
    }
    if fan {
        ensure!(report["fan_smoke"]["code"] == 0, "fan backend absent");
        if !fan_empty {
            argv.extend(["-global".into(), "md-sim-fan.reference-motors=on".into()]);
        }
        if fan_plan {
            ensure!(
                report["fan_input_plan_smoke"]["code"] == 0,
                "fan input plan backend absent"
            );
            argv.extend([
                "-global".into(),
                format!("md-sim-fan.input-plan={}", fan_plan_qtest::INPUT),
            ]);
        }
    }
    if reference_v1 {
        ensure!(
            report["s9io_v1_smoke"]["code"] == 0,
            "reference profile absent from backend"
        );
        argv.extend([
            "-global".into(),
            "md-sim-s9io.reference-v1-bm1387=on".into(),
        ]);
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
        stdin_object: Some(stdin),
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: if bm1387 { 30 } else { 10 },
            cpu_seconds: if bm1387 { 20 } else { 5 },
            memory_bytes: 1024 * 1024 * 1024,
            processes: 32,
            file_bytes: if bm1387 { 8 } else { 4 } * 1024 * 1024,
            trace_bytes: if bm1387 { 8 } else { 2 } * 1024 * 1024,
            tmp_bytes: 16 * 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    let stdout = hex::decode(&result.stdout)?;
    let stderr = hex::decode(&result.stderr)?;
    fs::write(out.join("responses.log"), &stdout)?;
    fs::write(out.join("backend.log"), &stderr)?;
    let text = String::from_utf8_lossy(&stdout);
    let lines: Vec<_> = text.lines().collect();
    let iic_expected = [
        "OK 0x0000000003722093",
        "OK",
        "OK 0x00000000000000c0",
        "OK",
        "OK",
        "OK 0x0000000000000044",
        "OK 90000",
        "OK 0x00000000000000c0",
        "OK 0x00000000000000d6",
        "OK",
        "OK 0x00000000000000d0",
        "OK 0x00000000000000c0",
    ];
    let fan_expected: Vec<_> = FAN_EXPECTED
        .iter()
        .enumerate()
        .map(|(index, value)| {
            if fan_empty && [7, 9, 11, 17, 20].contains(&index) {
                "OK 0x0000000000000000"
            } else {
                *value
            }
        })
        .collect();
    let expected: &[&str] = if bm1387 {
        &asic_expected
    } else if cpu_timers {
        &timer_expected
    } else if pic16 {
        &pic_expected
    } else if fan_plan {
        fan_plan_qtest::EXPECTED
    } else if fan {
        &fan_expected
    } else if gpio {
        GPIO_EXPECTED
    } else if legacy {
        &legacy_expected
    } else if onfi {
        ONFI_EXPECTED
    } else if timing {
        NAND_TIMING_EXPECTED
    } else if nand {
        NAND_EXPECTED
    } else {
        &iic_expected
    };
    let scope = if fabric {
        "Four SLCR fabric clocks and raw active-low reset signals. Static gates, dividers, PLL changes, locks. No FPGA equivalence or downstream reset-synchronizer pass."
    } else if bm1387 {
        "Three powered/resettable 63-chip BM1387 reference chains. Compiled AXI IIC PIC power outputs and GPIO reset pins control actual serial command/reply paths through S9IO FIFOs. Tests independent power and reset inhibition. No firmware execution, physical calibration, exact selected FPGA equivalence, mining or board acceptance claim."
    } else if swdt {
        "Zynq SWDT reference device: keyed writes, reload, four prescalers, status, disabled outputs, pulse timing and real GIC input9. No firmware, original board or physical calibration claim."
    } else if ttc_decrement {
        "All six TTC channels in decrement mode: reset to65535 or the interval value, full tick boundaries, three match events, interval reload, read acknowledgement and stopped reset. Reference device test; no physical calibration, firmware or whole-board pass."
    } else if ttc_modes {
        "All six TTC channels: matching enabled and disabled, latched status, read acknowledgement, masked event status, counter stop and documented control bit6 readback. External clock pins and waveform outputs are unconnected; no physical calibration, firmware or whole-board pass."
    } else if cpu_timers && timer_clock_tree {
        "SLCR-derived CPU and TTC clocks. Arithmetic count oracles cover global/private timers and all six TTC channels, including gating, prescalers, rate changes, fractional deadlines, stopped counters and an unconnected external source. No firmware, PLL lock calibration, full fabric clock coupling or board acceptance claim."
    } else if cpu_timers {
        "SLCR-derived CPU peripheral clocks and existing QEMU global/private timers. Arithmetic count oracles, disabled/gated clock hold, prescaler, divider change and private one-shot expiry. TTC fractional deadline regression retains upstream fixed133MHz input. No firmware, PLL lock calibration, full fabric clock coupling or board acceptance claim."
    } else if pic16 {
        "Three independent S9 PIC protocol-contract instances on the actual compiled AXI IIC bus. Exact app words, byte transactions, reset busy NACK, pointer readback, version contract and voltage state. Power-off pauses the software heartbeat; scheduled expiry cuts the actual power output and leaves application commands available. Explicit erased development storage and uncalibrated10s interval. No PIC instruction execution, factory calibration, connected board power or mining claim."
    } else if fan {
        "AM1 fan reference registers and explicit uncalibrated motor fixture. Tachometer values follow integrated motion, elapsed measurement windows, stall and power stimuli. Default absent motors stay at zero. No selected FPGA equivalence, physical calibration, thermal protection or firmware acceptance claim."
    } else if reference_v1 {
        "S9IO1.0.2 BM1387 reference-contract development model: 200MHz serializer, selected2022 address/IRQ layout, common legacy FIFO implementation. Internal timing/reset equivalence to the selected bitstream is unvalidated. Literal injected replies are component stimuli. No firmware, calibration, exact-board or mining pass."
    } else if sd2022 {
        "Selected2022 device-tree address and IRQ layout with reused legacy S9IO0.2 core. Tests compiled register, serial and interrupt routes. Core-to-bitstream correspondence remains a development hypothesis; no firmware, exact-board compatibility or mining pass."
    } else if gpio {
        "Fixed-direction9-bit input and13-bit output GPIO: actual compiled word MMIO, settled pin input, distinct output signals, width masks, TRI readback and absent registers. Pin transitions are explicit test stimuli, not hashboard detection evidence. No AXI channel or synchronizer timing, firmware execution, ASIC chain or exact-board pass."
    } else if legacy {
        "Three archived legacy S9IO0.2 blocks: actual compiled MMIO, UART waveforms, virtual timers, all nine GIC interrupt routes and byte lanes. Explicit development build ID. Atomic MMIO has no AXI channel-stall timing. Injected literal ASIC replies are component test input, not mining. No guest firmware, exact2022 bitstream mapping or physical calibration claim."
    } else if onfi {
        "Partial M69A development profile: actual compiled ID, parameter copies, independent geometry/CRC checks, real feature writes and busy intervals, unsupported ECC failure and status resume. Derived vendor parameter suffix; no physical chip identity or board boot pass."
    } else if timing {
        "Generic NAND storage timing fixture: pending program and erase abort on NAND reset; actual status polls preserve pending data; completed program survives reset. No physical power-loss or exact part calibration claim."
    } else if nand {
        "Generic NAND controller/storage fixture: command/address bytes, reset latency, ID, program/read, storage across chip reset, and block erase. No exact physical S9 NAND identity or firmware boot pass."
    } else {
        "Compiled MMIO binding, virtual byte timer, absent-device NACK, FIFO drain and software reset only. No firmware execution or board boot claim."
    };
    let irq_lines: Vec<_> = lines
        .iter()
        .copied()
        .filter(|s| s.starts_with("IRQ "))
        .collect();
    let expected_irqs: Vec<_> = if pic16 {
        (0..3)
            .flat_map(|_| ["IRQ raise 0".to_string(), "IRQ lower 0".to_string()])
            .collect()
    } else if gpio {
        ["raise", "lower"]
            .into_iter()
            .flat_map(|level| [0, 4, 12].map(|pin| format!("IRQ {level} {pin}")))
            .collect()
    } else {
        (if sd2022 {
            [30, 32, 31, 33, 35, 34, 36, 53, 52]
        } else {
            [35, 52, 36, 53, 55, 54, 56, 58, 57]
        })
        .into_iter()
        .flat_map(|irq| [format!("IRQ raise {irq}"), format!("IRQ lower {irq}")])
        .collect()
    };
    let observes_irqs = legacy || gpio || pic16;
    let irq_verified = observes_irqs
        .then(|| irq_lines == expected_irqs.iter().map(String::as_str).collect::<Vec<_>>());
    let mut compared = if observes_irqs {
        lines
            .iter()
            .copied()
            .filter(|s| !s.starts_with("IRQ "))
            .collect()
    } else {
        lines.clone()
    };
    let parameter_verified = if onfi {
        let checked = lines.get(10..202).is_some_and(verify_parameters);
        if checked {
            compared.splice(10..202, ["PARAMETER_PAGE_VERIFIED"]);
        }
        Some(checked)
    } else {
        None
    };
    let serial_verified = legacy.then(|| {
        verify_s9io_serial(
            &String::from_utf8_lossy(&stderr),
            if sd2022 { 6 } else { 5 },
            core_ns,
            !sd2022 && !report["fabric_outputs"].is_null(),
        )
    });
    let pic_trace_verified = pic16.then(|| {
        let log = String::from_utf8_lossy(&stderr);
        pic_case.verify_power(&log)
            && [0x55, 0x56, 0x57].into_iter().all(|address| {
                log.contains(&format!(
                    "md_sim_axi_iic_start address=0x{address:02x} read=1 result=0"
                ))
            })
            && log
                .lines()
                .filter(|line| line.starts_with("md_sim_pic_event "))
                .all(|line| line.ends_with("result=0x0") || line.ends_with("result=0x1"))
            && log
                .lines()
                .filter(|line| line.starts_with("md_sim_axi_iic_mmio write=0 addr=0x10c "))
                .all(|line| line.ends_with("faults=0"))
    });
    let asic_trace_verified = bm1387.then(|| {
        bm1387_qtest::verify(
            &String::from_utf8_lossy(&stderr),
            asic_burst,
            asic_temperature,
            asic_plan,
            asic_fabric,
        )
    });
    let fan_plan_verified =
        fan_plan.then(|| fan_plan_qtest::verify(&String::from_utf8_lossy(&stderr)));
    let fabric_verified = fabric.then(|| fabric_case.verify(&String::from_utf8_lossy(&stderr)));
    let passed = compared == expected
        && fabric_verified.unwrap_or(true)
        && fan_plan_verified.unwrap_or(true)
        && parameter_verified.unwrap_or(true)
        && serial_verified.unwrap_or(true)
        && irq_verified.unwrap_or(true)
        && pic_trace_verified.unwrap_or(true)
        && asic_trace_verified.unwrap_or(true)
        && !result.trace_truncated
        && result.status == "timed-out"
        && result.exit_code.is_none();
    save_json(
        &out.join("assessment.json"),
        &json!({
            "passed":passed, "parameter_verified":parameter_verified,
            "serial_verified":serial_verified, "irq_verified":irq_verified,
            "pic_trace_verified":pic_trace_verified, "irq_trace":irq_lines,
            "expected_pic_power_ns_state":if pic16 {Some(&pic_case.power)} else {None},
            "asic_trace_verified":asic_trace_verified,
            "fan_input_plan_verified":fan_plan_verified,
            "fabric_verified":fabric_verified,
            "fabric_case":if fabric {Some(&fabric_case)} else {None},
            "fabric_test_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/fabric_qtest.rs")),
            "asic_address_burst":asic_burst,
            "asic_fabric_reference":asic_fabric,
            "asic_temperature_fixture":asic_temperature,
            "expected_irq_trace":if observes_irqs {Some(&expected_irqs)} else {None},
            "expected_responses":expected, "actual_responses":lines,
            "build_report":hash_file(&build.join("build-report.json"))?,
            "spec":hash_file(&out.join("spec.json"))?,
            "result":hash_file(&out.join("result.json"))?,
            "backend_log":hash_file(&out.join("backend.log"))?,
            "responses":hash_file(&out.join("responses.log"))?,
            "probe_build":hash_file(std::path::Path::new("/proc/self/exe"))?,
            "probe_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_backend_qtest.rs")),
            "pic16_script_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/pic16_qtest.rs")),
            "cpu_timer_script_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/cpu_timer_qtest.rs")),
            "ttc_mode_script_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/ttc_mode_qtest.rs")),
            "ttc_decrement_script_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/ttc_decrement_qtest.rs")),
            "swdt_script_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/swdt_qtest.rs")),
            "bm1387_script_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/bm1387_qtest.rs")),
            "fan_input_script_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/fan_plan_qtest.rs")),
            "scope":scope, "timeout_is_expected":true,
            "process_status":result.status, "exit_code":result.exit_code,
            "termination":"The worker deadline ends qtest after the finite input stream. Acceptance requires every exact protocol response and the expected timeout disposition; a backend crash cannot pass."
        }),
    )?;
    ensure!(
        passed,
        "qtest responses differ; retained logs describe the failure"
    );
    Ok(())
}

fn verify_s9io_serial(log: &str, first_link: u32, core_ns: u64, fabric: bool) -> bool {
    let mut links: BTreeMap<u32, Vec<(u64, u8)>> = BTreeMap::new();
    let mut complete_commands = std::collections::BTreeSet::new();
    for line in log.lines() {
        let field = |prefix| line.split_whitespace().find_map(|s| s.strip_prefix(prefix));
        if core_ns == 5 && line.starts_with("md_sim_s9io_mmio ") {
            if field("write=") == Some("1")
                && field("addr=") == Some("0x1004")
                && field("value=")
                    .and_then(|value| u32::from_str_radix(value.strip_prefix("0x")?, 16).ok())
                    == Some(553_805_824)
            {
                let Some(link) = field("link=").and_then(|value| value.parse::<u32>().ok()) else {
                    return false;
                };
                complete_commands.insert(link);
            }
            continue;
        }
        if !line.starts_with("md_sim_s9io_event ") {
            continue;
        }
        let parsed = (|| {
            let link = field("link=")?.parse::<u32>().ok()?;
            let ns = field("virtual_ns=")?.parse::<u64>().ok()?;
            let flags = u32::from_str_radix(field("flags=0x")?, 16).ok()?;
            let byte = u8::from_str_radix(field("tx=0x")?, 16).ok()?;
            Some((link, ns, flags, byte))
        })();
        let Some((link, ns, flags, byte)) = parsed else {
            return false;
        };
        if flags & 1 != 0 {
            if core_ns == 5 && !complete_commands.contains(&link) {
                return false;
            }
            links.entry(link).or_default().push((ns, byte));
        }
    }
    let divisors = if core_ns == 5 {
        [108, 7, 3]
    } else {
        [0, 1, 13]
    };
    links.len() == 3
        && divisors.into_iter().enumerate().all(|(index, divisor)| {
            let link = first_link + index as u32;
            let Some(events) = links.get(&link) else {
                return false;
            };
            let interval = 161 * (divisor + 1) * core_ns;
            let spaced = |events: &[(u64, u8)]| {
                if fabric {
                    // One unknown initial fractional phase must explain every
                    // byte timestamp. This is exact rounding, not accumulated
                    // tolerance per byte. The bit timing is161 divider ticks.
                    let period = ((1_000_000_000u64 << 32) / 33_333_333 / 30) * 20;
                    let step = u128::from(period) * 161 * (divisor + 1) as u128;
                    let mut low = 0;
                    let mut high = u128::MAX;
                    for (n, (time, _)) in events.iter().enumerate() {
                        let delta = n as u128 * step;
                        let upper = u128::from(*time) << 32;
                        if upper < delta {
                            return false;
                        }
                        low = low.max(
                            upper
                                .saturating_sub(u128::from(u32::MAX))
                                .saturating_sub(delta),
                        );
                        high = high.min(upper - delta);
                    }
                    return low <= high;
                }
                events
                    .windows(2)
                    .all(|pair| pair[1].0.checked_sub(pair[0].0) == Some(interval))
            };
            if core_ns == 5 {
                events.iter().map(|(_, byte)| *byte).collect::<Vec<_>>()
                    == [72, 9, 0, 12, 0, 104, 2, 33, 2, 84, 5, 0, 0, 25]
                    && spaced(&events[..9])
                    && spaced(&events[9..])
            } else {
                events.iter().map(|(_, byte)| *byte).collect::<Vec<_>>() == [84, 5, 0, 0, 25]
                    && spaced(events)
            }
        })
}

fn s9io_transactions(sd2022: bool, reference_v1: bool, core_ns: u64) -> (String, Vec<String>) {
    struct Script {
        text: String,
        expected: Vec<String>,
        ns: u64,
        core_ns: u64,
    }
    impl Script {
        fn command(&mut self, command: String, response: String) {
            self.text.push_str(&command);
            self.text.push('\n');
            self.expected.push(response);
        }
        fn write(&mut self, address: u64, value: u32) {
            self.command(format!("writel {address:#x} {value:#x}"), "OK".into());
        }
        fn read(&mut self, address: u64, value: u32) {
            self.command(format!("readl {address:#x}"), format!("OK 0x{value:016x}"));
        }
        fn step(&mut self, ns: u64) {
            self.ns += ns;
            self.command(format!("clock_step {ns}"), format!("OK {}", self.ns));
        }
        fn serial(&mut self, link: u32, divisor: u32, bytes: &[u8]) {
            // Independent8N1 generator, not the model transmitter.
            for byte in bytes {
                for bit in 0..10 {
                    let level = match bit {
                        0 => false,
                        9 => true,
                        _ => byte & (1 << (bit - 1)) != 0,
                    };
                    self.command(
                        format!("set_irq_in /machine/s9io{link} rxd 0 {}", u8::from(level)),
                        "OK".into(),
                    );
                    self.step(16 * (u64::from(divisor) + 1) * self.core_ns);
                }
            }
            self.step(10000);
        }
    }
    let mut s = Script {
        text: String::new(),
        expected: vec![],
        ns: 0,
        core_ns,
    };
    // qtest's MMIO attributes are non-secure. Reset GIC Group0 pending
    // registers therefore read as zero. Observe the real GIC input lines
    // without changing security state. QOM must resolve this unique child.
    // CPU exception delivery still requires a separate guest execution test.
    s.command("irq_intercept_in gic".into(), "OK".into());
    if !sd2022 {
        // Archived legacy FCLK1: IO PLL30, divisors5 and4. The33333333Hz
        // source gives20.0000002ns, retained in the serial timing oracle.
        s.write(0xf8000008, 0xdf0d);
        s.write(0xf8000108, 30 << 12);
        s.write(0xf8000180, 0x00400500);
    }
    // Reference driver divisors for enumeration and its two operating rates.
    let divisors = if reference_v1 {
        [108, 7, 3]
    } else {
        [0, 1, 13]
    };
    for (index, divisor) in divisors.into_iter().enumerate() {
        let link = index as u32 + if sd2022 { 6 } else { 5 };
        let base = if sd2022 { 0x43c00000 } else { 0x43c50000 } + index as u64 * 0x10000;
        s.read(base, if reference_v1 { 0x00901002 } else { 0x00900200 });
        s.read(base + 4, 0x74657374);
        s.read(base + 0x100c, 5);
        // The guest enables the reset-default timer before configuring it.
        // One virtual second must retain its exact50MHz state without
        // fifty million unobservable empty-queue timer callbacks.
        s.write(base + 8, 8);
        s.step(1_000_000_000);
        s.read(base + 0x14, 1);
        s.read(base + 0x100c, 5);
        s.write(base + 8, 0);
        s.step(1000);
        s.write(base + 0x10, 0x12345678);
        s.command(format!("writeb {:#x} 0xaa", base + 0x12), "OK".into());
        s.read(base + 0xc010, 0x12aa5678);
        s.write(base + 0x10, divisor);
        s.write(base + 0x14, 257);
        s.write(base + 8, 8);
        s.write(base + 0x1008, 4);
        s.write(base + 0x2008, 4);
        s.step(10000);
        s.write(base + 0x3008, 4);
        s.write(base + 0x3008, 0);
        if reference_v1 {
            // The CPU can be preempted between two MMIO writes. Preserve
            // the first command word across a full second of virtual time.
            // The trace verifier requires the complete archived nine-byte
            // packet, followed by the existing five-byte query.
            s.write(base + 0x1004, 201_328_968);
            s.step(1_000_000_000);
            s.write(base + 0x1004, 553_805_824);
            s.step(1_000_000);
            s.read(base + 0x100c, 5);
        }
        s.write(base + 0x1004, 1364);
        s.step(500000);
        s.read(base + 0x100c, 5);
        s.serial(link, divisor, &[19, 135, 144, 0, 0, 0, 7]);
        s.read(base + 0x100c, 20);
        s.read(base + 0x1000, 0x00908713);
        s.read(base + 0x1000, 0x07000000);
        s.serial(link, divisor, &[114, 3, 234, 131, 0, 3, 152]);
        s.read(base + 0x200c, 16);
        s.read(base + 0x2000, 0x83ea0372);
        // No work has been transmitted: received ID3 belongs to the prior
        // modulo128 window relative to LAST_WORK_ID0, hence extendedff83.
        s.read(base + 0x2000, 0x98ff8300);
        s.serial(link, divisor, &[18, 135, 144, 0, 0, 0, 7]);
        s.read(base + 0x18, 1);
        s.write(base + 8, 9);
        s.step(1000);
        s.read(base + 0x18, 0);
        s.write(base + 8, 0);
        s.step(1000);
        s.read(base + 0x1000, 0x07000000);
        // QEMU rejects clock_step0. clock_set at the current time checks
        // the same fixed timestamp without requesting a positive step.
        s.command(format!("clock_set {}", s.ns), format!("OK {}", s.ns));
    }
    (s.text, s.expected)
}

fn main() -> Result<()> {
    let out = std::env::args_os().nth(2).map(PathBuf::from);
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

// Independent motion oracle: 6000RPM reached after0.5s with12000RPM/s.
// First second contains75 revolutions; steady windows100. After power loss
// the half-second ramp down contributes25 revolutions. Stall contributes0.
const FAN_TRANSACTIONS: &str = concat!(
    "readl 0x42800000\n",
    "readl 0x42800004\n",
    "writeb 0x42800014 100\n",
    "readl 0x42800014\n",
    "clock_step 999999999\n",
    "readl 0x42800004\n",
    "clock_step 1\n",
    "readl 0x42800004\n",
    "clock_step 2000000000\n",
    "readl 0x42800004\n",
    "writel 0x42800004 999\n",
    "readl 0x42800004\n",
    "set_irq_in /machine/fan stall 1 1\n",
    "clock_step 1000000000\n",
    "readl 0x42800004\n",
    "set_irq_in /machine/fan stall 1 0\n",
    "clock_step 2000000000\n",
    "readl 0x42800004\n",
    "set_irq_in /machine/fan power 1 0\n",
    "clock_step 1000000000\n",
    "readl 0x42800004\n",
    "clock_step 1000000000\n",
    "readl 0x42800004\n",
    "writeb 0x42800015 255\n",
    "readl 0x42800014\n",
);
const FAN_EXPECTED: &[&str] = &[
    "OK 0x0000000000000000",
    "OK 0x0000000000000000",
    "OK",
    "OK 0x0000000000000064",
    "OK 999999999",
    "OK 0x0000000000000000",
    "OK 1000000000",
    "OK 0x000000000000004b",
    "OK 3000000000",
    "OK 0x0000000000000064",
    "OK",
    "OK 0x0000000000000064",
    "OK",
    "OK 4000000000",
    "OK 0x0000000000000000",
    "OK",
    "OK 6000000000",
    "OK 0x0000000000000064",
    "OK",
    "OK 7000000000",
    "OK 0x0000000000000019",
    "OK 8000000000",
    "OK 0x0000000000000000",
    "OK",
    "OK 0x0000000000000064",
];

const GPIO_TRANSACTIONS: &str = concat!(
    "irq_intercept_out /machine/gpio-output output\n",
    "readl 0x41200000\n",
    "readl 0x41200004\n",
    "readl 0x41210004\n",
    "set_irq_in /machine/gpio-input input 5 1\n",
    "set_irq_in /machine/gpio-input input 6 1\n",
    "set_irq_in /machine/gpio-input input 7 1\n",
    "readl 0x41200000\n",
    "readl 0x41200200\n",
    "writel 0x41200000 0xffffffff\n",
    "readl 0x41200000\n",
    "set_irq_in /machine/gpio-input input 6 0\n",
    "readl 0x41200000\n",
    "writel 0x41200004 0\n",
    "readl 0x41200004\n",
    "readl 0x41200000\n",
    "writel 0x41210000 0xfffd1011\n",
    "readl 0x41210000\n",
    "writel 0x41210004 0xffff0f0f\n",
    "readl 0x41210004\n",
    "writel 0x41210128 0xffffffff\n",
    "readl 0x41210128\n",
    "readl 0x41210008\n",
    "writel 0x41210200 0\n",
    "readl 0x41210000\n",
);
const GPIO_EXPECTED: &[&str] = &[
    "OK",
    "OK 0x0000000000000000",
    "OK 0x00000000000001ff",
    "OK 0x0000000000001fff",
    "OK",
    "OK",
    "OK",
    "OK 0x00000000000000e0",
    "OK 0x00000000000000e0",
    "OK",
    "OK 0x00000000000000e0",
    "OK",
    "OK 0x00000000000000a0",
    "OK",
    "OK 0x0000000000000000",
    "OK 0x00000000000000a0",
    "OK",
    "OK 0x0000000000000000",
    "OK",
    "OK 0x0000000000000f0f",
    "OK",
    "OK 0x0000000000000000",
    "OK 0x0000000000000000",
    "OK",
    "OK 0x0000000000000000",
];

// Independent wire-level vector, executed against the compiled legacy storage engine.
const NAND_TRANSACTIONS: &str = concat!(
    "writel 0xe10007f8 0\n",
    "readl 0xe000e000\n",
    "clock_step 5000\n",
    "readl 0xe000e000\n",
    "writel 0xe000e00c 0x10\n",
    "writel 0xe1200480 0\n",
    "readb 0xe1080000\n",
    "readb 0xe1080000\n",
    "readb 0xe1080000\n",
    "readb 0xe1080000\n",
    "writel 0xe1a08400 0\n",
    "writel 0xe1a08400 0\n",
    "writel 0xe1088000 0x89abcdef\n",
    "writel 0xe1388000 0x01234567\n",
    "readl 0xe000e000\n",
    "clock_step 300000\n",
    "readl 0xe000e000\n",
    "writel 0xe000e00c 0x10\n",
    "writel 0xe1b18000 0\n",
    "writel 0xe1b18000 0\n",
    "clock_step 25000\n",
    "readl 0xe1080000\n",
    "readl 0xe1280000\n",
    "writel 0xe10007f8 0\n",
    "clock_step 5000\n",
    "writel 0xe1b18000 0\n",
    "writel 0xe1b18000 0\n",
    "clock_step 25000\n",
    "readl 0xe1080000\n",
    "readl 0xe1280000\n",
    "writel 0xe1768300 0\n",
    "clock_step 2000000\n",
    "writel 0xe1b18000 0\n",
    "writel 0xe1b18000 0\n",
    "clock_step 25000\n",
    "readl 0xe1080000\n",
    "readl 0xe1280000\n",
);
const NAND_EXPECTED: &[&str] = &[
    "OK",
    "OK 0x0000000000000000",
    "OK 5000",
    "OK 0x0000000000000040",
    "OK",
    "OK",
    "OK 0x00000000000000ec",
    "OK 0x00000000000000da",
    "OK 0x0000000000000051",
    "OK 0x0000000000000015",
    "OK",
    "OK",
    "OK",
    "OK",
    "OK 0x0000000000000000",
    "OK 305000",
    "OK 0x0000000000000040",
    "OK",
    "OK",
    "OK",
    "OK 330000",
    "OK 0x0000000089abcdef",
    "OK 0x0000000001234567",
    "OK",
    "OK 335000",
    "OK",
    "OK",
    "OK 360000",
    "OK 0x0000000089abcdef",
    "OK 0x0000000001234567",
    "OK",
    "OK 2360000",
    "OK",
    "OK",
    "OK 2385000",
    "OK 0x00000000ffffffff",
    "OK 0x00000000ffffffff",
];

const NAND_TIMING_TRANSACTIONS: &str = concat!(
    "writel 0xe10007f8 0\n",
    "clock_step 5000\n",
    "writel 0xe1a08400 0\n",
    "writel 0xe1a08400 0\n",
    "writel 0xe1388000 0x11223344\n",
    "writel 0xe1000380 0\n",
    "readb 0xe1080000\n",
    "clock_step 100000\n",
    "readb 0xe1080000\n",
    "writel 0xe10007f8 0\n",
    "clock_step 5000\n",
    "writel 0xe1b18000 0\n",
    "writel 0xe1b18000 0\n",
    "clock_step 25000\n",
    "readl 0xe1280000\n",
    "writel 0xe1a08400 0\n",
    "writel 0xe1a08400 0\n",
    "writel 0xe1388000 0xaabbccdd\n",
    "writel 0xe1000380 0\n",
    "readb 0xe1080000\n",
    "clock_step 300000\n",
    "readb 0xe1080000\n",
    "writel 0xe1b18000 0\n",
    "writel 0xe1b18000 0\n",
    "clock_step 25000\n",
    "readl 0xe1280000\n",
    "writel 0xe1768300 0\n",
    "clock_step 1000000\n",
    "writel 0xe1000380 0\n",
    "readb 0xe1080000\n",
    "writel 0xe10007f8 0\n",
    "clock_step 5000\n",
    "writel 0xe1b18000 0\n",
    "writel 0xe1b18000 0\n",
    "clock_step 25000\n",
    "readl 0xe1280000\n",
);
const NAND_TIMING_EXPECTED: &[&str] = &[
    "OK",
    "OK 5000",
    "OK",
    "OK",
    "OK",
    "OK",
    "OK 0x0000000000000080",
    "OK 105000",
    "OK 0x0000000000000080",
    "OK",
    "OK 110000",
    "OK",
    "OK",
    "OK 135000",
    "OK 0x00000000ffffffff",
    "OK",
    "OK",
    "OK",
    "OK",
    "OK 0x0000000000000080",
    "OK 435000",
    "OK 0x00000000000000c0",
    "OK",
    "OK",
    "OK 460000",
    "OK 0x00000000aabbccdd",
    "OK",
    "OK 1460000",
    "OK",
    "OK 0x0000000000000080",
    "OK",
    "OK 1465000",
    "OK",
    "OK",
    "OK 1490000",
    "OK 0x00000000aabbccdd",
];

fn verify_parameters(lines: &[&str]) -> bool {
    let mut bytes = Vec::with_capacity(768);
    for line in lines {
        let Some(encoded) = line.strip_prefix("OK 0x") else {
            return false;
        };
        let Ok(word) = u32::from_str_radix(encoded, 16) else {
            return false;
        };
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    if bytes.len() != 768 || bytes[..256] != bytes[256..512] || bytes[..256] != bytes[512..] {
        return false;
    }
    let p = &bytes[..256];
    if &p[..4] != b"ONFI"
        || p[4] != 2
        || &p[32..44] != b"MICRON      "
        || &p[44..64] != b"MT29F2G08ABAEAWP    "
        || p[64] != 0x2c
        || p[80..84] != [0, 8, 0, 0]
        || p[84..86] != [64, 0]
        || p[92..96] != [64, 0, 0, 0]
        || p[96..100] != [0, 8, 0, 0]
        || p[100..103] != [1, 0x23, 1]
    {
        return false;
    }
    // Independent polynomial division over individual transmitted bits.
    let mut rem = 0x4f4eu32;
    for byte in &p[..254] {
        for bit in (0..8).rev() {
            let feedback = ((rem >> 15) ^ u32::from((byte >> bit) & 1)) & 1;
            rem = ((rem << 1) & 0xffff) ^ (feedback * 0x8005);
        }
    }
    rem as u16 == u16::from_le_bytes([p[254], p[255]])
}
const ONFI_TRANSACTIONS: &str = concat!(
    "writel 0xe10007f8 0\n",
    "clock_step 5000\n",
    "writel 0xe1200480 0\n",
    "readl 0xe1080000\n",
    "readb 0xe1080000\n",
    "writel 0xe1200480 0x20\n",
    "readl 0xe1080000\n",
    "writel 0xe1200760 0\n",
    "readl 0xe000e000\n",
    "clock_step 25000\n",
    "PARAMETER_READS\n",
    "writel 0xe1200770 0x90\n",
    "clock_step 1000\n",
    "readl 0xe1080000\n",
    "writel 0xe000e00c 0x10\n",
    "writel 0xe1200778 0x90\n",
    "writew 0xe1080000 0\n",
    "readl 0xe000e000\n",
    "writew 0xe1080000 0\n",
    "clock_step 999\n",
    "readl 0xe000e000\n",
    "clock_step 1\n",
    "readl 0xe000e000\n",
    "writel 0xe1200778 0x90\n",
    "writel 0xe1080000 8\n",
    "clock_step 1000\n",
    "writel 0xe1000380 0\n",
    "readb 0xe1080000\n",
    "writel 0xe1200770 0x90\n",
    "clock_step 1000\n",
    "readl 0xe1080000\n",
    "writel 0xe10007f8 0\n",
    "clock_step 5000\n",
    "writel 0xe1200760 0\n",
    "clock_step 25000\n",
    "readb 0xe1080000\n",
    "writel 0xe1000380 0\n",
    "readb 0xe1080000\n",
    "writel 0xe1000000 0\n",
    "readb 0xe1080000\n",
    "writel 0xe1570028 0x165\n",
    "readb 0xe1080000\n",
    "writel 0xe1570028 0x300\n",
    "readb 0xe1080000\n",
    "writel 0xe1000380 0\n",
    "readb 0xe1080000\n",
);
const ONFI_EXPECTED: &[&str] = &[
    "OK",
    "OK 5000",
    "OK",
    "OK 0x000000009590da2c",
    "OK 0x0000000000000006",
    "OK",
    "OK 0x0000000049464e4f",
    "OK",
    "OK 0x0000000000000000",
    "OK 30000",
    "PARAMETER_PAGE_VERIFIED",
    "OK",
    "OK 31000",
    "OK 0x0000000000000000",
    "OK",
    "OK",
    "OK",
    "OK 0x0000000000000000",
    "OK",
    "OK 31999",
    "OK 0x0000000000000000",
    "OK 32000",
    "OK 0x0000000000000040",
    "OK",
    "OK",
    "OK 33000",
    "OK",
    "OK 0x00000000000000c1",
    "OK",
    "OK 34000",
    "OK 0x0000000000000000",
    "OK",
    "OK 39000",
    "OK",
    "OK 64000",
    "OK 0x000000000000004f",
    "OK",
    "OK 0x00000000000000c0",
    "OK",
    "OK 0x000000000000004e",
    "OK",
    "OK 0x0000000000000023",
    "OK",
    "OK 0x00000000000000ff",
    "OK",
    "OK 0x00000000000000c1",
];

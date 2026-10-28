//! Exercise actual QMP system reset and all six TTC channels in isolation.
//! This checks reference device reset, not firmware or physical-board recovery.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
};
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "support/debug_reference_qtest.rs"]
mod debug_reference_qtest;
#[path = "support/fabric_reset.rs"]
mod fabric_reset;
#[path = "support/fabric_uart_qtest.rs"]
mod fabric_uart_qtest;
#[path = "support/pcap_migration_qtest.rs"]
mod pcap_migration_qtest;
#[path = "support/pcap_qtest.rs"]
mod pcap_qtest;
#[path = "support/ps_gpio_qtest.rs"]
mod ps_gpio_qtest;
#[path = "support/rom_reset_qtest.rs"]
mod rom_reset_qtest;
#[path = "support/swdt_reset.rs"]
mod swdt_reset;

struct Backend(Child);
impl Drop for Backend {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn line(input: &mut impl BufRead) -> Result<String> {
    let mut bytes = Vec::new();
    input.take(16_385).read_until(b'\n', &mut bytes)?;
    ensure!(
        bytes.len() <= 16_384 && bytes.last() == Some(&b'\n'),
        "backend line absent or exceeds bound"
    );
    Ok(String::from_utf8(bytes)?.trim_end().to_owned())
}

fn qmp(
    write: &mut impl Write,
    read: &mut impl BufRead,
    id: u32,
    command: &str,
    events: &mut Vec<Value>,
) -> Result<Value> {
    serde_json::to_writer(&mut *write, &json!({"execute":command,"id":id}))?;
    writeln!(write)?;
    write.flush()?;
    for _ in 0..16 {
        let value: Value = serde_json::from_str(&line(read)?)?;
        println!("MD_SIM_TTC_RESET_QMP {value}");
        if value["event"].is_string() {
            ensure!(events.len() < 32, "QMP event bound");
            events.push(value);
            continue;
        }
        ensure!(
            value["id"] == id && value.get("return").is_some(),
            "QMP command failed: {value}"
        );
        return Ok(value["return"].clone());
    }
    anyhow::bail!("QMP reply absent")
}

struct Transactions {
    input: BufReader<File>,
    output: File,
    records: Vec<Value>,
    irq_events: Vec<(u32, bool)>,
    now: u64,
}
impl Transactions {
    fn command(&mut self, command: String, expected: String) -> Result<()> {
        self.command_recorded(command, expected, false)
    }
    fn command_recorded(&mut self, command: String, expected: String, compact: bool) -> Result<()> {
        writeln!(self.output, "{command}")?;
        self.output.flush()?;
        let mut actual = line(&mut self.input)?;
        for _ in 0..32 {
            let Some(event) = actual.strip_prefix("IRQ ") else {
                break;
            };
            let (direction, index) = event.split_once(' ').context("IRQ fields absent")?;
            ensure!(matches!(direction, "raise" | "lower"), "invalid IRQ event");
            ensure!(self.irq_events.len() < 64, "IRQ event bound");
            self.irq_events.push((index.parse()?, direction == "raise"));
            println!("MD_SIM_TTC_RESET_IRQ {event}");
            actual = line(&mut self.input)?;
        }
        // Emit actual observations before an assertion can stop this probe.
        let record = if compact && actual == expected {
            json!({"command":command,
                "actual_sha256":md_sim_firmware::prepare::hash_bytes(actual.as_bytes()),
                "expected_sha256":md_sim_firmware::prepare::hash_bytes(expected.as_bytes()),
                "actual_bytes":actual.len(),"expected_bytes":expected.len(),"equal":true})
        } else {
            json!({"command":command,"actual":actual,"expected":expected})
        };
        println!("MD_SIM_TTC_RESET_TRANSACTION {record}");
        self.records.push(record);
        ensure!(
            actual == expected,
            "transaction differs: {command}: {actual}, expected {expected}"
        );
        Ok(())
    }
    fn write(&mut self, address: u32, value: u32) -> Result<()> {
        self.command(format!("writel {address:#x} {value:#x}"), "OK".into())
    }
    fn read(&mut self, address: u32, value: u64) -> Result<()> {
        self.command(format!("readl {address:#x}"), format!("OK 0x{value:016x}"))
    }
    fn advance(&mut self, ns: u64) -> Result<()> {
        self.now += ns;
        self.command(format!("clock_step {ns}"), format!("OK {}", self.now))
    }
    fn reset_values(&mut self) -> Result<()> {
        for base in [0xf8001000, 0xf8002000] {
            for channel in 0..3 {
                for offset in [0, 0xc, 0x18, 0x24, 0x30, 0x3c, 0x48, 0x54, 0x60, 0x6c] {
                    self.read(
                        base + channel * 4 + offset,
                        if offset == 0xc { 0x21 } else { 0 },
                    )?;
                }
            }
        }
        Ok(())
    }
    fn irq_state(&self, raised: bool, cycle: u32) -> Result<()> {
        // Zynq TTC interrupts42..44 and69..71 enter the GIC at SPI offsets
        //10..12 and37..39. Require each real wire to alternate at every reset.
        let pins = [10, 11, 12, 37, 38, 39];
        ensure!(
            self.irq_events.iter().all(|(pin, _)| pins.contains(pin)),
            "unexpected GIC input transition"
        );
        let expected = [true, false].repeat((cycle + 1) as usize);
        let count = expected.len() - usize::from(raised);
        for pin in pins {
            let actual: Vec<_> = self
                .irq_events
                .iter()
                .filter(|(index, _)| *index == pin)
                .map(|(_, state)| *state)
                .collect();
            ensure!(actual == expected[..count], "TTC IRQ{pin} history differs");
        }
        Ok(())
    }
}

fn inner(kind: &str) -> Result<()> {
    if kind == "pcap-migration" {
        let result = pcap_migration_qtest::run_inside()?;
        println!("MD_SIM_TTC_RESET_RESULT {result}");
        return Ok(());
    }
    let swdt = kind == "swdt";
    let fabric = kind == "fabric";
    let uart = kind == "fabric-uart";
    let rom = matches!(
        kind,
        "rom-map"
            | "rom-swap-reject"
            | "rom-reason"
            | "rom-lvttl"
            | "rom-gpio"
            | "rom-gpio-reject"
            | "rom-debug-reference"
            | "rom-debug-reference-reject"
    );
    let pcap_case = pcap_qtest::Case::from_kind(kind);
    let pcap = pcap_case.is_some();
    confinement::require(false)?;
    // Both FIFOs exist only in the run-owned scratch mount. The enclosing
    // process deadline bounds blocked I/O. No network socket is required.
    for path in [c"/work/ttc-test.in", c"/work/ttc-test.out"] {
        ensure!(
            unsafe { libc::mkfifo(path.as_ptr(), 0o600) } == 0,
            "private FIFO creation failed: {}",
            std::io::Error::last_os_error()
        );
    }
    let output = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/work/ttc-test.in")?;
    let input = BufReader::new(
        OpenOptions::new()
            .read(true)
            .write(true)
            .open("/work/ttc-test.out")?,
    );
    let mut command = Command::new("/qemu");
    command
        .args([
            "-machine",
            if rom {
                "md-sim-s9-zynq-a9,boot-mode=sd,bootrom-reference=on"
            } else if uart {
                "md-sim-s9-legacy-io-development"
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
            "-trace",
            "enable=md_sim_*",
            "-qmp",
            "stdio",
            "-qtest",
            "pipe:/work/ttc-test",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if rom {
        command.args(["-bios", "/public-rom.bin", "-qtest-log", "/dev/null"]);
    }
    if pcap {
        command.args([
            "-global",
            "driver=xlnx.ps7-dev-cfg,property=md-bitstream,value=/pcap-reference.bit",
            "-global",
            "driver=xlnx.ps7-dev-cfg,property=md-part,value=/pcap-part.json",
            "-device",
            "loader,file=/pcap-dma.bin,addr=0x100000",
        ]);
    }
    let mut backend = Backend(command.spawn()?);
    let mut qmp_input = BufReader::new(backend.0.stdout.take().context("QMP stdout absent")?);
    let mut qmp_output = backend.0.stdin.take().context("QMP stdin absent")?;
    let greeting: Value = serde_json::from_str(&line(&mut qmp_input)?)?;
    ensure!(greeting["QMP"].is_object(), "QMP greeting absent");
    let mut events = Vec::new();
    qmp(
        &mut qmp_output,
        &mut qmp_input,
        1,
        "qmp_capabilities",
        &mut events,
    )?;
    let mut io = Transactions {
        input,
        output,
        records: Vec::new(),
        irq_events: Vec::new(),
        now: 0,
    };
    io.command("irq_intercept_in gic".into(), "OK".into())?;
    if kind == "rom-gpio" || kind == "rom-gpio-reject" {
        ps_gpio_qtest::run(
            &mut io,
            &mut qmp_output,
            &mut qmp_input,
            &mut events,
            kind == "rom-gpio-reject",
        )?;
    } else if kind == "rom-debug-reference" || kind == "rom-debug-reference-reject" {
        debug_reference_qtest::run(
            &mut io,
            &mut qmp_output,
            &mut qmp_input,
            &mut events,
            kind == "rom-debug-reference-reject",
        )?;
    } else if kind == "rom-reason" {
        rom_reset_qtest::run_reason(&mut io, &mut qmp_output, &mut qmp_input, &mut events)?;
    } else if kind == "rom-lvttl" {
        rom_reset_qtest::run_lvttl(&mut io, &mut qmp_output, &mut qmp_input, &mut events)?;
    } else if rom {
        rom_reset_qtest::run(
            &mut io,
            &mut qmp_output,
            &mut qmp_input,
            &mut events,
            kind == "rom-swap-reject",
        )?;
    } else if let Some(case) = pcap_case {
        pcap_qtest::run(&mut io, &mut qmp_output, &mut qmp_input, &mut events, case)?;
    } else if uart {
        let case = fabric_uart_qtest::transactions();
        ensure!(
            case.script.lines().count() == case.responses.len(),
            "UART script count"
        );
        for (command, expected) in case.script.lines().zip(case.responses) {
            io.command(command.into(), expected)?;
        }
    } else if fabric {
        fabric_reset::run(&mut io, &mut qmp_output, &mut qmp_input, &mut events)?;
    } else if swdt {
        swdt_reset::run(&mut io, &mut qmp_output, &mut qmp_input, &mut events)?;
    } else {
        io.reset_values()?;
        for cycle in 0..2 {
            io.write(0xf8000008, 0xdf0d)?;
            io.write(0xf8000100, 0x28000)?;
            io.write(0xf8000120, 0x1f000200)?;
            io.write(0xf80001c4, 1)?;
            for base in [0xf8001000, 0xf8002000] {
                for channel in 0..3 {
                    let clock = base + channel * 4;
                    io.write(clock, 0)?;
                    io.write(clock + 0x24, 37)?;
                    io.write(clock + 0x30, 11)?;
                    io.write(clock + 0x60, 0x3f)?;
                    io.write(clock + 0xc, 0x18)?;
                }
            }
            io.advance(1000)?;
            for base in [0xf8001000, 0xf8002000] {
                for channel in 0..3 {
                    io.read(base + channel * 4 + 0x18, 111)?;
                }
            }
            io.irq_state(true, cycle)?;
            qmp(
                &mut qmp_output,
                &mut qmp_input,
                2 + cycle * 2,
                "system_reset",
                &mut events,
            )?;
            qmp(
                &mut qmp_output,
                &mut qmp_input,
                3 + cycle * 2,
                "query-status",
                &mut events,
            )?;
            ensure!(
                events.iter().filter(|e| e["event"] == "RESET").count() == (cycle + 1) as usize,
                "actual reset event absent"
            );
            io.reset_values()?;
            io.irq_state(false, cycle)?;
            io.advance(1_000_000)?;
            io.reset_values()?;
            io.irq_state(false, cycle)?;
        }
    }
    qmp(&mut qmp_output, &mut qmp_input, 10, "quit", &mut events)?;
    ensure!(backend.0.wait()?.success(), "backend did not exit cleanly");
    println!(
        "MD_SIM_TTC_RESET_RESULT {}",
        json!({"passed":true,"transactions":io.records.len(),"qmp_events":events,"warm_resets":events.iter().filter(|event| event["event"] == "RESET").count(),"irq_events":io.irq_events,"test_kind":kind})
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|s| s == "__inside") {
        return inner(args.get(2).and_then(|s| s.to_str()).unwrap_or("ttc"));
    }
    ensure!(
        (3..=4).contains(&args.len())
            && args
                .get(3)
                .is_none_or(|s| s == "rom-map" || s == "rom-swap-reject" || s == "rom-reason" || s == "rom-lvttl" || s == "rom-gpio" || s == "rom-gpio-reject" || s == "rom-debug-reference" || s == "rom-debug-reference-reject" || s == "swdt" || s == "fabric" || s == "fabric-uart" || s == "pcap-migration" || s.to_str().and_then(pcap_qtest::Case::from_kind).is_some()),
        "usage: qemu_ttc_reset BACKEND_BUILD NEW_OUTPUT [rom-map|rom-swap-reject|rom-reason|rom-lvttl|rom-gpio|rom-gpio-reject|rom-debug-reference|rom-debug-reference-reject|swdt|fabric|fabric-uart|pcap|pcap-fixed-source|pcap-control-recovery|pcap-unsupported-dma|pcap-unlock-reset|pcap-migration]"
    );
    let kind = args.get(3).and_then(|s| s.to_str()).unwrap_or("ttc");
    let swdt = kind == "swdt";
    let fabric = kind == "fabric";
    let uart = kind == "fabric-uart";
    let rom = matches!(
        kind,
        "rom-map"
            | "rom-swap-reject"
            | "rom-reason"
            | "rom-lvttl"
            | "rom-gpio"
            | "rom-gpio-reject"
            | "rom-debug-reference"
            | "rom-debug-reference-reject"
    );
    let pcap_case = pcap_qtest::Case::from_kind(kind);
    let migration = kind == "pcap-migration";
    let pcap = pcap_case.is_some() || migration;
    let build = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    ensure!(!out.exists(), "new output required");
    fs::create_dir_all(&out)?;
    ensure!(
        fs::metadata(build.join("build-report.json"))?.len() < 16 * 1024 * 1024,
        "build report bound"
    );
    let report: Value = serde_json::from_slice(&fs::read(build.join("build-report.json"))?)?;
    let binary = build.join("qemu-system-arm");
    ensure!(
        report["build"]["code"] == 0
            && hash_file(&binary)? == serde_json::from_value::<ObjectId>(report["binary"].clone())?,
        "backend identity differs"
    );
    let store = Store::open(&out.join("inputs"), 128 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/qemu".into(), binary),
            ("/probe".into(), std::env::current_exe()?),
        ],
    )?;
    if rom {
        ensure!(
            report["rom_reset_patch"]["code"] == 0
                && report["rom_reset_reference"]["contract"] == "md-sim-public-zynq-rom-reset/1",
            "ROM reset backend capability absent"
        );
        rom_reset_qtest::capture(&store, &mut inputs)?;
    }
    if pcap {
        pcap_qtest::capture(&store, &mut inputs, &out)?;
    }
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: if swdt || fabric || uart || pcap || rom {
            vec!["/probe".into(), "__inside".into(), kind.into()]
        } else {
            vec!["/probe".into(), "__inside".into()]
        },
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 20,
            cpu_seconds: 5,
            memory_bytes: 1024 * 1024 * 1024,
            processes: 32,
            file_bytes: 4 * 1024 * 1024,
            trace_bytes: 2 * 1024 * 1024,
            tmp_bytes: 16 * 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    let stdout = hex::decode(&result.stdout)?;
    let stderr = hex::decode(&result.stderr)?;
    fs::write(out.join("transactions.log"), &stdout)?;
    fs::write(out.join("backend.log"), &stderr)?;
    let records: Vec<Value> = String::from_utf8(stdout)?
        .lines()
        .filter_map(|l| l.strip_prefix("MD_SIM_TTC_RESET_RESULT "))
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    let uart_verified =
        !uart || fabric_uart_qtest::transactions().verify(&String::from_utf8_lossy(&stderr));
    let pcap_verified = if migration {
        String::from_utf8_lossy(&stderr)
            .matches("Strict PCAP migration is unsupported")
            .count()
            == 2
    } else {
        pcap_case.is_none_or(|case| pcap_qtest::verify(&String::from_utf8_lossy(&stderr), case))
    };
    let gpio = kind == "rom-gpio" || kind == "rom-gpio-reject";
    let debug = kind == "rom-debug-reference" || kind == "rom-debug-reference-reject";
    let rom_verified = if debug {
        debug_reference_qtest::verify_diagnostics(
            &String::from_utf8_lossy(&stderr),
            kind == "rom-debug-reference-reject",
        )
    } else if gpio {
        ps_gpio_qtest::verify_diagnostics(
            &String::from_utf8_lossy(&stderr),
            kind == "rom-gpio-reject",
        )
    } else {
        !rom || rom_reset_qtest::verify_diagnostics(
            &String::from_utf8_lossy(&stderr),
            kind == "rom-swap-reject",
        )
    };

    let passed = rom_verified
        && uart_verified
        && pcap_verified
        && result.status == "exited"
        && result.exit_code == Some(0)
        && !result.trace_truncated
        && records.len() == 1
        && records[0]["passed"] == true
        && records[0]["transactions"]
            == if rom {
                if debug {
                    // Include the common irq_intercept_in command recorded before the plan.
                    1 + debug_reference_qtest::transaction_count(
                        kind == "rom-debug-reference-reject",
                    )
                } else if gpio {
                    ps_gpio_qtest::transaction_count(kind == "rom-gpio-reject")
                } else if kind == "rom-lvttl" {
                    20
                } else if kind == "rom-reason" {
                    44
                } else if kind == "rom-swap-reject" {
                    383
                } else {
                    340
                }
            } else if migration {
                17
            } else if kind == "pcap-fixed-source" {
                146
            } else if kind == "pcap-control-recovery" {
                136
            } else if kind == "pcap-unsupported-dma" {
                151
            } else if kind == "pcap-unlock-reset" {
                131
            } else if pcap {
                118
            } else if uart {
                45
            } else if fabric {
                128
            } else if swdt {
                60
            } else {
                385
            }
        && records[0]["warm_resets"]
            == if uart
                || migration
                || kind == "rom-gpio-reject"
                || kind == "rom-debug-reference-reject"
            {
                0
            } else if kind == "rom-reason" {
                4
            } else if swdt || kind == "pcap-unlock-reset" {
                3
            } else {
                2
            }
        && records[0]["test_kind"] == kind
        && records[0]["irq_events"].as_array().is_some_and(|events| {
            events.len()
                == if kind == "rom-gpio" {
                    14
                } else if migration {
                    0
                } else if pcap {
                    8
                } else if fabric || uart || rom {
                    0
                } else if swdt {
                    2
                } else {
                    24
                }
        });
    let scope = if debug {
        "Actual historical-QEMU SLCR0x500 storage reference. All32-bit data, SLCR keys, locked writes, warm-retention and strict word-access rejection. No physical debug/security/eFuse behavior, CPU execution, original-board or exact S9 identity pass."
    } else if gpio {
        "Actual four-bank Zynq PS GPIO register/IRQ reference: masked writes, settled inputs, unknown rejection, five interrupt modes, multi-bank OR, bus/clock guards and warm-reset retention. No CPU execution, physical MIO mux/pad timing or exact S9 pin-wiring claim."
    } else if kind == "rom-lvttl" {
        "Actual documented GPIOB_CFG_LVTTL register state, original BootROM programming value, ignored upper nibble and two warm-reset clears. No analog electrical behavior, physical pin calibration, CPU or exact S9 identity claim."
    } else if kind == "rom-reason" {
        "Actual SLCR read-only reset reason, write-one clear, reserved-bit behavior, one construction POR, two guest PSS soft resets and two QMP warm resets with retained REBOOT_STATUS. QMP has no invented hardware cause. No CPU execution, watchdog/external/debug cause, physical power cycle or exact S9 silicon claim."
    } else if rom {
        "Actual read-only public ROM and OCM alias checks, all RAM_HI masks, two QMP resets, and optional private SWAP rejection. No CPU, full ROM boot, physical retention or S9 identity acceptance."
    } else if migration {
        "Actual QEMU device-state save and reload in a qtest-only DevC fixture. Generic state reload is the positive control; strict save and load both reject before registers change. Existing S9 board migration restrictions remain separate. No firmware, FPGA, physical security or mining claim."
    } else if kind == "pcap-unlock-reset" {
        "Actual DMA/GIC/reset regression, invalid-key MMIO lockout, ignored access before system reset, restored reference cold state and complete programming after reset/enable. QEMU unassigned-read value and system-reset policy only; no physical reset-type, security or bus-response claim."
    } else if kind == "pcap-unsupported-dma" {
        "Actual DMA/GIC/reset regression, rejected non-configuration DMA with an empty source and nonempty destination, sticky failure after restoring a supported destination, and complete programming after reset/enable. Strict reference policy; no generic DMA, physical fault flag or PCAP timing claim."
    } else if kind == "pcap-control-recovery" {
        "Actual DMA/GIC/reset regression, rejected control state, sticky failure after controls are restored, and complete programming after explicit reset/enable. Reference failure policy; no physical fault flag or PCAP timing claim."
    } else if kind == "pcap-fixed-source" {
        "Actual DMA/GIC/reset regression, fixed 32-bit source reads from guest RAM, mode readback, mixed fixed/incrementing completion, and rejection of repeated words at the first differing configuration word. No peripheral FIFO, physical PCAP timing or exact-board claim."
    } else if pcap {
        "Actual DMA memory, exact selected configuration, full and sync-only streams, partial completion, actual GIC interrupt clear/mask behavior, programming reset, two QMP resets including asserted IRQ and changed guest memory rejection. No logic reconstruction or PCAP timing calibration."
    } else if uart {
        "Legacy S9IO FCLK1 UART: mid-byte gate, preserved state, half-frequency resume, independentFCLK0 and exact fractional timestamps. QMP requests clean process exit. No physical FPGA equivalence."
    } else if fabric {
        "Four fabric clock register banks and reset outputs: cold state, two actual QMP system resets, gate state clearing, retained reset defaults after advancing time. Downstream FPGA reset synchronization remains separate."
    } else if swdt {
        "Actual Zynq SWDT GIC interrupt, QMP reset while pulse is active, watchdog-caused system reset, reset while counter is armed, and cancellation of prior callbacks. Reference model test; no BootROM reset-reason, physical calibration, firmware or original-board claim."
    } else {
        "Actual QMP system reset after programming all six TTC channels. Cold reset defaults, two warm resets, stopped counters after reset and all six actual GIC input wires raised before and lowered by each reset. Reference device test only; no firmware or exact-board claim."
    };
    save_json(
        &out.join("assessment.json"),
        &json!({"passed":passed,"test_kind":kind,"verification_applicability":{"rom":rom,"pcap":pcap || migration,"uart":uart},"conditional_verifier_policy":"An inapplicable validator is true by definition; only test_kind and observations identify exercised checks.","rom_verified":rom_verified,"ps_gpio_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/ps_gpio_qtest.rs")),"rom_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/rom_reset_qtest.rs")),"pcap_verified":pcap_verified,"pcap_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/pcap_qtest.rs")),"pcap_migration_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/pcap_migration_qtest.rs")),"uart_verified":uart_verified,"fabric_uart_case":if uart {Some(fabric_uart_qtest::transactions())} else {None},"fabric_uart_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/fabric_uart_qtest.rs")),"backend":report["binary"],"driver":hash_file(&std::env::current_exe()?)?,"source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("qemu_ttc_reset.rs")),"swdt_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/swdt_reset.rs")),"fabric_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/fabric_reset.rs")),"spec":hash_file(&out.join("spec.json"))?,"result":hash_file(&out.join("result.json"))?,"observations":records,"scope":scope}),
    )?;
    ensure!(
        passed,
        "device reset check failed; retained assessment and logs"
    );
    Ok(())
}

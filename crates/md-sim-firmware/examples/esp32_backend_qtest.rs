//! Compiled S3 controller regression. This has no firmware or mining pass.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
};
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;

fn inner() -> Result<()> {
    confinement::require_board()?;
    let mut script = String::new();
    std::io::stdin()
        .take(64 * 1024 + 1)
        .read_to_string(&mut script)?;
    let count = script.lines().count();
    ensure!(
        script.len() <= 64 * 1024 && (1..=1024).contains(&count),
        "qtest script budget"
    );
    for path in [c"/work/monitor.in", c"/work/monitor.out"] {
        ensure!(
            unsafe { libc::mkfifo(path.as_ptr(), 0o600) } == 0,
            "private monitor pipe creation failed"
        );
    }
    let mut monitor = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/work/monitor.in")?;
    let _monitor_output = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/work/monitor.out")?;
    let mut command = Command::new("/qemu");
    command.args([
        "-machine",
        "esp32s3",
        "-accel",
        "qtest",
        "-m",
        "8M",
        "-global",
        "ssi_psram.is_octal=on",
        "-L",
        "/rom",
        "-display",
        "none",
        "-nic",
        "none",
        "-monitor",
        "pipe:/work/monitor",
        "-nodefaults",
        "-serial",
        "null",
        "-qtest",
        "stdio",
        "-trace",
        "enable=md_sim_i2c_*",
    ]);
    if std::env::var_os("MD_SIM_DS4432_FIXTURE").is_some() {
        command.args(["-device", "md-sim-ds4432,bus=md-sim-i2c-0,address=0x48,rfs0-ohms=80000,rfs1-ohms=0,supply-uv=3300000", "-trace", "md_sim_ds4432_bus"]);
    }
    if std::env::var_os("MD_SIM_EMC2101_FIXTURE").is_some() {
        command.args(["-device", "md-sim-emc2101,id=md-emc,bus=md-sim-i2c-0,address=0x4c,internal-eighths=216,external-kind=0,host-boot-full=off", "-trace", "enable=md_sim_emc2101_*"]);
    }
    if std::env::var_os("MD_SIM_INA260_FIXTURE").is_some() {
        command.args(["-device", "md-sim-ina260,id=md-ina,bus=md-sim-i2c-0,address=0x40,vin-uv=11980000,current-ua=-1250000", "-trace", "enable=md_sim_ina260_*"]);
    }
    if std::env::var_os("MD_SIM_SAR_INPUTS").is_some() {
        command.args([
            "-global",
            "md-sim-esp32s3-sar.adc1-channel1-uv=550000",
            "-global",
            "md-sim-esp32s3-sar.adc2-channel9-uv=2200000",
            "-trace",
            "enable=md_sim_sar_*",
        ]);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .context("qtest input pipe missing")?
        .write_all(script.as_bytes())?;
    let mut responses = BufReader::new(child.stdout.take().context("qtest output pipe missing")?);
    for _ in 0..count {
        let mut line = Vec::new();
        responses.by_ref().take(1025).read_until(b'\n', &mut line)?;
        ensure!(
            line.len() <= 1024 && line.last() == Some(&b'\n'),
            "missing or oversized qtest response"
        );
        std::io::stdout().write_all(&line)?;
    }
    // qtest EOF closes its transport, not QEMU. Request a real clean shutdown
    // only after all actual responses. The private HMP pipe is not exported.
    monitor.write_all(b"quit\n")?;
    let mut extra = Vec::new();
    responses.take(1025).read_to_end(&mut extra)?;
    ensure!(extra.is_empty(), "unexpected extra qtest response");
    ensure!(child.wait()?.success(), "QEMU qtest shutdown failed");
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "__inside") {
        return inner();
    }
    ensure!(
        args.len() == 3
            || (args.len() == 4
                && (args[3] == "ds4432"
                    || args[3] == "emc2101"
                    || args[3] == "ina260"
                    || args[3] == "sar-inputs")),
        "usage: esp32_backend_qtest BACKEND_BUILD NEW_OUTPUT [ds4432|emc2101|ina260|sar-inputs]"
    );
    let build = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    let sar_inputs = args.get(3).is_some_and(|a| a == "sar-inputs");
    let ds4432 = args.len() == 4 && !sar_inputs;
    let ina260 = args.get(3).is_some_and(|a| a == "ina260");
    let emc2101 = args.get(3).is_some_and(|a| a == "emc2101");
    ensure!(!out.exists(), "new output required");
    ensure!(
        fs::metadata(build.join("build-report.json"))?.len() < 16 * 1024 * 1024,
        "build report budget"
    );
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(build.join("build-report.json"))?)?;
    ensure!(
        !ds4432 || report.get("ds4432_model").is_some(),
        "DS4432 backend model absent"
    );
    ensure!(
        !emc2101 || report.get("emc2101_model").is_some(),
        "EMC2101 backend model absent"
    );
    ensure!(
        !ina260 || report.get("ina260_model").is_some(),
        "INA260 backend model absent"
    );
    ensure!(
        report["source"] == "5456034eb8a9f348b1f4c8820dc79f55677bddfe0d1c66566b6c42ceb9df66f0"
            && report.get("i2c_master_model").is_some(),
        "unexpected backend source/model"
    );
    for (file, key) in [
        ("qemu-system-xtensa", "binary"),
        ("esp32s3_rev0_rom.bin", "rom"),
    ] {
        ensure!(
            hash_file(&build.join(file))?
                == serde_json::from_value::<ObjectId>(report[key].clone())?,
            "backend identity differs"
        );
    }
    fs::create_dir_all(&out)?;
    let store = Store::open(&out.join("inputs"), 64 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/qemu".into(), build.join("qemu-system-xtensa")),
            ("/probe".into(), std::env::current_exe()?),
        ],
    )?;
    let rom = hash_file(&build.join("esp32s3_rev0_rom.bin"))?;
    store.import(&build.join("esp32s3_rev0_rom.bin"), &rom)?;
    inputs.push(Mount {
        object: rom,
        guest_path: "/rom/esp32s3_rev0_rom.bin".into(),
    });
    // Literal S3 TRM register contract. Bus0 has no slaves. 40MHz /4 gives
    // 10MHz controller clock; START100cycles, address9*100cycles.
    let mut transactions = vec![
        ("readl 0x60013004", "OK 0x000000000000010b"),
        ("readl 0x60027004", "OK 0x000000000000010b"),
        ("writel 0x60013004 0x113", "OK"),
        ("writel 0x60013000 49", "OK"),
        ("writel 0x60013038 0x3018", "OK"),
        ("writel 0x60013040 49", "OK"),
        ("writel 0x60013044 49", "OK"),
        ("writel 0x60013054 0x200003", "OK"),
        ("writel 0x60013004 0x913", "OK"),
        ("writel 0x60013028 0x400", "OK"),
        ("writel 0x60013058 0x3000", "OK"),
        ("writel 0x6001305c 0x901", "OK"),
        ("writel 0x60013060 0x1000", "OK"),
        ("writel 0x6001301c 0x78", "OK"),
        ("writel 0x60013004 0x133", "OK"),
        ("clock_step 99999", "OK 99999"),
        ("readl 0x6001302c", "OK 0x0000000000000000"),
        ("clock_step 1", "OK 100000"),
        ("readl 0x6001302c", "OK 0x0000000000000400"),
        ("readl 0x60013008", "OK 0x000000000000c001"),
        ("readl 0x60013058", "OK 0x0000000080003000"),
        ("readl 0x6001305c", "OK 0x0000000000000901"),
        ("readl 0x60013060", "OK 0x0000000000001000"),
        ("writel 0x60013020 0x400", "OK"),
        ("readl 0x6001302c", "OK 0x0000000000000000"),
        ("readl 0x60027028", "OK 0x0000000000000000"),
    ];
    if ds4432 {
        transactions.extend([
            ("writel 0x60013024 0x3ffff", "OK"),
            ("writel 0x60013028 0x580", "OK"),
            ("writel 0x60013058 0x3000", "OK"),
            ("writel 0x6001305c 0x903", "OK"),
            ("writel 0x60013060 0x1000", "OK"),
            ("writel 0x6001301c 0x90", "OK"),
            ("writel 0x6001301c 0xf8", "OK"),
            ("writel 0x6001301c 0xaa", "OK"),
            ("writel 0x60013004 0x133", "OK"),
            // START10us +3bytes*90us + STOP0.2us.
            ("clock_step 280200", "OK 380200"),
            ("readl 0x6001302c", "OK 0x0000000000000080"),
            // Reset the controller FSM, not the separately powered DAC.
            ("writel 0x60013004 0x513", "OK"),
            ("writel 0x60013058 0x3000", "OK"),
            ("writel 0x6001305c 0x902", "OK"),
            ("writel 0x60013060 0x3000", "OK"),
            ("writel 0x60013064 0x901", "OK"),
            ("writel 0x60013068 0x1c01", "OK"),
            ("writel 0x6001306c 0x1000", "OK"),
            ("writel 0x6001301c 0x90", "OK"),
            ("writel 0x6001301c 0xf8", "OK"),
            ("writel 0x6001301c 0x91", "OK"),
            ("writel 0x60013004 0x133", "OK"),
            // Two STARTs +4bytes +STOP =380.2us. The archived qtest
            // clock stops when no timers remain, even for larger steps.
            ("clock_step 380200", "OK 760400"),
            ("readl 0x6001302c", "OK 0x0000000000000080"),
            ("readl 0x6001301c", "OK 0x00000000000000aa"),
            // Undocumented register access must fail visibly in this model.
            ("writel 0x60013024 0x3ffff", "OK"),
            ("writel 0x60013058 0x3000", "OK"),
            ("writel 0x6001305c 0x902", "OK"),
            ("writel 0x60013060 0x1000", "OK"),
            ("writel 0x6001301c 0x90", "OK"),
            ("writel 0x6001301c 0xf7", "OK"),
            ("writel 0x60013004 0x133", "OK"),
            ("clock_step 190000", "OK 950400"),
            ("readl 0x6001302c", "OK 0x0000000000000400"),
            // Fixed-address DAC must not ACK the I2C general call.
            ("writel 0x60013024 0x3ffff", "OK"),
            ("writel 0x6001305c 0x901", "OK"),
            ("writel 0x6001301c 0", "OK"),
            ("writel 0x60013004 0x133", "OK"),
            ("clock_step 100000", "OK 1050400"),
            ("readl 0x6001302c", "OK 0x0000000000000400"),
            // Repeated START must select the changed address. No slave at3c.
            ("writel 0x60013004 0x513", "OK"),
            ("writel 0x60013024 0x3ffff", "OK"),
            ("writel 0x60013058 0x3000", "OK"),
            ("writel 0x6001305c 0x902", "OK"),
            ("writel 0x60013060 0x3000", "OK"),
            ("writel 0x60013064 0x901", "OK"),
            ("writel 0x60013068 0x1000", "OK"),
            ("writel 0x6001301c 0x90", "OK"),
            ("writel 0x6001301c 0xf8", "OK"),
            ("writel 0x6001301c 0x78", "OK"),
            ("writel 0x60013004 0x133", "OK"),
            ("clock_step 290000", "OK 1340400"),
            ("readl 0x6001302c", "OK 0x0000000000000400"),
            // Real firmware repeats a STOP write-trigger before it completes.
            ("writel 0x60013024 0x3ffff", "OK"),
            ("writel 0x60013058 0x1000", "OK"),
            ("writel 0x60013004 0x133", "OK"),
            ("clock_step 100", "OK 1340500"),
            ("writel 0x60013004 0x913", "OK"),
            ("writel 0x60013004 0x133", "OK"),
            ("clock_step 100", "OK 1340600"),
            ("readl 0x6001302c", "OK 0x0000000000000000"),
            ("clock_step 100", "OK 1340700"),
            ("readl 0x6001302c", "OK 0x0000000000000080"),
        ]);
    }
    let mut transactions: Vec<(String, String)> = transactions
        .into_iter()
        .map(|(a, b)| (a.into(), b.into()))
        .collect();
    if emc2101 {
        transactions.extend(emc_transactions());
    }
    if ina260 {
        transactions.extend(ina_transactions());
    }
    if sar_inputs {
        transactions.extend(
            sar_transactions()
                .into_iter()
                .map(|(a, b)| (a.into(), b.into())),
        );
    }
    let script = transactions
        .iter()
        .map(|(t, _)| t.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(out.join("transactions.qtest"), &script)?;
    let stdin = hash_file(&out.join("transactions.qtest"))?;
    store.import(&out.join("transactions.qtest"), &stdin)?;
    let spec = ProcessSpec {
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: ["/probe", "__inside"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        inputs,
        stdin_object: Some(stdin),
        environment: [
            ("MD_SIM_DS4432_FIXTURE", ds4432),
            ("MD_SIM_EMC2101_FIXTURE", emc2101),
            ("MD_SIM_INA260_FIXTURE", ina260),
            ("MD_SIM_SAR_INPUTS", sar_inputs),
        ]
        .into_iter()
        .filter(|(_, enabled)| *enabled)
        .map(|(name, _)| (name.into(), "1".into()))
        .collect::<BTreeMap<_, _>>(),
        limits: Limits {
            wall_seconds: 10,
            cpu_seconds: 5,
            memory_bytes: 1024 * 1024 * 1024,
            processes: 32,
            file_bytes: 4 * 1024 * 1024,
            trace_bytes: 2 * 1024 * 1024,
            tmp_bytes: 16 * 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    save_json(
        &out.join("inputs.json"),
        &json!({"recipe":hash_file(&std::env::current_exe()?)?,"build_report":hash_file(&build.join("build-report.json"))?,"expected":transactions.iter().map(|(_,r)|r).collect::<Vec<_>>()}),
    )?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    let stdout = hex::decode(&result.stdout)?;
    let stderr = hex::decode(&result.stderr)?;
    fs::write(out.join("responses.log"), &stdout)?;
    fs::write(out.join("backend.log"), &stderr)?;
    let actual = String::from_utf8_lossy(&stdout);
    let expected: Vec<_> = transactions.iter().map(|(_, r)| r.as_str()).collect();
    let response_match = actual.lines().collect::<Vec<_>>() == expected;
    let trace = String::from_utf8_lossy(&stderr);
    let actual_nack = trace.lines().any(|l| {
        l == "md_sim_i2c_bus index=0 operation=0 value=0x3c result=1"
            || l == "md_sim_i2c_bus index=0 operation=0 value=0x3c result=-1"
    });
    let dac_trace = !ds4432
        || (trace
            .lines()
            .any(|l| l == "md_sim_ds4432_bus read=1 value=170 result=0 writes=1 faults=0")
            && trace
                .lines()
                .any(|l| l == "md_sim_ds4432_bus read=0 value=247 result=-1 writes=1 faults=1"));
    let emc_trace = !emc2101
        || [40, 27, 127].into_iter().all(|v| {
            trace.lines().any(|l| {
                l.starts_with(&format!("md_sim_emc2101_bus read=1 value={v} result=0 "))
                    && l.ends_with("faults=0")
            })
        });
    let ina_trace = !ina260
        || [0x54, 0x49, 0xfc, 0x18, 0x25, 0x70]
            .into_iter()
            .all(|value| {
                trace.lines().any(|line| {
                    line.starts_with(&format!("md_sim_ina260_bus read=1 value={value} result=0 "))
                        && line.ends_with("faults=0")
                })
            })
            && !trace
                .lines()
                .any(|line| line.starts_with("md_sim_ina260_") && !line.ends_with("faults=0"));
    let passed = result.exit_code == Some(0)
        && !result.trace_truncated
        && response_match
        && actual_nack
        && dac_trace
        && emc_trace
        && ina_trace;
    save_json(
        &out.join("assessment.json"),
        &json!({"component_passed":passed,"response_match":response_match,"actual_absent_slave_nack":actual_nack,"ds4432_selected":ds4432,"ds4432_trace_match":dac_trace,"emc2101_selected":emc2101,"emc2101_trace_match":emc_trace,"ina260_selected":ina260,"ina260_trace_match":ina_trace,"sar_inputs_selected":sar_inputs,"scope":"Compiled controller MMIO, programmed virtual timing, FIFO, NACK, interrupt state, and independent controller state; optional DS4432, EMC2101, INA260 and explicit ADC inputs. No firmware or physical board acceptance.","complete_original_board_boot":false,"functional_mining":false}),
    )?;
    ensure!(
        passed,
        "compiled I2C component regression failed; retained exact responses"
    );
    Ok(())
}

fn sar_transactions() -> Vec<(&'static str, &'static str)> {
    // 0.55V /1.1V *4095 =2047.5, truncated. ADC2's2.2V input saturates.
    // Unit1/channel0 remains disconnected. Start/clock reset cannot turn it
    // into a valid sample. Initial qtest time is100000ns from the I2C test.
    vec![
        ("writel 0x60008904 0x40000000", "OK"),
        ("writel 0x6000883c 0x60000000", "OK"),
        ("writel 0x60008814 0", "OK"),
        ("writel 0x60008838 0", "OK"),
        ("writel 0x6000880c 0x80160000", "OK"),
        ("clock_step 6249", "OK 106249"),
        ("readl 0x6000880c", "OK 0x0000000080160000"),
        ("clock_step 1", "OK 106250"),
        ("readl 0x6000880c", "OK 0x00000000801707ff"),
        ("writel 0x60008908 0x40000000", "OK"),
        ("writel 0x60008908 0", "OK"),
        ("readl 0x6000880c", "OK 0x0000000080140000"),
        ("writel 0x6000880c 0x80160000", "OK"),
        ("clock_step 6250", "OK 112500"),
        ("readl 0x6000880c", "OK 0x00000000801707ff"),
        ("writel 0x60008830 0x90060000", "OK"),
        ("clock_step 6250", "OK 118750"),
        ("readl 0x60008830", "OK 0x0000000090070fff"),
        ("writel 0x6000880c 0", "OK"),
        ("writel 0x6000880c 0x800e0000", "OK"),
        ("readl 0x6000880c", "OK 0x00000000800e07ff"),
    ]
}

struct Vectors {
    values: Vec<(String, String)>,
    now: u64,
    address: u8,
}
impl Vectors {
    fn write(&mut self, addr: u32, value: u32) {
        self.values
            .push((format!("writel {addr:#x} {value:#x}"), "OK".into()));
    }
    fn read(&mut self, addr: u32, expected: u32) {
        self.values
            .push((format!("readl {addr:#x}"), format!("OK {expected:#018x}")));
    }
    fn step(&mut self, ns: u64) {
        self.now += ns;
        self.values
            .push((format!("clock_step {ns}"), format!("OK {}", self.now)));
    }
    fn reset(&mut self) {
        self.write(0x60013004, 0x513);
        self.write(0x60013024, 0x3ffff);
    }
    fn reg(&mut self, reg: u8, expected: u8) {
        self.reset();
        for (addr, value) in [
            (0x58, 0x3000),
            (0x5c, 0x902),
            (0x60, 0x3000),
            (0x64, 0x901),
            (0x68, 0x1c01),
            (0x6c, 0x1000),
        ] {
            self.write(0x60013000 + addr, value);
        }
        for value in [self.address << 1, reg, (self.address << 1) | 1] {
            self.write(0x6001301c, u32::from(value));
        }
        self.write(0x60013004, 0x133);
        // Two STARTs, four9-clock byte periods, one STOP at10MHz.
        self.step(380200);
        self.read(0x6001302c, 0x80);
        self.read(0x6001301c, u32::from(expected));
    }
    fn set_reg(&mut self, reg: u8, value: u8) {
        self.reset();
        for (addr, value) in [(0x58, 0x3000), (0x5c, 0x903), (0x60, 0x1000)] {
            self.write(0x60013000 + addr, value);
        }
        for byte in [self.address << 1, reg, value] {
            self.write(0x6001301c, u32::from(byte));
        }
        self.write(0x60013004, 0x133);
        self.step(280200);
        self.read(0x6001302c, 0x80);
    }
}
fn emc_transactions() -> Vec<(String, String)> {
    let mut v = Vectors {
        values: Vec::new(),
        now: 1_340_700,
        address: 0x4c,
    };
    v.reset();
    for (addr, value) in [(0x58, 0x3000), (0x5c, 0x901), (0x60, 0x1000)] {
        v.write(0x60013000 + addr, value);
    }
    v.write(0x6001301c, 0x98);
    v.write(0x60013004, 0x133);
    v.step(100000);
    v.read(0x6001302c, 0x400); // startup NACK, before10ms
    v.step(10_000_000 - v.now);
    v.reg(0xfd, 0x28);
    v.reg(0, 0);
    v.step(13_000_000 - v.now);
    v.reg(0, 27);
    v.reg(1, 0); // external conversion is still pending until34ms
    v.step(34_000_000 - v.now);
    v.reg(1, 127);
    v.reg(0x10, 0);
    v.reg(0x46, 0xff);
    v.reg(0x47, 0xff); // no supplied fan edges
    v.reg(2, 0x16); // open fault plus high/critical, no busy bit after conversion
    v.set_reg(3, 4);
    v.set_reg(0x4a, 0x2b); //1.4kHz base, documented absent-tach saturation mode
    for (index, level) in [1, 0, 1, 0, 1].into_iter().enumerate() {
        if index != 0 {
            v.step(5_000_000);
        }
        v.values.push((
            format!("set_irq_in /machine/peripheral/md-emc tach 0 {level}"),
            "OK".into(),
        ));
    }
    v.reg(0x46, 0x08);
    v.reg(0x47, 0x07); //20ms revolution: actual1800 counts
    v.step(728_166_667);
    v.reg(0x46, 0xff);
    v.reg(0x47, 0xff); //stopped input expires; no invented RPM
    v.values
}

impl Vectors {
    fn word(&mut self, reg: u8, expected: u16) {
        self.reset();
        for (addr, value) in [
            (0x58, 0x3000),
            (0x5c, 0x902),
            (0x60, 0x3000),
            (0x64, 0x901),
            (0x68, 0x1801),
            (0x6c, 0x1c01),
            (0x70, 0x1000),
        ] {
            self.write(0x60013000 + addr, value);
        }
        for byte in [self.address << 1, reg, (self.address << 1) | 1] {
            self.write(0x6001301c, u32::from(byte));
        }
        self.write(0x60013004, 0x133);
        // Two STARTs, five byte periods, one STOP:470.2us.
        self.step(470_200);
        self.read(0x6001302c, 0x80);
        for byte in expected.to_be_bytes() {
            self.read(0x6001301c, u32::from(byte));
        }
    }
    fn set_word(&mut self, reg: u8, value: u16) {
        self.reset();
        for (addr, value) in [(0x58, 0x3000), (0x5c, 0x904), (0x60, 0x1000)] {
            self.write(0x60013000 + addr, value);
        }
        let [high, low] = value.to_be_bytes();
        for byte in [self.address << 1, reg, high, low] {
            self.write(0x6001301c, u32::from(byte));
        }
        self.write(0x60013004, 0x133);
        self.step(370_200);
        self.read(0x6001302c, 0x80);
    }
}
fn ina_transactions() -> Vec<(String, String)> {
    let mut v = Vectors {
        values: Vec::new(),
        now: 1_340_700,
        address: 0x40,
    };
    v.set_word(0, 0x6120); // cancel default pair before bus conversion finishes
    v.word(0, 0x6120);
    v.word(0xfe, 0x5449);
    v.word(0xff, 0x2270);
    v.word(1, 0);
    v.word(2, 0);
    v.set_word(0, 0x6123); // same real input,40us recovery+2*1.1ms
    v.word(1, 0); // complete two-byte transfer before pair completion
                  // The configuration byte commits200ns before STOP. The following read
                  // consumes470200ns. Reach the actual conversion deadline exactly. Pinned
                  // QEMU9 qemu_clock_advance_virtual_time stops at the last pending timer;
                  // it cannot advance farther once a triggered conversion removes it.
                  // See qtest-clock-inspection-001 for the exact archived timer source.
    v.step(2_240_000 - 200 - 470_200);
    v.word(1, 0xfc18); // -1.25A, signed code-1000
    v.word(2, 0x2570); // TI example11.98V
    v.word(3, 1497); // positive14.97W after integer quantization
    v.word(6, 8);
    v.word(6, 0); // triggered mode: read clears CVRF; no second conversion
    v.set_word(7, 0xfe0c); // -0.625A undercurrent threshold
    v.set_word(6, 0x4403); // undercurrent,ready,latch,active-high
    v.set_word(0, 1); // current-only trigger at140us
    v.word(6, 0x441b); // actual current alarm plus ready flag
    v.word(6, 0x4403); // both flags cleared by prior read
    v.set_word(0, 0x8000); // real sensor reset
    v.word(0, 0x6127);
    v.word(7, 0);
    v.word(6, 0); // before new default2.2ms conversion completes
    v.values
}

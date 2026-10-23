//! Device-only migration checks using state produced by the real QEMU backend.
//! This does not exercise firmware, FPGA logic, or full-board migration.
use super::{line, Backend, Transactions};
use anyhow::{ensure, Context, Result};
use md_sim_firmware::prepare::hash_bytes;
use serde_json::{json, Value};
use std::{
    ffi::CString,
    fs::{self, OpenOptions},
    io::{BufReader, Read, Write},
    process::{ChildStdin, ChildStdout, Command, Stdio},
    time::Duration,
};

const BASE: u32 = 0xf8007000;
const MARKER: u32 = 0x4d445043;
const SAVED: &str = "/work/pcap-generic.state";
const REJECTED: &str = "/work/pcap-strict.state";
const STATE_LIMIT: u64 = 1 << 20;
const COLD_STATUS: u64 = 0x88020000;
// The retained generic reset callback submits a zero-length DMA. Its DMA-done
// flags remain in this positive control; strict mode suppresses that callback.
const GENERIC_COLD_STATUS: u64 = COLD_STATUS | 0x3004;

struct Instance {
    backend: Backend,
    input: BufReader<ChildStdout>,
    output: ChildStdin,
    io: Transactions,
    name: &'static str,
    next_id: u32,
    qmp: Vec<Value>,
    events: Vec<Value>,
}

impl Instance {
    fn start(name: &'static str, strict: bool, incoming: bool) -> Result<Self> {
        let prefix = format!("/work/pcap-migration-{name}");
        for suffix in ["in", "out"] {
            let path = CString::new(format!("{prefix}.{suffix}"))?;
            ensure!(
                unsafe { libc::mkfifo(path.as_ptr(), 0o600) } == 0,
                "private migration-test FIFO creation failed: {}",
                std::io::Error::last_os_error()
            );
        }
        let output = OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("{prefix}.in"))?;
        let input = BufReader::new(
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(format!("{prefix}.out"))?,
        );
        let mut command = Command::new("/qemu");
        command
            .args([
                "-machine",
                "md-sim-pcap-contract-test",
                "-accel",
                "qtest",
                "-S",
                "-nodefaults",
                "-nic",
                "none",
                "-display",
                "none",
                "-monitor",
                "none",
                "-serial",
                "null",
                "-qmp",
                "stdio",
                "-qtest",
            ])
            .arg(format!("pipe:{prefix}"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        if incoming {
            command.args(["-incoming", "defer"]);
        }
        if strict {
            command.args([
                "-global",
                "driver=xlnx.ps7-dev-cfg,property=md-bitstream,value=/pcap-reference.bit",
                "-global",
                "driver=xlnx.ps7-dev-cfg,property=md-part,value=/pcap-part.json",
            ]);
        }
        let mut backend = Backend(command.spawn()?);
        let mut qmp_input = BufReader::new(
            backend
                .0
                .stdout
                .take()
                .context("migration QMP output absent")?,
        );
        let qmp_output = backend
            .0
            .stdin
            .take()
            .context("migration QMP input absent")?;
        let greeting: Value = serde_json::from_str(&line(&mut qmp_input)?)?;
        println!(
            "MD_SIM_PCAP_MIGRATION_GREETING {}",
            json!({"instance":name,"message":greeting})
        );
        ensure!(greeting["QMP"].is_object(), "migration QMP greeting absent");
        let mut instance = Self {
            backend,
            input: qmp_input,
            output: qmp_output,
            io: Transactions {
                input,
                output,
                records: Vec::new(),
                irq_events: Vec::new(),
                now: 0,
            },
            name,
            next_id: 1,
            qmp: Vec::new(),
            events: Vec::new(),
        };
        instance.request("qmp_capabilities", json!({}))?;
        if incoming {
            let status = instance.request("query-status", json!({}))?;
            ensure!(
                status["return"]["status"] == "inmigrate",
                "receiver is not waiting for incoming migration"
            );
        } else {
            instance.request("stop", json!({}))?;
        }
        Ok(instance)
    }

    fn request(&mut self, command: &str, arguments: Value) -> Result<Value> {
        ensure!(self.next_id <= 128, "migration QMP command bound");
        let id = self.next_id;
        self.next_id += 1;
        let request = json!({"execute":command,"arguments":arguments,"id":id});
        println!(
            "MD_SIM_PCAP_MIGRATION_REQUEST {}",
            json!({"instance":self.name,"message":request})
        );
        serde_json::to_writer(&mut self.output, &request)?;
        writeln!(self.output)?;
        self.output.flush()?;
        for _ in 0..32 {
            let response: Value = serde_json::from_str(&line(&mut self.input)?)?;
            println!(
                "MD_SIM_PCAP_MIGRATION_RESPONSE {}",
                json!({"instance":self.name,"message":response})
            );
            if response["event"].is_string() {
                ensure!(self.events.len() < 64, "migration QMP event bound");
                self.events.push(response);
                continue;
            }
            self.qmp
                .push(json!({"request":request,"response":response}));
            ensure!(response["id"] == id, "migration QMP response ID differs");
            ensure!(
                response.get("return").is_some() && response.get("error").is_none(),
                "migration QMP command failed: {response}"
            );
            return Ok(response);
        }
        anyhow::bail!("migration QMP reply absent")
    }

    fn migrate(
        &mut self,
        incoming: bool,
        path: &str,
        expected_error: Option<&str>,
    ) -> Result<Value> {
        let uri = format!("file:{path}");
        let (command, arguments) = if incoming {
            // The normal incoming path registers and cleans up its migration
            // lifecycle. Keep an expected rejection alive for register checks.
            ("migrate-incoming", json!({"uri":uri,"exit-on-error":false}))
        } else {
            ("migrate", json!({"uri":uri}))
        };
        self.request(command, arguments)?;
        for _ in 0..100 {
            let response = self.request("query-migrate", json!({}))?;
            let status = &response["return"];
            match status["status"].as_str() {
                Some("completed") => {
                    ensure!(expected_error.is_none(), "strict migration was accepted");
                    ensure!(
                        status.get("error-desc").is_none(),
                        "completed migration retains an error: {status}"
                    );
                    return Ok(status.clone());
                }
                Some("failed") => {
                    let message = expected_error.context("generic migration failed")?;
                    ensure!(
                        status["error-desc"]
                            .as_str()
                            .is_some_and(|text| text.contains(message)),
                        "migration rejection differs: {status}; expected {message}"
                    );
                    return Ok(status.clone());
                }
                Some("setup" | "active" | "device" | "pre-switchover" | "failing") => {}
                _ => anyhow::bail!("unexpected migration state: {status}"),
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        anyhow::bail!("migration did not reach a final state within the polling bound")
    }

    fn finish(mut self) -> Result<Value> {
        self.request("quit", json!({}))?;
        ensure!(
            self.backend.0.wait()?.success(),
            "migration backend did not exit cleanly"
        );
        ensure!(
            self.io.irq_events.is_empty(),
            "unexpected migration-test IRQ"
        );
        Ok(json!({"instance":self.name,"transactions":self.io.records,
            "qmp":self.qmp,"events":self.events,"clean_exit":true}))
    }
}

fn state(path: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(metadata.is_file(), "migration state is not a regular file");
    ensure!(
        metadata.len() <= STATE_LIMIT,
        "migration state exceeds bound"
    );
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(STATE_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= STATE_LIMIT,
        "migration state exceeds bound"
    );
    Ok(bytes)
}

pub fn run_inside() -> Result<Value> {
    super::confinement::require(false)?;
    let mut sender = Instance::start("generic-sender", false, false)?;
    sender.io.read(BASE + 0x30, 0)?;
    sender.io.write(BASE + 0x30, MARKER)?;
    // Generic upstream writes recreate PCFG_DONE after clearing other status.
    // The receiver must import this value as well as the distinct SW_ID marker.
    sender.io.write(BASE + 0xc, u32::MAX)?;
    sender.io.read(BASE + 0x30, MARKER.into())?;
    sender.io.read(BASE + 0xc, 4)?;
    let outgoing = sender.migrate(false, SAVED, None)?;
    let sender = sender.finish()?;
    let saved = state(SAVED)?;
    ensure!(!saved.is_empty(), "generic migration state is empty");

    let mut receiver = Instance::start("generic-receiver", false, true)?;
    receiver.io.read(BASE + 0x30, 0)?;
    receiver.io.read(BASE + 0xc, GENERIC_COLD_STATUS)?;
    let incoming = receiver.migrate(true, SAVED, None)?;
    receiver.io.read(BASE + 0x30, MARKER.into())?;
    receiver.io.read(BASE + 0xc, 4)?;
    let receiver = receiver.finish()?;

    let mut strict_sender = Instance::start("strict-sender", true, false)?;
    strict_sender.io.read(BASE + 0x30, 0)?;
    strict_sender.io.read(BASE + 0xc, COLD_STATUS)?;
    let outgoing_rejection =
        strict_sender.migrate(false, REJECTED, Some("pre-save failed: xlnx_zynq_devcfg"))?;
    strict_sender.io.read(BASE + 0x30, 0)?;
    strict_sender.io.read(BASE + 0xc, COLD_STATUS)?;
    let strict_sender = strict_sender.finish()?;

    let mut strict_receiver = Instance::start("strict-receiver", true, true)?;
    strict_receiver.io.read(BASE + 0x30, 0)?;
    strict_receiver.io.read(BASE + 0xc, COLD_STATUS)?;
    let incoming_rejection = strict_receiver.migrate(
        true,
        SAVED,
        Some("pre load hook failed for: 'xlnx_zynq_devcfg'"),
    )?;
    strict_receiver.io.read(BASE + 0x30, 0)?;
    strict_receiver.io.read(BASE + 0xc, COLD_STATUS)?;
    let strict_receiver = strict_receiver.finish()?;
    ensure!(state(SAVED)? == saved, "generic migration state changed");
    let partial = state(REJECTED)?;
    let instances = [&sender, &receiver, &strict_sender, &strict_receiver];
    let transactions: usize = instances
        .iter()
        .map(|instance| instance["transactions"].as_array().unwrap().len())
        .sum();
    let qmp_events: Vec<_> = instances
        .iter()
        .flat_map(|instance| instance["events"].as_array().unwrap())
        .cloned()
        .collect();
    Ok(
        json!({"passed":true,"test_kind":"pcap-migration","transactions":transactions,
        "warm_resets":0,"irq_events":[],"qmp_events":qmp_events,
        "migration":{"generic_sender":sender,"generic_receiver":receiver,
            "strict_sender":strict_sender,"strict_receiver":strict_receiver,
            "outgoing":outgoing,"incoming":incoming,
            "outgoing_rejection":outgoing_rejection,"incoming_rejection":incoming_rejection,
            "state":{"sha256":hash_bytes(&saved),"size":saved.len(),"hex":hex::encode(&saved)},
            "rejected_outgoing_partial_state":{"sha256":hash_bytes(&partial),
                "size":partial.len(),"hex":hex::encode(&partial)},
            "generic_reload_verified":true,"strict_save_rejected":true,
            "strict_load_rejected_before_register_mutation":true,
            "scope":"Standard QEMU file migration between separate DevC test-machine processes, with the normal configuration and state-description sections. A generic receiver imports both register markers. Strict outgoing and incoming rejection are separate from S9 board migration, firmware, physical FPGA behavior and mining."}}),
    )
}

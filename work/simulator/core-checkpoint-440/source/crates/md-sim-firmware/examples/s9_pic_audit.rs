//! Audit retained firmware transactions. This does not grant a board or
//! mining pass, including when the diagnostic process completed normally.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{ProcessResult, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde::Serialize;
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
};

fn bounded(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(maximum + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= maximum, "file exceeds audit bound");
    Ok(bytes)
}

#[derive(Default, Serialize)]
struct Controller {
    events: u64,
    writes: u64,
    defined_reads: u64,
    undefined_reads: u64,
    nacks: u64,
    errors: Vec<serde_json::Value>,
    commands: Vec<serde_json::Value>,
    power: Vec<serde_json::Value>,
    #[serde(skip)]
    prefix: usize,
    #[serde(skip)]
    command: Option<(u8, usize, Vec<u8>)>,
}
impl Controller {
    fn byte(&mut self, byte: u8, line: usize, ns: Option<u64>) {
        if let Some((opcode, count, mut data)) = self.command.take() {
            data.push(byte);
            if data.len() == count {
                self.commands
                    .push(json!({"opcode":opcode,"data":hex::encode(data),"line":line,"ns":ns}));
            } else {
                self.command = Some((opcode, count, data));
            }
        } else if self.prefix < 2 {
            self.prefix = if byte == [0x55, 0xaa][self.prefix] {
                self.prefix + 1
            } else {
                0
            };
        } else {
            self.prefix = 0;
            let count = match byte {
                1 => 2,
                2 => 16,
                0x10 | 0x15 => 1,
                _ => 0,
            };
            if count == 0 {
                self.commands
                    .push(json!({"opcode":byte,"data":"","line":line,"ns":ns}));
            } else {
                self.command = Some((byte, count, Vec::new()));
            }
        }
    }
}
fn hexadecimal(fields: &BTreeMap<&str, &str>, key: &str) -> Result<u32> {
    u32::from_str_radix(
        fields
            .get(key)
            .and_then(|v| v.strip_prefix("0x"))
            .context("missing hexadecimal trace field")?,
        16,
    )
    .context("invalid hexadecimal trace field")
}
fn analyze(log: &str) -> Result<BTreeMap<String, Controller>> {
    let mut controllers: BTreeMap<String, Controller> = BTreeMap::new();
    for (index, line) in log.lines().enumerate() {
        let mut words = line.split_whitespace();
        let Some(kind @ ("md_sim_pic_event" | "md_sim_pic_power")) = words.next() else {
            continue;
        };
        let fields: BTreeMap<_, _> = words.filter_map(|s| s.split_once('=')).collect();
        let address = hexadecimal(&fields, "address")?;
        ensure!((0x55..=0x57).contains(&address), "unexpected PIC address");
        let ns = fields.get("ns").map(|s| s.parse::<u64>()).transpose()?;
        let controller = controllers.entry(format!("{address:02x}")).or_default();
        if kind == "md_sim_pic_power" {
            controller
                .power
                .push(json!({"state":hexadecimal(&fields,"state")?,"line":index+1,"ns":ns}));
            continue;
        }
        controller.events += 1;
        let event = *fields.get("event").context("missing event kind")?;
        let data = hexadecimal(&fields, "data")?;
        let result = hexadecimal(&fields, "result")?;
        if result == 1 {
            controller.nacks += 1;
        } else if result > 1 {
            controller
                .errors
                .push(json!({"event":event,"data":data,"result":result,"line":index+1,"ns":ns}));
        }
        match event {
            "external-reset" if result == 0 => {
                controller.prefix = 0;
                controller.command = None;
            }
            "send" if result == 0 => {
                controller.writes += 1;
                controller.byte(u8::try_from(data)?, index + 1, ns);
            }
            "receive" if result == 0 => {
                ensure!(data <= 0x1ff, "invalid read trace flags");
                if data & 0x100 == 0 {
                    controller.undefined_reads += 1;
                } else {
                    controller.defined_reads += 1;
                }
            }
            _ => {}
        }
    }
    ensure!(controllers.len() == 3, "three PIC traces required");
    Ok(controllers)
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 4,
        "usage: s9_pic_audit RUN BACKEND NEW_REPORT"
    );
    let run = Path::new(&args[1]);
    let backend = Path::new(&args[2]);
    let output = Path::new(&args[3]);
    ensure!(!output.exists(), "report must be new");
    let spec: ProcessSpec =
        serde_json::from_slice(&bounded(&run.join("process-spec.json"), 65536)?)?;
    let result: ProcessResult =
        serde_json::from_slice(&bounded(&run.join("result.json"), 128 * 1024 * 1024)?)?;
    let adaptation: serde_json::Value =
        serde_json::from_slice(&bounded(&run.join("adaptation.json"), 1024 * 1024)?)?;
    let store = Store::open(&run.join("input-store"), 512 * 1024 * 1024)?;
    for input in &spec.inputs {
        store.verified_path(&input.object)?;
    }
    let original: ObjectId = serde_json::from_value(adaptation["original_sd"].clone())?;
    ensure!(
        spec.inputs
            .iter()
            .any(|m| m.guest_path == "/sd.img" && m.object == original),
        "original firmware identity differs"
    );
    let expected_backend: ObjectId =
        serde_json::from_value(adaptation["backend_build"]["binary"].clone())?;
    ensure!(
        spec.inputs
            .iter()
            .any(|m| m.guest_path == "/qemu" && m.object == expected_backend)
            && hash_file(&backend.join("qemu-system-arm"))? == expected_backend
            && hash_file(&backend.join("build-report.json"))?
                == serde_json::from_value::<ObjectId>(
                    adaptation["backend_build"]["report"].clone()
                )?,
        "backend evidence differs"
    );
    let guest = bounded(&run.join("guest.log"), 32 * 1024 * 1024)?;
    let trace = bounded(&run.join("backend.log"), 32 * 1024 * 1024)?;
    ensure!(
        guest == hex::decode(&result.stdout)? && trace == hex::decode(&result.stderr)?,
        "live logs differ from retained result streams"
    );
    let controllers = analyze(std::str::from_utf8(&trace)?)?;
    let identities: BTreeMap<_, _> = [
        "process-spec.json",
        "adaptation.json",
        "result.json",
        "guest.log",
        "backend.log",
        "console.json",
    ]
    .into_iter()
    .map(|name| Ok((name, hash_file(&run.join(name))?)))
    .collect::<Result<_>>()?;
    let report = json!({
        "scope":"Retained original ARM firmware execution and PIC protocol transactions. PIC application instructions do not execute; erased development storage and other board adaptations remain explicit.",
        "input_objects_verified":spec.inputs.len(),
        "firmware":original,"backend":expected_backend,"identities":identities,
        "diagnostic":{"status":result.status,"exit_code":result.exit_code,"wall_milliseconds":result.wall_milliseconds,"trace_truncated":result.trace_truncated},
        "controllers":controllers,
        "original_board_gate":"incomplete","mining_gate":"incomplete",
        "audit_source":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_pic_audit.rs")),
        "audit_binary":hash_file(&std::env::current_exe()?)?
    });
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output)?;
    serde_json::to_writer_pretty(&mut file, &report)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    println!(
        "Verified input identities and retained traces. Board and mining gates remain incomplete."
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interleaved_controllers_retain_commands_and_visible_failures() {
        let mut log = String::new();
        // Literal original-driver voltage commands, interleaved on three
        // addresses. STOP must preserve each command's pending arguments.
        for byte in [0x55, 0xaa, 0x10, 6] {
            for address in [0x55, 0x56, 0x57] {
                log.push_str(&format!("md_sim_pic_event address=0x{address:x} event=send data=0x{byte:x} ns=100 result=0x0\n"));
                log.push_str(&format!(
                    "md_sim_pic_event address=0x{address:x} event=bus data=0x3 ns=101 result=0x0\n"
                ));
            }
        }
        log.push_str("md_sim_pic_power address=0x55 state=0x603 ns=200\n");
        log.push_str(
            "md_sim_pic_event address=0x55 event=timer data=0x0 ns=10000000200 result=0x8\n",
        );
        let parsed = analyze(&log).unwrap();
        for state in parsed.values() {
            assert_eq!(state.commands.len(), 1);
            assert_eq!(state.commands[0]["opcode"], 0x10);
            assert_eq!(state.commands[0]["data"], "06");
        }
        assert_eq!(parsed["55"].errors[0]["result"], 8);
        assert_eq!(parsed["55"].power[0]["state"], 0x603);
        assert!(parsed["56"].errors.is_empty());
    }
    #[test]
    fn incomplete_or_malformed_trace_cannot_be_audited() {
        assert!(analyze("").is_err());
        assert!(analyze("md_sim_pic_event address=0x58 event=send data=0x55 result=0x0").is_err());
        assert!(
            analyze("md_sim_pic_event address=0x55 event=receive data=0x200 result=0x0").is_err()
        );
        assert!(analyze("md_sim_pic_event address=0x55 event=send data=0x55").is_err());
    }
}

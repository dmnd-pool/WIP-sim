//! Derive requested rates from a retained real UART trace and explicit clocks.
//! This does not run firmware, infer effective clocks or test a physical line.
use anyhow::{ensure, Context, Result};
use md_sim_firmware::esp32s3_uart::{decode, ClockInputs};
use md_sim_runtime::{isolation::save_json, store::hash_file};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fs, path::Path};
#[path = "support/bounded_file.rs"]
mod bounded_file;
#[path = "support/esp32_uart_trace.rs"]
mod uart_trace;

fn run(run: &Path, clocks_path: &Path, out: &Path) -> Result<()> {
    let clocks_raw = bounded_file::capture(clocks_path, 4096)?;
    let clocks: ClockInputs = serde_json::from_slice(&clocks_raw.bytes)?;
    let raw = bounded_file::capture(&run.join("backend.log"), 16 << 20)?;
    let result_raw = bounded_file::capture(&run.join("result.json"), 32 << 20)?;
    let result: Value = serde_json::from_slice(&result_raw.bytes)?;
    ensure!(
        result["trace_truncated"] == false
            && hex::decode(
                result["stderr"]
                    .as_str()
                    .context("retained stderr absent")?
            )? == raw.bytes,
        "UART raw trace does not match complete process result"
    );
    let trace = uart_trace::parse(std::str::from_utf8(&raw.bytes)?)?;
    let retained_raw =
        bounded_file::capture(&run.join("uart-configuration-observations.json"), 1 << 20)?;
    let retained: Value = serde_json::from_slice(&retained_raw.bytes)?;
    ensure!(
        trace.complete && serde_json::to_value(&trace)? == retained,
        "UART observations differ from complete retained trace"
    );
    let mut banks = BTreeMap::<String, (Option<u32>, Option<u32>)>::new();
    let mut records = Vec::new();
    for write in trace.writes {
        let bank = banks.entry(write.device.clone()).or_default();
        // These are the last observed writes, not established live state.
        // Reset events are not in this trace. Do not seed initial values
        // or claim continuity of a pair across an unobserved device reset.
        match write.address {
            0x78 => bank.0 = Some(u32::try_from(write.stored)?),
            0x14 => bank.1 = Some(u32::try_from(write.stored)?),
            _ => anyhow::bail!("unexpected audited UART address"),
        }
        let request = match *bank {
            (Some(config), Some(divider)) => Some(decode(config, divider, clocks)),
            _ => None,
        };
        records.push(json!({"observation":write,"last_observed_clk_conf":bank.0,
            "last_observed_clkdiv":bank.1,"conditional_requested_clock":request}));
    }
    let mut inputs = BTreeMap::from([
        ("result.json", result_raw.object),
        ("backend.log", raw.object),
        ("uart-configuration-observations.json", retained_raw.object),
    ]);
    for name in ["input-identities.json", "spec.json"] {
        inputs.insert(
            name,
            bounded_file::capture(&run.join(name), 1 << 20)?.object,
        );
    }
    let report = json!({"schema":"md-sim-esp32s3-uart-requests/1",
        "scope":"Conditional requested rates from last observed writes and explicit caller-supplied clocks. Initial registers and continuity across unobserved device resets remain unknown. This is not measured clock, UART timing, board or mining acceptance.",
        "run":run,"run_inputs":inputs,"clock_inputs":clocks,"clock_inputs_file":clocks_raw.object,
        "recipe":hash_file(Path::new("/proc/self/exe"))?,
        "decoder_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("../src/esp32s3_uart.rs")),
        "trace_parser_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/esp32_uart_trace.rs")),
        "observations":records,"reset_continuity_established":false,"effective_line_rate_established":false,
        "physical_clock_calibrated":false,"original_board_gate":false,"functional_mining_gate":false});
    fs::create_dir(out)?;
    save_json(&out.join("requested-clocks.json"), &report)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 4,
        "usage: esp32_uart_requests ORIGINAL_RUN CLOCK_INPUTS_JSON NEW_OUTPUT"
    );
    run(
        Path::new(&args[1]),
        Path::new(&args[2]),
        Path::new(&args[3]),
    )
}

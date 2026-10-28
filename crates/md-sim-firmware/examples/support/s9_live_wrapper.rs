//! Assemble the existing selected-logic shell and AXI tasks for live commands.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::fmt::Write;

const CONTROL: &str = include_str!("../../../../simulator/backends/icarus/live-control.v");
const PS7: &str = "PSS2_X13Y53_PS7_X0Y0_PS7";

pub fn generate(netlist: &str, ports: &[u8]) -> Result<(String, Value)> {
    let (mut source, inventory) = super::axi::generate_shell(netlist, ports)?;
    let table: Value = serde_json::from_slice(ports)?;
    ensure!(
        table["IRQF2P"]["direction"] == "input" && table["IRQF2P"]["width"] == 20,
        "selected IRQ observation ABI differs"
    );
    let module = "module axi_probe;\n";
    ensure!(
        source.matches(module).count() == 1,
        "live wrapper module header differs"
    );
    source = source.replacen(
        module,
        "module axi_probe;\nreg [15:0] md_inputs=16'hzzzz;\n",
        1,
    );
    writeln!(source, "wire [19:0] md_irq=dut.{PS7}.IRQF2P;")?;
    source.push_str("wire [18:0] md_outputs;\n");
    let mut input_index = 0;
    let mut output_index = 0;
    let mut input_pins = Vec::new();
    let mut output_pins = Vec::new();
    for port in inventory["external_ports"]
        .as_array()
        .context("external port inventory")?
    {
        let name = port["name"].as_str().context("external name")?;
        // Names already pass the shared generator's strict identifier check.
        match port["direction"].as_str().context("external direction")? {
            "input" => {
                // Connect the package input normally. Icarus does not keep a
                // procedural force with a bit-select RHS live after time0.
                let old = format!(".{name}(1'bz)");
                ensure!(
                    source.matches(&old).count() == 1,
                    "selected input connection differs"
                );
                source = source.replacen(&old, &format!(".{name}(md_inputs[{input_index}])"), 1);
                input_pins.push(json!({"index":input_index,"port":name,"direction":"input"}));
                input_index += 1;
            }
            "output" => {
                writeln!(source, "assign md_outputs[{output_index}]=dut.{name};")?;
                output_pins.push(json!({"index":output_index,"port":name,"direction":"output"}));
                output_index += 1;
            }
            "inout" => {
                // A resolved external driver preserves the actual FPGA driver.
                // It never forces the inout net or an internal output value.
                writeln!(source,"assign external_{name}=md_inputs[{input_index}];\nassign md_outputs[{output_index}]=external_{name};")?;
                input_pins.push(json!({"index":input_index,"port":name,"direction":"inout"}));
                output_pins.push(json!({"index":output_index,"port":name,"direction":"inout"}));
                input_index += 1;
                output_index += 1;
            }
            _ => anyhow::bail!("unsupported external direction"),
        }
    }
    ensure!(
        input_index == 16 && output_index == 19,
        "live selected pin topology differs"
    );
    source.push_str(&super::axi::observed_tasks()?);
    source.push_str(CONTROL);
    source.push_str("endmodule\n");
    let report = json!({
        "schema":"md-sim-selected-live-pl-wrapper/1",
        "source_sha256":md_sim_firmware::prepare::hash_bytes(source.as_bytes()),
        "control_source_sha256":md_sim_firmware::prepare::hash_bytes(CONTROL.as_bytes()),
        "input_pins":input_pins,"output_pins":output_pins,
        "ps7_driven_ports":inventory["driven_ports"],
        "ps7_observed_ports":inventory["observed_ports"],
        "additional_observed_port":{"name":"IRQF2P","width":20,"direction":"input"},
        "clock_policy":"No default frequency. Explicit QEMU-Q32 periods. First enabled level0; period changes preserve level and start a new half-period. Gating holds level. Absolute edges round up to thedeclared1ps HDL lattice without repeated rounding drift.",
        "command_policy":"Only future timestamp drives. Existing clock edges and their active/NBA work settle before the batch in future read-write synchronization. New input work settles before the read-only receipt, at the same requested timestamp. IRQ and raw output changes retain time and four-state values separately.",
        "next_boundary_policy":"Only the next owned clock edge can be reported. This is not the next kernel event or a CPU run-ahead grant.",
        "external_role_policy":"Raw package-facing pin identity only. ASIC/UART/PSU role is not inferred from the port name.",
        "cpu_synchronization_complete":false,"original_board_gate":false,"physical_timing_calibrated":false
    });
    Ok((source, report))
}

/// Reuse the exact live clock/control code in the small event-order oracle.
/// The fixture's dummy AXI tasks fail if called; they never supply a bus reply.
pub fn clock_fixture() -> String {
    let prefix = include_str!("../../../../simulator/backends/icarus/clock-order-fixture.v");
    format!("{prefix}\n{CONTROL}\nendmodule\n")
}

//! A selected PS7-interface driver for the recovered S9 programmable logic.
//! It never drives fabric responses, internal state, or external miner inputs.
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::BTreeMap, fmt::Write};

const PS7: &str = "PSS2_X13Y53_PS7_X0Y0_PS7";
const PS7_ABSENCE_NOTICE: &str = "The Zynq-7000 All Programmable SoC does not have a simulation model. Behavioral simulation of Zynq-7000 (e.g. Zynq PS7 block) is not supported in any simulator. Please use the AXI BFM simulation model to verify the AXI transactions.";
pub fn expected_notice() -> String {
    format!("Warning on instance axi_probe.dut.{PS7} : {PS7_ABSENCE_NOTICE}")
}
pub fn verify_notice_source(source: &str) -> Result<()> {
    let literal = format!("$display(\"Warning on instance %m : {PS7_ABSENCE_NOTICE}\");");
    ensure!(
        source.matches(&literal).count() == 1,
        "pinned PS7 startup notice literal differs"
    );
    Ok(())
}
const TASKS: &str = include_str!("../../../../simulator/firmware/fixtures/s9-axi-tasks-511.v");
#[derive(Deserialize)]
struct Port {
    direction: String,
    width: u32,
}
#[derive(Deserialize)]
struct Version {
    mask: u32,
    expected: u32,
}
#[derive(Deserialize)]
struct Build {
    minimum_inclusive: u32,
    maximum_inclusive: u32,
}
#[derive(Deserialize)]
struct Offsets {
    version: u32,
    build_timestamp: u32,
    control: u32,
    baud_divisor: u32,
}
#[derive(Deserialize)]
struct Control {
    clear_then_set_mask: u32,
}
#[derive(Deserialize)]
struct Contract {
    common_bases: [u32; 3],
    common_offsets: Offsets,
    version: Version,
    build_timestamp: Build,
    control_startup: Control,
}
const PS7_ABI: &[(&str, &str, u32)] = &[
    ("FCLKCLK", "output", 4),
    ("FCLKRESETN", "output", 4),
    ("MAXIGP0ACLK", "input", 1),
    ("MAXIGP0ARADDR", "output", 32),
    ("MAXIGP0ARBURST", "output", 2),
    ("MAXIGP0ARCACHE", "output", 4),
    ("MAXIGP0ARESETN", "output", 1),
    ("MAXIGP0ARID", "output", 12),
    ("MAXIGP0ARLEN", "output", 4),
    ("MAXIGP0ARLOCK", "output", 2),
    ("MAXIGP0ARPROT", "output", 3),
    ("MAXIGP0ARQOS", "output", 4),
    ("MAXIGP0ARREADY", "input", 1),
    ("MAXIGP0ARSIZE", "output", 2),
    ("MAXIGP0ARVALID", "output", 1),
    ("MAXIGP0AWADDR", "output", 32),
    ("MAXIGP0AWBURST", "output", 2),
    ("MAXIGP0AWCACHE", "output", 4),
    ("MAXIGP0AWID", "output", 12),
    ("MAXIGP0AWLEN", "output", 4),
    ("MAXIGP0AWLOCK", "output", 2),
    ("MAXIGP0AWPROT", "output", 3),
    ("MAXIGP0AWQOS", "output", 4),
    ("MAXIGP0AWREADY", "input", 1),
    ("MAXIGP0AWSIZE", "output", 2),
    ("MAXIGP0AWVALID", "output", 1),
    ("MAXIGP0BID", "input", 12),
    ("MAXIGP0BREADY", "output", 1),
    ("MAXIGP0BRESP", "input", 2),
    ("MAXIGP0BVALID", "input", 1),
    ("MAXIGP0RDATA", "input", 32),
    ("MAXIGP0RID", "input", 12),
    ("MAXIGP0RLAST", "input", 1),
    ("MAXIGP0RREADY", "output", 1),
    ("MAXIGP0RRESP", "input", 2),
    ("MAXIGP0RVALID", "input", 1),
    ("MAXIGP0WDATA", "output", 32),
    ("MAXIGP0WID", "output", 12),
    ("MAXIGP0WLAST", "output", 1),
    ("MAXIGP0WREADY", "input", 1),
    ("MAXIGP0WSTRB", "output", 4),
    ("MAXIGP0WVALID", "output", 1),
];
fn validate_ports(ports: &BTreeMap<String, Port>) -> Result<()> {
    for &(name, direction, width) in PS7_ABI {
        ensure!(
            ports
                .get(name)
                .is_some_and(|p| p.direction == direction && p.width == width),
            "selected PS7 ABI differs: {name}"
        );
    }
    ensure!(
        ports
            .keys()
            .filter(|name| name.starts_with("MAXIGP0")
                || ["FCLKCLK", "FCLKRESETN"].contains(&name.as_str()))
            .count()
            == PS7_ABI.len(),
        "unexpected selected PS7 port"
    );
    Ok(())
}
fn identifier(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

pub fn generate_shell(netlist: &str, port_data: &[u8]) -> Result<(String, Value)> {
    ensure!(
        netlist.len() <= 32 << 20 && port_data.len() <= 128 << 10,
        "AXI input bound"
    );
    let ports: BTreeMap<String, Port> = serde_json::from_slice(port_data)?;
    validate_ports(&ports)?;
    ensure!(
        netlist.matches(&format!("  PS7 #(\n  ) {PS7} (\n")).count() == 1,
        "selected PS7 instance absent"
    );
    let mut source = String::from("`timescale 1ps/1ps\nmodule axi_probe;\n");
    let header = netlist
        .strip_prefix("`timescale 1ps/1ps\nmodule top(\n")
        .context("selected top header")?
        .split_once("  );")
        .context("top header end")?
        .0;
    let mut connections = Vec::new();
    let mut external_inputs = Vec::new();
    let mut external_outputs = Vec::new();
    let mut external_ports = Vec::new();
    for line in header.lines() {
        let (direction, name) = line
            .trim()
            .trim_end_matches(',')
            .split_once(' ')
            .context("top port form")?;
        ensure!(identifier(name), "invalid top port identifier");
        external_ports.push(json!({"name":name,"direction":direction}));
        match direction {
            "input" => {
                connections.push(format!(".{name}(1'bz)"));
                external_inputs.push(name);
            }
            "inout" => {
                writeln!(source, "tri external_{name};")?;
                connections.push(format!(".{name}(external_{name})"));
                external_inputs.push(name);
            }
            "output" => {
                connections.push(format!(".{name}()"));
                external_outputs.push(name);
            }
            _ => anyhow::bail!("unexpected top port direction"),
        }
    }
    ensure!(
        external_inputs.len() == 16 && external_outputs.len() == 17,
        "selected external topology differs"
    );
    writeln!(source, "top dut({});", connections.join(","))?;
    let mut driven = Vec::new();
    let mut observed = Vec::new();
    let mut force = String::new();
    for (name, port) in &ports {
        if !name.starts_with("MAXIGP0") && !["FCLKCLK", "FCLKRESETN"].contains(&name.as_str()) {
            continue;
        }
        ensure!(
            identifier(name) && (1..=32).contains(&port.width),
            "PS7 port form"
        );
        let range = if port.width == 1 {
            String::new()
        } else {
            format!("[{}:0] ", port.width - 1)
        };
        match port.direction.as_str() {
            "output" => {
                let initial = match name.as_str() {
                    "FCLKCLK" => "4'bzzxz",
                    "FCLKRESETN" => "4'bzzzx",
                    _ => "0",
                };
                writeln!(source, "reg {range}src_{name}={initial};")?;
                writeln!(force, "force dut.{PS7}.{name}=src_{name};")?;
                driven.push(json!({"name":name,"width":port.width}));
            }
            "input" => {
                writeln!(source, "wire {range}ps_{name}=dut.{PS7}.{name};")?;
                observed.push(json!({"name":name,"width":port.width}));
            }
            _ => anyhow::bail!("unexpected PS7 direction"),
        }
    }
    source.push_str("initial begin\n");
    source.push_str(&force);
    source.push_str("end\n");
    let inventory = json!({"driven_ports":driven,"observed_ports":observed,
        "external_inputs_high_impedance":external_inputs,
        "external_outputs_observed_by_fabric_only":external_outputs, "external_ports":external_ports});
    Ok((source, inventory))
}

/// Add observations only. The handshake task source remains a single fixture.
/// The original benchmark still includes that fixture byte for byte.
pub fn observed_tasks() -> Result<String> {
    let mut tasks = TASKS.to_owned();
    for (from, to) in [
        ("reg [31:0] last_read;", "reg [31:0] last_read;\nreg [11:0] last_id;\nreg [1:0] last_response;"),
        ("    last_read=saved; transactions=transactions+1;", "    last_id=saved_id; last_response=saved_response;\n    last_read=saved; transactions=transactions+1;"),
        ("    transactions=transactions+1;", "    last_id=saved_id; last_response=saved_response;\n    transactions=transactions+1;"),
    ] {
        ensure!(tasks.matches(from).count()==1, "shared AXI task observer insertion differs");
        tasks=tasks.replacen(from,to,1);
    }
    Ok(tasks)
}

pub fn generate(netlist: &str, port_data: &[u8], contract_data: &[u8]) -> Result<(String, Value)> {
    ensure!(contract_data.len() <= 64 << 10, "AXI contract bound");
    let (mut source, shell) = generate_shell(netlist, port_data)?;
    let contract: Contract = serde_json::from_slice(contract_data)?;
    source.push_str(TASKS);
    source.push_str("reg [31:0] bases[0:2]; reg [31:0] initial_baud[0:2],initial_control[0:2],initial_version[0:2],initial_build[0:2];\nreg [31:0] base,saved_control; integer chain;\ninitial begin #5000;src_FCLKCLK[1]=0;forever #5000 src_FCLKCLK[1]=~src_FCLKCLK[1];end\ninitial begin #50000000;$fatal(1,\"AXI global simulation deadline transactions=%0d\",transactions);end\n");
    // The clock is external stimulus at100MHz. No internal clock or response
    // is forced, and no physical frequency measurement is claimed.
    source.push_str("always @(src_FCLKCLK[1]) begin #1000; if($time>200000 && ps_MAXIGP0ACLK!==src_FCLKCLK[1]) $fatal(1,\"fabric clock failed to propagate source=%b fabric=%b\",src_FCLKCLK[1],ps_MAXIGP0ACLK);end\n");
    source.push_str("task reset_fabric;begin src_FCLKRESETN[0]=0;src_MAXIGP0ARESETN=0;repeat(64)@(negedge ps_MAXIGP0ACLK);src_FCLKRESETN[0]=1;src_MAXIGP0ARESETN=1;repeat(64)@(negedge ps_MAXIGP0ACLK);$display(\"AXI_RESET_OBS time=%0t\",$time);end endtask\n");
    source.push_str("initial begin\n");
    for (i, base) in contract.common_bases.iter().enumerate() {
        writeln!(source, "bases[{i}]=32'h{base:08x};")?;
    }
    source
        .push_str("reset_fabric();\nfor(chain=0;chain<3;chain=chain+1)begin base=bases[chain];\n");
    let v = &contract.version;
    let b = &contract.build_timestamp;
    let o = &contract.common_offsets;
    let control = contract.control_startup.clear_then_set_mask;
    let read = |source: &mut String, offset: u32| -> std::fmt::Result {
        writeln!(
            source,
            "read_word(base+32'h{offset:08x},transactions[11:0]+12'd1);"
        )
    };
    let write = |source: &mut String, offset: u32, value: &str, strobe: u8| -> std::fmt::Result {
        writeln!(
            source,
            "write_word(base+32'h{offset:08x},{value},4'h{strobe:x},transactions[11:0]+12'd1);"
        )
    };
    read(&mut source, o.version)?;
    source.push_str("initial_version[chain]=last_read;\n");
    writeln!(source,"if((last_read &32'h{:08x})!==32'h{:08x})$fatal(1,\"firmware version predicate failed chain=%0d value=%08x\",chain,last_read);",v.mask,v.expected)?;
    read(&mut source, o.build_timestamp)?;
    source.push_str("initial_build[chain]=last_read;\n");
    writeln!(source,"if(last_read<32'h{:08x}||last_read>32'h{:08x})$fatal(1,\"firmware build predicate failed chain=%0d value=%08x\",chain,last_read);",b.minimum_inclusive,b.maximum_inclusive)?;
    read(&mut source, o.control)?;
    source.push_str("saved_control=last_read;initial_control[chain]=last_read;\n");
    write(
        &mut source,
        o.control,
        &format!("saved_control&~32'h{control:08x}"),
        15,
    )?;
    read(&mut source, o.control)?;
    writeln!(
        source,
        "if((last_read&32'h{control:08x})!==0)$fatal(1,\"control clear failed\");"
    )?;
    write(
        &mut source,
        o.control,
        &format!("saved_control|32'h{control:08x}"),
        15,
    )?;
    read(&mut source, o.control)?;
    writeln!(
        source,
        "if((last_read&32'h{control:08x})!==32'h{control:08x})$fatal(1,\"control set failed\");"
    )?;
    read(&mut source, o.baud_divisor)?;
    source.push_str("initial_baud[chain]=last_read;\n");
    for (value, strobe, expected) in [(7u32, 15, 7u32), (17, 2, 7), (17, 1, 17), (31, 0, 17)] {
        write(
            &mut source,
            o.baud_divisor,
            &format!("32'h{value:08x}"),
            strobe,
        )?;
        read(&mut source, o.baud_divisor)?;
        writeln!(source,"if(last_read!==32'h{expected:08x})$fatal(1,\"baud write/readback or strobe failed value=%08x\",last_read);")?;
    }
    write(&mut source, o.baud_divisor, "initial_baud[chain]", 15)?;
    read(&mut source, o.baud_divisor)?;
    source.push_str("if(last_read!==initial_baud[chain])$fatal(1,\"baud restore failed\");\n");
    write(&mut source, o.baud_divisor, "initial_baud[chain]^32'd1", 15)?;
    read(&mut source, o.baud_divisor)?;
    source.push_str("if(last_read!==(initial_baud[chain]^32'd1))$fatal(1,\"reset precondition write failed\");\nend\nreset_fabric();\nfor(chain=0;chain<3;chain=chain+1)begin base=bases[chain];\n");
    read(&mut source, o.version)?;
    writeln!(
        source,
        "if(last_read!==initial_version[chain]||(last_read&32'h{:08x})!==32'h{:08x})$fatal(1,\"version after reset failed\");",
        v.mask, v.expected
    )?;
    read(&mut source, o.build_timestamp)?;
    writeln!(
        source,
        "if(last_read!==initial_build[chain]||last_read<32'h{:08x}||last_read>32'h{:08x})$fatal(1,\"build after reset failed\");",
        b.minimum_inclusive, b.maximum_inclusive
    )?;
    read(&mut source, o.baud_divisor)?;
    source.push_str("if(last_read!==initial_baud[chain])$fatal(1,\"baud reset value differs\");\n");
    read(&mut source, o.control)?;
    source.push_str("if(last_read!==initial_control[chain])$fatal(1,\"control reset value differs\");end\nif(transactions!==72)$fatal(1,\"transaction count differs\");\n$display(\"MD_SIM_SELECTED_AXI_PASS transactions=72 resets=2\");$finish;end\nendmodule\n");
    let metadata = json!({"scope":"Component stimulus at declared PS7 output ports only. No processor or firmware execution.","driven_ports":shell["driven_ports"],"observed_ports":shell["observed_ports"],"external_inputs_high_impedance":shell["external_inputs_high_impedance"],"external_outputs_observed_by_fabric_only":shell["external_outputs_observed_by_fabric_only"],"clock_stimulus_hz":100000000,"clock_output_index":1,"fabric_reset_output_index":0,"physical_timing_calibrated":false,"expected_transactions":72,"expected_reset_sequences":2,"source_sha256":md_sim_firmware::prepare::hash_bytes(source.as_bytes()),"task_source_sha256":md_sim_firmware::prepare::hash_bytes(TASKS.as_bytes())});
    Ok((source, metadata))
}

pub fn verify_contract(contract: &[u8], elf: &[u8], dtb: &[u8]) -> Result<()> {
    let value: Value = serde_json::from_slice(contract)?;
    for (key, bytes) in [("parent_elf", elf), ("base_address_evidence", dtb)] {
        ensure!(
            value[key]["size"].as_u64() == Some(bytes.len() as u64)
                && value[key]["sha256"].as_str()
                    == Some(md_sim_firmware::prepare::hash_bytes(bytes).as_str()),
            "AXI original input identity differs"
        );
    }
    for key in ["instructions", "literals"] {
        let records = value[key]
            .as_array()
            .context("contract byte records absent")?;
        ensure!(
            !records.is_empty() && records.len() <= 128,
            "contract record bound"
        );
        for record in records {
            let offset =
                usize::try_from(record["file_offset"].as_u64().context("contract offset")?)?;
            let expected = hex::decode(record["bytes"].as_str().context("contract bytes")?)?;
            ensure!(
                expected.len() == 4
                    && elf.get(offset..offset.checked_add(4).context("contract offset overflow")?)
                        == Some(expected.as_slice()),
                "original instruction bytes differ"
            );
            if key == "literals" {
                ensure!(
                    record["value"].as_u64()
                        == Some(u64::from(u32::from_le_bytes(expected.try_into().unwrap()))),
                    "contract literal value differs"
                );
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
struct Record {
    address: u32,
    tag: u32,
    data: u32,
    strobe: Option<u32>,
}
fn records(trace: &str) -> Result<Vec<Record>> {
    ensure!(trace.len() <= 64 << 10, "AXI trace bound");
    let mut result = Vec::new();
    let mut resets = Vec::new();
    let mut complete = 0;
    let mut finishes = 0;
    let expected_notice = expected_notice();
    let mut notice_seen = false;
    for (line_number, line) in trace.lines().enumerate() {
        if line_number == 0 {
            ensure!(
                line == expected_notice,
                "exact PS7 absent-CPU notice must be the first line"
            );
            notice_seen = true;
            continue;
        }
        if line.starts_with("AXI_READ_OBS ") || line.starts_with("AXI_WRITE_OBS ") {
            let write = line.starts_with("AXI_WRITE_OBS ");
            let mut fields = BTreeMap::new();
            for field in line.split_ascii_whitespace().skip(1) {
                let (key, value) = field.split_once('=').context("AXI trace field")?;
                ensure!(
                    fields.insert(key, value).is_none(),
                    "duplicate AXI trace field"
                );
            }
            ensure!(fields.len() == 5, "AXI trace field count");
            let get = |name: &str| -> Result<u32> {
                Ok(u32::from_str_radix(
                    fields.get(name).context("AXI trace field absent")?,
                    16,
                )?)
            };
            ensure!(get("resp")? == 0, "AXI non-OK response");
            if !write {
                ensure!(get("last")? == 1, "AXI non-final single response");
            }
            result.push(Record {
                address: get("addr")?,
                tag: get("id")?,
                data: get("data")?,
                strobe: if write { Some(get("strobe")?) } else { None },
            });
        } else if let Some(time) = line.strip_prefix("AXI_RESET_OBS time=") {
            let time = time.parse::<u64>()?;
            ensure!(time > 0, "AXI reset timestamp absent");
            resets.push((result.len(), time));
        } else if line == "MD_SIM_SELECTED_AXI_PASS transactions=72 resets=2" {
            ensure!(result.len() == 72, "AXI completion precedes results");
            complete += 1;
        } else if line.starts_with("/work/compiled/axi-probe.v:")
            && line.contains(": $finish called at ")
        {
            ensure!(complete == 1, "AXI finish before complete marker");
            finishes += 1;
        } else {
            anyhow::bail!("unexpected selected AXI trace line: {line}");
        }
    }
    ensure!(
        result.len() == 72
            && notice_seen
            && complete == 1
            && finishes == 1
            && resets.len() == 2
            && resets[0].0 == 0
            && resets[1].0 == 60
            && resets[0].1 < resets[1].1,
        "AXI trace incomplete or unordered"
    );
    for (i, row) in result.iter().enumerate() {
        ensure!(row.tag == i as u32 + 1, "AXI response ID/order differs");
    }
    Ok(result)
}
fn read_result(rows: &[Record], cursor: &mut usize, address: u32) -> Result<u32> {
    let row = rows.get(*cursor).context("AXI read result absent")?;
    *cursor += 1;
    ensure!(
        row.address == address && row.strobe.is_none(),
        "AXI read sequence differs"
    );
    Ok(row.data)
}
fn write_result(
    rows: &[Record],
    cursor: &mut usize,
    address: u32,
    value: u32,
    strobe: u32,
) -> Result<()> {
    let row = rows.get(*cursor).context("AXI write result absent")?;
    *cursor += 1;
    ensure!(
        row.address == address && row.data == value && row.strobe == Some(strobe),
        "AXI write sequence differs"
    );
    Ok(())
}
pub fn check_trace(trace: &str, contract_data: &[u8]) -> Result<Value> {
    let c: Contract = serde_json::from_slice(contract_data)?;
    let rows = records(trace)?;
    let mut cursor = 0;
    let mut initial = Vec::new();
    let o = &c.common_offsets;
    for base in c.common_bases {
        let version = read_result(&rows, &mut cursor, base + o.version)?;
        ensure!(
            version & c.version.mask == c.version.expected,
            "original firmware version predicate failed"
        );
        let build = read_result(&rows, &mut cursor, base + o.build_timestamp)?;
        ensure!(
            (c.build_timestamp.minimum_inclusive..=c.build_timestamp.maximum_inclusive)
                .contains(&build),
            "original firmware build predicate failed"
        );
        let control = read_result(&rows, &mut cursor, base + o.control)?;
        let mask = c.control_startup.clear_then_set_mask;
        for value in [control & !mask, control | mask] {
            write_result(&rows, &mut cursor, base + o.control, value, 15)?;
            let actual = read_result(&rows, &mut cursor, base + o.control)?;
            ensure!(
                actual & mask == value & mask,
                "control update result differs"
            );
        }
        let baud = read_result(&rows, &mut cursor, base + o.baud_divisor)?;
        for (value, strobe, expected) in [
            (7, 15, 7),
            (17, 2, 7),
            (17, 1, 17),
            (31, 0, 17),
            (baud, 15, baud),
            (baud ^ 1, 15, baud ^ 1),
        ] {
            write_result(&rows, &mut cursor, base + o.baud_divisor, value, strobe)?;
            ensure!(
                read_result(&rows, &mut cursor, base + o.baud_divisor)? == expected,
                "baud register/strobe result differs"
            );
        }
        initial.push((version, build, control, baud));
    }
    ensure!(cursor == 60, "pre-reset trace sequence differs");
    for (base, (version, build, control, baud)) in c.common_bases.into_iter().zip(&initial) {
        for (offset, value) in [
            (o.version, version),
            (o.build_timestamp, build),
            (o.baud_divisor, baud),
            (o.control, control),
        ] {
            ensure!(
                read_result(&rows, &mut cursor, base + offset)? == *value,
                "reset/immutable register result differs"
            );
        }
    }
    ensure!(cursor == rows.len(), "unverified AXI observations remain");
    Ok(
        json!({"passed":true,"transactions":rows.len(),"reset_sequences":2,"initial_registers":initial,"trace_sha256":md_sim_firmware::prepare::hash_bytes(trace.as_bytes()),"scope":"Rust checks the complete actual digital bus trace against pinned firmware predicates and register operations. No CPU/board or physical timing claim.","ps7_startup_notice":expected_notice(),"ps7_notice_count":1,"ps7_behavioral_cpu_model_present":false,"original_board_gate":false}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn abi() -> BTreeMap<String, Port> {
        PS7_ABI
            .iter()
            .map(|&(name, direction, width)| {
                (
                    name.into(),
                    Port {
                        direction: direction.into(),
                        width,
                    },
                )
            })
            .collect()
    }
    const RETAINED_TRACE: &str =
        include_str!("../../../../simulator/firmware/fixtures/s9-axi-trace-511d.txt");
    const RETAINED_CONTRACT: &[u8] =
        include_bytes!("../../../../simulator/firmware/s9-bosminer-mmio-511.json");
    #[test]
    fn observer_adds_no_timing_control_or_assertion_change() {
        let tasks = observed_tasks().unwrap();
        assert_eq!(
            tasks
                .matches("last_id=saved_id; last_response=saved_response;")
                .count(),
            2
        );
        let restored = tasks
            .replace("\nreg [11:0] last_id;\nreg [1:0] last_response;", "")
            .replace("    last_id=saved_id; last_response=saved_response;\n", "");
        assert_eq!(restored, TASKS);
    }
    #[test]
    #[ignore = "read-only retained511e generator identity; set MD_SIM_AXI_511E_ROOT"]
    fn extracted_shell_preserves_actual_511e_benchmark_bytes() {
        let root = std::path::PathBuf::from(std::env::var_os("MD_SIM_AXI_511E_ROOT").unwrap());
        let netlist = std::fs::read_to_string(
            root.join("work/simulator/qemu-s9/netlist-selected-502k/selected.v"),
        )
        .unwrap();
        let ports = std::fs::read(
            root.join("artifacts/files/source/source-fasm-graph-8372b58/zynq7/ps7_ports.json"),
        )
        .unwrap();
        let (source, _) = generate(&netlist, &ports, RETAINED_CONTRACT).unwrap();
        let old = std::fs::read_to_string(
            root.join("work/simulator/qemu-s9/axi-selected-511e/axi-probe.v"),
        )
        .unwrap();
        assert_eq!(source, old);
        let (prefix, inventory) = generate_shell(&netlist, &ports).unwrap();
        assert!(old.starts_with(&prefix));
        assert_eq!(
            inventory["driven_ports"].as_array().unwrap().len()
                + inventory["observed_ports"].as_array().unwrap().len(),
            42
        );
    }
    #[test]
    fn exact_absent_cpu_notice_is_retained_without_relaxing_bus_checks() {
        let result = check_trace(RETAINED_TRACE, RETAINED_CONTRACT).unwrap();
        assert_eq!(result["transactions"], 72);
        assert_eq!(result["ps7_startup_notice"], expected_notice());
        assert_eq!(result["ps7_behavioral_cpu_model_present"], false);
        assert_eq!(result["original_board_gate"], false);
        let (notice, rest) = RETAINED_TRACE.split_once('\n').unwrap();
        for changed in [
            rest.to_string(),
            format!("{notice}\n{RETAINED_TRACE}"),
            format!("AXI_RESET_OBS time=1\n{RETAINED_TRACE}"),
            RETAINED_TRACE.replacen("does not have", "has", 1),
            format!("{notice}\nWarning: unrelated failure\n{rest}"),
        ] {
            assert!(check_trace(&changed, RETAINED_CONTRACT).is_err());
        }
    }
    #[test]
    fn retained_bus_trace_rejects_altered_values_ids_strobes_and_reset_result() {
        for (from, to) in [
            ("id=001 data=00901002", "id=001 data=00001002"),
            ("id=002 data=", "id=001 data="),
            ("strobe=2 resp=00", "strobe=f resp=00"),
            (
                "addr=43c00010 id=03f data=00000000",
                "addr=43c00010 id=03f data=00000001",
            ),
            ("resp=00 last=1", "resp=10 last=1"),
        ] {
            let changed = RETAINED_TRACE.replacen(from, to, 1);
            assert_ne!(changed, RETAINED_TRACE);
            assert!(check_trace(&changed, RETAINED_CONTRACT).is_err(), "{from}");
        }
    }
    #[test]
    fn response_and_clock_abi_corruption_is_rejected_before_generation() {
        validate_ports(&abi()).unwrap();
        for name in [
            "MAXIGP0RRESP",
            "MAXIGP0RDATA",
            "MAXIGP0RID",
            "FCLKCLK",
            "FCLKRESETN",
        ] {
            let mut ports = abi();
            ports.get_mut(name).unwrap().width = 1;
            assert!(validate_ports(&ports).is_err(), "{name}");
        }
        let mut ports = abi();
        ports.get_mut("MAXIGP0RVALID").unwrap().direction = "output".into();
        assert!(validate_ports(&ports).is_err());
        let mut ports = abi();
        ports.remove("MAXIGP0WVALID");
        assert!(validate_ports(&ports).is_err());
        let mut ports = abi();
        ports.insert(
            "MAXIGP0FAKE".into(),
            Port {
                direction: "output".into(),
                width: 1,
            },
        );
        assert!(validate_ports(&ports).is_err());
    }
    #[test]
    fn unknown_error_and_premature_completion_are_rejected() {
        for line in [
            "AXI_READ_OBS addr=43c00000 id=001 data=xxxxxxxx resp=00 last=1",
            "AXI_READ_OBS addr=43c00000 id=001 data=00901002 resp=10 last=1",
            "AXI_READ_OBS addr=43c00000 id=001 data=00901002 resp=00 last=0",
            "MD_SIM_SELECTED_AXI_PASS transactions=72 resets=2",
            "fake success",
        ] {
            assert!(
                records(&format!("{}\n{line}", expected_notice())).is_err(),
                "{line}"
            );
        }
    }
    #[test]
    fn original_instruction_and_literal_bindings_reject_mutation() {
        let elf = [1u8, 2, 3, 4];
        let dtb = [5u8];
        let mut value = json!({"parent_elf":{"sha256":md_sim_firmware::prepare::hash_bytes(&elf),"size":4},
            "base_address_evidence":{"sha256":md_sim_firmware::prepare::hash_bytes(&dtb),"size":1},
            "instructions":[{"file_offset":0,"bytes":"01020304"}],
            "literals":[{"file_offset":0,"bytes":"01020304","value":0x04030201}]});
        verify_contract(&serde_json::to_vec(&value).unwrap(), &elf, &dtb).unwrap();
        assert!(
            verify_contract(&serde_json::to_vec(&value).unwrap(), &[0, 2, 3, 4], &dtb).is_err()
        );
        value["literals"][0]["value"] = json!(0);
        assert!(verify_contract(&serde_json::to_vec(&value).unwrap(), &elf, &dtb).is_err());
    }
}

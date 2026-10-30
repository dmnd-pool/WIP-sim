//! Icarus Verilog testbench for the rebuilt S9 FPGA circuit.
//! The circuit and Xilinx's own primitive models stay unchanged. The bench
//! forces only the PS7 clock, reset and AXI GP0 master outputs.
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub const PS7_INSTANCE: &str = "PSS2_X13Y53_PS7_X0Y0_PS7";
const PLAYER: &str = include_str!("player.v");

/// Package-facing pins of the circuit's top module, in header order. The
/// pin index used in sequences and traces is the position in these lists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pads {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub inouts: Vec<String>,
}

pub fn top_pads(netlist: &str) -> Result<Pads> {
    const HEAD: &str = "module top(\n";
    let start = netlist
        .find(HEAD)
        .context("netlist has no `module top(` header")?
        + HEAD.len();
    let rest = &netlist[start..];
    let end = rest.find("  );").context("netlist top header has no end")?;
    let mut pads = Pads::default();
    for line in rest[..end].lines() {
        let line = line.trim().trim_end_matches(',');
        if line.is_empty() {
            continue;
        }
        let (direction, name) = line.split_once(' ').context("top port line")?;
        ensure!(
            name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
            "unexpected port name {name}"
        );
        match direction {
            "input" => pads.inputs.push(name.into()),
            "output" => pads.outputs.push(name.into()),
            "inout" => pads.inouts.push(name.into()),
            _ => bail!("unexpected port direction {direction}"),
        }
    }
    ensure!(
        !pads.inputs.is_empty() && !pads.outputs.is_empty(),
        "top has no pins"
    );
    Ok(pads)
}

#[derive(Deserialize)]
struct Port {
    direction: String,
    width: u32,
}

/// Build module `tb`: pin vectors, PS7 port forces and observations, the
/// circuit instance, a 100MHz fabric clock and the sequence player.
pub fn testbench(pads: &Pads, ps7_ports: &str) -> Result<String> {
    let ports: BTreeMap<String, Port> = serde_json::from_str(ps7_ports)?;
    ensure!(
        ports.contains_key("IRQF2P") && ports.contains_key("MAXIGP0RDATA"),
        "PS7 port list is incomplete"
    );
    let mut s = String::new();
    writeln!(s, "`timescale 1ps/1ps\nmodule tb;")?;
    writeln!(
        s,
        "localparam integer NIN = {};\nlocalparam integer NOUT = {};",
        pads.inputs.len(),
        pads.outputs.len()
    )?;
    writeln!(
        s,
        "reg [NIN-1:0] pin_in = {{NIN{{1'b1}}}};\nwire [NOUT-1:0] pin_out;"
    )?;
    for i in 0..pads.inouts.len() {
        writeln!(s, "tri1 pin_io_{i};")?;
    }
    let mut force = String::new();
    for (name, port) in &ports {
        let driven = name.starts_with("MAXIGP0") || name == "FCLKCLK" || name == "FCLKRESETN";
        if !driven && name != "IRQF2P" {
            continue;
        }
        ensure!((1..=32).contains(&port.width), "PS7 port width {name}");
        let range = if port.width == 1 {
            String::new()
        } else {
            format!("[{}:0] ", port.width - 1)
        };
        match port.direction.as_str() {
            "output" => {
                writeln!(s, "reg {range}src_{name} = 0;")?;
                writeln!(force, "  force dut.{PS7_INSTANCE}.{name} = src_{name};")?;
            }
            "input" => writeln!(s, "wire {range}ps_{name} = dut.{PS7_INSTANCE}.{name};")?,
            other => bail!("unexpected PS7 direction {other} for {name}"),
        }
    }
    let mut connections = Vec::new();
    for (i, name) in pads.inputs.iter().enumerate() {
        connections.push(format!(".{name}(pin_in[{i}])"));
    }
    for (i, name) in pads.outputs.iter().enumerate() {
        connections.push(format!(".{name}(pin_out[{i}])"));
    }
    for (i, name) in pads.inouts.iter().enumerate() {
        connections.push(format!(".{name}(pin_io_{i})"));
    }
    writeln!(s, "top dut({});", connections.join(", "))?;
    writeln!(s, "initial begin\n{force}end")?;
    writeln!(s, "always #5000 src_FCLKCLK[1] = ~src_FCLKCLK[1];")?;
    s.push_str(PLAYER);
    s.push_str("endmodule\n");
    Ok(s)
}

/// Unpacked Icarus installation from the archived build.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tools {
    pub iverilog: PathBuf,
    pub vvp: PathBuf,
    pub lib: PathBuf,
}

/// Source files for one compile. The netlist and vendor models are read in
/// place and never modified.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sources {
    pub netlist: PathBuf,
    pub unisims: PathBuf,
    pub glbl: PathBuf,
}

pub fn extract_tools(archive: &Path, destination: &Path) -> Result<Tools> {
    let file = fs::File::open(archive).with_context(|| format!("open {}", archive.display()))?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(file));
    tar.set_preserve_permissions(true);
    fs::create_dir_all(destination)?;
    tar.unpack(destination)
        .with_context(|| format!("unpack {}", archive.display()))?;
    let root = destination.join("iverilog-install");
    let tools = Tools {
        iverilog: root.join("bin/iverilog"),
        vvp: root.join("bin/vvp"),
        lib: root.join("lib/ivl"),
    };
    ensure!(
        tools.iverilog.is_file() && tools.vvp.is_file() && tools.lib.join("system.vpi").is_file(),
        "Icarus archive layout differs from iverilog-install/{{bin,lib/ivl}}"
    );
    Ok(tools)
}

impl Tools {
    pub fn compile(&self, testbench: &Path, sources: &Sources, output: &Path) -> Result<String> {
        let out = Command::new(&self.iverilog)
            .arg("-B")
            .arg(&self.lib)
            .args(["-g2012", "-Ttyp", "-s", "tb", "-s", "glbl", "-o"])
            .arg(output)
            .arg("-y")
            .arg(&sources.unisims)
            .args(["-Y", ".v"])
            .arg(&sources.glbl)
            .arg(testbench)
            .arg(&sources.netlist)
            .output()
            .context("start iverilog")?;
        let log = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        ensure!(
            out.status.success() && output.is_file(),
            "iverilog failed:\n{log}"
        );
        Ok(log)
    }

    pub fn simulation(&self, compiled: &Path, sequence: &Path, output: &Path) -> Command {
        let mut command = Command::new(&self.vvp);
        command
            .arg("-M")
            .arg(&self.lib)
            .arg(compiled)
            .arg(format!("+seq={}", sequence.display()))
            .arg(format!("+out={}", output.display()));
        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NETLIST: &str = "`timescale 1ps/1ps\nmodule top(\n  input A_IPAD,\n  input B_IPAD,\n  output C_OPAD,\n  inout D_IOPAD\n  );\n  wire x;\nendmodule\n";

    #[test]
    fn top_pads_keep_header_order() {
        let pads = top_pads(NETLIST).unwrap();
        assert_eq!(pads.inputs, ["A_IPAD", "B_IPAD"]);
        assert_eq!(pads.outputs, ["C_OPAD"]);
        assert_eq!(pads.inouts, ["D_IOPAD"]);
    }

    #[test]
    fn testbench_connects_every_pin_and_forces_only_master_signals() {
        let pads = top_pads(NETLIST).unwrap();
        let ports = r#"{"FCLKCLK":{"direction":"output","width":4},
            "FCLKRESETN":{"direction":"output","width":4},
            "MAXIGP0ARVALID":{"direction":"output","width":1},
            "MAXIGP0ARREADY":{"direction":"input","width":1},
            "MAXIGP0RDATA":{"direction":"input","width":32},
            "IRQF2P":{"direction":"input","width":20},
            "EMIOGPIOO":{"direction":"output","width":64}}"#;
        let tb = testbench(&pads, ports).unwrap();
        assert!(tb.contains(".A_IPAD(pin_in[0])"));
        assert!(tb.contains(".B_IPAD(pin_in[1])"));
        assert!(tb.contains(".C_OPAD(pin_out[0])"));
        assert!(tb.contains(".D_IOPAD(pin_io_0)"));
        assert!(
            tb.contains("force dut.PSS2_X13Y53_PS7_X0Y0_PS7.MAXIGP0ARVALID = src_MAXIGP0ARVALID;")
        );
        assert!(tb.contains("wire [19:0] ps_IRQF2P"));
        assert!(!tb.contains("EMIOGPIOO"));
        assert!(tb.ends_with("endmodule\n"));
    }
}

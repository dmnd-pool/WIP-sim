//! Differential checks for the four omitted RAM inputs in the selected netlist.
//! This reads only the bounded, pinned converter output format. It is not an HDL parser.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeMap, fmt::Write};

pub const PINS: [(&str, u32); 4] = [
    ("CASCADEINA", 13),
    ("CASCADEINB", 14),
    ("INJECTDBITERR", 23),
    ("INJECTSBITERR", 24),
];

fn parameters(text: &str) -> Result<BTreeMap<&str, &str>> {
    let mut result = BTreeMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let line = line.trim().trim_end_matches(',');
        let (key, value) = line
            .strip_prefix('.')
            .context("RAM parameter prefix")?
            .split_once('(')
            .context("RAM parameter open")?;
        let value = value.strip_suffix(')').context("RAM parameter close")?;
        ensure!(
            key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                && !value.contains(['(', ')', '\n', '\r'])
                && result.insert(key, value).is_none(),
            "RAM parameter format or duplicate"
        );
    }
    Ok(result)
}

fn contract(p: &BTreeMap<&str, &str>) -> Result<u32> {
    for (key, value) in [
        ("RAM_MODE", "\"TDP\""),
        ("DOA_REG", "0"),
        ("DOB_REG", "0"),
        ("IS_CLKARDCLK_INVERTED", "0"),
        ("IS_CLKBWRCLK_INVERTED", "0"),
        ("IS_ENARDEN_INVERTED", "0"),
        ("IS_ENBWREN_INVERTED", "0"),
        ("IS_RSTRAMARSTRAM_INVERTED", "1"),
        ("IS_RSTRAMB_INVERTED", "1"),
        ("IS_RSTREGARSTREG_INVERTED", "1"),
        ("IS_RSTREGB_INVERTED", "1"),
        ("SRVAL_A", "36'h000000000"),
        ("SRVAL_B", "36'h000000000"),
    ] {
        ensure!(
            p.get(key) == Some(&value),
            "RAM selected parameter differs: {key}"
        );
    }
    for (key, default) in [
        ("EN_ECC_READ", "\"FALSE\""),
        ("EN_ECC_WRITE", "\"FALSE\""),
        ("RAM_EXTENSION_A", "\"NONE\""),
        ("RAM_EXTENSION_B", "\"NONE\""),
    ] {
        ensure!(
            p.get(key).copied().unwrap_or(default) == default,
            "RAM active optional input: {key}"
        );
    }
    let width = p.get("READ_WIDTH_A").context("RAM width")?.parse::<u32>()?;
    ensure!(matches!(width, 18 | 36), "RAM unreviewed width");
    for key in ["READ_WIDTH_B", "WRITE_WIDTH_A", "WRITE_WIDTH_B"] {
        ensure!(
            p.get(key).context("RAM width absent")?.parse::<u32>()? == width,
            "RAM unequal widths"
        );
    }
    Ok(width)
}

pub fn generate(netlist: &str) -> Result<(String, Value)> {
    ensure!(netlist.len() <= 32 << 20, "RAM source bound");
    let mut source = String::from("`timescale 1ps/1ps\nmodule ram_vectors;\nreg ca=0,cb=0,ea=0,eb=0,rst=1;\nreg [15:0] addr=0; reg [31:0] data=0; reg [3:0] parity=0,wa=0; reg [7:0] wb=0;\nwire [31:0] qa[0:14],qb[0:14]; wire [3:0] pa[0:14],pb[0:14];\nwire [1:0] cascade[0:14],err[0:14]; wire [7:0] ecc[0:14]; wire [8:0] eccaddr[0:14];\ninteger cycle,count=0;\n");
    let mut identities = Vec::new();
    let mut widths = Vec::new();
    for (index, tail) in netlist.split("  RAMB36E1 #(\n").skip(1).enumerate() {
        ensure!(index < 15, "extra selected RAM instance");
        let (params, tail) = tail
            .split_once("\n  ) ")
            .context("RAM instance parameter boundary")?;
        let (name, _) = tail
            .split_once(" (\n")
            .context("RAM instance name boundary")?;
        ensure!(
            params.len() < 32 << 10
                && name.len() < 256
                && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
            "RAM instance format"
        );
        let parsed = parameters(params)?;
        let width = contract(&parsed)?;
        widths.push(width);
        identities.push(json!({"selected_instance":name,"parameters":parsed,
            "parameter_text_sha256":md_sim_firmware::prepare::hash_bytes(params.as_bytes()),"width":width}));
        writeln!(source, "RAMB36E1 #(\n{params}\n) ram{index} (\n.ADDRARDADDR(addr),.ADDRBWRADDR(addr),\n.CLKARDCLK(ca),.CLKBWRCLK(cb),.DIADI(data),.DIBDI(data),.DIPADIP(parity),.DIPBDIP(parity),\n.DOADO(qa[{index}]),.DOBDO(qb[{index}]),.DOPADOP(pa[{index}]),.DOPBDOP(pb[{index}]),\n.CASCADEOUTA(cascade[{index}][0]),.CASCADEOUTB(cascade[{index}][1]),.DBITERR(err[{index}][0]),.SBITERR(err[{index}][1]),\n.ECCPARITY(ecc[{index}]),.RDADDRECC(eccaddr[{index}]),\n.ENARDEN(ea),.ENBWREN(eb),.REGCEAREGCE(1'b0),.REGCEB(1'b0),\n.RSTRAMARSTRAM(rst),.RSTRAMB(rst),.RSTREGARSTREG(rst),.RSTREGB(rst),.WEA(wa),.WEBWE(wb)\n`ifdef EXPLICIT_OPEN\n,.CASCADEINA(1'bz),.CASCADEINB(1'bz),.INJECTDBITERR(1'bz),.INJECTSBITERR(1'bz)\n`endif\n);\n")?;
    }
    ensure!(
        widths.len() == 15 && widths.iter().filter(|w| **w == 18).count() == 6,
        "selected RAM inventory differs"
    );
    source.push_str("task pulse_a; begin #1000 ca=1; #1000 ca=0; #1000; end endtask\ntask pulse_b; begin #1000 cb=1; #1000 cb=0; #1000; end endtask\ntask check; input integer port; input [31:0] want; input [3:0] wantp; begin\n");
    for (i, width) in widths.iter().enumerate() {
        let dbits = if *width == 18 { 16 } else { 32 };
        let pbits = if *width == 18 { 2 } else { 4 };
        writeln!(source, "if(port==0) begin if(qa[{i}][{}:0]!==want[{}:0] || pa[{i}][{}:0]!==wantp[{}:0]) $fatal(1,\"RAM{i} A read cycle=%0d got=%h/%h want=%h/%h\",cycle,qa[{i}],pa[{i}],want,wantp); end\nelse begin if(qb[{i}][{}:0]!==want[{}:0] || pb[{i}][{}:0]!==wantp[{}:0]) $fatal(1,\"RAM{i} B read cycle=%0d got=%h/%h want=%h/%h\",cycle,qb[{i}],pb[{i}],want,wantp); end\nif({{ram{i}.CASCADEINA,ram{i}.CASCADEINB,ram{i}.INJECTDBITERR,ram{i}.INJECTSBITERR}} !== 4'bzzzz) $fatal(1,\"RAM{i} omitted input semantics changed\");\n$display(\"TRACE cycle=%0d port=%0d ram={i} q=%h/%h p=%b/%b cascade=%b err=%b ecc=%h addr=%h\",cycle,port,qa[{i}],qb[{i}],pa[{i}],pb[{i}],cascade[{i}],err[{i}],ecc[{i}],eccaddr[{i}]); count=count+1;",
            dbits-1,dbits-1,pbits-1,pbits-1,dbits-1,dbits-1,pbits-1,pbits-1)?;
    }
    source.push_str("end endtask\ninitial begin\n#110000;\nfor(cycle=0;cycle<16;cycle=cycle+1) begin\naddr=cycle*32; data=32'h5aa5c33c ^ (cycle*32'h01020408); parity=cycle;\nea=1;eb=0;wa=4'hf;wb=0;pulse_a;wa=0;ea=0;eb=1;pulse_b;check(1,data,parity);\ndata=~data;parity=~parity;wb=8'h0f;pulse_b;wb=0;eb=0;ea=1;pulse_a;check(0,data,parity);\nend\ncycle=16;addr=16'h0200;data=32'hxxxxxxxx;parity=4'hx;ea=1;eb=0;wa=4'hf;pulse_a;wa=0;ea=0;eb=1;pulse_b;check(1,data,parity);\ncycle=17;rst=0;pulse_b;check(1,32'h00000000,4'h0);\nif(count!=510) $fatal(1,\"incomplete RAM fixture count=%0d\",count);\n$display(\"MD_SIM_RAM_OMISSION_PASS cases=%0d\",count); $finish; end\nendmodule\n");
    Ok((
        source,
        json!({"instances":identities,"cases":510,
        "scope":"Exact selected parameters, driven two-port data/parity writes and reads, unknown data and reset. Differential omission versus explicit Z is a representation check, not full RAM, routing or physical timing acceptance."}),
    ))
}

pub fn check_omission_diagnostics(text: &str) -> Result<()> {
    let mut counts = [0usize; 4];
    for line in text.lines() {
        let line = line
            .strip_prefix("/work/ram-vectors.v:")
            .context("unrelated RAM diagnostic")?;
        let (number, diagnostic) = line.split_once(':').context("RAM diagnostic line")?;
        number.parse::<u32>()?;
        let index = PINS.iter().position(|(pin, number)| diagnostic == format!(
            " warning: Instantiating module RAMB36E1 with dangling input port {number} ({pin}) floating."))
            .context("unexpected RAM diagnostic")?;
        counts[index] += 1;
    }
    ensure!(
        counts == [15; 4],
        "RAM omitted diagnostic inventory differs"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_oracle_requires_every_expected_message_and_no_other_warning() {
        let text = PINS.iter().flat_map(|(pin, n)| (0..15).map(move |i| format!(
            "/work/ram-vectors.v:{i}: warning: Instantiating module RAMB36E1 with dangling input port {n} ({pin}) floating.\n"))).collect::<String>();
        check_omission_diagnostics(&text).unwrap();
        assert!(check_omission_diagnostics(&(text.clone() + "other warning\n")).is_err());
        assert!(check_omission_diagnostics(
            text.lines().skip(1).collect::<Vec<_>>().join("\n").as_str()
        )
        .is_err());
    }
    #[test]
    fn unsupported_active_modes_and_bad_parameter_structure_fail() {
        assert!(parameters(".RAM_MODE(\"TDP\"),\n.RAM_MODE(\"SDP\")").is_err());
        assert!(parameters(".X((4))").is_err());
        assert!(contract(&parameters(".RAM_MODE(\"SDP\")").unwrap()).is_err());
        assert!(generate("module top; endmodule").is_err());
    }
    #[test]
    fn active_ecc_cascade_and_unreviewed_widths_are_rejected() {
        let mut p = parameters(concat!(
            ".RAM_MODE(\"TDP\"),\n.DOA_REG(0),\n.DOB_REG(0),\n",
            ".IS_CLKARDCLK_INVERTED(0),\n.IS_CLKBWRCLK_INVERTED(0),\n",
            ".IS_ENARDEN_INVERTED(0),\n.IS_ENBWREN_INVERTED(0),\n",
            ".IS_RSTRAMARSTRAM_INVERTED(1),\n.IS_RSTRAMB_INVERTED(1),\n",
            ".IS_RSTREGARSTREG_INVERTED(1),\n.IS_RSTREGB_INVERTED(1),\n",
            ".SRVAL_A(36'h000000000),\n.SRVAL_B(36'h000000000),\n",
            ".READ_WIDTH_A(18),\n.READ_WIDTH_B(18),\n",
            ".WRITE_WIDTH_A(18),\n.WRITE_WIDTH_B(18)"
        ))
        .unwrap();
        assert_eq!(contract(&p).unwrap(), 18);
        for (key, value) in [
            ("EN_ECC_READ", "\"TRUE\""),
            ("EN_ECC_WRITE", "\"TRUE\""),
            ("RAM_EXTENSION_A", "\"UPPER\""),
            ("RAM_EXTENSION_B", "\"LOWER\""),
        ] {
            p.insert(key, value);
            assert!(contract(&p).is_err(), "{key}");
            p.remove(key);
        }
        p.insert("READ_WIDTH_B", "36");
        assert!(contract(&p).is_err());
        for key in [
            "READ_WIDTH_A",
            "READ_WIDTH_B",
            "WRITE_WIDTH_A",
            "WRITE_WIDTH_B",
        ] {
            p.insert(key, "1");
        }
        assert!(contract(&p).is_err());
    }
}

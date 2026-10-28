//! Compare the two exact converter outputs without accepting other RTL edits.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

const OPEN: [&str; 4] = ["CASCADEINA", "CASCADEINB", "INJECTDBITERR", "INJECTSBITERR"];

fn framing(source: &str, adapted: bool) -> Result<Vec<String>> {
    let lines: Vec<_> = source
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let module = usize::from(adapted);
    ensure!(
        lines.get(module) == Some(&"module top(")
            && lines.last() == Some(&"endmodule")
            && lines.iter().filter(|l| l.starts_with("module ")).count() == 1
            && lines.iter().filter(|l| **l == "endmodule").count() == 1
            && lines.iter().filter(|l| l.starts_with("`timescale")).count() == module,
        "top module framing differs"
    );
    ensure!(
        !adapted || lines.first() == Some(&"`timescale 1ps/1ps"),
        "top timebase is not before the module"
    );
    let end = lines
        .iter()
        .position(|l| *l == ");")
        .context("top port header end absent")?;
    ensure!(end > module && end < 128, "top port header bound");
    Ok(lines[module..=end]
        .iter()
        .map(|line| (*line).to_owned())
        .collect())
}

fn alias_names(diagnostics: &str) -> Result<BTreeSet<String>> {
    let mut names = BTreeSet::new();
    for line in diagnostics.lines() {
        let Some((_, rest)) = line.split_once("warning: implicit definition of wire '") else {
            continue;
        };
        let name = rest
            .strip_suffix("'.")
            .context("implicit-wire diagnostic format")?;
        ensure!(
            !name.is_empty()
                && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                && names.insert(name.to_owned()),
            "implicit-wire diagnostic differs"
        );
    }
    Ok(names)
}

#[derive(Debug, PartialEq)]
struct Cell {
    module: String,
    attribute: String,
    parameters_sha256: String,
    body: Vec<String>,
}

fn cells(source: &str, explicit: bool) -> Result<(BTreeMap<String, Cell>, usize)> {
    let lines: Vec<_> = source.lines().collect();
    let mut result = BTreeMap::new();
    let mut added = 0;
    for (start, line) in lines.iter().enumerate() {
        let Some(module) = line.strip_prefix("  ").and_then(|s| s.strip_suffix(" #(")) else {
            continue;
        };
        ensure!(
            !module.is_empty()
                && module
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_'),
            "cell module format"
        );
        let boundary = (start + 1..lines.len())
            .find(|i| lines[*i].starts_with("  ) "))
            .context("cell parameter boundary")?;
        ensure!(boundary - start < 256, "cell parameter bound");
        let name = lines[boundary]
            .strip_prefix("  ) ")
            .unwrap()
            .strip_suffix(" (")
            .context("cell name boundary")?;
        ensure!(
            !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
            "cell name format"
        );
        let end = (boundary + 1..lines.len())
            .find(|i| lines[*i] == "  );")
            .context("cell port boundary")?;
        ensure!(end - boundary < 1024, "cell port bound");
        let mut body = Vec::new();
        let mut pins = BTreeSet::new();
        for l in &lines[start..=end] {
            let pin = OPEN.iter().find(|pin| l.trim() == format!(".{pin}(1'bz),"));
            if let Some(pin) = pin {
                ensure!(
                    explicit && module == "RAMB36E1" && pins.insert(*pin),
                    "unreviewed explicit-Z port"
                );
                added += 1;
            } else {
                body.push(l.trim().to_owned());
            }
        }
        ensure!(
            !explicit || module != "RAMB36E1" || pins.len() == 4,
            "RAM explicit-Z ports incomplete"
        );
        let attribute = lines[..start]
            .iter()
            .rev()
            .find(|l| !l.trim().is_empty())
            .copied()
            .unwrap_or("");
        ensure!(
            attribute.trim().starts_with("(*") && attribute.trim().ends_with("*)"),
            "cell attributes absent"
        );
        ensure!(
            result
                .insert(
                    name.to_owned(),
                    Cell {
                        module: module.into(),
                        attribute: attribute.trim().into(),
                        parameters_sha256: md_sim_firmware::prepare::hash_bytes(
                            lines[start + 1..boundary].join("\n").as_bytes()
                        ),
                        body
                    }
                )
                .is_none(),
            "duplicate cell name"
        );
    }
    Ok((result, added))
}

fn lines(
    source: &str,
    aliases: &BTreeSet<String>,
    adapted: bool,
) -> Result<BTreeMap<String, usize>> {
    let mut result = BTreeMap::new();
    let mut removed_aliases = BTreeSet::new();
    let mut timebases = 0;
    let mut z = 0;
    for line in source.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if adapted && line == "`timescale 1ps/1ps" {
            timebases += 1;
            continue;
        }
        if adapted && OPEN.iter().any(|pin| line == format!(".{pin}(1'bz),")) {
            z += 1;
            continue;
        }
        if let Some(name) = line
            .strip_prefix("wire [0:0] ")
            .and_then(|s| s.strip_suffix(';'))
        {
            if aliases.contains(name) {
                ensure!(
                    adapted && removed_aliases.insert(name.to_owned()),
                    "alias was already declared or declared twice"
                );
                continue;
            }
        }
        *result.entry(line.to_owned()).or_default() += 1;
    }
    ensure!(
        !adapted || (timebases == 1 && z == 60 && &removed_aliases == aliases),
        "structure adaptation inventory differs"
    );
    Ok(result)
}

pub fn compare(original: &str, adapted: &str, diagnostics: &Value, ram: &Value) -> Result<Value> {
    ensure!(
        original.len() <= 32 << 20 && adapted.len() <= 32 << 20,
        "structure source bound"
    );
    ensure!(
        framing(original, false)? == framing(adapted, true)?,
        "top module header changed"
    );
    ensure!(
        diagnostics["selected_elaborate"]["code"] == 0
            && diagnostics["selected_elaboration_pass"] == false,
        "original diagnostic evidence differs"
    );
    let aliases = alias_names(
        diagnostics["selected_elaborate"]["stderr"]
            .as_str()
            .context("alias diagnostics absent")?,
    )?;
    ensure!(aliases.len() == 1453, "alias inventory differs");
    let lhs: BTreeSet<_> = original
        .lines()
        .filter_map(|line| line.trim().strip_prefix("assign "))
        .filter_map(|line| line.split_once(" = ").map(|(lhs, _)| lhs.to_owned()))
        .collect();
    ensure!(
        aliases.is_subset(&lhs),
        "new declaration is not an existing assignment sink"
    );
    ensure!(
        ram["ram_omission_differential_pass"] == true
            && ram["ram_vectors"]["cases"] == 510
            && ram["ram_omitted_execute"]["code"] == 0
            && ram["ram_explicit_z_execute"]["code"] == 0
            && ram["ram_omitted_execute"]["stdout"] == ram["ram_explicit_z_execute"]["stdout"],
        "RAM differential evidence differs"
    );
    let (before, _) = cells(original, false)?;
    let (after, ports) = cells(adapted, true)?;
    ensure!(
        before.len() == 11122 && before == after && ports == 60,
        "cell, parameter, port or attribute changed"
    );
    let mut names = BTreeSet::new();
    for instance in ram["ram_vectors"]["instances"]
        .as_array()
        .context("RAM inventory absent")?
    {
        let name = instance["selected_instance"]
            .as_str()
            .context("RAM name absent")?;
        let cell = before.get(name).context("RAM evidence cell absent")?;
        ensure!(
            cell.module == "RAMB36E1"
                && instance["parameter_text_sha256"] == cell.parameters_sha256
                && names.insert(name),
            "RAM parameter evidence differs"
        );
    }
    ensure!(
        names.len() == 15 && before.values().filter(|c| c.module == "RAMB36E1").count() == 15,
        "RAM evidence inventory incomplete"
    );
    ensure!(
        lines(original, &aliases, false)? == lines(adapted, &aliases, true)?,
        "unreviewed converter output change"
    );
    Ok(
        json!({"passed":true,"cells":before.len(),"declared_aliases":aliases.len(),"alias_width_bits":1,
        "explicit_open_inputs":ports,"RAM_instances":names.len(),"timebase":"1ps/1ps",
        "preserved":"Every existing nonblank line, cell identity, attributes, parameter text, port connection and assignment. Line order alone is normalized.",
        "limits":"This comparison does not validate the original recovered logic, physical timing, PS7 behavior or firmware execution."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framing_rejects_reordered_endmodule_and_late_timebase() {
        let original = "module top(\ninput clk\n);\nwire a;\nendmodule\n";
        let adapted = format!("`timescale 1ps/1ps\n{original}");
        assert_eq!(
            framing(original, false).unwrap(),
            framing(&adapted, true).unwrap()
        );
        assert!(framing(
            "`timescale 1ps/1ps\nmodule top(\ninput clk\n);\nendmodule\nwire a;\n",
            true
        )
        .is_err());
        assert!(framing(
            "module top(\ninput clk\n);\n`timescale 1ps/1ps\nendmodule\n",
            true
        )
        .is_err());
    }
    #[test]
    fn cell_comparison_detects_parameter_reassociation() {
        let a="  (* KEEP *)\n  LUT6_2 #(\n    .INIT(64'h0)\n  ) first (\n.I0(a)\n  );\n  (* KEEP *)\n  LUT6_2 #(\n    .INIT(64'h1)\n  ) second (\n.I0(b)\n  );\n";
        let b = a
            .replace("64'h0", "64'h2")
            .replace("64'h1", "64'h0")
            .replace("64'h2", "64'h1");
        assert_ne!(cells(a, false).unwrap().0, cells(&b, false).unwrap().0);
        assert_ne!(
            cells(a, false).unwrap().0,
            cells(&a.replace(".I0(a)", ".I0(c)"), false).unwrap().0
        );
    }
    #[test]
    fn alias_evidence_rejects_duplicates_and_invalid_identifiers() {
        let a = "/selected.v:1: warning: implicit definition of wire 'one'.\n";
        assert_eq!(alias_names(a).unwrap(), BTreeSet::from(["one".into()]));
        assert!(alias_names(&(a.to_owned() + a)).is_err());
        assert!(alias_names(&a.replace("'one'", "'a[0]'")).is_err());
    }
    #[test]
    fn wide_ps7_port_list_is_compared_and_z_cannot_hide_a_lut_connection() {
        let mut s = String::from("  (* KEEP *)\n  PS7 #(\n  ) ps (\n");
        for i in 0..600 {
            s.push_str(&format!(".P{i}(wire{i}),\n"));
        }
        s.push_str("  );\n");
        let original = cells(&s, false).unwrap().0;
        assert_ne!(
            original,
            cells(&s.replace(".P599(wire599)", ".P599(other)"), false)
                .unwrap()
                .0
        );
        assert!(cells(
            "  (* KEEP *)\n  LUT6_2 #(\n  ) lut (\n.CASCADEINA(1'bz),\n  );\n",
            true
        )
        .is_err());
    }
}

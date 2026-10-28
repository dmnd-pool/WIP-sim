//! The pinned CVC compiler/launcher adapter. Device and protocol logic stay shared.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{path::Path, process::Command};

pub const BANNER: &str = "Copyright (c) 1991-2014 Tachyon Design Automation Corp.\n  All Rights reserved.  Licensed software subject to prohibitions and\n  restrictions.  See OSS CVC artistic license included with release.\n";
pub const TOOL: &str = "/work/cvc-install/cvc64";
pub const PLUGIN: &str = "+loadvpi=/work/compiled/mdsim_live.vpi:mdsim_live_bootstrap";

pub fn body(stdout: &str) -> Result<&str> {
    let body = stdout
        .strip_prefix(BANNER)
        .context("exact CVC copyright banner missing")?;
    ensure!(!body.contains(BANNER), "CVC copyright banner repeated");
    ensure!(
        !["ERROR**", "WARN**", "WARNING**", "FATAL**", "INFORM"]
            .iter()
            .any(|s| body.contains(s)),
        "CVC runtime/compiler diagnostic"
    );
    Ok(body)
}
pub fn clean(step: &Value) -> Result<()> {
    ensure!(
        step["code"] == 0 && step["stderr"] == "",
        "CVC compiler failed or diagnosed"
    );
    ensure!(
        body(step["stdout"].as_str().context("compiler stdout")?)?.is_empty(),
        "unresolved CVC compiler output"
    );
    Ok(())
}
/// Source files are explicit roots. Vendor models are library files; this
/// avoids creating unrelated root instances. glbl is the explicit global root.
/// No port direction, two-state, warning-suppression or timing-disable flag is set.
pub fn compile_args(
    sources: &[&str],
    output: &str,
    vendor: &[String],
    defines: &[&str],
    live: bool,
) -> Vec<String> {
    let mut args = [TOOL, "-q", "-sv", "+acc+2", "+typdelays", "-o", output]
        .map(str::to_owned)
        .to_vec();
    args.extend(defines.iter().map(|d| format!("+define+{d}")));
    if live {
        args.push(PLUGIN.into());
    }
    args.extend(sources.iter().map(|s| (*s).to_owned()));
    for file in vendor {
        if file != "/vendor/glbl.v" {
            args.push("-v".into());
        }
        args.push(file.clone());
    }
    args
}
pub fn command(model: &str) -> Command {
    let mut command = Command::new(model);
    command.current_dir("/work");
    command
}
fn run(report: &mut Value, name: &str, args: &[String]) -> Result<()> {
    ensure!(super::run_step(report, name, args)?, "CVC step failed");
    Ok(())
}
fn reference_lines<'a>(row: &'a Value, source: &str, count: usize) -> Result<Vec<&'a str>> {
    ensure!(
        row["code"] == 0 && row["stderr"] == "",
        "Icarus reference failed"
    );
    let lines = row["stdout"]
        .as_str()
        .context("reference stdout")?
        .lines()
        .collect::<Vec<_>>();
    ensure!(lines.len() == count + 1, "Icarus reference line count");
    let footer = lines[count];
    let position = footer
        .strip_prefix(&format!("{source}:"))
        .context("Icarus finish source")?;
    let (line, rest) = position
        .split_once(": $finish called at ")
        .context("Icarus finish marker")?;
    ensure!(
        line.parse::<u64>().is_ok_and(|v| v > 0),
        "Icarus finish line"
    );
    let ticks = rest.strip_suffix(" (1ps)").context("Icarus finish unit")?;
    ensure!(
        ticks.parse::<u64>().is_ok_and(|v| v > 0),
        "Icarus finish time"
    );
    Ok(lines[..count].to_vec())
}
fn compare(report: &Value, key: &str, reference: &str, source: &str, count: usize) -> Result<()> {
    let row = &report[key];
    ensure!(
        row["code"] == 0 && row["stderr"] == "",
        "CVC behavioral oracle failed"
    );
    let actual = body(row["stdout"].as_str().context("CVC oracle stdout")?)?;
    let expected = reference_lines(&report[reference], source, count)?;
    ensure!(
        actual.lines().eq(expected),
        "CVC and Icarus behavioral observations differ"
    );
    Ok(())
}
pub fn vectors(report: &mut Value, vendor: &[String]) -> Result<()> {
    ensure!(
        vendor
            .iter()
            .filter(|s| s.as_str() == "/vendor/glbl.v")
            .count()
            == 1,
        "global vendor root differs"
    );
    for (label, source, defines, reference, count) in [
        (
            "primitive",
            "/vectors.v",
            &[][..],
            "primitive_execute",
            2usize,
        ),
        (
            "ram_omitted",
            "/work/ram-vectors.v",
            &[][..],
            "ram_omitted_execute",
            511,
        ),
        (
            "ram_explicit_z",
            "/work/ram-vectors.v",
            &["EXPLICIT_OPEN"][..],
            "ram_explicit_z_execute",
            511,
        ),
    ] {
        let program = format!("/work/compiled/{label}-cvc");
        let compile = format!("cvc_{label}_compile");
        run(
            report,
            &compile,
            &compile_args(&[source], &program, vendor, defines, false),
        )?;
        clean(&report[&compile])?;
        report[format!("cvc_{label}_program")] =
            json!(md_sim_runtime::store::hash_file(Path::new(&program))?);
        let execute = format!("cvc_{label}_execute");
        run(report, &execute, &[program])?;
        compare(report, &execute, reference, source, count)?;
    }
    report["cvc_primitive_vectors_pass"] = json!(true);
    report["cvc_ram_omission_differential_pass"] = json!(true);
    report["cvc_cross_engine_policy"] = json!({"four_state_observations":"All2 primitive observation lines and each511 RAM observation lines must match exactly. Only the already validated Icarus $finish source/position/time footer and exact CVC copyright banner have different presentation.","physical_timing_claim":false});
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn banner_is_exact_once_and_diagnostics_cannot_pass() {
        assert_eq!(body(&format!("{BANNER}actual\n")).unwrap(), "actual\n");
        for s in [
            "actual".to_string(),
            format!("{BANNER}{BANNER}"),
            format!("{BANNER}file WARN** warning"),
        ] {
            assert!(body(&s).is_err());
        }
        assert!(
            clean(&json!({"code":0,"stderr":"","stdout":format!("{BANNER}unexplained\n")}))
                .is_err()
        );
    }
    #[test]
    fn behavioral_comparison_rejects_changed_trace_and_bad_footer() {
        let mut r = json!({"a":{"code":0,"stderr":"","stdout":format!("{BANNER}TRACE q=xz01\n")},"b":{"code":0,"stderr":"","stdout":"TRACE q=xz01\n/source.v:4: $finish called at 5 (1ps)\n"}});
        compare(&r, "a", "b", "/source.v", 1).unwrap();
        r["a"]["stdout"] = json!(format!("{BANNER}TRACE q=0001\n"));
        assert!(compare(&r, "a", "b", "/source.v", 1).is_err());
        r["a"]["stdout"] = json!(format!("{BANNER}TRACE q=xz01\n"));
        r["b"]["stdout"] = json!("TRACE q=xz01\n/source.v:4: $finish called at bad (1ps)\n");
        assert!(compare(&r, "a", "b", "/source.v", 1).is_err());
    }
}

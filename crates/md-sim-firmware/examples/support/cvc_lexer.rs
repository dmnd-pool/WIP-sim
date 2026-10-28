//! Small library-skip reproducer. It changes no archived vendor model.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{fs, path::Path};
const LIBRARY: &str = include_str!("../../../../simulator/backends/cvc/lexer-library.v");
const TOP: &str = include_str!("../../../../simulator/backends/cvc/lexer-top.v");
const PASS: &str = "MD_CVC_LEXER_PASS cases=3 time=3 output=x1x0";
fn outcome(row: &Value) -> Result<()> {
    super::clean(row)?;
    let stdout = row["stdout"].as_str().context("lexer stdout")?;
    ensure!(
        stdout
            .lines()
            .filter(|s| s.starts_with("MD_CVC_"))
            .collect::<Vec<_>>()
            == [PASS],
        "lexer actual observation missing or changed"
    );
    Ok(())
}
pub fn probe(report: &mut Value) -> Result<()> {
    fs::write("/work/lexer-library.v", LIBRARY)?;
    fs::write("/work/lexer-top.v", TOP)?;
    report["claim"]=json!("Actual literal library/conditional/implicit-event-control parser fixture. No selected model or firmware gate.");
    report["expected_observation"] = json!(PASS);
    report["lexer_sources"] = json!({"library":md_sim_firmware::prepare::hash_bytes(LIBRARY.as_bytes()),"top":md_sim_firmware::prepare::hash_bytes(TOP.as_bytes())});
    ensure!(
        super::run(
            report,
            "lexer_icarus_compile",
            "/work/iverilog-install/bin/iverilog",
            &[
                "-g2012",
                "-Wall",
                "-DMD_PRESENT",
                "-s",
                "lexer_top",
                "-o",
                "/work/lexer.vvp",
                "/work/lexer-top.v",
                "/work/lexer-library.v"
            ]
            .map(str::to_owned)
        )?,
        "Icarus lexical fixture compile failed"
    );
    super::clean(&report["lexer_icarus_compile"])?;
    report["lexer_icarus_program"] = json!(md_sim_runtime::store::hash_file(Path::new(
        "/work/lexer.vvp"
    ))?);
    let _ = super::run(
        report,
        "lexer_icarus_execute",
        "/work/iverilog-install/bin/vvp",
        &["/work/lexer.vvp"].map(str::to_owned),
    )?;
    outcome(&report["lexer_icarus_execute"])?;
    report["lexer_icarus_pass"] = json!(true);
    ensure!(
        super::run(
            report,
            "lexer_cvc_compile",
            "/work/cvc-install/cvc64",
            &[
                "-q",
                "-sv",
                "+define+MD_PRESENT",
                "-o",
                "/work/lexer-cvc",
                "/work/lexer-top.v",
                "-v",
                "/work/lexer-library.v"
            ]
            .map(str::to_owned)
        )?,
        "CVC literal library fixture compile failed"
    );
    super::clean(&report["lexer_cvc_compile"])?;
    report["lexer_cvc_program"] = json!(md_sim_runtime::store::hash_file(Path::new(
        "/work/lexer-cvc"
    ))?);
    let _ = super::run(report, "lexer_cvc_execute", "/work/lexer-cvc", &[])?;
    outcome(&report["lexer_cvc_execute"])?;
    report["lexer_cvc_pass"] = json!(true);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_or_wrong_observation_fails() {
        let mut row = json!({"code":0,"stdout":format!("{PASS}\n"),"stderr":""});
        outcome(&row).unwrap();
        row["stdout"] = json!(format!("{PASS}\n{PASS}\n"));
        assert!(outcome(&row).is_err());
        row["stdout"] = json!(PASS.replace("x1x0", "0100"));
        assert!(outcome(&row).is_err());
    }
}

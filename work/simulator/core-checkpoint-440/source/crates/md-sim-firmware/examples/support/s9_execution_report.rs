//! Shared evidence retention for local and worker execution of the same process.
use super::{boot_storage, pool_report, pool_vm};
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{save_json, ProcessResult, ProcessSpec},
    store::hash_file,
};
use serde_json::json;
use std::{fs, io::Read, path::Path};
#[path = "s9_external_pool.rs"]
mod external_pool;
#[path = "pcap_audit.rs"]
mod pcap_audit;

fn retain_process_result(path: &Path, result: &ProcessResult) -> Result<()> {
    match fs::File::open(path) {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take((257 << 20) + 1).read_to_end(&mut bytes)?;
            ensure!(bytes.len() <= 257 << 20, "retained result size bound");
            let saved: ProcessResult = serde_json::from_slice(&bytes)?;
            ensure!(
                serde_json::to_value(&saved)? == serde_json::to_value(result)?,
                "retained process result differs from supplied execution"
            );
            // The transport hash identifies these original bytes, including
            // their JSON layout. Observation reports must use that identity.
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => save_json(path, result),
        Err(error) => Err(error.into()),
    }
}

pub fn retain(out: &Path, spec: &ProcessSpec, result: &ProcessResult) -> Result<()> {
    let flag = |name: &str| -> Result<bool> {
        match spec.environment.get(name).map(String::as_str) {
            Some("1") => Ok(true),
            Some("0") => Ok(false),
            _ => anyhow::bail!("missing or invalid execution flag: {name}"),
        }
    };
    let save_sd = flag("MD_SIM_S9_SAVE_SD")?;
    let local_pool = flag("MD_SIM_S9_LOCAL_POOL")?;
    let external = external_pool::selected(spec)?;
    let use_pool_vm = flag("MD_SIM_S9_POOL_VM")?;
    let browser = flag("MD_SIM_S9_BROWSER")?;
    let dashboard_probe = flag("MD_SIM_S9_DASHBOARD_PROBE")?;
    let dashboard_update = spec
        .environment
        .contains_key(pool_vm::update_validation::ENVIRONMENT);
    let restarts: usize = spec
        .environment
        .get("MD_SIM_S9_RESTARTS")
        .context("restart count absent")?
        .parse()?;
    ensure!(restarts <= 2, "restart count bound");
    let console = spec
        .argv
        .get(1)
        .is_some_and(|value| value == "__boot-console");
    retain_process_result(&out.join("result.json"), result)?;
    let storage_retention = if save_sd {
        boot_storage::retain(out, result, restarts + 1)
    } else {
        Ok(())
    };
    if let Err(error) = &storage_retention {
        // A truncated snapshot remains invalid. Preserve its error and still
        // retain independent native HTTP and pool observations that preceded
        // it. Otherwise a JSON EOF hides the original firmware/test failure.
        save_json(
            &out.join("sd-storage-failure.json"),
            &json!({"passed":false,"error":format!("{error:#}"),
                "execution_result":hash_file(&out.join("result.json"))?,
                "execution_status":result.status,"exit_code":result.exit_code,
                "trace_truncated":result.trace_truncated,
                "scope":"Snapshot retention failed. Partial bytes remain in the guest log; they are not a valid saved disk or a passing result."}),
        )?;
    }
    let nand_retention = match spec
        .environment
        .get("MD_SIM_S9_NAND_STORAGE")
        .map(String::as_str)
    {
        None | Some("0") => Ok(()),
        Some("1") => {
            ensure!(save_sd, "NAND capture without paired SD capture");
            boot_storage::retain_storage(boot_storage::NAND, out, result, restarts + 1)
        }
        _ => anyhow::bail!("invalid NAND storage policy"),
    };
    if let Err(error) = &nand_retention {
        save_json(
            &out.join("nand-storage-failure.json"),
            &json!({
                "passed":false,"error":format!("{error:#}"),
                "execution_result":hash_file(&out.join("result.json"))?,
                "scope":"Invalid or incomplete NAND capture. Retained bytes do not establish successful persistence."
            }),
        )?;
    }
    let mut dashboard_passed = !dashboard_probe;
    if external {
        let report = external_pool::verify(out, false)?;
        save_json(&out.join("pool-result.json"), &report)?;
    } else if local_pool {
        let stdout = hex::decode(&result.stdout)?;
        let text = std::str::from_utf8(&stdout).context("pool runner output is not UTF-8")?;
        if use_pool_vm {
            pool_vm::retain_miner_packets(out, text, browser, dashboard_update)?;
        }
        if dashboard_probe {
            let records = pool_vm::markers(text, "MD_SIM_DASHBOARD_PROBE ")?;
            dashboard_passed = records.len() == 1 && records[0]["passed"] == true;
            save_json(
                &out.join("dashboard-result.json"),
                &json!({
                    "observations":records,"execution_result":hash_file(&out.join("result.json"))?,
                    "process_specification":hash_file(&out.join("process-spec.json"))?,
                    "full_dashboard_gate":"incomplete"
                }),
            )?;
        }
        if browser {
            let records = pool_vm::markers(text, "MD_SIM_BOSER_BROWSER_RESULT ")?;
            dashboard_passed &= records.len() == 1 && records[0]["passed"] == true;
            save_json(
                &out.join("browser-result.json"),
                &json!({
                    "observations":records,"failures":pool_vm::markers(text,"MD_SIM_BOSER_BROWSER_FAILURE ")?,
                    "execution_result":hash_file(&out.join("result.json"))?,
                    "process_specification":hash_file(&out.join("process-spec.json"))?,
                    "full_dashboard_gate":"incomplete"
                }),
            )?;
        }
        let sessions = text
            .lines()
            .filter_map(|line| line.strip_prefix(pool_report::MARKER))
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let envelope = pool_report::select_session(&sessions)?;
        let collection = text
            .lines()
            .filter_map(|line| line.strip_prefix("MD_SIM_PRIVATE_POOL_COLLECTION "))
            .map(serde_json::from_str::<serde_json::Value>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        save_json(
            &out.join("pool-result.json"),
            &json!({
                "runner_completed_pool_report":envelope.is_some(),
                "execution_result":hash_file(&out.join("result.json"))?,
                "process_specification":hash_file(&out.join("process-spec.json"))?,
                "adaptation":hash_file(&out.join("adaptation.json"))?,
                "pool_input":hash_file(&out.join("pool-input.json"))?,
                "execution_status":result.status,
                "trace_truncated":result.trace_truncated,
                "pool":envelope,
                "pool_sessions":sessions,
                "pool_collection":collection,
                "pool_selection":"First authorized V1 session in allocation order. All completed reports are retained; selection is independent of pass/fail.",
                "full_board_gate":"incomplete"
            }),
        )?;
    }
    println!(
        "{}",
        json!({"status":result.status,"exit_code":result.exit_code,"trace_truncated":result.trace_truncated,"original_board_gate":"incomplete"})
    );
    if console {
        ensure!(
            result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
            "guest console diagnostic failed; see retained result and logs"
        );
    }
    ensure!(
        dashboard_passed,
        "native dashboard probe failed or lacks evidence"
    );
    storage_retention?;
    nand_retention?;
    pcap_audit::retain_if_selected(out, spec, result, restarts + 1)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_retains_original_result_layout_and_rejects_changed_execution() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("result.json");
        let raw = br#"{ "tools":{}, "limits_scope":"test", "isolation":"test", "trace_truncated":false, "stderr":"", "stdout":"68690a", "wall_milliseconds":3, "exit_code":0, "status":"exited" }"#;
        let mut result: ProcessResult = serde_json::from_slice(raw).unwrap();
        fs::write(&path, raw).unwrap();
        retain_process_result(&path, &result).unwrap();
        assert_eq!(fs::read(&path).unwrap(), raw);
        result.exit_code = Some(1);
        assert!(retain_process_result(&path, &result).is_err());
        assert_eq!(fs::read(&path).unwrap(), raw);
        fs::write(&path, b"invalid JSON").unwrap();
        assert!(retain_process_result(&path, &result).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"invalid JSON");
    }
}

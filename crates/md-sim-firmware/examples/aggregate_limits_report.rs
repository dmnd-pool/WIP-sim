//! Host-side checks of captured VM resource-limit evidence.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{Limits, Mount, ProcessResult, ProcessSpec},
    store::{hash_file, ObjectId},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Read, path::Path};

fn read(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(maximum + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= maximum,
        "evidence exceeds bound: {}",
        path.display()
    );
    Ok(bytes)
}
fn identity(bytes: &[u8]) -> Value {
    json!({"sha256":hex::encode(Sha256::digest(bytes)),"size":bytes.len()})
}
fn object_for<'a>(mounts: &'a [Mount], path: &str) -> Result<&'a ObjectId> {
    let found: Vec<_> = mounts.iter().filter(|m| m.guest_path == path).collect();
    ensure!(
        found.len() == 1,
        "input identity must occur exactly once: {path}"
    );
    Ok(&found[0].object)
}
fn verify_objects(directory: &Path, mounts: &[Mount]) -> Result<()> {
    for mount in mounts {
        ensure!(
            mount.object.sha256.len() == 64
                && mount
                    .object
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid input hash"
        );
        ensure!(
            hash_file(
                &directory
                    .join("input-store/objects")
                    .join(&mount.object.sha256)
            )? == mount.object,
            "saved input bytes differ: {}",
            mount.guest_path
        );
    }
    Ok(())
}
fn check_recovery(event: &Value, inputs: &[Mount]) -> Result<()> {
    ensure!(
        hex::decode(
            event["readiness_stdout"]
                .as_str()
                .context("worker readiness absent")?
        )? == b"actual sleeping workload started\n"
            && event["populated_processes"]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty()
                    && s.lines()
                        .all(|line| line.parse::<u32>().is_ok_and(|pid| pid > 0))),
        "actual worker workload was not recorded before loss"
    );
    ensure!(
        event["actual_sigkill"] == true
            && event["same_socket_restart"] == true
            && event["unrelated_parent_preserved"] == true,
        "worker loss test was not executed"
    );
    let before = &event["before"];
    let after = &event["after"];
    ensure!(
        before["schema"] == "md-sim-cgroup/1"
            && before["state"] == "created"
            && after["state"] == "removed"
            && before["inode"].as_u64().is_some_and(|n| n > 0),
        "invalid cgroup recovery states"
    );
    let mut expected = before.clone();
    expected["state"] = json!("removed");
    ensure!(
        &expected == after,
        "recovery changed the owned group identity"
    );
    let name = before["name"].as_str().context("cgroup name absent")?;
    ensure!(
        before["parent"] == "/sys/fs/cgroup/md-sim-check/runs"
            && name.len() == 39
            && name.starts_with("md-sim-")
            && name[7..]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid cgroup owner"
    );
    let run = &event["lost_run"];
    let token = "c3".repeat(32);
    ensure!(
        run["found"] == true
            && run["state"] == "lost"
            && run["token"] == token
            && run["run_id"] == format!("run-{token}"),
        "lost run identity differs"
    );
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec!["/probe".into(), "workload".into(), "sleep-child".into()],
        inputs: inputs.to_vec(),
        environment: BTreeMap::new(),
        stdin_object: None,
        limits: Limits::default(),
    };
    ensure!(
        run["token_process_identity"] == identity(&serde_json::to_vec(&spec)?),
        "lost process identity differs"
    );
    Ok(())
}
fn check_capture(
    result_bytes: &[u8],
    log: &[u8],
    spec_bytes: &[u8],
    assessment: &Value,
) -> Result<ProcessResult> {
    let result: ProcessResult = serde_json::from_slice(result_bytes)?;
    let spec: ProcessSpec = serde_json::from_slice(spec_bytes)?;
    spec.validate()?;
    ensure!(
        result.status == "exited"
            && result.exit_code == Some(0)
            && !result.trace_truncated
            && result.stderr.is_empty()
            && hex::decode(&result.stdout)? == log,
        "VM did not exit cleanly with the saved complete log"
    );
    ensure!(
        assessment["guest_boot_and_tcp_probe"] == true
            && assessment["guest_log"] == identity(log)
            && assessment["process_result"] == identity(result_bytes)
            && assessment["process_spec"] == identity(spec_bytes),
        "VM assessment does not bind the captured execution"
    );
    Ok(result)
}
fn report(directory: &Path, prepared: &Path, output: &Path) -> Result<()> {
    ensure!(!output.try_exists()?, "report output must be new");
    let result_bytes = read(&directory.join("result.json"), 4 << 20)?;
    let spec_bytes = read(&directory.join("process-spec.json"), 1 << 20)?;
    let log = read(&directory.join("guest.log"), 2 << 20)?;
    let assessment: Value =
        serde_json::from_slice(&read(&directory.join("assessment.json"), 65536)?)?;
    check_capture(&result_bytes, &log, &spec_bytes, &assessment)?;
    let text = std::str::from_utf8(&log)?;
    ensure!(
        text.lines()
            .filter(|l| l.trim_end() == "MD_SIM_VM_SCENARIO selected guest command exited 0")
            .count()
            == 1,
        "scenario success record absent or duplicated"
    );
    let records: Vec<_> = text
        .lines()
        .filter_map(|l| l.strip_prefix("MD_SIM_AGGREGATE_EVIDENCE "))
        .collect();
    ensure!(records.len() == 1, "aggregate evidence must occur once");
    let evidence: Value = serde_json::from_str(records[0])?;
    let root: Vec<Mount> =
        serde_json::from_slice(&read(&directory.join("root-manifest.json"), 1 << 20)?)?;
    let outer: ProcessSpec =
        serde_json::from_slice(&read(&directory.join("process-spec.json"), 1 << 20)?)?;
    verify_objects(directory, &root)?;
    verify_objects(directory, &outer.inputs)?;
    ensure!(
        outer
            .argv
            .windows(2)
            .filter(|a| a[0] == "-nic" && a[1] == "none")
            .count()
            == 1,
        "VM does not declare the selected disconnected topology"
    );
    let driver = hash_file(&prepared.join("driver"))?;
    ensure!(
        object_for(&root, "/aggregate-check")? == &driver
            && object_for(&root, "/probe")? == &driver
            && evidence["driver"] == serde_json::to_value(&driver)?
            && evidence["uid"] == 1000
            && evidence["passed"] == true,
        "captured workload executable or role differs"
    );
    let input_bytes = read(&prepared.join("inputs.json"), 1 << 20)?;
    ensure!(
        serde_json::to_value(object_for(&root, "/aggregate-inputs.json")?)?
            == identity(&input_bytes),
        "prepared workload manifest differs"
    );
    let inputs: Vec<Mount> = serde_json::from_slice(&input_bytes)?;
    for input in &inputs {
        ensure!(
            object_for(&root, &input.guest_path)? == &input.object,
            "nested workload input differs"
        );
    }
    let events = evidence["events"].as_array().context("events absent")?;
    ensure!(events.len() == 6, "required case count differs");
    let mut summary = Vec::new();
    for (event, case) in events
        .iter()
        .zip(["ok", "cpu", "memory", "pids", "cancel", "worker-loss"])
    {
        ensure!(event["case"] == case, "required event order differs");
        if case == "worker-loss" {
            check_recovery(event, &inputs)?;
            summary.push(json!({"case":case,"passed":true,"same_endpoint_restart":true}));
            continue;
        }
        let spec: ProcessSpec = serde_json::from_value(event["spec"].clone())?;
        spec.validate()?;
        ensure!(
            serde_json::to_value(&spec.inputs)? == serde_json::to_value(&inputs)?
                && spec.argv
                    == [
                        "/probe",
                        "workload",
                        if case == "cancel" { "sleep" } else { case }
                    ]
                && spec.environment.is_empty()
                && spec.stdin_object.is_none(),
            "workload specification differs"
        );
        let r: ProcessResult = serde_json::from_value(event["result"].clone())?;
        let a = r
            .aggregate_limits
            .as_ref()
            .context("aggregate kernel counters absent")?;
        ensure!(
            !r.trace_truncated
                && r.wall_milliseconds <= u128::from(spec.limits.wall_seconds) * 1000 + 3000
                && a["mechanism"] == "linux-cgroup-v2"
                && a["empty_before_removal"] == true
                && a["memory_max"] == spec.limits.memory_bytes
                && a["pids_max"] == spec.limits.processes
                && a["swap_max"] == 0
                && a["cpu_total_budget_usec"] == spec.limits.cpu_seconds * 1_000_000,
            "kernel evidence does not match the selected limits"
        );
        match case {
            "ok" => ensure!(
                r.status == "exited"
                    && r.exit_code == Some(0)
                    && hex::decode(&r.stdout)? == b"actual bounded guest execution\n",
                "normal workload failed"
            ),
            "cpu" => ensure!(
                r.status == "cpu-limited"
                    && a["cpu"]["usage_usec"]
                        .as_u64()
                        .is_some_and(|n| (2_000_000..2_500_000).contains(&n)),
                "aggregate CPU enforcement failed"
            ),
            "memory" => ensure!(
                r.status == "memory-limited"
                    && a["memory_events"]["oom_kill"]
                        .as_u64()
                        .is_some_and(|n| n > 0)
                    && a["memory_events"]["oom_group_kill"]
                        .as_u64()
                        .is_some_and(|n| n > 0),
                "kernel group OOM enforcement failed"
            ),
            "pids" => {
                let p: Value = serde_json::from_slice(&hex::decode(&r.stdout)?)?;
                ensure!(
                    r.exit_code == Some(0)
                        && p["process_creation_bounded"] == true
                        && p["errno"] == libc::EAGAIN,
                    "process creation bound was not observed"
                );
            }
            "cancel" => {
                let trigger = &event["cancel_trigger"];
                let readiness = hex::decode(
                    trigger["readiness_stdout"]
                        .as_str()
                        .context("cancel readiness absent")?,
                )?;
                ensure!(
                    r.status == "cancelled"
                        && r.wall_milliseconds < 3000
                        && readiness == b"actual sleeping workload started\n".repeat(4)
                        && hex::decode(&r.stdout)?.starts_with(&readiness)
                        && trigger["receipt"]["state"] == "created"
                        && trigger["populated_processes"].as_str().is_some_and(|s| s
                            .lines()
                            .count()
                            >= 4
                            && s.lines()
                                .all(|line| line.parse::<u32>().is_ok_and(|pid| pid > 0))),
                    "cancellation of running descendants was not recorded"
                );
            }
            _ => unreachable!(),
        }
        summary.push(json!({"case":case,"passed":true,"status":r.status,"wall_milliseconds":r.wall_milliseconds,"kernel_counters":a}));
    }
    let report = json!({"schema":"md-sim-aggregate-check/1","passed":true,
        "scope":"captured test workloads in one disconnected local VM; no firmware or external-host claim",
        "reporter":hash_file(&std::env::current_exe()?)?,"reporter_source":identity(include_bytes!("aggregate_limits_report.rs")),
        "driver":driver,"guest_log":hash_file(&directory.join("guest.log"))?,
        "process_spec":hash_file(&directory.join("process-spec.json"))?,"result":hash_file(&directory.join("result.json"))?,
        "root_manifest":hash_file(&directory.join("root-manifest.json"))?,"events":summary,
        "limitations":["PID creation reaches the per-UID rlimit before the cgroup limit in this workload; no cgroup PID fault claim.",
        "CPU is a total usage budget checked every 10 ms; scheduling and cleanup can cause bounded overshoot."]});
    fs::write(output, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "{}",
        json!({"passed":true,"report":output,"exact_firmware_target_pass":false})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_changed_assessment_errors_and_invalid_budget() -> Result<()> {
        let log = b"actual test output";
        let result = json!({"status":"exited","exit_code":0,"wall_milliseconds":1,"stdout":hex::encode(log),
            "stderr":"","trace_truncated":false,"isolation":"test","limits_scope":"test","tools":{}});
        let mut spec = ProcessSpec {
            support_virtualization: None,
            private_channel: None,
            stdin_stream: None,
            namespace_uid: 0,
            dashboard: None,
            loopback: false,
            schema_version: "2.0".into(),
            argv: vec!["/test".into()],
            inputs: vec![Mount {
                guest_path: "/test".into(),
                object: ObjectId {
                    sha256: "a".repeat(64),
                    size: 1,
                },
            }],
            environment: BTreeMap::new(),
            stdin_object: None,
            limits: Limits::default(),
        };
        let result_bytes = serde_json::to_vec(&result)?;
        let spec_bytes = serde_json::to_vec(&spec)?;
        let mut assessment = json!({"guest_boot_and_tcp_probe":true,"guest_log":identity(log),
            "process_result":identity(&result_bytes),"process_spec":identity(&spec_bytes)});
        check_capture(&result_bytes, log, &spec_bytes, &assessment)?;
        for field in ["guest_log", "process_result", "process_spec"] {
            let mut changed = assessment.clone();
            changed[field] = identity(b"different");
            assert!(check_capture(&result_bytes, log, &spec_bytes, &changed).is_err());
        }
        let mut error = result;
        error["stderr"] = json!(hex::encode(b"actual backend error"));
        let error_bytes = serde_json::to_vec(&error)?;
        assessment["process_result"] = identity(&error_bytes);
        assert!(check_capture(&error_bytes, log, &spec_bytes, &assessment).is_err());
        spec.limits.cpu_seconds = u64::MAX;
        let invalid = serde_json::to_vec(&spec)?;
        assessment["process_result"] = identity(&result_bytes);
        assessment["process_spec"] = identity(&invalid);
        assert!(check_capture(&result_bytes, log, &invalid, &assessment).is_err());
        Ok(())
    }
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 4,
        "usage: aggregate_limits_report RUN_DIR PREPARED_DIR NEW_REPORT"
    );
    report(
        Path::new(&args[1]),
        Path::new(&args[2]),
        Path::new(&args[3]),
    )
}

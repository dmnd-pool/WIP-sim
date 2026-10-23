//! Validate retained observations from the actual local/SSH byte bridge suite.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{dashboard::Binding, isolation::ProcessResult};
use serde_json::Value;

pub fn verify_artifact_recovery(proof: &Value) -> Result<()> {
    use md_sim_runtime::store::ObjectId;
    const PROTOCOL_VERSION: &str = "md-sim-worker/2";
    let cases = proof["cases"]
        .as_array()
        .context("artifact recovery cases absent")?;
    ensure!(
        cases.len() == 2 && proof["firmware_gate"] == false,
        "artifact recovery scope differs"
    );
    for (case, operation) in cases.iter().zip(["chunk", "finish"]) {
        let object: ObjectId = serde_json::from_value(case["object"].clone())?;
        object.validate()?;
        let worker: md_sim_coordinator::ExecutorIdentity =
            serde_json::from_value(case["worker_before"].clone())?;
        worker.build.validate()?;
        let expected_offset = if operation == "chunk" {
            md_sim_runtime::store::MAX_CHUNK as u64
        } else {
            object.size
        };
        let error = case["interrupted_error"]
            .as_str()
            .context("interrupted transfer error absent")?;
        let withheld = &case["withheld_reply"];
        let response = &withheld["worker_response"];
        ensure!(
            case["operation"] == operation
                && object.size == super::ssh_artifact_ack_loss::SIZE
                && worker.kind == "worker-isolated-process"
                && worker.policy["protocol"] == PROTOCOL_VERSION
                && case["worker_before"] == case["worker_after"]
                && error.contains("SSH channel ended before a complete response")
                && super::ssh_artifact_ack_loss::observation(error)? == *withheld
                && case["automatic_mutation_reconnects"] == 0
                && withheld["operation"] == operation
                && withheld["object"] == case["object"]
                && withheld["response_withheld"] == true
                && response["ok"] == true
                && response["protocol"] == PROTOCOL_VERSION
                && case["resume_begin"]["offset"] == expected_offset
                && case["destination_finish"]["verified"] == case["object"]
                && case["retrieved"] == case["object"]
                && case["same_fresh_bytes"] == true
                && case["production_transfer_resumed"] == true,
            "artifact acknowledgement recovery identity or state differs"
        );
        if operation == "chunk" {
            ensure!(
                withheld["offset"] == 0 && response["result"]["offset"] == expected_offset,
                "lost chunk acknowledgement differs"
            );
        } else {
            ensure!(
                withheld["offset"].is_null() && response["result"]["verified"] == case["object"],
                "lost publication acknowledgement differs"
            );
        }
    }
    ensure!(
        cases[0]["object"] != cases[1]["object"],
        "artifact recovery reused a fixture"
    );
    Ok(())
}

pub fn verify_live_reconnect(proof: &Value, run_id: &str) -> Result<()> {
    ensure!(
        proof["run_id"] == run_id
            && proof["reconnects"] == 3
            && proof["forced_server_channel_close_after_each_reply"] == true,
        "live reconnect count or selected run differs"
    );
    let reads = proof["reads"]
        .as_array()
        .context("live reconnect reads absent")?;
    ensure!(reads.len() == 4, "live reconnect read sequence incomplete");
    for (index, read) in reads.iter().enumerate() {
        let request = &read["request"];
        let response = &read["response"];
        ensure!(
            request["run_id"] == run_id && response["run_id"] == run_id,
            "live reconnect read selected a different run"
        );
        if index == 0 || index == 3 {
            ensure!(
                request["operation"] == "status" && response["state"]["state"] == "booting",
                "selected run did not remain active across reconnect"
            );
        } else {
            let (stream, bytes): (&str, &[u8]) = if index == 1 {
                ("stdout", b"coordinator live stdout\n")
            } else {
                ("stderr", b"coordinator live stderr\n")
            };
            ensure!(
                request["operation"] == "logs"
                    && request["stream"] == stream
                    && request["offset"] == 0
                    && request["count"] == 4096
                    && response["stream"] == stream
                    && response["offset"] == 0
                    && response["hex"] == hex::encode(bytes)
                    && response["next_offset"] == bytes.len() as u64
                    && response["saved_bytes"] == bytes.len() as u64
                    && response["process_result_complete"] == false,
                "live logs after reconnect differ or process had already completed"
            );
        }
    }
    Ok(())
}

pub fn verify_stream(result: &ProcessResult, expected: &Value) -> Result<()> {
    ensure!(
        result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
        "dashboard fixture execution failed"
    );
    let text = String::from_utf8(hex::decode(&result.stdout)?)?;
    let records: Vec<_> = text
        .lines()
        .filter_map(|s| s.strip_prefix("MD_SIM_DASHBOARD_STREAM "))
        .collect();
    ensure!(
        records.len() == 1 && serde_json::from_str::<Value>(records[0])? == *expected,
        "dashboard reached another process or bytes differ"
    );
    ensure!(
        expected["request_bytes"] == 65568
            && expected["early_response_before_request_eof"] == true
            && expected["completed_connections"] == 1,
        "dashboard duplex observation differs"
    );
    for name in ["request_sha256", "guest_challenge"] {
        let value = expected[name]
            .as_str()
            .context("dashboard challenge absent")?;
        ensure!(
            value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()),
            "dashboard challenge invalid"
        );
    }
    let dashboard = result
        .dashboard
        .as_ref()
        .context("dashboard receipt missing")?;
    ensure!(
        dashboard["supervisor_healthy"] == true
            && dashboard["listener_closed"] == true
            && dashboard["connections_completed"] == 1
            && dashboard["connections_failed"] == 0
            && dashboard["forwarded_bytes"] == 65664,
        "dashboard lifecycle or byte count differs"
    );
    Ok(())
}

pub fn verify_report(report: &Value) -> Result<()> {
    ensure!(report["passed"] == true, "SSH report did not pass");
    let events = report["events"].as_array().context("SSH events absent")?;
    let one = |name: &str| -> Result<&Value> {
        let matching: Vec<_> = events.iter().filter_map(|event| event.get(name)).collect();
        ensure!(matching.len() == 1, "missing or duplicate {name}");
        Ok(matching[0])
    };
    let live = one("actual_coordinator_live_ssh")?;
    verify_artifact_recovery(one("actual_artifact_reply_loss")?)?;
    let run: md_sim_coordinator::LockedRun = serde_json::from_value(live["locked_run"].clone())?;
    let run_id = live["worker_run"]["run_id"]
        .as_str()
        .context("live selected run absent")?;
    verify_live_reconnect(&live["live_reconnect"], run_id)?;
    let result: ProcessResult = serde_json::from_value(live["final_process"].clone())?;
    ensure!(
        live["locked_run_hash"] == run.hash()?
            && live["worker_run"]["identity"] == live["executor"]
            && live["observed_both_streams_before_stop"] == true
            && live["firmware_gate"] == false
            && result.status == "cancelled"
            && !result.trace_truncated
            && hex::decode(&result.stdout)? == b"coordinator live stdout\n"
            && hex::decode(&result.stderr)? == b"coordinator live stderr\n"
            && live["cleanup"]["run_id"] == run_id
            && live["cleanup"]["removed"] == true
            && live["cleanup"]["acknowledged_result"] == live["verified_result"],
        "live coordinator result, identity or cleanup differs"
    );
    let local = one("dashboard_stream_local")?;
    let unix = one("dashboard_stream_unix")?;
    let remote = one("dashboard_stream_ssh")?;
    for entry in [local, unix, remote] {
        let result: ProcessResult = serde_json::from_value(entry["result"].clone())?;
        verify_stream(&result, &entry["observation"])?;
        ensure!(
            result
                .dashboard
                .as_ref()
                .context("dashboard receipt absent")?["binding"]
                == entry["binding"],
            "live dashboard binding differs from completed result"
        );
        let binding: Binding = serde_json::from_value(entry["binding"].clone())?;
        ensure!(
            binding.source == md_sim_runtime::dashboard::Source::SimulatorConsole
                && binding.listen.ip() == std::net::Ipv4Addr::LOCALHOST
                && binding.guest_port == 18080
                && binding.local_socket.is_some()
                && !binding.firmware_readiness_established,
            "transport fixture source or endpoint differs"
        );
    }
    ensure!(
        local["binding"]["process_identity"] == remote["binding"]["process_identity"]
            && local["binding"]["process_identity"] == unix["binding"]["process_identity"]
            && remote["same_process_specification"] == true
            && remote["native_firmware_gate"] == false,
        "local and SSH dashboard process identities differ"
    );
    let cancelled = one("dashboard_cancel_ssh")?;
    let result: ProcessResult = serde_json::from_value(cancelled["result"].clone())?;
    ensure!(
        result.status == "cancelled"
            && !result.trace_truncated
            && cancelled["open_stream_reached_guest"] == true
            && cancelled["stream_closed_after_stop"] == true
            && cancelled["elapsed_ms"]
                .as_u64()
                .is_some_and(|ms| ms < 10000)
            && cancelled["native_firmware_gate"] == false,
        "dashboard cancellation evidence incomplete"
    );
    let dashboard = result
        .dashboard
        .as_ref()
        .context("cancelled dashboard absent")?;
    ensure!(
        dashboard["supervisor_healthy"] == true
            && dashboard["binding"] == cancelled["binding"]
            && dashboard["listener_closed"] == true
            && dashboard["forwarded_bytes"] == 64,
        "cancelled dashboard receipt differs"
    );
    let text = String::from_utf8(hex::decode(result.stdout)?)?;
    let records: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("MD_SIM_DASHBOARD_OPEN "))
        .collect();
    ensure!(
        records.len() == 1
            && records[0]
                == cancelled["challenge_sha256"]
                    .as_str()
                    .context("cancel challenge absent")?,
        "cancelled stream did not match actual guest observation"
    );
    let tls = one("selected_gateway_tls")?;
    let run: md_sim_coordinator::LockedRun = serde_json::from_value(tls["locked_run"].clone())?;
    let process = run
        .process
        .as_ref()
        .context("selected TLS process absent")?;
    let policy = process
        .dashboard
        .as_ref()
        .context("selected TLS policy absent")?;
    let binding: Binding = serde_json::from_value(tls["binding"].clone())?;
    binding.verify(policy, process)?;
    ensure!(
        binding.local_socket.is_some()
            && tls["unique_unix_upstream"] == true
            && tls["publication"]["binding"] == tls["binding"]
            && tls["publication"]["run_id"] == tls["selected_run"]
            && tls["publication"]["locked_run"] == serde_json::to_value(run.object()?)?
            && tls["publication"]["endpoint"]["upstream_unix"]
                == serde_json::to_value(&binding.local_socket)?
            && tls["publication"]["endpoint"]["execution_identity"]
                == serde_json::to_value(run.object()?)?
            && tls["no_certificate_rejected"] == true
            && tls["automatic_stop"] == true
            && tls["client_disconnect_stopped_publication"] == true
            && tls["first_publication"]["run_id"] == tls["selected_run"]
            && tls["first_publication"]["binding"] == tls["binding"]
            && tls["wrong_execution_rejected_by_worker"]
                .as_str()
                .is_some_and(|text| text.contains("dashboard process identity differs"))
            && tls["native_firmware_gate"] == false,
        "selected TLS publication proof incomplete"
    );
    let result: ProcessResult = serde_json::from_value(tls["result"].clone())?;
    ensure!(
        result.status == "cancelled" && !result.trace_truncated,
        "selected TLS process did not stop"
    );
    let dashboard = result
        .dashboard
        .as_ref()
        .context("selected TLS runtime receipt absent")?;
    ensure!(
        dashboard["supervisor_healthy"] == true
            && dashboard["listener_closed"] == true
            && dashboard["binding"] == tls["binding"],
        "selected TLS runtime endpoint differs"
    );
    let text = String::from_utf8(hex::decode(result.stdout)?)?;
    let records: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("MD_SIM_SELECTED_HTTP "))
        .collect();
    ensure!(
        records.len() == 1 && serde_json::from_str::<Value>(records[0])? == tls["proof"],
        "selected TLS response differs from actual guest"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn artifact_recovery_requires_actual_lost_reply_saved_offset_and_destination_identity() {
        let worker = json!({"kind":"worker-isolated-process", "build":{"sha256":"a".repeat(64),"size":100},
            "policy":{"protocol":"md-sim-worker/2"},"tools":{}});
        let cases: Vec<_> = ["chunk", "finish"].into_iter().enumerate().map(|(index, operation)| {
            let object = json!({"sha256":if index == 0 {"b".repeat(64)} else {"c".repeat(64)},"size":270336});
            let result = if index == 0 {json!({"offset":262144})} else {json!({"verified":object})};
            let withheld = json!({"operation":operation,"object":object,
                "offset":if index == 0 {json!(0)} else {Value::Null},"response_withheld":true,
                "worker_response":{"protocol":"md-sim-worker/2","ok":true,"result":result}});
            json!({"operation":operation,"object":object,"worker_before":worker,"worker_after":worker,
                "interrupted_error":format!("SSH channel ended before a complete response: MD_SIM_WITHHELD_ARTIFACT_REPLY {withheld}\n"),
                "automatic_mutation_reconnects":0,"withheld_reply":withheld,
                "resume_begin":{"offset":if index == 0 {262144} else {270336}},
                "destination_finish":{"verified":object},"retrieved":object,
                "same_fresh_bytes":true,"production_transfer_resumed":true})
        }).collect();
        let proof = json!({"cases":cases,"firmware_gate":false});
        verify_artifact_recovery(&proof).unwrap();
        for (path, value) in [
            ("/firmware_gate", json!(true)),
            ("/cases/0/automatic_mutation_reconnects", json!(1)),
            ("/cases/0/resume_begin/offset", json!(0)),
            ("/cases/1/resume_begin/offset", json!(262144)),
            ("/cases/0/withheld_reply/response_withheld", json!(false)),
            ("/cases/0/withheld_reply/worker_response/ok", json!(false)),
            (
                "/cases/1/withheld_reply/object/sha256",
                json!("f".repeat(64)),
            ),
            ("/cases/1/worker_after/build/size", json!(101)),
            ("/cases/0/destination_finish/verified/size", json!(0)),
            ("/cases/1/retrieved/sha256", json!("d".repeat(64))),
            ("/cases/0/production_transfer_resumed", json!(false)),
            ("/cases/0/interrupted_error", json!("request not sent")),
        ] {
            let mut changed = proof.clone();
            *changed.pointer_mut(path).unwrap() = value;
            assert!(verify_artifact_recovery(&changed).is_err(), "{path}");
        }
        let mut missing = proof.clone();
        missing["cases"].as_array_mut().unwrap().pop();
        assert!(verify_artifact_recovery(&missing).is_err());
    }

    #[test]
    fn reconnect_evidence_rejects_other_run_completed_logs_and_no_channel_recovery() {
        let status = json!({"request":{"operation":"status","run_id":"selected"},
            "response":{"run_id":"selected","state":{"state":"booting"}}});
        let logs = |stream: &str, bytes: &[u8]| {
            json!({
            "request":{"operation":"logs","run_id":"selected","stream":stream,"offset":0,"count":4096},
            "response":{"run_id":"selected","stream":stream,"offset":0,"hex":hex::encode(bytes),
                "next_offset":bytes.len(),"saved_bytes":bytes.len(),"process_result_complete":false}})
        };
        let proof = json!({"run_id":"selected","reconnects":3,"forced_server_channel_close_after_each_reply":true,
            "reads":[status,logs("stdout",b"coordinator live stdout\n"),logs("stderr",b"coordinator live stderr\n"),status]});
        verify_live_reconnect(&proof, "selected").unwrap();
        for (path, value) in [
            ("/run_id", json!("other")),
            ("/reconnects", json!(0)),
            ("/reads/1/request/run_id", json!("other")),
            ("/reads/2/response/run_id", json!("other")),
            ("/reads/1/response/process_result_complete", json!(true)),
            (
                "/reads/2/response/hex",
                json!(hex::encode(b"different output")),
            ),
            ("/reads/3/response/state/state", json!("completed")),
        ] {
            let mut changed = proof.clone();
            *changed.pointer_mut(path).unwrap() = value;
            assert!(
                verify_live_reconnect(&changed, "selected").is_err(),
                "{path}"
            );
        }
    }
}

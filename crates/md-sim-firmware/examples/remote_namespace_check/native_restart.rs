//! Planned web-service restart observations. Only the gateway's transient502
//! is retryable. Authentication, malformed replies and other statuses fail.
use super::*;
use md_sim_runtime::isolation::save_json;
const DEADLINE_SECONDS: u64 = 60;
const MAX_ATTEMPTS: usize = 8;

fn result(status: u16, body: &[u8], id: u64) -> Result<Option<Value>> {
    if status == 502 {
        return Ok(None);
    }
    ensure!(
        status == 200,
        "unexpected native restart HTTP status {status}"
    );
    let value: Value = serde_json::from_slice(body)?;
    ensure!(
        value["id"] == id && value["error"].is_null(),
        "native restart RPC failed"
    );
    active_pids(&value["result"])?;
    Ok(Some(value["result"].clone()))
}

pub(super) fn wait_ready(client: &mut Http, session: &str, before: &Value) -> Result<Value> {
    let previous = active_pids(before)?;
    let start = Instant::now();
    let mut attempts = Vec::new();
    for id in 1..=MAX_ATTEMPTS as u64 {
        ensure!(
            start.elapsed() < Duration::from_secs(DEADLINE_SECONDS),
            "native restart readiness deadline"
        );
        let request = json!({"id":id,"method":"exec","params":[ACTIVE_UHTTPD_PIDS]}).to_string();
        let (status, body, _) = client.request(
            &format!("/cgi-bin/luci/rpc/sys?auth={session}"),
            Some(&request),
            "application/json",
        )?;
        ensure!(body.len() <= 4096, "native restart reply exceeds bound");
        let parsed = result(status, &body, id);
        attempts.push(json!({
            "id":id,"elapsed_ms":start.elapsed().as_millis(),"status":status,
            "request_body_hex":hex::encode(request),"response_body_hex":hex::encode(body),
            "pids":parsed.as_ref().ok().and_then(Clone::clone),
            "error":parsed.as_ref().err().map(ToString::to_string)
        }));
        let record = json!({"deadline_seconds":DEADLINE_SECONDS,"maximum_attempts":MAX_ATTEMPTS,
            "before":before,"attempts":attempts,"http":client.observations});
        save_json(
            Path::new("/work/native-restart-probe/restart.json"),
            &record,
        )?;
        ensure!(
            start.elapsed() < Duration::from_secs(DEADLINE_SECONDS),
            "native restart readiness deadline"
        );
        if let Some(pids) = parsed? {
            if previous.is_disjoint(&active_pids(&pids)?) {
                return Ok(record);
            }
        }
        thread::sleep(Duration::from_millis(500));
    }
    anyhow::bail!("native restart exhausted bounded readiness attempts")
}

pub(super) fn verify(record: &Value, before: &Value, after: &Value) -> Result<()> {
    ensure!(
        record["deadline_seconds"] == DEADLINE_SECONDS
            && record["maximum_attempts"] == MAX_ATTEMPTS
            && record["before"] == *before,
        "native restart policy or initial PID differs"
    );
    let previous = active_pids(before)?;
    let expected = active_pids(after)?;
    let attempts = record["attempts"]
        .as_array()
        .context("native restart attempts absent")?;
    let http = record["http"]
        .as_array()
        .context("native restart HTTP evidence absent")?;
    ensure!(
        !attempts.is_empty() && attempts.len() <= MAX_ATTEMPTS && http.len() == attempts.len(),
        "native restart attempt count differs"
    );
    let mut elapsed_before = 0;
    for (index, (attempt, response)) in attempts.iter().zip(http).enumerate() {
        let id = index as u64 + 1;
        let elapsed = attempt["elapsed_ms"]
            .as_u64()
            .context("native restart time absent")?;
        ensure!(
            elapsed >= elapsed_before
                && elapsed < DEADLINE_SECONDS * 1000
                && attempt["id"] == id
                && attempt["error"].is_null(),
            "native restart time, ID or outcome differs"
        );
        elapsed_before = elapsed;
        let request = hex::decode(
            attempt["request_body_hex"]
                .as_str()
                .context("restart request absent")?,
        )?;
        ensure!(
            serde_json::from_slice::<Value>(&request)?
                == json!({"id":id,"method":"exec","params":[ACTIVE_UHTTPD_PIDS]}),
            "native restart request differs"
        );
        let body = hex::decode(
            attempt["response_body_hex"]
                .as_str()
                .context("restart response absent")?,
        )?;
        ensure!(
            body.len() <= 4096
                && response["path"] == "/cgi-bin/luci/rpc/sys"
                && response["tls"] == true
                && response["status"] == attempt["status"]
                && response["bytes"] == body.len()
                && response["body_sha256"] == hex::encode(Sha256::digest(&body)),
            "native restart response bytes or endpoint differ"
        );
        let status = u16::try_from(
            attempt["status"]
                .as_u64()
                .context("restart status absent")?,
        )?;
        let pids = result(status, &body, id)?;
        ensure!(
            serde_json::to_value(&pids)? == attempt["pids"],
            "native restart PID summary differs"
        );
        let ready = match pids {
            Some(ref value) => previous.is_disjoint(&active_pids(value)?),
            None => false,
        };
        if index + 1 == attempts.len() {
            ensure!(
                ready && active_pids(pids.as_ref().unwrap())? == expected,
                "native restart has no stable new PID"
            );
        } else {
            ensure!(!ready, "native restart ignored an earlier ready response");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence() -> Value {
        let mut attempts = Vec::new();
        let mut http = Vec::new();
        for (index, (status, result_value)) in
            [(200, Some("31\n")), (502, None), (200, Some("48\n"))]
                .into_iter()
                .enumerate()
        {
            let id = index + 1;
            let body = result_value
                .map(|pids| json!({"id":id,"result":pids,"error":null}).to_string())
                .unwrap_or_else(|| "Bad Gateway".into());
            attempts.push(json!({"id":id,"elapsed_ms":id*1000,"status":status,
                "request_body_hex":hex::encode(json!({"id":id,"method":"exec","params":[ACTIVE_UHTTPD_PIDS]}).to_string()),
                "response_body_hex":hex::encode(&body),"pids":result_value,"error":null}));
            http.push(
                json!({"path":"/cgi-bin/luci/rpc/sys","tls":true,"status":status,
                "bytes":body.len(),"body_sha256":hex::encode(Sha256::digest(body.as_bytes()))}),
            );
        }
        json!({"deadline_seconds":DEADLINE_SECONDS,"maximum_attempts":MAX_ATTEMPTS,
            "before":"31\n","attempts":attempts,"http":http})
    }

    #[test]
    fn restart_requires_changed_live_pid_and_retains_bounded_transient_failure() -> Result<()> {
        let observed = evidence();
        verify(&observed, &json!("31\n"), &json!("48\n"))?;
        for (pointer, value) in [
            ("/deadline_seconds", json!(120)),
            ("/attempts/2/elapsed_ms", json!(60000)),
            ("/attempts/1/elapsed_ms", json!(0)),
            ("/attempts/2/pids", json!("31\n")),
            ("/http/1/status", json!(200)),
            ("/http/2/body_sha256", json!("a".repeat(64))),
            ("/http/2/path", json!("/another-service")),
        ] {
            let mut changed = observed.clone();
            *changed
                .pointer_mut(pointer)
                .context("missing negative case")? = value;
            assert!(
                verify(&changed, &json!("31\n"), &json!("48\n")).is_err(),
                "{pointer}"
            );
        }
        for status in [401, 403, 404, 500, 503] {
            assert!(result(status, b"failure", 1).is_err());
        }
        assert!(result(200, br#"{"id":1,"result":null,"error":"denied"}"#, 1).is_err());
        assert!(verify(&observed, &json!("31\n"), &json!("49\n")).is_err());
        Ok(())
    }
}

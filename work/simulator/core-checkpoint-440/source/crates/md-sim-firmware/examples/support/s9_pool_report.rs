//! Select recorded pool sessions without selecting for success.
use anyhow::{ensure, Context, Result};
pub const MARKER: &str = "MD_SIM_PRIVATE_POOL_RESULT ";
pub const MAX_SESSIONS: usize = 128;

/// A complete collection must retain every allocated connection. A later
/// successful connection cannot replace the first authorized V1 session.
pub fn require_completed(
    sessions: &[serde_json::Value],
    collection: &serde_json::Value,
    expected_limit: usize,
) -> Result<()> {
    require_collection(sessions, collection, expected_limit)?;
    let selected = select_session(sessions)?.context("no authorized mining session")?;
    ensure!(
        selected["report"]["completed"] == true
            && selected["report"]["stopped"] == false
            && selected["report"]["timed_out"] == false
            && selected["report"]["budget_exhausted"] == false
            && selected["report"]
                .get("failure")
                .is_some_and(serde_json::Value::is_null),
        "first authorized mining session failed or ended incompletely"
    );
    Ok(())
}

/// Allocation IDs start at zero and have no gaps, even for failed probes.
pub fn require_collection(
    sessions: &[serde_json::Value],
    collection: &serde_json::Value,
    expected_limit: usize,
) -> Result<()> {
    ensure!(
        (16..=MAX_SESSIONS).contains(&expected_limit),
        "invalid expected pool connection limit"
    );
    ensure!(
        collection["incomplete_sessions"] == serde_json::json!([])
            && collection["max_sessions"] == expected_limit
            && collection["sessions_completed"] == sessions.len(),
        "incomplete mining collection"
    );
    ensure!(
        sessions.len() <= expected_limit
            && sessions.iter().all(|s| s["session_id"]
                .as_u64()
                .is_some_and(|id| id < expected_limit as u64)),
        "pool connection limit exceeded"
    );
    let mut ids = sessions
        .iter()
        .map(|s| s["session_id"].as_u64().context("pool session ID absent"))
        .collect::<Result<Vec<_>>>()?;
    ids.sort_unstable();
    ensure!(
        ids.into_iter().eq(0..sessions.len() as u64),
        "pool collection has missing or duplicate allocation IDs"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn collection_cannot_hide_a_missing_probe_or_repeat_a_connection() {
        let collection =
            json!({"sessions_completed":3,"incomplete_sessions":[],"max_sessions":128});
        for ids in [[2, 0, 1], [0, 1, 2]] {
            let sessions = ids.map(|id| json!({"session_id":id}));
            require_collection(&sessions, &collection, 128).unwrap();
        }
        for ids in [[0, 2, 3], [1, 2, 3], [0, 1, 1], [0, 1, 128]] {
            let sessions = ids.map(|id| json!({"session_id":id}));
            assert!(require_collection(&sessions, &collection, 128).is_err());
        }
    }
    #[test]
    fn completed_work_does_not_hide_later_session_failure() {
        let trace = concat!(
            "{\"direction\":\"firmware-to-pool\",\"message\":{\"method\":\"mining.authorize\",\"params\":[\"simulator\",\"x\"]}}\n",
            "{\"direction\":\"pool-to-firmware\",\"message\":{\"method\":\"mining.notify\"}}\n");
        let session = json!({"session_id":0,"trace":trace,"report":{
            "completed":true,"stopped":false,"timed_out":false,"budget_exhausted":false,"failure":null}});
        let collection =
            json!({"sessions_completed":1,"incomplete_sessions":[],"max_sessions":MAX_SESSIONS});
        require_completed(std::slice::from_ref(&session), &collection, MAX_SESSIONS).unwrap();
        assert!(require_completed(std::slice::from_ref(&session), &collection, 16).is_err());
        for field in ["stopped", "timed_out", "budget_exhausted"] {
            let mut failed = session.clone();
            failed["report"][field] = json!(true);
            assert!(require_completed(&[failed], &collection, MAX_SESSIONS).is_err());
        }
        let mut failed = session.clone();
        failed["report"]["failure"] = json!("partial frame after completed work");
        assert!(require_completed(&[failed], &collection, MAX_SESSIONS).is_err());
        let mut missing = session;
        missing["report"].as_object_mut().unwrap().remove("stopped");
        assert!(require_completed(&[missing], &collection, MAX_SESSIONS).is_err());
    }
}

/// Select the first actual authorized V1 connection, in allocation order.
/// Keep every other session in the enclosing report, including failed probes.
/// Selection never depends on whether the connection passed its suite.
pub fn select_session(sessions: &[serde_json::Value]) -> Result<Option<serde_json::Value>> {
    let mut ordered = Vec::new();
    for session in sessions {
        let id = session["session_id"]
            .as_u64()
            .context("pool session ID absent")?;
        ensure!(id < MAX_SESSIONS as u64, "pool session ID out of range");
        ensure!(
            !ordered.iter().any(|(previous, _)| *previous == id),
            "duplicate pool session ID"
        );
        ordered.push((id, session));
    }
    ordered.sort_by_key(|(id, _)| *id);
    for (_, session) in ordered {
        let trace = session["trace"]
            .as_str()
            .context("pool session trace absent")?;
        let mut authorized = false;
        let mut notified = false;
        for line in trace.lines() {
            let entry: serde_json::Value = serde_json::from_str(line)?;
            let message = &entry["message"];
            authorized |= entry["direction"] == "firmware-to-pool"
                && message["method"] == "mining.authorize"
                && message["params"][0] == "simulator"
                && message["params"][1] == "x";
            notified |=
                entry["direction"] == "pool-to-firmware" && message["method"] == "mining.notify";
        }
        if authorized && notified {
            return Ok(Some(session.clone()));
        }
    }
    Ok(None)
}

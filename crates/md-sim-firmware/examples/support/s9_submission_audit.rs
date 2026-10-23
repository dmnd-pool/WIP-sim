//! Replay pool decisions and link submitted headers to actual ASIC serial output.
//! This is evidence for the declared reference environment, not a board certificate.
use super::{pool_input, pool_report};
use anyhow::{ensure, Context, Result};
use bitcoin_hashes::{sha256, sha256d, Hash, HashEngine};
use md_sim_mining::{stratum::PoolReport, PoolVerifier, Share, Verdict};
use md_sim_runtime::store::hash_file;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::Path,
};

fn selected_session(pool: &Value, requested: Option<u64>, limit: usize) -> Result<&Value> {
    ensure!(
        (16..=pool_report::MAX_SESSIONS).contains(&limit),
        "invalid pool input session limit"
    );
    let id = requested
        .or_else(|| pool["pool"]["session_id"].as_u64())
        .context("selected session ID absent")?;
    ensure!(id < limit as u64, "selected session ID out of range");
    let sessions = pool["pool_sessions"]
        .as_array()
        .context("pool sessions absent")?;
    ensure!(sessions.len() <= limit, "pool session audit bound");
    let mut ids = BTreeSet::new();
    for session in sessions {
        let current = session["session_id"]
            .as_u64()
            .context("session ID absent")?;
        ensure!(
            current < limit as u64 && ids.insert(current),
            "duplicate or invalid session ID"
        );
    }
    sessions
        .iter()
        .find(|s| s["session_id"] == id)
        .context("requested pool session absent")
}

fn word(value: &Value) -> Result<u32> {
    let text = value.as_str().context("missing hexadecimal word")?;
    ensure!(text.len() == 8, "wrong hexadecimal word length");
    Ok(u32::from_str_radix(text, 16)?)
}

/// Replay in wire order. A saved Accepted label is never sufficient.
fn replay(pool: &Value) -> Result<Vec<Value>> {
    let report: PoolReport = serde_json::from_value(pool["report"].clone())?;
    ensure!(
        report.completed
            && report.failure.is_none()
            && !report.budget_exhausted
            && !report.timed_out
            && !report.stopped,
        "pool suite incomplete"
    );
    ensure!(
        report.jobs.len() == 2 && report.submissions.len() <= 1024,
        "pool audit bounds"
    );
    let mut verifier = PoolVerifier::new(2, 1024).map_err(anyhow::Error::msg)?;
    let mut pending = BTreeMap::new();
    let mut request_ids = BTreeSet::new();
    let mut advertised = BTreeSet::new();
    let mut subscribed = false;
    let mut authorized = false;
    let mut mask = 0;
    let mut difficulty = None;
    let mut difficulties = BTreeSet::new();
    let mut verified = Vec::new();
    let mut last_ns = 0;
    let mut response_count = 0;
    for (line_index, line) in pool["trace"]
        .as_str()
        .context("missing pool trace")?
        .lines()
        .enumerate()
    {
        let event: Value = serde_json::from_str(line)?;
        let ns: u64 = event["elapsed_ns"]
            .as_str()
            .context("missing pool clock")?
            .parse()?;
        ensure!(ns >= last_ns, "pool trace time reversed");
        last_ns = ns;
        let message = &event["message"];
        match event["direction"].as_str() {
            Some("firmware-to-pool") => {
                let id = message["id"].as_u64().context("missing request ID")?;
                ensure!(request_ids.insert(id), "request ID reused");
                let method = message["method"]
                    .as_str()
                    .context("missing request method")?;
                let mut expected = None;
                if method == "mining.submit" {
                    ensure!(subscribed && authorized, "submission before authorization");
                    let params = message["params"]
                        .as_array()
                        .context("missing submission fields")?;
                    ensure!(
                        params.len() == if mask == 0 { 5 } else { 6 } && params[0] == "simulator",
                        "submission fields differ"
                    );
                    let job_id = params[1].as_str().context("missing job ID")?;
                    let job = report
                        .jobs
                        .iter()
                        .find(|job| job.id == job_id)
                        .context("submission job absent")?;
                    let rolled = if mask == 0 { 0 } else { word(&params[5])? };
                    ensure!(rolled & !mask == 0, "version outside negotiated mask");
                    let share = Share {
                        job_id: job_id.to_owned(),
                        extra_nonce2: hex::decode(
                            params[2].as_str().context("missing extra nonce")?,
                        )?,
                        time: word(&params[3])?,
                        nonce: word(&params[4])?,
                        version: (job.version & !mask) | rolled,
                    };
                    let saved = report
                        .submissions
                        .get(verified.len())
                        .context("unrecorded wire submission")?;
                    ensure!(
                        serde_json::to_value(&share)? == serde_json::to_value(&saved.share)?,
                        "saved submission differs from wire"
                    );
                    let verdict = verifier.verify(&share).map_err(anyhow::Error::msg)?;
                    ensure!(
                        verdict == saved.verdict,
                        "saved verdict differs from replay"
                    );
                    let header = job.reconstruct(&share).map_err(anyhow::Error::msg)?;
                    let accepted = matches!(verdict, Verdict::Accepted { .. });
                    expected = Some(if accepted {
                        json!({"id":id,"result":true,"error":null})
                    } else {
                        super::pool_work_audit::rejection(
                            report.response_policy,
                            json!(id),
                            &format!("{verdict:?}"),
                        )?
                    });
                    verified.push(json!({"submission_index":verified.len(),"wire_line":line_index+1,
                        "pool_elapsed_ns":ns,"share":share,"verdict":verdict,
                        "header_hex":hex::encode(header),"digest_hex":hex::encode(sha256d::Hash::hash(&header).to_byte_array())}));
                }
                pending.insert(id, (method.to_owned(), message["params"].clone(), expected));
            }
            Some("pool-to-firmware") => {
                if let Some(id) = message["id"].as_u64() {
                    let (method, params, expected) =
                        pending.remove(&id).context("response has no request")?;
                    if let Some(expected) = expected {
                        ensure!(
                            *message == expected,
                            "submission response differs from independent verdict"
                        );
                        response_count += 1;
                    } else {
                        ensure!(message["error"].is_null(), "negotiation request rejected");
                        match method.as_str() {
                            "mining.configure" => {
                                ensure!(
                                    !subscribed
                                        && !authorized
                                        && message["result"]["version-rolling"] == true,
                                    "late or unsupported negotiation"
                                );
                                mask = word(&message["result"]["version-rolling.mask"])?;
                                ensure!(
                                    mask & !word(&params[1]["version-rolling.mask"])? == 0,
                                    "pool mask exceeds request"
                                );
                            }
                            "mining.subscribe" => {
                                ensure!(!subscribed, "repeated subscription");
                                for job in &report.jobs {
                                    ensure!(
                                        message["result"][1] == hex::encode(&job.extra_nonce1)
                                            && message["result"][2] == job.extra_nonce2_size,
                                        "subscription nonce differs"
                                    );
                                }
                                subscribed = true;
                            }
                            "mining.authorize" => {
                                ensure!(
                                    params == json!(["simulator", "x"])
                                        && message["result"] == true,
                                    "authorization differs"
                                );
                                authorized = true;
                            }
                            "mining.extranonce.subscribe" | "mining.suggest_difficulty" => {
                                ensure!(message["result"] == true, "extension failed")
                            }
                            _ => anyhow::bail!("unhandled pool request {method}"),
                        }
                    }
                } else {
                    ensure!(
                        message["id"].is_null() && subscribed && authorized,
                        "notification before negotiation"
                    );
                    match message["method"].as_str() {
                        Some("mining.set_difficulty") => {
                            let params =
                                message["params"].as_array().context("missing difficulty")?;
                            ensure!(params.len() == 1, "difficulty field count");
                            let value = params[0].as_f64().context("difficulty not numeric")?;
                            let exponent = (-31..=0)
                                .find(|n| 2f64.powi(*n) == value)
                                .context("unsupported pool difficulty")?;
                            difficulty = Some(exponent);
                            difficulties.insert(exponent);
                        }
                        Some("mining.notify") => {
                            let id = message["params"][0].as_str().context("notify ID absent")?;
                            let job = report
                                .jobs
                                .get(advertised.len())
                                .context("too many notifications")?;
                            ensure!(
                                job.id == id && advertised.insert(id.to_owned()),
                                "notification order or ID differs"
                            );
                            super::pool_work_audit::check_notification(job, &message["params"])?;
                            ensure!(
                                job.version_mask == mask && message["params"][8] == true,
                                "clean transition or version mask differs"
                            );
                            let target = md_sim_mining::stratum::difficulty_target(
                                difficulty.take().context("notify without new difficulty")?,
                            )
                            .map_err(anyhow::Error::msg)?;
                            ensure!(
                                job.share_target == target,
                                "job target differs from advertised difficulty"
                            );
                            verifier
                                .install(job.clone(), true)
                                .map_err(anyhow::Error::msg)?;
                        }
                        _ => anyhow::bail!("unhandled pool notification"),
                    }
                }
            }
            _ => anyhow::bail!("invalid or partial pool frame in completed suite"),
        }
    }
    let accepted_jobs: BTreeSet<_> = verified
        .iter()
        .filter(|entry| entry["verdict"].get("Accepted").is_some())
        .map(|entry| entry["share"]["job_id"].as_str().unwrap())
        .collect();
    ensure!(
        pending.is_empty()
            && verified.len() == report.submissions.len()
            && response_count == verified.len()
            && accepted_jobs.len() == 2
            && advertised.len() == 2
            && difficulties.len() == 2,
        "incomplete fresh-work evidence"
    );
    Ok(verified)
}

/// Independent polynomial long division over all 56 transmitted bits.
/// x^5+x^2+1, initial remainder 31, no final XOR. Valid replies have remainder 0.
pub(super) fn valid_reply(frame: &[u8]) -> bool {
    if frame.len() != 7 || frame[6] & 0x60 != 0 {
        return false;
    }
    let mut bits: Vec<_> = frame
        .iter()
        .flat_map(|byte| (0..8).rev().map(move |bit| byte >> bit & 1))
        .collect();
    for bit in &mut bits[..5] {
        *bit ^= 1;
    }
    for index in 0..51 {
        if bits[index] != 0 {
            for offset in [0, 3, 5] {
                bits[index + offset] ^= 1;
            }
        }
    }
    bits[51..].iter().all(|bit| *bit == 0)
}

#[derive(Default)]
struct Serial {
    bytes: Vec<u8>,
    start: usize,
    candidates: VecDeque<usize>,
}

fn responses(log: &str, audit: &Value) -> Result<Vec<Value>> {
    let hashes = audit["hashes"].as_array().context("hash audit absent")?;
    let jobs = audit["jobs"].as_array().context("work audit absent")?;
    let by_line: BTreeMap<_, _> = hashes
        .iter()
        .enumerate()
        .filter(|(_, hash)| hash["asic_filter_match"] == true)
        .map(|(index, hash)| (hash["line"].as_u64().unwrap() as usize, index))
        .collect();
    let mut serial: BTreeMap<u32, Serial> = BTreeMap::new();
    let mut replies = Vec::new();
    for (line_index, line) in log.lines().enumerate() {
        let Some(body) = line
            .strip_prefix("md_sim_bm1387_state ")
            .or_else(|| line.strip_prefix("md_sim_bm1387_event "))
        else {
            continue;
        };
        let fields: BTreeMap<_, _> = body
            .split_whitespace()
            .filter_map(|field| field.split_once('='))
            .collect();
        let link: u32 = fields.get("link").context("missing chain")?.parse()?;
        let kind: u32 = fields.get("kind").context("missing kind")?.parse()?;
        let value = super::hex_field(&fields, "value")?;
        let chain = serial.entry(link).or_default();
        if matches!(kind, 8 | 9) && value == 0 {
            *chain = Serial::default();
        }
        if let Some(index) = by_line.get(&(line_index + 1)) {
            chain.candidates.push_back(*index);
        }
        if kind != 2 {
            continue;
        }
        if chain.bytes.is_empty() {
            chain.start = line_index + 1;
        }
        chain.bytes.push(u8::try_from(value)?);
        if chain.bytes.len() != 7 {
            continue;
        }
        ensure!(
            valid_reply(&chain.bytes),
            "invalid transmitted reply on chain {link} line {}",
            line_index + 1
        );
        if chain.bytes[6] & 0x80 != 0 {
            let index = chain
                .candidates
                .pop_front()
                .context("serial result has no preceding computed candidate")?;
            let hash = &hashes[index];
            let job = &jobs[hash["job_index"].as_u64().context("job index absent")? as usize];
            ensure!(
                hash["link"] == link
                    && hash["nonce"] == u32::from_le_bytes(chain.bytes[..4].try_into().unwrap())
                    && u64::from(chain.bytes[5])
                        == job["job_id"].as_u64().context("work ID absent")?
                            | hash["midstate_index"].as_u64().context("midstate absent")?
                    && chain.bytes[4] == 0,
                "transmitted result differs from queued ASIC computation"
            );
            replies.push(json!({"hash_index":index,"link":link,"frame_hex":hex::encode(&chain.bytes),
                "first_byte_line":chain.start,"last_byte_line":line_index+1,
                "last_byte_virtual_ns":fields.get("virtual_ns").context("reply clock absent")?.parse::<u64>()?}));
        }
        chain.bytes.clear();
    }
    // A stopped emulator may retain queued or partially transmitted replies.
    // Only complete transmitted frames can support a submission below.
    Ok(replies)
}

pub fn analyze(run: &Path) -> Result<Value> {
    analyze_session(run, None, PoolSource::Runner)
}

pub fn analyze_shared(run: &Path) -> Result<Value> {
    analyze_session(run, None, PoolSource::External)
}

enum PoolSource {
    Runner,
    External,
    HistoricalSplit,
}

pub fn historical_split(run: &Path) -> Result<Value> {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/s9-pool-354-session0.json"
    ))?;
    ensure!(
        serde_json::to_value(hash_file(&run.join("result.json"))?)? == fixture["native"]["result"],
        "historical split regression requires exact retained native354 result bytes"
    );
    let mut audit = analyze_session(run, None, PoolSource::HistoricalSplit)?;
    audit["kind"] = json!("historical-pool-and-hardware-input-split-regression/1");
    audit["scope"] = json!("Regression using unchanged archived native354 execution and actual pool/ASIC traces. Pool evidence is parsed separately. This is not execution of the new shared-pool path, fresh work at regression time, a remote gate or an exact-board pass.");
    Ok(audit)
}

pub(super) fn completed_logs(run: &Path) -> Result<(String, String)> {
    // ProcessResult stores hexadecimal traces. Match the runtime hard bound
    // and allow bounded metadata, as in the independent hash audit.
    let execution: Value = serde_json::from_str(&super::read_bounded(
        &run.join("result.json"),
        super::RESULT_BYTES,
    )?)?;
    ensure!(
        execution["exit_code"] == 0
            && execution["status"] == "exited"
            && execution["trace_truncated"] == false,
        "firmware execution failed or is incomplete"
    );
    let backend_log = super::read(&run.join("backend.log"))?;
    let guest_log = super::read(&run.join("guest.log"))?;
    for (field, log) in [("stderr", &backend_log), ("stdout", &guest_log)] {
        ensure!(
            hex::decode(
                execution[field]
                    .as_str()
                    .context("execution stream absent")?
            )? == log.as_bytes(),
            "retained {field} differs from execution result"
        );
    }
    Ok((backend_log, guest_log))
}

fn analyze_session(run: &Path, requested: Option<u64>, source: PoolSource) -> Result<Value> {
    let audit = super::hash_audit::analyze(run)?;
    ensure!(
        audit["trace_truncated"] == false && audit["execution_status"] == "exited",
        "execution incomplete"
    );
    let (backend_log, guest_log) = completed_logs(run)?;
    let pool_path = run.join("pool-result.json");
    let pool: Value = serde_json::from_str(&super::read(&pool_path)?)?;
    for key in ["execution_result", "process_specification", "adaptation"] {
        ensure!(pool[key] == audit[key], "pool {key} belongs to another run");
    }
    ensure!(
        pool["pool_input"] == serde_json::to_value(hash_file(&run.join("pool-input.json"))?)?,
        "pool input hash differs"
    );
    let spec: Value = serde_json::from_str(&super::read(&run.join("process-spec.json"))?)?;
    let inputs = spec["inputs"].as_array().context("process inputs absent")?;
    let pool_inputs: Vec<_> = inputs
        .iter()
        .filter(|mount| mount["guest_path"] == "/private-pool.json")
        .collect();
    ensure!(
        pool_inputs.len() == 1 && pool_inputs[0]["object"] == pool["pool_input"],
        "pool input differs from executed mount"
    );
    let verified_external = if matches!(source, PoolSource::External) {
        Some(super::external_pool::verify(run, true)?)
    } else {
        None
    };
    let separate_pool_text = if matches!(source, PoolSource::HistoricalSplit) {
        Some(
            guest_log
                .lines()
                .filter(|line| {
                    line.starts_with(pool_report::MARKER)
                        || line.starts_with("MD_SIM_PRIVATE_POOL_COLLECTION ")
                })
                .map(|line| format!("{line}\n"))
                .collect::<String>(),
        )
    } else {
        None
    };
    let pool_text = separate_pool_text.as_deref().unwrap_or(&guest_log);
    let sessions: Vec<Value> = if let Some(verified) = &verified_external {
        let mut retained = verified.clone();
        // The outer process is still live at the initial retention barrier.
        retained["external_pool"]["outer_result"] = Value::Null;
        ensure!(pool == retained, "retained external pool join differs");
        serde_json::from_value(verified["pool_sessions"].clone())?
    } else {
        pool_text
            .lines()
            .filter_map(|line| line.strip_prefix(pool_report::MARKER))
            .map(serde_json::from_str)
            .collect::<Result<_, _>>()?
    };
    ensure!(
        json!(sessions) == pool["pool_sessions"]
            && sessions
                .iter()
                .filter(|session| **session == pool["pool"])
                .count()
                == 1,
        "pool report differs from actual runner output"
    );
    let input = pool_input::read(&run.join("pool-input.json"))?;
    let collections = if let Some(verified) = &verified_external {
        serde_json::from_value::<Vec<Value>>(verified["pool_collection"].clone())?
    } else {
        pool_text
            .lines()
            .filter_map(|line| line.strip_prefix("MD_SIM_PRIVATE_POOL_COLLECTION "))
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?
    };
    ensure!(
        collections.len() == 1 && json!(collections) == pool["pool_collection"],
        "pool collection differs from actual runner output"
    );
    pool_report::require_completed(&sessions, &collections[0], input.session_limit()?)?;
    let selected = selected_session(&pool, requested, input.session_limit()?)?;
    let session_id = selected["session_id"]
        .as_u64()
        .context("selected session ID absent")?;
    let report: PoolReport = serde_json::from_value(selected["report"].clone())?;
    if input.completion_policy()? == md_sim_mining::stratum::CompletionPolicy::KeepOpen {
        ensure!(
            report.connection_policy == md_sim_mining::stratum::CompletionPolicy::KeepOpen
                && report.transport == "private-command-connection-posix-io",
            "persistent command policy differs from frozen input"
        );
    }
    ensure!(
        report.response_policy == input.response_policy()?,
        "pool response policy differs from frozen input"
    );
    ensure!(
        serde_json::to_value(&input.regtest_anchor)? == selected["report"]["regtest_anchor"]
            && selected["seed_hex"]
                == hex::encode(input.session_seed(usize::try_from(session_id)?)?)
            && selected["job_time"] == input.time,
        "pool session anchor, entropy or job time differs from frozen input"
    );
    let mut submissions = replay(selected)?;
    let replies = responses(&backend_log, &audit)?;
    let mut matched_hashes = BTreeSet::new();
    for submission in &mut submissions {
        let header = hex::decode(submission["header_hex"].as_str().unwrap())?;
        let mut engine = sha256::Hash::engine();
        engine.input(&header[..64]);
        let bytes = engine.midstate().to_byte_array();
        let midstate: Vec<_> = bytes
            .chunks_exact(4)
            .map(|word| u32::from_be_bytes(word.try_into().unwrap()))
            .collect();
        let mut matches = Vec::new();
        for reply in &replies {
            let index = reply["hash_index"].as_u64().unwrap() as usize;
            let hash = &audit["hashes"][index];
            let job = &audit["jobs"][hash["job_index"].as_u64().unwrap() as usize];
            if hash["digest_hex"] == submission["digest_hex"]
                && hash["nonce"] == submission["share"]["nonce"]
                && job["tail_hex"] == hex::encode(&header[64..76])
                && job["midstates"][hash["midstate_index"].as_u64().unwrap() as usize]
                    == json!(midstate)
            {
                matched_hashes.insert(index);
                matches.push(json!({"computation":hash,"work_packet":job,"serial_reply":reply}));
            }
        }
        ensure!(
            !matches.is_empty(),
            "submission has no computed and transmitted hardware result"
        );
        submission["hardware_matches"] = json!(matches);
    }
    let mut result = json!({"kind":"independent-original-firmware-submission-correlation/1",
        "execution_result":audit["execution_result"],"process_specification":audit["process_specification"],
        "adaptation":audit["adaptation"],"backend_log":audit["backend_log"],"pool_result":hash_file(&pool_path)?,
        "pool_input":pool["pool_input"],"selected_session_id":session_id,
        "session_scope":"Only the explicitly selected actual pool connection is replayed. Other connections retain separate results and require their own audits.",
        "verifier_source":{"sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_submission_audit.rs")),"size":include_bytes!("s9_submission_audit.rs").len()},
        "hash_audit_source":audit["verifier_source"],
        "monotonic_grant_rate":audit["monotonic_grant_rate"],
        "whole_run_rate_bound":audit["whole_run_rate_bound"],
        "total_computed_hashes":audit["hashes"].as_array().unwrap().len(),
        "verified_serial_results":replies.len(),"matched_computations":matched_hashes.len(),
        "submissions":submissions,"fresh_jobs":2,"clean_job_transition":true,"difficulty_change":true,
        "clock_scope":"ASIC times are emulator virtual nanoseconds. Pool times are elapsed nanoseconds in a separate Linux VM. No cross-clock ordering is inferred. Identical headers can have several real hardware matches; all are retained.",
        "scope":"Submitted headers match actual applied work, independently computed SHA256d, programmed/adapted filters, permitted nonces and complete CRC-checked hardware serial responses. Pool verdicts are replayed in wire order. This does not establish exact physical board identity, BootROM execution, Core acceptance, or the full target suite."});
    if let Some(verified) = verified_external {
        result["external_pool"] = verified["external_pool"].clone();
    }
    if let Some(separated) = separate_pool_text {
        result["historical_split"] = json!({
            "pool_bytes":{"sha256":md_sim_firmware::prepare::hash_bytes(separated.as_bytes()),"size":separated.len()},
            "native_guest_log":hash_file(&run.join("guest.log"))?,
            "actual_asic_log":hash_file(&run.join("backend.log"))?,
            "extraction":"Only exact complete pool result and collection lines, with their newline. Native result and source logs remain unchanged."
        });
    }
    Ok(result)
}

pub fn regtest(run: &Path) -> Result<Value> {
    regtest_session(run, None)
}

pub fn regtest_session(run: &Path, requested: Option<u64>) -> Result<Value> {
    regtest_mode(run, requested, false)
}

pub fn regtest_shared(run: &Path, requested: Option<u64>) -> Result<Value> {
    regtest_mode(run, requested, true)
}

fn regtest_mode(run: &Path, requested: Option<u64>, external: bool) -> Result<Value> {
    let mut audit = analyze_session(
        run,
        requested,
        if external {
            PoolSource::External
        } else {
            PoolSource::Runner
        },
    )?;
    let pool: Value = serde_json::from_str(&super::read(&run.join("pool-result.json"))?)?;
    let input = pool_input::read(&run.join("pool-input.json"))?;
    let report: PoolReport = serde_json::from_value(
        selected_session(&pool, requested, input.session_limit()?)?["report"].clone(),
    )?;
    let candidates = md_sim_mining::regtest::regtest_blocks(&report).map_err(anyhow::Error::msg)?;
    for block in &candidates {
        let submission = &audit["submissions"][block.source_submission_index];
        ensure!(
            submission["header_hex"] == block.header_hex
                && submission["share"] == serde_json::to_value(&block.share)?,
            "block differs from hardware-correlated submission"
        );
    }
    // Persistent pool sessions can produce more than one block candidate
    // from the final job. Correlate every candidate above. Select one per
    // distinct fresh job for the two independent node acceptance runs, and
    // retain all remaining candidates rather than discarding their evidence.
    let expected: BTreeSet<_> = report.jobs.iter().map(|job| job.id.clone()).collect();
    let mut selected = BTreeSet::new();
    let (blocks, additional): (Vec<_>, Vec<_>) = candidates
        .into_iter()
        .partition(|block| selected.insert(block.share.job_id.clone()));
    ensure!(
        blocks.len() == 2 && selected == expected,
        "need a hardware-correlated block candidate from each distinct fresh job"
    );
    audit["additional_block_candidates"] = serde_json::to_value(additional)?;
    audit["regtest_blocks"] = serde_json::to_value(blocks)?;
    audit["node_acceptance"] = json!("not-established; requires independent Core execution");
    Ok(audit)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn session_selection_never_changes_a_failed_or_missing_requested_connection() {
        let failed = json!({"session_id":0,"report":{"completed":false}});
        let passed = json!({"session_id":1,"report":{"completed":true}});
        let mut pool = json!({"pool":failed,"pool_sessions":[failed,passed]});
        assert_eq!(selected_session(&pool, None, 16).unwrap(), &failed);
        assert_eq!(selected_session(&pool, Some(0), 16).unwrap(), &failed);
        assert_eq!(selected_session(&pool, Some(1), 16).unwrap(), &passed);
        assert!(selected_session(&pool, Some(2), 16).is_err());
        assert!(selected_session(&pool, Some(16), 16).is_err());
        pool["pool_sessions"].as_array_mut().unwrap().push(passed);
        assert!(selected_session(&pool, Some(1), 16).is_err());
    }
    fn pool_fixture() -> Value {
        let value: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/s9-pool-028-session0.json"
        ))
        .unwrap();
        value["pool"].clone()
    }

    #[test]
    fn high_session_ids_require_the_new_frozen_budget() {
        let session = json!({"session_id":127,"report":{"completed":true}});
        let pool = json!({"pool":session,"pool_sessions":[session]});
        assert!(selected_session(&pool, None, 16).is_err());
        assert!(selected_session(&pool, None, 127).is_err());
        assert!(selected_session(&pool, Some(128), 128).is_err());
        assert_eq!(selected_session(&pool, None, 128).unwrap(), &session);
    }

    #[test]
    fn replay_checks_real_rejection_and_detects_forged_or_incomplete_evidence() {
        let pool = pool_fixture();
        let submissions = replay(&pool).unwrap();
        assert_eq!(submissions.len(), 3);
        assert_eq!(submissions[1]["verdict"], "LowDifficulty");
        for mode in 0..8 {
            let mut bad = pool.clone();
            let mut events: Vec<Value> = bad["trace"]
                .as_str()
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            match mode {
                0 => bad["report"]["completed"] = json!(false),
                1 => bad["report"]["submissions"][1]["verdict"] = json!("Duplicate"),
                2 => {
                    events
                        .iter_mut()
                        .find(|e| e["message"]["method"] == "mining.submit")
                        .unwrap()["message"]["params"][4] = json!("00000000")
                }
                3 => {
                    events
                        .iter_mut()
                        .find(|e| e["message"]["method"] == "mining.notify")
                        .unwrap()["message"]["params"][8] = json!(false)
                }
                4 => {
                    events
                        .iter_mut()
                        .find(|e| e["message"]["method"] == "mining.set_difficulty")
                        .unwrap()["message"]["params"][0] = json!(1.0)
                }
                5 => {
                    events
                        .iter_mut()
                        .find(|e| e["message"]["error"][0] == 23)
                        .unwrap()["message"]["result"] = json!(true)
                }
                6 => {
                    events.pop();
                }
                7 => events.last_mut().unwrap()["message"]["id"] = json!(900),
                _ => unreachable!(),
            }
            bad["trace"] = json!(events
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n"));
            assert!(
                replay(&bad).is_err(),
                "accepted damaged evidence mode{mode}"
            );
        }
    }

    #[test]
    fn serial_reply_requires_prior_matching_computation_and_surviving_reset_state() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/s9-serial-028-match.json"
        ))
        .unwrap();
        let mut hash = fixture["candidate"]["computation"].clone();
        hash["line"] = json!(1);
        hash["job_index"] = json!(0);
        let audit = json!({"hashes":[hash],"jobs":[fixture["candidate"]["work_packet"]]});
        let link = hash["link"].as_u64().unwrap();
        let frame = hex::decode(
            fixture["candidate"]["serial_reply"]["frame_hex"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        let event = |kind: u32, value: u32| {
            format!("md_sim_bm1387_state link={link} virtual_ns=1 kind={kind} value=0x{value:x} extra=0\n")
        };
        let prefix = event(10, hash["nonce"].as_u64().unwrap() as u32);
        let wire: String = frame
            .iter()
            .map(|byte| event(2, u32::from(*byte)))
            .collect();
        assert_eq!(
            responses(&format!("{prefix}{wire}"), &audit).unwrap().len(),
            1
        );
        assert!(responses(&format!("{prefix}{}{wire}", event(9, 0)), &audit).is_err());
        assert!(responses(&wire, &json!({"hashes":[],"jobs":[]})).is_err());
        let mut bad = audit.clone();
        bad["hashes"][0]["nonce"] = json!(999);
        assert!(responses(&format!("{prefix}{wire}"), &bad).is_err());
        bad = audit.clone();
        bad["hashes"][0]["asic_filter_match"] = json!(false);
        assert!(responses(&format!("{prefix}{wire}"), &bad).is_err());
        let partial: String = frame[..6]
            .iter()
            .map(|byte| event(2, u32::from(*byte)))
            .collect();
        assert!(responses(&format!("{prefix}{partial}"), &audit)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn crc_long_division_rejects_each_single_bit_error() {
        // Literal register reply from the retained BM1387 HDL contract.
        let frame = hex::decode("13879000000007").unwrap();
        assert!(valid_reply(&frame));
        for bit in 0..56 {
            let mut bad = frame.clone();
            bad[bit / 8] ^= 1 << (bit % 8);
            assert!(!valid_reply(&bad));
        }
        assert!(!valid_reply(&frame[..6]));
    }
}

//! Independent SHA implementation for retained ASIC work traces.
//! A midstate audit cannot establish a complete Bitcoin header or pool pass.
use anyhow::{ensure, Context, Result};
use bitcoin_hashes::{sha256, Hash, HashEngine};
use md_sim_firmware::bm1387::{Command, Decoder, Job, Packet};
use serde_json::json;
use std::{collections::BTreeMap, io::Read, path::Path};

struct Chain {
    decoder: Decoder,
    pending: Option<Packet>,
    addresses: [u8; 63],
    tickets: [Option<u32>; 63],
    next_address: usize,
    active: Option<(Job, usize)>,
    last_ns: u64,
    compact_boundary: Option<(u64, u32)>,
}

impl Default for Chain {
    fn default() -> Self {
        Self {
            decoder: Decoder::default(),
            pending: None,
            addresses: [0; 63],
            tickets: [None; 63],
            next_address: 0,
            active: None,
            last_ns: 0,
            compact_boundary: None,
        }
    }
}

fn digest(job: &Job, index: usize, nonce: u32) -> Result<[u8; 32]> {
    let state = job.states.get(index).context("invalid midstate index")?;
    let mut midstate = [0; 32];
    for (word, bytes) in state.iter().zip(midstate.chunks_exact_mut(4)) {
        bytes.copy_from_slice(&word.to_be_bytes());
    }
    let mut engine =
        sha256::HashEngine::from_midstate(sha256::Midstate::from_byte_array(midstate), 64);
    engine.input(&job.tail);
    engine.input(&nonce.to_le_bytes());
    let first = sha256::Hash::from_engine(engine);
    Ok(sha256::Hash::hash(&first.to_byte_array()).to_byte_array())
}

fn executed_cap(process: &serde_json::Value, declared: &serde_json::Value) -> Result<f64> {
    let cap = declared.as_f64().context("aggregate hash cap absent")?;
    ensure!(
        cap.is_finite() && cap > 0.0 && cap <= 1e9,
        "invalid aggregate hash cap"
    );
    let argv = process["argv"]
        .as_array()
        .context("missing process arguments")?;
    let machines: Vec<_> = argv
        .windows(2)
        .filter(|pair| matches!(pair[0].as_str(), Some("-machine" | "-M")))
        .map(|pair| pair[1].as_str().context("machine argument is not text"))
        .collect::<Result<_>>()?;
    ensure!(
        machines.len() == 1,
        "aggregate cap requires one selected machine"
    );
    let mut options = machines[0].split(',');
    ensure!(
        options.next() == Some("md-sim-s9-bm1387-development"),
        "aggregate cap belongs to a different machine"
    );
    let caps: Vec<_> = options
        .filter_map(|s| s.strip_prefix("hash-cap-hps="))
        .collect();
    ensure!(caps.len() <= 1, "duplicate aggregate cap property");
    let executed = match caps.first() {
        Some(text) => text.parse::<f64>().context("invalid executed hash cap")?,
        None => 10000.0,
    };
    ensure!(
        executed == cap,
        "declared cap differs from executed machine property"
    );
    Ok(cap)
}

fn whole_run_rate(cap: f64, wall_ms: u64, hashes: usize) -> Result<serde_json::Value> {
    // ProcessResult floors elapsed time to milliseconds. Use its upper
    // millisecond boundary. This includes startup and idle time; it cannot
    // prove the minimum spacing between individual computations.
    let upper_ms = wall_ms.checked_add(1).context("wall duration overflow")?;
    ensure!(
        hashes as f64 <= cap * (upper_ms as f64 / 1000.0),
        "computed hash count exceeds the aggregate cap over the whole run"
    );
    Ok(json!({"computed_hashes":hashes,"wall_milliseconds":wall_ms,
        "elapsed_upper_bound_milliseconds":upper_ms,"cap_hps":cap,
        "scope":"Aggregate whole-run upper bound only. Includes boot and idle time; does not prove per-hash spacing or a physical hashrate."}))
}

fn grant_rate(cap: f64, wall_ms: u64, hashes: &[serde_json::Value]) -> Result<serde_json::Value> {
    let present = hashes
        .iter()
        .filter(|h| !h["grant"].is_null() || !h["wall_ns"].is_null())
        .count();
    if present == 0 {
        return Ok(
            json!({"available":false,"scope":"Legacy trace has no monotonic grant timestamps. Per-hash spacing remains unverified."}),
        );
    }
    ensure!(
        present == hashes.len(),
        "hash trace mixes timed and untimed grants"
    );
    let interval = (1_000_000_000.0 / cap).ceil();
    ensure!(
        interval.is_finite() && interval >= 1.0 && interval < u64::MAX as f64,
        "grant interval exceeds clock range"
    );
    let interval = interval as u64;
    let wall_bound =
        u128::from(wall_ms.checked_add(1).context("wall duration overflow")?) * 1_000_000;
    let mut ordered = BTreeMap::new();
    for hash in hashes {
        let ordinal = hash["grant"]
            .as_u64()
            .context("hash grant ordinal absent")?;
        let ns = hash["wall_ns"].as_u64().context("hash grant time absent")?;
        ensure!(
            ordered.insert(ordinal, ns).is_none(),
            "duplicate aggregate hash grant"
        );
        ensure!(
            u128::from(ns) <= wall_bound,
            "hash grant exceeds process wall duration"
        );
    }
    let mut previous = 0;
    let mut minimum = u64::MAX;
    for (index, (&ordinal, &ns)) in ordered.iter().enumerate() {
        ensure!(
            ordinal == index as u64 + 1,
            "missing or zero aggregate hash grant"
        );
        let spacing = ns
            .checked_sub(previous)
            .context("hash grant clock reversed")?;
        ensure!(
            spacing >= interval,
            "hash grant spacing violates the aggregate cap"
        );
        minimum = minimum.min(spacing);
        previous = ns;
    }
    Ok(
        json!({"available":true,"computed_hashes":hashes.len(),"cap_hps":cap,
        "required_interval_ns":interval,"minimum_observed_interval_ns":minimum,
        "last_grant_elapsed_ns":previous,"contiguous_grants":true,
        "scope":"Actual monotonic admission timestamps, shared across every chain and reset. Each retained computation has one unique grant. Virtual bus time is separate. This checks the software limit, not physical hashrate calibration."}),
    )
}

pub fn analyze(run: &Path) -> Result<serde_json::Value> {
    let path = run.join("backend.log");
    let log = super::read(&path)?;
    let result_path = run.join("result.json");
    let result_identity = md_sim_runtime::store::hash_file(&result_path)?;
    ensure!(
        result_identity.size <= super::RESULT_BYTES,
        "result exceeds audit bound"
    );
    let execution: serde_json::Value =
        serde_json::from_reader(std::fs::File::open(&result_path)?.take(super::RESULT_BYTES + 1))?;
    let truncated = execution["trace_truncated"]
        .as_bool()
        .context("execution lacks trace completeness")?;
    let adaptation: serde_json::Value =
        serde_json::from_str(&super::read(&run.join("adaptation.json"))?)?;
    let process: serde_json::Value =
        serde_json::from_str(&super::read(&run.join("process-spec.json"))?)?;
    let cap = executed_cap(&process, &adaptation["legacy_io"]["hash_cap_hps_total"])?;
    let fixed_bits = match &adaptation["legacy_io"]["fixed_filter_zero_bits"] {
        serde_json::Value::Null => 32,
        value => value
            .as_u64()
            .context("fixed filter bit count is not an integer")?,
    };
    ensure!((1..=32).contains(&fixed_bits), "invalid fixed ASIC filter");
    ensure!(
        adaptation["legacy_io"]["filter_override"]
            .as_bool()
            .unwrap_or(false)
            == (fixed_bits < 32),
        "filter override declaration differs"
    );
    if fixed_bits < 32 {
        let property = format!("md-sim-s9io.fixed-filter-zero-bits={fixed_bits}");
        let properties: Vec<_> = process["argv"]
            .as_array()
            .context("missing process arguments")?
            .iter()
            .filter_map(|arg| arg.as_str())
            .filter(|arg| arg.starts_with("md-sim-s9io.fixed-filter-zero-bits="))
            .collect();
        ensure!(
            properties == [property.as_str()],
            "adapted filter differs from executed property"
        );
    }
    let raw = log
        .lines()
        .any(|line| line.starts_with("md_sim_bm1387_event "));
    let compact = log.lines().any(|line| {
        line.starts_with("md_sim_bm1387_packet ") || line.starts_with("md_sim_bm1387_state ")
    });
    ensure!(raw != compact, "require one nonempty ASIC trace format");
    let mut chains: BTreeMap<u32, Chain> = BTreeMap::new();
    let mut jobs = Vec::new();
    let mut hashes = Vec::new();
    let mut counts: BTreeMap<u32, usize> = BTreeMap::new();
    for (line_index, line) in log.lines().enumerate() {
        let is_packet = line.starts_with("md_sim_bm1387_packet ");
        let Some(body) = line
            .strip_prefix("md_sim_bm1387_event ")
            .or_else(|| line.strip_prefix("md_sim_bm1387_state "))
            .or_else(|| line.strip_prefix("md_sim_bm1387_packet "))
        else {
            continue;
        };
        let fields: BTreeMap<_, _> = body
            .split_whitespace()
            .filter_map(|s| s.split_once('='))
            .collect();
        let link: u32 = fields.get("link").context("missing link")?.parse()?;
        ensure!((6..=8).contains(&link), "unsupported reference chain");
        let kind: u32 = fields.get("kind").context("missing kind")?.parse()?;
        let ns: u64 = fields.get("virtual_ns").context("missing time")?.parse()?;
        let chain = chains.entry(link).or_default();
        ensure!(ns >= chain.last_ns, "chain trace time reversed");
        if is_packet {
            ensure!(
                chain.compact_boundary.is_none() && chain.pending.is_none(),
                "packet before prior completion boundary"
            );
            ensure!(
                (4..=9).contains(&kind) || kind == 12,
                "invalid packet boundary kind"
            );
            let text = fields.get("frame_hex").context("missing packet bytes")?;
            ensure!(
                !text.is_empty() && text.len() <= 300 && text.len() % 2 == 0,
                "invalid compact packet length"
            );
            let bytes = hex::decode(text)?;
            if kind == 4 {
                for (index, byte) in bytes.iter().enumerate() {
                    if let Some(packet) = chain.decoder.push(*byte) {
                        ensure!(index + 1 == bytes.len(), "bytes after completed packet");
                        chain.pending =
                            Some(packet.map_err(|e| anyhow::anyhow!("wire packet: {e:?}"))?);
                    }
                }
                ensure!(chain.pending.is_some(), "incomplete applied packet");
            }
            chain.compact_boundary = Some((ns, kind));
            chain.last_ns = ns;
            continue;
        }
        let value = super::hex_field(&fields, "value")?;
        let extra: u32 = fields.get("extra").context("missing extra")?.parse()?;
        ensure!(
            !compact || !matches!(kind, 1 | 3),
            "raw event in compact state"
        );
        if let Some((packet_ns, packet_kind)) = chain.compact_boundary.take() {
            ensure!(
                packet_ns == ns && packet_kind == kind && (!matches!(kind, 8 | 9) || value == 0),
                "packet completion boundary differs"
            );
        }
        match kind {
            8 | 9 if value == 0 => *chain = Chain::default(),
            6 | 7 | 12 => {
                chain.decoder.reset();
                chain.pending = None;
            }
            1 => {
                if let Some(packet) = chain.decoder.push(u8::try_from(value)?) {
                    chain.pending =
                        Some(packet.map_err(|e| anyhow::anyhow!("wire packet: {e:?}"))?);
                }
            }
            4 => {
                let packet = chain
                    .pending
                    .take()
                    .context("command has no complete RX packet")?;
                match packet {
                    Packet::Job(job) => {
                        ensure!(value == 0x21, "work opcode differs");
                        let position = jobs.len();
                        jobs.push(json!({"link":link,"ns":ns,"line":line_index+1,
                            "job_id":job.id,"midstates":job.states,"tail_hex":hex::encode(job.tail)}));
                        chain.active = Some((job, position));
                    }
                    Packet::Command { broadcast, command } => match command {
                        Command::Inactive { .. } => {
                            chain.next_address = 0;
                            chain.active = None;
                        }
                        Command::SetAddress { address, .. } => {
                            if let Some(slot) = chain.addresses.get_mut(chain.next_address) {
                                *slot = address;
                                chain.next_address += 1;
                            }
                        }
                        Command::Write {
                            address,
                            register: 0x18,
                            value,
                        } => {
                            for (index, &chip) in chain.addresses.iter().enumerate() {
                                if broadcast || chip == address {
                                    chain.tickets[index] = Some(value);
                                }
                            }
                        }
                        _ => {}
                    },
                }
            }
            10 => {
                ensure!(extra & !0x1037f == 0, "unknown hash trace bits");
                let (job, job_index) = chain.active.as_ref().context("hash without active work")?;
                ensure!(
                    u32::from(job.id) == extra & 127,
                    "hash belongs to replaced or unknown job"
                );
                let chip_address = (value & 0xfc) as u8;
                let owners: Vec<_> = chain
                    .addresses
                    .iter()
                    .enumerate()
                    .filter(|(_, address)| **address == chip_address)
                    .map(|(index, _)| index)
                    .collect();
                ensure!(owners.len() == 1, "nonce has ambiguous or absent chip");
                let core = (value >> 24) & 127;
                ensure!(
                    matches!(core,0..=56|64..=120),
                    "nonce outside reference core map"
                );
                let ticket = chain.tickets[owners[0]].context("nonce before filter programming")?;
                let midstate = (extra >> 8 & 3) as usize;
                let hash = digest(job, midstate, value)?;
                let accepted =
                    u32::from_le_bytes(hash[28..].try_into().unwrap()) >> (32 - fixed_bits) == 0
                        && u32::from_be_bytes(hash[24..28].try_into().unwrap()) & ticket == 0;
                ensure!(
                    accepted == (extra & 0x10000 != 0),
                    "reported ASIC filter result differs"
                );
                hashes.push(json!({"link":link,"ns":ns,"line":line_index+1,
                    "job_index":job_index,"nonce":value,"midstate_index":midstate,
                    "chip_position":owners[0],"core_address":core,"ticket":ticket,
                    "grant":fields.get("grant").map(|v| v.parse::<u64>()).transpose()?,
                    "wall_ns":fields.get("wall_ns").map(|v| v.parse::<u64>()).transpose()?,
                    "digest_hex":hex::encode(hash),"asic_filter_match":accepted}));
                *counts.entry(link).or_default() += 1;
            }
            _ => {}
        }
        chain.last_ns = ns;
    }
    ensure!(
        !hashes.is_empty(),
        "no computed nonce traces; no audit pass"
    );
    ensure!(
        truncated
            || chains
                .values()
                .all(|chain| chain.compact_boundary.is_none()),
        "unmatched packet completion boundary"
    );
    let rate = whole_run_rate(
        cap,
        execution["wall_milliseconds"]
            .as_u64()
            .context("execution wall duration absent")?,
        hashes.len(),
    )?;
    let grants = grant_rate(
        cap,
        execution["wall_milliseconds"]
            .as_u64()
            .context("execution wall duration absent")?,
        &hashes,
    )?;
    Ok(json!({
        "kind":"independent-ASIC-midstate-audit",
        "backend_log":md_sim_runtime::store::hash_file(&path)?,
        "execution_result":result_identity,
        "execution_status":execution["status"],
        "trace_truncated":truncated,
        "fixed_filter_zero_bits":fixed_bits,
        "hash_cap_hps_total":cap,"whole_run_rate_bound":rate,
        "monotonic_grant_rate":grants,
        "trace_format":if compact {"md-sim-bm1387-packets/1"} else {"md-sim-bm1387-events/1"},
        "trace_scope":if truncated {"Retained prefix only; events after the limit are unavailable."} else {"Retained log; no truncation reported by the runner."},
        "adaptation":md_sim_runtime::store::hash_file(&run.join("adaptation.json"))?,
        "process_specification":md_sim_runtime::store::hash_file(&run.join("process-spec.json"))?,
        "verifier_source":{
            "sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_hash_audit.rs")),
            "size":include_bytes!("s9_hash_audit.rs").len()
        },
        "oracle":"bitcoin_hashes0.14.1 SHA256 HashEngine, independent of model sha2 compression",
        "jobs":jobs,"hashes":hashes,"counts_by_link":counts,
        "scope":"Every retained hash trace is checked against the active firmware work, chip address and programmed filter. This audit has no complete Bitcoin prefix or pool job; it grants no share, block, boot or mining acceptance."
    }))
}

/// A separate required check. Legacy or truncated traces cannot establish
/// the aggregate admission limit even if their retained hashes are valid.
pub fn analyze_rate(run: &Path) -> Result<serde_json::Value> {
    let audit = analyze(run)?;
    ensure!(
        audit["trace_truncated"] == false,
        "rate check requires a complete trace"
    );
    ensure!(
        audit["monotonic_grant_rate"]["available"] == true,
        "rate check requires actual monotonic grant timestamps"
    );
    Ok(json!({
        "kind":"independent-aggregate-hash-admission-audit",
        "backend_log":audit["backend_log"],"execution_result":audit["execution_result"],
        "execution_status":audit["execution_status"],"trace_truncated":false,
        "adaptation":audit["adaptation"],"process_specification":audit["process_specification"],
        "verifier_source":audit["verifier_source"],
        "counts_by_link":audit["counts_by_link"],
        "whole_run_rate_bound":audit["whole_run_rate_bound"],
        "monotonic_grant_rate":audit["monotonic_grant_rate"],
        "scope":"Every retained computation was independently checked against its firmware work and programmed filter. All shared grants obey the wall-clock cap. This does not establish pool shares, physical calibration or a complete target."
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregate_grant_oracle_rejects_bursts_gaps_duplicates_and_missing_clock_evidence(
    ) -> Result<()> {
        // Delivery order can differ across chain queues. Grant order cannot.
        let valid = vec![
            json!({"grant":2,"wall_ns":200_000_001}),
            json!({"grant":1,"wall_ns":100_000_001}),
        ];
        assert_eq!(
            grant_rate(10.0, 200, &valid)?["minimum_observed_interval_ns"],
            100_000_000
        );
        assert_eq!(grant_rate(10.0, 200, &[json!({})])?["available"], false);
        for bad in [
            vec![json!({"grant":1,"wall_ns":99_999_999})],
            vec![
                json!({"grant":1,"wall_ns":100_000_001}),
                json!({"grant":2,"wall_ns":200_000_000}),
            ],
            vec![
                json!({"grant":1,"wall_ns":200_000_001}),
                json!({"grant":2,"wall_ns":100_000_001}),
            ],
            vec![json!({"grant":0,"wall_ns":100_000_001})],
            vec![json!({"grant":2,"wall_ns":100_000_001})],
            vec![valid[0].clone(), valid[0].clone()],
            vec![valid[1].clone(), json!({})],
            vec![json!({"grant":1})],
            vec![json!({"wall_ns":100_000_001})],
            vec![json!({"grant":1,"wall_ns":201_000_001})],
        ] {
            assert!(grant_rate(10.0, 200, &bad).is_err(), "{bad:?}");
        }
        Ok(())
    }

    #[test]
    fn rate_audit_checks_executed_cap_and_all_chain_computations() -> Result<()> {
        let explicit = json!({"argv":["/runner","-machine",
            "md-sim-s9-bm1387-development,boot-mode=sd,hash-cap-hps=10"]});
        assert_eq!(executed_cap(&explicit, &json!(10))?, 10.0);
        assert!(executed_cap(&explicit, &json!(10000)).is_err());
        assert!(executed_cap(&explicit, &serde_json::Value::Null).is_err());
        for machine in [
            "other,hash-cap-hps=10",
            "md-sim-s9-bm1387-development,hash-cap-hps=10,hash-cap-hps=10",
            "md-sim-s9-bm1387-development,hash-cap-hps=NaN",
        ] {
            assert!(executed_cap(&json!({"argv":["-M",machine]}), &json!(10)).is_err());
        }
        assert_eq!(
            executed_cap(
                &json!({"argv":["-M","md-sim-s9-bm1387-development"]}),
                &json!(10000)
            )?,
            10000.0
        );
        assert!(executed_cap(
            &json!({"argv":["-M","md-sim-s9-bm1387-development","-M","other"]}),
            &json!(10000)
        )
        .is_err());
        whole_run_rate(10.0, 999, 10)?;
        assert!(whole_run_rate(10.0, 999, 11).is_err());
        assert!(whole_run_rate(10.0, u64::MAX, 1).is_err());
        Ok(())
    }
    use bitcoin_hashes::sha256d;
    use md_sim_firmware::{
        bm1397::crc5,
        s9io::{work_frame, Midstates},
    };

    fn append(log: &mut String, frame: &[u8]) {
        for byte in frame {
            log.push_str(&format!(
                "md_sim_bm1387_event link=6 virtual_ns={} kind=1 value=0x{byte:x} extra=0\n",
                log.len()
            ));
        }
        log.push_str(&format!(
            "md_sim_bm1387_event link=6 virtual_ns={} kind=4 value=0x{:x} extra=0\n",
            log.len(),
            frame[0]
        ));
    }

    fn command(log: &mut String, bytes: &[u8]) {
        let mut frame = bytes.to_vec();
        frame.push(crc5(bytes));
        append(log, &frame);
    }

    fn compact(log: &str) -> String {
        let mut bytes = Vec::new();
        let mut output = String::new();
        for line in log.lines() {
            let fields: BTreeMap<_, _> = line
                .split_whitespace()
                .filter_map(|s| s.split_once('='))
                .collect();
            if fields["kind"] == "1" {
                bytes.push(
                    u8::try_from(super::super::hex_field(&fields, "value").unwrap()).unwrap(),
                );
            } else {
                if fields["kind"] == "4" {
                    output.push_str(&format!(
                        "md_sim_bm1387_packet link=6 virtual_ns={} kind=4 frame_hex={}\n",
                        fields["virtual_ns"],
                        hex::encode(&bytes)
                    ));
                    bytes.clear();
                }
                output.push_str(&line.replace("md_sim_bm1387_event ", "md_sim_bm1387_state "));
                output.push('\n');
            }
        }
        assert!(bytes.is_empty());
        output
    }

    #[test]
    fn verifier_checks_fresh_full_headers_and_rejects_a_forged_filter_success() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("adaptation.json"),
            br#"{"legacy_io":{"hash_cap_hps_total":10000}}"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("process-spec.json"),
            br#"{"argv":["-machine","md-sim-s9-bm1387-development"]}"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("result.json"),
            br#"{"status":"test-fixture","trace_truncated":false,"wall_milliseconds":1}"#,
        )
        .unwrap();
        let mut log = String::new();
        command(&mut log, &[0x55, 5, 0, 0]);
        for address in (0..252).step_by(4) {
            command(&mut log, &[0x41, 5, address, 0]);
        }
        command(&mut log, &[0x58, 9, 0, 0x18, 0, 0, 0, 0]);
        let mut expected = Vec::new();
        for id in [0, 4] {
            let mut header = [0; 80];
            std::fs::File::open("/dev/urandom")
                .unwrap()
                .read_exact(&mut header)
                .unwrap();
            header[76..].copy_from_slice(&4u32.to_le_bytes());
            let mut engine = sha256::Hash::engine();
            engine.input(&header[..64]);
            let midstate = engine.midstate().to_byte_array();
            let mut words = vec![id];
            for offset in [72, 68, 64] {
                words.push(u32::from_le_bytes(
                    header[offset..offset + 4].try_into().unwrap(),
                ));
            }
            for word in midstate.chunks_exact(4).rev() {
                words.push(u32::from_be_bytes(word.try_into().unwrap()));
            }
            append(&mut log, &work_frame(&words, Midstates::One).unwrap());
            let digest = sha256d::Hash::hash(&header).to_byte_array();
            let extra = id | u32::from(digest[28..] == [0; 4]) << 16;
            log.push_str(&format!(
                "md_sim_bm1387_event link=6 virtual_ns={} kind=10 value=0x4 extra={extra}\n",
                log.len()
            ));
            expected.push(hex::encode(digest));
        }
        let path = root.path().join("backend.log");
        std::fs::write(&path, &log).unwrap();
        let report = analyze(root.path()).unwrap();
        assert!(analyze_rate(root.path()).is_err());
        assert_eq!(report["hashes"].as_array().unwrap().len(), 2);
        for (hash, expected) in report["hashes"].as_array().unwrap().iter().zip(&expected) {
            assert_eq!(hash["digest_hex"], *expected);
        }
        let compact_log = compact(&log);
        let mut ordinal = 0u64;
        let timed = compact_log
            .lines()
            .map(|line| {
                if line.contains(" kind=10 ") {
                    ordinal += 1;
                    format!("{line} grant={ordinal} wall_ns={}\n", ordinal * 100_000)
                } else {
                    format!("{line}\n")
                }
            })
            .collect::<String>();
        std::fs::write(&path, &timed).unwrap();
        let timing = analyze_rate(root.path()).unwrap();
        assert_eq!(timing["monotonic_grant_rate"]["computed_hashes"], 2);
        std::fs::write(
            root.path().join("result.json"),
            br#"{"status":"test-fixture","trace_truncated":true,"wall_milliseconds":1}"#,
        )
        .unwrap();
        assert!(analyze_rate(root.path()).is_err());
        std::fs::write(
            root.path().join("result.json"),
            br#"{"status":"test-fixture","trace_truncated":false,"wall_milliseconds":1}"#,
        )
        .unwrap();
        std::fs::write(&path, &compact_log).unwrap();
        let compact_report = analyze(root.path()).unwrap();
        assert_eq!(compact_report["trace_format"], "md-sim-bm1387-packets/1");
        assert_eq!(compact_report["counts_by_link"], report["counts_by_link"]);
        assert_eq!(compact_report["jobs"].as_array().unwrap().len(), 2);
        for (hash, expected) in compact_report["hashes"]
            .as_array()
            .unwrap()
            .iter()
            .zip(&expected)
        {
            assert_eq!(hash["digest_hex"], *expected);
        }
        let last_packet = compact_log
            .lines()
            .filter(|line| line.starts_with("md_sim_bm1387_packet "))
            .next_back()
            .unwrap();
        let mut corrupt_packet = last_packet.as_bytes().to_vec();
        let last = corrupt_packet.last_mut().unwrap();
        *last = if *last == b'0' { b'1' } else { b'0' };
        for bad in [
            compact_log.replace(last_packet, &String::from_utf8(corrupt_packet).unwrap()),
            compact_log.replace(&format!("{last_packet}\n"), ""),
            compact_log.replace(last_packet, &format!("{last_packet}00")),
            compact_log.replace(last_packet, &last_packet.replace("kind=4", "kind=5")),
            format!("{compact_log}{log}"),
            format!(
                "{compact_log}md_sim_bm1387_packet link=6 virtual_ns={} kind=8 frame_hex=55\n",
                u64::MAX
            ),
        ] {
            std::fs::write(&path, bad).unwrap();
            assert!(analyze(root.path()).is_err());
        }
        let last = compact_log.lines().last().unwrap();
        let (prefix, extra) = last.rsplit_once("extra=").unwrap();
        let extra: u32 = extra.parse().unwrap();
        std::fs::write(
            &path,
            compact_log.replace(last, &format!("{prefix}extra={}", extra ^ 0x10000)),
        )
        .unwrap();
        assert!(analyze(root.path())
            .unwrap_err()
            .to_string()
            .contains("filter result differs"));
        std::fs::write(&path, &log).unwrap();
        std::fs::write(
            root.path().join("result.json"),
            br#"{"status":"timed-out","trace_truncated":true,"wall_milliseconds":1}"#,
        )
        .unwrap();
        let prefix = analyze(root.path()).unwrap();
        assert_eq!(prefix["trace_truncated"], true);
        assert_eq!(prefix["execution_status"], "timed-out");
        let last = log.lines().last().unwrap();
        let (prefix, extra) = last.rsplit_once("extra=").unwrap();
        let extra: u32 = extra.parse().unwrap();
        let tampered = log.strip_suffix(&format!("{last}\n")).unwrap().to_owned()
            + &format!("{prefix}extra={}\n", extra ^ 0x10000);
        std::fs::write(path, tampered).unwrap();
        assert!(analyze(root.path())
            .unwrap_err()
            .to_string()
            .contains("filter result differs"));
    }
}

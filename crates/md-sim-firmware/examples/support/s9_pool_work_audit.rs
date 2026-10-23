//! Correlate retained ASIC work with actual pool notifications.
//! This diagnostic does not submit work or grant mining acceptance.
use anyhow::{ensure, Context, Result};
use bitcoin_hashes::{sha256, sha256d, Hash, HashEngine};
use md_sim_mining::pool::{PoolJob, Share};
use md_sim_mining::stratum::{PoolReport, ResponsePolicy};
use md_sim_runtime::store::hash_file;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::Path,
};

type State = [u32; 8];
type Prefix = [u8; 76];

pub(super) fn check_notification(job: &PoolJob, params: &Value) -> Result<()> {
    let values = params
        .as_array()
        .context("notify parameters are not an array")?;
    ensure!(values.len() == 9, "unexpected notify parameter count");
    let text = |index: usize| values[index].as_str().context("notify field is not text");
    ensure!(text(0)? == job.id, "notify ID differs");
    let mut previous = hex::decode(text(1)?)?;
    ensure!(previous.len() == 32, "previous hash length differs");
    for word in previous.chunks_exact_mut(4) {
        word.reverse();
    }
    ensure!(previous == job.previous, "notify previous hash differs");
    ensure!(
        hex::decode(text(2)?)? == job.coinbase_prefix
            && hex::decode(text(3)?)? == job.coinbase_suffix,
        "notify coinbase differs"
    );
    let branch: Vec<String> = serde_json::from_value(values[4].clone())?;
    ensure!(
        branch
            .iter()
            .map(hex::decode)
            .collect::<Result<Vec<_>, _>>()?
            == job
                .merkle_branch
                .iter()
                .map(|hash| hash.to_vec())
                .collect::<Vec<_>>(),
        "notify Merkle branch differs"
    );
    for (index, expected) in [(5, job.version), (6, job.bits), (7, job.time_min)] {
        let field = text(index)?;
        ensure!(
            field.len() == 8 && u32::from_str_radix(field, 16)? == expected,
            "notify integer differs"
        );
    }
    ensure!(values[8].is_boolean(), "missing clean-job state");
    Ok(())
}

// A real ASIC result may meet its filter but miss the pool target. Such a
// rejection is protocol evidence, not a failed subscription or negotiation.
// This table is independent of the pool server's response encoder. The pinned
// SRI SubmitError contract supplies the numeric codes; old inputs retain their
// recorded, nonconforming false/code23 response behavior.
pub(super) fn rejection(policy: ResponsePolicy, id: Value, verdict: &str) -> Result<Value> {
    let code = match verdict {
        "Stale" | "UnknownJob" => 21,
        "Duplicate" => 22,
        "LowDifficulty" => 23,
        "InvalidExtraNonce" | "OutOfRange" | "InvalidVersion" => 20,
        _ => anyhow::bail!("unknown pool rejection verdict"),
    };
    Ok(match policy {
        ResponsePolicy::LegacyFalseCode23 => {
            json!({"id":id,"result":false,"error":[23,verdict,null]})
        }
        ResponsePolicy::JsonRpc1StratumV1 => {
            json!({"id":id,"result":null,"error":[code,verdict,null]})
        }
    })
}

fn check_response(policy: ResponsePolicy, method: &str, value: &Value) -> Result<()> {
    ensure!(
        value.get("result").is_some() && value.get("error").is_some(),
        "incomplete JSON-RPC response"
    );
    if value["error"].is_null() {
        return Ok(());
    }
    ensure!(method == "mining.submit", "pool negotiation request failed");
    let expected = rejection(
        policy,
        value["id"].clone(),
        value["error"][1]
            .as_str()
            .context("missing rejection verdict")?,
    )?;
    ensure!(*value == expected, "pool request has an invalid rejection");
    Ok(())
}

fn state(prefix: &Prefix) -> State {
    let mut engine = sha256::Hash::engine();
    engine.input(&prefix[..64]);
    let bytes = engine.midstate().to_byte_array();
    std::array::from_fn(|index| {
        u32::from_be_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap())
    })
}

/// Every candidate is reconstructed from the independent pool contract.
/// The bounded search infers a prefix only when the whole SHA midstate
/// and the remaining12 bytes match. A tail match alone is insufficient.
fn find_prefixes(
    job: &PoolJob,
    wanted: &BTreeSet<(State, [u8; 12])>,
) -> Result<BTreeMap<(State, [u8; 12]), Prefix>> {
    job.validate().map_err(anyhow::Error::msg)?;
    ensure!(
        job.extra_nonce2_size == 4,
        "diagnostic requires four extra-nonce bytes"
    );
    let bits: Vec<_> = (0..32)
        .filter(|bit| job.version_mask & (1 << bit) != 0)
        .collect();
    ensure!(bits.len() <= 16, "version search exceeds diagnostic bound");
    let mut by_merkle: BTreeMap<[u8; 4], BTreeMap<State, Vec<[u8; 12]>>> = BTreeMap::new();
    for (midstate, tail) in wanted {
        if tail[8..] == job.bits.to_le_bytes() {
            by_merkle
                .entry(tail[..4].try_into().unwrap())
                .or_default()
                .entry(*midstate)
                .or_default()
                .push(*tail);
        }
    }
    let mut matches = BTreeMap::new();
    let mut tried = BTreeSet::new();
    for counter in 0..=1024u32 {
        for extra_nonce2 in [counter.to_le_bytes(), counter.to_be_bytes()] {
            if !tried.insert(extra_nonce2) {
                continue;
            }
            let share = Share {
                job_id: job.id.clone(),
                extra_nonce2: extra_nonce2.to_vec(),
                nonce: 0,
                time: job.time_min,
                version: job.version,
            };
            let header = job.reconstruct(&share).map_err(anyhow::Error::msg)?;
            // The last Merkle word identifies candidate coinbases cheaply.
            let key: [u8; 4] = header[64..68].try_into().unwrap();
            let Some(relevant) = by_merkle.get(&key) else {
                continue;
            };
            for ordinal in 0..(1u32 << bits.len()) {
                let version = bits
                    .iter()
                    .enumerate()
                    .fold(job.version & !job.version_mask, |version, (index, bit)| {
                        version | ((ordinal >> index & 1) << bit)
                    });
                let mut prefix: Prefix = header[..76].try_into().unwrap();
                prefix[..4].copy_from_slice(&version.to_le_bytes());
                let midstate = state(&prefix);
                for tail in relevant.get(&midstate).into_iter().flatten() {
                    let time = u32::from_le_bytes(tail[4..8].try_into().unwrap());
                    if !(job.time_min..=job.time_max).contains(&time) {
                        continue;
                    }
                    prefix[68..72].copy_from_slice(&time.to_le_bytes());
                    if let Some(previous) = matches.insert((midstate, *tail), prefix) {
                        ensure!(
                            previous == prefix,
                            "ambiguous independently reconstructed prefix"
                        );
                    }
                }
            }
        }
    }
    Ok(matches)
}

fn computed_work_states(jobs: &[Value], hashes: &[Value]) -> Result<BTreeSet<(State, [u8; 12])>> {
    // The preceding packet/hash audit validates the whole bounded trace.
    // Firmware can replace many jobs between low-rate computations. Search
    // prefixes for every actual computation, without expanding unused jobs.
    let mut wanted = BTreeSet::new();
    for hash in hashes {
        let index = usize::try_from(hash["job_index"].as_u64().context("missing job index")?)?;
        let job = jobs.get(index).context("computed work job is absent")?;
        let midstate = usize::try_from(
            hash["midstate_index"]
                .as_u64()
                .context("missing midstate index")?,
        )?;
        let state: State = serde_json::from_value(
            job["midstates"]
                .get(midstate)
                .context("computed midstate is absent")?
                .clone(),
        )?;
        let tail: [u8; 12] = hex::decode(job["tail_hex"].as_str().context("missing tail")?)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid tail length"))?;
        wanted.insert((state, tail));
        ensure!(
            wanted.len() <= 262144,
            "unique computed states exceed diagnostic bound"
        );
    }
    Ok(wanted)
}

pub fn analyze(run: &Path) -> Result<Value> {
    let audit = super::hash_audit::analyze(run)?;
    let path = run.join("pool-result.json");
    ensure!(
        hash_file(&path)?.size <= 2 * 1024 * 1024,
        "pool result exceeds bound"
    );
    let pool: Value =
        serde_json::from_reader(std::fs::File::open(&path)?.take(2 * 1024 * 1024 + 1))?;
    let input_path = run.join("pool-input.json");
    ensure!(
        pool["pool_input"] == serde_json::to_value(hash_file(&input_path)?)?,
        "pool input identity differs"
    );
    let input = super::pool_input::read(&input_path)?;
    let report: PoolReport = serde_json::from_value(pool["pool"]["report"].clone())?;
    ensure!(
        report.response_policy == input.response_policy()?,
        "pool response policy differs from frozen input"
    );
    ensure!(
        pool["execution_result"] == audit["execution_result"]
            && pool["process_specification"] == audit["process_specification"]
            && pool["adaptation"] == audit["adaptation"],
        "pool evidence belongs to a different execution"
    );
    let mut advertised = BTreeMap::new();
    let mut requests = BTreeMap::new();
    let mut subscription = None;
    let mut negotiated_mask = 0;
    for line in pool["pool"]["trace"]
        .as_str()
        .context("missing pool trace")?
        .lines()
    {
        let message: Value = serde_json::from_str(line)?;
        let value = &message["message"];
        if message["direction"] == "firmware-to-pool" {
            if let (Some(id), Some(method)) = (value["id"].as_u64(), value["method"].as_str()) {
                requests.insert(id, method.to_owned());
            }
        } else if message["direction"] == "pool-to-firmware" {
            if let Some(method) = value["id"].as_u64().and_then(|id| requests.remove(&id)) {
                check_response(report.response_policy, &method, value)?;
                if method == "mining.subscribe" {
                    subscription = Some((
                        hex::decode(
                            value["result"][1]
                                .as_str()
                                .context("missing subscription nonce")?,
                        )?,
                        value["result"][2]
                            .as_u64()
                            .context("missing subscription nonce size")?,
                    ));
                } else if method == "mining.configure" && value["result"]["version-rolling"] == true
                {
                    negotiated_mask = u32::from_str_radix(
                        value["result"]["version-rolling.mask"]
                            .as_str()
                            .context("missing negotiated mask")?,
                        16,
                    )?;
                }
            }
        }
        if message["direction"] == "pool-to-firmware"
            && message["message"]["method"] == "mining.notify"
        {
            let id = message["message"]["params"][0]
                .as_str()
                .context("notify lacks ID")?
                .to_owned();
            ensure!(
                advertised
                    .insert(id, message["message"]["params"].clone())
                    .is_none(),
                "pool notification ID reused"
            );
        }
    }
    let jobs = audit["jobs"].as_array().context("missing audited jobs")?;
    let hashes = audit["hashes"].as_array().context("missing hashes")?;
    let wanted = computed_work_states(jobs, hashes)?;
    let pool_jobs: Vec<PoolJob> = serde_json::from_value(pool["pool"]["report"]["jobs"].clone())?;
    ensure!(
        pool_jobs.len() <= 2,
        "pool job count exceeds diagnostic bound"
    );
    let mut prefixes = BTreeMap::new();
    for job in pool_jobs
        .iter()
        .filter(|job| advertised.contains_key(&job.id))
    {
        check_notification(job, &advertised[&job.id])?;
        let (nonce, size) = subscription
            .as_ref()
            .context("no successful subscription")?;
        ensure!(
            nonce == &job.extra_nonce1
                && *size == job.extra_nonce2_size as u64
                && negotiated_mask == job.version_mask,
            "pool contract differs from negotiation"
        );
        for (midstate, prefix) in find_prefixes(job, &wanted)? {
            ensure!(
                prefixes
                    .insert(midstate, (job.id.clone(), prefix))
                    .is_none(),
                "midstate matches multiple pool jobs"
            );
        }
    }
    let mut verified = Vec::new();
    let mut unmatched = Vec::new();
    let mut matched_jobs = BTreeSet::new();
    let mut intervals = BTreeMap::new();
    let policy = md_sim_firmware::bm1387::filter::Policy::from_fixed_bits(
        audit["fixed_filter_zero_bits"]
            .as_u64()
            .context("missing filter policy")? as u8,
    )
    .map_err(|error| anyhow::anyhow!("invalid filter policy: {error:?}"))?;
    let cap = audit["hash_cap_hps_total"]
        .as_f64()
        .context("missing aggregate hash cap")?;
    for (index, hash) in hashes.iter().enumerate() {
        let job_index = hash["job_index"].as_u64().context("missing job index")? as usize;
        let job = jobs.get(job_index).context("job index out of range")?;
        let midstate_index = hash["midstate_index"]
            .as_u64()
            .context("missing midstate index")? as usize;
        let midstate: State = serde_json::from_value(job["midstates"][midstate_index].clone())?;
        let tail: [u8; 12] = hex::decode(job["tail_hex"].as_str().context("missing tail")?)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid tail length"))?;
        let Some((pool_id, prefix)) = prefixes.get(&(midstate, tail)) else {
            unmatched.push(index);
            continue;
        };
        ensure!(
            hex::encode(&prefix[64..]) == job["tail_hex"],
            "reconstructed prefix tail differs"
        );
        let mut header = [0; 80];
        header[..76].copy_from_slice(prefix);
        let nonce = u32::try_from(hash["nonce"].as_u64().context("missing nonce")?)?;
        header[76..].copy_from_slice(&nonce.to_le_bytes());
        let digest = sha256d::Hash::hash(&header).to_byte_array();
        ensure!(
            hex::encode(digest) == hash["digest_hex"],
            "independent full-header digest differs"
        );
        matched_jobs.insert(job_index);
        let ticket = u32::try_from(hash["ticket"].as_u64().context("missing ticket")?)?;
        let key = (pool_id.clone(), ticket);
        if let std::collections::btree_map::Entry::Vacant(entry) = intervals.entry(key) {
            let target = pool_jobs
                .iter()
                .find(|job| job.id == *pool_id)
                .context("matched pool job absent")?
                .share_target;
            let filter = policy.programmed_filter(ticket);
            let seconds = filter
                .expected_interval_seconds(target, cap)
                .map_err(anyhow::Error::msg)?;
            entry.insert(json!({"pool_job_id":pool_id,"ticket":ticket,
                "fixed_filter_zero_bits":policy.fixed_zero_bits(),"aggregate_cap_hps":cap,
                "intersection_probability":filter.intersection_probability(target),
                "expected_interval_seconds_at_full_aggregate_cap":seconds}));
        }
        verified.push(json!({
            "hash_index":index, "job_index":job_index, "pool_job_id":pool_id,
            "header_hex":hex::encode(header), "digest_hex":hex::encode(digest),
            "asic_filter_match":hash["asic_filter_match"]
        }));
    }
    ensure!(
        !verified.is_empty(),
        "no computed work matched an advertised pool job"
    );
    Ok(json!({
        "kind":"independent-firmware-pool-work-correlation",
        "pool_result":hash_file(&path)?,
        "execution_result":audit["execution_result"], "backend_log":audit["backend_log"],
        "trace_truncated":audit["trace_truncated"],
        "verifier_source":{"sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_pool_work_audit.rs")),
            "size":include_bytes!("s9_pool_work_audit.rs").len()},
        "hash_audit_source":audit["verifier_source"],
        "advertised_jobs":advertised.keys().collect::<Vec<_>>(), "matched_work_jobs":matched_jobs,
        "total_work_jobs":jobs.len(), "total_hashes":audit["hashes"].as_array().unwrap().len(),
        "searched_computed_states":wanted.len(),"unmatched_computed_hash_indices":unmatched,
        "all_computed_hashes_correlated":unmatched.is_empty(),
        "whole_run_rate_bound":audit["whole_run_rate_bound"],
        "monotonic_grant_rate":audit["monotonic_grant_rate"],
        "verified_full_headers":verified,
        "expected_share_intervals":intervals.values().collect::<Vec<_>>(),
        "interval_scope":"Lower bounds that assume the entire aggregate hash cap searches this job with this filter. Startup, inactive cores, other jobs and unequal filters can only increase these intervals.",
        "search":"Four-byte extra nonce counters0..1024 in either byte order; at most16 version mask bits; full midstate and tail must match.",
        "scope":"Only matched retained work is correlated. Unmatched work remains unresolved. No firmware submission, accepted share, block, boot or full-target pass is inferred."
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_job_history_keeps_every_computed_state_and_checks_its_indices() -> Result<()> {
        let first = json!({"midstates":[[1,2,3,4,5,6,7,8],[8,7,6,5,4,3,2,1]],
            "tail_hex":"0102030405060708090a0b0c"});
        let mut jobs = vec![first; 65537];
        jobs[65536]["tail_hex"] = json!("0c0b0a090807060504030201");
        let hashes = vec![
            json!({"job_index":0,"midstate_index":1}),
            json!({"job_index":65536,"midstate_index":0}),
            json!({"job_index":0,"midstate_index":1}),
        ];
        let wanted = computed_work_states(&jobs, &hashes)?;
        assert_eq!(
            wanted,
            BTreeSet::from([
                (
                    [8, 7, 6, 5, 4, 3, 2, 1],
                    [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]
                ),
                (
                    [1, 2, 3, 4, 5, 6, 7, 8],
                    [12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]
                )
            ])
        );
        for bad in [
            json!({"job_index":65537,"midstate_index":0}),
            json!({"job_index":0,"midstate_index":2}),
            json!({"job_index":-1,"midstate_index":0}),
            json!({"job_index":0}),
        ] {
            assert!(computed_work_states(&jobs, &[bad]).is_err());
        }
        jobs[65536]["tail_hex"] = json!("00");
        assert!(computed_work_states(&jobs, &hashes).is_err());
        Ok(())
    }

    #[test]
    fn pool_rejection_is_evidence_but_not_negotiation_success() {
        let current = ResponsePolicy::JsonRpc1StratumV1;
        let legacy = ResponsePolicy::LegacyFalseCode23;
        for (name, code) in [
            ("LowDifficulty", 23),
            ("Duplicate", 22),
            ("Stale", 21),
            ("UnknownJob", 21),
            ("OutOfRange", 20),
            ("InvalidVersion", 20),
            ("InvalidExtraNonce", 20),
        ] {
            let response = json!({"id":4,"result":null,"error":[code,name,null]});
            let old = json!({"id":4,"result":false,"error":[23,name,null]});
            check_response(current, "mining.submit", &response).unwrap();
            check_response(legacy, "mining.submit", &old).unwrap();
            assert!(check_response(legacy, "mining.submit", &response).is_err());
            assert!(check_response(current, "mining.submit", &old).is_err());
            assert!(check_response(current, "mining.subscribe", &response).is_err());
        }
        for bad in [
            json!({"id":4,"result":true,"error":[23,"LowDifficulty",null]}),
            json!({"id":4,"result":null,"error":[23,"unknown",null]}),
            json!({"id":4,"result":null,"error":[20,"LowDifficulty",null]}),
            json!({"id":4,"result":null,"error":[23,"LowDifficulty"]}),
            json!({"id":4,"result":null,"error":[23,"LowDifficulty",{}]}),
            json!({"id":4,"error":null}),
            json!({"id":4,"result":true}),
        ] {
            assert!(check_response(current, "mining.submit", &bad).is_err());
        }
    }

    #[test]
    fn fresh_prefix_search_requires_the_whole_state_and_all_bounds() {
        let mut random = [0; 32];
        std::fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut random)
            .unwrap();
        let job = PoolJob {
            id: hex::encode(random),
            previous: random,
            coinbase_prefix: random.to_vec(),
            extra_nonce1: vec![1, 2, 3, 4],
            extra_nonce2_size: 4,
            coinbase_suffix: vec![5, 6, 7, 8],
            merkle_branch: vec![random],
            version: 0x20000000,
            version_mask: 0x6000,
            bits: 0x207fffff,
            time_min: 100,
            time_max: 102,
            nonce_min: 0,
            nonce_max: u32::MAX,
            share_target: md_sim_mining::Target([255; 32]),
        };
        let mut wire_previous = random;
        for word in wire_previous.chunks_exact_mut(4) {
            word.reverse();
        }
        let notification = json!([
            job.id,
            hex::encode(wire_previous),
            hex::encode(&job.coinbase_prefix),
            hex::encode(&job.coinbase_suffix),
            [hex::encode(random)],
            "20000000",
            "207fffff",
            "00000064",
            true
        ]);
        check_notification(&job, &notification).unwrap();
        for (index, bad) in [
            (0, json!("other-job")),
            (1, json!("00")),
            (2, json!("")),
            (3, json!("")),
            (4, json!([])),
            (5, json!("20000001")),
            (6, json!("1d00ffff")),
            (7, json!("00000065")),
            (8, Value::Null),
        ] {
            let mut changed = notification.clone();
            changed[index] = bad;
            assert!(check_notification(&job, &changed).is_err());
        }
        let mut wanted = BTreeSet::new();
        let mut expected = BTreeMap::new();
        for (version, time) in [(0x20002000, 100), (0x20002000, 101), (0x20006000, 102)] {
            let header = job
                .reconstruct(&Share {
                    job_id: job.id.clone(),
                    extra_nonce2: 7u32.to_be_bytes().to_vec(),
                    nonce: 0,
                    time,
                    version,
                })
                .unwrap();
            let prefix: Prefix = header[..76].try_into().unwrap();
            let key = (state(&prefix), header[64..76].try_into().unwrap());
            wanted.insert(key);
            expected.insert(key, prefix);
        }
        assert_eq!(find_prefixes(&job, &wanted).unwrap(), expected);
        let (midstate, tail) = *wanted.first().unwrap();
        let mut bad_state = midstate;
        bad_state[0] ^= 1;
        assert!(find_prefixes(&job, &BTreeSet::from([(bad_state, tail)]))
            .unwrap()
            .is_empty());
        let mut bad_tail = tail;
        bad_tail[4..8].copy_from_slice(&103u32.to_le_bytes());
        assert!(find_prefixes(&job, &BTreeSet::from([(midstate, bad_tail)]))
            .unwrap()
            .is_empty());
        let mut bad_job = job.clone();
        bad_job.version_mask = 0;
        assert!(find_prefixes(&bad_job, &wanted).unwrap().is_empty());
        bad_job.version_mask = u32::MAX;
        assert!(find_prefixes(&bad_job, &wanted).is_err());
        bad_job.version_mask = job.version_mask;
        bad_job.previous[0] ^= 1;
        assert!(find_prefixes(&bad_job, &wanted).unwrap().is_empty());
    }
}

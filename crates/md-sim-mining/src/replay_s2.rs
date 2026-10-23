//! Replay saved S2 controller work/results against independent complete headers.
//! This tool does not execute firmware or establish an exact hardware identity.
use crate::{
    stratum::{difficulty_target, PoolReport},
    work::{cgminer_sha256d, header_midstate},
    Header, Share,
};
use bitcoin_hashes::{sha256d, Hash};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub type ReplayResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
type Result<T> = ReplayResult<T>;
struct PacketWork {
    midstate: [u8; 32],
    tail: [u8; 12],
    exponent: i32,
}
// Bitwise CRC-16/Modbus oracle. It does not use the device model's table.
fn crc16(bytes: &[u8]) -> u16 {
    let mut crc = 0xffff;
    for byte in bytes {
        crc ^= u16::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xa001
            } else {
                crc >> 1
            };
        }
    }
    crc
}
fn check_crc(packet: &[u8]) -> Result<()> {
    if packet.len() < 2 {
        return Err("packet is too short for CRC".into());
    }
    let n = packet.len() - 2;
    if crc16(&packet[..n]) != u16::from_le_bytes(packet[n..].try_into()?) {
        return Err("saved packet CRC differs from independent calculation".into());
    }
    Ok(())
}
fn check_metadata(event: &Value, sequence: u64, previous: u64, limit: u64) -> ReplayResult<u64> {
    let time = event["nanoseconds"]
        .as_u64()
        .ok_or("missing device event timestamp")?;
    if event["sequence"].as_u64() != Some(sequence)
        || time < previous
        || time > limit
        || event["clock"] != "wall-monotonic"
        || event["component"] != "s2-controller-stream"
        || event["parser"] != "md-sim-s2-v1"
    {
        return Err("device trace sequence, clock, timestamp or parser identity differs".into());
    }
    Ok(time)
}
pub fn replay(report: &PoolReport, trace_bytes: &[u8], max_elapsed_ns: u64) -> ReplayResult<Value> {
    let trace = std::str::from_utf8(trace_bytes)?;
    let mut works = BTreeMap::new();
    let mut input = Vec::new();
    let mut results = Vec::new();
    let mut reconstructions = 0u64;
    if report.jobs.len() > 64 || report.submissions.len() > 100000 || trace.len() > 20 * 1024 * 1024
    {
        return Err("replay input budget exceeded".into());
    }
    let mut previous_time = 0;
    for (sequence, line) in trace.lines().enumerate() {
        if results.len() > 4096 || works.len() > 65536 {
            return Err("replay state budget exceeded".into());
        }
        let event: Value = serde_json::from_str(line)?;
        previous_time = check_metadata(&event, sequence as u64, previous_time, max_elapsed_ns)?;
        let bytes = hex::decode(event["raw_hex"].as_str().ok_or("missing raw packet")?)?;
        if event["direction"] == "device-to-firmware" {
            check_crc(&bytes)?;
            if bytes.len() < 4
                || usize::from(u16::from_le_bytes(bytes[2..4].try_into()?)) + 4 != bytes.len()
            {
                return Err("device packet length differs from frame".into());
            }
        } else if event["direction"] != "firmware-to-device" {
            return Err("unknown device trace direction".into());
        }
        if event["direction"] == "firmware-to-device" {
            input.extend(bytes);
            while input.len() >= 4 {
                let length = usize::from(u16::from_le_bytes(input[2..4].try_into()?)) + 4;
                if !(6..=8192).contains(&length) {
                    return Err("invalid packet length".into());
                }
                if input.len() < length {
                    break;
                }
                let packet: Vec<_> = input.drain(..length).collect();
                check_crc(&packet)?;
                if packet[0] != 0x52 {
                    continue;
                }
                if packet.len() < 58 || (packet.len() - 10) % 48 != 0 {
                    return Err("invalid work frame".into());
                }
                if packet[4] & 1 != 0 {
                    works.clear();
                }
                for item in packet[8..length - 2].chunks_exact(48) {
                    works.insert(
                        u32::from_le_bytes(item[..4].try_into()?),
                        PacketWork {
                            midstate: item[4..36].try_into()?,
                            tail: item[36..48].try_into()?,
                            exponent: i32::from(packet[5] as i8),
                        },
                    );
                }
            }
        } else if bytes.first() == Some(&0xa2) {
            if bytes.len() != 26 {
                return Err("unsupported result frame".into());
            }
            check_crc(&bytes)?;
            let id = u32::from_le_bytes(bytes[16..20].try_into()?);
            let nonce = u32::from_be_bytes(bytes[20..24].try_into()?);
            let work = works.get(&id).ok_or("result refers to unavailable work")?;
            let time = u32::from_be_bytes(work.tail[4..8].try_into()?);
            let model_digest = cgminer_sha256d(work.midstate, work.tail, nonce);
            let mut matched = None;
            'jobs: for job in &report.jobs {
                // This is a bounded replay search over cgminer's observed
                // extranonce representation, not a search for a valid nonce.
                for n2 in 0..4096u32 {
                    let share = Share {
                        job_id: job.id.clone(),
                        extra_nonce2: n2.to_le_bytes().to_vec(),
                        nonce,
                        time,
                        version: job.version,
                    };
                    reconstructions += 1;
                    if reconstructions > 2_000_000 {
                        return Err("replay reconstruction budget exceeded".into());
                    }
                    let bytes = job.reconstruct(&share)?;
                    let header = Header::decode(&bytes)?;
                    let state_bytes: Vec<_> = header_midstate(&header)
                        .iter()
                        .flat_map(|word| word.to_le_bytes())
                        .collect();
                    let mut tail: [u8; 12] = bytes[64..76].try_into()?;
                    for word in tail.chunks_exact_mut(4) {
                        word.reverse();
                    }
                    if state_bytes.as_slice() != work.midstate || tail != work.tail {
                        continue;
                    }
                    let oracle = sha256d::Hash::hash(&bytes).to_byte_array();
                    if oracle != model_digest {
                        return Err("independent full-header hash differs from ASIC result".into());
                    }
                    let asic_valid = difficulty_target(work.exponent)?.accepts(&oracle);
                    if !asic_valid {
                        return Err("model returned a result outside the programmed filter".into());
                    }
                    let mut display = oracle;
                    display.reverse();
                    matched = Some(
                        json!({"sequence":event["sequence"],"work_id":id,"nonce":nonce,
                        "job_id":job.id,"extra_nonce2":hex::encode(&share.extra_nonce2),"time":time,
                        "header_hex":hex::encode(bytes),"hash_display":hex::encode(display),
                        "asic_exponent":work.exponent,"asic_valid":asic_valid,
                        "pool_target_met":job.share_target.accepts(&oracle),
                        "firmware_pool_submission_present":report.submissions.iter().any(|s|s.share.job_id==share.job_id && s.share.extra_nonce2==share.extra_nonce2 && s.share.nonce==nonce && s.share.time==time && s.share.version==share.version)}),
                    );
                    break 'jobs;
                }
            }
            results.push(matched.ok_or(
                "no complete pool header matches the recorded device work within replay budget",
            )?);
        }
    }
    if !input.is_empty() {
        return Err("trace ends inside a work packet".into());
    }
    Ok(
        json!({"scope":"saved source-interface evidence replay only","hash_oracle":"bitcoin_hashes 0.14.1 sha256d","results":results}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trace_order_and_declared_time_budget_are_enforced() {
        let mut event = json!({"nanoseconds":100,"sequence":0,"clock":"wall-monotonic","component":"s2-controller-stream","parser":"md-sim-s2-v1"});
        assert_eq!(check_metadata(&event, 0, 0, 100).unwrap(), 100);
        assert!(check_metadata(&event, 1, 0, 100).is_err());
        assert!(check_metadata(&event, 0, 101, 200).is_err());
        assert!(check_metadata(&event, 0, 0, 99).is_err());
        event["clock"] = json!("synthetic");
        assert!(check_metadata(&event, 0, 0, 100).is_err());
    }
    #[test]
    fn reference_crc_and_corrupted_packet() {
        assert_eq!(crc16(b"123456789"), 0x4b37);
        let mut packet = b"123456789\x37\x4b".to_vec();
        assert!(check_crc(&packet).is_ok());
        packet[0] ^= 1;
        assert!(check_crc(&packet).is_err());
    }
}

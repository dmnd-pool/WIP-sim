//! Audit retained hardware traffic. A decoded packet is not a mining pass.
use anyhow::{ensure, Context, Result};
#[path = "support/prepared_vm_image.rs"]
mod prepared_vm_image;
use md_sim_firmware::bm1387::{self, Packet};
use md_sim_runtime::store::hash_file;
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
};
#[allow(dead_code)] // The common pool bundle contract uses this artifact codec.
#[path = "support/worker_channel_artifacts.rs"]
mod channel_artifacts;
#[path = "support/s9_external_pool.rs"]
mod external_pool;
#[path = "support/s9_hash_audit.rs"]
mod hash_audit;
#[allow(dead_code)]
#[path = "support/s9_pool_input.rs"]
mod pool_input;
#[path = "support/s9_pool_report.rs"]
mod pool_report;
#[path = "support/s9_pool_work_audit.rs"]
mod pool_work_audit;
#[allow(dead_code)]
#[path = "support/worker_retention_frame.rs"]
mod retention_frame;
#[allow(dead_code)]
#[path = "support/s9_shared_pool.rs"]
mod shared_manifest;
#[allow(dead_code)]
#[path = "support/worker_shared_retention.rs"]
mod shared_retention;
#[path = "support/s9_submission_audit.rs"]
mod submission_audit;
#[path = "support/s9_temperature_audit.rs"]
mod temperature_audit;

const RESULT_BYTES: u64 = md_sim_runtime::isolation::MAX_TRACE_BYTES * 2 + 1024 * 1024;

fn read(path: &Path) -> Result<String> {
    read_bounded(path, md_sim_runtime::isolation::MAX_TRACE_BYTES)
}

fn read_bounded(path: &Path, limit: u64) -> Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "trace exceeds audit bound");
    Ok(String::from_utf8(bytes)?)
}

fn hex_field(fields: &BTreeMap<&str, &str>, key: &str) -> Result<u32> {
    let value = fields.get(key).context("missing trace field")?;
    Ok(u32::from_str_radix(
        value.strip_prefix("0x").context("not hexadecimal")?,
        16,
    )?)
}

fn packets(run: &Path) -> Result<serde_json::Value> {
    let path = run.join("backend.log");
    let log = read(&path)?;
    let mut pending: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut commands: BTreeMap<u32, usize> = BTreeMap::new();
    let mut work = Vec::new();
    for (line_index, line) in log.lines().enumerate() {
        let Some(body) = line.strip_prefix("md_sim_s9io_event ") else {
            continue;
        };
        let fields: BTreeMap<_, _> = body
            .split_whitespace()
            .filter_map(|field| field.split_once('='))
            .collect();
        if hex_field(&fields, "flags")? & 1 == 0 {
            continue;
        }
        let link: u32 = fields.get("link").context("no link")?.parse()?;
        let ns: u64 = fields.get("virtual_ns").context("no time")?.parse()?;
        let bytes = pending.entry(link).or_default();
        bytes.push(u8::try_from(hex_field(&fields, "tx")?)?);
        if bytes.len() < 2 {
            continue;
        }
        let length = usize::from(bytes[1]);
        ensure!((5..=150).contains(&length), "invalid packet length");
        if bytes.len() == length {
            let packet = bm1387::decode(bytes).map_err(|error| {
                anyhow::anyhow!("link{link}, line{}: {error:?}", line_index + 1)
            })?;
            match packet {
                Packet::Command { .. } => *commands.entry(link).or_default() += 1,
                Packet::Job(job) => work.push(json!({
                    "link":link, "last_byte_started_ns":ns, "last_byte_started_line":line_index+1,
                    "frame_hex":hex::encode(&*bytes), "job_id":job.id,
                    "midstates":job.states, "tail_hex":hex::encode(job.tail),
                    "nonce_origin_hex":hex::encode(job.nonce_origin)
                })),
            }
            bytes.clear();
        }
    }
    let partial: BTreeMap<_, _> = pending
        .into_iter()
        .filter(|(_, bytes)| !bytes.is_empty())
        .map(|(link, bytes)| (link, hex::encode(bytes)))
        .collect();
    ensure!(
        !commands.is_empty() || !work.is_empty(),
        "no S9IO TX-start packets; use hashes for an ASIC packet trace"
    );
    Ok(json!({
        "kind":"retained-firmware-asic-traffic",
        "backend_log":hash_file(&path)?,
        "adaptation":hash_file(&run.join("adaptation.json"))?,
        "process_specification":hash_file(&run.join("process-spec.json"))?,
        "commands_by_link":commands,"work_packets":work,"partial_packets":partial,
        "scope":"S9IO TX-start bytes from actual guest execution. Every complete packet checksum is checked. A final TX-start event alone does not establish completion at the peer. No accepted share, boot or mining gate is granted."
    }))
}

fn compare(baseline: &Path, changed: &Path) -> Result<serde_json::Value> {
    fn events(log: &str) -> Vec<&str> {
        log.lines()
            .filter(|line| {
                line.starts_with("md_sim_bm1387_event ") || line.starts_with("md_sim_s9io_event ")
            })
            .collect()
    }
    let first = read(&baseline.join("backend.log"))?;
    let second = read(&changed.join("backend.log"))?;
    let a = events(&first);
    let b = events(&second);
    ensure!(!a.is_empty() && !b.is_empty(), "missing device events");
    let identities = |root: &Path| -> Result<BTreeMap<_, _>> {
        ["backend.log", "assessment.json", "spec.json"]
            .into_iter()
            .map(|file| Ok((file, hash_file(&root.join(file))?)))
            .collect()
    };
    Ok(json!({
        "kind":"compiled-device-trace-comparison",
        "baseline":identities(baseline)?,"changed":identities(changed)?,
        "baseline_events":a.len(),"changed_events":b.len(),
        "exact_ordered_events_match":a==b,
        "first_difference":a.iter().zip(&b).position(|(left,right)| left!=right),
        "scope":"ASIC and S9IO events, including byte values, order and virtual timestamps. This comparison does not grant a firmware or mining pass."
    }))
}

/// Compare the compiled trace encoder with the original byte/event stream.
/// Both streams must come from the same retained execution.
fn compact_trace(log: &str) -> Result<usize> {
    let mut pending: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut expected = Vec::new();
    let actual: Vec<_> = log
        .lines()
        .filter(|line| {
            line.starts_with("md_sim_bm1387_packet ") || line.starts_with("md_sim_bm1387_state ")
        })
        .collect();
    for line in log.lines() {
        let Some(body) = line.strip_prefix("md_sim_bm1387_event ") else {
            continue;
        };
        let fields: BTreeMap<_, _> = body
            .split_whitespace()
            .filter_map(|field| field.split_once('='))
            .collect();
        let link: u32 = fields.get("link").context("missing link")?.parse()?;
        let kind: u32 = fields.get("kind").context("missing kind")?.parse()?;
        let value = hex_field(&fields, "value")?;
        let ns: u64 = fields.get("virtual_ns").context("missing time")?.parse()?;
        let bytes = pending.entry(link).or_default();
        if kind == 1 {
            ensure!(bytes.len() < 150, "raw packet exceeds bound");
            bytes.push(u8::try_from(value)?);
        } else if ((4..=7).contains(&kind) || kind == 12 || (matches!(kind, 8 | 9) && value == 0))
            && !bytes.is_empty()
        {
            expected.push(format!(
                "md_sim_bm1387_packet link={link} virtual_ns={ns} kind={kind} frame_hex={}",
                hex::encode(&*bytes)
            ));
            bytes.clear();
        }
        if !matches!(kind, 1 | 3) {
            expected.push(format!("md_sim_bm1387_state {body}"));
        }
    }
    ensure!(!expected.is_empty(), "no raw events for compact comparison");
    ensure!(expected == actual, "compact trace differs from raw events");
    Ok(actual.len())
}

fn compact_audit(run: &Path) -> Result<serde_json::Value> {
    let log = read(&run.join("backend.log"))?;
    let count = compact_trace(&log)?;
    let assessment: serde_json::Value = serde_json::from_str(&read(&run.join("assessment.json"))?)?;
    ensure!(
        assessment["passed"] == true,
        "component execution did not pass"
    );
    Ok(json!({
        "kind":"compiled-compact-trace-comparison",
        "backend_log":hash_file(&run.join("backend.log"))?,
        "assessment":hash_file(&run.join("assessment.json"))?,
        "process_specification":hash_file(&run.join("spec.json"))?,
        "exact_ordered_events_match":true, "compact_events":count,
        "scope":"Compact packets and states match the raw events from this component run. No firmware, boot or mining pass is granted."
    }))
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let (report, output) = match args.get(1).map(String::as_str) {
        Some("packets") if args.len() == 4 => (packets(Path::new(&args[2]))?, &args[3]),
        Some("hashes") if args.len() == 4 => (hash_audit::analyze(Path::new(&args[2]))?, &args[3]),
        Some("rates") if args.len() == 4 => (hash_audit::analyze_rate(Path::new(&args[2]))?, &args[3]),
        Some("compact") if args.len() == 4 => (compact_audit(Path::new(&args[2]))?, &args[3]),
        Some("pool-work") if args.len() == 4 => (pool_work_audit::analyze(Path::new(&args[2]))?, &args[3]),
        Some("submissions") if args.len() == 4 => (submission_audit::analyze(Path::new(&args[2]))?, &args[3]),
        Some("regtest") if args.len() == 4 => (submission_audit::regtest(Path::new(&args[2]))?, &args[3]),
        Some("regression-pool-split") if args.len() == 4 => (submission_audit::historical_split(Path::new(&args[2]))?, &args[3]),
        Some("submissions-shared") if args.len() == 4 => (submission_audit::analyze_shared(Path::new(&args[2]))?, &args[3]),
        Some("regtest-shared") if args.len() == 4 => (submission_audit::regtest_shared(Path::new(&args[2]), None)?, &args[3]),
        Some("regtest-session-shared") if args.len() == 5 => (submission_audit::regtest_shared(Path::new(&args[2]), Some(args[3].parse()?))?, &args[4]),
        Some("rates-shared") if args.len() == 4 => {
            let joined = external_pool::verify(Path::new(&args[2]), true)?;
            let mut report = hash_audit::analyze_rate(Path::new(&args[2]))?;
            report["external_pool"] = joined["external_pool"].clone();
            (report, &args[3])
        },
        Some("temperature") if args.len() == 4 => (temperature_audit::analyze(Path::new(&args[2]))?, &args[3]),
        Some("cooling") if args.len() == 4 => (temperature_audit::analyze_cooling(Path::new(&args[2]))?, &args[3]),
        Some("fan-recovery") if args.len() == 4 => (temperature_audit::analyze_fan_recovery(Path::new(&args[2]))?, &args[3]),
        Some("regtest-session") if args.len() == 5 => (
            submission_audit::regtest_session(Path::new(&args[2]), Some(args[3].parse()?))?,
            &args[4],
        ),
        Some("compare") if args.len() == 5 => {
            (compare(Path::new(&args[2]), Path::new(&args[3]))?, &args[4])
        }
        _ => anyhow::bail!(
            "usage: s9_asic_audit packets|hashes|rates|compact|pool-work|submissions|regtest|submissions-shared|regtest-shared|rates-shared|regression-pool-split|temperature|cooling|fan-recovery RUN NEW_REPORT | regtest-session|regtest-session-shared RUN SESSION_ID NEW_REPORT | compare BASELINE CHANGED NEW_REPORT"
        ),
    };
    let matches = report["exact_ordered_events_match"]
        .as_bool()
        .unwrap_or(true);
    let report = json!({
        "audit_source":{
            "sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_asic_audit.rs")),
            "size":include_bytes!("s9_asic_audit.rs").len()
        },
        "audit_binary":hash_file(Path::new("/proc/self/exe"))?,
        "result":report
    });
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&serde_json::to_vec_pretty(&report)?)?;
    ensure!(matches, "compiled event traces differ");
    Ok(())
}

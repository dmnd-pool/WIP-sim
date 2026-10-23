//! Independent Stratum V1 verification over explicit stream transports.
//! TCP callers must establish their own listener isolation. A pool report
//! alone does not establish firmware, regtest or remote execution gates.
use crate::regtest::RegtestAnchor;
use crate::{PoolJob, PoolVerifier, Result, Share, Target, Verdict};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
mod extensions;

pub struct StratumConfig {
    pub response_policy: ResponsePolicy,
    /// An explicit genesis parent for offline regtest import. None keeps the
    /// random-parent protocol suite. This is not a node-acceptance result.
    pub regtest_anchor: Option<RegtestAnchor>,
    pub seed: [u8; 32],
    pub time: u32,
    /// Power-of-two difficulties. -20 means difficulty 2^-20.
    pub difficulty_exponents: Vec<i32>,
    pub difficulty_scale: DifficultyScale,
    pub timeout: Duration,
    pub trace_path: PathBuf,
    pub limits: StratumLimits,
}
/// The legacy policy exists only to replay old, frozen inputs. New execution
/// uses JSON-RPC 1.0 null error results and the Stratum 20..25 error codes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResponsePolicy {
    #[default]
    LegacyFalseCode23,
    JsonRpc1StratumV1,
}

impl ResponsePolicy {
    fn submit_error(self, id: Value, code: i32, message: &str) -> Value {
        let result = match self {
            Self::LegacyFalseCode23 => json!(false),
            Self::JsonRpc1StratumV1 => Value::Null,
        };
        json!({"id":id,"result":result,"error":[code,message,null]})
    }

    fn verdict(self, id: Value, verdict: &Verdict) -> Value {
        let code = match verdict {
            Verdict::Accepted { .. } => return json!({"id":id,"result":true,"error":null}),
            Verdict::Stale | Verdict::UnknownJob => 21,
            Verdict::Duplicate => 22,
            Verdict::LowDifficulty => 23,
            Verdict::InvalidExtraNonce | Verdict::OutOfRange | Verdict::InvalidVersion => 20,
        };
        self.submit_error(
            id,
            if self == Self::LegacyFalseCode23 {
                23
            } else {
                code
            },
            &format!("{verdict:?}"),
        )
    }
}
/// Exact supported rational difficulty factors. ThreeHalves tests a pool
/// target stricter than the ASIC power-of-two filter selected by the firmware.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum DifficultyScale {
    One,
    ThreeHalves,
}
impl DifficultyScale {
    fn value(self) -> f64 {
        match self {
            Self::One => 1.0,
            Self::ThreeHalves => 1.5,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StratumLimits {
    pub max_messages: usize,
    pub max_submissions: usize,
    pub trace_bytes: u64,
}
impl Default for StratumLimits {
    fn default() -> Self {
        Self {
            max_messages: 10_000,
            max_submissions: 5_000,
            trace_bytes: 16 * 1024 * 1024,
        }
    }
}
struct Trace {
    file: File,
    saved: u64,
    limit: u64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompletionPolicy {
    #[default]
    CloseAfterWork,
    KeepOpen,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct SubmissionEvidence {
    pub share: Share,
    pub verdict: Verdict,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct PoolReport {
    #[serde(default)]
    pub response_policy: ResponsePolicy,
    #[serde(default)]
    pub connection_policy: CompletionPolicy,
    #[serde(default)]
    pub connection_end: Option<String>,
    #[serde(default)]
    pub regtest_anchor: Option<RegtestAnchor>,
    pub transport: String,
    pub hash_implementation: String,
    pub jobs: Vec<PoolJob>,
    pub submissions: Vec<SubmissionEvidence>,
    pub completed: bool,
    pub stopped: bool,
    pub timed_out: bool,
    pub limits: StratumLimits,
    pub messages_received: usize,
    pub budget_exhausted: bool,
    pub failure: Option<String>,
}

pub fn difficulty_target(exponent: i32) -> Result<Target> {
    if !(-31..=32).contains(&exponent) {
        return Err("difficulty exponent must be -31 through 32".into());
    }
    let mut target = Target::from_compact(0x1d00ffff)?.0;
    if exponent < 0 {
        for _ in 0..-exponent {
            let mut carry = 0;
            for byte in target.iter_mut().rev() {
                let next = *byte >> 7;
                *byte = (*byte << 1) | carry;
                carry = next;
            }
        }
    } else {
        for _ in 0..exponent {
            let mut carry = 0;
            for byte in &mut target {
                let next = (*byte & 1) << 7;
                *byte = (*byte >> 1) | carry;
                carry = next;
            }
        }
    }
    Ok(Target(target))
}

pub fn difficulty_target_scaled(exponent: i32, scale: DifficultyScale) -> Result<Target> {
    let target = difficulty_target(exponent)?;
    if matches!(scale, DifficultyScale::One) {
        return Ok(target);
    }
    // A 257-bit intermediate preserves the multiply, then integer long division
    // computes floor(2*T/3). The pool comparison remains an exact integer test.
    let mut doubled = [0u8; 33];
    let mut carry = 0;
    for i in (0..32).rev() {
        doubled[i + 1] = (target.0[i] << 1) | carry;
        carry = target.0[i] >> 7;
    }
    doubled[0] = carry;
    let mut remainder = 0u16;
    for byte in &mut doubled {
        let value = (remainder << 8) | u16::from(*byte);
        *byte = (value / 3) as u8;
        remainder = value % 3;
    }
    if doubled[0] != 0 {
        return Err("scaled target overflow".into());
    }
    Ok(Target(doubled[1..].try_into().unwrap()))
}

fn make_job(config: &StratumConfig, index: usize) -> Result<PoolJob> {
    let time_max = job_time_max(config)?;
    // A serialized coinbase transaction. Anchored mode starts with the exact
    // BIP34 height-one script (OP_1), then entropy and both extra nonces.
    // The older random-parent protocol fixture has no consensus claim.
    let mut prefix = vec![1, 0, 0, 0, 1];
    prefix.extend([0; 32]);
    prefix.extend([255; 4]);
    if config.regtest_anchor.is_some() {
        prefix.extend([42, 0x51, 0x20]);
    } else {
        prefix.extend([42, 1, 1]);
    }
    let mut entropy = config.seed;
    if config.regtest_anchor.is_some() {
        entropy[0] ^= index as u8;
    }
    prefix.extend(entropy);
    let mut suffix = vec![255; 4];
    suffix.push(1);
    suffix.extend(5_000_000_000u64.to_le_bytes());
    suffix.extend([1, 0x51, 0, 0, 0, 0]);
    let mut previous = config.seed;
    previous[0] ^= index as u8;
    if let Some(anchor) = &config.regtest_anchor {
        anchor.validate()?;
        if config.time <= anchor.genesis_time {
            return Err("regtest job time must be after genesis".into());
        }
        previous = anchor.previous_hash;
    }
    Ok(PoolJob {
        id: format!("{}-{index}", hex::encode(&config.seed[..8])),
        previous,
        coinbase_prefix: prefix,
        extra_nonce1: config.seed[..4].to_vec(),
        extra_nonce2_size: 4,
        coinbase_suffix: suffix,
        merkle_branch: vec![],
        version: 0x20000000,
        version_mask: 0,
        bits: 0x207fffff,
        time_min: config.time,
        time_max,
        nonce_min: 0,
        nonce_max: u32::MAX,
        share_target: difficulty_target_scaled(
            config.difficulty_exponents[index],
            config.difficulty_scale,
        )?,
    })
}

fn job_time_max(config: &StratumConfig) -> Result<u32> {
    if config.timeout.is_zero() || config.timeout > Duration::from_secs(86_400) {
        return Err("mining timeout must be positive and at most one day".into());
    }
    // The guest can roll ntime while a low-rate job remains active. Cover the
    // full work budget, rounded up, with a fixed 120-second clock margin.
    // Bus and network deadlines do not use this work-dependent window.
    let seconds = config.timeout.as_secs() + u64::from(config.timeout.subsec_nanos() > 0);
    config
        .time
        .checked_add(seconds as u32)
        .and_then(|time| time.checked_add(120))
        .ok_or_else(|| "job time overflow".into())
}

fn trace(file: &mut Trace, start: Instant, direction: &str, value: &Value) -> Result<()> {
    let mut bytes=serde_json::to_vec(&json!({"elapsed_ns":start.elapsed().as_nanos().to_string(),"direction":direction,"message":value})).map_err(|e|e.to_string())?;
    bytes.push(b'\n');
    if file.saved + bytes.len() as u64 > file.limit {
        return Err("pool trace budget exhausted".into());
    }
    file.file.write_all(&bytes).map_err(|e| e.to_string())?;
    file.saved += bytes.len() as u64;
    file.file.flush().map_err(|e| e.to_string())
}
fn send<S: Write>(stream: &mut S, file: &mut Trace, start: Instant, value: Value) -> Result<()> {
    trace(file, start, "pool-to-firmware", &value)?;
    writeln!(stream, "{value}").map_err(|e| e.to_string())
}
fn announce<S: Write>(
    stream: &mut S,
    file: &mut Trace,
    start: Instant,
    job: &PoolJob,
    exponent: i32,
    scale: DifficultyScale,
) -> Result<()> {
    send(
        stream,
        file,
        start,
        json!({"id":null,"method":"mining.set_difficulty","params":[scale.value()*2f64.powi(exponent)]}),
    )?;
    let mut previous = job.previous;
    for word in previous.chunks_exact_mut(4) {
        word.reverse();
    }
    send(
        stream,
        file,
        start,
        json!({"id":null,"method":"mining.notify","params":[
        job.id,hex::encode(previous),hex::encode(&job.coinbase_prefix),hex::encode(&job.coinbase_suffix),
        job.merkle_branch.iter().map(hex::encode).collect::<Vec<_>>(),format!("{:08x}",job.version),
        format!("{:08x}",job.bits),format!("{:08x}",job.time_min),true]}),
    )
}
fn parse_u32_hex(value: &Value) -> Result<u32> {
    let s = value.as_str().ok_or("expected hexadecimal string")?;
    if s.len() != 8 {
        return Err("expected exactly four hex bytes".into());
    }
    u32::from_str_radix(s, 16).map_err(|e| e.to_string())
}
enum SubmissionError {
    Malformed(String),
    UnauthorizedWorker,
}
impl From<String> for SubmissionError {
    fn from(message: String) -> Self {
        Self::Malformed(message)
    }
}
impl From<&str> for SubmissionError {
    fn from(message: &str) -> Self {
        Self::Malformed(message.into())
    }
}

fn parse_share(
    params: &Value,
    verifier: &PoolVerifier,
    version_mask: Option<u32>,
) -> std::result::Result<Share, SubmissionError> {
    let a = params
        .as_array()
        .ok_or("submit parameters must be an array")?;
    if a.len() != if version_mask.is_some() { 6 } else { 5 } {
        return Err("unsupported submit parameter count".into());
    }
    if a[0] != "simulator" {
        if !a[0].is_string() {
            return Err("submission worker differs from authorized worker".into());
        }
        return Err(SubmissionError::UnauthorizedWorker);
    }
    let id = a[1].as_str().ok_or("invalid job ID")?.to_string();
    let mut version = verifier.job(&id).map(|j| j.version).unwrap_or(0x20000000);
    if let Some(mask) = version_mask {
        let bits = parse_u32_hex(&a[5])?;
        if bits & !mask != 0 {
            return Err("submitted version bits exceed negotiated mask".into());
        }
        version = (version & !mask) | bits;
    }
    Ok(Share {
        job_id: id,
        extra_nonce2: hex::decode(a[2].as_str().ok_or("invalid extra nonce")?)
            .map_err(|e| e.to_string())?,
        time: parse_u32_hex(&a[3])?,
        nonce: parse_u32_hex(&a[4])?,
        version,
    })
}

/// Serve one firmware session. The listener must use run-owned storage. The
/// caller owns its socket path and must remove it after this function returns.
pub fn serve_unix(
    listener: UnixListener,
    config: StratumConfig,
    stop: Arc<AtomicBool>,
) -> Result<PoolReport> {
    let (report, file) = initialize(&config)?;
    let mut report = report;
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let start = Instant::now();
    let stream = loop {
        if stop.load(Ordering::Relaxed) {
            report.stopped = true;
            return Ok(report);
        }
        if start.elapsed() >= config.timeout {
            report.timed_out = true;
            return Ok(report);
        }
        match listener.accept() {
            Ok((s, _)) => break s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    configure_stream(&stream)?;
    serve_connection(stream, config, stop, report, file, start)
}

/// Serve a private inherited socket pair. No listener or network is opened.
pub fn serve_stream(
    stream: UnixStream,
    config: StratumConfig,
    stop: Arc<AtomicBool>,
) -> Result<PoolReport> {
    let (report, file) = initialize(&config)?;
    configure_stream(&stream)?;
    serve_connection(stream, config, stop, report, file, Instant::now())
}

/// One accepted TCP connection. The caller owns its listening endpoint and
/// must establish the network isolation recorded in its run specification.
pub fn serve_tcp_stream(
    stream: std::net::TcpStream,
    config: StratumConfig,
    stop: Arc<AtomicBool>,
) -> Result<PoolReport> {
    tcp_connection(stream, config, stop, CompletionPolicy::CloseAfterWork)
}

/// Keep verifying the final job after all required fresh jobs have accepted
/// work. A clean peer close or the declared window ends a completed session.
/// Cancellation, limits, incomplete frames and protocol errors still fail.
pub fn serve_tcp_window(
    stream: std::net::TcpStream,
    config: StratumConfig,
    stop: Arc<AtomicBool>,
) -> Result<PoolReport> {
    tcp_connection(stream, config, stop, CompletionPolicy::KeepOpen)
}

fn tcp_connection(
    stream: std::net::TcpStream,
    config: StratumConfig,
    stop: Arc<AtomicBool>,
    policy: CompletionPolicy,
) -> Result<PoolReport> {
    let (mut report, file) = initialize(&config)?;
    report.connection_policy = policy;
    report.transport = "tcp-stream-posix-io".into();
    stream
        .set_read_timeout(Some(Duration::from_millis(50)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .map_err(|e| e.to_string())?;
    serve_connection(stream, config, stop, report, file, Instant::now())
}

fn initialize(config: &StratumConfig) -> Result<(PoolReport, Trace)> {
    if config.difficulty_exponents.len() < 2 || config.difficulty_exponents.len() > 64 {
        return Err("fresh-work suite requires two through 64 jobs".into());
    }
    if config.limits.max_messages == 0
        || config.limits.max_messages > 100_000
        || config.limits.max_submissions == 0
        || config.limits.max_submissions > 100_000
        || config.limits.trace_bytes == 0
        || config.limits.trace_bytes > 64 * 1024 * 1024
    {
        return Err("invalid pool resource limits".into());
    }
    let jobs = (0..config.difficulty_exponents.len())
        .map(|i| make_job(config, i))
        .collect::<Result<Vec<_>>>()?;
    let report = PoolReport {
        response_policy: config.response_policy,
        connection_policy: CompletionPolicy::CloseAfterWork,
        connection_end: None,
        regtest_anchor: config.regtest_anchor.clone(),
        transport: "adapted-stratum-v1-unix-stream".into(),
        hash_implementation: "bitcoin_hashes=0.14.1/sha256d".into(),
        jobs,
        submissions: vec![],
        completed: false,
        stopped: false,
        timed_out: false,
        limits: config.limits.clone(),
        messages_received: 0,
        budget_exhausted: false,
        failure: None,
    };
    let file = File::options()
        .write(true)
        .create_new(true)
        .open(&config.trace_path)
        .map_err(|e| e.to_string())?;
    Ok((
        report,
        Trace {
            file,
            saved: 0,
            limit: config.limits.trace_bytes,
        },
    ))
}

fn serve_connection<S: Read + Write>(
    mut stream: S,
    config: StratumConfig,
    stop: Arc<AtomicBool>,
    mut report: PoolReport,
    mut file: Trace,
    start: Instant,
) -> Result<PoolReport> {
    let mut input = Vec::new();
    let mut verifier = PoolVerifier::default();
    let mut job_index = 0;
    let mut subscribed = false;
    let mut version_mask = None;
    let mut authorized = false;
    let mut announced = false;
    let result = (|| -> Result<()> {
        loop {
            if stop.load(Ordering::Relaxed) {
                report.stopped = true;
                report.connection_end = Some("cancelled".into());
                break;
            }
            if start.elapsed() >= config.timeout {
                if report.connection_policy == CompletionPolicy::KeepOpen && report.completed {
                    report.connection_end = Some("planned-window-end".into());
                } else {
                    report.timed_out = true;
                    report.connection_end = Some("work-deadline".into());
                }
                break;
            }
            let mut bytes = [0; 4096];
            match stream.read(&mut bytes) {
                Ok(0) => {
                    report.connection_end = Some("peer-closed".into());
                    if report.connection_policy == CompletionPolicy::KeepOpen
                        && report.completed
                        && input.is_empty()
                    {
                        return Ok(());
                    }
                    return Err(
                        "firmware stream closed before suite completion or with a partial frame"
                            .into(),
                    );
                }
                Ok(n) => input.extend_from_slice(&bytes[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    continue
                }
                Err(e) => return Err(e.to_string()),
            }
            if input.len() > 65536 {
                return Err("Stratum frame exceeds 64 KiB".into());
            }
            while let Some(end) = input.iter().position(|byte| *byte == b'\n') {
                if report.messages_received >= config.limits.max_messages {
                    return Err("pool message budget exhausted".into());
                }
                report.messages_received += 1;
                let line: Vec<u8> = input.drain(..=end).collect();
                let request: Value = match serde_json::from_slice(&line) {
                    Ok(request) => request,
                    Err(error) => {
                        // Retain the exact rejected frame. A parse message
                        // alone cannot distinguish bad guest data from a
                        // transport or framing defect.
                        trace(
                            &mut file,
                            start,
                            "firmware-to-pool-invalid-json",
                            &json!({"raw_hex":hex::encode(&line),"error":error.to_string()}),
                        )?;
                        return Err(error.to_string());
                    }
                };
                trace(&mut file, start, "firmware-to-pool", &request)?;
                let id = request.get("id").cloned().unwrap_or(Value::Null);
                let method = request["method"].as_str().ok_or("missing Stratum method")?;
                let params = &request["params"];
                let result = match method {
                    "mining.subscribe" => {
                        subscribed = true;
                        json!([
                            [
                                ["mining.notify", "md-sim-v1"],
                                ["mining.set_difficulty", "md-sim-v1"]
                            ],
                            hex::encode(&config.seed[..4]),
                            4
                        ])
                    }
                    "mining.authorize" => {
                        authorized = params[0] == "simulator" && params[1] == "x";
                        json!(authorized)
                    }
                    "mining.configure" => {
                        match extensions::configure(params, version_mask, announced) {
                            Ok((response, negotiated)) => {
                                version_mask = negotiated;
                                for job in &mut report.jobs {
                                    job.version_mask = negotiated.unwrap_or(0);
                                }
                                response
                            }
                            Err(error) => {
                                send(
                                    &mut stream,
                                    &mut file,
                                    start,
                                    json!({"id":id,"result":null,"error":[20,error,null]}),
                                )?;
                                continue;
                            }
                        }
                    }
                    "mining.extranonce.subscribe" | "mining.suggest_difficulty" => json!(true),
                    "mining.submit" => {
                        if !authorized || !subscribed {
                            let (code, message) = if config.response_policy
                                == ResponsePolicy::JsonRpc1StratumV1
                                && authorized
                                && !subscribed
                            {
                                (25, "not subscribed")
                            } else {
                                (24, "not authorized")
                            };
                            send(
                                &mut stream,
                                &mut file,
                                start,
                                config.response_policy.submit_error(id, code, message),
                            )?;
                            continue;
                        }
                        let share = match parse_share(params, &verifier, version_mask) {
                            Ok(s) => s,
                            Err(e) => {
                                let (code, message) = match e {
                                    SubmissionError::Malformed(message) => (20, message),
                                    SubmissionError::UnauthorizedWorker => (
                                        if config.response_policy
                                            == ResponsePolicy::LegacyFalseCode23
                                        {
                                            20
                                        } else {
                                            24
                                        },
                                        "submission worker differs from authorized worker".into(),
                                    ),
                                };
                                send(
                                    &mut stream,
                                    &mut file,
                                    start,
                                    config.response_policy.submit_error(id, code, &message),
                                )?;
                                continue;
                            }
                        };
                        if report.submissions.len() >= config.limits.max_submissions {
                            return Err("pool submission budget exhausted".into());
                        }
                        let verdict = verifier.verify(&share)?;
                        let accepted = matches!(verdict, Verdict::Accepted { .. });
                        let response = config.response_policy.verdict(id, &verdict);
                        report
                            .submissions
                            .push(SubmissionEvidence { share, verdict });
                        send(&mut stream, &mut file, start, response)?;
                        if accepted && !report.completed {
                            job_index += 1;
                            if job_index == report.jobs.len() {
                                report.completed = true;
                                if report.connection_policy == CompletionPolicy::CloseAfterWork {
                                    report.connection_end = Some("required-work-complete".into());
                                    return Ok(());
                                }
                                continue;
                            }
                            verifier.install(report.jobs[job_index].clone(), true)?;
                            announce(
                                &mut stream,
                                &mut file,
                                start,
                                &report.jobs[job_index],
                                config.difficulty_exponents[job_index],
                                config.difficulty_scale,
                            )?;
                        }
                        continue;
                    }
                    _ => {
                        send(
                            &mut stream,
                            &mut file,
                            start,
                            json!({"id":id,"result":null,"error":[20,"unsupported method",null]}),
                        )?;
                        continue;
                    }
                };
                send(
                    &mut stream,
                    &mut file,
                    start,
                    json!({"id":id,"result":result,"error":null}),
                )?;
                if subscribed && authorized && !announced {
                    verifier.install(report.jobs[0].clone(), true)?;
                    announce(
                        &mut stream,
                        &mut file,
                        start,
                        &report.jobs[0],
                        config.difficulty_exponents[0],
                        config.difficulty_scale,
                    )?;
                    announced = true;
                }
            }
        }
        Ok(())
    })();
    let result = if input.is_empty() {
        result
    } else {
        let retained = trace(
            &mut file,
            start,
            "firmware-to-pool-incomplete-frame",
            &json!({"raw_hex":hex::encode(&input)}),
        );
        match (result, retained) {
            (Err(error), Err(retention)) => Err(format!("{error}; {retention}")),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), retention) => retention,
        }
    };
    if let Err(error) = result {
        report.budget_exhausted = error.contains("budget exhausted");
        report.failure = Some(error);
    }
    Ok(report)
}

fn configure_stream(stream: &UnixStream) -> Result<()> {
    stream
        .set_read_timeout(Some(Duration::from_millis(50)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(1)))
        .map_err(|e| e.to_string())
}

/// Private inherited IPC with POSIX read/write. This adapter is separate from
/// the Unix listener transport. It never opens a network connection.
pub fn serve_inherited(
    stream: File,
    config: StratumConfig,
    stop: Arc<AtomicBool>,
) -> Result<PoolReport> {
    inherited_connection(stream, config, stop, CompletionPolicy::CloseAfterWork)
}

/// The same persistent-session contract as `serve_tcp_window`, over private
/// inherited IPC. It opens no listener and does not claim firmware execution.
pub fn serve_inherited_window(
    stream: File,
    config: StratumConfig,
    stop: Arc<AtomicBool>,
) -> Result<PoolReport> {
    inherited_connection(stream, config, stop, CompletionPolicy::KeepOpen)
}

fn inherited_connection(
    stream: File,
    config: StratumConfig,
    stop: Arc<AtomicBool>,
    policy: CompletionPolicy,
) -> Result<PoolReport> {
    let (mut report, file) = initialize(&config)?;
    report.connection_policy = policy;
    report.transport = "adapted-stratum-v1-inherited-socketpair-posix-io".into();
    serve_connection(
        InheritedStream::nonblocking(stream)?,
        config,
        stop,
        report,
        file,
        Instant::now(),
    )
}

/// Two run-owned pipes. A caller may connect these to an isolated emulator's
/// TCP forwarding device. This function alone does not establish guest TCP.
pub fn serve_pipes(
    reader: File,
    writer: File,
    config: StratumConfig,
    stop: Arc<AtomicBool>,
) -> Result<PoolReport> {
    serve_split_io(
        reader,
        writer,
        config,
        stop,
        "private-duplex-pipes-posix-io",
        CompletionPolicy::CloseAfterWork,
    )
}

/// A single connection inherited from an emulator's per-connection command.
/// This function does not itself prove the enclosing guest TCP path.
pub fn serve_command_stream(
    reader: File,
    writer: File,
    config: StratumConfig,
    stop: Arc<AtomicBool>,
) -> Result<PoolReport> {
    serve_split_io(
        reader,
        writer,
        config,
        stop,
        "private-command-connection-posix-io",
        CompletionPolicy::CloseAfterWork,
    )
}

/// Keep one emulator command connection active for its declared work window.
/// Later submissions retain the same verifier, stale-job and resource checks.
pub fn serve_command_window(
    reader: File,
    writer: File,
    config: StratumConfig,
    stop: Arc<AtomicBool>,
) -> Result<PoolReport> {
    serve_split_io(
        reader,
        writer,
        config,
        stop,
        "private-command-connection-posix-io",
        CompletionPolicy::KeepOpen,
    )
}

fn serve_split_io(
    reader: File,
    writer: File,
    config: StratumConfig,
    stop: Arc<AtomicBool>,
    transport: &str,
    policy: CompletionPolicy,
) -> Result<PoolReport> {
    let (mut report, file) = initialize(&config)?;
    report.transport = transport.into();
    report.connection_policy = policy;
    serve_connection(
        PipeStream {
            reader: InheritedStream::nonblocking(reader)?,
            writer: InheritedStream::nonblocking(writer)?,
        },
        config,
        stop,
        report,
        file,
        Instant::now(),
    )
}

struct PipeStream {
    reader: InheritedStream,
    writer: InheritedStream,
}
impl Read for PipeStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reader.read(buf)
    }
}
impl Write for PipeStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

struct InheritedStream(File);
impl InheritedStream {
    fn nonblocking(stream: File) -> Result<Self> {
        let fd = stream.as_raw_fd();
        // SAFETY: this owned descriptor stays live through both fcntl calls.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(Self(stream))
    }
    fn wait(&self, events: i16) -> std::io::Result<()> {
        let mut descriptor = libc::pollfd {
            fd: self.0.as_raw_fd(),
            events,
            revents: 0,
        };
        // SAFETY: descriptor points to one initialized pollfd for this owned fd.
        let ready = unsafe { libc::poll(&mut descriptor, 1, 50) };
        if ready < 0 {
            return Err(std::io::Error::last_os_error());
        }
        if ready == 0 {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        if descriptor.revents & libc::POLLNVAL != 0 {
            return Err(std::io::ErrorKind::InvalidInput.into());
        }
        if descriptor.revents & libc::POLLERR != 0 {
            return Err(std::io::ErrorKind::BrokenPipe.into());
        }
        if events == libc::POLLOUT && descriptor.revents & libc::POLLHUP != 0 {
            return Err(std::io::ErrorKind::BrokenPipe.into());
        }
        if descriptor.revents & (events | libc::POLLHUP) == 0 {
            return Err(std::io::ErrorKind::WouldBlock.into());
        }
        Ok(())
    }
}
impl Read for InheritedStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.wait(libc::POLLIN)?;
        self.0.read(buf)
    }
}
impl Write for InheritedStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.wait(libc::POLLOUT)?;
        self.0.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

#[cfg(test)]
mod io_tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::os::fd::OwnedFd;

    #[test]
    fn stalled_peer_cannot_block_a_large_inherited_write() {
        let (stream, _peer) = UnixStream::pair().unwrap();
        stream.set_nonblocking(true).unwrap();
        let mut stream = InheritedStream(File::from(OwnedFd::from(stream)));
        let start = Instant::now();
        let data = vec![0u8; 4 * 1024 * 1024];
        assert!(stream.write_all(&data).is_err());
        assert!(start.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn regtest_export_preserves_actual_share_and_rejects_forged_verdict() {
        let dir = tempfile::tempdir().unwrap();
        let config = StratumConfig {
            response_policy: ResponsePolicy::JsonRpc1StratumV1,
            regtest_anchor: Some(RegtestAnchor::genesis()),
            seed: [0x42; 32],
            time: 1_750_000_000,
            difficulty_exponents: vec![-31, -30],
            difficulty_scale: DifficultyScale::ThreeHalves,
            timeout: Duration::from_secs(2),
            trace_path: dir.path().join("trace.jsonl"),
            limits: StratumLimits::default(),
        };
        let (mut report, _) = initialize(&config).unwrap();
        assert_eq!(report.jobs[0].previous, report.jobs[1].previous);
        assert_ne!(
            report.jobs[0].coinbase_prefix,
            report.jobs[1].coinbase_prefix
        );
        let job = report.jobs[0].clone();
        let mut share = Share {
            job_id: job.id.clone(),
            extra_nonce2: vec![0; 4],
            nonce: 0,
            time: job.time_min,
            version: job.version,
        };
        let mut verifier = PoolVerifier::default();
        verifier.install(job.clone(), true).unwrap();
        let verdict = loop {
            let verdict = verifier.verify(&share).unwrap();
            if matches!(
                verdict,
                Verdict::Accepted {
                    block_candidate: true,
                    ..
                }
            ) {
                break verdict;
            }
            share.nonce += 1;
            assert!(share.nonce < 1000);
        };
        report
            .submissions
            .push(SubmissionEvidence { share, verdict });
        let blocks = crate::regtest::regtest_blocks(&report).unwrap();
        assert_eq!(blocks.len(), 1);
        let block = hex::decode(&blocks[0].block_hex).unwrap();
        let import = hex::decode(&blocks[0].import_hex).unwrap();
        assert_eq!(&import[..4], &[0xfa, 0xbf, 0xb5, 0xda]);
        assert_eq!(u32::from_le_bytes(import[4..8].try_into().unwrap()), 184);
        assert_eq!(&import[8..], block);
        assert_eq!(block[80], 1); // CompactSize transaction count.
        let tx = &block[81..];
        // Independent fixed-format consensus checks for this one-input,
        // one-output transaction: null outpoint, canonical BIP34 height,
        // subsidy, OP_TRUE output, zero locktime. Core supplies the full oracle.
        assert_eq!(&tx[..5], &[1, 0, 0, 0, 1]);
        assert_eq!(&tx[5..37], &[0; 32]);
        assert_eq!(&tx[37..41], &[255; 4]);
        assert_eq!(&tx[41..44], &[42, 0x51, 0x20]);
        assert_eq!(&tx[84..89], &[255, 255, 255, 255, 1]);
        assert_eq!(
            u64::from_le_bytes(tx[89..97].try_into().unwrap()),
            5_000_000_000
        );
        assert_eq!(&tx[97..], &[1, 0x51, 0, 0, 0, 0]);
        let root = Sha256::digest(Sha256::digest(tx));
        assert_eq!(&block[36..68], root.as_slice());
        assert_eq!(
            &block[76..80],
            report.submissions[0].share.nonce.to_le_bytes()
        );
        let original_nonce = report.submissions[0].share.nonce;
        report.submissions[0].share.nonce = original_nonce.wrapping_add(1);
        assert!(crate::regtest::regtest_blocks(&report).is_err());
        report.submissions[0].share.nonce = original_nonce;
        let Verdict::Accepted { header_hex, .. } = &mut report.submissions[0].verdict else {
            unreachable!()
        };
        *header_hex = "00".repeat(80);
        assert!(crate::regtest::regtest_blocks(&report).is_err());
        report.regtest_anchor = None;
        assert!(crate::regtest::regtest_blocks(&report).is_err());
    }

    #[test]
    fn rolled_time_covers_long_mining_budget_but_rejects_outside_work_window() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = StratumConfig {
            response_policy: ResponsePolicy::JsonRpc1StratumV1,
            regtest_anchor: None,
            seed: [0x39; 32],
            time: 1_750_000_000,
            difficulty_exponents: vec![-31, -30],
            difficulty_scale: DifficultyScale::ThreeHalves,
            timeout: Duration::from_secs(600),
            trace_path: dir.path().join("trace.jsonl"),
            limits: StratumLimits::default(),
        };
        let job = make_job(&config, 0).unwrap();
        let mut share = Share {
            job_id: job.id.clone(),
            extra_nonce2: vec![0; 4],
            nonce: 0,
            time: config.time + 300,
            version: job.version,
        };
        // Search this test submission with the implementation independent of
        // the pool verifier. No sleeping or firmware-success claim is involved.
        loop {
            let header = job.reconstruct(&share).unwrap();
            let digest: [u8; 32] = Sha256::digest(Sha256::digest(header)).into();
            if job.share_target.accepts(&digest) {
                break;
            }
            share.nonce += 1;
            assert!(share.nonce < 1000);
        }
        let mut verifier = PoolVerifier::default();
        verifier.install(job, true).unwrap();
        assert!(matches!(
            verifier.verify(&share).unwrap(),
            Verdict::Accepted { .. }
        ));
        share.time = config.time + 600 + 121;
        assert_eq!(verifier.verify(&share).unwrap(), Verdict::OutOfRange);
        config.timeout = Duration::ZERO;
        assert!(make_job(&config, 0).is_err());
        config.timeout = Duration::from_secs(86_401);
        assert!(make_job(&config, 0).is_err());
        config.timeout = Duration::from_secs(600);
        config.time = u32::MAX - 600;
        assert!(make_job(&config, 0).is_err());
    }
}

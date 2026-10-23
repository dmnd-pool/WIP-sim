//! A bounded local pool behind QEMU's restricted guest TCP forwarding.
//! No listening host socket, external network, or generated mining answer.
use super::pool_input;
use anyhow::{ensure, Context, Result};
use md_sim_mining::stratum::{
    serve_command_stream, serve_command_window, CompletionPolicy, DifficultyScale, PoolReport,
    StratumConfig,
};
pub use pool_input::{prepare, prepare_window};
use serde_json::json;
use std::{
    fs::{self, File},
    io::Read,
    os::fd::FromRawFd,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

/// Observation time remains inside the selected process resource profile.
/// This changes no firmware, device, boot or thermal-protection deadline.
pub fn validate_execution_window(seconds: u64, budget: u64, nested: bool) -> Result<()> {
    let maximum = if nested { 900 } else { 300 };
    let reserve = if nested { 71 } else { 10 };
    ensure!(
        (20..=maximum).contains(&seconds)
            && seconds
                .checked_add(reserve)
                .is_some_and(|end| end <= budget),
        "pool work window exceeds policy or run budget"
    );
    Ok(())
}

use super::pool_report;
#[cfg(test)]
use pool_report::select_session;
use pool_report::{MARKER, MAX_SESSIONS};
pub const NIC: &str =
    "user,model=cadence_gem,restrict=on,ipv6=off,guestfwd=tcp:10.0.2.100:3333-cmd:/boot-runner __pool-session";

pub fn nic(dashboard: bool) -> String {
    if dashboard {
        format!("{NIC},hostfwd=tcp:127.0.0.1:18080-10.0.2.15:80")
    } else {
        NIC.into()
    }
}
const ROOT: &str = "/work/pool-sessions";
const STOP: &str = "/work/pool-stop";
const DEADLINE: &str = "/work/pool-deadline";
const MAX_REPORT_BYTES: u64 = 2 * 1024 * 1024;

#[cfg(test)]
#[test]
fn explicit_window_stays_within_the_existing_process_profile() {
    assert!(validate_execution_window(300, 360, false).is_ok());
    assert!(validate_execution_window(301, 360, false).is_err());
    assert!(validate_execution_window(300, 309, false).is_err());
    assert!(validate_execution_window(300, 310, false).is_ok());
    assert!(validate_execution_window(900, 971, true).is_ok());
    assert!(validate_execution_window(900, 970, true).is_err());
    assert!(validate_execution_window(u64::MAX, u64::MAX, true).is_err());
    assert!(validate_execution_window(19, 360, false).is_err());
}

fn boot_wait_seconds(work: u64) -> Result<u64> {
    super::confinement::nested_mining_wall_seconds()?
        .map(|wall| {
            wall.checked_sub(work + 70)
                .context("pool work window exceeds nested run budget")
        })
        .transpose()
        .map(|seconds| seconds.unwrap_or(0))
}

fn remaining_session_ns(remaining: u64, seconds: u64, boot_wait: u64) -> Result<u64> {
    let total = seconds
        .checked_add(boot_wait)
        .and_then(|s| s.checked_mul(1_000_000_000))
        .context("pool time budget overflow")?;
    ensure!(
        remaining > 0 && remaining <= total,
        "pool remaining budget invalid"
    );
    Ok(remaining.min(seconds * 1_000_000_000))
}

pub struct LocalPool {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<Collection>>>,
    deadline: u64,
    collected: Option<Collection>,
    session_limit: usize,
}

struct Collection {
    sessions: Vec<serde_json::Value>,
    report: serde_json::Value,
}

fn monotonic_ns() -> Result<u64> {
    let mut stamp = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    ensure!(
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut stamp) } == 0,
        "monotonic clock: {}",
        std::io::Error::last_os_error()
    );
    u64::try_from(stamp.tv_sec)?
        .checked_mul(1_000_000_000)
        .and_then(|ns| ns.checked_add(stamp.tv_nsec as u64))
        .context("monotonic clock overflow")
}

impl LocalPool {
    pub fn start() -> Result<Self> {
        let input = pool_input::read(Path::new("/private-pool.json"))?;
        fs::create_dir(ROOT)?;
        let boot_wait = boot_wait_seconds(input.seconds)?;
        let deadline = monotonic_ns()?
            .checked_add((input.seconds + boot_wait) * 1_000_000_000)
            .context("pool deadline overflow")?;
        fs::write(DEADLINE, deadline.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let persistent = input.completion_policy()? == CompletionPolicy::KeepOpen;
        let session_limit = input.session_limit()?;
        let thread = std::thread::spawn(move || {
            collect_sessions(signal, deadline, persistent, session_limit)
        });
        Ok(Self {
            stop,
            thread: Some(thread),
            deadline,
            collected: None,
            session_limit,
        })
    }

    /// Keep the actual miner alive until the existing collection deadline.
    /// This does not extend either the per-session work window or run budget.
    pub fn wait_mining_window(&mut self) -> Result<()> {
        let deadline = self
            .deadline
            .checked_add(5_000_000_000)
            .context("collection grace overflow")?;
        while self
            .thread
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
        {
            ensure!(
                monotonic_ns()? < deadline,
                "private pool collection deadline"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        self.join_collection()?;
        let collected = self
            .collected
            .as_ref()
            .context("private pool collection absent")?;
        pool_report::require_completed(&collected.sessions, &collected.report, self.session_limit)
    }

    fn join_collection(&mut self) -> Result<()> {
        if let Some(thread) = self.thread.take() {
            self.collected = Some(
                thread
                    .join()
                    .map_err(|_| anyhow::anyhow!("pool collector panicked"))??,
            );
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<()> {
        self.stop.store(true, Ordering::Release);
        self.join_collection()
    }
}

fn collect_sessions(
    stop: Arc<AtomicBool>,
    deadline: u64,
    persistent: bool,
    session_limit: usize,
) -> Result<Collection> {
    ensure!(
        (16..=MAX_SESSIONS).contains(&session_limit),
        "pool collection limit"
    );
    let mut emitted = vec![false; session_limit];
    let mut sessions = Vec::new();
    let mut stopping = None;
    loop {
        if stopping.is_none() && (stop.load(Ordering::Acquire) || monotonic_ns()? >= deadline) {
            // A normal persistent window ends by its own bounded timer.
            // Do not turn its final verified work into an explicit cancel.
            if stop.load(Ordering::Acquire) || !persistent {
                fs::write(STOP, b"stop\n")?;
            }
            stopping = Some(Instant::now());
        }
        for (index, emitted) in emitted.iter_mut().enumerate() {
            let path = Path::new(ROOT).join(index.to_string()).join("report.json");
            if *emitted || !path.exists() {
                continue;
            }
            ensure!(
                fs::metadata(&path)?.len() <= MAX_REPORT_BYTES,
                "pool session report bound"
            );
            let report: serde_json::Value =
                serde_json::from_reader(File::open(path)?.take(MAX_REPORT_BYTES + 1))?;
            ensure!(
                report["session_id"] == index,
                "pool session identity differs"
            );
            println!("\n{MARKER}{report}");
            sessions.push(report);
            *emitted = true;
        }
        if stopping.is_some_and(|start: Instant| start.elapsed() >= Duration::from_secs(2)) {
            fs::write(STOP, b"stop\n")?;
            let mut incomplete = Vec::new();
            for (index, emitted) in emitted.iter().enumerate() {
                if !emitted && Path::new(ROOT).join(index.to_string()).exists() {
                    incomplete.push(index);
                }
            }
            let report = json!({
                "sessions_completed":emitted.iter().filter(|done| **done).count(),
                "incomplete_sessions":incomplete,
                "max_sessions":session_limit
            });
            println!("\nMD_SIM_PRIVATE_POOL_COLLECTION {report}");
            return Ok(Collection { sessions, report });
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn allocate_session(root: &Path, limit: usize) -> Result<(usize, PathBuf)> {
    ensure!(
        (16..=MAX_SESSIONS).contains(&limit),
        "pool allocation limit"
    );
    for index in 0..limit {
        let path = root.join(index.to_string());
        match fs::create_dir(&path) {
            Ok(()) => return Ok((index, path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    anyhow::bail!("private pool connection budget exhausted")
}

/// QEMU/libslirp runs this entry point once for each guest TCP connection.
/// Protocol bytes use only inherited stdin/stdout. Reports use private files.
pub fn session() -> Result<()> {
    super::confinement::require_mining_execution()?;
    let input = pool_input::read(Path::new("/private-pool.json"))?;
    ensure!(!Path::new(STOP).exists(), "private pool has stopped");
    ensure!(fs::metadata(DEADLINE)?.len() <= 32, "pool deadline bound");
    let deadline: u64 = fs::read_to_string(DEADLINE)?.parse()?;
    let started_ns = monotonic_ns()?;
    let remaining = deadline
        .checked_sub(started_ns)
        .context("private pool deadline expired")?;
    let remaining =
        remaining_session_ns(remaining, input.seconds, boot_wait_seconds(input.seconds)?)?;
    let (index, directory) = allocate_session(Path::new(ROOT), input.session_limit()?)?;
    let seed = input.session_seed(index)?;
    let trace = directory.join("trace.jsonl");
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let watcher = std::thread::spawn(move || {
        while !signal.load(Ordering::Acquire) && !Path::new(STOP).exists() {
            std::thread::sleep(Duration::from_millis(20));
        }
        signal.store(true, Ordering::Release);
    });
    let config = StratumConfig {
        response_policy: input.response_policy()?,
        regtest_anchor: input.regtest_anchor.clone(),
        seed,
        time: input.time,
        difficulty_exponents: input.difficulty_exponents.to_vec(),
        difficulty_scale: DifficultyScale::One,
        timeout: Duration::from_nanos(remaining),
        trace_path: trace.clone(),
        limits: md_sim_mining::stratum::StratumLimits {
            max_messages: 2048,
            max_submissions: 1024,
            trace_bytes: 512 * 1024,
        },
    };
    let report = (|| {
        let reader = duplicate_fd(0)?;
        let writer = duplicate_fd(1)?;
        if input.completion_policy()? == CompletionPolicy::KeepOpen {
            serve_command_window(reader, writer, config, stop.clone())
        } else {
            serve_command_stream(reader, writer, config, stop.clone())
        }
        .map_err(anyhow::Error::msg)
    })();
    stop.store(true, Ordering::Release);
    watcher
        .join()
        .map_err(|_| anyhow::anyhow!("pool stop watcher panicked"))?;
    let report = report?;
    let mut envelope = envelope(&report, seed, input.time, index, &trace)?;
    envelope["observation_timing"] = json!({"clock":"worker CLOCK_MONOTONIC",
        "started_ns":started_ns,"finished_ns":monotonic_ns()?,
        "session_budget_ns":remaining,"collection_deadline_ns":deadline,
        "scope":"Measured support-process timing; no guest clock or mining success is inferred."});
    let bytes = serde_json::to_vec(&envelope)?;
    ensure!(bytes.len() as u64 <= MAX_REPORT_BYTES, "pool report bound");
    let temporary = directory.join("report.pending");
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, directory.join("report.json"))?;
    Ok(())
}

fn duplicate_fd(fd: i32) -> Result<File> {
    // The child owns each returned descriptor. No listener or connection
    // is opened by this adapter.
    let copy = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    ensure!(
        copy >= 0,
        "pool descriptor duplication: {}",
        std::io::Error::last_os_error()
    );
    Ok(unsafe { File::from_raw_fd(copy) })
}

fn envelope(
    report: &PoolReport,
    seed: [u8; 32],
    time: u32,
    index: usize,
    path: &Path,
) -> Result<serde_json::Value> {
    let mut trace = String::new();
    File::open(path)?
        .take(512 * 1024 + 1)
        .read_to_string(&mut trace)?;
    ensure!(trace.len() <= 512 * 1024, "pool trace exceeds bound");
    Ok(json!({
        "schema":"md-sim-s9-private-pool-session/2", "session_id":index,
        "seed_hex":hex::encode(seed),"job_time":time,
        "network":"One guest TCP connection to10.0.2.100:3333 through QEMU libslirp restrict=on and a separate command process. No host listener or external route.",
        "report":report,"trace":trace,
        "scope":"Protocol and work evidence only. The execution adaptation manifest declares the ASIC filter policy. No exact target, accepted-block or full-board gate is inferred.",
        "source":{"sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_private_pool.rs")),"size":include_bytes!("s9_private_pool.rs").len()}
    }))
}

impl Drop for LocalPool {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn waiting_keeps_the_pool_open_and_rejects_an_empty_collection() {
        use super::*;
        for authorized in [false, true] {
            let stop = Arc::new(AtomicBool::new(false));
            let observed = stop.clone();
            let producer = std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(20));
                ensure!(
                    !observed.load(Ordering::Acquire),
                    "wait stopped the active pool"
                );
                let trace = concat!(
                    "{\"direction\":\"firmware-to-pool\",\"message\":{\"method\":\"mining.authorize\",\"params\":[\"simulator\",\"x\"]}}\n",
                    "{\"direction\":\"pool-to-firmware\",\"message\":{\"method\":\"mining.notify\"}}\n"
                );
                let sessions = if authorized {
                    vec![
                        json!({"session_id":0,"trace":trace,"report":{"completed":true,
                        "stopped":false,"timed_out":false,"budget_exhausted":false,"failure":null}}),
                    ]
                } else {
                    vec![]
                };
                Ok(Collection {
                    report: json!({"sessions_completed":sessions.len(),"incomplete_sessions":[],"max_sessions":MAX_SESSIONS}),
                    sessions,
                })
            });
            let mut pool = LocalPool {
                stop: stop.clone(),
                thread: Some(producer),
                deadline: monotonic_ns().unwrap() + 1_000_000_000,
                collected: None,
                session_limit: MAX_SESSIONS,
            };
            assert_eq!(pool.wait_mining_window().is_ok(), authorized);
            assert!(!stop.load(Ordering::Acquire));
            pool.finish().unwrap();
            assert!(stop.load(Ordering::Acquire));
        }
    }

    #[test]
    fn boot_allowance_does_not_extend_a_session_work_deadline() {
        assert_eq!(
            super::remaining_session_ns(800_000_000_000, 230, 900).unwrap(),
            230_000_000_000
        );
        assert_eq!(
            super::remaining_session_ns(100_000_000_000, 230, 0).unwrap(),
            100_000_000_000
        );
        assert!(super::remaining_session_ns(0, 230, 900).is_err());
        assert!(super::remaining_session_ns(1_131_000_000_000, 230, 900).is_err());
        assert!(super::remaining_session_ns(1, u64::MAX, 900).is_err());
        assert_eq!(
            super::remaining_session_ns(1_000_000_000_000, 600, 1730).unwrap(),
            600_000_000_000
        );
        assert_eq!(
            super::remaining_session_ns(100_000_000_000, 600, 1730).unwrap(),
            100_000_000_000
        );
    }
    use super::*;

    #[test]
    fn frozen_pool_input_records_and_checks_both_difficulty_changes() {
        let frozen: pool_input::PoolInput =
            serde_json::from_value(prepare(230, [-28, -27], None).unwrap()).unwrap();
        assert_eq!(frozen.schema, "md-sim-s9-pool-input/7");
        assert_eq!(frozen.difficulty_exponents, [-28, -27]);
        assert_eq!(frozen.seconds, 230);
        for pair in [[-32, -30], [-30, 1], [-20, -20]] {
            assert!(prepare(230, pair, None).is_err());
        }
        let extended: pool_input::PoolInput =
            serde_json::from_value(prepare(600, [-28, -27], None).unwrap()).unwrap();
        assert_eq!(extended.seconds, 600);
        for seconds in [19, 901, u64::MAX] {
            assert!(prepare(seconds, [-28, -27], None).is_err());
        }
    }

    #[test]
    fn connection_slots_are_unique_and_bounded() {
        let root = tempfile::tempdir().unwrap();
        for index in 0..MAX_SESSIONS {
            let (actual, directory) = allocate_session(root.path(), MAX_SESSIONS).unwrap();
            assert_eq!(actual, index);
            assert_eq!(directory, root.path().join(index.to_string()));
        }
        assert!(allocate_session(root.path(), MAX_SESSIONS).is_err());
        assert!(allocate_session(root.path(), 16).is_err());
        let legacy = tempfile::tempdir().unwrap();
        for index in 0..16 {
            assert_eq!(allocate_session(legacy.path(), 16).unwrap().0, index);
        }
        assert!(allocate_session(legacy.path(), 16).is_err());
        assert_eq!(allocate_session(legacy.path(), 128).unwrap().0, 16);
    }

    #[test]
    fn primary_selection_preserves_first_authorized_failure_and_rejects_duplicate_ids() {
        let trace = format!(
            "{}\n{}\n",
            json!({"direction":"firmware-to-pool","message":{
                "method":"mining.authorize","params":["simulator","x"]}}),
            json!({"direction":"pool-to-firmware","message":{"method":"mining.notify"}})
        );
        let failed = json!({"session_id":1,"trace":trace,"report":{"completed":false}});
        let passed = json!({"session_id":2,"trace":trace,"report":{"completed":true}});
        let probe = json!({"session_id":0,"trace":"","report":{"completed":false}});
        assert_eq!(
            select_session(&[passed, failed.clone(), probe]).unwrap(),
            Some(failed.clone())
        );
        assert!(select_session(&[failed.clone(), failed]).is_err());
    }
}

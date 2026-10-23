//! Convert coordinator signals into the existing bounded cancellation path.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::isolation::save_json;
use serde_json::json;
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        mpsc, Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

static SIGNAL: AtomicI32 = AtomicI32::new(0);
extern "C" fn request(signal: libc::c_int) {
    let _ = SIGNAL.compare_exchange(0, signal, Ordering::SeqCst, Ordering::SeqCst);
}

pub struct Cancellation {
    pub requested: Arc<AtomicBool>,
    stop: Option<mpsc::Sender<()>>,
    watcher: Option<JoinHandle<Result<()>>>,
}
impl Cancellation {
    pub fn start(out: &Path) -> Result<Self> {
        for signal in [libc::SIGINT, libc::SIGTERM] {
            ensure!(
                unsafe { libc::signal(signal, request as *const () as libc::sighandler_t) }
                    != libc::SIG_ERR,
                "cannot install cancellation signal handler"
            );
        }
        let requested = Arc::new(AtomicBool::new(false));
        let target = requested.clone();
        let record = out.join("cancellation-request.json");
        let (stop, receive) = mpsc::channel();
        let started = Instant::now();
        let watcher = thread::spawn(move || {
            loop {
                let signal = SIGNAL.load(Ordering::SeqCst);
                if signal != 0 {
                    // Cancellation must still reach the runtime if recording fails.
                    target.store(true, Ordering::Release);
                    save_json(
                        &record,
                        &json!({"schema":"md-sim-coordinator-signal-cancellation/1",
                        "signal":signal,"observed_elapsed_milliseconds":started.elapsed().as_millis(),
                        "scope":"Signal delivered to this coordinator. Actual process status and worker result establish whether cancellation stopped execution."}),
                    )?;
                    return Ok(());
                }
                match receive.recv_timeout(Duration::from_millis(50)) {
                    Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                    Err(mpsc::RecvTimeoutError::Timeout) => (),
                }
            }
        });
        Ok(Self {
            requested,
            stop: Some(stop),
            watcher: Some(watcher),
        })
    }
    pub fn finish(mut self) -> Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        self.watcher
            .take()
            .context("cancellation watcher absent")?
            .join()
            .map_err(|_| anyhow::anyhow!("cancellation watcher panicked"))?
    }
}
impl Drop for Cancellation {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_signal_records_request_and_sets_runtime_cancel_flag() {
        const CHILD: &str = "MD_SIM_SIGNAL_CANCELLATION_TEST_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "cancellation::tests::actual_signal_records_request_and_sets_runtime_cancel_flag"])
                .env(CHILD, "1").output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"),
                "child did not execute its selected signal test"
            );
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let cancellation = Cancellation::start(root.path()).unwrap();
        assert!(!cancellation.requested.load(Ordering::Acquire));
        assert_eq!(unsafe { libc::kill(libc::getpid(), libc::SIGTERM) }, 0);
        let deadline = Instant::now() + Duration::from_secs(2);
        while !cancellation.requested.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(cancellation.requested.load(Ordering::Acquire));
        cancellation.finish().unwrap();
        let record: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.path().join("cancellation-request.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(record["signal"], libc::SIGTERM);
    }
}

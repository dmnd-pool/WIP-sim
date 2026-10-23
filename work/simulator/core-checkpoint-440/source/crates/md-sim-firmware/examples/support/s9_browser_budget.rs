//! Separate support-browser startup from the miner's existing execution bounds.
use anyhow::{ensure, Result};
use std::{sync::mpsc, thread::JoinHandle, time::Duration};

pub const STARTUP_SECONDS: u64 = 330;
pub const SUPPORT_CPU_SECONDS: u64 = 600;
// Original056's four-CPU support QEMU could not mmap its captured initramfs
// under3GiB. This virtual-address allowance does not increase guest RAM.
pub const SUPPORT_ADDRESS_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub const MINER_ADDRESS_BYTES: u64 = 3 * 1024 * 1024 * 1024;
// The two-profile suite includes an independent same-origin login rejection
// and a final authenticated-session check. Run051 reached its old260s
// support deadline during those extra browser operations. This allowance
// stays inside the existing360s miner wall bound and never changes330s CPU.
pub const COMPLETION_SECONDS: u64 = 300;
pub const PACKET_BYTES: usize = 16 * 1024 * 1024;

pub struct MiningDeadline {
    stop: mpsc::Sender<()>,
    thread: Option<JoinHandle<()>>,
}
impl MiningDeadline {
    pub fn start(seconds: u64, cpu_seconds: u64) -> Result<Self> {
        ensure!(
            (30..=360).contains(&seconds) && cpu_seconds <= 330,
            "mining bounds differ"
        );
        // The support VM is already running with its own larger CPU limit.
        // Reduce this supervisor and its future miner child to the original
        // CPU and address limits. Existing support children keep their bounds.
        for (resource, bound) in [
            (libc::RLIMIT_CPU, cpu_seconds),
            (libc::RLIMIT_AS, MINER_ADDRESS_BYTES),
        ] {
            let limit = libc::rlimit {
                rlim_cur: bound,
                rlim_max: bound,
            };
            ensure!(
                unsafe { libc::setrlimit(resource, &limit) } == 0,
                "mining resource bound failed: {resource}"
            );
        }
        let (stop, receiver) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            if receiver.recv_timeout(Duration::from_secs(seconds))
                == Err(mpsc::RecvTimeoutError::Timeout)
            {
                eprintln!("MD_SIM_MINER_WALL_DEADLINE: {seconds}");
                // Only this run-owned supervisor is signalled. Its live
                // child emulators use parent-death SIGKILL. There is no PID
                // reuse race: this thread is still part of the target process.
                unsafe {
                    libc::kill(libc::getpid(), libc::SIGKILL);
                }
            }
        });
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}
impl Drop for MiningDeadline {
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

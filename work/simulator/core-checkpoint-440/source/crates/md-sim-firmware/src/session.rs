//! A bounded source-runtime experiment. It must run inside the worker sandbox.
use crate::bitmain_s2::Controller;
use anyhow::{bail, ensure, Context, Result};
use md_sim_mining::engine::RateLimiter;
use serde_json::json;
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::process::CommandExt,
    },
    path::Path,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

struct ChildGuard(std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The caller must provide run-owned output and inherited private pool IPC.
/// Refuse broad host mounts, physical networks and missing resource limits.
pub fn run(
    firmware: &Path,
    pool_stream: &File,
    out: &Path,
    duration: Duration,
    cap_hps: f64,
    stop: Arc<AtomicBool>,
) -> Result<serde_json::Value> {
    crate::confinement::require(false)?;
    ensure!(
        duration > Duration::ZERO && duration <= Duration::from_secs(86_390),
        "invalid session budget"
    );
    fs::create_dir_all(out)?;
    let mut traces = File::create(out.join("device-transactions.jsonl"))?;
    let mut trace_bytes = 0usize;
    let trace_limit = 20 * 1024 * 1024;
    let mut master = -1;
    let mut slave = -1;
    let mut name = [0i8; 128];
    // SAFETY: valid output pointers, no optional termios or window dimensions.
    ensure!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                name.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
            )
        } == 0,
        "openpty: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: ownership transfers exactly once from openpty's fresh descriptors.
    let mut master = unsafe { File::from_raw_fd(master) };
    let slave = unsafe { File::from_raw_fd(slave) };
    for fd in [master.as_raw_fd(), slave.as_raw_fd()] {
        ensure!(
            unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } == 0,
            "set PTY close-on-exec failed"
        );
    }
    let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: initialized by successful tcgetattr; descriptor remains live.
    ensure!(
        unsafe { libc::tcgetattr(slave.as_raw_fd(), termios.as_mut_ptr()) } == 0,
        "tcgetattr failed"
    );
    let mut termios = unsafe { termios.assume_init() };
    unsafe {
        libc::cfmakeraw(&mut termios);
    }
    ensure!(
        unsafe { libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &termios) } == 0,
        "tcsetattr failed"
    );
    let flags = unsafe { libc::fcntl(master.as_raw_fd(), libc::F_GETFL) };
    ensure!(
        flags >= 0
            && unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) }
                == 0,
        "set PTY nonblocking failed"
    );
    let slave_name = unsafe { std::ffi::CStr::from_ptr(name.as_ptr()) }.to_str()?;
    let log = File::create(out.join("guest.log"))?;
    let mut command = Command::new(firmware);
    let pool_fd = pool_stream.as_raw_fd();
    // SAFETY: fcntl is async-signal-safe. Only the selected IPC descriptor is
    // inherited by the firmware. Other adapter descriptors have CLOEXEC set.
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(pool_fd, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = ChildGuard(
        command
            .args([
                "--text-only",
                "-o",
                "stratum+tcp://127.0.0.1:3333",
                "-u",
                "simulator",
                "-p",
                "x",
                "--bitmain-options",
                "115200:8:32",
                "--bitmain-dev",
                slave_name,
                "--queue",
                "1",
                "--scan-time",
                "5",
                "--expiry",
                "30",
                "--log",
                "1",
            ])
            .env_clear()
            .env("PATH", "/usr/bin")
            .env("HOME", "/tmp")
            .env("MD_SIM_STRATUM_FD", pool_fd.to_string())
            .env("MD_SIM_LOW_RATE", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log))
            .spawn()
            .context("start source-adapted cgminer")?,
    );
    let mut controller = Controller::new(vec![32; 8], true)?;
    let mut limiter = RateLimiter::new(cap_hps).map_err(anyhow::Error::msg)?;
    let start = Instant::now();
    let mut seq = 0u64;
    let mut record = |direction: &str, bytes: &[u8], elapsed: Duration| -> Result<()> {
        let line = serde_json::to_vec(
            &json!({"sequence":seq,"clock":"wall-monotonic","nanoseconds":elapsed.as_nanos(),
            "component":"s2-controller-stream","direction":direction,"raw_hex":hex::encode(bytes),"parser":"md-sim-s2-v1"}),
        )?;
        trace_bytes += line.len() + 1;
        ensure!(trace_bytes <= trace_limit, "trace size budget exceeded");
        traces.write_all(&line)?;
        traces.write_all(b"\n")?;
        seq += 1;
        Ok(())
    };
    let mut bytes = [0u8; 8192];
    let mut exit_status = None;
    let execution = (|| -> Result<()> {
        while start.elapsed() < duration && !stop.load(Ordering::Relaxed) {
            if let Some(status) = child.0.try_wait()? {
                exit_status = Some(status.to_string());
                ensure!(status.success(), "firmware exited with {status}");
                break;
            }
            match master.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => {
                    record("firmware-to-device", &bytes[..count], start.elapsed())?;
                    for response in controller.receive(&bytes[..count])? {
                        record("device-to-firmware", &response, start.elapsed())?;
                        master.write_all(&response)?;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error.into()),
            }
            if !controller.queue.is_empty() && limiter.try_acquire() {
                if let Some(response) = controller.step() {
                    record("device-to-firmware", &response, start.elapsed())?;
                    master.write_all(&response)?;
                }
            }
            let wait = limiter.wait_duration();
            if wait >= Duration::from_millis(1) {
                let mut fd = libc::pollfd {
                    fd: master.as_raw_fd(),
                    events: libc::POLLIN,
                    revents: 0,
                };
                let result = unsafe { libc::poll(&mut fd, 1, wait.as_millis().min(5) as i32) };
                ensure!(
                    result >= 0
                        || std::io::Error::last_os_error().kind()
                            == std::io::ErrorKind::Interrupted,
                    "serial poll failed"
                );
            } else {
                std::thread::sleep(wait.min(Duration::from_micros(100)));
            }
        }
        Ok(())
    })();
    // ChildGuard kills and waits only this run-owned process.
    drop(child);
    let report = json!({"execution_mode":"source-adapter","firmware_identity":"rebuilt-source","boot_entry":"process",
        "hardware_identity":"source-derived-controller-interface-fixture-not-an-exact-board",
        "chains":8,"chips_per_chain":32,"configured":controller.configured,"resets":controller.resets,
        "work_generations":controller.generations,"computed_hashes":controller.computed_hashes,"device_results":controller.results,
        "hash_cap_hps":cap_hps,"measured_hps":controller.computed_hashes as f64/start.elapsed().as_secs_f64(),
        "crc_errors":controller.crc_errors,"trace_truncated":trace_bytes > trace_limit,"guest_exit":exit_status,
        "execution_error":execution.as_ref().err().map(ToString::to_string),
        "gate_limits":["No original board boot", "No downstream FPGA or ASIC chain bus model", "No exact board target", "No calibrated cooling or power model", "No TCP, SSH, TLS, or native dashboard test"]});
    fs::write(
        out.join("session.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    execution?;
    if !controller.configured {
        bail!("firmware did not configure the controller; see guest.log");
    }
    Ok(report)
}

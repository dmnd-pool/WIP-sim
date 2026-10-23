//! Bounded serial access to a QEMU guest inside the board sandbox.
//! Deadlines and output limits are enforced by the enclosing process runner.
use anyhow::{ensure, Context, Result};
use serde_json::json;
use std::{
    fs::File,
    io::{Read, Write},
    os::fd::{AsFd, AsRawFd},
    os::unix::process::ExitStatusExt,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};
const DONE: &[u8] = b"\r\nMD_SIM_SERIAL_SCRIPT_FINISHED:";
const RESTART: &[u8] = b"reboot: Restarting system";
const RESTART_TRACE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GuestRestart {
    pub emulator_exit_code: i32,
    pub serial: md_sim_runtime::store::ObjectId,
}

impl GuestRestart {
    /// Check the retained bytes separately from the child-process observation.
    pub fn verify_serial(&self, bytes: &[u8]) -> Result<()> {
        ensure!(
            self.emulator_exit_code == 0,
            "emulator did not exit successfully"
        );
        ensure!(
            bytes.len() <= RESTART_TRACE_BYTES
                && bytes.len() as u64 == self.serial.size
                && crate::prepare::hash_bytes(bytes) == self.serial.sha256
                && bytes
                    .windows(RESTART.len())
                    .filter(|s| *s == RESTART)
                    .count()
                    == 1,
            "guest restart serial evidence differs"
        );
        Ok(())
    }
}

/// An intentional emulator interruption. This is not a guest shutdown or a
/// claim about physical power loss. Recovery needs a separate boot and check.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GuestInterruption {
    pub emulator_signal: i32,
    pub serial: md_sim_runtime::store::ObjectId,
}

impl GuestInterruption {
    pub fn verify_serial(&self, bytes: &[u8]) -> Result<()> {
        ensure!(
            self.emulator_signal == libc::SIGKILL
                && bytes.len() <= RESTART_TRACE_BYTES
                && bytes.len() as u64 == self.serial.size
                && crate::prepare::hash_bytes(bytes) == self.serial.sha256
                && !bytes.windows(RESTART.len()).any(|s| s == RESTART),
            "emulator interruption evidence differs or contains native restart"
        );
        Ok(())
    }
}

pub struct SerialGuest {
    child: std::process::Child,
    input: Option<std::process::ChildStdin>,
    output: std::process::ChildStdout,
    window: Vec<u8>,
    started: Instant,
    usage_before: Option<ChildUsage>,
    reaped: Option<(std::process::ExitStatus, Option<ChildUsage>)>,
}

impl Drop for SerialGuest {
    fn drop(&mut self) {
        if matches!(self.reap(false), Ok(None)) {
            let _ = self.child.kill();
        }
        if let Ok(Some(status)) = self.reap(true) {
            eprintln!("serial-driver: emulator_exit={status}");
        }
        let after = child_usage();
        eprintln!(
            "serial-driver: emulator_resources={}",
            json!({
                "wall_milliseconds":self.started.elapsed().as_millis(),
                "exact_waited_emulator_usage":self.reaped.as_ref().and_then(|(_, usage)|usage.as_ref()),
                "exact_usage_scope":"wait4 for the owned emulator PID only; includes descendants reaped by that emulator, not sibling support processes",
                "waited_child_usage_before":self.usage_before,
                "waited_child_usage_after":after,
                "cpu_user_microseconds":self.usage_before.as_ref().zip(after.as_ref())
                    .and_then(|(a,b)|b.user_microseconds.checked_sub(a.user_microseconds)),
                "cpu_system_microseconds":self.usage_before.as_ref().zip(after.as_ref())
                    .and_then(|(a,b)|b.system_microseconds.checked_sub(a.system_microseconds)),
                "scope":"Cumulative waited-child difference for this supervisor. Completed support children can contribute; this is not per-emulator CPU accounting. RSS fields are cumulative waited-child maxima, not their difference."
            })
        );
    }
}

impl SerialGuest {
    /// Reap this child exactly once and retain its own CPU accounting. The
    /// standard Child must not reap it separately, since wait4 consumes both
    /// the exit status and the resource record. A cached exit is never killed.
    fn reap(&mut self, blocking: bool) -> std::io::Result<Option<std::process::ExitStatus>> {
        if let Some((status, _)) = self.reaped {
            return Ok(Some(status));
        }
        loop {
            let mut status = 0;
            let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
            // The PID belongs to this Child. Both output pointers are valid.
            // Read usage only if wait4 returns that same reaped child PID.
            let pid = unsafe {
                libc::wait4(
                    self.child.id() as libc::pid_t,
                    &mut status,
                    if blocking { 0 } else { libc::WNOHANG },
                    usage.as_mut_ptr(),
                )
            };
            if pid == 0 {
                return Ok(None);
            }
            if pid < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            let status = std::process::ExitStatus::from_raw(status);
            let usage = unsafe { usage.assume_init() };
            self.reaped = Some((status, usage_values(usage)));
            return Ok(Some(status));
        }
    }

    pub fn start(command: Command, activate_console: bool) -> Result<Self> {
        crate::confinement::require_board()?;
        Self::start_confined(command, activate_console, || {})
    }

    /// Same serial path with the explicit original-boot/mining CPU budget.
    pub fn start_mining(command: Command, activate_console: bool) -> Result<Self> {
        crate::confinement::require_mining_execution()?;
        Self::start_confined(command, activate_console, || {})
    }

    /// Close parent-owned child endpoints immediately after spawn, before any
    /// blocking console observation. This callback does not prove guest boot.
    pub fn start_mining_with_spawned(
        command: Command,
        activate_console: bool,
        after_spawn: impl FnOnce(),
    ) -> Result<Self> {
        crate::confinement::require_mining_execution()?;
        Self::start_confined(command, activate_console, after_spawn)
    }

    fn start_confined(
        mut command: Command,
        activate_console: bool,
        after_spawn: impl FnOnce(),
    ) -> Result<Self> {
        let started = Instant::now();
        let usage_before = child_usage();
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let input = child.stdin.take().context("missing QEMU stdin")?;
        let output = child.stdout.take().context("missing QEMU stdout")?;
        let mut guest = Self {
            child,
            input: Some(input),
            output,
            window: Vec::new(),
            started,
            usage_before,
            reaped: None,
        };
        after_spawn();
        if activate_console {
            guest.activate_console()?;
        } else {
            guest.wait_prompt(false)?;
        }
        Ok(guest)
    }

    fn activate_console(&mut self) -> Result<()> {
        const ACTIVATE: &[u8] = b"Please press Enter to activate this console.";
        let mut buffer = [0; 4096];
        loop {
            let count = self.output.read(&mut buffer)?;
            ensure!(count > 0, "guest serial closed before console activation");
            std::io::stdout().write_all(&buffer[..count])?;
            std::io::stdout().flush()?;
            self.window.extend_from_slice(&buffer[..count]);
            if self.window.windows(ACTIVATE.len()).any(|w| w == ACTIVATE) {
                break;
            }
            if self.window.len() > 8192 {
                self.window.drain(..self.window.len() - 8192);
            }
        }
        // Send the activation newline separately. A getty may consume the
        // first line before the shell exists; it must not consume the opening
        // brace of our script group.
        self.window.clear();
        let input = self.input.as_mut().context("serial input absent")?;
        input.write_all(b"\n")?;
        input.flush()?;
        self.wait_prompt(false)?;
        // The guest must execute a command before the caller's script can run.
        // A login prompt or a stopped init cannot satisfy the output sentinel.
        self.script(b"true\n".to_vec())
    }

    fn wait_prompt(&mut self, require_reset: bool) -> Result<()> {
        let mut reset = !require_reset;
        let mut buffer = [0; 4096];
        loop {
            let count = self.output.read(&mut buffer)?;
            ensure!(count > 0, "guest serial closed before shell prompt");
            std::io::stdout().write_all(&buffer[..count])?;
            std::io::stdout().flush()?;
            self.window.extend_from_slice(&buffer[..count]);
            if prompt_after_reset(&mut self.window, &mut reset) {
                self.window.clear();
                return Ok(());
            }
            if self.window.len() > 8192 {
                self.window.drain(..self.window.len() - 8192);
            }
        }
    }

    pub fn script(&mut self, script: Vec<u8>) -> Result<()> {
        ensure!(
            script.len() <= 65536 && !script.contains(&0),
            "serial script exceeds policy"
        );
        let script = script_frame(&script);
        let mut input = self.input.take().context("serial sender still active")?;
        self.window.clear();
        eprintln!(
            "serial-driver: send {} script bytes; wait for guest completion",
            script.len()
        );
        let writer = thread::spawn(move || -> std::io::Result<std::process::ChildStdin> {
            for chunk in script.chunks(32) {
                input.write_all(chunk)?;
                input.flush()?;
                thread::sleep(Duration::from_millis(40));
            }
            Ok(input)
        });
        let status = read_until_done(&mut self.output, &mut self.window);
        if status.as_ref().ok().copied().flatten() != Some(0) {
            // Record whether QEMU already exited before our failure cleanup.
            // A later SIGKILL can otherwise be mistaken for an external kill.
            let observation_start = Instant::now();
            let mut before_cleanup = self.reap(false);
            // A killed multithreaded backend can close stdout before its
            // parent can reap it. Give that exit a short observation window
            // before our own SIGKILL makes the cause ambiguous.
            while matches!(before_cleanup, Ok(None))
                && observation_start.elapsed() < Duration::from_millis(50)
            {
                thread::sleep(Duration::from_millis(1));
                before_cleanup = self.reap(false);
            }
            let pid = self.child.id();
            let observed = matches!(before_cleanup, Ok(None)).then(|| ["stat", "status", "limits"].into_iter().map(|name| {
                let value = std::fs::read_to_string(format!("/proc/{pid}/{name}"));
                (name, json!({"text":value.as_ref().ok(),"error":value.as_ref().err().map(ToString::to_string)}))
            }).collect::<std::collections::BTreeMap<_, _>>());
            eprintln!(
                "serial-driver: before_failure_cleanup={}",
                json!({
                "script_status":status.as_ref().ok(),
                "serial_eof":status.as_ref().is_ok_and(Option::is_none),
                "exit_observation_milliseconds":observation_start.elapsed().as_millis(),
                "read_error":status.as_ref().err().map(|e|format!("{e:#}")),
                "emulator_status":before_cleanup.as_ref().ok().and_then(|status|status.as_ref()).map(ToString::to_string),
                "wait_error":before_cleanup.as_ref().err().map(ToString::to_string),
                "owned_emulator_proc":observed})
            );
            if matches!(before_cleanup, Ok(None)) {
                let _ = self.child.kill();
            }
        }
        let input = writer
            .join()
            .map_err(|_| anyhow::anyhow!("serial sender panicked"))?;
        let status = status?.context("guest script did not finish")?;
        eprintln!("serial-driver: script_status={status}");
        ensure!(status == 0, "guest script failed");
        self.input = Some(input?);
        Ok(())
    }

    /// Use the archived shell without its interactive line editor. This is
    /// an explicit console-environment adaptation, not a new guest service.
    /// The following command checks the actual shell flag before proceeding.
    pub fn enter_batch_shell(&mut self) -> Result<()> {
        let input = self.input.as_mut().context("serial input absent")?;
        input.write_all(b"exec /bin/sh +i\n")?;
        input.flush()?;
        self.script(b"case $- in *i*) false;; *) true;; esac\n".to_vec())
    }

    pub fn reboot(&mut self) -> Result<()> {
        let pid = self.child.id();
        self.window.clear();
        let input = self.input.as_mut().context("serial input absent")?;
        input.write_all(b"/sbin/reboot -f\n")?;
        input.flush()?;
        self.wait_prompt(true)?;
        ensure!(self.reap(false)?.is_none(), "emulator exited during reset");
        println!(
            "\nMD_SIM_STORAGE_WARM_RESET {}",
            json!({
                "command":"/sbin/reboot -f","emulator_pid_before":pid,
                "emulator_pid_after":self.child.id(),"kernel_restart_and_new_shell_observed":true
            })
        );
        Ok(())
    }

    /// Request the guest's normal service shutdown, then observe its kernel
    /// restart and actual exit under QEMU -no-reboot. This does not force reset
    /// or assert that the filesystem is clean; inspect the saved disk as well.
    pub fn reboot_and_exit(&mut self, seconds: u64) -> Result<GuestRestart> {
        const COMMAND: &[u8] = b"/sbin/reboot\n";
        let mut input = File::from(
            self.input
                .as_ref()
                .context("serial input absent")?
                .as_fd()
                .try_clone_to_owned()?,
        );
        let ((), observation) = self.wait_for_restart_exit(seconds, || {
            input.write_all(COMMAND)?;
            input.flush()?;
            println!("\nMD_SIM_GUEST_RESTART_REQUEST {}", json!({
                "command":"/sbin/reboot", "serial_bytes":COMMAND.len(),
                "serial_sha256":crate::prepare::hash_bytes(COMMAND),
                "scope":"Request written to the guest serial shell. Completion requires the separate native restart and emulator-exit observation."
            }));
            Ok(())
        })?;
        Ok(observation)
    }

    /// Stop only this owned live emulator. Do not send sync or reboot to the
    /// guest. Drain the real serial pipe to EOF and require an actual SIGKILL
    /// status before permitting the caller to retain interrupted storage.
    pub fn interrupt_and_exit(&mut self, seconds: u64) -> Result<GuestInterruption> {
        ensure!(
            (1..=30).contains(&seconds),
            "interruption observation bound"
        );
        ensure!(
            self.reap(false)?.is_none(),
            "emulator already exited before interruption"
        );
        let output = File::from(self.output.as_fd().try_clone_to_owned()?);
        self.window.clear();
        self.child.kill()?;
        let bytes = read_until_exit(
            output,
            std::io::stdout(),
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(seconds),
        )?;
        let status = self.reap_after_eof()?;
        let observation = GuestInterruption {
            emulator_signal: status
                .signal()
                .context("interruption did not terminate by signal")?,
            serial: md_sim_runtime::store::ObjectId {
                sha256: crate::prepare::hash_bytes(&bytes),
                size: bytes.len() as u64,
            },
        };
        observation.verify_serial(&bytes)?;
        println!(
            "\nMD_SIM_GUEST_INTERRUPTION {}",
            json!({"observation":observation,
            "serial_hex":hex::encode(&bytes),"observation_seconds":seconds,
            "scope":"Actual SIGKILL of this owned emulator, complete serial EOF and reaped status. No guest sync or reboot was requested. This is software interruption; physical power-loss behavior and storage recovery are not established."})
        );
        Ok(observation)
    }

    /// Retain guest serial output while a separate client checks services.
    /// Leaving this pipe unread can block QEMU when the guest writes logs.
    /// Stop the reader before another console command can consume its bytes.
    pub fn drain_while<T>(&mut self, wait: impl FnOnce() -> Result<T>) -> Result<T> {
        let output = File::from(self.output.as_fd().try_clone_to_owned()?);
        self.window.clear();
        let stop = AtomicBool::new(false);
        thread::scope(|scope| {
            let reader = scope.spawn(|| drain_output(output, std::io::stdout(), &stop));
            let result = {
                // Also release the reader if the waiting code unwinds.
                let _stop_on_drop = StopReader(&stop);
                wait()
            };
            let drained = reader
                .join()
                .map_err(|_| anyhow::anyhow!("serial output reader panicked"))?;
            drained?;
            result
        })
    }

    /// Observe a native Linux restart with QEMU's -no-reboot policy. The
    /// caller must also verify the request that initiated the restart.
    /// A banner, an EOF, or exit0 alone is insufficient. No kill is used on
    /// the successful path. The outer worker still enforces its total budget.
    pub fn wait_for_restart_exit<T>(
        &mut self,
        seconds: u64,
        wait: impl FnOnce() -> Result<T>,
    ) -> Result<(T, GuestRestart)> {
        ensure!((1..=120).contains(&seconds), "restart observation bound");
        let output = File::from(self.output.as_fd().try_clone_to_owned()?);
        self.window.clear();
        let stop = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(seconds);
        let (result, bytes) = thread::scope(|scope| -> Result<_> {
            let reader =
                scope.spawn(|| read_until_exit(output, std::io::stdout(), &stop, deadline));
            let _stop_on_unwind = StopReader(&stop);
            let result = wait();
            if result.is_err() {
                stop.store(true, Ordering::Release);
            }
            let bytes = reader
                .join()
                .map_err(|_| anyhow::anyhow!("restart serial reader panicked"))?;
            Ok((result?, bytes?))
        })?;
        let status = self.reap_after_eof()?;
        ensure!(
            status.success(),
            "emulator failed during native restart: {status}"
        );
        let observation = GuestRestart {
            emulator_exit_code: status.code().context("emulator exited by signal")?,
            serial: md_sim_runtime::store::ObjectId {
                sha256: crate::prepare::hash_bytes(&bytes),
                size: bytes.len() as u64,
            },
        };
        observation.verify_serial(&bytes)?;
        println!(
            "\nMD_SIM_GUEST_RESTART {}",
            json!({"observation":observation,
            "serial_hex":hex::encode(&bytes),"observation_seconds":seconds,
            "scope":"Native kernel restart text, complete serial EOF and actual emulator exit0 under -no-reboot. No new boot or update correctness is inferred."})
        );
        Ok((result, observation))
    }

    fn reap_after_eof(&mut self) -> Result<std::process::ExitStatus> {
        // EOF can precede reaping a multithreaded emulator. A process that
        // merely closes its serial descriptor cannot pass an exit check.
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = self.reap(false)? {
                return Ok(status);
            }
            ensure!(
                Instant::now() < deadline,
                "emulator remained alive after serial EOF"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
}

fn read_until_exit(
    mut input: File,
    mut output: impl Write,
    stop: &AtomicBool,
    deadline: Instant,
) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    loop {
        ensure!(
            !stop.load(Ordering::Acquire),
            "exit request verification failed"
        );
        ensure!(Instant::now() < deadline, "serial EOF observation deadline");
        let mut buffer = [0; 4096];
        let Some(count) = polled_read(&mut input, &mut buffer)? else {
            continue;
        };
        if count == 0 {
            return Ok(bytes);
        }
        ensure!(
            bytes.len() + count <= RESTART_TRACE_BYTES,
            "exit serial evidence bound"
        );
        output.write_all(&buffer[..count])?;
        output.flush()?;
        bytes.extend_from_slice(&buffer[..count]);
    }
}

struct StopReader<'a>(&'a AtomicBool);
impl Drop for StopReader<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

fn drain_output(mut input: File, mut output: impl Write, stop: &AtomicBool) -> Result<()> {
    let mut buffer = [0; 4096];
    while !stop.load(Ordering::Acquire) {
        let Some(count) = polled_read(&mut input, &mut buffer)? else {
            continue;
        };
        ensure!(
            count != 0,
            "guest serial closed during service verification"
        );
        output.write_all(&buffer[..count])?;
        output.flush()?;
    }
    Ok(())
}

fn polled_read(input: &mut File, buffer: &mut [u8]) -> Result<Option<usize>> {
    let mut ready = libc::pollfd {
        fd: input.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // The owned descriptor and initialized poll structure stay live.
    let result = unsafe { libc::poll(&mut ready, 1, 20) };
    if result < 0 {
        let error = std::io::Error::last_os_error();
        if error.kind() == std::io::ErrorKind::Interrupted {
            return Ok(None);
        }
        return Err(error.into());
    }
    if result == 0 {
        return Ok(None);
    }
    ensure!(
        ready.revents & (libc::POLLERR | libc::POLLNVAL) == 0,
        "serial output descriptor failed"
    );
    Ok(Some(input.read(buffer)?))
}

#[derive(serde::Serialize)]
struct ChildUsage {
    user_microseconds: u64,
    system_microseconds: u64,
    maximum_waited_child_rss_kib: u64,
}

fn cpu_microseconds(time: libc::timeval) -> Option<u64> {
    let seconds = u64::try_from(time.tv_sec).ok()?;
    let micros = u64::try_from(time.tv_usec).ok()?;
    if micros >= 1_000_000 {
        return None;
    }
    seconds.checked_mul(1_000_000)?.checked_add(micros)
}

fn child_usage() -> Option<ChildUsage> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // Linux writes the complete structure on success. No uninitialized
    // field is read when the syscall fails.
    if unsafe { libc::getrusage(libc::RUSAGE_CHILDREN, usage.as_mut_ptr()) } != 0 {
        return None;
    }
    let usage = unsafe { usage.assume_init() };
    usage_values(usage)
}

fn usage_values(usage: libc::rusage) -> Option<ChildUsage> {
    Some(ChildUsage {
        user_microseconds: cpu_microseconds(usage.ru_utime)?,
        system_microseconds: cpu_microseconds(usage.ru_stime)?,
        maximum_waited_child_rss_kib: u64::try_from(usage.ru_maxrss).ok()?,
    })
}

fn script_frame(script: &[u8]) -> Vec<u8> {
    use std::fmt::Write as _;
    // The shell must receive the completion command before it starts the
    // script. Otherwise a sleep or busy command can stop console reads while
    // the sender is still delivering the suffix, overflowing a guest UART.
    // POSIX compound-command parsing buffers the whole group before execution.
    //
    // Interactive ash consumes literal tabs as completion keys, even in a
    // quoted argument (observed in original S9 run019). Send only printable
    // ASCII lines and reconstruct the bytes with POSIX printf inside the
    // guest. A trailing sentinel preserves all final newlines through command
    // substitution. No external decoder or temporary guest file is required.
    let mut frame = String::from("{\n__md_sim_serial_payload=$(printf '%b'");
    for chunk in script.chunks(96) {
        frame.push_str(" \\\n'");
        for &byte in chunk {
            match byte {
                b'\\' => frame.push_str("\\\\"),
                b'\'' => frame.push_str("'\\''"),
                b' '..=b'~' => frame.push(char::from(byte)),
                _ => write!(frame, "\\0{byte:03o}").expect("writing a String"),
            }
        }
        frame.push('\'');
    }
    frame.push_str(
        "; printf '.')\neval \"${__md_sim_serial_payload%.}\"\n\
         __md_sim_rc=$?; printf '\\nMD_SIM_SERIAL_SCRIPT_FINISHED:%s\\n' \"$__md_sim_rc\"\n}\n",
    );
    frame.into_bytes()
}

fn prompt_after_reset(window: &mut Vec<u8>, reset: &mut bool) -> bool {
    if !*reset {
        let marker = b"reboot: Restarting system";
        if let Some(offset) = window.windows(marker.len()).position(|w| w == marker) {
            // A prompt buffered before the reset is from the previous boot.
            window.drain(..offset + marker.len());
            *reset = true;
        }
    }
    *reset && (window.windows(4).any(|w| w == b"/ # ") || window.windows(4).any(|w| w == b":/# "))
}

fn script_status(window: &[u8]) -> Option<u8> {
    let offset = window.windows(DONE.len()).position(|w| w == DONE)? + DONE.len();
    let rest = &window[offset..];
    let end = rest.windows(2).position(|w| w == b"\r\n")?;
    if end == 0 || end > 3 || !rest[..end].iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(&rest[..end]).ok()?.parse().ok()
}

fn read_until_done(output: &mut impl Read, window: &mut Vec<u8>) -> Result<Option<u8>> {
    let mut buffer = [0u8; 4096];
    loop {
        let count = output.read(&mut buffer)?;
        if count == 0 {
            return Ok(None);
        }
        std::io::stdout().write_all(&buffer[..count])?;
        std::io::stdout().flush()?;
        window.extend_from_slice(&buffer[..count]);
        if let Some(status) = script_status(window) {
            return Ok(Some(status));
        }
        if window.len() > 8192 {
            window.drain(..window.len() - 8192);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{prompt_after_reset, script_frame, script_status};
    use std::{
        io::{Read, Write},
        process::{Command, Stdio},
    };

    #[test]
    fn spawn_callback_runs_before_console_failure_and_not_after_spawn_failure() {
        let mut spawned = false;
        let error = super::SerialGuest::start_confined(Command::new("/bin/false"), false, || {
            spawned = true
        });
        assert!(error.is_err());
        assert!(
            spawned,
            "successful spawn must release parent endpoints before console EOF"
        );
        spawned = false;
        let error = super::SerialGuest::start_confined(
            Command::new("/md-sim-absent-serial-regression-executable"),
            false,
            || spawned = true,
        );
        assert!(error.is_err());
        assert!(!spawned, "spawn failure must not report child attachment");
    }

    #[test]
    fn emulator_accounting_reaps_only_the_owned_child_and_retains_its_status() {
        // Both processes are test fixtures. Leave the sibling's exit pending
        // before our owned child exits, then verify it was not consumed.
        let mut sibling = Command::new("/bin/sh")
            .args(["-c", "printf ready; exit 23"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut ready = String::new();
        sibling
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut ready)
            .unwrap();
        assert_eq!(ready, "ready");
        let mut child = Command::new("/bin/sh")
            .args(["-c", "IFS= read -r ignored; exit 7"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut guest = super::SerialGuest {
            input: child.stdin.take(),
            output: child.stdout.take().unwrap(),
            child,
            window: Vec::new(),
            started: std::time::Instant::now(),
            usage_before: None,
            reaped: None,
        };
        assert!(guest.reap(false).unwrap().is_none());
        guest
            .input
            .as_mut()
            .unwrap()
            .write_all(b"finish\n")
            .unwrap();
        let status = guest.reap(true).unwrap().unwrap();
        assert_eq!(status.code(), Some(7));
        assert!(guest.reaped.as_ref().unwrap().1.is_some());
        assert_eq!(guest.reap(false).unwrap(), Some(status));
        assert_eq!(guest.reap(true).unwrap(), Some(status));
        drop(guest);
        assert_eq!(sibling.wait().unwrap().code(), Some(23));
    }

    #[test]
    fn interruption_requires_owned_signal_exit_and_preserves_pending_serial() {
        use std::os::unix::process::ExitStatusExt;
        for already_exited in [false, true] {
            // Our own process fixture, never archived firmware evidence.
            let mut child = Command::new("/bin/sh")
                .args([
                    "-c",
                    if already_exited {
                        "exit 7"
                    } else {
                        "printf 'ready\\nremaining bytes\\n'; IFS= read -r ignored"
                    },
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let mut guest = super::SerialGuest {
                input: child.stdin.take(),
                output: child.stdout.take().unwrap(),
                child,
                window: Vec::new(),
                started: std::time::Instant::now(),
                usage_before: None,
                reaped: None,
            };
            if already_exited {
                let status = guest.reap(true).unwrap().unwrap();
                assert_eq!(status.code(), Some(7));
                assert_eq!(guest.reap(false).unwrap(), Some(status));
                assert!(guest.reaped.as_ref().unwrap().1.is_some());
                assert!(guest.interrupt_and_exit(2).is_err());
            } else {
                let mut ready = [0; 6];
                guest.output.read_exact(&mut ready).unwrap();
                assert_eq!(&ready, b"ready\n");
                let observation = guest.interrupt_and_exit(2).unwrap();
                assert!(guest.reaped.as_ref().unwrap().1.is_some());
                assert_eq!(
                    guest.reap(false).unwrap().unwrap().signal(),
                    Some(libc::SIGKILL)
                );
                observation.verify_serial(b"remaining bytes\n").unwrap();
                assert!(observation.verify_serial(b"changed").is_err());
                assert!(guest.interrupt_and_exit(2).is_err());
                let mut changed = observation;
                changed.emulator_signal = libc::SIGTERM;
                assert!(changed.verify_serial(b"remaining bytes\n").is_err());
                changed.emulator_signal = libc::SIGKILL;
                let bytes = b"reboot: Restarting system\n";
                changed.serial.sha256 = crate::prepare::hash_bytes(bytes);
                changed.serial.size = bytes.len() as u64;
                assert!(changed.verify_serial(bytes).is_err());
            }
        }
    }

    #[test]
    fn native_restart_request_uses_normal_guest_shutdown_and_requires_exit() {
        for (script, expected) in [
            ("IFS= read -r request; [ \"$request\" = /sbin/reboot ] || exit 9; printf 'unit fixture\\nreboot: Restarting system\\n'", true),
            ("IFS= read -r request; [ \"$request\" = /sbin/reboot ] || exit 9; printf 'unit fixture refused shutdown\\n'", false),
        ] {
            // Our own shell process checks the bytes. It is not firmware evidence.
            let mut child = Command::new("/bin/sh").args(["-c", script])
                .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
            let mut guest = super::SerialGuest {
                input:child.stdin.take(), output:child.stdout.take().unwrap(), child,
                window:Vec::new(), started:std::time::Instant::now(), usage_before:None, reaped:None,
            };
            let result = guest.reboot_and_exit(2);
            assert_eq!(result.is_ok(), expected);
            if let Ok(observation) = result {
                observation.verify_serial(b"unit fixture\nreboot: Restarting system\n").unwrap();
            }
        }
    }

    #[test]
    fn restart_exit_requires_real_successful_exit_marker_and_request() {
        use super::SerialGuest;
        use std::time::Instant;
        for (script, request_ok, expected) in [
            (
                "printf 'unit fixture\\nreboot: Restarting system\\n'; exit 0",
                true,
                true,
            ),
            (
                "printf 'unit fixture\\nreboot: Restarting system\\n'; exit 7",
                true,
                false,
            ),
            (
                "printf 'unit fixture without restart\\n'; exit 0",
                true,
                false,
            ),
            (
                "printf 'reboot: Restarting system\\nreboot: Restarting system\\n'; exit 0",
                true,
                false,
            ),
            (
                "printf 'reboot: Restarting system\\n'; exit 0",
                false,
                false,
            ),
            (
                "printf 'reboot: Restarting system\\n'; exec 1>&-; read value",
                true,
                false,
            ),
            (
                "printf 'reboot: Restarting system\\n'; read value",
                true,
                false,
            ),
        ] {
            // This is our own process fixture, not acquired firmware.
            let mut child = Command::new("/bin/sh")
                .args(["-c", script])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            let mut guest = SerialGuest {
                input: child.stdin.take(),
                output: child.stdout.take().unwrap(),
                child,
                window: Vec::new(),
                started: Instant::now(),
                usage_before: None,
                reaped: None,
            };
            let result = guest.wait_for_restart_exit(2, || {
                anyhow::ensure!(request_ok, "unit fixture rejected request");
                Ok(17)
            });
            assert_eq!(result.is_ok(), expected, "{script}");
            if let Ok((value, observation)) = result {
                assert_eq!(value, 17);
                observation
                    .verify_serial(b"unit fixture\nreboot: Restarting system\n")
                    .unwrap();
                assert!(observation.verify_serial(b"different bytes").is_err());
                let mut changed = observation;
                changed.emulator_exit_code = 7;
                assert!(changed
                    .verify_serial(b"unit fixture\nreboot: Restarting system\n")
                    .is_err());
            }
        }
    }

    #[test]
    fn service_wait_drains_output_larger_than_the_transport_buffer() {
        use std::{
            fs::File,
            os::{fd::OwnedFd, unix::net::UnixStream},
            sync::{
                atomic::{AtomicBool, AtomicUsize, Ordering},
                Arc, Mutex,
            },
            time::{Duration, Instant},
        };
        struct Sink(Arc<Mutex<Vec<u8>>>, Arc<AtomicUsize>);
        impl Write for Sink {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                self.1.fetch_add(bytes.len(), Ordering::Release);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let expected = b"retained native serial output\n".repeat(40_000);
        let data = expected.clone();
        let (input, producer) = UnixStream::pair().unwrap();
        // Use POSIX read/write like the actual emulator pipe. This host
        // permits Unix pairs but restricts the socket send/receive syscalls.
        let mut producer = File::from(OwnedFd::from(producer));
        let bytes = Arc::new(Mutex::new(Vec::new()));
        let count = Arc::new(AtomicUsize::new(0));
        let stop = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let sink = Sink(bytes.clone(), count.clone());
            let reader =
                scope.spawn(|| super::drain_output(File::from(OwnedFd::from(input)), sink, &stop));
            let writer = scope.spawn(move || {
                producer.write_all(&data).unwrap();
                producer // Keep the endpoint open until the reader is stopped.
            });
            {
                let _stop_on_drop = super::StopReader(&stop);
                let deadline = Instant::now() + Duration::from_secs(5);
                while count.load(Ordering::Acquire) < expected.len() {
                    assert!(Instant::now() < deadline, "serial producer stayed blocked");
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
            reader.join().unwrap().unwrap();
            drop(writer.join().unwrap());
        });
        assert_eq!(*bytes.lock().unwrap(), expected);
    }

    #[test]
    fn service_wait_rejects_an_unexpected_serial_close() {
        use std::{
            fs::File,
            os::{fd::OwnedFd, unix::net::UnixStream},
            sync::atomic::AtomicBool,
        };
        let (input, producer) = UnixStream::pair().unwrap();
        drop(producer);
        let result = super::drain_output(
            File::from(OwnedFd::from(input)),
            Vec::new(),
            &AtomicBool::new(false),
        );
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("guest serial closed"));
    }

    #[test]
    fn shell_parses_completion_before_executing_any_script_command() {
        for (script, status) in [
            (b"printf 'script-ran\\n'\ntrue\n".as_slice(), 0),
            (
                b"(set -e; printf 'script-ran\\n'; exit 23)\n".as_slice(),
                23,
            ),
        ] {
            let frame = script_frame(script);
            for complete in [false, true] {
                let mut shell = Command::new("/bin/sh")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                let end = if complete {
                    frame.len()
                } else {
                    frame.len() - 2 // The closing brace has not arrived.
                };
                shell
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(&frame[..end])
                    .unwrap();
                let output = shell.wait_with_output().unwrap();
                if complete {
                    assert!(output.status.success());
                    assert_eq!(
                        output.stdout,
                        format!("script-ran\n\nMD_SIM_SERIAL_SCRIPT_FINISHED:{status}\n")
                            .as_bytes()
                    );
                } else {
                    assert!(!output.status.success());
                    assert!(output.stdout.is_empty(), "partial input executed script");
                }
            }
        }
    }

    #[test]
    fn serial_frame_preserves_quoted_control_bytes_and_long_scripts() {
        let mut scripts = vec![
            b"printf '%s' 'tab:\t cr:\r quote:'\"'\"' slash:\\ dollar:$ backtick:`'\n".to_vec(),
            b"cat <<'END'\n\tliteral tab and \\n\nEND\n\n".to_vec(),
            "printf '%s\\n' 'UTF-8: ç✓'\n(false)\n".as_bytes().to_vec(),
            Vec::new(),
        ];
        scripts.push(format!("printf '%s' '{}'\n", "quoted'\t' data ".repeat(1000)).into_bytes());
        for script in scripts {
            let frame = script_frame(&script);
            assert!(frame.iter().all(|b| *b == b'\n' || (32..=126).contains(b)));
            assert!(frame.split(|b| *b == b'\n').all(|line| line.len() < 600));
            let run = |input: &[u8]| {
                let mut shell = Command::new("/bin/sh")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap();
                shell.stdin.take().unwrap().write_all(input).unwrap();
                shell.wait_with_output().unwrap()
            };
            let reference = run(&script);
            let actual = run(&frame);
            assert!(actual.status.success());
            let mut expected = reference.stdout;
            expected.extend_from_slice(
                format!(
                    "\nMD_SIM_SERIAL_SCRIPT_FINISHED:{}\n",
                    reference.status.code().unwrap()
                )
                .as_bytes(),
            );
            assert_eq!(actual.stdout, expected);
            assert_eq!(actual.stderr, reference.stderr);
        }
    }

    #[test]
    fn normal_init_prompt_must_arrive_in_full_after_activation() {
        let prompt = b"root@(none):/# ";
        let mut window = b"Please press Enter to activate this console.\r\n".to_vec();
        let mut reset = true;
        assert!(!prompt_after_reset(&mut window, &mut reset));
        for (index, byte) in prompt.iter().enumerate() {
            window.push(*byte);
            assert_eq!(
                prompt_after_reset(&mut window, &mut reset),
                index == prompt.len() - 1
            );
        }
    }

    #[test]
    fn reboot_requires_a_prompt_after_the_actual_kernel_reset_line() {
        let stream = b"/ # /sbin/reboot -f\r\n[12.3] reboot: Restarting system\r\n[1.0] console [ttyPS0] enabled\r\n/ # ";
        for chunk_size in 1..=stream.len() {
            let mut window = Vec::new();
            let mut reset = false;
            let mut received = 0;
            for chunk in stream.chunks(chunk_size) {
                received += chunk.len();
                window.extend_from_slice(chunk);
                assert_eq!(
                    prompt_after_reset(&mut window, &mut reset),
                    received == stream.len(),
                    "chunk size{chunk_size},received{received}"
                );
            }
        }
        let mut reset = false;
        let mut failed = b"/ # /sbin/reboot -f\r\nreboot: not found\r\n/ # ".to_vec();
        assert!(!prompt_after_reset(&mut failed, &mut reset));
        assert!(!reset);
    }

    #[test]
    fn completion_requires_a_full_guest_output_line_and_keeps_failure_status() {
        assert_eq!(
            script_status(
                b"/ # printf '\\nMD_SIM_SERIAL_SCRIPT_FINISHED:%s\\n' \"$__md_sim_rc\"\r\n"
            ),
            None
        );
        assert_eq!(script_status(b"\r\nMD_SIM_SERIAL_SCRIPT_FINISHED:0"), None);
        assert_eq!(
            script_status(b"\r\nMD_SIM_SERIAL_SCRIPT_FINISHED:256\r\n"),
            None
        );
        assert_eq!(
            script_status(b"\r\nMD_SIM_SERIAL_SCRIPT_FINISHED:1\r\n"),
            Some(1)
        );
        assert_eq!(
            script_status(b"output\r\nMD_SIM_SERIAL_SCRIPT_FINISHED:0\r\n"),
            Some(0)
        );
    }
}

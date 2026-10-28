//! One owned process and its private inherited fixed-frame duplex descriptor.
//! The caller must use the existing isolated preparation environment before
//! launching acquired code. This supervisor is not standalone hostile-process
//! containment; the existing private PID namespace is required. Unit fixtures
//! here execute only this Rust test code.
pub const FRAME_BYTES: usize = 128;
pub type Frame = [u8; FRAME_BYTES];
use serde::Serialize;
use std::{
    collections::VecDeque,
    fs::File,
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, RawFd},
        unix::{
            net::UnixStream,
            process::{CommandExt, ExitStatusExt},
        },
    },
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Serialize)]
pub struct ChildLimits {
    pub command_timeout_ms: u64,
    pub shutdown_timeout_ms: u64,
    pub stdout_limit: usize,
    pub stderr_limit: usize,
    /// Total retained request and response frames, including startup events.
    pub frame_limit: usize,
}
impl ChildLimits {
    fn validate(self) -> io::Result<()> {
        if !(1..=120_000).contains(&self.command_timeout_ms)
            || !(1..=5_000).contains(&self.shutdown_timeout_ms)
            || self.stdout_limit > 16 << 20
            || self.stderr_limit > 16 << 20
            || !(2..=262_144).contains(&self.frame_limit)
        {
            return Err(invalid("live child limits outside supported bounds"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SentFrame {
    pub hex: String,
    /// Actual bytes written. A failed partial write is never reported as sent.
    pub written: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Exit {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Evidence {
    pub process_id_in_current_namespace: u32,
    pub limits: ChildLimits,
    pub sent_frames: Vec<SentFrame>,
    pub received_frames: Vec<String>,
    pub partial_response_hex: String,
    pub stdout_hex: String,
    pub stderr_hex: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub protocol_truncated: bool,
    pub stdout_eof: bool,
    pub stderr_eof: bool,
    pub protocol_eof: bool,
    pub exit: Option<Exit>,
    pub failure: Option<String>,
    pub cleanup_error: Option<String>,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn nonblocking(fd: RawFd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
fn log_pipe() -> io::Result<(File, File)> {
    let mut fds = [-1; 2];
    if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let read = unsafe { File::from_raw_fd(fds[0]) };
    let write = unsafe { File::from_raw_fd(fds[1]) };
    nonblocking(read.as_raw_fd())?;
    Ok((read, write))
}

pub struct FramedChild {
    child: Option<Child>,
    pid: u32,
    wire: UnixStream,
    stdout: File,
    stderr: File,
    limits: ChildLimits,
    cancelled: Arc<AtomicBool>,
    pending: VecDeque<Frame>,
    sent: Vec<SentFrame>,
    received: Vec<String>,
    partial: Vec<u8>,
    out: Vec<u8>,
    err: Vec<u8>,
    out_truncated: bool,
    err_truncated: bool,
    protocol_truncated: bool,
    out_eof: bool,
    err_eof: bool,
    wire_eof: bool,
    exit: Option<Exit>,
    failure: Option<String>,
    cleanup_error: Option<String>,
}

impl FramedChild {
    pub fn launch(
        mut command: Command,
        limits: ChildLimits,
        cancelled: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        limits.validate()?;
        if cancelled.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "cancelled before child launch",
            ));
        }
        let (wire, child_wire) = UnixStream::pair()?;
        wire.set_nonblocking(true)?;
        child_wire.set_nonblocking(false)?;
        // Keep the source socket clear of descriptors0..2, which Command
        // replaces before pre_exec. The fixed child endpoint is descriptor3.
        let child_fd = unsafe { libc::fcntl(child_wire.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 10) };
        if child_fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let child_wire = unsafe { UnixStream::from_raw_fd(child_fd) };
        let (stdout, child_stdout) = log_pipe()?;
        let (stderr, child_stderr) = log_pipe()?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(child_stdout))
            .stderr(Stdio::from(child_stderr));
        unsafe {
            command.pre_exec(move || {
                if libc::setsid() < 0
                    || libc::dup2(child_fd, 3) < 0
                    || libc::fcntl(3, libc::F_SETFD, 0) < 0
                {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn()?;
        let pid = child.id();
        // In particular, close Command's retained write ends in this process.
        drop(command);
        drop(child_wire);
        Ok(Self {
            child: Some(child),
            pid,
            wire,
            stdout,
            stderr,
            limits,
            cancelled,
            pending: VecDeque::new(),
            sent: Vec::new(),
            received: Vec::new(),
            partial: Vec::new(),
            out: Vec::new(),
            err: Vec::new(),
            out_truncated: false,
            err_truncated: false,
            protocol_truncated: false,
            out_eof: false,
            err_eof: false,
            wire_eof: false,
            exit: None,
            failure: None,
            cleanup_error: None,
        })
    }

    pub fn evidence(&self) -> Evidence {
        Evidence {
            process_id_in_current_namespace: self.pid,
            limits: self.limits,
            sent_frames: self.sent.clone(),
            received_frames: self.received.clone(),
            partial_response_hex: hex::encode(&self.partial),
            stdout_hex: hex::encode(&self.out),
            stderr_hex: hex::encode(&self.err),
            stdout_truncated: self.out_truncated,
            stderr_truncated: self.err_truncated,
            protocol_truncated: self.protocol_truncated,
            stdout_eof: self.out_eof,
            stderr_eof: self.err_eof,
            protocol_eof: self.wire_eof,
            exit: self.exit.clone(),
            failure: self.failure.clone(),
            cleanup_error: self.cleanup_error.clone(),
        }
    }

    pub fn is_live(&self) -> bool {
        self.child.is_some() && self.failure.is_none()
    }

    pub fn command_timeout_ms(&self) -> u64 {
        self.limits.command_timeout_ms
    }

    fn deadline(&self, timeout_ms: u64) -> io::Result<Instant> {
        if !(1..=self.limits.command_timeout_ms).contains(&timeout_ms) {
            return Err(invalid(
                "operation timeout outside configured command bound",
            ));
        }
        if !self.is_live() {
            return Err(invalid("framed child is not live"));
        }
        Ok(Instant::now() + Duration::from_millis(timeout_ms))
    }

    fn prepare_send(&mut self, frame: &Frame) -> io::Result<()> {
        self.reserve_frame()?;
        self.sent.push(SentFrame {
            hex: hex::encode(frame),
            written: 0,
        });
        Ok(())
    }

    fn write_progress(&mut self, send: &mut Option<Frame>) -> io::Result<()> {
        if let Some(bytes) = send {
            let record = self.sent.last_mut().unwrap();
            match self.wire.write(&bytes[record.written..]) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "private request write returned zero",
                    ))
                }
                Ok(count) => {
                    record.written += count;
                    if record.written == FRAME_BYTES {
                        *send = None;
                    }
                }
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock
                        || error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// Send one frame while draining all streams. Frames received during the
    /// write stay in the bounded pending queue for the next receive operation.
    /// The caller must also enforce its overall multi-frame request deadline.
    pub fn send_frame(&mut self, frame: Frame, timeout_ms: u64) -> io::Result<()> {
        let result = (|| {
            let deadline = self.deadline(timeout_ms)?;
            self.prepare_send(&frame)?;
            let mut send = Some(frame);
            loop {
                self.check_command_deadline(deadline)?;
                self.write_progress(&mut send)?;
                let incoming = self.pump()?;
                self.pending.extend(incoming);
                if send.is_none() {
                    self.check_command_deadline(deadline)?;
                    return Ok(());
                }
                self.check_waiting_peer()?;
                self.poll(deadline, true)?;
            }
        })();
        self.finish_operation(result)
    }

    /// Receive one complete frame. This is transport only: the caller must
    /// validate its magic, fields, order, and meaning before using any value.
    pub fn receive_frame(&mut self, timeout_ms: u64) -> io::Result<Frame> {
        let result = (|| {
            let deadline = self.deadline(timeout_ms)?;
            loop {
                self.check_command_deadline(deadline)?;
                let incoming = self.pump()?;
                self.pending.extend(incoming);
                if let Some(frame) = self.pending.pop_front() {
                    self.check_command_deadline(deadline)?;
                    return Ok(frame);
                }
                self.check_waiting_peer()?;
                self.poll(deadline, false)?;
            }
        })();
        self.finish_operation(result)
    }

    /// One bounded request/receipt exchange for a strict protocol adapter.
    /// `observe` returns true only for its terminal receipt. All complete
    /// frames from the same pump are validated, so a duplicate or observation
    /// after that receipt fails. No codec or device meaning is defined here.
    pub fn exchange_until(
        &mut self,
        send: Option<Frame>,
        timeout_ms: u64,
        observe: impl FnMut(&Frame) -> io::Result<bool>,
    ) -> io::Result<()> {
        let result = self.wait_exchange(send, timeout_ms, observe);
        self.finish_operation(result)
    }

    /// Require the explicit protocol shutdown to be followed by clean exit,
    /// complete stream EOF and no trailing complete or partial private frame.
    pub fn finish(&mut self) -> io::Result<()> {
        let result = (|| {
            if self.failure.is_some() {
                return Err(invalid("cannot finish a failed framed child"));
            }
            if !self.pending.is_empty() {
                return Err(invalid("pending private response at shutdown"));
            }
            self.cleanup(false)?;
            if !self
                .exit
                .as_ref()
                .is_some_and(|e| e.code == Some(0) && e.signal.is_none())
            {
                return Err(invalid("shutdown was not followed by a clean child exit"));
            }
            Ok(())
        })();
        self.finish_operation(result)
    }

    fn finish_operation<T>(&mut self, result: io::Result<T>) -> io::Result<T> {
        if let Err(error) = &result {
            self.failure.get_or_insert_with(|| error.to_string());
            if let Err(cleanup) = self.cleanup(true) {
                self.cleanup_error
                    .get_or_insert_with(|| cleanup.to_string());
            }
        }
        result
    }

    pub fn abort(&mut self, reason: &str) -> io::Result<()> {
        self.failure.get_or_insert_with(|| reason.to_owned());
        let result = self.cleanup(true);
        if let Err(error) = &result {
            self.cleanup_error.get_or_insert_with(|| error.to_string());
        }
        result
    }

    fn reserve_frame(&mut self) -> io::Result<()> {
        if self.sent.len() + self.received.len() >= self.limits.frame_limit {
            self.protocol_truncated = true;
            return Err(invalid("private frame retention bound exceeded"));
        }
        Ok(())
    }

    fn wait_exchange(
        &mut self,
        mut send: Option<Frame>,
        timeout_ms: u64,
        mut observe: impl FnMut(&Frame) -> io::Result<bool>,
    ) -> io::Result<()> {
        let deadline = self.deadline(timeout_ms)?;
        if let Some(frame) = &send {
            self.prepare_send(frame)?;
        }
        loop {
            self.check_command_deadline(deadline)?;
            self.write_progress(&mut send)?;
            let incoming = self.pump()?;
            self.pending.extend(incoming);
            let mut done = false;
            while let Some(bytes) = self.pending.pop_front() {
                if done {
                    return Err(invalid("observation followed a completed command"));
                }
                if observe(&bytes)? {
                    if send.is_some() {
                        return Err(invalid("completion before request transmission"));
                    }
                    done = true;
                }
            }
            if done {
                // Receipt acceptance is after bounded I/O and validation.
                self.check_command_deadline(deadline)?;
                return Ok(());
            }
            self.check_waiting_peer()?;
            self.poll(deadline, send.is_some())?;
        }
    }

    fn check_waiting_peer(&self) -> io::Result<()> {
        if self.wire_eof {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "private channel ended before complete receipt",
            ));
        }
        if self.terminal()? {
            return Err(invalid("child exited before complete receipt"));
        }
        Ok(())
    }

    fn check_command_deadline(&self, deadline: Instant) -> io::Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "live child cancelled",
            ));
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "live child command deadline",
            ));
        }
        Ok(())
    }

    fn pump(&mut self) -> io::Result<Vec<[u8; FRAME_BYTES]>> {
        let mut output = Vec::new();
        let mut first_error = None;
        for (file, bytes, eof, truncated, limit) in [
            (
                &mut self.stdout,
                &mut self.out,
                &mut self.out_eof,
                &mut self.out_truncated,
                self.limits.stdout_limit,
            ),
            (
                &mut self.stderr,
                &mut self.err,
                &mut self.err_eof,
                &mut self.err_truncated,
                self.limits.stderr_limit,
            ),
        ] {
            if let Err(error) = drain_log(file, bytes, eof, truncated, limit) {
                first_error.get_or_insert(error);
            }
        }
        for _ in 0..32 {
            if self.wire_eof {
                break;
            }
            let mut bytes = [0; FRAME_BYTES];
            match self
                .wire
                .read(&mut bytes[..FRAME_BYTES - self.partial.len()])
            {
                Ok(0) => {
                    self.wire_eof = true;
                    break;
                }
                Ok(count) => {
                    self.partial.extend_from_slice(&bytes[..count]);
                    if self.partial.len() == FRAME_BYTES {
                        let bytes: [u8; FRAME_BYTES] = self.partial[..].try_into().unwrap();
                        self.partial.clear();
                        if let Err(error) = self.reserve_frame() {
                            first_error.get_or_insert(error);
                        } else {
                            self.received.push(hex::encode(bytes));
                            output.push(bytes);
                        }
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    first_error.get_or_insert(error);
                    break;
                }
            }
        }
        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(output)
        }
    }

    fn poll(&self, deadline: Instant, writing: bool) -> io::Result<()> {
        let mut fds = [
            libc::pollfd {
                fd: if self.wire_eof {
                    -1
                } else {
                    self.wire.as_raw_fd()
                },
                events: libc::POLLIN | if writing { libc::POLLOUT } else { 0 },
                revents: 0,
            },
            libc::pollfd {
                fd: if self.out_eof {
                    -1
                } else {
                    self.stdout.as_raw_fd()
                },
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if self.err_eof {
                    -1
                } else {
                    self.stderr.as_raw_fd()
                },
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let remaining = deadline.saturating_duration_since(Instant::now());
        let millis = remaining.as_millis().clamp(1, 50) as i32;
        if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, millis) } < 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
        Ok(())
    }

    /// Never reap here. Keeping the leader's PID allocated prevents PID reuse
    /// between its observed exit and the later owned-process-group kill.
    fn terminal(&self) -> io::Result<bool> {
        if self.child.is_none() {
            return Ok(true);
        }
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        loop {
            let rc = unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.pid,
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if rc == 0 {
                return Ok(unsafe { info.si_pid() } == self.pid as i32);
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }

    fn kill_group(&self) -> io::Result<()> {
        if self.child.is_some() && unsafe { libc::kill(-(self.pid as i32), libc::SIGKILL) } != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
        Ok(())
    }

    fn cleanup(&mut self, kill_now: bool) -> io::Result<()> {
        if self.child.is_none() {
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_millis(self.limits.shutdown_timeout_ms);
        let mut capture_error = None;
        if kill_now {
            self.kill_group()?;
        }
        while !self.terminal()? {
            // Errors have already been retained; keep draining bounded streams
            // while the process exits so a full pipe cannot prevent teardown.
            match self.pump() {
                Ok(frames) if !kill_now && !frames.is_empty() => {
                    capture_error
                        .get_or_insert_with(|| invalid("private response after stop receipt"));
                }
                Err(error) => {
                    capture_error.get_or_insert(error);
                }
                _ => {}
            }
            if Instant::now() >= deadline {
                self.kill_group()?;
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "child did not exit within shutdown bound",
                ));
            }
            self.poll(deadline, false)?;
        }
        self.kill_group()?;
        let status = self.child.as_mut().unwrap().wait()?;
        self.exit = Some(Exit {
            code: status.code(),
            signal: status.signal(),
        });
        self.child.take();
        while !(self.wire_eof && self.out_eof && self.err_eof) {
            match self.pump() {
                Ok(frames) if !kill_now && !frames.is_empty() => {
                    capture_error
                        .get_or_insert_with(|| invalid("private response after stop receipt"));
                }
                Err(error) => {
                    capture_error.get_or_insert(error);
                }
                _ => {}
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "child descriptors did not close within shutdown bound",
                ));
            }
            if !(self.wire_eof && self.out_eof && self.err_eof) {
                self.poll(deadline, false)?;
            }
        }
        if !self.partial.is_empty() {
            return Err(invalid("partial private frame at child exit"));
        }
        if let Some(error) = capture_error {
            return Err(error);
        }
        Ok(())
    }
}
impl Drop for FramedChild {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup(true) {
            self.cleanup_error.get_or_insert_with(|| error.to_string());
        }
    }
}

fn drain_log(
    file: &mut File,
    retained: &mut Vec<u8>,
    eof: &mut bool,
    truncated: &mut bool,
    limit: usize,
) -> io::Result<()> {
    let mut overflow = false;
    for _ in 0..8 {
        if *eof {
            break;
        }
        let mut bytes = [0; 8192];
        match file.read(&mut bytes) {
            Ok(0) => {
                *eof = true;
                break;
            }
            Ok(count) => {
                let keep = count.min(limit.saturating_sub(retained.len()));
                retained.extend_from_slice(&bytes[..keep]);
                if keep < count {
                    *truncated = true;
                    overflow = true;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    if overflow {
        Err(invalid("child log retention bound exceeded"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn limits() -> ChildLimits {
        ChildLimits {
            command_timeout_ms: 5_000,
            shutdown_timeout_ms: 2_000,
            stdout_limit: 8 << 10,
            stderr_limit: 8 << 10,
            frame_limit: 64,
        }
    }
    fn launch(mode: &str) -> FramedChild {
        let name = module_path!().split_once("::").unwrap().1;
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                &format!("{name}::peer"),
                "--nocapture",
            ])
            .env("MD_SIM_FRAMED_PEER_514", mode);
        FramedChild::launch(command, limits(), Arc::new(AtomicBool::new(false))).unwrap()
    }

    #[test]
    #[ignore = "owned peer for actual private-channel tests only"]
    fn peer() {
        let mode = std::env::var("MD_SIM_FRAMED_PEER_514").unwrap();
        let mut wire = unsafe { UnixStream::from_raw_fd(3) };
        assert_eq!(
            unsafe { libc::fcntl(3, libc::F_GETFL) } & libc::O_NONBLOCK,
            0
        );
        println!("RAW_STDOUT");
        eprintln!("RAW_STDERR");
        match mode.as_str() {
            "exchange" => {
                // Child-initiated request followed by multiple parent events
                // and one explicit shutdown reply. These are arbitrary bytes,
                // not the512 or514 device codec.
                for bytes in [0x12u8; FRAME_BYTES].chunks(5) {
                    wire.write_all(bytes).unwrap();
                }
                for expected in [0xa0u8, 0xa1, 0xff] {
                    let mut frame = [0u8; FRAME_BYTES];
                    wire.read_exact(&mut frame).unwrap();
                    assert_eq!(frame, [expected; FRAME_BYTES]);
                }
                wire.write_all(&[0x33; FRAME_BYTES]).unwrap();
            }
            "partial" => {
                wire.write_all(&[0x44; 19]).unwrap();
                drop(wire);
                thread::sleep(Duration::from_secs(60));
            }
            "batch" => {
                let mut frame = [0; FRAME_BYTES];
                wire.read_exact(&mut frame).unwrap();
                assert_eq!(frame, [0x55; FRAME_BYTES]);
                wire.write_all(&[0x66; FRAME_BYTES * 2]).unwrap();
            }
            "trailing" => {
                wire.write_all(&[0x77; FRAME_BYTES * 2]).unwrap();
            }
            _ => panic!("unknown raw peer mode"),
        }
    }

    #[test]
    fn child_initiated_frames_and_multiple_replies_keep_complete_evidence() {
        let mut child = launch("exchange");
        assert_eq!(child.receive_frame(5_000).unwrap(), [0x12; FRAME_BYTES]);
        for byte in [0xa0, 0xa1, 0xff] {
            child.send_frame([byte; FRAME_BYTES], 5_000).unwrap();
        }
        assert_eq!(child.receive_frame(5_000).unwrap(), [0x33; FRAME_BYTES]);
        child.finish().unwrap();
        let evidence = child.evidence();
        assert_eq!(evidence.sent_frames.len(), 3);
        assert_eq!(evidence.received_frames.len(), 2);
        assert!(evidence
            .sent_frames
            .iter()
            .all(|f| f.written == FRAME_BYTES));
        assert_eq!(evidence.exit.unwrap().code, Some(0));
        assert!(evidence.stdout_eof && evidence.stderr_eof && evidence.protocol_eof);
        assert!(evidence.failure.is_none() && evidence.cleanup_error.is_none());
        assert!(String::from_utf8(hex::decode(evidence.stdout_hex).unwrap())
            .unwrap()
            .contains("RAW_STDOUT"));
        assert!(String::from_utf8(hex::decode(evidence.stderr_hex).unwrap())
            .unwrap()
            .contains("RAW_STDERR"));
    }

    #[test]
    fn send_pumping_and_successive_receives_do_not_drop_batched_frames() {
        let mut child = launch("batch");
        child.send_frame([0x55; FRAME_BYTES], 5_000).unwrap();
        assert_eq!(child.receive_frame(5_000).unwrap(), [0x66; FRAME_BYTES]);
        assert_eq!(child.receive_frame(5_000).unwrap(), [0x66; FRAME_BYTES]);
        child.finish().unwrap();
        assert_eq!(child.evidence().received_frames.len(), 2);
    }

    #[test]
    fn partial_eof_and_unconsumed_shutdown_frames_fail_and_are_retained() {
        let mut child = launch("partial");
        assert_eq!(
            child.receive_frame(5_000).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
        let evidence = child.evidence();
        assert_eq!(evidence.partial_response_hex, "44".repeat(19));
        assert_eq!(evidence.exit.unwrap().signal, Some(libc::SIGKILL));
        assert!(child.finish().is_err());
        let mut child = launch("trailing");
        assert_eq!(child.receive_frame(5_000).unwrap(), [0x77; FRAME_BYTES]);
        assert!(child.finish().is_err());
        assert_eq!(child.evidence().received_frames.len(), 2);
        assert!(child.evidence().exit.is_some());
    }

    #[test]
    fn timeout_cannot_exceed_configured_bound_or_be_zero() {
        for timeout in [0, 5_001, u64::MAX] {
            let mut child = launch("exchange");
            assert!(child.receive_frame(timeout).is_err());
            let evidence = child.evidence();
            assert!(evidence.failure.is_some() && evidence.exit.is_some());
        }
    }
}

//! A single owned SSH child with bounded newline frames and per-request timers.
use super::{drain_pipe, limit_ssh_child, nonblocking, OwnedSshChild};
use anyhow::{ensure, Context, Result};
use md_sim_runtime::worker::{MAX_CONTROL_MESSAGE, MAX_RELAY_REQUESTS};
use std::{
    io::Write,
    process::{ChildStderr, ChildStdin, ChildStdout, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug)]
struct InterruptedChannel(String);
impl std::fmt::Display for InterruptedChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for InterruptedChannel {}

pub(super) fn retryable(error: &anyhow::Error) -> bool {
    error.is::<InterruptedChannel>()
        || error.downcast_ref::<std::io::Error>().is_some_and(|e| {
            matches!(
                e.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::TimedOut
            )
        })
}

pub(super) struct Session {
    child: OwnedSshChild,
    stdin: ChildStdin,
    stdout: ChildStdout,
    stderr: ChildStderr,
    diagnostic: Vec<u8>,
    requests: u32,
}

impl Session {
    pub(super) fn start(mut command: Command) -> Result<Self> {
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // CPU is bounded per session; each frame also has a wall deadline.
        limit_ssh_child(&mut command, Some(900), 64 * 1024 * 1024);
        let mut child = OwnedSshChild::new(command.spawn().context("start streaming SSH client")?);
        let stdin = child.child.stdin.take().context("SSH stdin missing")?;
        let stdout = child.child.stdout.take().context("SSH stdout missing")?;
        let stderr = child.child.stderr.take().context("SSH stderr missing")?;
        nonblocking(&stdin)?;
        nonblocking(&stdout)?;
        nonblocking(&stderr)?;
        Ok(Self {
            child,
            stdin,
            stdout,
            stderr,
            diagnostic: Vec::new(),
            requests: 0,
        })
    }

    pub(super) fn exchange(&mut self, message: &[u8], timeout: Duration) -> Result<Vec<u8>> {
        ensure!(
            self.requests < MAX_RELAY_REQUESTS,
            "SSH session request limit"
        );
        self.requests += 1;
        let began = Instant::now();
        let mut sent = 0;
        let mut received = Vec::new();
        // No response can precede its request. Do not accept delayed trailing
        // bytes from a prior frame as the response to the next operation.
        let eof = drain_pipe(
            &mut self.stdout,
            &mut received,
            MAX_CONTROL_MESSAGE as usize,
        )?;
        ensure!(received.is_empty(), "SSH channel has unsolicited bytes");
        if eof {
            return Err(InterruptedChannel("SSH channel closed".into()).into());
        }
        loop {
            if began.elapsed() >= timeout {
                return Err(InterruptedChannel(
                    "SSH request deadline exceeded; mutations require submission-token recovery"
                        .into(),
                )
                .into());
            }
            if sent < message.len() {
                let end = message.len().min(sent + 65536);
                match self.stdin.write(&message[sent..end]) {
                    Ok(0) => {
                        return Err(InterruptedChannel(
                            "SSH input closed before request completion".into(),
                        )
                        .into())
                    }
                    Ok(count) => sent += count,
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            let eof = drain_pipe(
                &mut self.stdout,
                &mut received,
                MAX_CONTROL_MESSAGE as usize,
            )?;
            drain_pipe(&mut self.stderr, &mut self.diagnostic, 65536)?;
            if let Some(end) = received.iter().position(|byte| *byte == b'\n') {
                ensure!(
                    sent == message.len() && end + 1 == received.len(),
                    "unsolicited or multiple SSH response frames"
                );
                return Ok(received);
            }
            if eof || self.child.exited()? {
                return Err(InterruptedChannel(format!(
                    "SSH channel ended before a complete response: {}",
                    String::from_utf8_lossy(&self.diagnostic)
                ))
                .into());
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    pub(super) fn exhausted(&self) -> bool {
        self.requests >= MAX_RELAY_REQUESTS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_child_handles_multiple_frames_and_keeps_deadlines() {
        let mut session = Session::start(Command::new("/usr/bin/cat")).unwrap();
        let pid = session.child.child.id();
        for size in [2, 128 * 1024, 3] {
            let mut frame = vec![b'x'; size];
            frame.push(b'\n');
            assert_eq!(
                session.exchange(&frame, Duration::from_secs(3)).unwrap(),
                frame
            );
            assert_eq!(session.child.child.id(), pid);
        }
        let mut command = Command::new("/usr/bin/sleep");
        command.arg("10");
        let mut silent = Session::start(command).unwrap();
        let began = Instant::now();
        assert!(silent
            .exchange(b"{}\n", Duration::from_millis(100))
            .is_err());
        assert!(began.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn multiple_replies_and_oversized_replies_are_rejected() {
        let mut echo = Session::start(Command::new("/usr/bin/cat")).unwrap();
        assert!(echo.exchange(b"{}\n{}\n", Duration::from_secs(3)).is_err());
        let mut echo = Session::start(Command::new("/usr/bin/cat")).unwrap();
        assert!(echo
            .exchange(
                &vec![b'x'; MAX_CONTROL_MESSAGE as usize + 1],
                Duration::from_secs(3)
            )
            .is_err());
    }

    #[test]
    fn session_limit_is_observed_before_another_request_is_written() {
        let mut echo = Session::start(Command::new("/usr/bin/cat")).unwrap();
        echo.requests = MAX_RELAY_REQUESTS - 1;
        assert!(!echo.exhausted());
        assert_eq!(
            echo.exchange(b"final-frame\n", Duration::from_secs(3))
                .unwrap(),
            b"final-frame\n"
        );
        assert!(echo.exhausted());
        assert!(echo
            .exchange(b"not-sent\n", Duration::from_secs(3))
            .is_err());
    }
}

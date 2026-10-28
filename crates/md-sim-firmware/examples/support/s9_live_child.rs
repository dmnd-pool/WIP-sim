//! Strict512 programmable-logic codec over the shared owned child transport.
use super::framed_child::FramedChild;
pub use super::framed_child::{ChildLimits, Evidence};
use super::live_protocol::{Kind, Op, Order, Request, Response};
use std::{
    io,
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub struct LiveChild {
    raw: FramedChild,
    order: Order,
    ready: bool,
}
impl LiveChild {
    pub fn launch(
        command: Command,
        limits: ChildLimits,
        cancelled: Arc<AtomicBool>,
    ) -> io::Result<Self> {
        Ok(Self {
            raw: FramedChild::launch(command, limits, cancelled)?,
            order: Order::default(),
            ready: false,
        })
    }
    pub fn evidence(&self) -> Evidence {
        self.raw.evidence()
    }
    pub fn abort(&mut self, reason: &str) -> io::Result<()> {
        self.raw.abort(reason)
    }
    fn finish_operation<T>(&mut self, result: io::Result<T>) -> io::Result<T> {
        if let Err(error) = &result {
            let _ = self.raw.abort(&error.to_string());
        }
        result
    }
    pub fn ready(&mut self) -> io::Result<Vec<Response>> {
        let result = (|| {
            if self.ready || !self.raw.is_live() {
                return Err(invalid("transport startup is not available"));
            }
            let frames = self.wait_for(Kind::Ready, None)?;
            self.ready = true;
            Ok(frames)
        })();
        self.finish_operation(result)
    }
    pub fn transact(&mut self, request: &Request) -> io::Result<Vec<Response>> {
        let result = (|| {
            if !self.ready || !self.raw.is_live() {
                return Err(invalid("transport is not live"));
            }
            self.order.admit(request)?;
            let frames = self.wait_for(Kind::Complete, Some(request.encode()?))?;
            if request.operation == Op::Stop {
                self.raw.finish()?;
            }
            Ok(frames)
        })();
        self.finish_operation(result)
    }
    fn wait_for(&mut self, goal: Kind, send: Option<[u8; 128]>) -> io::Result<Vec<Response>> {
        let mut replies = Vec::new();
        let order = &mut self.order;
        self.raw
            .exchange_until(send, self.raw.command_timeout_ms(), |bytes| {
                let response = Response::decode(bytes)?;
                let completed = order.observe(&response)?;
                let done = response.kind == goal;
                if done && goal == Kind::Complete && !completed {
                    return Err(invalid("incomplete terminal receipt"));
                }
                replies.push(response);
                Ok(done)
            })?;
        Ok(replies)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::{
        io::{Read, Write},
        os::{fd::FromRawFd, unix::net::UnixStream},
        sync::atomic::Ordering,
        thread,
        time::Duration,
    };

    fn limits() -> ChildLimits {
        ChildLimits {
            command_timeout_ms: 5_000,
            shutdown_timeout_ms: 2_000,
            stdout_limit: 16 << 10,
            stderr_limit: 16 << 10,
            frame_limit: 128,
        }
    }
    fn launch(mode: &str, limits: ChildLimits) -> LiveChild {
        launch_with_cancel(mode, limits, Arc::new(AtomicBool::new(false)))
    }
    fn launch_with_cancel(
        mode: &str,
        limits: ChildLimits,
        cancelled: Arc<AtomicBool>,
    ) -> LiveChild {
        let name = module_path!().split_once("::").unwrap().1;
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                &format!("{name}::fixture_process"),
                "--nocapture",
            ])
            .env("MD_SIM_PRIVATE_CHILD_FIXTURE_512", mode);
        LiveChild::launch(command, limits, cancelled).unwrap()
    }
    fn request(sequence: u64, at_ps: u64, operation: Op) -> Request {
        Request {
            sequence,
            at_ps,
            operation,
            args: [0; 12],
        }
    }
    fn literal_response(kind: u64, sequence: u64, time: u64, operation: u64) -> [u8; 128] {
        let mut frame = [0u8; 128];
        frame[..8].copy_from_slice(b"MDPLR01\0");
        frame[8..16].copy_from_slice(&sequence.to_le_bytes());
        frame[16..24].copy_from_slice(&time.to_le_bytes());
        frame[24..32].copy_from_slice(&kind.to_le_bytes());
        if kind == 1 {
            frame[88..96].copy_from_slice(&operation.to_le_bytes());
        }
        frame
    }
    fn send_chunks(wire: &mut UnixStream, frame: &[u8]) {
        for chunk in frame.chunks(7) {
            wire.write_all(chunk).unwrap();
        }
    }

    /// Helper process for real private-channel lifecycle tests. It is never
    /// a firmware or programmable-logic acceptance fixture.
    #[test]
    #[ignore = "called only as an owned child by this module's transport tests"]
    fn fixture_process() {
        let mode = std::env::var("MD_SIM_PRIVATE_CHILD_FIXTURE_512").unwrap();
        let mut wire = unsafe { UnixStream::from_raw_fd(3) };
        assert_eq!(
            unsafe { libc::fcntl(3, libc::F_GETFL) } & libc::O_NONBLOCK,
            0
        );
        println!("STDOUT_ONLY");
        eprintln!("STDERR_ONLY");
        send_chunks(&mut wire, &literal_response(5, 0, 0, 0));
        loop {
            let mut bytes = [0; 128];
            wire.read_exact(&mut bytes).unwrap();
            let request = Request::decode(&bytes).unwrap();
            match mode.as_str() {
                "stall" => thread::sleep(Duration::from_secs(60)),
                "truncated" => {
                    wire.write_all(
                        &literal_response(
                            1,
                            request.sequence,
                            request.at_ps,
                            request.operation as u64,
                        )[..13],
                    )
                    .unwrap();
                    drop(wire);
                    thread::sleep(Duration::from_secs(60));
                    return;
                }
                "bad-magic" => {
                    wire.write_all(&[0; 128]).unwrap();
                    thread::sleep(Duration::from_secs(60));
                }
                "exit-with-descendant" => {
                    let descendant = Command::new("/usr/bin/sleep").arg("60").spawn().unwrap();
                    println!("OWNED_DESCENDANT={}", descendant.id());
                    std::io::stdout().flush().unwrap();
                    std::process::exit(0);
                }
                "flood" => {
                    std::io::stdout().write_all(&[b'x'; 32 << 10]).unwrap();
                    std::io::stdout().flush().unwrap();
                }
                "normal" | "late-flood" => {}
                _ => panic!("unknown child fixture mode"),
            }
            let mut event = literal_response(2, request.sequence, request.at_ps - 1, 0);
            // Preserve a real four-state fixture value, including X and Z.
            event[32..40].copy_from_slice(&((3u64 << 32) | 1).to_le_bytes());
            send_chunks(&mut wire, &event);
            send_chunks(
                &mut wire,
                &literal_response(1, request.sequence, request.at_ps, request.operation as u64),
            );
            if request.operation == Op::Stop {
                if mode == "late-flood" {
                    std::io::stdout().write_all(&[b'y'; 32 << 10]).unwrap();
                    std::io::stdout().flush().unwrap();
                }
                return;
            }
        }
    }

    #[test]
    fn actual_fragmented_frames_keep_vendor_logs_and_four_state_data_separate() {
        let mut child = launch("normal", limits());
        assert_eq!(child.ready().unwrap().last().unwrap().kind, Kind::Ready);
        let frames = child.transact(&request(1, 10, Op::Advance)).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!((frames[0].at_ps, frames[0].args[0]), (9, (3u64 << 32) | 1));
        child.transact(&request(2, 11, Op::Stop)).unwrap();
        let evidence = child.evidence();
        assert_eq!(evidence.exit.unwrap().code, Some(0));
        assert!(evidence.failure.is_none() && evidence.cleanup_error.is_none());
        assert!(evidence.stdout_eof && evidence.stderr_eof && evidence.protocol_eof);
        assert!(evidence.sent_frames.iter().all(|f| f.written == 128));
        assert!(hex::decode(evidence.stdout_hex)
            .unwrap()
            .windows(11)
            .any(|w| w == b"STDOUT_ONLY"));
        assert!(hex::decode(evidence.stderr_hex)
            .unwrap()
            .windows(11)
            .any(|w| w == b"STDERR_ONLY"));
        assert_eq!(evidence.received_frames.len(), 5);
    }

    #[test]
    fn malformed_or_partial_reply_is_retained_and_child_is_reaped() {
        for mode in ["bad-magic", "truncated"] {
            let mut child = launch(mode, limits());
            child.ready().unwrap();
            assert!(child.transact(&request(1, 10, Op::Advance)).is_err());
            let evidence = child.evidence();
            assert!(evidence.failure.is_some() && evidence.exit.is_some());
            if mode == "truncated" {
                assert_eq!(evidence.partial_response_hex.len(), 26);
            } else {
                assert_eq!(evidence.received_frames.last().unwrap(), &"00".repeat(128));
            }
        }
    }

    #[test]
    fn deadline_and_cancellation_interrupt_a_stalled_callback() {
        let mut child = launch("stall", limits());
        child.ready().unwrap();
        assert_eq!(
            child
                .raw
                .exchange_until(
                    Some(request(1, 10, Op::Advance).encode().unwrap()),
                    100,
                    |_| Ok(false),
                )
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert_eq!(child.evidence().exit.unwrap().signal, Some(libc::SIGKILL));
        let token = Arc::new(AtomicBool::new(false));
        let mut child = launch_with_cancel("stall", limits(), Arc::clone(&token));
        child.ready().unwrap();
        let cancel = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            token.store(true, Ordering::Release);
        });
        assert_eq!(
            child
                .transact(&request(1, 10, Op::Advance))
                .unwrap_err()
                .kind(),
            io::ErrorKind::Interrupted
        );
        cancel.join().unwrap();
        assert_eq!(child.evidence().exit.unwrap().signal, Some(libc::SIGKILL));
    }

    #[test]
    fn log_bounds_fail_even_after_a_valid_stop_receipt() {
        for mode in ["flood", "late-flood"] {
            let mut limits = limits();
            limits.stdout_limit = 1024;
            let mut child = launch(mode, limits);
            child.ready().unwrap();
            assert!(child.transact(&request(1, 10, Op::Stop)).is_err());
            let evidence = child.evidence();
            assert!(
                evidence.stdout_truncated && evidence.failure.is_some() && evidence.exit.is_some()
            );
            assert_eq!(evidence.stdout_hex.len(), 2048);
        }
    }

    #[test]
    fn exited_leader_keeps_group_identity_until_descendants_are_killed() {
        let mut child = launch("exit-with-descendant", limits());
        child.ready().unwrap();
        assert!(child.transact(&request(1, 10, Op::Advance)).is_err());
        let evidence = child.evidence();
        assert_eq!(evidence.exit.unwrap().code, Some(0));
        assert!(evidence.stdout_eof && evidence.stderr_eof && evidence.protocol_eof);
        let text = String::from_utf8(hex::decode(evidence.stdout_hex).unwrap()).unwrap();
        let pid: u32 = text
            .lines()
            .find_map(|line| line.strip_prefix("OWNED_DESCENDANT="))
            .unwrap()
            .parse()
            .unwrap();
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            assert_eq!(
                stat.rsplit_once(") ").unwrap().1.split_whitespace().next(),
                Some("Z")
            );
        }
    }
}

//! Bounded full-duplex relay for caller-owned connected stream sockets.
//! This creates no listener and proves neither endpoint identity nor topology.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    os::{
        fd::{AsRawFd, OwnedFd, RawFd},
        unix::fs::FileTypeExt,
    },
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub const MAX_CONNECTION_BYTES: u64 = 64 << 20;
pub const MAX_RUN_BYTES: u64 = 256 << 20;
pub const MAX_IDLE_SECONDS: u64 = 60;

/// Shared admission budget. Charge both directions of each connection together.
/// Consumed bytes are admitted reads, not a claim of application delivery.
pub struct ByteBudget {
    connection_bytes: u64,
    initial: u64,
    remaining: AtomicU64,
    idle: Duration,
    cancel: Arc<AtomicBool>,
}

impl ByteBudget {
    pub fn new(
        connection_bytes: u64,
        run_bytes: u64,
        idle_seconds: u64,
        cancel: Arc<AtomicBool>,
    ) -> Result<Self> {
        ensure!(
            (1..=MAX_CONNECTION_BYTES).contains(&connection_bytes),
            "stream connection byte bound"
        );
        ensure!(
            (1..=MAX_RUN_BYTES).contains(&run_bytes),
            "stream shared byte bound"
        );
        ensure!(
            (1..=MAX_IDLE_SECONDS).contains(&idle_seconds),
            "stream idle bound"
        );
        Ok(Self {
            connection_bytes,
            initial: run_bytes,
            remaining: AtomicU64::new(run_bytes),
            idle: Duration::from_secs(idle_seconds),
            cancel,
        })
    }
    pub fn remaining(&self) -> u64 {
        self.remaining.load(Ordering::Acquire)
    }
    pub fn consumed(&self) -> u64 {
        self.initial - self.remaining()
    }
    fn running(&self) -> Result<()> {
        ensure!(
            !self.cancel.load(Ordering::Acquire),
            "stream relay cancelled"
        );
        Ok(())
    }
    fn charge(&self, bytes: u64, connection_count: &mut u64) -> Result<()> {
        *connection_count = connection_count
            .checked_add(bytes)
            .context("stream byte count overflow")?;
        ensure!(
            *connection_count <= self.connection_bytes,
            "stream connection byte limit"
        );
        self.remaining
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |left| {
                left.checked_sub(bytes)
            })
            .map_err(|_| anyhow::anyhow!("stream shared byte limit"))?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectionReceipt {
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub eof: bool,
    pub write_shutdown: bool,
    pub pending_bytes: u64,
}

/// Every started relay returns its actual counters, including failed relays.
/// A limit-rejected read is counted but is not admitted or forwarded.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamReceipt {
    pub left_to_right: DirectionReceipt,
    pub right_to_left: DirectionReceipt,
    pub charged_bytes: u64,
    pub elapsed_ms: u64,
    pub completed: bool,
    pub error: Option<String>,
}
impl StreamReceipt {
    pub fn require_completed(&self) -> Result<()> {
        ensure!(
            self.completed && self.error.is_none(),
            "stream relay did not complete: {}",
            self.error
                .as_deref()
                .unwrap_or("both half-closes were not observed")
        );
        Ok(())
    }
}

struct Direction {
    bytes: [u8; 16384],
    start: usize,
    end: usize,
    eof: bool,
    shutdown: bool,
    read: u64,
    written: u64,
    charged: u64,
}
impl Direction {
    fn new() -> Self {
        Self {
            bytes: [0; 16384],
            start: 0,
            end: 0,
            eof: false,
            shutdown: false,
            read: 0,
            written: 0,
            charged: 0,
        }
    }
    fn events(&self) -> i16 {
        if !self.eof && self.start == self.end {
            libc::POLLIN
        } else {
            0
        }
    }
    fn pending(&self) -> bool {
        self.start < self.end
    }
    fn receipt(&self) -> DirectionReceipt {
        DirectionReceipt {
            bytes_read: self.read,
            bytes_written: self.written,
            eof: self.eof,
            write_shutdown: self.shutdown,
            pending_bytes: (self.end - self.start) as u64,
        }
    }
    fn read(&mut self, fd: RawFd, budget: &ByteBudget, count: &mut u64) -> Result<bool> {
        let n = unsafe { libc::read(fd, self.bytes.as_mut_ptr().cast(), self.bytes.len()) };
        if n < 0 {
            return io_retry();
        }
        if n == 0 {
            self.eof = true;
            return Ok(false);
        }
        self.read = self
            .read
            .checked_add(n as u64)
            .context("stream direction count overflow")?;
        budget.charge(n as u64, count)?;
        self.charged += n as u64;
        self.start = 0;
        self.end = n as usize;
        Ok(true)
    }
    fn write(&mut self, fd: RawFd) -> Result<bool> {
        let n = unsafe {
            libc::send(
                fd,
                self.bytes[self.start..self.end].as_ptr().cast(),
                self.end - self.start,
                libc::MSG_NOSIGNAL,
            )
        };
        if n < 0 {
            return io_retry();
        }
        ensure!(n > 0, "stream write made no progress");
        self.start += n as usize;
        self.written += n as u64;
        Ok(true)
    }
    fn close_write(&mut self, fd: RawFd) -> Result<()> {
        if self.eof && !self.pending() && !self.shutdown {
            ensure!(
                unsafe { libc::shutdown(fd, libc::SHUT_WR) } == 0,
                "stream half-close failed: {}",
                std::io::Error::last_os_error()
            );
            self.shutdown = true;
        }
        Ok(())
    }
}

pub(crate) fn io_retry() -> Result<bool> {
    let error = std::io::Error::last_os_error();
    if matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
    ) {
        Ok(false)
    } else {
        Err(error.into())
    }
}

fn prepare(file: &File) -> Result<()> {
    ensure!(
        file.metadata()?.file_type().is_socket(),
        "stream endpoint is not a socket"
    );
    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    let descriptor = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    ensure!(
        flags >= 0
            && descriptor >= 0
            && unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == 0
            && unsafe { libc::fcntl(fd, libc::F_SETFD, descriptor | libc::FD_CLOEXEC) } == 0,
        "stream descriptor setup failed: {}",
        std::io::Error::last_os_error()
    );
    Ok(())
}

/// Take two connected stream sockets. No provided descriptor enters a guest.
/// The caller establishes stream type, peer provenance, connection count, and
/// overall lifetime. This checks socket file types and sets nonblocking I/O.
/// Errors before I/O return Err. After I/O starts, inspect require_completed()
/// and retain the receipt even on failure. Cancellation is checked every poll.
pub fn relay(left: OwnedFd, right: OwnedFd, budget: &ByteBudget) -> Result<StreamReceipt> {
    let left = File::from(left);
    let right = File::from(right);
    prepare(&left)?;
    prepare(&right)?;
    let mut forward = Direction::new();
    let mut reverse = Direction::new();
    let began = Instant::now();
    let result = pump(
        left.as_raw_fd(),
        right.as_raw_fd(),
        budget,
        &mut forward,
        &mut reverse,
    );
    Ok(StreamReceipt {
        left_to_right: forward.receipt(),
        right_to_left: reverse.receipt(),
        charged_bytes: forward.charged + reverse.charged,
        elapsed_ms: began.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        completed: result.is_ok() && forward.shutdown && reverse.shutdown,
        error: result.err().map(|error| format!("{error:#}")),
    })
}

fn pump(
    left: RawFd,
    right: RawFd,
    budget: &ByteBudget,
    forward: &mut Direction,
    reverse: &mut Direction,
) -> Result<()> {
    let mut count = 0;
    let mut activity = Instant::now();
    while !(forward.shutdown && reverse.shutdown) {
        budget.running()?;
        ensure!(activity.elapsed() < budget.idle, "stream idle deadline");
        let events = [
            forward.events() | if reverse.pending() { libc::POLLOUT } else { 0 },
            reverse.events() | if forward.pending() { libc::POLLOUT } else { 0 },
        ];
        let mut items = [
            libc::pollfd {
                fd: if events[0] == 0 { -1 } else { left },
                events: events[0],
                revents: 0,
            },
            libc::pollfd {
                fd: if events[1] == 0 { -1 } else { right },
                events: events[1],
                revents: 0,
            },
        ];
        let status = unsafe { libc::poll(items.as_mut_ptr(), 2, 100) };
        if status < 0 {
            io_retry()?;
            continue;
        }
        // A cancellation can arrive while poll sleeps. Check it before
        // consuming or forwarding the newly ready bytes.
        budget.running()?;
        ensure!(
            items.iter().all(|p| p.revents & libc::POLLNVAL == 0),
            "stream descriptor invalid"
        );
        let mut changed = false;
        for (index, source, dest, direction) in [
            (0, left, right, &mut *forward),
            (1, right, left, &mut *reverse),
        ] {
            if direction.events() != 0
                && items[index].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0
            {
                changed |= direction.read(source, budget, &mut count)?;
            }
            if direction.pending()
                && items[1 - index].revents & (libc::POLLOUT | libc::POLLERR | libc::POLLHUP) != 0
            {
                changed |= direction.write(dest)?;
            }
            direction.close_write(dest)?;
        }
        if changed {
            activity = Instant::now();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::Shutdown,
        os::unix::net::UnixStream,
        thread,
    };

    fn budget(connection: u64, run: u64) -> ByteBudget {
        ByteBudget::new(connection, run, 1, Arc::new(AtomicBool::new(false))).unwrap()
    }
    fn pair() -> (File, OwnedFd) {
        let (peer, socket) = UnixStream::pair().unwrap();
        (File::from(OwnedFd::from(peer)), socket.into())
    }
    #[test]
    fn policy_rejects_zero_and_excessive_limits() {
        for (connection, run, idle) in [
            (0, 1, 1),
            (1, 0, 1),
            (1, 1, 0),
            (MAX_CONNECTION_BYTES + 1, 1, 1),
            (1, MAX_RUN_BYTES + 1, 1),
            (1, 1, MAX_IDLE_SECONDS + 1),
        ] {
            assert!(
                ByteBudget::new(connection, run, idle, Arc::new(AtomicBool::new(false))).is_err()
            );
        }
    }
    #[test]
    fn limit_rejected_actual_reads_are_retained_without_forwarding() {
        for (connection, run) in [(3, 8), (8, 3)] {
            let limits = budget(connection, run);
            let (mut sender, left) = pair();
            let (_receiver, right) = pair();
            sender.write_all(b"abcd").unwrap();
            let receipt = relay(left, right, &limits).unwrap();
            assert!(receipt.require_completed().is_err());
            assert!(receipt.error.as_ref().unwrap().contains("byte limit"));
            assert_eq!(receipt.left_to_right.bytes_read, 4);
            assert_eq!(receipt.left_to_right.bytes_written, 0);
            assert_eq!(receipt.charged_bytes, 0);
            assert_eq!(limits.consumed(), 0);
        }
    }
    #[test]
    fn shared_admission_cannot_overdraw_under_concurrency() {
        let limits = Arc::new(budget(1024, 1000));
        let jobs: Vec<_> = (0..16)
            .map(|_| {
                let limits = limits.clone();
                thread::spawn(move || {
                    let mut count = 0;
                    let mut admitted = 0;
                    for _ in 0..100 {
                        if limits.charge(1, &mut count).is_ok() {
                            admitted += 1;
                        }
                    }
                    admitted
                })
            })
            .collect();
        assert_eq!(
            jobs.into_iter().map(|job| job.join().unwrap()).sum::<u64>(),
            1000
        );
        assert_eq!(limits.remaining(), 0);
        assert_eq!(limits.consumed(), 1000);
    }
    #[test]
    fn cancellation_and_idle_deadline_retain_empty_receipts() {
        for cancelled in [false, true] {
            let signal = Arc::new(AtomicBool::new(cancelled));
            let limits = ByteBudget::new(32, 32, 1, signal).unwrap();
            let (_sender, left) = pair();
            let (_receiver, right) = pair();
            let receipt = relay(left, right, &limits).unwrap();
            assert!(receipt.require_completed().is_err());
            assert_eq!(receipt.charged_bytes, 0);
            assert_eq!(receipt.left_to_right.bytes_read, 0);
            assert!(receipt.error.as_ref().unwrap().contains(if cancelled {
                "cancelled"
            } else {
                "idle deadline"
            }));
            if !cancelled {
                assert!(receipt.elapsed_ms >= 900);
            }
        }
    }
    #[test]
    fn regular_file_endpoint_is_rejected_before_io() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("untouched");
        std::fs::write(&path, b"original").unwrap();
        let (_peer, right) = pair();
        assert!(relay(File::open(&path).unwrap().into(), right, &budget(32, 32)).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"original");
    }

    #[test]
    fn cancellation_while_waiting_rejects_newly_ready_bytes() {
        let signal = Arc::new(AtomicBool::new(false));
        let limits = Arc::new(ByteBudget::new(32, 32, 1, signal.clone()).unwrap());
        let (mut sender, left) = pair();
        let (_receiver, right) = pair();
        let worker_limits = limits.clone();
        let worker = thread::spawn(move || relay(left, right, &worker_limits).unwrap());
        thread::sleep(Duration::from_millis(30));
        signal.store(true, Ordering::Release);
        if let Err(error) = sender.write_all(b"late") {
            assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
        }
        let receipt = worker.join().unwrap();
        assert!(receipt.error.as_ref().unwrap().contains("cancelled"));
        assert_eq!(receipt.left_to_right.bytes_read, 0);
        assert_eq!(receipt.left_to_right.bytes_written, 0);
        assert_eq!(limits.consumed(), 0);
    }

    fn random_bytes(count: usize) -> Vec<u8> {
        let mut bytes = vec![0; count];
        File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut bytes)
            .unwrap();
        bytes
    }

    fn vm_pair() -> (UnixStream, OwnedFd) {
        let (peer, endpoint) = UnixStream::pair().unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        (peer, endpoint.into())
    }

    fn duplex_vm() {
        let (mut left_peer, left) = vm_pair();
        let (mut right_peer, right) = vm_pair();
        let sent_left = random_bytes(2 << 20);
        let sent_right = random_bytes(1 << 20);
        let limits = Arc::new(budget(8 << 20, 8 << 20));
        let relay_budget = limits.clone();
        let relay = thread::spawn(move || super::relay(left, right, &relay_budget).unwrap());
        thread::scope(|scope| {
            let mut left_writer = left_peer.try_clone().unwrap();
            let mut right_writer = right_peer.try_clone().unwrap();
            let left_bytes = &sent_left;
            let right_bytes = &sent_right;
            let a = scope.spawn(move || {
                left_writer.write_all(left_bytes).unwrap();
                left_writer.shutdown(Shutdown::Write).unwrap();
            });
            let b = scope.spawn(move || {
                right_writer.write_all(right_bytes).unwrap();
                right_writer.shutdown(Shutdown::Write).unwrap();
            });
            // Both peers withhold reads while writes exceed socket buffers.
            thread::sleep(Duration::from_millis(150));
            let reverse = scope.spawn(|| {
                let mut received = Vec::new();
                left_peer.read_to_end(&mut received).unwrap();
                received
            });
            let mut received = Vec::new();
            right_peer.read_to_end(&mut received).unwrap();
            assert_eq!(received, sent_left);
            assert_eq!(reverse.join().unwrap(), sent_right);
            a.join().unwrap();
            b.join().unwrap();
        });
        let result = relay.join().unwrap();
        result.require_completed().unwrap();
        assert_eq!(result.left_to_right.bytes_read, sent_left.len() as u64);
        assert_eq!(result.left_to_right.bytes_written, sent_left.len() as u64);
        assert_eq!(result.right_to_left.bytes_read, sent_right.len() as u64);
        assert_eq!(result.right_to_left.bytes_written, sent_right.len() as u64);
        assert_eq!(
            result.left_to_right.pending_bytes + result.right_to_left.pending_bytes,
            0
        );
        assert_eq!(
            result.charged_bytes,
            (sent_left.len() + sent_right.len()) as u64
        );
        assert_eq!(result.charged_bytes, limits.consumed());
        println!(
            "MD_SIM_BYTE_STREAM_DUPLEX {}",
            serde_json::to_string(&result).unwrap()
        );
    }

    fn response_after_half_close_vm() {
        let (mut client, left) = vm_pair();
        let (mut server, right) = vm_pair();
        let request = random_bytes(4096);
        let response = random_bytes(8192);
        let relay =
            thread::spawn(move || super::relay(left, right, &budget(65536, 65536)).unwrap());
        thread::scope(|scope| {
            let service = scope.spawn(|| {
                let mut observed = Vec::new();
                server.read_to_end(&mut observed).unwrap();
                assert_eq!(observed, request);
                server.write_all(&response).unwrap();
                server.shutdown(Shutdown::Write).unwrap();
            });
            client.write_all(&request).unwrap();
            client.shutdown(Shutdown::Write).unwrap();
            let mut observed = Vec::new();
            client.read_to_end(&mut observed).unwrap();
            assert_eq!(observed, response);
            service.join().unwrap();
        });
        let result = relay.join().unwrap();
        result.require_completed().unwrap();
        assert_eq!(result.left_to_right.bytes_written, request.len() as u64);
        assert_eq!(result.right_to_left.bytes_written, response.len() as u64);
        println!(
            "MD_SIM_BYTE_STREAM_HALF_CLOSE {}",
            serde_json::to_string(&result).unwrap()
        );
    }

    fn partial_limit_vm() {
        for (connection, run) in [(3, 8), (8, 3)] {
            let (mut sender, left) = vm_pair();
            let (mut receiver, right) = vm_pair();
            let relay =
                thread::spawn(move || super::relay(left, right, &budget(connection, run)).unwrap());
            sender.write_all(b"ab").unwrap();
            let mut received = [0; 2];
            receiver.read_exact(&mut received).unwrap();
            assert_eq!(&received, b"ab");
            sender.write_all(b"cd").unwrap();
            let result = relay.join().unwrap();
            assert!(result.require_completed().is_err());
            assert!(result.error.as_ref().unwrap().contains("byte limit"));
            assert_eq!(result.left_to_right.bytes_read, 4);
            assert_eq!(result.left_to_right.bytes_written, 2);
            assert_eq!(result.charged_bytes, 2);
            println!(
                "MD_SIM_BYTE_STREAM_LIMIT {}",
                serde_json::to_string(&result).unwrap()
            );
        }
    }

    fn blocked_cancel_vm() {
        let (mut sender, left) = vm_pair();
        let (_blocked, right) = vm_pair();
        let cancel = Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let relay = thread::spawn(move || {
            super::relay(
                left,
                right,
                &ByteBudget::new(32 << 20, 32 << 20, 2, signal).unwrap(),
            )
            .unwrap()
        });
        let writer = thread::spawn(move || sender.write_all(&random_bytes(16 << 20)));
        thread::sleep(Duration::from_millis(250));
        cancel.store(true, Ordering::Release);
        let result = relay.join().unwrap();
        assert!(writer.join().unwrap().is_err());
        assert!(result.error.as_ref().unwrap().contains("cancelled"));
        assert!(!result.completed && result.elapsed_ms < 2000);
        assert!(result.left_to_right.bytes_read > 0);
        assert!(result.left_to_right.bytes_read >= result.left_to_right.bytes_written);
        println!(
            "MD_SIM_BYTE_STREAM_CANCEL {}",
            serde_json::to_string(&result).unwrap()
        );
    }

    #[test]
    #[ignore = "requires captured private Linux VM; host socket shutdown/send may be restricted"]
    fn vm_full_duplex_half_close_and_backpressure() {
        assert!(std::fs::read_to_string("/proc/cmdline")
            .unwrap()
            .split_whitespace()
            .any(|word| word == "md_sim_private_guest=1"));
        duplex_vm();
        response_after_half_close_vm();
        partial_limit_vm();
        blocked_cancel_vm();
    }
}

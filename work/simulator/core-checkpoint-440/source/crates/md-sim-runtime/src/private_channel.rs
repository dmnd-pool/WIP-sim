//! One private QEMU Ethernet carrier. The supervisor creates both socket pairs;
//! callers supply no host descriptor, address or listening socket.
//!
//! QEMU's existing carrier uses a four-byte big-endian packet length followed
//! by the Ethernet frame. EOF disconnects the whole carrier. TCP half-close is
//! carried within Ethernet packets and is not a half-close of this descriptor.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::{fs::FileTypeExt, net::UnixStream},
    },
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub const GUEST_FD: i32 = 3;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Protocol {
    #[serde(rename = "qemu-private-ethernet-v1")]
    QemuEthernetV1,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub protocol: Protocol,
    pub guest_fd: i32,
    /// Payload bytes, excluding the four-byte carrier header.
    pub max_frame_bytes: u32,
    /// Each direction has a separate allowance, including carrier headers.
    pub max_wire_bytes: u64,
    pub max_packets: u64,
}

impl Spec {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.guest_fd == GUEST_FD,
            "private Ethernet requires descriptor3"
        );
        ensure!(
            (14..=65535).contains(&self.max_frame_bytes),
            "Ethernet frame bound out of range"
        );
        ensure!(
            (18..=1 << 30).contains(&self.max_wire_bytes),
            "Ethernet byte bound out of range"
        );
        ensure!(
            (1..=1 << 24).contains(&self.max_packets),
            "Ethernet packet bound out of range"
        );
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectionReceipt {
    /// Observed wire bytes. A rejected next header can add at most four bytes
    /// beyond the accepted byte allowance; none of that frame is forwarded.
    pub observed_bytes: u64,
    pub observed_sha256: String,
    pub forwarded_bytes: u64,
    pub forwarded_sha256: String,
    pub received_packets: u64,
    pub forwarded_packets: u64,
    pub pending_bytes: usize,
    pub eof: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub policy: Spec,
    pub guest_to_peer: DirectionReceipt,
    pub peer_to_guest: DirectionReceipt,
    pub end: String,
    pub error: Option<String>,
    /// Kernel CPU accounting for this relay thread, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_cpu_microseconds: Option<u64>,
    /// Counts cover actual relay reads/writes, not unread kernel queues or a
    /// peer's application-level receipt. A stopped carrier is not a pcap file.
    pub scope: String,
}

struct Direction {
    buffer: Vec<u8>,
    wanted: usize,
    written: usize,
    observed_bytes: u64,
    observed_hash: Sha256,
    forwarded_bytes: u64,
    forwarded_hash: Sha256,
    received_packets: u64,
    forwarded_packets: u64,
    eof: bool,
}

impl Direction {
    fn new() -> Self {
        Self {
            buffer: Vec::new(),
            wanted: 4,
            written: 0,
            observed_bytes: 0,
            observed_hash: Sha256::new(),
            forwarded_bytes: 0,
            forwarded_hash: Sha256::new(),
            received_packets: 0,
            forwarded_packets: 0,
            eof: false,
        }
    }
    fn ready(&self) -> bool {
        self.wanted > 4 && self.buffer.len() == self.wanted
    }
    fn read(&mut self, source: &mut File, policy: &Spec) -> Result<()> {
        if self.ready() || self.eof {
            return Ok(());
        }
        let remaining = self.wanted - self.buffer.len();
        let mut chunk = [0u8; 8192];
        let capacity = remaining.min(chunk.len());
        let count = match source.read(&mut chunk[..capacity]) {
            Ok(count) => count,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                return Ok(())
            }
            Err(error) => return Err(error.into()),
        };
        if count == 0 {
            self.eof = true;
            ensure!(
                self.buffer.is_empty(),
                "Ethernet carrier ended in a partial frame"
            );
            return Ok(());
        }
        self.observed_bytes += count as u64;
        self.observed_hash.update(&chunk[..count]);
        self.buffer.extend_from_slice(&chunk[..count]);
        if self.buffer.len() == 4 && self.wanted == 4 {
            let size = u32::from_be_bytes(self.buffer[..4].try_into().expect("four-byte prefix"));
            ensure!(
                (14..=policy.max_frame_bytes).contains(&size),
                "invalid or oversized Ethernet frame"
            );
            ensure!(
                self.received_packets < policy.max_packets,
                "Ethernet packet limit exceeded"
            );
            let wire = size as u64 + 4;
            // No new frame is read until the previous one was fully written.
            ensure!(
                wire <= policy.max_wire_bytes - self.forwarded_bytes,
                "Ethernet byte limit exceeded"
            );
            self.wanted = wire as usize;
        }
        if self.ready() {
            self.received_packets += 1;
        }
        Ok(())
    }
    fn write(&mut self, destination: &mut File) -> Result<()> {
        if !self.ready() {
            return Ok(());
        }
        let bytes = &self.buffer[self.written..];
        let count = match destination.write(bytes) {
            Ok(count) => count,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                return Ok(())
            }
            Err(error) => return Err(error.into()),
        };
        ensure!(count != 0, "Ethernet carrier write returned zero");
        self.forwarded_hash.update(&bytes[..count]);
        self.forwarded_bytes += count as u64;
        self.written += count;
        if self.written == self.wanted {
            self.forwarded_packets += 1;
            self.buffer.clear();
            self.wanted = 4;
            self.written = 0;
        }
        Ok(())
    }
    fn receipt(self) -> DirectionReceipt {
        DirectionReceipt {
            observed_bytes: self.observed_bytes,
            observed_sha256: hex::encode(self.observed_hash.finalize()),
            forwarded_bytes: self.forwarded_bytes,
            forwarded_sha256: hex::encode(self.forwarded_hash.finalize()),
            received_packets: self.received_packets,
            forwarded_packets: self.forwarded_packets,
            pending_bytes: self.buffer.len() - self.written,
            eof: self.eof,
        }
    }
}

fn bridge(
    mut guest: File,
    mut peer: File,
    policy: Spec,
    stop: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    deadline: Option<Instant>,
) -> Receipt {
    let cpu_started = thread_cpu_microseconds();
    let mut outward = Direction::new();
    let mut inward = Direction::new();
    let result = (|| -> Result<&str> {
        loop {
            if stop.load(Ordering::Acquire) {
                return Ok("supervisor-closed");
            }
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                return Ok("timed-out");
            }
            let mut events = [
                libc::pollfd {
                    fd: guest.as_raw_fd(),
                    events: if outward.ready() { 0 } else { libc::POLLIN }
                        | if inward.ready() { libc::POLLOUT } else { 0 },
                    revents: 0,
                },
                libc::pollfd {
                    fd: peer.as_raw_fd(),
                    events: if inward.ready() { 0 } else { libc::POLLIN }
                        | if outward.ready() { libc::POLLOUT } else { 0 },
                    revents: 0,
                },
            ];
            // poll reports HUP even for an events mask of zero. A complete
            // pending frame can disable reads while its destination is full.
            // Ignore that descriptor until an I/O operation is possible, so a
            // closed source cannot spin a host CPU under backpressure.
            for event in &mut events {
                if event.events == 0 {
                    event.fd = -1;
                }
            }
            let ready =
                unsafe { libc::poll(events.as_mut_ptr(), events.len() as libc::nfds_t, 20) };
            if ready < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error.into());
            }
            ensure!(
                events
                    .iter()
                    .all(|event| event.revents & libc::POLLNVAL == 0),
                "private Ethernet descriptor invalid"
            );
            if stop.load(Ordering::Acquire) {
                return Ok("supervisor-closed");
            }
            // Attempt bounded nonblocking I/O after poll. POLLHUP can arrive
            // with final readable bytes; read() establishes the actual EOF.
            outward.write(&mut peer)?;
            inward.write(&mut guest)?;
            outward.read(&mut guest, &policy)?;
            inward.read(&mut peer, &policy)?;
            if outward.eof {
                return Ok("guest-eof");
            }
            if inward.eof {
                return Ok("peer-eof");
            }
        }
    })();
    if result.is_err() {
        failed.store(true, Ordering::Release);
    }
    let (end, error) = match result {
        Ok(end) => (end.into(), None),
        Err(error) => ("failed".into(), Some(format!("{error:#}"))),
    };
    let relay_cpu_microseconds = cpu_started
        .zip(thread_cpu_microseconds())
        .and_then(|(start, end)| end.checked_sub(start));
    Receipt {policy,guest_to_peer:outward.receipt(),peer_to_guest:inward.receipt(),end,error,relay_cpu_microseconds,
        scope:"Exact relay reads and writes on private socket pairs. Whole-carrier EOF; no general byte-stream half-close or application delivery claim.".into()}
}

fn thread_cpu_microseconds() -> Option<u64> {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    if unsafe { libc::getrusage(libc::RUSAGE_THREAD, &mut usage) } != 0 {
        return None;
    }
    ((usage.ru_utime.tv_sec as i128 + usage.ru_stime.tv_sec as i128) * 1_000_000
        + usage.ru_utime.tv_usec as i128
        + usage.ru_stime.tv_usec as i128)
        .try_into()
        .ok()
}

/// Relay two owned socket endpoints in a trusted provider. These descriptors
/// never enter an isolated guest. The caller must prove their private origin
/// and topology, and must not retain competing readers or writers. Socket
/// type checks do not establish the peer's identity or network isolation.
/// The relay uses the same framing and caps as execution, plus this explicit
/// wall-time bound. A raised stop flag returns `end="cancelled"`.
pub fn relay_provider(
    guest: File,
    peer: File,
    policy: Spec,
    stop: Arc<AtomicBool>,
    wall_seconds: u64,
) -> Result<Receipt> {
    policy.validate()?;
    ensure!(
        (1..=86400).contains(&wall_seconds),
        "provider wall-time limit out of range"
    );
    for file in [&guest, &peer] {
        ensure!(
            file.metadata()?.file_type().is_socket(),
            "provider endpoint must be a socket"
        );
        let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
        ensure!(flags >= 0, "provider descriptor flags unavailable");
        ensure!(
            unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } == 0,
            "provider descriptor cannot become nonblocking"
        );
    }
    let mut receipt = bridge(
        guest,
        peer,
        policy,
        stop,
        Arc::new(AtomicBool::new(false)),
        Some(Instant::now() + Duration::from_secs(wall_seconds)),
    );
    if receipt.end == "supervisor-closed" {
        receipt.end = "cancelled".into();
    }
    receipt.scope = "Trusted provider relay reads and writes; socket types checked; endpoint provenance, peer identity, topology, and application delivery require separate evidence.".into();
    Ok(receipt)
}

pub(crate) struct Session {
    guest: Option<File>,
    stop: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    join: Option<JoinHandle<Receipt>>,
}

impl Session {
    pub(crate) fn new(policy: &Spec, provider: SyncSender<File>) -> Result<Self> {
        policy.validate()?;
        let (guest, relay_guest) = UnixStream::pair()?;
        let (peer, relay_peer) = UnixStream::pair()?;
        relay_guest.set_nonblocking(true)?;
        relay_peer.set_nonblocking(true)?;
        // Reserve a source descriptor above standard I/O and Rust's child
        // status pipes. It remains CLOEXEC until duplicated explicitly to3.
        let fd = unsafe { libc::fcntl(guest.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 64) };
        ensure!(
            fd >= 0,
            "reserve private Ethernet descriptor: {}",
            std::io::Error::last_os_error()
        );
        let guest = unsafe { File::from_raw_fd(fd) };
        let peer: OwnedFd = peer.into();
        provider
            .try_send(File::from(peer))
            .context("private Ethernet provider is unavailable or not ready")?;
        let relay_guest: OwnedFd = relay_guest.into();
        let relay_peer: OwnedFd = relay_peer.into();
        let stop = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let (policy, thread_stop, thread_failed) = (policy.clone(), stop.clone(), failed.clone());
        let join = thread::spawn(move || {
            bridge(
                File::from(relay_guest),
                File::from(relay_peer),
                policy,
                thread_stop,
                thread_failed,
                None,
            )
        });
        Ok(Self {
            guest: Some(guest),
            stop,
            failed,
            join: Some(join),
        })
    }
    pub(crate) fn guest_fd(&self) -> RawFd {
        self.guest
            .as_ref()
            .expect("guest descriptor before launch")
            .as_raw_fd()
    }
    pub(crate) fn launched(&mut self) {
        self.guest.take();
    }
    pub(crate) fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
    pub(crate) fn finish(mut self) -> Result<Receipt> {
        self.stop.store(true, Ordering::Release);
        self.guest.take();
        self.join
            .take()
            .expect("private Ethernet relay")
            .join()
            .map_err(|_| anyhow::anyhow!("private Ethernet relay panicked"))
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.guest.take();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

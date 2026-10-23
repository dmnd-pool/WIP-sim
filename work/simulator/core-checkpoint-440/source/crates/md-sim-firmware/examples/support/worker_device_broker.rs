//! Persistent VM-side worker bridge. SSH clients own only short Unix socket
//! connections; the broker owns the native worker transport across their loss.
//! Whole-VM loss and an uncertain native channel remain separate failure cases.
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::WorkerTransport;
use md_sim_runtime::worker::{relay_request, CompletedWorkerError, Request, MAX_RELAY_REQUESTS};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::{
            fs::{FileTypeExt, MetadataExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const IO_SECONDS: u64 = 5;
const AUDIT_BYTES: usize = 8 << 20;

struct Endpoint {
    listener: UnixListener,
    path: PathBuf,
    device: u64,
    inode: u64,
    owner: u32,
}

impl Endpoint {
    fn bind(path: &Path) -> Result<Self> {
        ensure!(path.is_absolute(), "broker socket path must be absolute");
        let parent = path.parent().context("broker socket parent absent")?;
        let metadata = fs::symlink_metadata(parent)?;
        let owner = unsafe { libc::geteuid() };
        ensure!(
            metadata.is_dir() && metadata.uid() == owner && metadata.mode() & 0o077 == 0,
            "broker socket needs an owner-only real parent directory"
        );
        // Canonicalize the parent once. Never remove a pre-existing endpoint.
        let path = parent
            .canonicalize()?
            .join(path.file_name().context("broker socket name absent")?);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
            Ok(_) => anyhow::bail!("broker endpoint already exists; retained"),
        }
        let listener = UnixListener::bind(&path)?;
        let metadata = fs::symlink_metadata(&path)?;
        let endpoint = Self {
            listener,
            path,
            device: metadata.dev(),
            inode: metadata.ino(),
            owner,
        };
        fs::set_permissions(&endpoint.path, fs::Permissions::from_mode(0o600))?;
        endpoint.listener.set_nonblocking(true)?;
        Ok(endpoint)
    }

    fn owns_path(&self) -> bool {
        fs::symlink_metadata(&self.path).is_ok_and(|metadata| {
            metadata.file_type().is_socket()
                && metadata.uid() == self.owner
                && metadata.dev() == self.device
                && metadata.ino() == self.inode
        })
    }

    fn close(self) -> Result<()> {
        ensure!(self.owns_path(), "broker endpoint changed; retained");
        fs::remove_file(&self.path)?;
        File::open(self.path.parent().context("broker parent absent")?)?.sync_all()?;
        Ok(())
    }
}

impl Drop for Endpoint {
    fn drop(&mut self) {
        if self.owns_path() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn peer_owner(stream: &UnixStream, expected: u32) -> Result<u32> {
    let mut credentials = libc::ucred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut size = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    ensure!(
        unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credentials as *mut libc::ucred).cast(),
                &mut size,
            )
        } == 0
            && size as usize == std::mem::size_of::<libc::ucred>(),
        "cannot verify broker peer identity: {}",
        io::Error::last_os_error()
    );
    ensure!(
        credentials.uid == expected,
        "broker peer does not own this endpoint"
    );
    Ok(credentials.pid as u32)
}

/// Separate instances give the input and output phases independent deadlines.
/// Plain read/write supports hosts that deny UnixStream recv/send operations.
struct TimedFile {
    file: File,
    deadline: Option<Instant>,
}

impl TimedFile {
    fn new(stream: UnixStream) -> Result<Self> {
        stream.set_nonblocking(true)?;
        let descriptor: OwnedFd = stream.into();
        Ok(Self {
            file: File::from(descriptor),
            deadline: None,
        })
    }

    fn wait(&mut self, events: i16) -> io::Result<()> {
        let deadline = *self
            .deadline
            .get_or_insert_with(|| Instant::now() + Duration::from_secs(IO_SECONDS));
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let mut event = libc::pollfd {
                fd: self.file.as_raw_fd(),
                events,
                revents: 0,
            };
            let ready =
                unsafe { libc::poll(&mut event, 1, remaining.as_millis().clamp(1, 100) as i32) };
            if ready < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error);
            }
            if ready != 0 {
                return Ok(());
            }
        }
    }
}

impl Read for TimedFile {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            self.wait(libc::POLLIN)?;
            match self.file.read(buffer) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                result => return result,
            }
        }
    }
}
impl Write for TimedFile {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        loop {
            self.wait(libc::POLLOUT)?;
            match self.file.write(buffer) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn audit(file: &mut File, total: &mut usize, event: Value) -> Result<()> {
    let mut bytes = serde_json::to_vec(&event)?;
    bytes.push(b'\n');
    *total = total
        .checked_add(bytes.len())
        .context("broker audit size overflow")?;
    ensure!(*total <= AUDIT_BYTES, "broker audit limit reached");
    file.write_all(&bytes)?;
    Ok(())
}

/// The production transport owns the checked virtual character device. This
/// service must outlive individual SSH relay processes and client namespaces.
/// Cancellation is a broker/service stop, not a request to cancel one miner.
pub fn serve<T: WorkerTransport>(
    socket: &Path,
    mut transport: T,
    cancel: Arc<AtomicBool>,
    mut log: File,
) -> Result<Value> {
    let endpoint = Endpoint::bind(socket)?;
    let mut count = 0u32;
    let mut forwarded = 0u32;
    let mut rejected = 0u32;
    let mut undelivered = 0u32;
    let mut audit_bytes = 0usize;
    audit(
        &mut log,
        &mut audit_bytes,
        json!({"event":"ready","owner":endpoint.owner,
        "socket":endpoint.path,"transport":transport.description(),"io_seconds":IO_SECONDS,
        "connection_limit":MAX_RELAY_REQUESTS,"audit_limit":AUDIT_BYTES}),
    )?;
    let mut stopped = false;
    while !cancel.load(Ordering::Acquire) {
        ensure!(
            count < MAX_RELAY_REQUESTS,
            "broker connection limit reached"
        );
        let stream = match endpoint.listener.accept() {
            Ok((stream, _)) => stream,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        };
        count += 1;
        let peer = match peer_owner(&stream, endpoint.owner) {
            Ok(peer) => peer,
            Err(error) => {
                rejected += 1;
                audit(
                    &mut log,
                    &mut audit_bytes,
                    json!({"event":"peer-rejected","connection":count,
                    "error":format!("{error:#}")}),
                )?;
                continue;
            }
        };
        let input = TimedFile::new(stream.try_clone()?)?;
        let output = TimedFile::new(stream)?;
        let mut native_failed = false;
        let mut dispatched = false;
        let mut operation = Value::Null;
        let delivered = relay_request(input, output, |request| {
            operation = serde_json::to_value(request)?["operation"].clone();
            dispatched = true;
            forwarded += 1;
            // Do not inspect local client liveness here. A response must be
            // consumed fully before this native channel can carry another call.
            let response = transport.request(request);
            native_failed = response
                .as_ref()
                .is_err_and(|error| error.downcast_ref::<CompletedWorkerError>().is_none());
            stopped = matches!(request, Request::Shutdown) && response.is_ok();
            response
        });
        if delivered.is_err() {
            undelivered += 1;
        }
        audit(
            &mut log,
            &mut audit_bytes,
            json!({"event":"exchange","connection":count,"peer_pid":peer,
            "operation":operation,"forwarded":dispatched,"reply_delivered":delivered.is_ok(),
            "native_failed":native_failed,"shutdown":stopped}),
        )?;
        ensure!(
            !native_failed,
            "native worker exchange failed; broker transport is closed"
        );
        if stopped {
            break;
        }
    }
    log.sync_all()?;
    endpoint.close()?;
    Ok(
        json!({"connections":count,"forwarded":forwarded,"rejected_peers":rejected,
        "undelivered_replies":undelivered,"worker_shutdown_acknowledged":stopped,
        "cancelled":cancel.load(Ordering::Acquire),"audit_bytes":audit_bytes,
        "scope":"Persistent VM-side owner-only worker broker; whole-VM EOF survival is not established"}),
    )
}

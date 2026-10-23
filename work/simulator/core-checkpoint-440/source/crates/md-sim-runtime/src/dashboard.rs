//! A run-owned incoming byte bridge. Only one Unix socket is mounted into
//! the isolated process. No host directory or network namespace is shared.
use crate::byte_stream::{io_retry, ByteBudget};
use crate::{isolation::save_json, store::ObjectId};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::Read,
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::{
            fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::{UnixListener, UnixStream},
        },
    },
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub const SOCKET_PATH: &str = "/md-sim-dashboard.sock";
pub const TOKEN_ENV: &str = "MD_SIM_DASHBOARD_BRIDGE_TOKEN";
pub const PORT_ENV: &str = "MD_SIM_DASHBOARD_BRIDGE_PORT";
const CONNECTIONS: usize = 8;
const CONNECTION_BYTES: u64 = 64 << 20;
const RUN_BYTES: u64 = 256 << 20;

/// Check the sole optional incoming socket before an emulator starts. The
/// runtime owns its environment and mount; this does not establish HTTP health.
pub fn guest_mount_expected() -> Result<bool> {
    let token = std::env::var(TOKEN_ENV).ok();
    let port = std::env::var(PORT_ENV).ok();
    if token.is_none() && port.is_none() {
        return Ok(false);
    }
    validate_agent_environment(token.as_deref(), port.as_deref())?;
    validate_guest_socket(Path::new(SOCKET_PATH))?;
    Ok(true)
}

fn validate_guest_socket(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.file_type().is_socket()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.nlink() == 1
            && metadata.mode() & 0o777 == 0o600,
        "dashboard mount is not the private runtime-owned socket"
    );
    Ok(())
}

fn validate_agent_environment(token: Option<&str>, port: Option<&str>) -> Result<u16> {
    let token = token.context("partial dashboard bridge environment")?;
    let port: u16 = port
        .context("partial dashboard bridge environment")?
        .parse()?;
    ensure!(
        token.len() == 64
            && token
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && port >= 1024,
        "dashboard bridge environment invalid"
    );
    Ok(port)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    Native,
    AdaptedNative,
    SimulatorConsole,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub source: Source,
    /// Only the isolated namespace's IPv4 loopback can be reached.
    pub guest_port: u16,
    /// Zero selects a fresh worker-loopback port.
    pub listen_port: u16,
}
impl Spec {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.guest_port >= 1024,
            "dashboard guest port must be unprivileged"
        );
        ensure!(
            self.listen_port == 0 || self.listen_port >= 1024,
            "dashboard worker port must be unprivileged"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub process_identity: ObjectId,
    pub source: Source,
    pub listen: SocketAddr,
    /// A unique run-owned upstream for a worker-local HTTP/TLS gateway.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_socket: Option<std::path::PathBuf>,
    pub guest_port: u16,
    pub connection_limit: usize,
    pub connection_byte_limit: u64,
    pub run_byte_limit: u64,
    pub firmware_readiness_established: bool,
}

impl Binding {
    /// Validate a live endpoint against the complete selected process, not a
    /// caller-supplied run label or the hash of the firmware image alone.
    pub fn verify(&self, spec: &Spec, process: &impl Serialize) -> Result<()> {
        spec.validate()?;
        if let Some(path) = &self.local_socket {
            ensure!(
                path.is_absolute()
                    && path.file_name().is_some_and(|s| s == "http.sock")
                    && path
                        .parent()
                        .and_then(Path::file_name)
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| s.starts_with("md-sim-dashboard-") && s.len() > 17)
                    && path.components().all(|c| matches!(
                        c,
                        std::path::Component::RootDir | std::path::Component::Normal(_)
                    )),
                "dashboard local socket path invalid"
            );
        }
        let bytes = serde_json::to_vec(process)?;
        ensure!(
            self.process_identity
                == ObjectId {
                    sha256: hex::encode(Sha256::digest(&bytes)),
                    size: bytes.len() as u64,
                },
            "dashboard process identity differs"
        );
        ensure!(
            self.source == spec.source
                && self.guest_port == spec.guest_port
                && self.listen.ip() == std::net::IpAddr::V4(Ipv4Addr::LOCALHOST)
                && self.listen.port() >= 1024
                && (spec.listen_port == 0 || self.listen.port() == spec.listen_port)
                && self.connection_limit == CONNECTIONS
                && self.connection_byte_limit == CONNECTION_BYTES
                && self.run_byte_limit == RUN_BYTES
                && !self.firmware_readiness_established,
            "dashboard endpoint differs from the selected process policy"
        );
        Ok(())
    }

    pub fn read(path: &Path, spec: &Spec, process: &impl Serialize) -> Result<Self> {
        // Nonblocking also prevents a substituted FIFO from blocking status.
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        let metadata = file.metadata()?;
        ensure!(
            metadata.is_file() && metadata.nlink() == 1 && metadata.len() <= 65536,
            "unsafe or oversized dashboard receipt"
        );
        let mut bytes = Vec::new();
        file.take(65537).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 65536, "dashboard receipt grew beyond bound");
        let binding: Self = serde_json::from_slice(&bytes)?;
        binding.verify(spec, process)?;
        Ok(binding)
    }
}

struct State {
    stop: Arc<AtomicBool>,
    budget: ByteBudget,
    completed: AtomicU64,
    failed: AtomicU64,
    empty: AtomicU64,
    infrastructure_failed: AtomicBool,
}
impl State {
    fn new() -> Arc<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        Arc::new(Self {
            stop: stop.clone(),
            budget: ByteBudget::new(CONNECTION_BYTES, RUN_BYTES, 60, stop)
                .expect("fixed dashboard stream limits"),
            completed: AtomicU64::new(0),
            failed: AtomicU64::new(0),
            empty: AtomicU64::new(0),
            infrastructure_failed: AtomicBool::new(false),
        })
    }
    fn running(&self) -> Result<()> {
        ensure!(
            !self.stop.load(Ordering::Acquire),
            "dashboard bridge stopped"
        );
        Ok(())
    }
}

/// Supervisor side. Dropping it closes only this run's endpoint and joins all
/// bounded handlers. The secret control token never enters the result record.
pub struct Server {
    directory: tempfile::TempDir,
    pub binding: Binding,
    token: String,
    state: Arc<State>,
    join: Option<JoinHandle<()>>,
}
fn private_directory() -> Result<tempfile::TempDir> {
    Ok(tempfile::Builder::new()
        .prefix("md-sim-dashboard-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()?)
}
impl Server {
    pub fn start(spec: &Spec, process: &impl Serialize, receipt: &Path) -> Result<Self> {
        spec.validate()?;
        let directory = private_directory()?;
        let socket = directory.path().join("bridge.sock");
        let private = UnixListener::bind(&socket)?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
        private.set_nonblocking(true)?;
        let upstream = directory.path().join("http.sock");
        let published = UnixListener::bind(&upstream)?;
        fs::set_permissions(&upstream, fs::Permissions::from_mode(0o600))?;
        published.set_nonblocking(true)?;
        let public = TcpListener::bind((Ipv4Addr::LOCALHOST, spec.listen_port))?;
        public.set_nonblocking(true)?;
        let bytes = serde_json::to_vec(process)?;
        let binding = Binding {
            process_identity: ObjectId {
                sha256: hex::encode(Sha256::digest(&bytes)),
                size: bytes.len() as u64,
            },
            source: spec.source.clone(),
            listen: public.local_addr()?,
            local_socket: Some(upstream),
            guest_port: spec.guest_port,
            connection_limit: CONNECTIONS,
            connection_byte_limit: CONNECTION_BYTES,
            run_byte_limit: RUN_BYTES,
            firmware_readiness_established: false,
        };
        let mut secret = [0; 32];
        File::open("/dev/urandom")?.read_exact(&mut secret)?;
        let token = hex::encode(secret);
        let expected = token.clone();
        let state = State::new();
        let signal = state.clone();
        save_json(receipt, &binding)?;
        let join = thread::Builder::new()
            .name("dashboard-listener".into())
            .stack_size(256 << 10)
            .spawn(move || serve(public, published, private, expected, signal))?;
        Ok(Self {
            directory,
            binding,
            token,
            state,
            join: Some(join),
        })
    }
    pub fn socket(&self) -> std::path::PathBuf {
        self.directory.path().join("bridge.sock")
    }
    pub fn token(&self) -> &str {
        &self.token
    }
    pub fn finish(mut self) -> serde_json::Value {
        self.stop();
        serde_json::json!({"binding":self.binding,"forwarded_bytes":self.state.budget.consumed(),
            "connections_completed":self.state.completed.load(Ordering::Acquire),"connections_failed":self.state.failed.load(Ordering::Acquire),
            "empty_connections":self.state.empty.load(Ordering::Acquire),
            "listener_closed":true,"supervisor_healthy":!self.state.infrastructure_failed.load(Ordering::Acquire),"firmware_test_pass":false})
    }
    fn stop(&mut self) {
        self.state.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            if join.join().is_err() {
                self.state
                    .infrastructure_failed
                    .store(true, Ordering::Release);
            }
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

fn accept_client(
    public: &TcpListener,
    published: &UnixListener,
    unix_first: bool,
) -> std::io::Result<OwnedFd> {
    for unix in [unix_first, !unix_first] {
        let result = if unix {
            published.accept().and_then(|(stream, _)| {
                stream.set_nonblocking(true)?;
                Ok(stream.into())
            })
        } else {
            public.accept().and_then(|(stream, peer)| {
                if !peer.ip().is_loopback() {
                    return Err(std::io::ErrorKind::PermissionDenied.into());
                }
                stream.set_nonblocking(true)?;
                Ok(stream.into())
            })
        };
        match result {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            other => return other,
        }
    }
    Err(std::io::ErrorKind::WouldBlock.into())
}

fn serve(
    public: TcpListener,
    published: UnixListener,
    private: UnixListener,
    token: String,
    state: Arc<State>,
) {
    let mut handlers: Vec<JoinHandle<()>> = Vec::new();
    let mut unix_first = false;
    while !state.stop.load(Ordering::Acquire) {
        let mut index = 0;
        while index < handlers.len() {
            if handlers[index].is_finished() {
                if handlers.swap_remove(index).join().is_err() {
                    state.infrastructure_failed.store(true, Ordering::Release);
                    state.stop.store(true, Ordering::Release);
                }
            } else {
                index += 1;
            }
        }
        unix_first = !unix_first;
        match accept_client(&public, &published, unix_first) {
            Ok(client) if handlers.len() < CONNECTIONS => {
                let private = match private.try_clone() {
                    Ok(socket) => socket,
                    Err(_) => {
                        state.infrastructure_failed.store(true, Ordering::Release);
                        break;
                    }
                };
                let token = token.clone();
                let signal = state.clone();
                let handler = thread::Builder::new()
                    .name("dashboard-stream".into())
                    .stack_size(256 << 10)
                    .spawn(move || {
                        let result = forward_client(client, &private, &token, &signal);
                        match result {
                            Ok(true) => {
                                signal.completed.fetch_add(1, Ordering::AcqRel);
                            }
                            Ok(false) => {
                                signal.empty.fetch_add(1, Ordering::AcqRel);
                            }
                            Err(_) => {
                                signal.failed.fetch_add(1, Ordering::AcqRel);
                            }
                        }
                    });
                match handler {
                    Ok(handler) => handlers.push(handler),
                    Err(_) => {
                        state.failed.fetch_add(1, Ordering::AcqRel);
                        state.infrastructure_failed.store(true, Ordering::Release);
                        break;
                    }
                }
            }
            Ok(_) => {
                state.failed.fetch_add(1, Ordering::AcqRel);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(_) => {
                state.infrastructure_failed.store(true, Ordering::Release);
                break;
            }
        }
    }
    state.stop.store(true, Ordering::Release);
    for handler in handlers {
        if handler.join().is_err() {
            state.infrastructure_failed.store(true, Ordering::Release);
        }
    }
}

fn forward_client(
    client: OwnedFd,
    private: &UnixListener,
    token: &str,
    state: &State,
) -> Result<bool> {
    let deadline = Instant::now() + Duration::from_secs(5);
    // SSH readiness probes connect and close without an HTTP request. Do not
    // create a native service connection until the client sends its first byte.
    loop {
        ready(client.as_raw_fd(), libc::POLLIN, state, Some(deadline))?;
        let mut byte = [0u8];
        let count = unsafe {
            libc::recv(
                client.as_raw_fd(),
                byte.as_mut_ptr().cast(),
                1,
                libc::MSG_PEEK | libc::MSG_DONTWAIT,
            )
        };
        if count == 0 {
            return Ok(false);
        }
        if count > 0 {
            break;
        }
        io_retry()?;
    }
    let agent = loop {
        state.running()?;
        ensure!(
            Instant::now() < deadline,
            "dashboard agent connection deadline"
        );
        match private.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10))
            }
            Err(error) => return Err(error.into()),
        }
    };
    agent.set_nonblocking(true)?;
    let mut observed = [0u8; 64];
    control(
        agent.as_raw_fd(),
        &mut observed,
        false,
        Some(deadline),
        state,
    )?;
    ensure!(
        observed == token.as_bytes(),
        "dashboard agent identity differs"
    );
    control(agent.as_raw_fd(), &mut [1], true, Some(deadline), state)?;
    let mut ready = [0xff];
    control(agent.as_raw_fd(), &mut ready, false, Some(deadline), state)?;
    ensure!(ready == [0], "native dashboard is unavailable");
    pump(client, agent.into(), state)?;
    Ok(true)
}

/// Trusted adapter inside the process sandbox. It receives only incoming
/// stream bytes and connects only to the declared private loopback port.
pub struct Agent {
    state: Arc<State>,
    joins: Vec<JoinHandle<()>>,
}
impl Agent {
    pub fn start_from_environment() -> Result<Option<Self>> {
        let token = std::env::var(TOKEN_ENV).ok();
        let port = std::env::var(PORT_ENV).ok();
        let (Some(token), Some(port)) = (token.clone(), port.clone()) else {
            ensure!(
                token.is_none() && port.is_none(),
                "partial dashboard bridge environment"
            );
            return Ok(None);
        };
        let port = validate_agent_environment(Some(&token), Some(&port))?;
        let state = State::new();
        let mut agent = Self {
            state: state.clone(),
            joins: Vec::new(),
        };
        for _ in 0..CONNECTIONS {
            let token = token.clone();
            let signal = state.clone();
            let join = thread::Builder::new()
                .name("dashboard-agent".into())
                .stack_size(256 << 10)
                .spawn(move || {
                    while !signal.stop.load(Ordering::Acquire) {
                        let result = agent_connection(&token, port, &signal);
                        if result.is_ok() {
                            signal.completed.fetch_add(1, Ordering::AcqRel);
                        } else {
                            signal.failed.fetch_add(1, Ordering::AcqRel);
                            thread::sleep(Duration::from_millis(50));
                        }
                    }
                })?;
            agent.joins.push(join);
        }
        Ok(Some(agent))
    }

    pub fn completed_connections(&self) -> u64 {
        self.state.completed.load(Ordering::Acquire)
    }
}
impl Drop for Agent {
    fn drop(&mut self) {
        self.state.stop.store(true, Ordering::Release);
        for join in self.joins.drain(..) {
            let _ = join.join();
        }
    }
}
fn agent_connection(token: &str, port: u16, state: &State) -> Result<()> {
    let socket = connect_agent()?;
    let mut secret = token.as_bytes().to_vec();
    control(
        socket.as_raw_fd(),
        &mut secret,
        true,
        Some(Instant::now() + Duration::from_secs(5)),
        state,
    )?;
    let mut start = [0];
    control(socket.as_raw_fd(), &mut start, false, None, state)?;
    ensure!(start == [1], "dashboard agent request invalid");
    let native = TcpStream::connect_timeout(
        &SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        Duration::from_secs(2),
    )?;
    native.set_nonblocking(true)?;
    control(
        socket.as_raw_fd(),
        &mut [0],
        true,
        Some(Instant::now() + Duration::from_secs(2)),
        state,
    )?;
    pump(socket.into(), native.into(), state)
}

fn connect_agent() -> Result<UnixStream> {
    // Set nonblocking before connect. A full Unix listen backlog must not
    // hold an agent thread indefinitely during cancellation or shutdown.
    let fd = unsafe {
        libc::socket(
            libc::AF_UNIX,
            libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            0,
        )
    };
    ensure!(
        fd >= 0,
        "dashboard socket: {}",
        std::io::Error::last_os_error()
    );
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
    ensure!(
        SOCKET_PATH.len() < address.sun_path.len(),
        "dashboard socket path bound"
    );
    address.sun_family = libc::AF_UNIX as libc::sa_family_t;
    for (slot, byte) in address.sun_path.iter_mut().zip(SOCKET_PATH.bytes()) {
        *slot = byte as libc::c_char;
    }
    let length = std::mem::offset_of!(libc::sockaddr_un, sun_path) + SOCKET_PATH.len() + 1;
    let result = unsafe {
        libc::connect(
            fd,
            (&address as *const libc::sockaddr_un).cast(),
            length as libc::socklen_t,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(UnixStream::from(owned))
}

fn ready(fd: RawFd, events: i16, state: &State, deadline: Option<Instant>) -> Result<()> {
    loop {
        state.running()?;
        ensure!(
            deadline.is_none_or(|end| Instant::now() < end),
            "dashboard I/O deadline"
        );
        let mut item = libc::pollfd {
            fd,
            events,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut item, 1, 100) };
        if result < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        ensure!(
            item.revents & libc::POLLNVAL == 0,
            "dashboard descriptor invalid"
        );
        if result > 0 {
            return Ok(());
        }
    }
}
fn control(
    fd: RawFd,
    bytes: &mut [u8],
    write: bool,
    deadline: Option<Instant>,
    state: &State,
) -> Result<()> {
    let mut offset = 0;
    while offset < bytes.len() {
        ready(
            fd,
            if write { libc::POLLOUT } else { libc::POLLIN },
            state,
            deadline,
        )?;
        let count = unsafe {
            if write {
                libc::send(
                    fd,
                    bytes[offset..].as_ptr().cast(),
                    bytes.len() - offset,
                    libc::MSG_NOSIGNAL,
                )
            } else {
                libc::read(
                    fd,
                    bytes[offset..].as_mut_ptr().cast(),
                    bytes.len() - offset,
                )
            }
        };
        if count < 0 {
            let error = std::io::Error::last_os_error();
            if matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
            ) {
                continue;
            }
            return Err(error.into());
        }
        ensure!(count > 0, "dashboard control stream ended");
        offset += count as usize;
    }
    Ok(())
}

fn pump(left: OwnedFd, right: OwnedFd, state: &State) -> Result<()> {
    crate::byte_stream::relay(left, right, &state.budget)?.require_completed()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gateway_socket_parent_is_private_from_creation() {
        let directory = private_directory().unwrap();
        let metadata = fs::symlink_metadata(directory.path()).unwrap();
        assert!(metadata.is_dir());
        assert_eq!(metadata.mode() & 0o777, 0o700);
        assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    }

    #[test]
    fn optional_mount_requires_a_private_owned_socket_and_valid_runtime_environment() {
        let valid = "aa".repeat(32);
        assert_eq!(
            validate_agent_environment(Some(&valid), Some("18080")).unwrap(),
            18080
        );
        for (token, port) in [
            (None, Some("18080")),
            (Some(valid.as_str()), None),
            (Some("no"), Some("18080")),
            (Some(valid.as_str()), Some("80")),
            (Some(valid.as_str()), Some("65536")),
            (Some(valid.as_str()), Some("-1")),
        ] {
            assert!(validate_agent_environment(token, port).is_err());
        }
        let root = tempfile::tempdir().unwrap();
        // Host socket binding can be prohibited. The positive socket/mount
        // case runs in the actual local/SSH VM suite. These host regressions
        // must still reject a regular file, symlink and directory.
        let path = root.path().join("not-a-socket");
        fs::write(&path, b"not a socket").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(validate_guest_socket(&path).is_err());
        let link = root.path().join("alias");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(validate_guest_socket(&link).is_err());
        assert!(validate_guest_socket(root.path()).is_err());
    }

    #[test]
    fn receipt_rejects_changed_identity_policy_and_unsafe_files() {
        let spec = Spec {
            source: Source::AdaptedNative,
            guest_port: 18080,
            listen_port: 0,
        };
        let process = serde_json::json!({"dashboard":spec,"firmware":"test-input"});
        let bytes = serde_json::to_vec(&process).unwrap();
        let binding = Binding {
            process_identity: ObjectId {
                sha256: hex::encode(Sha256::digest(&bytes)),
                size: bytes.len() as u64,
            },
            source: Source::AdaptedNative,
            listen: "127.0.0.1:18081".parse().unwrap(),
            local_socket: None,
            guest_port: 18080,
            connection_limit: CONNECTIONS,
            connection_byte_limit: CONNECTION_BYTES,
            run_byte_limit: RUN_BYTES,
            firmware_readiness_established: false,
        };
        binding.verify(&spec, &process).unwrap();
        for (key, value) in [
            ("listen", serde_json::json!("192.0.2.1:18081")),
            ("listen", serde_json::json!("127.0.0.1:80")),
            ("listen", serde_json::json!("[::1]:18081")),
            ("guest_port", serde_json::json!(18082)),
            ("source", serde_json::json!("native")),
            ("connection_limit", serde_json::json!(9)),
            (
                "connection_byte_limit",
                serde_json::json!(CONNECTION_BYTES + 1),
            ),
            ("run_byte_limit", serde_json::json!(RUN_BYTES + 1)),
            ("firmware_readiness_established", serde_json::json!(true)),
        ] {
            let mut changed = serde_json::to_value(&binding).unwrap();
            changed[key] = value;
            assert!(
                serde_json::from_value::<Binding>(changed)
                    .unwrap()
                    .verify(&spec, &process)
                    .is_err(),
                "{key}"
            );
        }
        assert!(binding
            .verify(&spec, &serde_json::json!({"firmware":"another-input"}))
            .is_err());
        let fixed = Spec {
            listen_port: 18082,
            ..spec.clone()
        };
        assert!(binding.verify(&fixed, &process).is_err());
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("receipt.json");
        save_json(&path, &binding).unwrap();
        Binding::read(&path, &spec, &process).unwrap();
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(Binding::read(&link, &spec, &process).is_err());
        fs::remove_file(&link).unwrap();
        fs::hard_link(&path, &link).unwrap();
        assert!(Binding::read(&path, &spec, &process).is_err());
        fs::remove_file(&link).unwrap();
        File::create(&path).unwrap().set_len(65537).unwrap();
        assert!(Binding::read(&path, &spec, &process).is_err());
        assert!(Binding::read(root.path(), &spec, &process).is_err());
    }

    #[test]
    fn publication_rejects_unknown_fields_and_privileged_ports() {
        for value in [
            serde_json::json!({"source":"native","guest_port":80,"listen_port":0}),
            serde_json::json!({"source":"native","guest_port":18080,"listen_port":22}),
            serde_json::json!({"source":"native","guest_port":18080,"listen_port":0,"host":"192.0.2.1"}),
            serde_json::json!({"source":"anything","guest_port":18080,"listen_port":0}),
        ] {
            assert!(serde_json::from_value::<Spec>(value)
                .and_then(|spec| spec.validate().map_err(serde::de::Error::custom))
                .is_err());
        }
        Spec {
            source: Source::Native,
            guest_port: 18080,
            listen_port: 0,
        }
        .validate()
        .unwrap();
    }

    #[test]
    fn cancellation_and_deadline_interrupt_idle_io() {
        let (_writer, reader) = UnixStream::pair().unwrap();
        let state = State::new();
        assert!(ready(
            reader.as_raw_fd(),
            libc::POLLIN,
            &state,
            Some(Instant::now())
        )
        .is_err());
        state.stop.store(true, Ordering::Release);
        assert!(ready(reader.as_raw_fd(), libc::POLLIN, &state, None).is_err());
    }
}

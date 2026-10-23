//! Access to an actual miner through a selected private carrier.
//!
//! This is a separate endpoint contract. It never manufactures the runtime's
//! dashboard binding or changes a worker response. Provider liveness must come
//! from an authenticated, owned control transport, not from an IP address.
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::{LockedRun, WorkerTransport};
use md_sim_runtime::{
    byte_stream::{self, ByteBudget, StreamReceipt},
    dashboard::Source,
    isolation::ProcessSpec,
    store::ObjectId,
    worker::{process_identity, Request},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::{
            fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
            net::UnixListener,
        },
    },
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const POLICY_BYTES: usize = 65536;
const CONFIG_BYTES: usize = 4 << 20;

pub(crate) fn object(bytes: &[u8]) -> ObjectId {
    ObjectId {
        sha256: hex::encode(Sha256::digest(bytes)),
        size: bytes.len() as u64,
    }
}

fn save_new(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn token(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Exact policy bytes are a locked input. The support identity must not refer
/// back to this policy. Provider configuration is bound separately afterward.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub source: Source,
    pub support_identity: ObjectId,
    pub destination: SocketAddr,
    /// Only server loopback is bound. Zero selects a fresh owned port.
    pub listen_port: u16,
    pub connection_limit: usize,
    pub connection_attempt_limit: usize,
    pub connection_bytes: u64,
    pub run_bytes: u64,
    pub connect_milliseconds: u64,
    pub idle_seconds: u64,
    pub wall_seconds: u64,
    /// The listener expires without a fresh owner reply, even if its caller stalls.
    pub lease_milliseconds: u64,
}

impl Policy {
    pub fn validate(&self) -> Result<()> {
        self.support_identity.validate()?;
        let private = match self.destination.ip() {
            IpAddr::V4(address) => address.is_private(),
            IpAddr::V6(_) => false,
        };
        ensure!(
            self.schema == "md-sim-carrier-dashboard-policy/1"
                && matches!(self.source, Source::Native | Source::AdaptedNative)
                && self.support_identity.size > 0
                && private
                && self.destination.port() > 0
                && (self.listen_port == 0 || self.listen_port >= 1024),
            "invalid carrier dashboard identity or endpoint policy"
        );
        ensure!(
            (1..=8).contains(&self.connection_limit)
                && (1..=4096).contains(&self.connection_attempt_limit)
                && (1..=64 << 20).contains(&self.connection_bytes)
                && (1..=256 << 20).contains(&self.run_bytes)
                && (1..=2000).contains(&self.connect_milliseconds)
                && (1..=60).contains(&self.idle_seconds)
                && (1..=3600).contains(&self.wall_seconds)
                && (100..=10000).contains(&self.lease_milliseconds),
            "carrier dashboard resource policy exceeds bounds"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub locked_run: ObjectId,
    pub process: ObjectId,
    pub policy: ObjectId,
    pub support: ObjectId,
    pub token: String,
    pub run_id: String,
}

/// Created only from exact captured policy bytes and a validated full lock.
#[derive(Clone, Debug)]
pub struct Selection {
    identity: Identity,
    policy: Policy,
    process: ProcessSpec,
}

impl Selection {
    pub fn new(run: &LockedRun, policy: ObjectId, bytes: &[u8], run_token: &str) -> Result<Self> {
        run.validate()?;
        policy.validate()?;
        ensure!(
            bytes.len() <= POLICY_BYTES && object(bytes) == policy && token(run_token),
            "carrier policy bytes or token differ"
        );
        ensure!(
            run.inputs.values().any(|input| *input == policy)
                && run.policies["carrier_dashboard"] == serde_json::to_value(&policy)?,
            "carrier dashboard policy is not the selected locked input"
        );
        let parsed: Policy = serde_json::from_slice(bytes)?;
        parsed.validate()?;
        let process = run.process.as_ref().context("carrier process absent")?;
        ensure!(
            process.private_channel.is_some()
                && process.dashboard.is_none()
                && process
                    .inputs
                    .iter()
                    .any(|input| input.object == parsed.support_identity),
            "carrier process or captured support identity differs"
        );
        Ok(Self {
            identity: Identity {
                locked_run: run.object()?,
                process: process_identity(process)?,
                policy,
                support: parsed.support_identity.clone(),
                token: run_token.into(),
                run_id: format!("run-{run_token}"),
            },
            policy: parsed,
            process: process.clone(),
        })
    }

    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }
}

/// Exact outer provider configuration. Its capture in the outer process must
/// also be checked by the caller; this validates the bytes and selected process.
#[derive(Clone, Debug)]
pub struct ProviderBinding {
    selection: Selection,
    configuration: ObjectId,
}

impl ProviderBinding {
    pub fn from_config(
        selection: Selection,
        configuration: ObjectId,
        bytes: &[u8],
    ) -> Result<Self> {
        configuration.validate()?;
        ensure!(
            bytes.len() <= CONFIG_BYTES && object(bytes) == configuration,
            "provider configuration bytes differ"
        );
        let config: Value = serde_json::from_slice(bytes)?;
        let process: ProcessSpec = serde_json::from_value(config["process"].clone())?;
        let leased: LockedRun = serde_json::from_value(config["lease"]["locked_run"].clone())?;
        process.validate()?;
        ensure!(
            config["schema"] == "md-sim-worker-private-carrier/1"
                && config["support_identity"] == serde_json::to_value(&selection.identity.support)?
                && process_identity(&process)? == selection.identity.process
                && serde_json::to_value(&process)? == serde_json::to_value(&selection.process)?
                && leased.object()? == selection.identity.locked_run
                && config["lease"]["policy"] == serde_json::to_value(&selection.identity.policy)?,
            "provider configuration belongs to another carrier process"
        );
        Ok(Self {
            selection,
            configuration,
        })
    }

    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn configuration(&self) -> &ObjectId {
        &self.configuration
    }

    /// Both transports must be authenticated and bounded by their owners. The
    /// service has its own lease timer and closes independently while this call
    /// waits. No provider or transport code runs on the service's relay thread.
    pub fn authorize(
        &self,
        worker: &mut impl WorkerTransport,
        provider: &mut impl ProviderTransport,
    ) -> Result<Authorization> {
        self.authorize_or_wait(worker, provider)?
            .context("selected owning provider is still waiting")
    }

    /// Readiness polling may observe a correctly bound waiting owner. That
    /// state never produces an authorization or opens an endpoint.
    pub(crate) fn authorize_or_wait(
        &self,
        worker: &mut impl WorkerTransport,
        provider: &mut impl ProviderTransport,
    ) -> Result<Option<Authorization>> {
        let started = Instant::now();
        let lookup = worker.request(&Request::LookupToken {
            token: self.selection.identity.token.clone(),
        })?;
        let status = worker.request(&Request::Status {
            run_id: self.selection.identity.run_id.clone(),
        })?;
        ensure!(
            serde_json::to_vec(&lookup)?.len() <= 65536
                && serde_json::to_vec(&status)?.len() <= 65536,
            "carrier worker authorization metadata exceeds bound"
        );
        self.check_worker(&lookup, &status)?;
        let mut nonce = [0; 32];
        File::open("/dev/urandom")?.read_exact(&mut nonce)?;
        let challenge = hex::encode(nonce);
        let lease = provider.lease(self, &challenge)?;
        ensure!(
            lease.schema == "md-sim-carrier-provider-lease/1"
                && lease.identity == self.selection.identity
                && lease.configuration == self.configuration
                && lease.challenge == challenge
                && lease.sequence > 0
                && matches!(lease.state, ProviderState::Active | ProviderState::Waiting),
            "selected owning provider is not live or its lease differs"
        );
        let duration = Duration::from_millis(self.selection.policy.lease_milliseconds);
        ensure!(
            started.elapsed() < duration,
            "carrier authorization exceeded lease deadline"
        );
        if lease.state == ProviderState::Waiting {
            return Ok(None);
        }
        Ok(Some(Authorization {
            identity: self.selection.identity.clone(),
            configuration: self.configuration.clone(),
            expires: started + duration,
            lease,
            lookup,
            status,
        }))
    }

    fn check_worker(&self, lookup: &Value, status: &Value) -> Result<()> {
        let identity = &self.selection.identity;
        ensure!(
            lookup["found"] == true
                && lookup["token"] == identity.token
                && lookup["run_id"] == identity.run_id
                && lookup["token_process_identity"] == serde_json::to_value(&identity.process)?
                && lookup["state"] == "booting"
                && status["run_id"] == identity.run_id
                && status["state"]["state"] == "booting"
                && status.get("dashboard") == Some(&Value::Null),
            "selected native worker is not active or its identity differs"
        );
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderState {
    Waiting,
    Active,
    Closed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderLease {
    pub schema: String,
    pub identity: Identity,
    pub configuration: ObjectId,
    pub challenge: String,
    /// Strictly increasing within the one owned provider instance.
    pub sequence: u64,
    pub state: ProviderState,
}

/// Implementations query the owning provider on its authenticated control
/// channel. They must not infer liveness from an address or invent an active
/// reply. The fresh challenge and sequence prevent replay of a saved reply.
pub trait ProviderTransport {
    fn lease(&mut self, binding: &ProviderBinding, challenge: &str) -> Result<ProviderLease>;
}

/// Query the real owning-provider monitor through its authenticated worker
/// transport. This does not infer liveness from ordinary worker Status.
pub struct WorkerProviderTransport<T> {
    transport: T,
}

impl<T: WorkerTransport> WorkerProviderTransport<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }
}

impl<T: WorkerTransport> ProviderTransport for WorkerProviderTransport<T> {
    fn lease(&mut self, binding: &ProviderBinding, challenge: &str) -> Result<ProviderLease> {
        ensure!(
            token(challenge),
            "provider challenge must be lowercase SHA-256-sized hex"
        );
        let reply = self.transport.request(&Request::PrivateChannelStatus {
            token: binding.selection.identity.token.clone(),
            challenge: challenge.into(),
        })?;
        ensure!(
            serde_json::to_vec(&reply)?.len() <= 65536,
            "provider lease metadata exceeds bound"
        );
        Ok(serde_json::from_value(reply)?)
    }
}

/// An authorization can only be obtained by the actual transport checks above.
/// The service consumes it once and rejects expired or repeated sequence values.
#[derive(Debug)]
pub struct Authorization {
    identity: Identity,
    configuration: ObjectId,
    expires: Instant,
    lease: ProviderLease,
    lookup: Value,
    status: Value,
}

impl Authorization {
    pub(crate) fn sequence(&self) -> Result<u64> {
        ensure!(
            Instant::now() < self.expires,
            "carrier authorization expired"
        );
        Ok(self.lease.sequence)
    }

    pub(crate) fn evidence(&self) -> Value {
        json!({"provider":self.lease,"worker_lookup":self.lookup,"worker_status":self.status,
            "scope":"Responses from the selected worker and checked owning-provider transports. Listener readiness does not establish firmware readiness."})
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CarrierEndpoint {
    pub schema: String,
    pub identity: Identity,
    pub provider_configuration: ObjectId,
    pub policy: Policy,
    pub listen: SocketAddr,
    pub local_socket: PathBuf,
    pub firmware_readiness_established: bool,
}

impl CarrierEndpoint {
    /// Portable identity validation. The publishing service also checks the
    /// actual private socket ownership before giving it to the TLS gateway.
    pub fn verify(&self, binding: &ProviderBinding) -> Result<()> {
        let expected = binding.selection();
        self.policy.validate()?;
        ensure!(
            self.schema == "md-sim-carrier-dashboard-endpoint/1"
                && self.identity == expected.identity
                && self.provider_configuration == binding.configuration
                && self.policy == expected.policy
                && self.listen.ip() == IpAddr::V4(Ipv4Addr::LOCALHOST)
                && self.listen.port() >= 1024
                && (self.policy.listen_port == 0 || self.listen.port() == self.policy.listen_port)
                && self.local_socket.is_absolute()
                && self
                    .local_socket
                    .file_name()
                    .is_some_and(|name| name == "http.sock")
                && self.local_socket.components().all(|part| matches!(
                    part,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                ))
                && !self.firmware_readiness_established,
            "carrier endpoint does not belong to the selected publication"
        );
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionReceipt {
    pub index: usize,
    pub incoming: String,
    pub connected: bool,
    pub stream: Option<StreamReceipt>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Receipt {
    pub schema: String,
    pub endpoint: CarrierEndpoint,
    pub end: String,
    pub error: Option<String>,
    pub elapsed_milliseconds: u128,
    pub accepted_connections: usize,
    pub charged_bytes: u64,
    pub authorizations: u64,
    pub first_authorization: Value,
    pub final_authorization: Value,
    pub connections: Vec<ConnectionReceipt>,
    pub listeners_closed: bool,
    pub firmware_readiness_established: bool,
}

struct LeaseState {
    expires: Instant,
    sequence: u64,
    authorizations: u64,
    first: Value,
    last: Value,
    failure: Option<String>,
}

struct Shared {
    cancel: Arc<AtomicBool>,
    lease: Mutex<LeaseState>,
}

struct SocketPath {
    path: PathBuf,
    identity: Option<(u64, u64)>,
}

impl SocketPath {
    fn owned(path: &Path) -> Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        ensure!(
            metadata.file_type().is_socket()
                && metadata.nlink() == 1
                && metadata.uid() == unsafe { libc::geteuid() },
            "carrier socket ownership differs"
        );
        Ok(Self {
            path: path.into(),
            identity: Some((metadata.dev(), metadata.ino())),
        })
    }

    fn remove(&mut self) -> Result<()> {
        if let Some(expected) = self.identity {
            let metadata = fs::symlink_metadata(&self.path)?;
            ensure!(
                (metadata.dev(), metadata.ino()) == expected,
                "owned carrier socket path changed before cleanup"
            );
            fs::remove_file(&self.path)?;
            self.identity = None;
        }
        Ok(())
    }
}

impl Drop for SocketPath {
    fn drop(&mut self) {
        let _ = self.remove();
    }
}

struct Streams {
    cancel: Arc<AtomicBool>,
    active: Vec<JoinHandle<ConnectionReceipt>>,
}

impl Drop for Streams {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        for join in self.active.drain(..) {
            let _ = join.join();
        }
    }
}

/// Owns only its listeners and bounded stream threads. Owner control calls run
/// on the caller, so loss of control cannot keep its independently timed
/// listener alive. The caller must retain the final receipt from `stop`.
pub struct CarrierService {
    binding: ProviderBinding,
    endpoint: CarrierEndpoint,
    shared: Arc<Shared>,
    join: Option<JoinHandle<Receipt>>,
}

impl CarrierService {
    pub fn start(
        binding: ProviderBinding,
        worker: &mut impl WorkerTransport,
        provider: &mut impl ProviderTransport,
        output: &Path,
    ) -> Result<Self> {
        let first = binding.authorize(worker, provider)?;
        fs::DirBuilder::new().mode(0o700).create(output)?;
        let output = output.canonicalize()?;
        let metadata = fs::symlink_metadata(&output)?;
        ensure!(
            metadata.is_dir()
                && metadata.uid() == unsafe { libc::geteuid() }
                && metadata.mode() & 0o777 == 0o700,
            "carrier output directory is not private"
        );
        let socket = output.join("http.sock");
        let unix = UnixListener::bind(&socket)?;
        let socket_path = SocketPath::owned(&socket)?;
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
        unix.set_nonblocking(true)?;
        let tcp = TcpListener::bind((Ipv4Addr::LOCALHOST, binding.selection.policy.listen_port))?;
        tcp.set_nonblocking(true)?;
        let endpoint = CarrierEndpoint {
            schema: "md-sim-carrier-dashboard-endpoint/1".into(),
            identity: binding.selection.identity.clone(),
            provider_configuration: binding.configuration.clone(),
            policy: binding.selection.policy.clone(),
            listen: tcp.local_addr()?,
            local_socket: socket,
            firmware_readiness_established: false,
        };
        endpoint.verify(&binding)?;
        save_new(&output.join("endpoint.json"), &endpoint)?;
        let evidence = first.evidence();
        let shared = Arc::new(Shared {
            cancel: Arc::new(AtomicBool::new(false)),
            lease: Mutex::new(LeaseState {
                expires: first.expires,
                sequence: first.lease.sequence,
                authorizations: 1,
                first: evidence.clone(),
                last: evidence,
                failure: None,
            }),
        });
        let policy = &endpoint.policy;
        let budget = Arc::new(ByteBudget::new(
            policy.connection_bytes,
            policy.run_bytes,
            policy.idle_seconds,
            shared.cancel.clone(),
        )?);
        let (thread_shared, thread_endpoint) = (shared.clone(), endpoint.clone());
        let join = thread::Builder::new()
            .name("carrier-dashboard".into())
            .stack_size(256 << 10)
            .spawn(move || {
                serve(
                    tcp,
                    unix,
                    socket_path,
                    thread_endpoint,
                    thread_shared,
                    budget,
                )
            })?;
        let mut service = Self {
            binding,
            endpoint,
            shared,
            join: Some(join),
        };
        // A live before-start response alone cannot authorize a listener after
        // the selected run or provider ended during startup.
        service.refresh(worker, provider)?;
        Ok(service)
    }

    pub fn endpoint(&self) -> &CarrierEndpoint {
        &self.endpoint
    }

    pub fn is_running(&self) -> bool {
        self.join.as_ref().is_some_and(|join| !join.is_finished())
            && !self.shared.cancel.load(Ordering::Acquire)
    }

    /// Any failed refresh revokes the listener immediately. No caller can keep
    /// it alive by discarding the error or repeatedly replaying the same lease.
    pub fn refresh(
        &mut self,
        worker: &mut impl WorkerTransport,
        provider: &mut impl ProviderTransport,
    ) -> Result<()> {
        let outcome = (|| {
            ensure!(self.is_running(), "carrier service already ended");
            let authorization = self.binding.authorize(worker, provider)?;
            let mut lease = self
                .shared
                .lease
                .lock()
                .map_err(|_| anyhow::anyhow!("carrier lease lock failed"))?;
            ensure!(
                authorization.identity == self.endpoint.identity
                    && authorization.configuration == self.endpoint.provider_configuration
                    && authorization.lease.sequence > lease.sequence
                    && Instant::now() < lease.expires
                    && Instant::now() < authorization.expires
                    && !self.shared.cancel.load(Ordering::Acquire),
                "carrier lease expired or was replayed"
            );
            lease.expires = authorization.expires;
            lease.sequence = authorization.lease.sequence;
            lease.authorizations += 1;
            lease.last = authorization.evidence();
            Ok(())
        })();
        if let Err(error) = &outcome {
            if let Ok(mut lease) = self.shared.lease.lock() {
                lease.failure = Some(format!("{error:#}").chars().take(1024).collect());
            }
            self.shared.cancel.store(true, Ordering::Release);
        }
        outcome
    }

    pub fn stop(&mut self) -> Result<Receipt> {
        self.shared.cancel.store(true, Ordering::Release);
        let receipt = self
            .join
            .take()
            .context("carrier receipt was already retrieved")?
            .join()
            .map_err(|_| anyhow::anyhow!("carrier listener thread panicked"))?;
        save_new(
            &self.endpoint.local_socket.with_file_name("receipt.json"),
            &receipt,
        )?;
        Ok(receipt)
    }
}

impl Drop for CarrierService {
    fn drop(&mut self) {
        if self.join.is_some() {
            let _ = self.stop();
        }
    }
}

fn serve(
    tcp: TcpListener,
    unix: UnixListener,
    mut socket_path: SocketPath,
    endpoint: CarrierEndpoint,
    shared: Arc<Shared>,
    budget: Arc<ByteBudget>,
) -> Receipt {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(endpoint.policy.wall_seconds);
    let mut connections = Vec::new();
    let mut streams = Streams {
        cancel: shared.cancel.clone(),
        active: Vec::new(),
    };
    let mut accepted = 0;
    let mut unix_first = false;
    let outcome = (|| -> Result<&'static str> {
        loop {
            let mut index = 0;
            while index < streams.active.len() {
                if streams.active[index].is_finished() {
                    connections.push(
                        streams
                            .active
                            .swap_remove(index)
                            .join()
                            .map_err(|_| anyhow::anyhow!("carrier stream thread panicked"))?,
                    );
                } else {
                    index += 1;
                }
            }
            if shared.cancel.load(Ordering::Acquire) {
                return Ok("stopped");
            }
            if Instant::now() >= deadline {
                return Ok("wall-expired");
            }
            let expires = shared
                .lease
                .lock()
                .map_err(|_| anyhow::anyhow!("carrier lease lock failed"))?
                .expires;
            if Instant::now() >= expires {
                return Ok("lease-expired");
            }
            if accepted >= endpoint.policy.connection_attempt_limit && streams.active.is_empty() {
                return Ok("connection-attempt-limit");
            }
            let room = streams.active.len() < endpoint.policy.connection_limit
                && accepted < endpoint.policy.connection_attempt_limit;
            let mut poll = [
                libc::pollfd {
                    fd: if room { tcp.as_raw_fd() } else { -1 },
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: if room { unix.as_raw_fd() } else { -1 },
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            let ready = unsafe { libc::poll(poll.as_mut_ptr(), 2, 20) };
            if ready < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error.into());
            }
            ensure!(
                poll.iter().all(|entry| entry.revents
                    & (libc::POLLNVAL | libc::POLLERR | libc::POLLHUP)
                    == 0),
                "carrier listener failed"
            );
            if !room || ready == 0 {
                continue;
            }
            if shared.cancel.load(Ordering::Acquire)
                || Instant::now() >= expires
                || Instant::now() >= deadline
            {
                continue;
            }
            for is_unix in [unix_first, !unix_first] {
                let (descriptor, incoming): (OwnedFd, _) = if is_unix {
                    match unix.accept() {
                        Ok((stream, _)) => (stream.into(), "unix"),
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
                        Err(error) => return Err(error.into()),
                    }
                } else {
                    match tcp.accept() {
                        Ok((stream, address)) => {
                            ensure!(address.ip().is_loopback(), "non-loopback carrier client");
                            (stream.into(), "loopback")
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
                        Err(error) => return Err(error.into()),
                    }
                };
                accepted += 1;
                let (policy, stream_budget) = (endpoint.policy.clone(), budget.clone());
                let cancel = shared.cancel.clone();
                let index = accepted;
                streams.active.push(
                    thread::Builder::new()
                        .name("carrier-http-stream".into())
                        .stack_size(256 << 10)
                        .spawn(move || {
                            connection(
                                descriptor,
                                incoming,
                                index,
                                &policy,
                                &stream_budget,
                                &cancel,
                            )
                        })?,
                );
                unix_first = !is_unix;
                break;
            }
        }
    })();
    // Close listeners first. Bounded connect attempts may finish afterward,
    // but cancellation prevents them from forwarding any new stream bytes.
    shared.cancel.store(true, Ordering::Release);
    drop(tcp);
    drop(unix);
    let mut error = outcome.as_ref().err().map(|error| format!("{error:#}"));
    for join in streams.active.drain(..) {
        match join.join() {
            Ok(receipt) => connections.push(receipt),
            Err(_) => error = Some("carrier stream thread panicked".into()),
        }
    }
    if let Err(cause) = socket_path.remove() {
        error = Some(format!("{cause:#}"));
    }
    let (authorizations, first, last, failure) = match shared.lease.lock() {
        Ok(lease) => (
            lease.authorizations,
            lease.first.clone(),
            lease.last.clone(),
            lease.failure.clone(),
        ),
        Err(_) => (
            0,
            Value::Null,
            Value::Null,
            Some("carrier lease lock failed".into()),
        ),
    };
    connections.sort_unstable_by_key(|connection| connection.index);
    Receipt {
        schema: "md-sim-carrier-dashboard-receipt/1".into(),
        endpoint,
        end: if failure.is_some() {
            "authorization-failed"
        } else {
            outcome.unwrap_or("failed")
        }
        .into(),
        error: failure.or(error),
        elapsed_milliseconds: started.elapsed().as_millis(),
        accepted_connections: accepted,
        charged_bytes: budget.consumed(),
        authorizations,
        first_authorization: first,
        final_authorization: last,
        connections,
        listeners_closed: true,
        firmware_readiness_established: false,
    }
}

fn connection(
    incoming: OwnedFd,
    label: &str,
    index: usize,
    policy: &Policy,
    budget: &ByteBudget,
    cancel: &AtomicBool,
) -> ConnectionReceipt {
    let mut connected = false;
    let result = (|| {
        ensure!(
            !cancel.load(Ordering::Acquire),
            "carrier stream cancelled before connect"
        );
        let native = TcpStream::connect_timeout(
            &policy.destination,
            Duration::from_millis(policy.connect_milliseconds),
        )?;
        connected = true;
        ensure!(
            !cancel.load(Ordering::Acquire),
            "carrier stream cancelled after connect"
        );
        byte_stream::relay(incoming, native.into(), budget)
    })();
    match result {
        Ok(receipt) => ConnectionReceipt {
            index,
            incoming: label.into(),
            connected,
            stream: Some(receipt),
            error: None,
        },
        Err(error) => ConnectionReceipt {
            index,
            incoming: label.into(),
            connected,
            stream: None,
            error: Some(format!("{error:#}").chars().take(1024).collect()),
        },
    }
}

#[cfg(test)]
#[path = "carrier_tests.rs"]
pub(crate) mod tests;

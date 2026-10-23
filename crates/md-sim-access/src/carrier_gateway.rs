//! TLS publication of the actual server-owned endpoint for one carrier run.
//! The runtime dashboard binding and its existing publication path are separate.
use crate::{
    carrier::{self, CarrierEndpoint, ProviderBinding, WorkerProviderTransport},
    gateway::{Gateway, GatewaySpec},
};
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::WorkerTransport;
use md_sim_runtime::{dashboard::Source, isolation::save_json, store::ObjectId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

/// Absolute-deadline Unix control. It uses the existing worker wire codec.
pub struct Control {
    socket: PathBuf,
    timeout: Duration,
}
impl Control {
    pub fn new(socket: &Path, timeout: Duration) -> Result<Self> {
        ensure!(
            !timeout.is_zero() && timeout <= Duration::from_secs(60),
            "carrier control deadline bound"
        );
        Ok(Self {
            socket: socket.into(),
            timeout,
        })
    }
}
impl WorkerTransport for Control {
    fn request(&mut self, request: &md_sim_runtime::worker::Request) -> Result<Value> {
        md_sim_runtime::worker::request_with_timeout(&self.socket, request, self.timeout)
    }
    fn description(&self) -> String {
        format!(
            "owned Unix worker control with absolute {:?} deadline",
            self.timeout
        )
    }
}

fn read_endpoint(
    path: &Path,
    binding: &ProviderBinding,
) -> Result<(CarrierEndpoint, ObjectId, (u64, u64))> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file()
            && metadata.nlink() == 1
            && metadata.len() <= 65536
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o777 == 0o600,
        "carrier endpoint record is not private and owned"
    );
    let mut bytes = Vec::new();
    file.take(65537).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 65536,
        "carrier endpoint record grew beyond bound"
    );
    let endpoint: CarrierEndpoint = serde_json::from_slice(&bytes)?;
    endpoint.verify(binding)?;
    ensure!(
        path.is_absolute()
            && path.file_name().is_some_and(|name| name == "endpoint.json")
            && path.parent() == endpoint.local_socket.parent()
            && path.canonicalize()? == path,
        "carrier publication must use its actual bridge endpoint record"
    );
    let parent = fs::symlink_metadata(path.parent().context("carrier endpoint parent absent")?)?;
    let socket = fs::symlink_metadata(&endpoint.local_socket)?;
    ensure!(
        parent.is_dir()
            && parent.uid() == unsafe { libc::geteuid() }
            && parent.mode() & 0o777 == 0o700
            && socket.file_type().is_socket()
            && socket.uid() == unsafe { libc::geteuid() }
            && socket.nlink() == 1
            && socket.mode() & 0o777 == 0o600,
        "carrier bridge socket is not private and owned"
    );
    Ok((
        endpoint,
        carrier::object(&bytes),
        (socket.dev(), socket.ino()),
    ))
}

pub(crate) fn verify_spec(
    binding: &ProviderBinding,
    endpoint: &CarrierEndpoint,
    spec: &GatewaySpec,
) -> Result<()> {
    endpoint.verify(binding)?;
    let source = match endpoint.policy.source {
        Source::Native => "native",
        Source::AdaptedNative => "adapted-native",
        Source::SimulatorConsole => anyhow::bail!("carrier source must be native"),
    };
    ensure!(
        spec.run_id == endpoint.identity.run_id
            && spec.execution_identity == endpoint.identity.locked_run
            && spec.dashboard_source == source
            && spec.upstream == endpoint.listen
            && spec.upstream_unix.as_ref() == Some(&endpoint.local_socket)
            && spec.upstream_tls.is_none(),
        "gateway policy does not match the selected carrier endpoint"
    );
    Ok(())
}

fn authorize(socket: &Path, binding: &ProviderBinding) -> Result<crate::carrier::Authorization> {
    let timeout = control_timeout(binding);
    binding.authorize(
        &mut Control::new(socket, timeout)?,
        &mut WorkerProviderTransport::new(Control::new(socket, timeout)?),
    )
}

fn control_timeout(binding: &ProviderBinding) -> Duration {
    Duration::from_millis((binding.selection().policy().lease_milliseconds / 4).min(5000))
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ReadyRecord {
    schema: String,
    identity: carrier::Identity,
    provider_configuration: ObjectId,
    endpoint_record: ObjectId,
    endpoint_path: PathBuf,
    firmware_readiness_established: bool,
}

fn ready_record(
    path: &Path,
    binding: &ProviderBinding,
    endpoint_path: &Path,
    endpoint_object: &ObjectId,
) -> Result<(Value, ObjectId)> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    let parent = fs::symlink_metadata(path.parent().context("readiness parent absent")?)?;
    ensure!(
        path.is_absolute()
            && path.canonicalize()? == path
            && metadata.is_file()
            && metadata.nlink() == 1
            && metadata.len() <= 65536
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o022 == 0
            && parent.is_dir()
            && parent.uid() == unsafe { libc::geteuid() }
            && parent.mode() & 0o777 == 0o700,
        "carrier readiness record is not private and owned"
    );
    let mut bytes = Vec::new();
    file.take(65537).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 65536,
        "carrier readiness record grew beyond bound"
    );
    let value: ReadyRecord = serde_json::from_slice(&bytes)?;
    ensure!(
        value.schema == "md-sim-carrier-dashboard-ready/1"
            && value.identity == *binding.selection().identity()
            && value.provider_configuration == *binding.configuration()
            && value.endpoint_record == *endpoint_object
            && value.endpoint_path == endpoint_path
            && !value.firmware_readiness_established,
        "carrier readiness belongs to another endpoint or execution"
    );
    Ok((serde_json::to_value(value)?, carrier::object(&bytes)))
}

struct ReadyEndpoint {
    endpoint: CarrierEndpoint,
    object: ObjectId,
    socket_identity: (u64, u64),
    ready: Value,
    ready_object: ObjectId,
    authorization: crate::carrier::Authorization,
}

fn wait_ready(
    socket: &Path,
    binding: &ProviderBinding,
    ready: &Path,
    endpoint: &Path,
    cancelled: &AtomicBool,
) -> Result<ReadyEndpoint> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        ensure!(
            !cancelled.load(Ordering::Acquire),
            "carrier publication cancelled"
        );
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .context("carrier readiness deadline")?;
        let timeout = control_timeout(binding).min(remaining / 4);
        ensure!(!timeout.is_zero(), "carrier readiness deadline");
        let authorization = binding.authorize_or_wait(
            &mut Control::new(socket, timeout)?,
            &mut WorkerProviderTransport::new(Control::new(socket, timeout)?),
        )?;
        if let Some(authorization) = authorization {
            if ready.try_exists()? {
                let (endpoint_value, object, socket_identity) = read_endpoint(endpoint, binding)?;
                let (ready_value, ready_object) = ready_record(ready, binding, endpoint, &object)?;
                authorization.sequence()?;
                ensure!(
                    !cancelled.load(Ordering::Acquire),
                    "carrier publication cancelled"
                );
                ensure!(Instant::now() < deadline, "carrier readiness deadline");
                return Ok(ReadyEndpoint {
                    endpoint: endpoint_value,
                    object,
                    socket_identity,
                    ready: ready_value,
                    ready_object,
                    authorization,
                });
            }
        }
        std::thread::sleep(
            Duration::from_millis(20).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

/// Inspect the actual bridge record over the server's control path. This does
/// not start TLS and is not an HTTP readiness check.
pub fn inspect(
    socket: &Path,
    binding: &ProviderBinding,
    ready: &Path,
    path: &Path,
) -> Result<Value> {
    let selected = wait_ready(socket, binding, ready, path, &AtomicBool::new(false))?;
    Ok(
        json!({"schema":"md-sim-carrier-endpoint-inspection/1", "endpoint":selected.endpoint,
        "endpoint_record":selected.object,"authorization":selected.authorization.evidence(),
        "readiness_record":selected.ready,"readiness_object":selected.ready_object,
        "firmware_readiness_established":false}),
    )
}

pub struct CarrierGateway {
    gateway: Gateway,
    socket: PathBuf,
    endpoint_path: PathBuf,
    ready_path: PathBuf,
    ready_object: ObjectId,
    endpoint_object: ObjectId,
    socket_identity: (u64, u64),
    binding: ProviderBinding,
    endpoint: CarrierEndpoint,
    authorization: Value,
    sequence: u64,
    directory: PathBuf,
    stopped: bool,
}

impl CarrierGateway {
    pub fn start(
        socket: &Path,
        binding: ProviderBinding,
        ready_path: &Path,
        endpoint_path: &Path,
        spec: &GatewaySpec,
        directory: &Path,
    ) -> Result<Self> {
        Self::start_with_cancel(
            socket,
            binding,
            ready_path,
            endpoint_path,
            spec,
            directory,
            &AtomicBool::new(false),
        )
    }

    pub fn start_with_cancel(
        socket: &Path,
        binding: ProviderBinding,
        ready_path: &Path,
        endpoint_path: &Path,
        spec: &GatewaySpec,
        directory: &Path,
        cancelled: &AtomicBool,
    ) -> Result<Self> {
        let selected = wait_ready(socket, &binding, ready_path, endpoint_path, cancelled)?;
        let (endpoint, endpoint_object, socket_identity) =
            (selected.endpoint, selected.object, selected.socket_identity);
        verify_spec(&binding, &endpoint, spec)?;
        let first = selected.authorization;
        let sequence = first.sequence()?;
        ensure!(
            !cancelled.load(Ordering::Acquire),
            "carrier publication cancelled"
        );
        let gateway = Gateway::start(spec, directory)?;
        let mut service = Self {
            gateway,
            socket: socket.into(),
            endpoint_path: endpoint_path.into(),
            ready_path: ready_path.into(),
            ready_object: selected.ready_object,
            endpoint_object,
            socket_identity,
            binding,
            endpoint,
            authorization: first.evidence(),
            sequence,
            directory: directory.into(),
            stopped: false,
        };
        // Confirm the exact process/provider/bridge again after TLS startup.
        service.poll()?;
        ensure!(
            !cancelled.load(Ordering::Acquire),
            "carrier publication cancelled"
        );
        save_json(
            &directory.join("carrier-selected.json"),
            &service.manifest()?,
        )?;
        Ok(service)
    }

    pub fn manifest(&self) -> Result<Value> {
        Ok(json!({"schema":"md-sim-selected-carrier-gateway/1",
            "run_id":self.endpoint.identity.run_id,"locked_run":self.endpoint.identity.locked_run,
            "carrier_endpoint":self.endpoint,"endpoint_record":self.endpoint_object,
            "readiness_object":self.ready_object,
            "endpoint":self.gateway.manifest()?,"authorization":self.authorization,
            "firmware_readiness_established":false,"native_dashboard_gate":false,
            "lifecycle":"Actual worker and owning-provider state plus the same owned bridge record and socket are checked before/after startup and on every poll."}))
    }

    /// A lost or changed control reply closes TLS. Bridge lease expiry also
    /// removes its Unix listener, so it remains independently fail-closed.
    pub fn poll(&mut self) -> Result<bool> {
        if self.stopped {
            return Ok(false);
        }
        let outcome = (|| {
            ensure!(self.gateway.is_running()?, "carrier TLS process ended");
            let (endpoint, object, socket) = read_endpoint(&self.endpoint_path, &self.binding)?;
            ensure!(
                endpoint == self.endpoint
                    && object == self.endpoint_object
                    && socket == self.socket_identity,
                "selected carrier endpoint was replaced"
            );
            ensure!(
                ready_record(
                    &self.ready_path,
                    &self.binding,
                    &self.endpoint_path,
                    &object
                )?
                .1 == self.ready_object,
                "selected carrier readiness record changed"
            );
            let next = authorize(&self.socket, &self.binding)?;
            let sequence = next.sequence()?;
            ensure!(
                sequence > self.sequence,
                "provider publication lease was replayed"
            );
            self.sequence = sequence;
            self.authorization = next.evidence();
            Ok(true)
        })();
        if !matches!(outcome, Ok(true)) {
            self.stop()?;
        }
        outcome
    }

    pub fn stop(&mut self) -> Result<()> {
        if !self.stopped {
            self.gateway.stop()?;
            self.stopped = true;
            save_json(
                &self.directory.join("carrier-selected-stop.json"),
                &json!({"run_id":self.endpoint.identity.run_id,"gateway_stopped":true,
                    "final_authorization":self.authorization}),
            )?;
        }
        Ok(())
    }
}

impl Drop for CarrierGateway {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
#[path = "carrier_gateway_tests.rs"]
pub(crate) mod tests;

//! Owned SSH deployment of a gateway bound to one active worker process.
use crate::gateway::GatewaySpec;
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::{executor::transfer_object, LockedRun, SshTransport};
use md_sim_runtime::{
    dashboard::Binding,
    isolation::save_json,
    store::{hash_file, Store},
};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::Read,
    net::TcpStream,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    process::{Child, Stdio},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

/// Paths belong to the authenticated worker account. No server layout is
/// inferred from a file name or from the client's filesystem.
pub struct RemotePaths<'a> {
    pub access_program: &'a str,
    pub store: &'a Path,
    pub socket: &'a Path,
    pub output: &'a Path,
}
pub struct RemoteGateway {
    child: Child,
    directory: PathBuf,
    spec: GatewaySpec,
    manifest: Value,
    stopped: bool,
}
fn read(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(65537).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 65536, "remote gateway metadata bound");
    Ok(bytes)
}
impl RemoteGateway {
    pub fn start(
        transport: &mut SshTransport,
        run: &LockedRun,
        spec: &GatewaySpec,
        paths: RemotePaths<'_>,
        output: &Path,
    ) -> Result<Self> {
        run.validate()?;
        ensure!(
            run.object()? == spec.execution_identity,
            "remote gateway execution identity differs"
        );
        fs::DirBuilder::new().mode(0o700).create(output)?;
        let directory = output.canonicalize()?;
        save_json(&directory.join("gateway.json"), spec)?;
        save_json(&directory.join("locked-run.json"), run)?;
        let store = Store::open(&directory.join("input-store"), 16 << 20)?;
        let mut objects = Vec::new();
        for name in ["gateway.json", "locked-run.json"] {
            let path = directory.join(name);
            let object = hash_file(&path)?;
            store.import(&path, &object)?;
            transfer_object(transport, &store, &object, &AtomicBool::new(false))?;
            objects.push(object);
        }
        save_json(
            &directory.join("transfer.json"),
            &json!({"verified_at_destination":objects,"transport":transport.evidence()}),
        )?;
        let remote_spec = paths.store.join("objects").join(&objects[0].sha256);
        let remote_run = paths.store.join("objects").join(&objects[1].sha256);
        let mut command = transport.service_command(
            paths.access_program,
            &[
                "serve-selected-connected",
                remote_spec.to_str().context("remote spec path encoding")?,
                remote_run.to_str().context("remote run path encoding")?,
                paths
                    .socket
                    .to_str()
                    .context("worker socket path encoding")?,
                paths
                    .output
                    .to_str()
                    .context("worker gateway path encoding")?,
            ],
        )?;
        let child = command
            .stdin(Stdio::piped())
            .stdout(File::create_new(directory.join("stdout.json"))?)
            .stderr(File::create_new(directory.join("stderr.log"))?)
            .spawn()?;
        let mut gateway = Self {
            child,
            directory,
            spec: spec.clone(),
            manifest: Value::Null,
            stopped: false,
        };
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if let Ok(value) =
                serde_json::from_slice::<Value>(&read(&gateway.directory.join("stdout.json"))?)
            {
                ensure!(
                    value["schema"] == "md-sim-selected-worker-gateway/1"
                        && value["run_id"] == spec.run_id
                        && value["locked_run"] == serde_json::to_value(run.object()?)?
                        && value["endpoint"]["upstream_unix"]
                            == serde_json::to_value(&spec.upstream_unix)?,
                    "remote publication selected another run or endpoint"
                );
                let binding: Binding = serde_json::from_value(value["binding"].clone())?;
                let process = run.process.as_ref().context("published process absent")?;
                binding.verify(
                    process
                        .dashboard
                        .as_ref()
                        .context("published dashboard policy absent")?,
                    process,
                )?;
                ensure!(
                    binding.listen == spec.upstream
                        && binding.local_socket == spec.upstream_unix
                        && value["endpoint"]["execution_identity"]
                            == serde_json::to_value(run.object()?)?
                        && value["endpoint"]["dashboard_source"] == spec.dashboard_source
                        && value["endpoint"]["url"] == spec.public_url(),
                    "remote gateway publication policy differs"
                );
                gateway.manifest = value;
                break;
            }
            ensure!(
                gateway.child.try_wait()?.is_none() && Instant::now() < deadline,
                "remote gateway startup failed; inspect its retained stderr.log"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        ensure!(
            TcpStream::connect_timeout(&spec.listen, Duration::from_secs(3)).is_ok(),
            "selected TLS listener not reachable"
        );
        Ok(gateway)
    }
    pub fn manifest(&self) -> &Value {
        &self.manifest
    }
    pub fn wait_stopped(&mut self) -> Result<()> {
        if self.stopped {
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = self.child.try_wait()? {
                break status;
            }
            ensure!(
                Instant::now() < deadline,
                "remote gateway did not acknowledge shutdown"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        ensure!(
            status.success()
                && TcpStream::connect_timeout(&self.spec.listen, Duration::from_secs(1)).is_err(),
            "selected gateway exited unsuccessfully or listener survived"
        );
        self.stopped = true;
        save_json(
            &self.directory.join("stopped.json"),
            &json!({"run_id":self.spec.run_id,"acknowledged":true}),
        )?;
        Ok(())
    }
    pub fn stop(&mut self) -> Result<()> {
        self.child.stdin.take();
        self.wait_stopped()
    }
}
impl Drop for RemoteGateway {
    fn drop(&mut self) {
        if !self.stopped && self.stop().is_err() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

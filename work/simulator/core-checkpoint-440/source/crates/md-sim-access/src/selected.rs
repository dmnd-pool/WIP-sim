//! Publish only the incoming endpoint of the selected active worker process.
use crate::gateway::{Gateway, GatewaySpec};
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::LockedRun;
use md_sim_runtime::{
    dashboard::{Binding, Source},
    isolation::save_json,
    worker::{self, Request},
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct SelectedGateway {
    gateway: Gateway,
    socket: PathBuf,
    run_id: String,
    run: LockedRun,
    binding: Binding,
    directory: PathBuf,
    stopped: bool,
}

fn checked_binding(status: &Value, run_id: &str, run: &LockedRun) -> Result<Binding> {
    run.validate()?;
    ensure!(
        status["run_id"] == run_id && status["state"]["state"] == "booting",
        "selected worker is not active"
    );
    let process = run
        .process
        .as_ref()
        .context("selected run has no executable process")?;
    let policy = process
        .dashboard
        .as_ref()
        .context("selected run has no dashboard")?;
    let binding: Binding = serde_json::from_value(status["dashboard"].clone())?;
    binding.verify(policy, process)?;
    Ok(binding)
}

impl SelectedGateway {
    pub fn start(
        socket: &Path,
        run: LockedRun,
        spec: &GatewaySpec,
        directory: &Path,
    ) -> Result<Self> {
        let status = worker::request(
            socket,
            &Request::Status {
                run_id: spec.run_id.clone(),
            },
        )?;
        let binding = checked_binding(&status, &spec.run_id, &run)?;
        let source = match binding.source {
            Source::Native => "native",
            Source::AdaptedNative => "adapted-native",
            Source::SimulatorConsole => "simulator-console",
        };
        ensure!(
            spec.execution_identity == run.object()?
                && spec.upstream == binding.listen
                && spec.dashboard_source == source
                && spec.upstream_tls.is_none()
                && binding.local_socket.is_some()
                && spec.upstream_unix == binding.local_socket,
            "gateway does not match the selected execution and incoming bridge"
        );
        let gateway = Gateway::start(spec, directory)?;
        // Close the endpoint if the run finished during TLS process startup.
        let status = worker::request(
            socket,
            &Request::Status {
                run_id: spec.run_id.clone(),
            },
        )?;
        let confirmed = checked_binding(&status, &spec.run_id, &run)?;
        ensure!(
            serde_json::to_value(&confirmed)? == serde_json::to_value(&binding)?,
            "worker endpoint changed during publication"
        );
        let selected = Self {
            gateway,
            socket: socket.into(),
            run_id: spec.run_id.clone(),
            run,
            binding,
            directory: directory.into(),
            stopped: false,
        };
        save_json(&directory.join("selected.json"), &selected.manifest()?)?;
        Ok(selected)
    }
    pub fn manifest(&self) -> Result<Value> {
        Ok(
            json!({"schema":"md-sim-selected-worker-gateway/1","run_id":self.run_id,
            "locked_run":self.run.object()?,"binding":self.binding,"endpoint":self.gateway.manifest()?,
            "firmware_readiness_established":false,"native_dashboard_gate":false,
            "lifecycle":"Publication is checked against the selected worker before and after startup and on each supervisor poll."}),
        )
    }
    /// False means the selected execution ended and its gateway has stopped.
    /// Lost status, changed metadata or a failed proxy is an error, never a
    /// reason to keep a stale listener available for a reused port.
    pub fn poll(&mut self) -> Result<bool> {
        if self.stopped {
            return Ok(false);
        }
        let outcome = (|| -> Result<bool> {
            ensure!(self.gateway.is_running()?, "selected TLS gateway exited");
            let status = worker::request(
                &self.socket,
                &Request::Status {
                    run_id: self.run_id.clone(),
                },
            )?;
            ensure!(
                status["run_id"] == self.run_id,
                "worker status changed selected run"
            );
            if status["dashboard"].is_null() {
                return Ok(false);
            }
            let binding = checked_binding(&status, &self.run_id, &self.run)?;
            ensure!(
                serde_json::to_value(binding)? == serde_json::to_value(&self.binding)?,
                "selected worker endpoint changed"
            );
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
                &self.directory.join("selected-stop.json"),
                &json!({"run_id":self.run_id,"gateway_stopped":true}),
            )?;
        }
        Ok(())
    }
}
impl Drop for SelectedGateway {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

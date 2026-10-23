//! Actual SSH inspection and publication for the selected carrier endpoint.
//! Metadata travels through worker Put/Get; no host store path is assumed.
use crate::{
    carrier::{self, CarrierEndpoint, ProviderBinding, Selection},
    carrier_gateway::verify_spec,
    gateway::GatewaySpec,
};
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::{executor::transfer_object, LockedRun, OwnedSshChild, SshTransport};
use md_sim_runtime::{
    isolation::save_json,
    store::{ObjectId, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

pub struct CarrierInputs<'a> {
    pub run: &'a LockedRun,
    pub policy: &'a Path,
    pub provider: &'a Path,
    pub token: &'a str,
}

#[derive(Clone, Copy)]
pub struct CarrierRemotePaths<'a> {
    pub access_program: &'a str,
    pub socket: &'a Path,
    pub ready: &'a Path,
    pub endpoint: &'a Path,
    pub output: &'a Path,
}

pub struct CarrierInspection {
    pub endpoint: CarrierEndpoint,
    pub record: Value,
}

fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= limit as u64,
        "carrier metadata type or size bound"
    );
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "carrier metadata grew beyond bound");
    Ok(bytes)
}

struct Prepared {
    directory: PathBuf,
    binding: ProviderBinding,
    objects: BTreeMap<String, ObjectId>,
}

fn prepare(
    transport: &mut SshTransport,
    inputs: &CarrierInputs<'_>,
    spec: Option<&GatewaySpec>,
    output: &Path,
) -> Result<Prepared> {
    inputs.run.validate()?;
    let policy = read(inputs.policy, 65536)?;
    let provider = read(inputs.provider, 4 << 20)?;
    let policy_id = carrier::object(&policy);
    let selected = Selection::new(inputs.run, policy_id, &policy, inputs.token)?;
    let binding = ProviderBinding::from_config(selected, carrier::object(&provider), &provider)?;
    fs::DirBuilder::new().mode(0o700).create(output)?;
    let directory = output.canonicalize()?;
    let store = Store::open(&directory.join("input-store"), 16 << 20)?;
    let mut bodies = vec![
        ("locked_run", serde_json::to_vec_pretty(inputs.run)?),
        ("policy", policy),
        ("provider", provider),
    ];
    if let Some(spec) = spec {
        bodies.push(("gateway", serde_json::to_vec_pretty(spec)?));
    }
    let mut objects = BTreeMap::new();
    for (name, bytes) in bodies {
        let object = carrier::object(&bytes);
        let path = directory.join(format!("{name}.json"));
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?
            .write_all(&bytes)?;
        store.import(&path, &object)?;
        transfer_object(transport, &store, &object, &AtomicBool::new(false))?;
        objects.insert(name.into(), object);
    }
    save_json(
        &directory.join("transfer.json"),
        &json!({"verified_at_worker":objects,
        "transport":transport.evidence(),"server_retrieval":"separate hash-checked Worker Get; host store path not exposed"}),
    )?;
    Ok(Prepared {
        directory,
        binding,
        objects,
    })
}

fn id_token(object: &ObjectId) -> String {
    format!("{}:{}", object.sha256, object.size)
}

fn command(
    transport: &SshTransport,
    prepared: &Prepared,
    paths: CarrierRemotePaths<'_>,
    publish: bool,
) -> Result<Command> {
    let mut arguments = vec![if publish {
        "serve-carrier-connected"
    } else {
        "inspect-carrier"
    }
    .into()];
    if publish {
        arguments.push(id_token(&prepared.objects["gateway"]));
    }
    for name in ["locked_run", "policy", "provider"] {
        arguments.push(id_token(&prepared.objects[name]));
    }
    arguments.push(prepared.binding.selection().identity().token.clone());
    for path in [paths.socket, paths.ready, paths.endpoint, paths.output] {
        arguments.push(
            path.to_str()
                .context("carrier remote path encoding")?
                .into(),
        );
    }
    transport.service_command(
        paths.access_program,
        &arguments.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

fn child_output(directory: &Path, command: &mut Command, connected: bool) -> Result<OwnedSshChild> {
    command
        .stdin(if connected {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(File::create_new(directory.join("stdout.json"))?)
        .stderr(File::create_new(directory.join("stderr.log"))?);
    OwnedSshChild::spawn_service(command, 60, 1 << 20)
}

fn validate_inputs(record: &Value, prepared: &Prepared) -> Result<()> {
    ensure!(
        record["input_objects"] == serde_json::to_value(&prepared.objects)?,
        "server fetched a different metadata selection"
    );
    Ok(())
}

fn validate_publication(
    record: &Value,
    endpoint: &CarrierEndpoint,
    spec: &GatewaySpec,
) -> Result<()> {
    ensure!(
        record["schema"] == "md-sim-selected-carrier-gateway/1"
            && record["carrier_endpoint"] == serde_json::to_value(endpoint)?
            && record["run_id"] == spec.run_id
            && record["locked_run"] == serde_json::to_value(&spec.execution_identity)?
            && record["endpoint"]["schema"] == "md-sim-dashboard-endpoint/1"
            && record["endpoint"]["run_id"] == spec.run_id
            && record["endpoint"]["execution_identity"]
                == serde_json::to_value(&spec.execution_identity)?
            && record["endpoint"]["url"] == spec.public_url()
            && record["endpoint"]["upstream"] == format!("http://{}", spec.upstream)
            && record["endpoint"]["upstream_unix"] == serde_json::to_value(&spec.upstream_unix)?
            && record["endpoint"]["dashboard_source"] == spec.dashboard_source
            && record["endpoint"]["browser_policy"] == serde_json::to_value(spec.browser_policy)?
            && record["endpoint"]["client_access_rules"]
                == serde_json::to_value(&spec.allow_clients)?
            && record["endpoint"]["cookie_policy"]["mode"] == "isolated-browser-context"
            && record["endpoint"]["cookie_policy"]["context_id"] == spec.browser_context_id
            && record["firmware_readiness_established"] == false
            && record["native_dashboard_gate"] == false,
        "carrier gateway published another execution or endpoint"
    );
    Ok(())
}

pub struct RemoteCarrierGateway {
    child: OwnedSshChild,
    directory: PathBuf,
    spec: GatewaySpec,
    manifest: Value,
    stopped: bool,
}

// A publication identifies the selected service. It does not prove that its
// child has reached bind/listen. Keep both phases inside one startup budget.
fn wait_listener(
    child: &OwnedSshChild,
    address: SocketAddr,
    started: Instant,
    deadline: Instant,
    evidence: &Path,
) -> Result<()> {
    let mut attempts = Vec::new();
    let outcome = (|| -> Result<()> {
        loop {
            ensure!(
                !child.exited()?,
                "carrier SSH child exited before TLS readiness"
            );
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .context("selected carrier TLS listener startup deadline")?;
            let began = Instant::now();
            let result =
                TcpStream::connect_timeout(&address, remaining.min(Duration::from_secs(3)));
            attempts.push(json!({"elapsed_milliseconds":started.elapsed().as_millis(),
                "connect_milliseconds":began.elapsed().as_millis(),
                "connected":result.is_ok(),
                "error":result.as_ref().err().map(ToString::to_string),
                "error_kind":result.as_ref().err().map(|error|format!("{:?}",error.kind())),
                "raw_os_error":result.as_ref().err().and_then(std::io::Error::raw_os_error)}));
            ensure!(
                Instant::now() < deadline,
                "selected carrier TLS listener startup deadline"
            );
            ensure!(
                !child.exited()?,
                "carrier SSH child exited during TLS readiness"
            );
            if result.is_ok() {
                return Ok(());
            }
            // The caller supplies at most30s, so this interval also bounds
            // the receipt to601 attempts even for immediate refusal.
            std::thread::sleep(
                Duration::from_millis(50).min(deadline.saturating_duration_since(Instant::now())),
            );
        }
    })();
    save_json(
        evidence,
        &json!({"schema":"md-sim-carrier-tls-listener-readiness/1",
        "address":address,"elapsed_milliseconds":started.elapsed().as_millis(),
        "startup_budget_milliseconds":deadline.saturating_duration_since(started).as_millis(),
        "attempts":attempts,"reachable":outcome.is_ok(),
        "error":outcome.as_ref().err().map(|error|format!("{error:#}")),
        "scope":"TCP listener reachability only; TLS authentication and selected native HTTP require their own checks"}),
    )?;
    outcome
}

impl RemoteCarrierGateway {
    pub fn inspect(
        transport: &mut SshTransport,
        inputs: &CarrierInputs<'_>,
        paths: CarrierRemotePaths<'_>,
        output: &Path,
    ) -> Result<CarrierInspection> {
        let prepared = prepare(transport, inputs, None, output)?;
        let mut owned = child_output(
            &prepared.directory,
            &mut command(transport, &prepared, paths, false)?,
            false,
        )?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if owned.exited()? {
                let status = owned.finish()?;
                ensure!(
                    status.success(),
                    "carrier inspection failed; inspect retained stderr.log"
                );
                break;
            }
            ensure!(Instant::now() < deadline, "carrier SSH inspection deadline");
            std::thread::sleep(Duration::from_millis(20));
        }
        let record: Value =
            serde_json::from_slice(&read(&prepared.directory.join("stdout.json"), 128 << 10)?)?;
        ensure!(
            record["schema"] == "md-sim-carrier-endpoint-inspection/1"
                && record["firmware_readiness_established"] == false,
            "carrier inspection schema or readiness claim differs"
        );
        validate_inputs(&record, &prepared)?;
        let endpoint: CarrierEndpoint = serde_json::from_value(record["endpoint"].clone())?;
        endpoint.verify(&prepared.binding)?;
        save_json(&prepared.directory.join("inspection.json"), &record)?;
        Ok(CarrierInspection { endpoint, record })
    }

    pub fn start(
        transport: &mut SshTransport,
        inputs: &CarrierInputs<'_>,
        spec: &GatewaySpec,
        endpoint: &CarrierEndpoint,
        paths: CarrierRemotePaths<'_>,
        output: &Path,
    ) -> Result<Self> {
        let prepared = prepare(transport, inputs, Some(spec), output)?;
        verify_spec(&prepared.binding, endpoint, spec)?;
        let child = child_output(
            &prepared.directory,
            &mut command(transport, &prepared, paths, true)?,
            true,
        )?;
        let mut service = Self {
            child,
            directory: prepared.directory.clone(),
            spec: spec.clone(),
            manifest: Value::Null,
            stopped: false,
        };
        let started = Instant::now();
        let deadline = started + Duration::from_secs(30);
        loop {
            ensure!(
                !service.child.exited()? && Instant::now() < deadline,
                "carrier TLS startup failed; inspect retained stderr.log"
            );
            let bytes = read(&service.directory.join("stdout.json"), 128 << 10)?;
            if let Ok(record) = serde_json::from_slice::<Value>(&bytes) {
                validate_publication(&record, endpoint, spec)?;
                validate_inputs(&record, &prepared)?;
                service.manifest = record;
                break;
            }
            std::thread::sleep(
                Duration::from_millis(20).min(deadline.saturating_duration_since(Instant::now())),
            );
        }
        wait_listener(
            &service.child,
            spec.listen,
            started,
            deadline,
            &service.directory.join("listener-readiness.json"),
        )?;
        save_json(
            &service.directory.join("publication.json"),
            &service.manifest,
        )?;
        Ok(service)
    }

    pub fn manifest(&self) -> &Value {
        &self.manifest
    }

    pub fn stop(&mut self) -> Result<()> {
        if self.stopped {
            return Ok(());
        }
        self.child.close_stdin();
        let deadline = Instant::now() + Duration::from_secs(15);
        let status = loop {
            if self.child.exited()? {
                break self.child.finish()?;
            }
            ensure!(
                Instant::now() < deadline,
                "carrier gateway shutdown deadline"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        ensure!(
            status.success()
                && TcpStream::connect_timeout(&self.spec.listen, Duration::from_secs(1)).is_err(),
            "carrier gateway did not acknowledge listener shutdown"
        );
        self.stopped = true;
        save_json(
            &self.directory.join("stopped.json"),
            &json!({"run_id":self.spec.run_id,"acknowledged":true}),
        )
    }
}

#[cfg(test)]
#[path = "remote_carrier_readiness.rs"]
mod readiness_tests;

impl Drop for RemoteCarrierGateway {
    fn drop(&mut self) {
        if !self.stopped && self.stop().is_err() {
            let _ = self.child.finish();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carrier_gateway::tests::gateway_fixture;

    #[test]
    fn publication_rejects_changed_run_endpoint_browser_or_success_claim() -> Result<()> {
        let (_, endpoint, spec) = gateway_fixture()?;
        let record = json!({"schema":"md-sim-selected-carrier-gateway/1", "carrier_endpoint":endpoint,
            "run_id":spec.run_id,"locked_run":spec.execution_identity,"firmware_readiness_established":false,
            "native_dashboard_gate":false,"endpoint":{"schema":"md-sim-dashboard-endpoint/1",
            "run_id":spec.run_id,"execution_identity":spec.execution_identity,"url":spec.public_url(),
            "upstream":format!("http://{}",spec.upstream),"upstream_unix":spec.upstream_unix,
            "dashboard_source":spec.dashboard_source,"browser_policy":spec.browser_policy,
            "client_access_rules":spec.allow_clients,"cookie_policy":{"mode":"isolated-browser-context",
            "context_id":spec.browser_context_id}}});
        validate_publication(&record, &endpoint, &spec)?;
        for (pointer, value) in [
            ("/run_id", json!("another-run")),
            ("/locked_run", json!(carrier::object(b"other lock"))),
            (
                "/carrier_endpoint/provider_configuration",
                json!(carrier::object(b"other provider")),
            ),
            (
                "/endpoint/execution_identity",
                json!(carrier::object(b"other execution")),
            ),
            ("/endpoint/upstream", json!("http://127.0.0.1:1")),
            ("/endpoint/upstream_unix", json!("/other/http.sock")),
            (
                "/endpoint/browser_policy",
                json!("upgrade-insecure-requests"),
            ),
            ("/endpoint/client_access_rules", json!(["0.0.0.0/0"])),
            (
                "/endpoint/cookie_policy/context_id",
                json!("different-context"),
            ),
            ("/firmware_readiness_established", json!(true)),
            ("/native_dashboard_gate", json!(true)),
        ] {
            let mut changed = record.clone();
            *changed
                .pointer_mut(pointer)
                .context("fixture pointer missing")? = value;
            assert!(
                validate_publication(&changed, &endpoint, &spec).is_err(),
                "accepted {pointer}"
            );
        }
        Ok(())
    }

    #[test]
    fn owned_service_limits_retained_output_before_it_is_read() -> Result<()> {
        let root = tempfile::tempdir()?;
        let mut owned = child_output(
            root.path(),
            Command::new("/usr/bin/head").args(["-c", "2097152", "/dev/zero"]),
            false,
        )?;
        let deadline = Instant::now() + Duration::from_secs(3);
        while !owned.exited()? {
            ensure!(Instant::now() < deadline, "log writer deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
        let status = owned.finish()?;
        assert!(!status.success());
        assert_eq!(
            fs::metadata(root.path().join("stdout.json"))?.len(),
            1 << 20
        );
        assert!(fs::metadata(root.path().join("stderr.log"))?.len() <= 1 << 20);
        Ok(())
    }

    #[test]
    fn completed_leader_retains_group_identity_until_descendant_cleanup() -> Result<()> {
        let root = tempfile::tempdir()?;
        let descendant = root.path().join("descendant.pid");
        let mut owned = child_output(
            root.path(),
            Command::new("/bin/sh")
                .args([
                    "-c",
                    "/usr/bin/sleep 60 & printf '%s\\n' \"$!\" > \"$1\"",
                    "carrier-child-fixture",
                ])
                .arg(&descendant),
            false,
        )?;
        let deadline = Instant::now() + Duration::from_secs(3);
        while !owned.exited()? {
            ensure!(Instant::now() < deadline, "leader completion deadline");
            std::thread::sleep(Duration::from_millis(10));
        }
        let pid: u32 = fs::read_to_string(&descendant)?.trim().parse()?;
        let alive = || -> Result<bool> {
            match fs::read_to_string(format!("/proc/{pid}/stat")) {
                Ok(stat) => Ok(!stat
                    .rsplit_once(") ")
                    .context("process stat shape")?
                    .1
                    .starts_with(['Z', 'X'])),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
                Err(error) => Err(error.into()),
            }
        };
        ensure!(alive()?, "fixture descendant did not survive leader exit");
        assert!(owned.finish()?.success());
        while alive()? {
            ensure!(
                Instant::now() < deadline,
                "descendant survived owned cleanup"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    }
}

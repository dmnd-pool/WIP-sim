//! Exercise the selected firmware through the separately owned carrier service.
//! The native worker has no dashboard binding. All access uses its actual
//! carrier identity and the server's authenticated, leased endpoint.
use super::{board_dashboard_probe, worker_broker::bounded};
use anyhow::{ensure, Context, Result};
use md_sim_access::{
    gateway::{BrowserPolicy, GatewaySpec},
    remote_carrier::{CarrierInputs, CarrierRemotePaths, RemoteCarrierGateway},
};
use md_sim_coordinator::{ssh_transport::SshTransport, LockedRun, WorkerTransport};
use md_sim_runtime::{isolation::save_json, store::hash_file, worker::Request};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const ROOT: &str = "/work/carrier-client";

pub struct Observer {
    cancel: Arc<AtomicBool>,
    join: Option<JoinHandle<Result<Value>>>,
}

impl Observer {
    pub fn start(run: &LockedRun) -> Result<Option<Self>> {
        if run.policies.get("carrier_dashboard").is_none() {
            return Ok(None);
        }
        fs::create_dir(ROOT)?;
        let run = run.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancellation = cancel.clone();
        let join = thread::spawn(move || {
            let mut observations = Vec::new();
            let outcome = exercise(&run, &cancellation, &mut observations);
            let report = json!({"schema":"md-sim-shared-carrier-client/1",
                "passed":outcome.is_ok(),"result":outcome.as_ref().ok(),
                "error":outcome.as_ref().err().map(|error|format!("{error:#}")),
                "locked_run":run.object()?,"observations":observations,
                "topology":"Separate client network namespace in disconnected support VM; native worker outside VM. No external physical-host claim.",
                "scope":"Selected native HTTP over SSH and mutually authenticated TLS. Browser and restart persistence are separate results."});
            save_json(&Path::new(ROOT).join("report.json"), &report)?;
            Ok(report)
        });
        Ok(Some(Self {
            cancel,
            join: Some(join),
        }))
    }

    pub fn finish(&mut self) -> Result<Value> {
        self.join
            .take()
            .context("carrier observer already joined")?
            .join()
            .map_err(|_| anyhow::anyhow!("carrier observer panicked"))?
    }
}

impl Drop for Observer {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn paths<'a>(output: &'a Path) -> CarrierRemotePaths<'a> {
    CarrierRemotePaths {
        access_program: "/md-sim-access",
        socket: Path::new("/worker/worker.sock"),
        endpoint: Path::new("/work/access/bridge/endpoint.json"),
        ready: Path::new("/work/access/ready.json"),
        output,
    }
}

fn exercise(run: &LockedRun, cancel: &AtomicBool, observations: &mut Vec<Value>) -> Result<Value> {
    let started = Instant::now();
    let process = run
        .process
        .as_ref()
        .context("selected carrier process absent")?;
    let transfer_deadline =
        Instant::now() + Duration::from_secs(super::shared_pool::SERVER_SECONDS);
    while !Path::new("/work/attempt/worker-run.json").exists() {
        ensure!(
            !cancel.load(Ordering::Acquire) && Instant::now() < transfer_deadline,
            "carrier observer cancelled or execution did not start"
        );
        thread::sleep(Duration::from_millis(50));
    }
    let deadline = Instant::now() + Duration::from_secs(process.limits.wall_seconds);
    let request: Value = serde_json::from_slice(&bounded(
        Path::new("/work/attempt/worker-request.json"),
        65536,
    )?)?;
    let selected: Value =
        serde_json::from_slice(&bounded(Path::new("/work/attempt/worker-run.json"), 65536)?)?;
    let token = request["token"]
        .as_str()
        .context("selected worker token absent")?;
    let run_id = selected["run_id"]
        .as_str()
        .context("selected worker run absent")?;
    ensure!(
        request["locked_run_hash"] == run.hash()? && run_id == format!("run-{token}"),
        "carrier observer selected another execution"
    );
    let mut browser = super::board_browser::Client::start()?;
    let inputs = CarrierInputs {
        run,
        policy: Path::new("/carrier-dashboard.json"),
        provider: Path::new("/provider.json"),
        token,
    };
    let mut transport = SshTransport::new_streaming(super::ssh_config())?;
    let inspected = RemoteCarrierGateway::inspect(
        &mut transport,
        &inputs,
        paths(Path::new("/work/access/inspection")),
        &Path::new(ROOT).join("inspection"),
    )?;
    let endpoint = inspected.endpoint;
    let mut tunnel = super::SshTunnel::start(
        super::ssh_config(),
        "127.0.0.1:18090".parse()?,
        endpoint.listen,
        &Path::new(ROOT).join("ssh-tunnel.log"),
    )?;
    observations.push(json!({"selected_run":run_id,"endpoint":endpoint,
        "tunnel":tunnel.evidence(),"elapsed_milliseconds":started.elapsed().as_millis()}));
    let outcome: Result<Value> = (|| {
        ready(deadline, cancel, observations)?;
        let http = board_dashboard_probe::run_selected(
            deadline,
            "127.0.0.1:18090".parse()?,
            &Path::new(ROOT).join("http"),
        )?;
        let mut context = [0; 32];
        File::open("/dev/urandom")?.read_exact(&mut context)?;
        let spec = GatewaySpec {
            run_id: run_id.into(),
            execution_identity: run.object()?,
            dashboard_source: "adapted-native".into(),
            listen: format!("{}:9443", super::WORKER).parse()?,
            upstream: endpoint.listen,
            upstream_unix: Some(endpoint.local_socket.clone()),
            upstream_tls: None,
            // The archived application uses ws://. This gateway response header
            // records that adaptation without changing archived asset bytes.
            browser_policy: BrowserPolicy::UpgradeInsecureRequests,
            certificate: "/worker/access-credentials/server.crt".into(),
            private_key: "/worker/access-credentials/server.key".into(),
            client_ca: "/worker/access-credentials/ca.crt".into(),
            allow_clients: vec![format!("{}/32", super::CLIENT)],
            browser_context_id: hex::encode(context),
            nginx: "/usr/bin/nginx".into(),
            openssl: "/usr/bin/openssl".into(),
        };
        let mut gateway = RemoteCarrierGateway::start(
            &mut transport,
            &inputs,
            &spec,
            &endpoint,
            paths(Path::new("/work/access/gateway")),
            &Path::new(ROOT).join("gateway"),
        )?;
        let publication = gateway.manifest().clone();
        save_json(&Path::new(ROOT).join("publication.json"), &publication)?;
        let tls = (|| {
            reject_unauthenticated(&spec, observations)?;
            let tls = board_dashboard_probe::run_selected_tls(
                deadline,
                spec.listen,
                &Path::new(ROOT).join("tls"),
                board_dashboard_probe::TlsClient {
                    ca: "/credentials/ca.crt".into(),
                    certificate: "/credentials/client.crt".into(),
                    private_key: "/credentials/client.key".into(),
                    selected_run: run_id.into(),
                },
            )?;
            let browser = browser
                .as_mut()
                .map(|browser| {
                    browser.exercise_carrier(&super::board_browser_target::CarrierSelection {
                        schema: "md-sim-selected-carrier-boser-browser/1".into(),
                        selected_run: run_id.into(),
                        run: run.clone(),
                        policy: hash_file(inputs.policy)?,
                        policy_bytes: String::from_utf8(bounded(inputs.policy, 65536)?)?,
                        provider: hash_file(inputs.provider)?,
                        provider_bytes: String::from_utf8(bounded(inputs.provider, 4 << 20)?)?,
                        endpoint: endpoint.clone(),
                        http: "127.0.0.1:18090".parse()?,
                        publication: publication.clone(),
                    })
                })
                .transpose()?;
            Ok::<_, anyhow::Error>((tls, browser))
        })();
        let stopped = gateway.stop();
        observations.push(json!({"gateway_stopped":stopped.is_ok(),
            "stop_error":stopped.as_ref().err().map(|error|format!("{error:#}"))}));
        let (tls, browser) = tls?;
        stopped?;
        let status = transport.request(&Request::Status {
            run_id: run_id.into(),
        })?;
        ensure!(
            status["run_id"] == run_id && status["state"]["state"] == "booting",
            "selected firmware ended before access checks finished"
        );
        Ok(
            json!({"selected_run":run_id,"endpoint":endpoint,"http":http,"tls":tls,
            "publication":publication,"final_worker_status":status,"browser_tested":browser.is_some(),"browser":browser}),
        )
    })();
    let browser_finished = browser.as_mut().map(|browser| browser.finish()).transpose();
    if let Ok(Some(evidence)) = &browser_finished {
        save_json(&Path::new(ROOT).join("browser-process.json"), evidence)?;
    }
    let stopped = tunnel.stop();
    observations.push(json!({"tunnel_stopped":stopped.is_ok(),
        "stop_error":stopped.as_ref().err().map(|error|format!("{error:#}"))}));
    let result = outcome?;
    browser_finished?;
    stopped?;
    Ok(result)
}

fn ready(deadline: Instant, cancel: &AtomicBool, observations: &mut Vec<Value>) -> Result<()> {
    let mut index = 0;
    loop {
        ensure!(
            !cancel.load(Ordering::Acquire) && Instant::now() < deadline && index < 120,
            "native carrier HTTP readiness deadline"
        );
        let prefix = Path::new(ROOT).join(format!("ready-{index}"));
        let output = Command::new("/usr/bin/curl")
            .env_clear()
            .args([
                "--noproxy",
                "*",
                "--silent",
                "--show-error",
                "--proto",
                "=http",
                "--connect-timeout",
                "1",
                "--max-time",
                "3",
                "--max-filesize",
                "4096",
                "--write-out",
                "%{http_code}",
                "--output",
            ])
            .arg(prefix.with_extension("body"))
            .arg("http://127.0.0.1:18090/")
            .stdin(Stdio::null())
            .output()?;
        let status = String::from_utf8(output.stdout)?;
        save_json(
            &prefix.with_extension("json"),
            &json!({"status":status,"exit_code":output.status.code(),
            "stderr":String::from_utf8(output.stderr)?}),
        )?;
        observations.push(
            json!({"readiness_attempt":index,"status":status,"exit_code":output.status.code()}),
        );
        if status == "200" && output.status.success() {
            let bytes = bounded(&prefix.with_extension("body"), 4096)?;
            ensure!(
                bytes.len() == 2044
                    && md_sim_firmware::prepare::hash_bytes(&bytes)
                        == "d0d769811cbf2e5290dafb5f9f9a005bcafd45469e94899080353093e1f20947",
                "carrier native page differs from archived bytes"
            );
            return Ok(());
        }
        ensure!(
            status == "000"
                || (status == "200" && output.status.code() == Some(28))
                || (status == "408" && output.status.success()),
            "native carrier HTTP response failed"
        );
        index += 1;
        thread::sleep(Duration::from_secs(1));
    }
}

fn reject_unauthenticated(spec: &GatewaySpec, observations: &mut Vec<Value>) -> Result<()> {
    let denied = Command::new("/usr/bin/curl")
        .env_clear()
        .args([
            "--noproxy",
            "*",
            "--silent",
            "--show-error",
            "--fail",
            "--proto",
            "=https",
            "--connect-timeout",
            "2",
            "--max-time",
            "5",
            "--cacert",
            "/credentials/ca.crt",
            "--write-out",
            "%{http_code}",
            "--output",
        ])
        .arg(Path::new(ROOT).join("unauthenticated.body"))
        .arg(spec.public_url())
        .stdin(Stdio::null())
        .output()?;
    let status = String::from_utf8(denied.stdout)?;
    let evidence = json!({"status":status,"exit_code":denied.status.code(),
        "stderr":String::from_utf8(denied.stderr)?,"url":spec.public_url()});
    save_json(&Path::new(ROOT).join("unauthenticated.json"), &evidence)?;
    observations.push(evidence);
    ensure!(
        !denied.status.success() && ["400", "403"].contains(&status.as_str()),
        "TLS request without client credentials did not produce an authentication rejection"
    );
    Ok(())
}

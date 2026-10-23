//! Reach the HTTP service of the actual selected board process, while it runs.
use super::*;
use md_sim_access::remote::{RemoteGateway, RemotePaths};
use md_sim_coordinator::{ssh_transport::SshTransport, WorkerTransport};
use md_sim_runtime::{dashboard::Binding, isolation::ProcessSpec, worker::Request};

const READINESS_INTERVAL_SECONDS: u64 = 10;

fn readiness_limit(wall_seconds: u64) -> Result<usize> {
    ensure!(wall_seconds > 0, "dashboard process wall budget absent");
    Ok(usize::try_from(
        wall_seconds.div_ceil(READINESS_INTERVAL_SECONDS) + 1,
    )?)
}

fn readiness_complete(status: &str, curl_exit: Option<i32>) -> Result<bool> {
    // A header can arrive before the body. Local163 received200, then curl28
    // with zero of2044 body bytes. Keep this failed attempt and poll again;
    // only a complete200 response may reach the archived-body hash check.
    // A native408 means this GET expired while the server waited for its
    // request. Retain it and retry within the existing readiness budget.
    // It grants no readiness; the complete200 body still must match.
    if status == "000"
        || (status == "200" && curl_exit == Some(28))
        || (status == "408" && curl_exit == Some(0))
    {
        return Ok(false);
    }
    ensure!(
        status == "200" && curl_exit == Some(0),
        "native dashboard initial HTTP response failed"
    );
    Ok(true)
}

pub(super) struct Probe {
    spec: ProcessSpec,
    local: bool,
    started: Instant,
    readiness_limit: usize,
    next: Instant,
    binding: Option<Binding>,
    tunnel: Option<SshTunnel>,
    attempts: Vec<Value>,
    outcome: Option<Value>,
    error: Option<String>,
    selected: Option<String>,
    publication: Option<Value>,
    tls_outcome: Option<Value>,
    tls_stopped: bool,
    no_certificate_rejected: bool,
    browser: Option<super::board_browser::Client>,
    browser_outcome: Option<Value>,
}
impl Probe {
    pub(super) fn new(spec: ProcessSpec, local: bool) -> Result<Self> {
        let readiness_limit = readiness_limit(spec.limits.wall_seconds)?;
        Ok(Self {
            spec,
            local,
            started: Instant::now(),
            readiness_limit,
            next: Instant::now(),
            binding: None,
            tunnel: None,
            attempts: Vec::new(),
            outcome: None,
            error: None,
            selected: None,
            publication: None,
            tls_outcome: None,
            tls_stopped: false,
            no_certificate_rejected: false,
            browser: super::board_browser::Client::start()?,
            browser_outcome: None,
        })
    }
    pub(super) fn diagnostic(&self) -> Value {
        json!({"diagnostic_only":true,"provisional":true,
            "selected_run":self.selected,"client_elapsed_ms":self.started.elapsed().as_millis(),
            "readiness_attempts":self.attempts.len(),"readiness_limit":self.readiness_limit,"last_readiness_attempt":self.attempts.last(),
            "native_probe_finished":self.outcome.is_some(),
            "browser_result_present":self.browser_outcome.is_some(),
            "browser_diagnostic":self.browser.as_ref().and_then(|b|b.diagnostic()),"error":self.error})
    }
    pub(super) fn poll(&mut self, out: &Path, run: Option<&str>) {
        if self.spec.dashboard.is_none()
            || self.outcome.is_some()
            || self.error.is_some()
            || Instant::now() < self.next
        {
            return;
        }
        self.next = Instant::now() + Duration::from_secs(READINESS_INTERVAL_SECONDS);
        if let Some(run) = run {
            if let Err(error) = self.step(out, run) {
                self.error = Some(format!("{error:#}"));
            }
        }
    }
    fn step(&mut self, out: &Path, run: &str) -> Result<()> {
        if self.binding.is_none() {
            let policy = self
                .spec
                .dashboard
                .as_ref()
                .context("dashboard policy absent")?;
            let binding = if self.local {
                let path = out.join("dashboard.json");
                if !path.exists() {
                    return Ok(());
                }
                Binding::read(&path, policy, &self.spec)?
            } else {
                let status = SshTransport::new(ssh_config())?
                    .request(&Request::Status { run_id: run.into() })?;
                ensure!(
                    status["run_id"] == run,
                    "dashboard status selected another run"
                );
                if status["dashboard"].is_null() {
                    return Ok(());
                }
                let binding: Binding = serde_json::from_value(status["dashboard"].clone())?;
                binding.verify(policy, &self.spec)?;
                binding
            };
            ensure!(
                binding.source == md_sim_runtime::dashboard::Source::AdaptedNative,
                "board dashboard source is not adapted native firmware"
            );
            if !self.local {
                self.tunnel = Some(SshTunnel::start(
                    ssh_config(),
                    "127.0.0.1:18090".parse()?,
                    binding.listen,
                    Path::new("/work/board-dashboard-tunnel.log"),
                )?);
            }
            self.binding = Some(binding);
            self.selected = Some(run.into());
        }
        ensure!(
            self.selected.as_deref() == Some(run),
            "selected dashboard run changed"
        );
        let endpoint = if self.local {
            self.binding
                .as_ref()
                .context("dashboard binding absent")?
                .listen
        } else {
            "127.0.0.1:18090".parse()?
        };
        ensure!(
            self.attempts.len() < self.readiness_limit,
            "native dashboard readiness observation bound"
        );
        // The bridge's listener is not a firmware readiness signal. Observe
        // the native HTTP response; retain every bounded readiness failure.
        let result = Command::new("/usr/bin/curl")
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
                "--output",
                "/work/board-http-ready.body",
                "--write-out",
                "%{http_code}",
            ])
            .arg(format!("http://{endpoint}/"))
            .stdin(Stdio::null())
            .output()?;
        let status = String::from_utf8(result.stdout)?;
        self.attempts.push(json!({"client_elapsed_ms":self.started.elapsed().as_millis(),"http_status":status,"curl_exit":result.status.code(),"stderr":String::from_utf8(result.stderr)?}));
        if !readiness_complete(&status, result.status.code())? {
            return Ok(());
        }
        let index = read_evidence(Path::new("/work/board-http-ready.body"), 4096)?;
        ensure!(
            index.len() == 2044
                && hex::encode(Sha256::digest(&index))
                    == "d0d769811cbf2e5290dafb5f9f9a005bcafd45469e94899080353093e1f20947",
            "selected native page differs from archived bytes"
        );
        self.outcome = Some(board_dashboard_probe::run_selected(
            Instant::now() + Duration::from_secs(120),
            endpoint,
            Path::new("/work/selected-board-dashboard"),
        )?);
        record_progress("selected-board-native-http-passed")?;
        if !self.local {
            self.check_tls(out, run)?;
            record_progress("selected-board-native-tls-passed")?;
        } else {
            self.check_browser(out, run, endpoint)?;
        }
        Ok(())
    }
    fn check_tls(&mut self, out: &Path, run_id: &str) -> Result<()> {
        let run: md_sim_coordinator::LockedRun =
            serde_json::from_slice(&read_evidence(&out.join("locked-run.json"), 16 << 20)?)?;
        ensure!(
            serde_json::to_value(&run.process)? == serde_json::to_value(Some(&self.spec))?,
            "native TLS execution specification differs"
        );
        let binding = self.binding.as_ref().context("native TLS binding absent")?;
        let mut context = [0; 32];
        fs::File::open("/dev/urandom")?.read_exact(&mut context)?;
        let spec = GatewaySpec {
            run_id: run_id.into(),
            execution_identity: run.object()?,
            dashboard_source: "adapted-native".into(),
            listen: format!("{WORKER}:9443").parse()?,
            upstream: binding.listen,
            upstream_unix: binding.local_socket.clone(),
            upstream_tls: None,
            // The exact archived 425409-byte Boser application hard-codes
            // ws:// in graphqlws. Record this response-header adaptation;
            // keep the archived application and its authentication unchanged.
            browser_policy: BrowserPolicy::UpgradeInsecureRequests,
            certificate: "/worker/access-credentials/server.crt".into(),
            private_key: "/worker/access-credentials/server.key".into(),
            client_ca: "/worker/access-credentials/ca.crt".into(),
            allow_clients: vec![format!("{CLIENT}/32")],
            browser_context_id: hex::encode(context),
            nginx: "/usr/bin/nginx".into(),
            openssl: "/usr/bin/openssl".into(),
        };
        let mut service = RemoteGateway::start(
            &mut SshTransport::new(ssh_config())?,
            &run,
            &spec,
            RemotePaths {
                access_program: "/md-sim-access",
                store: Path::new("/worker/worker/store"),
                socket: Path::new("/worker/worker.sock"),
                output: Path::new("/worker/selected-board-gateway"),
            },
            Path::new("/work/selected-board-gateway"),
        )?;
        self.publication = Some(service.manifest().clone());
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
            ])
            .arg(spec.public_url())
            .stdin(Stdio::null())
            .output()?;
        self.no_certificate_rejected = !denied.status.success();
        ensure!(
            self.no_certificate_rejected,
            "native TLS accepted a client without a certificate"
        );
        let result = board_dashboard_probe::run_selected_tls(
            Instant::now() + Duration::from_secs(120),
            spec.listen,
            Path::new("/work/selected-board-tls"),
            board_dashboard_probe::TlsClient {
                ca: "/credentials/ca.crt".into(),
                certificate: "/credentials/client.crt".into(),
                private_key: "/credentials/client.key".into(),
                selected_run: run_id.into(),
            },
        );
        let browser_result = if result.is_ok() {
            self.check_browser(out, run_id, "127.0.0.1:18090".parse()?)
        } else {
            Ok(())
        };
        let stopped = service.stop();
        self.tls_stopped = stopped.is_ok();
        self.tls_outcome = Some(result?);
        stopped?;
        browser_result?;
        Ok(())
    }
    fn check_browser(
        &mut self,
        out: &Path,
        run_id: &str,
        http: std::net::SocketAddr,
    ) -> Result<()> {
        let Some(browser) = self.browser.as_mut() else {
            return Ok(());
        };
        let selection = super::board_browser_target::Selection {
            schema: "md-sim-selected-boser-browser/1".into(),
            selected_run: run_id.into(),
            run: serde_json::from_slice(&read_evidence(&out.join("locked-run.json"), 16 << 20)?)?,
            binding: self.binding.clone().context("browser binding absent")?,
            http,
            publication: self.publication.clone(),
        };
        self.browser_outcome = Some(browser.exercise(&selection)?);
        record_progress("selected-board-native-browser-passed")?;
        Ok(())
    }
    pub(super) fn finish(mut self, out: &Path) -> Result<Option<Value>> {
        if self.spec.dashboard.is_none() {
            return Ok(None);
        }
        if let Some(browser) = self.browser.as_mut() {
            let retained = browser.finish()?;
            if self.browser_outcome.is_none() {
                self.browser_outcome = Some(retained);
            }
        }
        let tunnel = self.tunnel.as_ref().map(SshTunnel::evidence);
        if let Some(tunnel) = self.tunnel.as_mut() {
            tunnel.stop()?;
        }
        // Retain a failed native assertion as well as a successful one.
        let native = if let Some(value) = self.outcome {
            value
        } else {
            let path = Path::new("/work/selected-board-dashboard/report.json");
            if path.exists() {
                serde_json::from_slice(&read_evidence(path, 3 << 20)?)?
            } else {
                Value::Null
            }
        };
        let tls_native = if let Some(value) = self.tls_outcome {
            value
        } else {
            let path = Path::new("/work/selected-board-tls/report.json");
            if path.exists() {
                serde_json::from_slice(&read_evidence(path, 3 << 20)?)?
            } else {
                Value::Null
            }
        };
        let mut service_logs = Vec::new();
        for name in ["stdout.json", "stderr.log", "transfer.json", "stopped.json"] {
            let path = Path::new("/work/selected-board-gateway").join(name);
            if path.exists() {
                service_logs
                    .push(json!({"name":name,"hex":hex::encode(read_evidence(&path, 65536)?)}));
            }
        }
        let tls_passed =
            self.tls_stopped && self.no_certificate_rejected && tls_native["passed"] == true;
        let browser_passed = self
            .browser_outcome
            .as_ref()
            .is_some_and(|v| v["report"]["passed"] == true);
        let report = json!({"schema":"md-sim-selected-board-http/2","selected_run":self.selected,"binding":self.binding,
            "local":self.local,"passed":self.error.is_none() && native["passed"] == true && (self.local || tls_passed)
                && (self.browser.is_none() || browser_passed),
            "error":self.error,"readiness":self.attempts,
            "readiness_policy":{"max_attempts":self.readiness_limit,"interval_seconds":READINESS_INTERVAL_SECONDS,"process_wall_seconds":self.spec.limits.wall_seconds,"http_attempt_seconds":3},
            "native":native,"tunnel":tunnel,
            "publication":self.publication,"tls_native":tls_native,"tls_stopped":self.tls_stopped,
            "no_certificate_rejected":self.no_certificate_rejected,"service_logs":service_logs,
            "full_dashboard_gate":false,"tls_tested":tls_passed,"browser_tested":browser_passed,
            "browser_required":self.browser.is_some(),"browser":self.browser_outcome});
        save(&out.join("selected-dashboard.json"), &report)?;
        Ok(Some(report))
    }
}
fn save(path: &Path, value: &Value) -> Result<()> {
    md_sim_runtime::isolation::save_json(path, value)
}

pub(super) fn verify(
    out: &Path,
    process: &ProcessSpec,
    result: &md_sim_runtime::isolation::ProcessResult,
    local: bool,
    browser_driver: Option<&ObjectId>,
) -> Result<()> {
    let Some(policy) = &process.dashboard else {
        return Ok(());
    };
    let report: Value = serde_json::from_slice(&read_evidence(
        &out.join("selected-dashboard.json"),
        4 << 20,
    )?)?;
    let binding: Binding = serde_json::from_value(report["binding"].clone())?;
    binding.verify(policy, process)?;
    let attempts = report["readiness"]
        .as_array()
        .context("readiness observations absent")?;
    ensure!(
        report["readiness_policy"]["max_attempts"] == readiness_limit(process.limits.wall_seconds)?
            && report["readiness_policy"]["process_wall_seconds"] == process.limits.wall_seconds
            && report["readiness_policy"]["interval_seconds"] == READINESS_INTERVAL_SECONDS
            && report["readiness_policy"]["http_attempt_seconds"] == 3
            && !attempts.is_empty()
            && attempts.len() <= readiness_limit(process.limits.wall_seconds)?
            && attempts
                .last()
                .is_some_and(|last| last["http_status"] == "200" && last["curl_exit"] == 0),
        "selected readiness policy or final HTTP observation differs"
    );
    let final_state = result
        .dashboard
        .as_ref()
        .context("board final dashboard receipt absent")?;
    ensure!(
        final_state["supervisor_healthy"] == true
            && final_state["binding"] == report["binding"]
            && final_state["listener_closed"] == true
            && report["passed"] == true
            && report["native"]["passed"] == true
            && report["error"].is_null()
            && report["local"] == local
            && report["native"]["fresh_cookie_store"] == true,
        "selected native dashboard checks failed"
    );
    let selected = if local {
        let identity: Value =
            serde_json::from_slice(&read_evidence(&out.join("execution-identity.json"), 65536)?)?;
        format!(
            "local-{}",
            identity["locked_run_hash"]
                .as_str()
                .context("local run identity absent")?
        )
    } else {
        let worker: Value =
            serde_json::from_slice(&read_evidence(&out.join("worker-run.json"), 65536)?)?;
        worker["run_id"]
            .as_str()
            .context("worker run absent")?
            .into()
    };
    ensure!(
        report["selected_run"] == selected,
        "native HTTP tested another run"
    );
    ensure!(
        (report["browser_required"] == true) == browser_driver.is_some(),
        "selected browser requirement differs from captured scenario"
    );
    if let Some(driver) = browser_driver {
        ensure!(
            report["browser_tested"] == true,
            "selected native browser did not pass"
        );
        let selection = super::board_browser_target::Selection {
            schema: "md-sim-selected-boser-browser/1".into(),
            selected_run: selected.clone(),
            run: serde_json::from_slice(&read_evidence(&out.join("locked-run.json"), 16 << 20)?)?,
            binding: binding.clone(),
            http: if local {
                binding.listen
            } else {
                "127.0.0.1:18090".parse()?
            },
            publication: if local {
                None
            } else {
                Some(report["publication"].clone())
            },
        };
        super::board_browser::verify(&report["browser"]["report"], &selection, driver)?;
    }
    if !local {
        let run: md_sim_coordinator::LockedRun =
            serde_json::from_slice(&read_evidence(&out.join("locked-run.json"), 16 << 20)?)?;
        ensure!(
            report["tls_tested"] == true
                && report["tls_stopped"] == true
                && report["no_certificate_rejected"] == true
                && report["tls_native"]["passed"] == true
                && report["tls_native"]["fresh_cookie_store"] == true
                && report["tls_native"]["tls"]["selected_run"] == selected
                && report["publication"]["run_id"] == selected
                && report["publication"]["binding"] == report["binding"]
                && report["publication"]["locked_run"] == serde_json::to_value(run.object()?)?
                && report["publication"]["endpoint"]["upstream_unix"]
                    == serde_json::to_value(&binding.local_socket)?
                && report["tls_native"]["origin"]
                    .as_str()
                    .map(|s| format!("{s}/"))
                    == report["publication"]["endpoint"]["url"]
                        .as_str()
                        .map(str::to_owned),
            "native TLS checks did not reach the selected worker and publication"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_observations_cover_the_selected_long_boot_budget() {
        assert!(readiness_limit(4800).unwrap() > 300);
        assert_eq!(readiness_limit(4800).unwrap(), 481);
        assert_eq!(readiness_limit(360).unwrap(), 37);
        assert!(readiness_limit(0).is_err());
    }

    #[test]
    fn partial_success_header_is_not_readiness_or_a_permanent_http_failure() -> Result<()> {
        assert!(!readiness_complete("200", Some(28))?);
        assert!(!readiness_complete("000", Some(28))?);
        assert!(!readiness_complete("000", Some(52))?);
        assert!(!readiness_complete("408", Some(0))?);
        assert!(readiness_complete("408", Some(28)).is_err());
        assert!(readiness_complete("200", Some(0))?);
        for (status, code) in [
            ("500", Some(0)),
            ("302", Some(0)),
            ("200", Some(63)),
            ("200", None),
            ("garbled", Some(0)),
        ] {
            assert!(readiness_complete(status, code).is_err());
        }
        Ok(())
    }
}

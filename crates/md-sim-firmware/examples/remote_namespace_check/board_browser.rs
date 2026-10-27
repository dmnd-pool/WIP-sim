//! Captured Firefox for the same selected firmware process and publication.
use super::board_browser_target::{CarrierSelection, Selection};
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

#[path = "board_browser_readiness.rs"]
mod readiness;

pub(super) const PREPARATION_SECONDS: u64 = 650;

pub(super) fn enabled() -> bool {
    std::env::var("MD_SIM_BOARD_BROWSER").as_deref() == Ok("1")
}

pub(super) fn carrier_enabled() -> bool {
    std::env::var("MD_SIM_CARRIER_BROWSER").as_deref() == Ok("1")
}

pub(super) fn prepare_carrier(scenario: &mut Value, payload: &Path, relay: &Path) -> Result<()> {
    ensure!(
        scenario["environment"][super::carrier_dashboard::ENVIRONMENT] == "1"
            && scenario.get("payload_manifest").is_none(),
        "carrier browser requires selected carrier access and a new payload"
    );
    let bytes = read_evidence(payload, 4 << 20)?;
    let manifest: Value = serde_json::from_slice(&bytes)?;
    ensure!(
        manifest["schema"] == "md-sim-vm-payload/1",
        "carrier browser payload schema differs"
    );
    for path in manifest["files"]
        .as_object()
        .context("carrier browser payload files absent")?
        .keys()
        .chain(
            manifest["symlinks"]
                .as_object()
                .context("carrier browser payload links absent")?
                .keys(),
        )
    {
        ensure!(
            (path.starts_with("/usr/") || path.starts_with("/etc/"))
                && !path.starts_with("/etc/md-sim-board/"),
            "carrier browser payload contains non-support input"
        );
    }
    scenario["payload_manifest"] = json!(payload.canonicalize()?);
    scenario["files"]["/carrier-browser-payload.json"] =
        json!({"path":payload.canonicalize()?,"mode":292});
    scenario["programs"]["/s9-browser-check"] = json!(relay
        .parent()
        .context("relay directory absent")?
        .join("examples/s9_browser_check")
        .canonicalize()?);
    scenario["environment"]["MD_SIM_BOARD_BROWSER"] = json!("1");
    scenario["environment"]["MD_SIM_CARRIER_BROWSER"] = json!("1");
    Ok(())
}

pub(super) struct Client {
    child: Child,
    driver: ObjectId,
    ended: bool,
    diagnostic: Option<Value>,
}
impl Client {
    pub(super) fn start() -> Result<Option<Self>> {
        if !enabled() {
            return Ok(None);
        }
        let driver = hash_file(Path::new("/s9-browser-check"))?;
        let child = Command::new("/s9-browser-check")
            .arg(if super::board::long_boot_mode() {
                "--selected-long-boot"
            } else {
                "--selected"
            })
            .env_clear()
            .env("PATH", "/usr/bin")
            .stdin(Stdio::null())
            .stdout(fs::File::create("/work/board-browser.stdout")?)
            .stderr(fs::File::create("/work/board-browser.stderr")?)
            .spawn()?;
        Ok(Some(Self {
            child,
            driver,
            ended: false,
            diagnostic: None,
        }))
    }

    pub(super) fn wait_prepared(&mut self, cancel: &AtomicBool) -> Result<Value> {
        let started = Instant::now();
        // Two existing300s per-profile budgets plus bounded credential setup.
        // This does not extend native work, boot or page-operation deadlines.
        let deadline = started + Duration::from_secs(PREPARATION_SECONDS);
        readiness::await_record(cancel, deadline, || {
            if let Some(status) = self.child.try_wait()? {
                self.ended = true;
                anyhow::bail!("browser ended before worker dispatch: {status}");
            }
            let bytes = read_evidence(Path::new("/work/board-browser.stdout"), 4 << 20)?;
            if let Some(prepared) = readiness::from_log(&bytes, &self.driver)? {
                readiness::check_owned_processes(&prepared)?;
                ensure!(
                    !cancel.load(Ordering::Acquire) && self.child.try_wait()?.is_none(),
                    "browser cancelled or ended before worker dispatch"
                );
                return Ok(Some(json!({"ready":prepared,"helper_pid":self.child.id(),
                    "elapsed_milliseconds":started.elapsed().as_millis(),
                    "ready_monotonic_ns":super::shared_pool::monotonic_ns()?,
                    "deadline_seconds":PREPARATION_SECONDS,"owned_profiles_live":true})));
            }
            Ok(None)
        })
    }

    pub(super) fn exercise(&mut self, selection: &Selection) -> Result<Value> {
        selection.targets()?;
        self.exercise_selected(&serde_json::to_value(selection)?)
    }

    pub(super) fn exercise_carrier(&mut self, selection: &CarrierSelection) -> Result<Value> {
        selection.targets()?;
        self.exercise_selected(&serde_json::to_value(selection)?)
    }

    fn exercise_selected(&mut self, selection: &Value) -> Result<Value> {
        let temporary = Path::new("/work/selected-board-browser.tmp");
        md_sim_runtime::isolation::save_json(temporary, &serde_json::to_value(selection)?)?;
        fs::set_permissions(temporary, fs::Permissions::from_mode(0o444))?;
        fs::rename(temporary, "/work/selected-board-browser.json")?;
        let start = Instant::now();
        let outcome = loop {
            if let Some(status) = self.child.try_wait()? {
                self.ended = true;
                break Ok(status);
            }
            if start.elapsed() >= Duration::from_secs(360) {
                break Err(anyhow::anyhow!("selected native browser deadline"));
            }
            thread::sleep(Duration::from_millis(100));
        };
        if outcome.is_err() {
            self.stop()?;
        }
        let mut retained = self.evidence()?;
        retained["exit_code"] = json!(outcome.as_ref().ok().and_then(|s| s.code()));
        retained["error"] = json!(outcome.as_ref().err().map(ToString::to_string));
        self.diagnostic = Some(json!({
            "diagnostic_only":true,"provisional":true,
            "exit_code":retained["exit_code"],"process_error":retained["error"],
            "browser":retained["diagnostic"]
        }));
        md_sim_runtime::isolation::save_json(
            Path::new("/work/board-browser-process.json"),
            &retained,
        )?;
        ensure!(outcome?.success(), "selected native browser process failed");
        let report = &retained["report"];
        super::board_browser_target::verify_report(report, selection, &self.driver)?;
        Ok(retained)
    }

    pub(super) fn diagnostic(&self) -> Option<&Value> {
        self.diagnostic.as_ref()
    }

    fn evidence(&self) -> Result<Value> {
        let stdout = read_evidence(Path::new("/work/board-browser.stdout"), 4 << 20)?;
        let stderr = read_evidence(Path::new("/work/board-browser.stderr"), 65536)?;
        let reports = std::str::from_utf8(&stdout)?
            .lines()
            .filter_map(|line| line.strip_prefix("MD_SIM_BOSER_BROWSER_RESULT "))
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?;
        let report = if reports.len() == 1 {
            reports[0].clone()
        } else {
            Value::Null
        };
        Ok(
            json!({"driver":self.driver,"report":report,"report_count":reports.len(),
            "stdout":hash_file(Path::new("/work/board-browser.stdout"))?,
            "stderr":hash_file(Path::new("/work/board-browser.stderr"))?,
            "diagnostic":diagnostic(std::str::from_utf8(&stdout)?, &report),
            "stdout_hex":if report.is_null() {Some(hex::encode(stdout))} else {None},
            "stderr_hex":hex::encode(stderr),"deadline_seconds":360}),
        )
    }

    pub(super) fn stop(&mut self) -> Result<()> {
        if !self.ended {
            if self.child.try_wait()?.is_some() {
                self.ended = true;
                return Ok(());
            }
            // The helper's signal handler holds dedicated cgroup.kill fds.
            // It stops both complete browser trees before it exits.
            ensure!(
                unsafe { libc::kill(self.child.id() as i32, libc::SIGTERM) } == 0,
                "stop owned browser helper"
            );
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if self.child.try_wait()?.is_some() {
                    self.ended = true;
                    break;
                }
                ensure!(
                    Instant::now() < deadline,
                    "browser termination cleanup deadline"
                );
                thread::sleep(Duration::from_millis(20));
            }
        }
        Ok(())
    }

    pub(super) fn finish(&mut self) -> Result<Value> {
        self.stop()?;
        self.evidence()
    }
}

// Small, explicitly selected fields for live progress. Keep raw browser logs,
// assets, cookies and response bodies in the retained evidence only.
fn diagnostic(stdout: &str, report: &Value) -> Value {
    let last = |prefix: &str| {
        stdout
            .lines()
            .rev()
            .find_map(|line| serde_json::from_str::<Value>(line.strip_prefix(prefix)?).ok())
            .unwrap_or(Value::Null)
    };
    let progress = last("MD_SIM_BOSER_BROWSER_PROGRESS ");
    let failure = last("MD_SIM_BOSER_BROWSER_FAILURE ");
    let error = report["error"]
        .as_str()
        .or_else(|| failure["error"].as_str());
    json!({"diagnostic_only":true,"provisional":true,
        "last_phase":progress["phase"].as_str().map(|s| s.chars().take(128).collect::<String>()),
        "elapsed_milliseconds":progress["elapsed_milliseconds"].as_u64(),
        "error":error.map(|s| s.chars().take(1024).collect::<String>()),
        "report_present":!report.is_null()})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_diagnostic_preserves_failure_and_phase_without_exporting_browser_payloads() {
        let text = format!(
            "MD_SIM_BOSER_BROWSER_PROGRESS {}\nMD_SIM_BOSER_BROWSER_FAILURE {}\n",
            json!({"phase":"native-login","elapsed_milliseconds":47,"cookies":"private"}),
            json!({"error":"request failed","body":"unbounded native page"})
        );
        let observed = diagnostic(&text, &Value::Null);
        assert_eq!(observed["last_phase"], "native-login");
        assert_eq!(observed["elapsed_milliseconds"], 47);
        assert_eq!(observed["error"], "request failed");
        assert_eq!(observed["report_present"], false);
        assert!(!observed.to_string().contains("private"));
        assert!(!observed.to_string().contains("native page"));
        let report = json!({"error":"é".repeat(2000),"cookies":["private"]});
        let observed = diagnostic(&text, &report);
        assert_eq!(observed["error"].as_str().unwrap().chars().count(), 1024);
        assert_eq!(observed["report_present"], true);
        assert_eq!(observed["provisional"], true);
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

pub(super) fn verify(report: &Value, selection: &Selection, driver: &ObjectId) -> Result<()> {
    super::board_browser_target::verify_report(report, &serde_json::to_value(selection)?, driver)
}

//! Root supervisor in the private support VM. Services use separate users.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
#[allow(dead_code)]
#[path = "s9_browser_budget.rs"]
mod budget;
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

struct Service {
    child: Child,
    pump: Option<JoinHandle<Result<()>>>,
    ready: Arc<AtomicBool>,
    passed: Arc<AtomicBool>,
}
impl Service {
    fn spawn(program: &str, args: &[&str], label: &'static str) -> Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(File::create(format!("/work/{label}.stderr"))?)
            .spawn()?;
        let output = child.stdout.take().context("service stdout missing")?;
        let ready = Arc::new(AtomicBool::new(false));
        let passed = Arc::new(AtomicBool::new(false));
        let ready_signal = ready.clone();
        let passed_signal = passed.clone();
        let pump = std::thread::spawn(move || -> Result<()> {
            let mut reader = BufReader::new(output);
            let mut total = 0usize;
            loop {
                let mut bytes = Vec::new();
                reader
                    .by_ref()
                    .take(2 * 1024 * 1024 + 1)
                    .read_until(b'\n', &mut bytes)?;
                if bytes.is_empty() {
                    break;
                }
                total += bytes.len();
                ensure!(
                    bytes.len() <= 2 * 1024 * 1024 && total <= 8 * 1024 * 1024,
                    "service output bound"
                );
                let line = std::str::from_utf8(&bytes)?.trim_end_matches(['\r', '\n']);
                if let Some(encoded) = line.strip_prefix("MD_SIM_BOSER_BROWSER_READY ") {
                    let record: Value = serde_json::from_str(encoded)?;
                    ensure!(
                        record["origin"] == "http://10.0.2.15",
                        "browser origin differs"
                    );
                    ready_signal.store(true, Ordering::Release);
                }
                if let Some(encoded) = line.strip_prefix("MD_SIM_BOSER_BROWSER_RESULT ") {
                    let record: Value = serde_json::from_str(encoded)?;
                    ensure!(
                        record["origin"] == "http://10.0.2.15",
                        "browser result origin differs"
                    );
                    passed_signal.store(record["passed"] == true, Ordering::Release);
                }
                if let Some(encoded) = line.strip_prefix("MD_SIM_DASHBOARD_SESSION_CLOSED ") {
                    let record: Value = serde_json::from_str(encoded)?;
                    ensure!(
                        record["origin"] == "http://10.0.2.15",
                        "HTTP probe origin differs"
                    );
                    if record["passed"] == true {
                        // End the HTTP probe's own session before the browser
                        // opens its separate native session.
                        fs::write("/work/boser-http-ready", b"passed\n")?;
                    }
                }
                // A single process serializes complete JSON records from both
                // services. Separate child writes must not splice their lines.
                println!("{line}");
            }
            Ok(())
        });
        Ok(Self {
            child,
            pump: Some(pump),
            ready,
            passed,
        })
    }
    fn finish(&mut self, label: &str) -> Result<()> {
        let status = self.child.wait()?;
        self.pump
            .take()
            .context("service pump missing")?
            .join()
            .map_err(|_| anyhow::anyhow!("service output thread panicked"))??;
        let path = format!("/work/{label}.stderr");
        ensure!(
            fs::metadata(&path)?.len() <= 65536,
            "service error log bound"
        );
        println!(
            "MD_SIM_POOL_SERVICE_EXIT {}",
            json!({"service":label,"status":status.to_string(),"stderr":fs::read_to_string(path)?})
        );
        ensure!(status.success(), "{label} service failed");
        Ok(())
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        // This supervisor retains root only inside the disconnected VM. It
        // can stop its UID1000 pool and UID1001 browser on every error path.
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
        if let Some(pump) = self.pump.take() {
            let _ = pump.join();
        }
    }
}

pub fn run() -> Result<()> {
    ensure!(
        unsafe { libc::geteuid() } == 0,
        "support supervisor requires VM root"
    );
    let mut browser = Service::spawn("/boser-browser", &[], "boser-browser")?;
    let deadline = Instant::now() + Duration::from_secs(320);
    while !browser.ready.load(Ordering::Acquire) {
        if browser.child.try_wait()?.is_some() {
            browser.finish("boser-browser")?;
            anyhow::bail!("browser exited before readiness");
        }
        ensure!(
            Instant::now() < deadline,
            "support browser startup deadline"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut pool = Service::spawn("/pool-guest", &["--server"], "pool-server")?;
    let deadline = Instant::now() + Duration::from_secs(budget::COMPLETION_SECONDS);
    loop {
        let browser_status = browser.child.try_wait()?;
        let pool_status = pool.child.try_wait()?;
        if browser_status.is_some_and(|status| !status.success()) {
            browser.finish("boser-browser")?;
        }
        if pool_status.is_some_and(|status| !status.success()) {
            pool.finish("pool-server")?;
        }
        if browser_status.is_some() && pool_status.is_some() {
            break;
        }
        ensure!(
            Instant::now() < deadline,
            "support services completion deadline"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    pool.finish("pool-server")?;
    browser.finish("boser-browser")?;
    ensure!(
        browser.passed.load(Ordering::Acquire),
        "browser completion record absent"
    );
    Ok(())
}

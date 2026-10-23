//! Selected-run client setup. This executes only inside the disconnected VM.
use crate::{browser_driver, browser_environment, browser_target::Selected};
use anyhow::{ensure, Result};
use md_sim_access::credentials::{self, BrowserProfileTool, BrowserProfileTools};
use md_sim_runtime::store::hash_file;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
    time::{Duration, Instant},
};

fn read(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    ensure!(file.metadata()?.is_file(), "browser input is not regular");
    let mut bytes = Vec::new();
    file.take(maximum + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= maximum, "browser input exceeds bound");
    Ok(bytes)
}

pub fn copy_credentials() -> Result<()> {
    ensure!(
        !Path::new("/credentials/server.key").exists()
            && !Path::new("/worker/access-credentials/server.key").exists(),
        "browser can access the server private key"
    );
    ensure!(
        !fs::read_to_string("/proc/net/route")?
            .lines()
            .skip(1)
            .any(|line| line.split_whitespace().nth(1) == Some("00000000")),
        "selected browser has a default network route"
    );
    fs::DirBuilder::new()
        .mode(0o700)
        .create("/work/browser/credentials")?;
    browser_environment::chown(Path::new("/work/browser/credentials"))?;
    for name in ["ca.crt", "client.crt", "client.key"] {
        let bytes = read(&Path::new("/credentials").join(name), 65536)?;
        let path = Path::new("/work/browser/credentials").join(name);
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&path)?
            .write_all(&bytes)?;
        browser_environment::chown(&path)?;
    }
    Ok(())
}

pub fn prepare_profiles(selected: bool) -> Result<Value> {
    let mut reports = Vec::new();
    for index in 0..2 {
        let profile = browser_driver::prepare_profile(
            index,
            if selected {
                "user_pref(\"security.default_personal_cert\", \"Select Automatically\");\n"
            } else {
                ""
            },
        )?;
        if selected {
            let tool = |program: &str| -> Result<BrowserProfileTool> {
                Ok(BrowserProfileTool {
                    program: program.into(),
                    object: hash_file(Path::new(program))?,
                })
            };
            let tools = BrowserProfileTools {
                openssl: tool("/usr/bin/openssl")?,
                certutil: tool("/usr/bin/certutil")?,
                pk12util: tool("/usr/bin/pk12util")?,
            };
            // The enclosing isolated test execution exists before the worker
            // starts. Its identity names this fresh client credential context.
            // The later selection separately pins the actual firmware run.
            let context = hash_file(Path::new("/execution-spec.json"))?;
            reports.push(credentials::prepare_browser_profile(
                &tools,
                Path::new(&profile),
                Path::new("/work/browser/credentials/ca.crt"),
                Path::new("/work/browser/credentials/client.crt"),
                Path::new("/work/browser/credentials/client.key"),
                &context.sha256,
            )?);
        }
    }
    Ok(json!(reports))
}

pub fn wait_selection(long_boot: bool) -> Result<Selected> {
    let start = Instant::now();
    let path = Path::new("/work/selected-board-browser.json");
    loop {
        if path.exists() {
            // The parent uses atomic rename after native HTTP/TLS preflight.
            let selection: Selected = serde_json::from_slice(&read(path, 16 << 20)?)?;
            selection.targets()?;
            return Ok(selection);
        }
        ensure!(
            start.elapsed() < Duration::from_secs(if long_boot { 4900 } else { 2500 }),
            "selected browser endpoint deadline"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

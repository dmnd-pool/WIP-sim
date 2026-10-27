//! Validate completed browser preparation before the selected miner starts.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::store::{hash_file, ObjectId};
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

const MARKER: &str = "MD_SIM_BOSER_BROWSER_READY ";

pub(super) fn await_record(
    cancel: &AtomicBool,
    deadline: Instant,
    mut poll: impl FnMut() -> Result<Option<Value>>,
) -> Result<Value> {
    loop {
        ensure!(
            !cancel.load(Ordering::Acquire),
            "browser preparation cancelled before worker dispatch"
        );
        ensure!(
            Instant::now() < deadline,
            "browser preparation deadline before worker dispatch"
        );
        if let Some(ready) = poll()? {
            ensure!(
                !cancel.load(Ordering::Acquire) && Instant::now() < deadline,
                "browser preparation cancelled or expired before worker dispatch"
            );
            return Ok(ready);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub(super) fn from_log(bytes: &[u8], driver: &ObjectId) -> Result<Option<Value>> {
    let mut ready = None;
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        // An incomplete write is not a readiness record.
        if !line.ends_with(b"\n") {
            continue;
        }
        let Some(text) = std::str::from_utf8(line)?.strip_prefix(MARKER) else {
            continue;
        };
        ensure!(ready.is_none(), "duplicate browser readiness record");
        let value: Value = serde_json::from_str(text)?;
        verify(&value, driver)?;
        ready = Some(value);
    }
    Ok(ready)
}

fn startups(value: &Value) -> [&Value; 2] {
    [&value["startup"], &value["isolation_startup"]]
}

fn verify(value: &Value, driver: &ObjectId) -> Result<()> {
    driver.validate()?;
    ensure!(
        value["schema"] == "md-sim-browser-prepared/1"
            && value["selected"] == true
            && value.get("origin") == Some(&Value::Null)
            && value["helper"] == serde_json::to_value(driver)?
            && value["firmware_gate"] == false
            && value["native_dashboard_gate"] == false,
        "browser readiness identity or scope differs"
    );
    let profiles = value["profiles"]
        .as_array()
        .filter(|profiles| profiles.len() == 2)
        .context("browser readiness needs two fresh profiles")?;
    let mut sessions = Vec::new();
    let mut pids = Vec::new();
    for (index, startup) in startups(value).into_iter().enumerate() {
        let profile = format!("/work/browser/profile-{index}");
        let response = &startup["capabilities"];
        let caps = &response["capabilities"];
        let session = response["sessionId"]
            .as_str()
            .filter(|id| !id.is_empty() && id.len() <= 128)
            .context("browser NewSession identity absent")?;
        let pid = caps["moz:processID"]
            .as_u64()
            .filter(|pid| (1..=i32::MAX as u64).contains(pid))
            .context("browser process identity absent")?;
        let prefs: ObjectId = serde_json::from_value(startup["prefs"].clone())?;
        prefs.validate()?;
        ensure!(
            startup["profile"] == profile
                && caps["moz:profile"] == profile
                && caps["browserName"] == "firefox"
                && caps["acceptInsecureCerts"] == false
                && caps["moz:headless"] == true
                && caps["pageLoadStrategy"] == "eager"
                && startup["startup_millis"]
                    .as_u64()
                    .is_some_and(|ms| ms <= 300_000)
                && !sessions.contains(&session)
                && !pids.contains(&pid),
            "browser session is not a distinct verified fresh profile"
        );
        let state = &profiles[index];
        ensure!(
            state["index"] == index
                && state["cookies"] == json!([])
                && state["page"]["url"] == "about:blank"
                && state["page"]["title"] == ""
                && state["page"]["ready"] == "complete",
            "browser profile already visited an origin or holds state"
        );
        sessions.push(session);
        pids.push(pid);
    }
    Ok(())
}

pub(super) fn check_owned_processes(value: &Value) -> Result<()> {
    for (index, startup) in startups(value).into_iter().enumerate() {
        let pid = startup["capabilities"]["capabilities"]["moz:processID"]
            .as_u64()
            .context("prepared browser process absent")?;
        // The helper mounts cgroupfs in its private mount namespace. The
        // parent checks membership through the shared PID namespace instead.
        let membership = fs::read_to_string(format!("/proc/{pid}/cgroup"))?;
        ensure!(
            membership
                .lines()
                .any(|line| line == format!("0::/md-sim-browser/profile-{index}")),
            "prepared Firefox is outside its owned profile group"
        );
        let status = fs::read_to_string(format!("/proc/{pid}/status"))?;
        ensure!(
            status.lines().any(|line| line
                .strip_prefix("Uid:")
                .is_some_and(|uids| uids.split_whitespace().collect::<Vec<_>>() == ["1001"; 4])),
            "prepared Firefox has another owner"
        );
        let prefs: ObjectId = serde_json::from_value(startup["prefs"].clone())?;
        ensure!(
            hash_file(Path::new(&format!("/work/browser/profile-{index}/user.js")))? == prefs,
            "prepared browser preferences changed"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (ObjectId, Value) {
        let driver = ObjectId {
            sha256: "12".repeat(32),
            size: 41,
        };
        let startup = |index| {
            json!({"profile":format!("/work/browser/profile-{index}"),
            "prefs":driver,"startup_millis":49000,
            "capabilities":{"sessionId":format!("fresh-{index}"),"capabilities":{
                "moz:profile":format!("/work/browser/profile-{index}"),
                "moz:processID":27+index,"browserName":"firefox",
                "moz:headless":true,"acceptInsecureCerts":false,"pageLoadStrategy":"eager"}}})
        };
        let profiles = [0, 1].map(|index| {
            json!({"index":index,"cookies":[],
            "page":{"url":"about:blank","title":"","ready":"complete"}})
        });
        let value = json!({"schema":"md-sim-browser-prepared/1","selected":true,
            "origin":null,"helper":driver,"startup":startup(0),"isolation_startup":startup(1),
            "firmware_gate":false,"native_dashboard_gate":false,
            "profiles":profiles});
        (driver, value)
    }

    #[test]
    fn partial_or_duplicate_records_cannot_release_dispatch() {
        let (driver, value) = fixture();
        let line = format!("{MARKER}{value}\n");
        assert!(from_log(b"ordinary startup log\n", &driver)
            .unwrap()
            .is_none());
        assert!(from_log(line.trim_end().as_bytes(), &driver)
            .unwrap()
            .is_none());
        assert_eq!(from_log(line.as_bytes(), &driver).unwrap(), Some(value));
        assert!(from_log(format!("{line}{line}").as_bytes(), &driver).is_err());
    }

    #[test]
    fn readiness_rejects_wrong_identity_reused_sessions_and_existing_state() {
        let (driver, value) = fixture();
        for (pointer, replacement) in [
            ("/schema", json!("other")),
            ("/selected", json!(false)),
            ("/helper/sha256", json!("34".repeat(32))),
            ("/firmware_gate", json!(true)),
            ("/native_dashboard_gate", json!(true)),
            ("/origin", json!("http://selected-miner")),
            (
                "/isolation_startup/capabilities/sessionId",
                json!("fresh-0"),
            ),
            (
                "/isolation_startup/capabilities/capabilities/moz:processID",
                json!(27),
            ),
            (
                "/startup/capabilities/capabilities/acceptInsecureCerts",
                json!(true),
            ),
            ("/startup/startup_millis", json!(300001)),
            ("/profiles/1/page/url", json!("http://selected-miner")),
            (
                "/profiles/1/cookies",
                json!([{"name":"session","value":"old"}]),
            ),
        ] {
            let mut bad = value.clone();
            *bad.pointer_mut(pointer).unwrap() = replacement;
            let bytes = format!("{MARKER}{bad}\n");
            assert!(from_log(bytes.as_bytes(), &driver).is_err(), "{pointer}");
        }
        let mut missing = value;
        missing["profiles"] = json!([]);
        assert!(verify(&missing, &driver).is_err());
        let (_, mut missing) = fixture();
        missing.as_object_mut().unwrap().remove("origin");
        assert!(verify(&missing, &driver).is_err());
    }

    #[test]
    fn cancelled_expired_or_failed_preparation_never_releases_dispatch() {
        let cancel = AtomicBool::new(true);
        assert!(
            await_record(&cancel, Instant::now() + Duration::from_secs(1), || {
                panic!("cancelled preparation must not poll readiness")
            })
            .is_err()
        );
        cancel.store(false, Ordering::Release);
        assert!(await_record(&cancel, Instant::now(), || {
            panic!("expired preparation must not poll a browser")
        })
        .is_err());
        assert!(
            await_record(&cancel, Instant::now() + Duration::from_secs(1), || {
                cancel.store(true, Ordering::Release);
                Ok(Some(json!({"component_fixture":true})))
            })
            .is_err()
        );
        cancel.store(false, Ordering::Release);
        let failure = await_record(&cancel, Instant::now() + Duration::from_secs(1), || {
            anyhow::bail!("owned browser exited")
        })
        .unwrap_err();
        assert_eq!(failure.to_string(), "owned browser exited");
    }
}

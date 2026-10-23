//! Independent host-side binding and checks for the real-browser component.
//! The fixture has no firmware, mining or native-dashboard acceptance claim.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{save_json, Mount, ProcessResult, ProcessSpec},
    store::{hash_file, ObjectId},
};
use serde_json::{json, Value};
use std::{fs, io::Read, path::Path};

fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "evidence exceeds size bound");
    Ok(bytes)
}
fn object(value: &Value) -> Result<ObjectId> {
    let object: ObjectId = serde_json::from_value(value.clone())?;
    object.validate()?;
    Ok(object)
}
fn input<'a>(inputs: &'a [Mount], path: &str) -> Result<&'a ObjectId> {
    let matches: Vec<_> = inputs.iter().filter(|i| i.guest_path == path).collect();
    ensure!(matches.len() == 1, "missing or duplicate input {path}");
    Ok(&matches[0].object)
}
fn check_behavior(evidence: &Value) -> Result<()> {
    ensure!(
        evidence["schema"] == "md-sim-browser-component/1"
            && evidence["component_pass"] == true
            && evidence["firmware_gate"] == false
            && evidence["native_dashboard_gate"] == false
            && evidence["error"].is_null()
            && evidence["server_error"].is_null(),
        "browser component did not pass its stated scope"
    );
    let events = evidence["events"]
        .as_array()
        .context("browser events absent")?;
    ensure!(events.len() == 4, "browser case count differs");
    let cleanup = evidence["cleanup"]
        .as_array()
        .context("browser cleanup absent")?;
    ensure!(cleanup.len() == 2, "browser cleanup count differs");
    for (index, record) in cleanup.iter().enumerate() {
        ensure!(
            record["empty"] == true
                && record["error"].is_null()
                && record["group"] == format!("/sys/fs/cgroup/md-sim-browser/profile-{index}"),
            "browser descendants were not cleaned"
        );
    }
    let mut origin = None;
    for (index, pair) in events.chunks_exact(2).enumerate() {
        let start = &pair[0];
        let check = &pair[1];
        ensure!(
            start["metadata"]["capabilities"]["capabilities"]["timeouts"]
                == json!({"pageLoad":30000,"script":5000,"implicit":0}),
            "browser did not apply requested command capabilities"
        );
        ensure!(
            start["event"] == "browser-started"
                && start["index"] == index
                && start["handshake"]["applicationType"] == "gecko"
                && start["handshake"]["marionetteProtocol"] == 3
                && start["metadata"]["profile"] == format!("/work/browser/profile-{index}")
                && start["metadata"]["capabilities"]["capabilities"]["acceptInsecureCerts"]
                    == false,
            "browser session or profile identity differs"
        );
        ensure!(
            check["event"] == "browser-fixture-check"
                && check["index"] == index
                && check["title"] == "Simulator browser fixture"
                && check["real_DOM_and_script"] == true
                && check["separate_profile"] == true
                && check["service_counter"] == 1
                && check["cleanup"] == cleanup[index],
            "real DOM/service behavior absent"
        );
        let url = check["url"].as_str().context("browser URL absent")?;
        let address: std::net::SocketAddr = url
            .strip_prefix("http://")
            .context("unexpected browser URL scheme")?
            .parse()?;
        ensure!(
            address.ip() == std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
                && address.port() > 0,
            "component service is not loopback"
        );
        if let Some(old) = origin {
            ensure!(old == url, "profiles did not use the same origin");
        } else {
            origin = Some(url);
        }
        let cookies = check["cookies"]
            .as_array()
            .context("browser cookie list absent")?;
        if index == 0 {
            ensure!(
                cookies.len() == 1
                    && cookies[0]["name"] == "fixture_session"
                    && cookies[0]["value"] == "first-profile"
                    && cookies[0]["httpOnly"] == true,
                "first profile did not retain its real service cookie"
            );
        } else {
            ensure!(
                cookies.is_empty(),
                "new profile inherited another run's cookie"
            );
        }
    }
    let requests = evidence["requests"]
        .as_array()
        .context("service requests absent")?;
    ensure!(
        !requests.is_empty() && requests.len() < 128,
        "service trace count outside bound"
    );
    let mut post = None;
    let mut first_page = false;
    let mut first_read = false;
    let mut final_read = false;
    let mut second_page = false;
    let authority = origin
        .context("browser origin absent")?
        .strip_prefix("http://")
        .context("browser scheme differs")?;
    for (index, request) in requests.iter().enumerate() {
        let text = request["request"]
            .as_str()
            .context("request bytes absent")?;
        ensure!(
            text.len() <= 16384 && text.ends_with("\r\n\r\n"),
            "request evidence malformed"
        );
        let headers = text
            .lines()
            .skip(1)
            .filter_map(|line| line.split_once(':'))
            .collect::<Vec<_>>();
        let hosts = headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case("host"))
            .map(|(_, value)| value.trim())
            .collect::<Vec<_>>();
        ensure!(
            hosts == [authority],
            "actual browser request selects another service origin"
        );
        let cookies = headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case("cookie"))
            .map(|(_, value)| value.trim())
            .collect::<Vec<_>>();
        match text.lines().next().context("request line absent")? {
            "GET /set-cookie HTTP/1.1" => {
                ensure!(
                    !first_page
                        && !first_read
                        && post.is_none()
                        && request["counter"] == 0
                        && cookies.is_empty(),
                    "initial cookie-setting page absent, repeated or out of order"
                );
                first_page = true;
            }
            "POST /increment HTTP/1.1" => {
                ensure!(
                    post.replace(index).is_none() && request["counter"] == 1 && first_read,
                    "counter increment absent, repeated or out of order"
                );
            }
            "GET /count HTTP/1.1" if post.is_none() => {
                ensure!(
                    first_page
                        && request["counter"] == 0
                        && cookies == ["fixture_session=first-profile"],
                    "initial service counter or actual retained cookie differs"
                );
                first_read = true;
            }
            "GET /count HTTP/1.1" => {
                ensure!(request["counter"] == 1, "updated service counter differs");
                final_read = true;
            }
            "GET / HTTP/1.1" => {
                ensure!(
                    post.is_some()
                        && request["counter"] == 1
                        && !text
                            .lines()
                            .any(|line| line.to_ascii_lowercase().starts_with("cookie:")),
                    "second profile's actual request contains a prior cookie"
                );
                second_page = true;
            }
            _ => (),
        }
    }
    ensure!(
        first_page && post.is_some() && final_read && second_page,
        "actual browser request sequence incomplete"
    );
    Ok(())
}
fn report(directory: &Path, prepared: &Path, capture: &Path, output: &Path) -> Result<()> {
    ensure!(!output.try_exists()?, "report output must be new");
    let log = read(&directory.join("guest.log"), 4 << 20)?;
    let text = std::str::from_utf8(&log)?;
    let records: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("MD_SIM_BROWSER_REPORT "))
        .collect();
    ensure!(records.len() == 1, "browser report absent or duplicated");
    let evidence: Value = serde_json::from_str(records[0])?;
    let result: ProcessResult =
        serde_json::from_slice(&read(&directory.join("result.json"), 9 << 20)?)?;
    ensure!(
        result.status == "exited"
            && result.exit_code == Some(0)
            && !result.trace_truncated
            && result.stderr.is_empty()
            && hex::decode(&result.stdout)? == log,
        "VM did not exit with complete browser evidence"
    );
    let assessment: Value =
        serde_json::from_slice(&read(&directory.join("assessment.json"), 65536)?)?;
    ensure!(
        assessment["guest_boot_and_tcp_probe"] == true
            && object(&assessment["guest_log"])? == hash_file(&directory.join("guest.log"))?
            && object(&assessment["process_result"])? == hash_file(&directory.join("result.json"))?
            && object(&assessment["process_spec"])?
                == hash_file(&directory.join("process-spec.json"))?,
        "VM assessment does not bind this execution"
    );
    let spec: ProcessSpec =
        serde_json::from_slice(&read(&directory.join("process-spec.json"), 1 << 20)?)?;
    spec.validate()?;
    ensure!(
        spec.argv
            .windows(2)
            .filter(|pair| pair[0] == "-nic" && pair[1] == "none")
            .count()
            == 1
            && !spec
                .argv
                .iter()
                .any(|arg| matches!(arg.as_str(), "-netdev" | "-net")),
        "component VM has an undeclared network backend"
    );
    let root: Vec<Mount> =
        serde_json::from_slice(&read(&directory.join("root-manifest.json"), 4 << 20)?)?;
    for mount in root.iter().chain(&spec.inputs) {
        mount.object.validate()?;
        ensure!(
            hash_file(
                &directory
                    .join("input-store/objects")
                    .join(&mount.object.sha256)
            )? == mount.object,
            "captured input bytes differ: {}",
            mount.guest_path
        );
    }
    ensure!(
        input(&root, "/browser-probe")? == &hash_file(&prepared.join("driver"))?
            && object(&evidence["driver"])? == *input(&root, "/browser-probe")?
            && evidence["source_sha256"]
                == md_sim_firmware::prepare::hash_bytes(include_bytes!("browser_probe.rs")),
        "browser driver/source identity differs"
    );
    for (field, path, bytes) in [
        (
            "source_sha256",
            "browser_probe.rs",
            include_bytes!("browser_probe.rs").as_slice(),
        ),
        (
            "driver_source_sha256",
            "support/browser_driver.rs",
            include_bytes!("support/browser_driver.rs").as_slice(),
        ),
        (
            "environment_source_sha256",
            "support/browser_environment.rs",
            include_bytes!("support/browser_environment.rs").as_slice(),
        ),
        (
            "groups_source_sha256",
            "support/browser_groups.rs",
            include_bytes!("support/browser_groups.rs").as_slice(),
        ),
    ] {
        let expected = md_sim_firmware::prepare::hash_bytes(bytes);
        ensure!(
            evidence[field] == expected
                && hash_file(&prepared.join("source").join(path))?.sha256 == expected,
            "browser shared source differs: {path}"
        );
    }
    ensure!(
        input(&spec.inputs, "/runner")? == &hash_file(&prepared.join("vm-driver"))?,
        "VM driver identity differs"
    );
    for (field, path) in [
        ("firefox", "/usr/lib/firefox/firefox"),
        ("libxul", "/usr/lib/firefox/libxul.so"),
    ] {
        ensure!(
            object(&evidence[field])? == *input(&root, path)?,
            "executed browser bytes differ"
        );
    }
    let payload: Value =
        serde_json::from_slice(&read(&directory.join("payload-report.json"), 1 << 20)?)?;
    ensure!(
        object(&payload["manifest"])? == hash_file(&capture.join("payload.json"))?
            && object(&payload["manifest"])?
                == hash_file(&directory.join("payload-manifest.json"))?
            && object(&payload["combined_initramfs"])?
                == *input(&spec.inputs, "/initramfs.cpio.gz")?,
        "payload capture or executed image differs"
    );
    let manifest: Value = serde_json::from_slice(&read(&capture.join("payload.json"), 4 << 20)?)?;
    for (name, entry) in manifest["files"]
        .as_object()
        .context("payload files absent")?
    {
        ensure!(
            object(&entry["object"])? == *input(&root, name)?,
            "browser payload file identity differs"
        );
    }
    check_behavior(&evidence)?;
    save_json(
        output,
        &json!({"schema":"md-sim-browser-component-report/1","component_pass":true,"firmware_gate":false,"native_dashboard_gate":false,"external_host":false,"scope":"Real Firefox DOM, JavaScript fetch, live service update and same-origin cookie isolation using two fresh profiles against an explicit component fixture","evidence":evidence,"guest_log":hash_file(&directory.join("guest.log"))?,"process_spec":hash_file(&directory.join("process-spec.json"))?,"process_result":hash_file(&directory.join("result.json"))?,"payload_manifest":hash_file(&capture.join("payload.json"))?,"verifier":hash_file(&std::env::current_exe()?)?,"verifier_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("browser_report.rs"))}),
    )?;
    println!(
        "{}",
        json!({"component_pass":true,"native_dashboard_gate":false,"report":output})
    );
    Ok(())
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 5,
        "usage: browser_report VM_DIRECTORY PREPARED_DIRECTORY CAPTURE_DIRECTORY NEW_REPORT"
    );
    report(
        Path::new(&args[1]),
        Path::new(&args[2]),
        Path::new(&args[3]),
        Path::new(&args[4]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Protocol transcript only. This unit test cannot establish a browser or
    /// firmware execution gate; the host report also requires captured VM data.
    fn transcript() -> Value {
        let mut events = Vec::new();
        let mut cleanup = Vec::new();
        for index in 0..2 {
            cleanup.push(json!({"group":format!("/sys/fs/cgroup/md-sim-browser/profile-{index}"),"empty":true}));
            events.push(json!({"event":"browser-started","index":index,"handshake":{"applicationType":"gecko","marionetteProtocol":3},"metadata":{"profile":format!("/work/browser/profile-{index}"),"capabilities":{"capabilities":{"acceptInsecureCerts":false,"timeouts":{"pageLoad":30000,"script":5000,"implicit":0}}}}}));
            events.push(json!({"event":"browser-fixture-check","index":index,"title":"Simulator browser fixture","real_DOM_and_script":true,"separate_profile":true,"service_counter":1,"url":"http://127.0.0.1:12345","cookies":if index==0 {json!([{"name":"fixture_session","value":"first-profile","httpOnly":true}])}else{json!([])}}));
            events.last_mut().unwrap()["cleanup"] = cleanup[index].clone();
        }
        let request = |line: &str, cookie: bool, counter: u64| json!({"request":format!("{line} HTTP/1.1\r\nHost: 127.0.0.1:12345\r\n{}\r\n",if cookie{"Cookie: fixture_session=first-profile\r\n"}else{""}),"counter":counter});
        json!({"schema":"md-sim-browser-component/1","component_pass":true,"firmware_gate":false,"native_dashboard_gate":false,"error":null,"server_error":null,"events":events,"cleanup":cleanup,"requests":[request("GET /set-cookie",false,0),request("GET /count",true,0),request("POST /increment",true,1),request("GET /count",true,1),request("GET /",false,1),request("GET /count",false,1)]})
    }
    #[test]
    fn rejects_omitted_origin_changed_replayed_or_cookie_leaking_requests() -> Result<()> {
        let original = transcript();
        check_behavior(&original)?;
        let mut omitted = original.clone();
        omitted["requests"].as_array_mut().unwrap().remove(0);
        assert!(check_behavior(&omitted).is_err());
        let mut changed = original.clone();
        changed["requests"][0]["request"] =
            json!("GET /set-cookie HTTP/1.1\r\nHost: 127.0.0.1:12346\r\n\r\n");
        assert!(check_behavior(&changed).is_err());
        let mut replay = original.clone();
        let post = replay["requests"][2].clone();
        replay["requests"].as_array_mut().unwrap().insert(3, post);
        assert!(check_behavior(&replay).is_err());
        let mut leaked = original.clone();
        leaked["requests"][4]["request"] = json!("GET / HTTP/1.1\r\nHost: 127.0.0.1:12345\r\nCookie: fixture_session=first-profile\r\n\r\n");
        assert!(check_behavior(&leaked).is_err());
        let mut remaining = original.clone();
        remaining["cleanup"][0]["empty"] = json!(false);
        assert!(check_behavior(&remaining).is_err());
        let mut omitted_cleanup = original.clone();
        omitted_cleanup.as_object_mut().unwrap().remove("cleanup");
        assert!(check_behavior(&omitted_cleanup).is_err());
        let mut ignored_caps = original;
        ignored_caps["events"][0]["metadata"]["capabilities"]["capabilities"]["timeouts"]
            ["pageLoad"] = json!(300000);
        assert!(check_behavior(&ignored_caps).is_err());
        Ok(())
    }
}

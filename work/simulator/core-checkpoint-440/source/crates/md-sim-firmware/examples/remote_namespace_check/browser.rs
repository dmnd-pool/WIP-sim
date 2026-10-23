//! Native browser subprocess in the existing isolated client environment.
use super::*;

const MARKER: &str = "MD_SIM_NATIVE_BROWSER_REPORT ";

fn parse_report(stdout: &[u8], endpoint: &Value, driver: &ObjectId) -> Result<Value> {
    ensure!(
        stdout.len() <= 1024 * 1024,
        "native browser report exceeds bound"
    );
    let text = std::str::from_utf8(stdout)?;
    let records = text
        .lines()
        .filter_map(|line| line.strip_prefix(MARKER))
        .collect::<Vec<_>>();
    ensure!(
        records.len() == 1,
        "missing or duplicate native browser report"
    );
    let report: Value = serde_json::from_str(records[0])?;
    ensure!(
        report["schema"] == "md-sim-native-browser/1"
            && report["passed"] == true
            && report["error"].is_null()
            && report["selected_run"] == endpoint["run_id"]
            && report["execution_identity"] == endpoint["execution_identity"]
            && report["driver"] == serde_json::to_value(driver)?,
        "native browser did not pass for this selected endpoint and driver"
    );
    Ok(report)
}

pub(super) fn client_checks(events: &mut Vec<Value>) -> Result<()> {
    let endpoint: Value =
        serde_json::from_slice(&read_evidence(Path::new("/native-endpoint.json"), 65536)?)?;
    let driver = hash_file(Path::new("/native-browser-check"))?;
    let mut child = Owned(
        Command::new("/native-browser-check")
            .env_clear()
            .env("PATH", "/usr/bin")
            .stdin(Stdio::null())
            .stdout(fs::File::create("/work/native-browser.stdout")?)
            .stderr(fs::File::create("/work/native-browser.stderr")?)
            .spawn()?,
    );
    let deadline = Instant::now() + Duration::from_secs(native_browser_budget::HELPER_WALL_SECONDS);
    let outcome = loop {
        if let Some(status) = child.0.try_wait()? {
            break Ok(status);
        }
        if Instant::now() >= deadline {
            break Err(anyhow::anyhow!("native browser helper deadline"));
        }
        thread::sleep(Duration::from_millis(100));
    };
    if outcome.is_err() {
        let _ = child.0.kill();
        let _ = child.0.wait();
    }
    let stdout = read_evidence(Path::new("/work/native-browser.stdout"), 1024 * 1024)?;
    let stderr = read_evidence(Path::new("/work/native-browser.stderr"), 65536)?;
    events.push(json!({"native_browser_process":{"driver":driver,
        "stdout":hash_file(Path::new("/work/native-browser.stdout"))?,
        "stderr":hash_file(Path::new("/work/native-browser.stderr"))?,
        "stderr_hex":hex::encode(stderr),"deadline_seconds":native_browser_budget::HELPER_WALL_SECONDS,
        "exit_code":outcome.as_ref().ok().and_then(|status|status.code()),
        "error":outcome.as_ref().err().map(ToString::to_string)}}));
    if let Ok(report) = parse_report(&stdout, &endpoint, &driver) {
        events.push(json!({"native_browser_report":report}));
    } else {
        events.push(json!({"native_browser_failed_stdout_hex":hex::encode(&stdout)}));
    }
    ensure!(outcome?.success(), "native browser helper failed");
    parse_report(&stdout, &endpoint, &driver)?;
    Ok(())
}

pub(super) fn verify_captured_inputs(
    directory: &Path,
    specification: &Value,
    inputs: &[md_sim_runtime::isolation::Mount],
    process: &md_sim_runtime::isolation::ProcessSpec,
) -> Result<()> {
    let manifest_path = directory.join("payload-manifest.json");
    let manifest: Value = serde_json::from_slice(&read_evidence(&manifest_path, 4 << 20)?)?;
    let report: Value = serde_json::from_slice(&read_evidence(
        &directory.join("payload-report.json"),
        1 << 20,
    )?)?;
    ensure!(
        manifest["schema"] == "md-sim-vm-payload/1"
            && specification["symlinks"] == manifest["symlinks"]
            && report["manifest"] == serde_json::to_value(hash_file(&manifest_path)?)?,
        "captured browser links or payload identity differs"
    );
    for (path, file) in manifest["files"]
        .as_object()
        .context("browser files absent")?
    {
        ensure!(
            specification["inputs"][path] == file["object"]
                && inputs
                    .iter()
                    .filter(|input| input.guest_path == *path
                        && serde_json::to_value(&input.object).ok().as_ref()
                            == Some(&file["object"]))
                    .count()
                    == 1,
            "browser file was not captured exactly: {path}"
        );
    }
    ensure!(
        process
            .inputs
            .iter()
            .filter(|input| input.guest_path == "/initramfs.cpio.gz"
                && serde_json::to_value(&input.object).ok().as_ref()
                    == Some(&report["combined_initramfs"]))
            .count()
            == 1,
        "browser payload differs from the executed image"
    );
    Ok(())
}

fn single<'a>(events: &'a [Value], key: &str) -> Result<&'a Value> {
    let matches = events
        .iter()
        .filter(|event| !event[key].is_null())
        .collect::<Vec<_>>();
    ensure!(
        matches.len() == 1,
        "missing or duplicate native browser {key}"
    );
    Ok(&matches[0][key])
}

fn object(value: &Value) -> Result<ObjectId> {
    let object: ObjectId = serde_json::from_value(value.clone())?;
    object.validate()?;
    ensure!(object.size > 0, "empty browser evidence identity");
    Ok(object)
}

fn check_dashboard(browser: &Value, origin: &str) -> Result<()> {
    let prefix = format!("{origin}/cgi-bin/luci/");
    let initial = &browser["initial"];
    let final_state = &browser["final"];
    let uptime = |state: &Value| -> Result<String> {
        let text = state["uptime"]
            .as_str()
            .context("native browser uptime absent")?;
        ensure!(
            !text.trim().is_empty() && text.trim() != "-" && text.len() <= 256,
            "native browser uptime has no live value"
        );
        Ok(text.to_owned())
    };
    ensure!(
        browser["origin"] == origin
            && browser["real_DOM_login"] == true
            && browser["real_page_script_poll"] == true
            && initial["login"] == false
            && initial["ready"] == "complete"
            && final_state["login"] == false
            && initial["url"]
                .as_str()
                .is_some_and(|url| url.starts_with(&prefix))
            && final_state["url"]
                .as_str()
                .is_some_and(|url| url.starts_with(&prefix))
            && uptime(initial)? != uptime(final_state)?,
        "native browser login or live DOM update differs"
    );
    ensure!(
        browser["asset"]
            == json!({"path":"/luci-static/bootstrap/cascade.css",
        "sha256":"8204c85c4ebe203985adc4b63aa0fd717ce12ffc16f78c4eeb9890063501c477", "size":50527}),
        "native browser asset differs from archive"
    );
    let polls = final_state["resources"]
        .as_array()
        .context("native browser poll trace absent")?;
    ensure!(polls.len() <= 32 && polls.iter().any(|poll| {
        let url = poll["url"].as_str().unwrap_or("");
        let status = url.split_once('?').is_some_and(|(_, query)| query.split('&').any(|pair| pair == "status=1"));
        let start = poll["start"].as_f64();
        let end = poll["end"].as_f64();
        url.starts_with(&prefix) && status && matches!((start,end),(Some(a),Some(b)) if a.is_finite() && b.is_finite() && a >= 0.0 && b > 0.0 && b >= a)
    }), "native browser has no completed page poll");
    let cookies = browser["cookies"]
        .as_array()
        .context("native browser cookie metadata absent")?;
    let domain = match origin {
        "http://127.0.0.1:18081" => "127.0.0.1",
        "https://192.0.2.1:8443" => "192.0.2.1",
        _ => anyhow::bail!("unrecognized selected native cookie origin"),
    };
    ensure!(
        cookies.len() <= 64
            && cookies
                .iter()
                .filter(|cookie| cookie["name"] == "sysauth")
                .count()
                == 1
            && cookies.iter().any(|cookie| cookie["name"] == "sysauth"
                && cookie["httpOnly"] == false
                && cookie["secure"] == false
                && cookie["domain"] == domain
                && cookie["path"] == "/cgi-bin/luci/")
            && cookies.iter().all(|cookie| cookie.get("value").is_none()),
        "native browser authentication cookie metadata differs"
    );
    Ok(())
}

pub(super) fn verify_report(
    client: &[Value],
    server: &[Value],
    captured: &Value,
    selected: &ObjectId,
) -> Result<()> {
    let endpoint_event = client
        .iter()
        .filter(|event| event["actual_native_ssh_tunnel"] == true)
        .collect::<Vec<_>>();
    ensure!(endpoint_event.len() == 1, "selected native endpoint absent");
    let endpoint = &endpoint_event[0]["endpoint"];
    let report = single(client, "native_browser_report")?;
    ensure!(
        report["budgets"]
            == json!({
                "page_load_milliseconds": native_browser_budget::PAGE_LOAD_MILLISECONDS,
                "command_seconds": native_browser_budget::COMMAND_SECONDS,
                "login_telemetry_seconds": native_browser_budget::LOGIN_TELEMETRY_SECONDS
            }),
        "native browser support budgets differ"
    );
    let process = single(client, "native_browser_process")?;
    let driver = object(&captured["inputs"]["/native-browser-check"])?;
    let line = format!("{MARKER}{report}\n");
    parse_report(line.as_bytes(), endpoint, &driver)?;
    let stdout: ObjectId = serde_json::from_value(process["stdout"].clone())?;
    let stderr = hex::decode(
        process["stderr_hex"]
            .as_str()
            .context("browser stderr absent")?,
    )?;
    ensure!(
        process["driver"] == serde_json::to_value(&driver)?
            && process["exit_code"] == 0
            && process["error"].is_null()
            && process["deadline_seconds"] == native_browser_budget::HELPER_WALL_SECONDS
            && stdout.size == line.len() as u64
            && stdout.sha256 == hex::encode(Sha256::digest(line.as_bytes()))
            && process["stderr"]
                == json!({"size":stderr.len(),"sha256":hex::encode(Sha256::digest(&stderr))}),
        "browser process or exact report output differs"
    );
    ensure!(
        report["execution_identity"] == serde_json::to_value(selected)?
            && report["selected_run"] == selected.sha256
            && report["uid"] == 1001
            && report["gid"] == 1001
            && endpoint["run_manifest"]["worker_uid"] == 1000
            && report["firefox"] == captured["inputs"]["/usr/lib/firefox/firefox"]
            && report["libxul"] == captured["inputs"]["/usr/lib/firefox/libxul.so"]
            && report["original_board_gate"] == false
            && report["mining_gate"] == false
            && report["external_host_gate"] == false,
        "native browser execution identity or scope differs"
    );
    object(&report["firefox"])?;
    object(&report["libxul"])?;
    for (field, bytes) in [
        (
            "source_sha256",
            include_bytes!("../native_browser_check.rs").as_slice(),
        ),
        (
            "driver_source_sha256",
            include_bytes!("../support/browser_driver.rs").as_slice(),
        ),
        (
            "environment_source_sha256",
            include_bytes!("../support/browser_environment.rs").as_slice(),
        ),
        (
            "groups_source_sha256",
            include_bytes!("../support/browser_groups.rs").as_slice(),
        ),
    ] {
        ensure!(
            report[field] == hex::encode(Sha256::digest(bytes)),
            "browser compiled source differs: {field}"
        );
    }
    let credentials = server
        .iter()
        .filter(|event| event["schema"] == "md-sim-test-credentials/1")
        .collect::<Vec<_>>();
    ensure!(
        credentials.len() == 1
            && credentials[0]["run_id"] == selected.sha256
            && credentials[0]["ca"] == endpoint["gateway"]["client_ca"]
            && credentials[0]["server_certificate"] == endpoint["gateway"]["server_certificate"],
        "browser selected credential authority differs"
    );
    for field in ["ca", "client_certificate", "server_certificate"] {
        object(&credentials[0][field])?;
    }
    let events = report["events"]
        .as_array()
        .context("browser events absent")?;
    let expected = [
        "browser-started",
        "native-browser-ssh",
        "native-browser-tls",
        "browser-stopped",
        "browser-started",
        "native-browser-isolation",
        "browser-stopped",
    ];
    ensure!(
        events.len() == expected.len()
            && events
                .iter()
                .zip(expected)
                .all(|(event, name)| event["event"] == name),
        "native browser lifecycle events are missing, duplicate or unordered"
    );
    let cleanup = report["cleanup"]
        .as_array()
        .context("browser cleanup absent")?;
    ensure!(cleanup.len() == 2, "browser cleanup count differs");
    for (index, start, stop) in [(0, &events[0], &events[3]), (1, &events[4], &events[6])] {
        let profile = format!("/work/browser/profile-{index}");
        let metadata = &start["metadata"];
        let credential = &start["credentials"];
        ensure!(
            start["index"] == index
                && stop["index"] == index
                && metadata["profile"] == profile
                && metadata["capabilities"]["capabilities"]["acceptInsecureCerts"] == false
                && metadata["capabilities"]["capabilities"]["browserName"] == "firefox"
                && credential["schema"] == "md-sim-browser-profile/1"
                && credential["profile"] == profile
                && credential["run_id"] == selected.sha256
                && credential["ca"] == credentials[0]["ca"]
                && credential["client_certificate"] == credentials[0]["client_certificate"]
                && credential["client_certificate_validated"] == true
                && credential["private_import_file_removed"] == true
                && credential["ca_trust"] == "CT,,",
            "browser profile or selected credentials differ"
        );
        object(&metadata["prefs"])?;
        for name in ["ca", "client"] {
            object(&credential["certificate_der_readback"][name])?;
        }
        for name in ["openssl", "certutil", "pk12util"] {
            ensure!(
                credential["tools"][name] == captured["inputs"][format!("/usr/bin/{name}")],
                "browser support tool differs: {name}"
            );
        }
        ensure!(
            cleanup[index]["group"] == format!("/sys/fs/cgroup/md-sim-browser/profile-{index}")
                && cleanup[index]["empty"] == true
                && cleanup[index]["error"].is_null()
                && stop["cleanup"] == cleanup[index],
            "browser cleanup does not prove empty owned leaves"
        );
    }
    for event in [&events[1], &events[2], &events[5]] {
        ensure!(
            event["selected_run"] == selected.sha256,
            "browser event selected another run"
        );
    }
    check_dashboard(&events[1]["browser"], "http://127.0.0.1:18081")?;
    check_dashboard(&events[2]["browser"], "https://192.0.2.1:8443")?;
    ensure!(
        events[1]["transport"] == endpoint_event[0]["transport"]
            && events[2]["certificate_verification"] == true
            && events[5]["profile"] == "/work/browser/profile-1"
            && events[5]["same_origin"] == "https://192.0.2.1:8443"
            && events[5]["cookies_empty"] == true
            && events[5]["state"]["login"] == true
            && events[5]["state"]["url"]
                .as_str()
                .is_some_and(|url| url.starts_with("https://192.0.2.1:8443/cgi-bin/luci/")),
        "browser certificate verification or same-origin profile isolation failed"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    // Construct one complete observation independently of the live helper. It
    // exercises joins between process bytes, endpoint, profiles and captures.
    fn observation() -> Result<(Vec<Value>, Vec<Value>, Value, ObjectId)> {
        let identity = |digit: &str, size: u64| json!({"sha256":digit.repeat(64),"size":size});
        let selected: ObjectId = serde_json::from_value(identity("a", 10))?;
        let captured = json!({"inputs":{
            "/native-browser-check":identity("b",20),
            "/usr/lib/firefox/firefox":identity("c",30),
            "/usr/lib/firefox/libxul.so":identity("d",40),
            "/usr/bin/openssl":identity("e",50),
            "/usr/bin/certutil":identity("f",60),
            "/usr/bin/pk12util":identity("0",70)}});
        let credential = json!({"schema":"md-sim-test-credentials/1","run_id":selected.sha256,
            "ca":identity("1",80),"client_certificate":identity("2",90),"server_certificate":identity("3",100)});
        let endpoint = json!({"run_id":selected.sha256,"execution_identity":selected,
            "run_manifest":{"worker_uid":1000},
            "gateway":{"client_ca":credential["ca"],"server_certificate":credential["server_certificate"]}});
        let tunnel = json!({"transport":{"host":"192.0.2.1","host_key_policy":"StrictHostKeyChecking=yes; UpdateHostKeys=no"},
            "listen":"127.0.0.1:18081","remote_loopback":"127.0.0.1:18080"});
        let cleanup = |index| json!({"group":format!("/sys/fs/cgroup/md-sim-browser/profile-{index}"),"empty":true,"reaped_helpers":2});
        let start = |index| {
            let profile = format!("/work/browser/profile-{index}");
            json!({"event":"browser-started","index":index,"metadata":{"profile":profile,"prefs":identity("4",110),
                "capabilities":{"capabilities":{"browserName":"firefox","acceptInsecureCerts":false}}},
                "credentials":{"schema":"md-sim-browser-profile/1","profile":profile,"run_id":selected.sha256,
                    "ca":credential["ca"],"client_certificate":credential["client_certificate"],
                    "client_certificate_validated":true,"private_import_file_removed":true,"ca_trust":"CT,,",
                    "certificate_der_readback":{"ca":identity("5",120),"client":identity("6",130)},
                    "tools":{"openssl":captured["inputs"]["/usr/bin/openssl"],"certutil":captured["inputs"]["/usr/bin/certutil"],"pk12util":captured["inputs"]["/usr/bin/pk12util"]}}})
        };
        let dashboard = |origin: &str| {
            let url = format!("{origin}/cgi-bin/luci/admin/status/overview");
            let domain = if origin.starts_with("http:") {
                "127.0.0.1"
            } else {
                "192.0.2.1"
            };
            json!({"origin":origin,"real_DOM_login":true,"real_page_script_poll":true,
                "initial":{"url":url,"ready":"complete","login":false,"uptime":"0h 3m 21s"},
                "final":{"login":false,"url":url,"uptime":"0h 3m 26s","resources":[{"url":format!("{url}?status=1"),"start":1,"end":2}]},
                "asset":{"path":"/luci-static/bootstrap/cascade.css","sha256":"8204c85c4ebe203985adc4b63aa0fd717ce12ffc16f78c4eeb9890063501c477","size":50527},
                "cookies":[{"name":"sysauth","httpOnly":false,"secure":false,"domain":domain,"path":"/cgi-bin/luci/"}]})
        };
        let mut report = json!({"schema":"md-sim-native-browser/1","passed":true,"error":null,
            "budgets":{"page_load_milliseconds":native_browser_budget::PAGE_LOAD_MILLISECONDS,"command_seconds":native_browser_budget::COMMAND_SECONDS,"login_telemetry_seconds":native_browser_budget::LOGIN_TELEMETRY_SECONDS},
            "selected_run":selected.sha256,"execution_identity":selected,"driver":captured["inputs"]["/native-browser-check"],
            "firefox":captured["inputs"]["/usr/lib/firefox/firefox"],"libxul":captured["inputs"]["/usr/lib/firefox/libxul.so"],
            "original_board_gate":false,"mining_gate":false,"external_host_gate":false,
            "cleanup":[cleanup(0),cleanup(1)],"events":[start(0),
                {"event":"native-browser-ssh","selected_run":selected.sha256,"browser":dashboard("http://127.0.0.1:18081"),"transport":tunnel},
                {"event":"native-browser-tls","selected_run":selected.sha256,"browser":dashboard("https://192.0.2.1:8443"),"certificate_verification":true},
                {"event":"browser-stopped","index":0,"cleanup":cleanup(0)},start(1),
                {"event":"native-browser-isolation","selected_run":selected.sha256,"profile":"/work/browser/profile-1","same_origin":"https://192.0.2.1:8443","cookies_empty":true,
                    "state":{"login":true,"url":"https://192.0.2.1:8443/cgi-bin/luci/admin/status/overview"}},
                {"event":"browser-stopped","index":1,"cleanup":cleanup(1)}]});
        for (field, bytes) in [
            (
                "source_sha256",
                include_bytes!("../native_browser_check.rs").as_slice(),
            ),
            (
                "driver_source_sha256",
                include_bytes!("../support/browser_driver.rs").as_slice(),
            ),
            (
                "environment_source_sha256",
                include_bytes!("../support/browser_environment.rs").as_slice(),
            ),
            (
                "groups_source_sha256",
                include_bytes!("../support/browser_groups.rs").as_slice(),
            ),
        ] {
            report[field] = json!(hex::encode(Sha256::digest(bytes)));
        }
        report["uid"] = json!(1001);
        report["gid"] = json!(1001);
        let mut client = vec![
            json!({"actual_native_ssh_tunnel":true,"endpoint":endpoint,"transport":tunnel}),
            json!({"native_browser_report":report}),
            json!({"native_browser_process":{"driver":captured["inputs"]["/native-browser-check"],
            "exit_code":0,"error":null,"deadline_seconds":native_browser_budget::HELPER_WALL_SECONDS,"stderr_hex":"","stderr":{"size":0,"sha256":hex::encode(Sha256::digest([]))}}}),
        ];
        sign_observed_stdout(&mut client);
        Ok((client, vec![credential], captured, selected))
    }
    fn sign_observed_stdout(client: &mut [Value]) {
        let line = format!("{MARKER}{}\n", client[1]["native_browser_report"]);
        client[2]["native_browser_process"]["stdout"] =
            json!({"sha256":hex::encode(Sha256::digest(line.as_bytes())),"size":line.len()});
    }
    #[test]
    fn complete_browser_observation_joins_are_checked() -> Result<()> {
        let (client, server, captured, selected) = observation()?;
        verify_report(&client, &server, &captured, &selected)?;
        for (pointer, changed) in [
            ("/uid", json!(1000)),
            ("/gid", json!(1000)),
            ("/source_sha256", json!("0".repeat(64))),
            ("/budgets/page_load_milliseconds", json!(300000)),
            ("/budgets/command_seconds", json!(1200)),
            ("/budgets/login_telemetry_seconds", json!(300)),
            ("/events/1/browser/initial/ready", json!("loading")),
            ("/events/1/browser/cookies/0/httpOnly", json!(true)),
            ("/events/1/browser/cookies/0/secure", json!(true)),
            ("/events/1/browser/cookies/0/domain", json!("192.0.2.1")),
            ("/events/1/browser/cookies/0/path", json!("/")),
            (
                "/events/0/credentials/client_certificate/sha256",
                json!("9".repeat(64)),
            ),
            (
                "/events/4/metadata/profile",
                json!("/work/browser/profile-0"),
            ),
            (
                "/events/4/credentials/tools/certutil/sha256",
                json!("8".repeat(64)),
            ),
            ("/events/5/cookies_empty", json!(false)),
            ("/cleanup/1/empty", json!(false)),
            ("/events/2/browser/final/uptime", json!("0h 3m 21s")),
            ("/events/2/selected_run", json!("9".repeat(64))),
            (
                "/events/0/metadata/capabilities/capabilities/acceptInsecureCerts",
                json!(true),
            ),
        ] {
            let mut altered = client.clone();
            *altered[1]["native_browser_report"]
                .pointer_mut(pointer)
                .context("regression observation absent")? = changed;
            sign_observed_stdout(&mut altered);
            assert!(
                verify_report(&altered, &server, &captured, &selected).is_err(),
                "accepted changed observation {pointer}"
            );
        }
        let mut reordered = client.clone();
        reordered[1]["native_browser_report"]["events"]
            .as_array_mut()
            .unwrap()
            .swap(0, 4);
        sign_observed_stdout(&mut reordered);
        assert!(verify_report(&reordered, &server, &captured, &selected).is_err());
        let mut unbound = client.clone();
        unbound[2]["native_browser_process"]["stdout"]["size"] = json!(1);
        assert!(verify_report(&unbound, &server, &captured, &selected).is_err());
        let mut changed_capture = captured.clone();
        changed_capture["inputs"]["/usr/lib/firefox/firefox"]["sha256"] = json!("7".repeat(64));
        assert!(verify_report(&client, &server, &changed_capture, &selected).is_err());
        let mut changed_server = server.clone();
        changed_server[0]["ca"]["sha256"] = json!("7".repeat(64));
        assert!(verify_report(&client, &changed_server, &captured, &selected).is_err());
        Ok(())
    }
    #[test]
    fn dashboard_requires_real_change_completed_poll_and_selected_origin() -> Result<()> {
        let origin = "https://192.0.2.1:8443";
        let url = format!("{origin}/cgi-bin/luci/admin/status/overview");
        let observed = json!({"origin":origin,"real_DOM_login":true,"real_page_script_poll":true,
            "initial":{"login":false,"ready":"complete","url":url,"uptime":"0h 3m 21s"},
            "final":{"login":false,"url":url,"uptime":"0h 3m 26s","resources":[{"url":format!("{url}?status=1"),"start":7.0,"end":10.5}]},
            "asset":{"path":"/luci-static/bootstrap/cascade.css","sha256":"8204c85c4ebe203985adc4b63aa0fd717ce12ffc16f78c4eeb9890063501c477","size":50527},
            "cookies":[{"name":"sysauth","httpOnly":false,"secure":false,"domain":"192.0.2.1","path":"/cgi-bin/luci/"}]});
        check_dashboard(&observed, origin)?;
        for (pointer, value) in [
            ("/final/uptime", json!("0h 3m 21s")),
            ("/final/login", json!(true)),
            ("/final/url", json!("https://192.0.2.3:8443/cgi-bin/luci/")),
            ("/final/resources/0/end", json!(0)),
            ("/final/resources/0/end", json!(6)),
            ("/final/resources/0/url", json!(format!("{url}?status=11"))),
            ("/asset/sha256", json!("a".repeat(64))),
            ("/cookies/0/httpOnly", json!(true)),
        ] {
            let mut changed = observed.clone();
            *changed
                .pointer_mut(pointer)
                .context("missing regression field")? = value;
            assert!(
                check_dashboard(&changed, origin).is_err(),
                "accepted altered observation {pointer}"
            );
        }
        Ok(())
    }
    #[test]
    fn payload_cannot_be_substituted_after_capture() -> Result<()> {
        use md_sim_runtime::isolation::{Limits, Mount, ProcessSpec};
        let directory = tempfile::tempdir()?;
        let browser = ObjectId {
            sha256: "a".repeat(64),
            size: 123,
        };
        let image = ObjectId {
            sha256: "b".repeat(64),
            size: 456,
        };
        let manifest = json!({"schema":"md-sim-vm-payload/1",
            "files":{"/usr/lib/firefox/firefox":{"object":browser,"source":"/captured/object","mode":365}},
            "symlinks":{"/usr/bin/firefox":"/usr/lib/firefox/firefox"}});
        let path = directory.path().join("payload-manifest.json");
        fs::write(&path, serde_json::to_vec(&manifest)?)?;
        let report = json!({"manifest":hash_file(&path)?,"combined_initramfs":image});
        fs::write(
            directory.path().join("payload-report.json"),
            serde_json::to_vec(&report)?,
        )?;
        let specification =
            json!({"inputs":{"/usr/lib/firefox/firefox":browser},"symlinks":manifest["symlinks"]});
        let inputs = vec![Mount {
            guest_path: "/usr/lib/firefox/firefox".into(),
            object: browser,
        }];
        let mut process = ProcessSpec {
            private_channel: None,
            stdin_stream: None,
            namespace_uid: 0,
            dashboard: None,
            loopback: false,
            schema_version: "2.0".into(),
            argv: vec!["/qemu".into()],
            inputs: vec![Mount {
                guest_path: "/initramfs.cpio.gz".into(),
                object: image,
            }],
            stdin_object: None,
            environment: BTreeMap::new(),
            limits: Limits::default(),
        };
        verify_captured_inputs(directory.path(), &specification, &inputs, &process)?;
        let mut changed = specification.clone();
        changed["inputs"]["/usr/lib/firefox/firefox"]["sha256"] = json!("c".repeat(64));
        assert!(verify_captured_inputs(directory.path(), &changed, &inputs, &process).is_err());
        let mut changed = specification.clone();
        changed["symlinks"]["/usr/bin/firefox"] = json!("/usr/bin/other");
        assert!(verify_captured_inputs(directory.path(), &changed, &inputs, &process).is_err());
        let mut duplicated = inputs.clone();
        duplicated.push(inputs[0].clone());
        assert!(
            verify_captured_inputs(directory.path(), &specification, &duplicated, &process)
                .is_err()
        );
        process.inputs[0].object.sha256 = "d".repeat(64);
        assert!(
            verify_captured_inputs(directory.path(), &specification, &inputs, &process).is_err()
        );
        Ok(())
    }
    #[test]
    fn subprocess_report_requires_exact_driver_endpoint_and_one_record() -> Result<()> {
        let driver = ObjectId {
            sha256: "a".repeat(64),
            size: 1,
        };
        let endpoint = json!({"run_id":"b".repeat(64),"execution_identity":{"sha256":"b".repeat(64),"size":2}});
        let report = json!({"schema":"md-sim-native-browser/1","passed":true,"error":null,
            "selected_run":endpoint["run_id"],"execution_identity":endpoint["execution_identity"],"driver":driver});
        let raw = format!("{MARKER}{report}\n");
        parse_report(raw.as_bytes(), &endpoint, &driver)?;
        assert!(parse_report(format!("{raw}{raw}").as_bytes(), &endpoint, &driver).is_err());
        let mut wrong = endpoint.clone();
        wrong["run_id"] = json!("c".repeat(64));
        assert!(parse_report(raw.as_bytes(), &wrong, &driver).is_err());
        let mut failed = report.clone();
        failed["passed"] = json!(false);
        assert!(
            parse_report(format!("{MARKER}{failed}\n").as_bytes(), &endpoint, &driver).is_err()
        );
        assert!(parse_report(
            raw.as_bytes(),
            &endpoint,
            &ObjectId {
                sha256: "d".repeat(64),
                size: 1
            }
        )
        .is_err());
        Ok(())
    }
}

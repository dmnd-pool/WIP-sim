//! Actual captured Firefox against the selected archived LuCI service.
//! The parent owns the disconnected VM, native firmware and TLS gateway.
#[path = "support/browser_driver.rs"]
#[allow(dead_code)] // The component probe also uses the renderer diagnostic.
mod browser_driver;
#[path = "support/browser_environment.rs"]
mod browser_environment;
#[path = "support/browser_groups.rs"]
mod browser_groups;
#[path = "support/native_browser_budget.rs"]
#[allow(dead_code)] // Parent helper uses the other support-tool limits.
mod native_browser_budget;
use anyhow::{ensure, Context, Result};
use md_sim_access::{
    browser::Session,
    credentials::{self, BrowserProfileTool, BrowserProfileTools},
};
use md_sim_coordinator::ssh_transport::{SshConfig, SshTunnel};
use md_sim_runtime::{
    isolation::save_json,
    store::{hash_file, ObjectId},
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

const CSS_PATH: &str = "/luci-static/bootstrap/cascade.css";
const CSS_SHA: &str = "8204c85c4ebe203985adc4b63aa0fd717ce12ffc16f78c4eeb9890063501c477";
fn value(value: Value) -> Value {
    value.get("value").cloned().unwrap_or(value)
}
fn read(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "native browser input exceeds bound");
    Ok(bytes)
}
fn copy_private(source: &str, name: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    ensure!(
        metadata.is_file(),
        "client credential is not a regular file"
    );
    let bytes = read(Path::new(source), 65536)?;
    let path = Path::new("/work/browser/credentials").join(name);
    let mut output = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&path)?;
    output.write_all(&bytes)?;
    browser_environment::chown(&path)?;
    Ok(())
}
fn element(session: &mut Session, selector: &str) -> Result<String> {
    value(session.find(selector)?)["element-6066-11e4-a52e-4f735466cecf"]
        .as_str()
        .context("native DOM element identity absent")
        .map(str::to_owned)
}
fn probe_tunnel() -> Result<()> {
    let program = Path::new("/usr/bin/curl");
    let start = Instant::now();
    let result = Command::new(program)
        .env_clear()
        .args([
            "--silent",
            "--show-error",
            "--noproxy",
            "*",
            "--max-time",
            "15",
            "--max-filesize",
            "1048576",
            "--write-out",
            "%{http_code}",
            "--output",
            "/work/browser/tunnel-probe.body",
            "--dump-header",
            "/work/browser/tunnel-probe.headers",
            "http://127.0.0.1:18081/cgi-bin/luci/",
        ])
        .output()?;
    let body = read(Path::new("/work/browser/tunnel-probe.body"), 1 << 20).unwrap_or_default();
    let headers =
        read(Path::new("/work/browser/tunnel-probe.headers"), 1 << 20).unwrap_or_default();
    save_json(
        Path::new("/work/browser/tunnel-probe.json"),
        &json!({
            "program":hash_file(program)?,"elapsed_ms":start.elapsed().as_millis(),
            "url":"http://127.0.0.1:18081/cgi-bin/luci/","exit_code":result.status.code(),
            "status":String::from_utf8_lossy(&result.stdout),"stderr_hex":hex::encode(&result.stderr),
            "body_sha256":md_sim_firmware::prepare::hash_bytes(&body),
            "body_size":body.len(),"headers_hex":hex::encode(headers),
            "login_form_present":body.windows(b"luci_password".len()).any(|w|w==b"luci_password")
        }),
    )?;
    ensure!(
        result.status.success()
            && result.stdout == b"403"
            && body
                .windows(b"luci_password".len())
                .any(|w| w == b"luci_password"),
        "native browser's own SSH tunnel did not serve the archived login form"
    );
    Ok(())
}

fn failed_page(session: &mut Session, stage: &str, error: &anyhow::Error) {
    let observation = (|| -> Result<Value> {
        session.set_timeout(Duration::from_secs(5))?;
        session.execute("return {url:location.href,ready:document.readyState,title:document.title,body:document.body?.innerText?.slice(0,4096),login:!!document.querySelector('input[name=luci_password]'),navigation:performance.getEntriesByType('navigation').map(r=>r.toJSON()),resources:performance.getEntriesByType('resource').slice(-32).map(r=>r.toJSON())}",vec![])
    })();
    let _ = save_json(
        Path::new("/work/browser/failure-dom.json"),
        &json!({
            "stage":stage,"error":format!("{error:#}"),
            "observation":observation.as_ref().ok(),
            "observation_error":observation.as_ref().err().map(|e|format!("{e:#}"))
        }),
    );
}
fn login(session: &mut Session, origin: &str) -> Result<Value> {
    session.navigate(&format!("{origin}/cgi-bin/luci/"))?;
    let username = element(session, "input[name=luci_username]")?;
    let password = element(session, "input[name=luci_password]")?;
    session.clear(&username)?;
    session.send_keys(&username, "root")?;
    session.clear(&password)?;
    let submit = element(session, "input[type=submit],button[type=submit]")?;
    session.click(&submit)?;
    let start = Instant::now();
    let mut page_ready_since = None;
    loop {
        let state=value(session.execute(
            "return {url:location.href,ready:document.readyState,login:!!document.querySelector('input[name=luci_password]'),uptime:document.querySelector('#uptime')?.textContent??null,title:document.title}",vec![])?);
        // Click can return before its redirect finishes. Apply the declared
        // page allowance to that navigation, then start the live-data deadline.
        // Native018 was still parsing the real200 overview when the old
        // combined30s navigation/telemetry timer expired.
        if state["ready"] == "complete" && state["login"] == false {
            page_ready_since.get_or_insert_with(Instant::now);
        }
        if state["login"] == false
            && state["ready"] == "complete"
            && state["uptime"]
                .as_str()
                .is_some_and(|text| !text.trim().is_empty() && text.trim() != "-")
        {
            ensure!(
                state["url"]
                    .as_str()
                    .is_some_and(|url| url.starts_with(&format!("{origin}/cgi-bin/luci/"))),
                "native login redirected outside selected origin"
            );
            return Ok(state);
        }
        if let Some(ready) = page_ready_since {
            ensure!(
                ready.elapsed()
                    < Duration::from_secs(native_browser_budget::LOGIN_TELEMETRY_SECONDS),
                "native login telemetry readiness deadline: {state}"
            );
        } else {
            ensure!(
                start.elapsed()
                    < Duration::from_millis(u64::from(
                        native_browser_budget::PAGE_LOAD_MILLISECONDS
                    )),
                "native login page readiness deadline: {state}"
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}
fn live_dashboard(session: &mut Session, origin: &str) -> Result<Value> {
    let initial = login(session, origin)?;
    let asset=value(session.execute(
        "return fetch(arguments[0],{cache:'no-store'}).then(async r=>({status:r.status,url:r.url,bytes:Array.from(new Uint8Array(await r.arrayBuffer()))}))",vec![json!(CSS_PATH)])?);
    let bytes: Vec<u8> = serde_json::from_value(asset["bytes"].clone())?;
    ensure!(
        asset["status"] == 200
            && asset["url"] == format!("{origin}{CSS_PATH}")
            && bytes.len() == 50527
            && md_sim_firmware::prepare::hash_bytes(&bytes) == CSS_SHA,
        "real browser native CSS differs from archive"
    );
    let start = Instant::now();
    let final_state = loop {
        let state=value(session.execute(
            "return {url:location.href,login:!!document.querySelector('input[name=luci_password]'),uptime:document.querySelector('#uptime')?.textContent??null,resources:performance.getEntriesByType('resource').filter(r=>r.initiatorType==='xmlhttprequest').map(r=>({url:r.name,start:r.startTime,end:r.responseEnd})).slice(-32)}",vec![])?);
        if state["uptime"].is_string() && state["uptime"] != initial["uptime"] {
            break state;
        }
        ensure!(
            start.elapsed() < Duration::from_secs(25),
            "native page script did not update uptime: {state}"
        );
        std::thread::sleep(Duration::from_millis(200));
    };
    let polls = final_state["resources"]
        .as_array()
        .context("native browser resource trace absent")?;
    ensure!(
        polls.iter().any(|record| record["url"]
            .as_str()
            .is_some_and(|url| url.starts_with(&format!("{origin}/cgi-bin/luci/"))
                && url.contains("status=1"))
            && record["end"].as_f64().is_some_and(|end| end > 0.0)),
        "native live poll request absent"
    );
    let cookies = value(session.cookies()?);
    let cookies = cookies.as_array().context("native cookie list absent")?;
    let domain = origin
        .split_once("://")
        .context("native cookie origin has no scheme")?
        .1
        .split(':')
        .next()
        .context("native cookie origin has no host")?;
    ensure!(
        cookies
            .iter()
            .filter(|cookie| cookie["name"] == "sysauth")
            .count()
            == 1
            && cookies.iter().any(|cookie| cookie["name"] == "sysauth"
                && cookie["httpOnly"] == false
                && cookie["secure"] == false
                && cookie["domain"] == domain
                && cookie["path"] == "/cgi-bin/luci/"),
        "native login cookie differs from archived LuCI attributes"
    );
    // Exact dispatcher.lua and native018 HTTP headers set only sysauth/path.
    // HttpOnly and Secure are absent in this firmware. Preserve that behavior;
    // profile isolation below must still prove that credentials never leak.
    let cookie_metadata:Vec<_>=cookies.iter().map(|cookie|json!({"name":cookie["name"],"domain":cookie["domain"],"path":cookie["path"],"httpOnly":cookie["httpOnly"],"secure":cookie["secure"]})).collect();
    Ok(
        json!({"origin":origin,"initial":initial,"final":final_state,"asset":{"path":CSS_PATH,"sha256":CSS_SHA,"size":bytes.len()},"cookies":cookie_metadata,"real_DOM_login":true,"real_page_script_poll":true}),
    )
}
fn exercise(
    endpoint: &Value,
    groups: Vec<browser_groups::Group>,
    events: &mut Vec<Value>,
) -> Result<()> {
    let run_id = endpoint["run_id"]
        .as_str()
        .context("selected native run absent")?;
    let origin = endpoint["gateway"]["url"]
        .as_str()
        .context("selected gateway URL absent")?
        .strip_suffix('/')
        .context("gateway URL has no canonical root path")?;
    ensure!(
        origin == "https://192.0.2.1:8443",
        "test gateway is outside isolated worker IP"
    );
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
    let mut tunnel = SshTunnel::start(
        SshConfig {
            host: "192.0.2.1".into(),
            user: "worker".into(),
            port: 2222,
            known_hosts: "/work/browser/credentials/known_hosts".into(),
            identity_file: "/work/browser/credentials/clientkey".into(),
            remote_relay: "/md-sim-worker-relay".into(),
            worker_socket: "/worker/worker.sock".into(),
            ssh_program: "/usr/bin/ssh".into(),
            timeout_seconds: 30,
        },
        "127.0.0.1:18081".parse()?,
        "127.0.0.1:18080".parse()?,
        Path::new("/work/browser/ssh-tunnel.log"),
    )?;
    probe_tunnel()?;
    for (index, group) in groups.into_iter().enumerate() {
        let profile = browser_driver::prepare_profile(
            index,
            "user_pref(\"security.default_personal_cert\", \"Select Automatically\");\n",
        )?;
        let credential = credentials::prepare_browser_profile(
            &tools,
            Path::new(&profile),
            Path::new("/work/browser/credentials/ca.crt"),
            Path::new("/work/browser/credentials/client.crt"),
            Path::new("/work/browser/credentials/client.key"),
            run_id,
        )?;
        let (mut browser, mut session, metadata) = browser_driver::launch(index, group)?;
        events.push(json!({"event":"browser-started","index":index,"metadata":metadata,"handshake":session.handshake,"credentials":credential}));
        session.set_timeout(Duration::from_secs(native_browser_budget::COMMAND_SECONDS))?;
        session.set_page_timeout(native_browser_budget::PAGE_LOAD_MILLISECONDS)?;
        session.set_script_timeout(15000)?;
        ensure!(
            value(session.cookies()?) == json!([]),
            "fresh browser profile contains cookies"
        );
        if index == 0 {
            let ssh = live_dashboard(&mut session, "http://127.0.0.1:18081")
                .inspect_err(|error| failed_page(&mut session, "ssh", error))
                .context("native browser SSH dashboard")?;
            events.push(json!({"event":"native-browser-ssh","selected_run":run_id,"browser":ssh,"transport":tunnel.evidence()}));
            let tls = live_dashboard(&mut session, origin)
                .inspect_err(|error| failed_page(&mut session, "tls", error))
                .context("native browser TLS dashboard")?;
            events.push(json!({"event":"native-browser-tls","selected_run":run_id,"browser":tls,"certificate_verification":true}));
        } else {
            session.navigate(&format!("{origin}/cgi-bin/luci/admin/status/overview"))?;
            let state=value(session.execute("return {url:location.href,login:!!document.querySelector('input[name=luci_password]')}",vec![])?);
            ensure!(
                state["login"] == true && value(session.cookies()?) == json!([]),
                "new same-origin profile inherited authentication"
            );
            events.push(json!({"event":"native-browser-isolation","selected_run":run_id,"profile":profile,"same_origin":origin,"state":state,"cookies_empty":true}));
        }
        session.close()?;
        let cleanup = browser.stop()?;
        events.push(json!({"event":"browser-stopped","index":index,"cleanup":cleanup}));
        drop(browser);
    }
    tunnel.stop()?;
    Ok(())
}
fn main() -> Result<()> {
    ensure!(
        std::env::args().len() == 1,
        "native browser helper takes no arguments"
    );
    let endpoint: Value =
        serde_json::from_slice(&read(Path::new("/native-endpoint.json"), 1 << 20)?)?;
    let identity: ObjectId = serde_json::from_value(endpoint["execution_identity"].clone())?;
    identity.validate()?;
    let manifest = serde_json::to_vec_pretty(&endpoint["run_manifest"])?;
    ensure!(
        manifest.len() as u64 == identity.size
            && md_sim_firmware::prepare::hash_bytes(&manifest) == identity.sha256
            && endpoint["run_id"] == identity.sha256
            && endpoint["gateway"]["execution_identity"] == endpoint["execution_identity"]
            && endpoint["gateway"]["run_id"] == endpoint["run_id"],
        "native endpoint identity differs"
    );
    ensure!(
        !Path::new("/credentials/server.key").exists() && !Path::new("/worker/native").exists(),
        "browser can access worker private inputs"
    );
    ensure!(
        !fs::read_to_string("/proc/net/route")?
            .lines()
            .skip(1)
            .any(|line| line.split_whitespace().nth(1) == Some("00000000")),
        "native browser has a default network route"
    );
    let groups = browser_environment::root_setup(&["client0", "lo"], 1)?;
    fs::create_dir("/work/browser/credentials")?;
    fs::set_permissions(
        "/work/browser/credentials",
        fs::Permissions::from_mode(0o700),
    )?;
    browser_environment::chown(Path::new("/work/browser/credentials"))?;
    for (source, name) in [
        ("/credentials/ca.crt", "ca.crt"),
        ("/credentials/client.crt", "client.crt"),
        ("/credentials/client.key", "client.key"),
        ("/work/clientkey", "clientkey"),
        ("/work/known_hosts", "known_hosts"),
    ] {
        copy_private(source, name)?;
    }
    browser_environment::drop_privileges()?;
    ensure!(
        endpoint["run_manifest"]["worker_uid"] == 1000 && unsafe { libc::getuid() } != 1000,
        "browser shares worker task accounting"
    );
    let mut events = Vec::new();
    let result = exercise(&endpoint, groups, &mut events);
    let mut logs = Vec::new();
    for index in 0..2 {
        for stream in ["stdout", "stderr"] {
            let path = format!("/work/browser/{index}-{stream}.log");
            if Path::new(&path).exists() {
                logs.push(
                    json!({"path":path,"log":browser_driver::bounded_log(Path::new(&path))?}),
                );
            }
        }
    }
    for name in ["ssh-tunnel.log", "tunnel-probe.json", "failure-dom.json"] {
        let path = format!("/work/browser/{name}");
        if Path::new(&path).exists() {
            logs.push(json!({"path":path,"log":browser_driver::bounded_log(Path::new(&path))?}));
        }
    }
    let cleanup = (0..2)
        .map(|index| -> Result<Value> {
            Ok(serde_json::from_slice(&read(
                Path::new(&format!("/work/browser/cleanup-{index}.json")),
                65536,
            )?)?)
        })
        .collect::<Result<Vec<_>>>()?;
    let cleaned = cleanup
        .iter()
        .all(|record| record["empty"] == true && record["error"].is_null());
    let mut report = json!({"schema":"md-sim-native-browser/1","passed":result.is_ok() && cleaned,"selected_run":identity.sha256,"execution_identity":identity,
        "driver":hash_file(&std::env::current_exe()?)?,"firefox":hash_file(Path::new("/usr/lib/firefox/firefox"))?,"libxul":hash_file(Path::new("/usr/lib/firefox/libxul.so"))?,
        "source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("native_browser_check.rs")),
        "driver_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/browser_driver.rs")),
        "environment_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/browser_environment.rs")),
        "groups_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/browser_groups.rs")),
        "budgets":{"page_load_milliseconds":native_browser_budget::PAGE_LOAD_MILLISECONDS,"command_seconds":native_browser_budget::COMMAND_SECONDS,"login_telemetry_seconds":native_browser_budget::LOGIN_TELEMETRY_SECONDS},
        "uid":unsafe { libc::getuid() },"gid":unsafe { libc::getgid() },
        "events":events,"logs":logs,"cleanup":cleanup,"error":result.as_ref().err().map(|e|format!("{e:#}")),
        "original_board_gate":false,"mining_gate":false,"external_host_gate":false,
        "topology":"Captured Firefox in private client namespace of disconnected local VM; actual selected archived LuCI through SSH and authenticated TLS; isolated profiles"});
    if Path::new("/work/browser/start-failure.json").exists() {
        report["startup_diagnostics"] = serde_json::from_slice(&read(
            Path::new("/work/browser/start-failure.json"),
            1 << 20,
        )?)?;
    }
    save_json(Path::new("/work/browser/report.json"), &report)?;
    println!("MD_SIM_NATIVE_BROWSER_REPORT {report}");
    result?;
    ensure!(cleaned, "native browser descendants remain");
    Ok(())
}

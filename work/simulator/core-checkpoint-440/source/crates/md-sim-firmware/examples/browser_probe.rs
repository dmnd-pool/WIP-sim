//! Real Firefox component check in the disconnected test VM. The HTTP service
//! is an explicit test fixture, not miner firmware or a native dashboard.
#[path = "support/browser_driver.rs"]
mod browser_driver;
#[path = "support/browser_environment.rs"]
mod browser_environment;
#[path = "support/browser_groups.rs"]
mod browser_groups;
use anyhow::{ensure, Context, Result};
use browser_driver::{bounded_log, launch, prepare_profile};
use md_sim_runtime::{isolation::save_json, store::hash_file};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

const PAGE: &str = r#"<!doctype html><html><head><title>Simulator browser fixture</title></head><body><h1>SIMULATOR BROWSER FIXTURE</h1><button id="increment">Increment</button><output id="count">pending</output><script>async function read(){let r=await fetch('/count');document.querySelector('#count').textContent=await r.text()}document.querySelector('#increment').onclick=async()=>{await fetch('/increment',{method:'POST'});await read()};read()</script></body></html>"#;
fn value(result: Value) -> Value {
    result.get("value").cloned().unwrap_or(result)
}
fn exercise(
    url: &str,
    events: &mut Vec<Value>,
    groups: Vec<browser_groups::Group>,
    profile_only_addons: bool,
    trace_startup: bool,
) -> Result<()> {
    for (index, group) in groups.into_iter().enumerate() {
        // Exact captured XPIProvider source defines1 as the profile scope.
        // This diagnostic omits application/system add-ons; the fresh profile
        // contains none. It does not disable the browser's content sandbox.
        prepare_profile(
            index,
            if profile_only_addons {
                "user_pref(\"extensions.enabledScopes\", 1);\n"
            } else {
                ""
            },
        )?;
        let (mut browser, mut session, metadata) = if trace_startup {
            browser_driver::launch_with_trace(index, group, true)?
        } else {
            launch(index, group)?
        };
        events.push(json!({"event":"browser-started","index":index,"metadata":metadata,"handshake":session.handshake}));
        let initial = value(session.cookies()?);
        ensure!(initial == json!([]), "new profile contains cookies");
        session.navigate(&format!(
            "{url}/{}",
            if index == 0 { "set-cookie" } else { "" }
        ))?;
        let title = value(session.execute("return document.title", vec![])?);
        ensure!(
            title == "Simulator browser fixture",
            "real browser title differs: {title}"
        );
        let start = Instant::now();
        loop {
            let count = value(session.execute(
                "return document.querySelector('#count').textContent",
                vec![],
            )?);
            if count.as_str() == Some(if index == 0 { "0" } else { "1" }) {
                break;
            }
            ensure!(
                start.elapsed() < Duration::from_secs(20),
                "browser script did not fetch live count: {count}"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        let cookies = value(session.cookies()?);
        if index == 0 {
            ensure!(
                cookies
                    .as_array()
                    .context("invalid cookies")?
                    .iter()
                    .any(|c| c["name"] == "fixture_session" && c["value"] == "first-profile"),
                "browser did not retain service cookie"
            );
            let button = value(session.find("#increment")?);
            let id = button["element-6066-11e4-a52e-4f735466cecf"]
                .as_str()
                .context("browser element id absent")?;
            session.click(id)?;
            let start = Instant::now();
            loop {
                let count = value(session.execute(
                    "return document.querySelector('#count').textContent",
                    vec![],
                )?);
                if count == "1" {
                    break;
                }
                ensure!(
                    start.elapsed() < Duration::from_secs(20),
                    "real button did not change live service count: {count}"
                );
                std::thread::sleep(Duration::from_millis(100));
            }
        } else {
            ensure!(
                cookies == json!([]),
                "independent profile inherited another run's cookies"
            );
        }
        session.close()?;
        let cleanup = browser.stop()?;
        events.push(json!({"event":"browser-fixture-check","index":index,"url":url,"title":title,"cookies":cookies,"service_counter":if index==0 {1}else{index},"real_DOM_and_script":true,"separate_profile":true,"cleanup":cleanup}));
        drop(browser);
    }
    Ok(())
}
fn main() -> Result<()> {
    let groups = browser_environment::root_setup(&["lo"], 1)?;
    browser_environment::drop_privileges()?;
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments == ["--render-diagnostic"] || arguments == ["--render-memory-diagnostic"] {
        let trace_memory = arguments == ["--render-memory-diagnostic"];
        let mut groups = groups.into_iter();
        let result =
            browser_driver::render(groups.next().context("render group absent")?, trace_memory);
        drop(groups);
        let mut report = json!({"schema":"md-sim-browser-render/1","render_pass":result.is_ok(),
            "browser_component_gate":false,"native_dashboard_gate":false,"firmware_gate":false,
            "result":result.as_ref().ok(),"error":result.as_ref().err().map(|e|format!("{e:#}")),
            "driver":hash_file(&std::env::current_exe()?)?,
            "firefox":hash_file(Path::new("/usr/lib/firefox/firefox"))?,
            "libxul":hash_file(Path::new("/usr/lib/firefox/libxul.so"))?,
            "stdout":bounded_log(Path::new("/work/browser/0-stdout.log"))?,
            "stderr":bounded_log(Path::new("/work/browser/0-stderr.log"))?});
        if Path::new("/work/browser/start-failure.json").exists() {
            report["diagnostics"] =
                serde_json::from_slice(&fs::read("/work/browser/start-failure.json")?)?;
        }
        if Path::new("/work/browser/memory-syscalls.log").exists() {
            report["memory_syscalls"] =
                bounded_log(Path::new("/work/browser/memory-syscalls.log"))?;
            report["strace"] = serde_json::to_value(hash_file(Path::new("/usr/bin/strace"))?)?;
        }
        report["cleanup"] = (0..2)
            .map(|index| -> Result<Value> {
                Ok(serde_json::from_slice(&fs::read(format!(
                    "/work/browser/cleanup-{index}.json"
                ))?)?)
            })
            .collect::<Result<Vec<_>>>()?
            .into();
        save_json(Path::new("/work/browser/report.json"), &report)?;
        println!("MD_SIM_BROWSER_RENDER_REPORT {report}");
        result?;
        return Ok(());
    }
    let trace_startup = arguments == ["--profile-only-addons", "--trace-startup"];
    let profile_only_addons = arguments == ["--profile-only-addons"] || trace_startup;
    ensure!(
        arguments.is_empty() || profile_only_addons,
        "unexpected browser probe argument"
    );
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let url = format!("http://{}", listener.local_addr()?);
    let stop = Arc::new(AtomicBool::new(false));
    let stop_server = stop.clone();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let server_requests = requests.clone();
    let counter = Arc::new(AtomicU64::new(0));
    let server_counter = counter.clone();
    let server = std::thread::spawn(move || -> Result<()> {
        'connections: while !stop_server.load(Ordering::SeqCst) {
            let (mut stream, _) = match listener.accept() {
                Ok(connection) => connection,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
                Err(error) => return Err(error.into()),
            };
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            stream.set_write_timeout(Some(Duration::from_secs(5)))?;
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                match stream.read(&mut byte) {
                    Ok(0) if bytes.is_empty() => continue 'connections,
                    Ok(1) => (),
                    Ok(_) => anyhow::bail!("fixture request ended inside header"),
                    Err(error) => return Err(error.into()),
                }
                bytes.push(byte[0]);
                ensure!(bytes.len() <= 16384, "fixture request header exceeds bound");
            }
            let request = std::str::from_utf8(&bytes)?;
            let first = request.lines().next().context("empty fixture request")?;
            let route = first
                .split_whitespace()
                .nth(1)
                .context("missing fixture route")?;
            let (body, content_type) = match (first.split_whitespace().next().unwrap_or(""), route)
            {
                ("POST", "/increment") => {
                    server_counter.fetch_add(1, Ordering::SeqCst);
                    ("ok".to_owned(), "text/plain")
                }
                ("GET", "/count") => (
                    server_counter.load(Ordering::SeqCst).to_string(),
                    "text/plain",
                ),
                ("GET", "/" | "/set-cookie") => (PAGE.to_owned(), "text/html; charset=utf-8"),
                _ => (String::new(), "text/plain"),
            };
            let mut records = server_requests
                .lock()
                .map_err(|_| anyhow::anyhow!("fixture request log poisoned"))?;
            ensure!(records.len() < 128, "fixture request count exceeds bound");
            records
                .push(json!({"request":request,"counter":server_counter.load(Ordering::SeqCst)}));
            drop(records);
            let cookie = if route == "/set-cookie" {
                "Set-Cookie: fixture_session=first-profile; Path=/; HttpOnly; SameSite=Strict\r\n"
            } else {
                ""
            };
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n{cookie}\r\n{body}", body.len())?;
        }
        Ok(())
    });
    let mut events = Vec::new();
    let result = exercise(
        &url,
        &mut events,
        groups,
        profile_only_addons,
        trace_startup,
    );
    stop.store(true, Ordering::SeqCst);
    let server_result = server
        .join()
        .map_err(|_| anyhow::anyhow!("fixture server panicked"))?;
    let mut logs = Vec::new();
    for index in 0..2 {
        for stream in ["stdout", "stderr"] {
            let path = format!("/work/browser/{index}-{stream}.log");
            if Path::new(&path).exists() {
                logs.push(json!({"path":path,"log":bounded_log(Path::new(&path))?}));
            }
        }
    }
    let report = json!({"schema":"md-sim-browser-component/1","component_pass":result.is_ok() && server_result.is_ok() && counter.load(Ordering::SeqCst)==1,"firmware_gate":false,"native_dashboard_gate":false,"service":"explicit Rust component fixture","topology":"captured Firefox in disconnected local Linux VM; loopback service; dedicated UID1001, zero capabilities, no-new-privileges","driver":hash_file(&std::env::current_exe()?)?,"source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("browser_probe.rs")),"firefox":hash_file(Path::new("/usr/lib/firefox/firefox"))?,"libxul":hash_file(Path::new("/usr/lib/firefox/libxul.so"))?,"events":events,"requests":*requests.lock().map_err(|_|anyhow::anyhow!("request log poisoned"))?,"logs":logs,"error":result.as_ref().err().map(|e|format!("{e:#}")),"server_error":server_result.as_ref().err().map(|e|format!("{e:#}")),"limits":{"cpu_seconds":browser_driver::CPU_SECONDS,"address_space_bytes":browser_driver::ADDRESS_SPACE_BYTES,"address_space_reason":"Trace013 shows an actual 16GiB PROT_NONE reservation; physical leaf memory remains768MiB and guest2GiB under outer3GiB cap","file_bytes":64*1024*1024,"uid_tasks":browser_driver::UID_TASK_LIMIT,"browser_leaf_tasks":browser_groups::TASK_LIMIT,"browser_start_seconds":browser_driver::STARTUP_SECONDS,"new_session_seconds":browser_driver::NEW_SESSION_SECONDS,"command_seconds":60}});
    let mut report = report;
    report["trace_startup"] = json!(trace_startup);
    if trace_startup && Path::new("/work/browser/memory-syscalls.log").exists() {
        report["failed_syscalls"] = bounded_log(Path::new("/work/browser/memory-syscalls.log"))?;
        report["strace"] = serde_json::to_value(hash_file(Path::new("/usr/bin/strace"))?)?;
    }
    report["profile_only_addons"] = json!(profile_only_addons);
    for (field, bytes) in [
        (
            "driver_source_sha256",
            include_bytes!("support/browser_driver.rs").as_slice(),
        ),
        (
            "environment_source_sha256",
            include_bytes!("support/browser_environment.rs").as_slice(),
        ),
        (
            "groups_source_sha256",
            include_bytes!("support/browser_groups.rs").as_slice(),
        ),
    ] {
        report[field] = json!(md_sim_firmware::prepare::hash_bytes(bytes));
    }
    let mut cleanup = Vec::new();
    for index in 0..2 {
        cleanup.push(serde_json::from_slice::<Value>(&fs::read(format!(
            "/work/browser/cleanup-{index}.json"
        ))?)?);
    }
    let cleanup_pass = cleanup
        .iter()
        .all(|record| record["empty"] == true && record["error"].is_null());
    report["cleanup"] = json!(cleanup);
    if !cleanup_pass {
        report["component_pass"] = json!(false);
    }
    if Path::new("/work/browser/start-failure.json").exists() {
        report["startup_diagnostics"] =
            serde_json::from_slice(&fs::read("/work/browser/start-failure.json")?)?;
    }
    save_json(Path::new("/work/browser/report.json"), &report)?;
    println!("MD_SIM_BROWSER_REPORT {}", serde_json::to_string(&report)?);
    result?;
    server_result?;
    ensure!(
        report["component_pass"] == true,
        "browser component did not pass"
    );
    Ok(())
}

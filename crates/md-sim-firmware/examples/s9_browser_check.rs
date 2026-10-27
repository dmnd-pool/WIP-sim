//! Captured Firefox against the native Boser service on the private miner VM.
//! This helper must run inside the support VM, before its pool drops privileges.
#[path = "support/browser_driver.rs"]
#[allow(dead_code)]
mod browser_driver;
#[path = "support/browser_environment.rs"]
mod browser_environment;
#[path = "support/browser_groups.rs"]
mod browser_groups;
#[path = "support/s9_browser_selected.rs"]
mod browser_selected;
#[path = "support/s9_browser_target.rs"]
mod browser_target;
#[path = "support/s9_browser_telemetry.rs"]
mod native_telemetry;
#[allow(dead_code)]
#[path = "support/s9_pool_input.rs"]
mod pool_input;
use anyhow::{ensure, Context, Result};
use browser_target::Target;
use md_sim_access::browser::Session;
use md_sim_runtime::{isolation::save_json, store::hash_file};
use serde_json::{json, Value};
use std::{
    fs,
    net::TcpStream,
    path::Path,
    sync::mpsc::{Receiver, Sender},
    time::{Duration, Instant},
};

const QUERY: &str = "query { bos { hostname uptime { since durationS } } }";
const SUBSCRIPTION_SCHEMA: &str = r#"query {
    sample: __type(name: "BosminerInfoSample") { ...Fields }
    summary: __type(name: "SummaryInfo") { ...Fields }
    solver: __type(name: "WorkSolverInfo") { ...Fields }
    temperature: __type(name: "TempCtrlInfo") { ...Fields }
} fragment Fields on __Type {
    name fields { name type { name kind ofType { name kind ofType { name kind } } } }
}"#;
const DOM: &str = "return {url:location.href,title:document.title,ready:document.readyState,login:!!document.querySelector('#login-username'),body:document.body?.innerText?.slice(0,16384),resources:performance.getEntriesByType('resource').map(r=>({name:r.name,initiatorType:r.initiatorType,transferSize:r.transferSize})).slice(-128)}";

fn value(value: Value) -> Value {
    value.get("value").cloned().unwrap_or(value)
}

fn element(value: Value) -> Result<String> {
    value["element-6066-11e4-a52e-4f735466cecf"]
        .as_str()
        .context("native browser element absent")
        .map(str::to_owned)
}

fn stage(evidence: &mut Value, phase: &str, start: Instant) {
    let record = json!({"origin":evidence["origin"],"phase":phase,"elapsed_milliseconds":start.elapsed().as_millis(),
        "diagnostic_only":true});
    evidence["last_stage"] = record.clone();
    println!("MD_SIM_BOSER_BROWSER_PROGRESS {record}");
}

fn graph_response(session: &mut Session, target: &Target, query: &str) -> Result<Value> {
    // Record only timing and response metadata. This excludes query text,
    // credentials and cookies. Console receipts survive a blocked page task.
    let response = value(session.execute(
        r#"const sent=Date.now(),identity=arguments[1];
        const trace=(phase,extra={})=>console.info('MD_SIM_GRAPH_PHASE',JSON.stringify({schema:'md-sim-browser-graph-phase/1',query_sha256:identity,phase,at:Date.now(),...extra}));
        trace('fetch-start');
        return fetch('/graphql',{method:'POST',credentials:'same-origin',cache:'no-store',redirect:'error',headers:{'Content-Type':'application/json'},body:JSON.stringify({query:arguments[0]})}).then(async r=>{
          trace('headers',{status:r.status});const text=await r.text();trace('body',{bytes:text.length});
          let body=null,parse_error=null;if(text.length<=262144){try{body=JSON.parse(text)}catch(e){parse_error=String(e)}}else{parse_error='body bound'}
          return {status:r.status,url:r.url,selected_run:r.headers.get('x-md-sim-run'),connection:r.headers.get('connection'),body,raw_body:text.slice(0,262144),raw_length:text.length,parse_error,browser_sent_ms:sent,browser_received_ms:Date.now()}
        }).catch(error=>{trace('failed',{name:String(error.name).slice(0,64)});throw error})"#,
        vec![json!(query), json!(md_sim_firmware::prepare::hash_bytes(query.as_bytes()))],
    )?);
    target.check_response("/graphql", &response)?;
    Ok(response)
}

fn check_graph(response: &Value) -> Result<()> {
    ensure!(
        response["status"] == 200,
        "native GraphQL HTTP failure: {response}"
    );
    ensure!(
        response["parse_error"].is_null() && response["body"].is_object(),
        "native GraphQL JSON failure: {response}"
    );
    ensure!(
        response["body"]["errors"].is_null(),
        "native GraphQL error: {response}"
    );
    Ok(())
}

fn graph(session: &mut Session, target: &Target, query: &str) -> Result<Value> {
    let response = graph_response(session, target, query)?;
    check_graph(&response)?;
    Ok(response)
}

fn retry_unsubmitted_session_check(response: &Value, attempt: usize) -> bool {
    attempt == 0
        && response["status"] == 408
        && response["raw_length"] == 0
        && response["raw_body"] == ""
        && response["connection"]
            .as_str()
            .is_some_and(|value| value.eq_ignore_ascii_case("close"))
}

fn rejected_session(session: &mut Session, target: &Target, evidence: &mut Value) -> Result<Value> {
    // Run076 packet stream266 proves the native server sent408 and FIN before
    // the browser sent any HTTP bytes on an idle speculative connection.
    // RFC9110 section15.5.9 permits a new request after408. Retry once only
    // for this unauthenticated session check, within its existing30s bound.
    // Keep both responses; a408 is never an authentication or test pass.
    evidence["session_rejection_attempts"] = json!([]);
    let deadline = Instant::now() + Duration::from_secs(30);
    for attempt in 0..2 {
        let remaining = deadline
            .saturating_duration_since(Instant::now())
            .as_millis();
        ensure!(remaining > 0, "unauthenticated session check deadline");
        session.set_script_timeout(u32::try_from(remaining)?)?;
        let response = graph_response(
            session,
            target,
            "mutation { auth { extend { __typename ... on Error { message } } } }",
        )?;
        evidence["session_rejection_attempts"]
            .as_array_mut()
            .unwrap()
            .push(response.clone());
        if retry_unsubmitted_session_check(&response, attempt) {
            continue;
        }
        check_graph(&response)?;
        return Ok(response);
    }
    unreachable!("the second response cannot retry")
}

#[cfg(test)]
mod graph_tests {
    use super::*;

    #[test]
    fn only_one_empty_408_close_response_can_retry_the_session_check() {
        let response = json!({"status":408,"raw_length":0,"raw_body":"","connection":"close"});
        assert!(retry_unsubmitted_session_check(&response, 0));
        assert!(!retry_unsubmitted_session_check(&response, 1));
        assert!(check_graph(&response).is_err());
        for status in [0, 200, 400, 401, 403, 429, 500, 502, 503, 504] {
            let mut changed = response.clone();
            changed["status"] = json!(status);
            assert!(!retry_unsubmitted_session_check(&changed, 0));
        }
        for (field, value) in [
            ("raw_length", json!(1)),
            ("raw_body", json!("x")),
            ("connection", json!("keep-alive")),
            ("connection", Value::Null),
        ] {
            let mut changed = response.clone();
            changed[field] = value;
            assert!(!retry_unsubmitted_session_check(&changed, 0));
        }
        assert!(
            check_graph(&json!({"status":200,"parse_error":"invalid JSON","body":null})).is_err()
        );
        assert!(check_graph(
            &json!({"status":200,"parse_error":null,"body":{"errors":["failure"]}})
        )
        .is_err());
    }
}

fn asset(
    session: &mut Session,
    target: &Target,
    path: &str,
    expected: &str,
    size: usize,
) -> Result<Value> {
    let response = value(session.execute(
        "return fetch(arguments[0],{cache:'no-store',redirect:'error'}).then(async r=>({status:r.status,url:r.url,selected_run:r.headers.get('x-md-sim-run'),text:await r.text()}))",
        vec![json!(path)],
    )?);
    target.check_response(path, &response)?;
    ensure!(response["status"] == 200, "native asset HTTP failure");
    // This exact archived JavaScript member is UTF-8. Returning its text
    // avoids hundreds of thousands of JSON numbers over browser control.
    // Its complete byte length and SHA-256 must still match the archive.
    let bytes = response["text"]
        .as_str()
        .context("native asset text absent")?
        .as_bytes();
    let digest = md_sim_firmware::prepare::hash_bytes(bytes);
    ensure!(
        bytes.len() == size && digest == expected,
        "native asset differs from archive"
    );
    Ok(json!({"path":path,"url":response["url"],"sha256":digest,"size":bytes.len()}))
}

fn dashboard(
    session: &mut Session,
    target: &Target,
    evidence: &mut Value,
    authenticated: Sender<()>,
    selected_clock_policy: Option<&Value>,
) -> Result<()> {
    let start = Instant::now();
    if target.selected_run.is_none() {
        let input = pool_input::read(Path::new("/private-pool.json"))?;
        stage(evidence, "waiting-for-http-probe", start);
        // The parent emits pool readiness only after Firefox has started. Thus
        // this support-tool wait starts before the selected miner starts.
        loop {
            if fs::read("/work/boser-http-ready").is_ok_and(|bytes| bytes == b"passed\n")
                && TcpStream::connect_timeout(&"10.0.2.15:80".parse()?, Duration::from_millis(100))
                    .is_ok()
            {
                break;
            }
            ensure!(
                start.elapsed() < Duration::from_secs(input.seconds),
                "native HTTP readiness deadline"
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        evidence["http_preflight"] =
            serde_json::to_value(hash_file(Path::new("/work/boser-http-ready"))?)?;
        evidence["http_preflight_milliseconds"] = json!(start.elapsed().as_millis());
    }
    session.set_timeout(Duration::from_secs(105))?;
    session.set_page_timeout(90_000)?;
    session.set_script_timeout(30_000)?;
    stage(evidence, "native-navigation-started", start);
    session.navigate(&target.origin)?;
    stage(evidence, "native-navigation-returned", start);
    let render = Instant::now();
    loop {
        evidence["initial_dom"] = value(session.execute(DOM, vec![])?);
        if evidence["initial_dom"]["login"] == true {
            break;
        }
        ensure!(
            render.elapsed() < Duration::from_secs(30),
            "native login form absent"
        );
        std::thread::sleep(Duration::from_millis(250));
    }
    evidence["cookies_before_login"] = value(session.cookies()?);
    stage(evidence, "native-login-form-present", start);
    warm_subscription(session, evidence)?;
    let username = element(value(session.find("#login-username")?))?;
    let password = element(value(session.find("#login-password")?))?;
    session.clear(&username)?;
    session.send_keys(&username, "root")?;
    session.clear(&password)?;
    let submit = element(value(session.execute(
        "return Array.from(document.querySelectorAll('button')).find(e=>e.textContent.trim()==='Log In')",
        vec![],
    )?))?;
    session.click(&submit)?;
    stage(evidence, "native-login-click-returned", start);
    let login = Instant::now();
    loop {
        let page = value(session.execute(DOM, vec![])?);
        evidence["authenticated_dom"] = page.clone();
        if page["login"] == false
            && page["ready"] == "complete"
            && page["body"].as_str().is_some_and(|body| {
                body.contains("Hash Boards")
                    && body.contains("Pool Status")
                    && body.contains("System Status")
            })
        {
            ensure!(
                page["url"]
                    .as_str()
                    .is_some_and(|url| url.starts_with(&format!("{}/", target.origin))),
                "native login left selected origin"
            );
            break;
        }
        ensure!(
            login.elapsed() < Duration::from_secs(60),
            "native login deadline"
        );
        std::thread::sleep(Duration::from_millis(250));
    }
    evidence["cookies_after_login"] = value(session.cookies()?);
    stage(evidence, "native-dashboard-rendered", start);
    ensure!(
        evidence["cookies_after_login"]
            .as_array()
            .is_some_and(|cookies| cookies.iter().any(|cookie| cookie["name"] == "session_id")),
        "native session cookie absent"
    );
    evidence["session_extension"] = graph(
        session,
        target,
        "mutation { auth { extend { __typename ... on Error { message } } } }",
    )?;
    ensure!(
        evidence["session_extension"]["body"]["data"]["auth"]["extend"]["__typename"]
            == "VoidResult",
        "native browser session was not authenticated"
    );
    // A fast support browser can reach Boser before the low-rate miner has
    // returned its first solution. Use the existing native DOM readiness
    // check before sampling. It requires real accepted work and keeps its
    // original 60-second bound; the WebSocket assertions stay unchanged.
    stage(evidence, "native-dom-readiness-started", start);
    native_view(session, evidence)?;
    stage(evidence, "native-dom-ready", start);
    subscription(session, target, evidence, start)?;
    evidence["asset"] = asset(
        session,
        target,
        "/app.00964ffb2140d6973b03.js",
        "1c318483df06a03e460e35f7419d8562c007f5dc4965a20d412c96392ad36423",
        425409,
    )?;
    stage(evidence, "native-asset-verified", start);
    let first = graph(session, target, QUERY)?;
    evidence["telemetry"] = json!([first]);
    let telemetry_deadline = Instant::now() + Duration::from_secs(30);
    loop {
        ensure!(
            Instant::now() < telemetry_deadline
                && evidence["telemetry"].as_array().unwrap().len() < 16,
            "native uptime observation deadline"
        );
        std::thread::sleep(Duration::from_secs(2));
        let next = graph(session, target, QUERY)?;
        evidence["telemetry"].as_array_mut().unwrap().push(next);
        ensure!(
            Instant::now() <= telemetry_deadline,
            "native uptime observation deadline"
        );
        let samples = evidence["telemetry"].as_array().unwrap();
        if md_sim_firmware::s9_telemetry::uptime_progress(
            &samples[0]["body"]["data"]["bos"],
            &samples.last().unwrap()["body"]["data"]["bos"],
        )? {
            break;
        }
    }
    let clock_policy = if let Some(policy) = selected_clock_policy {
        Some(policy.clone())
    } else if target.selected_run.is_none() && Path::new("/clock-policy.json").exists() {
        ensure!(
            fs::metadata("/clock-policy.json")?.len() <= 4096,
            "clock policy exceeds bound"
        );
        let policy: Value = serde_json::from_slice(&fs::read("/clock-policy.json")?)?;
        ensure!(
            policy["schema"] == "md-sim-private-wall-clock/1"
                && policy["epoch_unix_seconds"].as_u64().is_some(),
            "invalid clock policy"
        );
        Some(policy)
    } else {
        None
    };
    if let Some(policy) = clock_policy {
        evidence["clock_policy"] = policy.clone();
        browser_target::verify_clock_observation(evidence, &policy)?;
    }
    let first = &evidence["telemetry"][0]["body"]["data"]["bos"];
    ensure!(
        evidence["telemetry"]
            .as_array()
            .unwrap()
            .iter()
            .all(
                |sample| sample["body"]["data"]["bos"]["uptime"]["durationS"]
                    .as_u64()
                    .is_some()
            ),
        "native browser whole-second uptime absent"
    );
    let second = &evidence["telemetry"].as_array().unwrap().last().unwrap()["body"]["data"]["bos"];
    ensure!(
        md_sim_firmware::s9_telemetry::uptime_progress(first, second)?,
        "native uptime did not advance"
    );
    // Run 469 stalled a GraphQL fetch while the second profile loaded the
    // native application. Keep that independent navigation after this
    // profile's checks, with the first authenticated session still live.
    stage(evidence, "native-primary-checks-complete", start);
    authenticated
        .send(())
        .context("isolation browser stopped before authenticated session")?;
    evidence["elapsed_milliseconds"] = json!(start.elapsed().as_millis());
    Ok(())
}

fn native_view(session: &mut Session, evidence: &mut Value) -> Result<()> {
    let start = Instant::now();
    loop {
        let view = value(session.execute(r#"return {url:location.href,
            body:document.body.innerText.slice(0,32768),
            tables:Array.from(document.querySelectorAll('table')).map(table=>({
                headers:Array.from(table.querySelectorAll('thead th')).map(e=>e.innerText.replace(/\s/g,'')),
                rows:Array.from(table.querySelectorAll('tbody tr')).slice(0,32).map(row=>
                    Array.from(row.querySelectorAll('td')).map(e=>e.innerText.trim()))
            })).slice(0,16)}"#, vec![])?);
        evidence["native_view"] = view.clone();
        let tables = view["tables"]
            .as_array()
            .context("native UI table observations absent")?;
        let boards = tables
            .iter()
            .filter_map(|table| {
                let headers = table["headers"].as_array()?;
                let chips = headers.iter().position(|header| header == "ASIC#")?;
                let frequency = headers.iter().position(|header| header == "Frequency")?;
                let rows = table["rows"].as_array()?;
                Some(
                    rows.iter()
                        .filter(|row| {
                            row[chips] == "63"
                                && row[frequency].as_str().is_some_and(|text| {
                                    text.split_whitespace().next() == Some("100")
                                })
                        })
                        .count(),
                )
            })
            .sum::<usize>();
        let pools = tables
            .iter()
            .filter_map(|table| {
                let headers = table["headers"].as_array()?;
                let accepted = headers.iter().position(|header| header == "Accepted")?;
                let rows = table["rows"].as_array()?;
                Some(
                    rows.iter()
                        .filter(|row| {
                            row.as_array().is_some_and(|cells| {
                                cells.iter().any(|cell| {
                                    cell.as_str()
                                        .is_some_and(|text| text.contains("10.0.2.100:3333"))
                                })
                            }) && row[accepted]
                                .as_str()
                                .and_then(|text| text.replace(',', "").parse::<u64>().ok())
                                .is_some_and(|n| n > 0)
                        })
                        .count(),
                )
            })
            .sum::<usize>();
        if boards == 3 && pools == 1 {
            evidence["native_view_verified"] = json!({"boards":boards,"chips_per_board":63,
                "frequency_mhz":100,"private_pool_rows_with_accepted_work":pools,
                "source":"native application DOM; test did not fill or replace telemetry"});
            evidence["native_view_observation"] = json!({"elapsed_milliseconds":start.elapsed().as_millis(),
                "deadline_seconds":60,"before_websocket_sampling":true});
            return Ok(());
        }
        ensure!(
            start.elapsed() < Duration::from_secs(60),
            "native page did not render the selected board and pool telemetry"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn warm_subscription(session: &mut Session, evidence: &mut Value) -> Result<()> {
    // Worker startup is a support-tool cost. Start it during the login page,
    // before the native service's bounded WebSocket transaction begins.
    // This creates no connection and does not change the native application.
    evidence["websocket_worker_startup"] = value(session.execute(
        r#"if(window.__mdSimBoserProbe)throw new Error('probe already exists');
        function probe(){self.onmessage=({data:input})=>{
          const events=[],lifecycle=[]; let done=false;
          const socket=new WebSocket(input.url,'graphql-transport-ws');
          const finish=reason=>{if(done)return;done=true;clearTimeout(timer);try{socket.send(JSON.stringify({id:'probe',type:'complete'}))}catch(e){} socket.close(1000,'test complete');self.postMessage({reason,protocol:socket.protocol,events,lifecycle,worker:true})};
          const timer=setTimeout(()=>finish('deadline'),20000);
          socket.onopen=()=>{lifecycle.push({type:'open',at:Date.now()});socket.send(JSON.stringify({type:'connection_init',payload:{headers:{'Accept-Language':input.language}}}))};
          socket.onmessage=e=>{let message;try{message=JSON.parse(e.data)}catch(error){finish('invalid JSON');return}events.push(message);lifecycle.push({type:message.type,at:Date.now()});if(events.length>16){finish('message bound');return}
            if(message.type==='connection_ack')socket.send(JSON.stringify({id:'probe',type:'subscribe',payload:{query:input.query}}));
            if(message.type==='ping')socket.send(JSON.stringify({type:'pong',payload:message.payload}));
            if(events.filter(e=>e.type==='next').length===2)finish('two samples');
            if(message.type==='error')finish('server error');};
          socket.onerror=()=>{lifecycle.push({type:'error',readyState:socket.readyState});finish('socket error')};socket.onclose=e=>{lifecycle.push({type:'close',code:e.code,reason:e.reason,wasClean:e.wasClean});finish('closed')};
        };self.postMessage({kind:'ready',at:Date.now()})}
        const blob=URL.createObjectURL(new Blob(['('+probe.toString()+')()'],{type:'application/javascript'}));
        const worker=new Worker(blob),state={createdAt:Date.now(),readyAt:null,error:null,pending:null,used:false};
        worker.onmessage=event=>{if(event.data.kind==='ready')state.readyAt=event.data.at;else if(state.pending)state.pending(event.data)};
        worker.onerror=event=>{state.error=event.message;if(state.pending)state.pending({reason:'worker error',message:event.message,worker:true})};
        window.__mdSimBoserProbe={state,run:query=>{
          if(state.used)throw new Error('probe already used');state.used=true;
          if(state.readyAt===null||state.error){worker.terminate();URL.revokeObjectURL(blob);return {reason:'worker not ready',startup:state,worker:true}}
          return new Promise(resolve=>{
            let done=false;
            const finish=result=>{if(done)return;done=true;clearTimeout(timer);worker.terminate();URL.revokeObjectURL(blob);state.pending=null;resolve({...result,startup:{createdAt:state.createdAt,readyAt:state.readyAt}})};
            const timer=setTimeout(()=>finish({reason:'worker delivery deadline',worker:true}),25000);
            state.pending=finish;
            worker.postMessage({query,url:(location.protocol==='https:'?'wss://':'ws://')+location.host+'/graphql',language:window.i18n?.locale??'en'});
          })
        }};
        return {createdAt:state.createdAt,networkStarted:false}"#,
        vec![],
    )?);
    ensure!(
        evidence["websocket_worker_startup"]["networkStarted"] == false,
        "native subscription worker warmup differs"
    );
    Ok(())
}

fn subscription(
    session: &mut Session,
    target: &Target,
    evidence: &mut Value,
    start: Instant,
) -> Result<()> {
    // This is the protocol present in the pinned native vendors asset.
    // The browser itself performs the WebSocket handshake and framing.
    evidence["subscription_schema"] = graph(session, target, SUBSCRIPTION_SCHEMA)?;
    stage(evidence, "native-websocket-started", start);
    let websocket = value(session.execute(
        "if(!window.__mdSimBoserProbe)throw new Error('probe missing after login');return window.__mdSimBoserProbe.run(arguments[0])",
        vec![json!(native_telemetry::QUERY)],
    )?);
    evidence["websocket"] = websocket.clone();
    ensure!(
        websocket["protocol"] == "graphql-transport-ws" && websocket["reason"] == "two samples",
        "native subscription failed: {websocket}"
    );
    ensure!(
        websocket["events"]
            .as_array()
            .context("subscription events absent")?
            .iter()
            .filter(|event| event["type"] == "next")
            .all(|event| event["payload"]["errors"].is_null()
                && event["payload"]["data"]["bosminerInfo"]["__typename"] == "BosminerInfoSample"),
        "native subscription returned unavailable/error data"
    );
    let samples: Vec<_> = websocket["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["type"] == "next")
        .map(|event| &event["payload"]["data"]["bosminerInfo"]["timestamp"])
        .collect();
    ensure!(
        samples.len() == 2
            && !samples[0].is_null()
            && !samples[1].is_null()
            && samples[0] != samples[1],
        "native subscription did not deliver two different sample timestamps"
    );
    let observations = websocket["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|event| event["type"] == "next")
        .map(|event| native_telemetry::assess(&event["payload"]["data"]["bosminerInfo"]))
        .collect::<Result<Vec<_>>>()?;
    evidence["live_mining_telemetry"] = json!(observations);
    Ok(())
}

fn isolated_profile(
    mut session: Session,
    target: Target,
    authenticated: Receiver<()>,
) -> (Session, Value) {
    let mut evidence = json!({"origin":target.origin,"profile":"/work/browser/profile-1"});
    let result = (|| -> Result<()> {
        // The same origin must already have an authenticated session in the
        // first profile. Different ports would not establish cookie isolation.
        authenticated.recv_timeout(Duration::from_secs(230))?;
        let started = Instant::now();
        stage(&mut evidence, "isolation-navigation-started", started);
        session.set_timeout(Duration::from_secs(105))?;
        session.set_page_timeout(90_000)?;
        session.set_script_timeout(30_000)?;
        evidence["cookies_before_navigation"] = value(session.cookies()?);
        ensure!(
            evidence["cookies_before_navigation"] == json!([]),
            "second profile inherited cookies"
        );
        session.navigate(&target.origin)?;
        let start = Instant::now();
        loop {
            evidence["dom"] = value(session.execute(DOM, vec![])?);
            if evidence["dom"]["login"] == true {
                break;
            }
            ensure!(
                start.elapsed() < Duration::from_secs(30),
                "isolated profile did not show native login"
            );
            std::thread::sleep(Duration::from_millis(250));
        }
        evidence["cookies_at_selected_origin"] = value(session.cookies()?);
        ensure!(
            evidence["dom"]["url"]
                .as_str()
                .is_some_and(|url| url.starts_with(&format!("{}/", target.origin))),
            "isolated profile left the selected origin"
        );
        ensure!(
            evidence["cookies_at_selected_origin"] == json!([]),
            "same-origin authentication crossed profiles"
        );
        evidence["session_rejection"] = rejected_session(&mut session, &target, &mut evidence)?;
        ensure!(
            evidence["session_rejection"]["body"]["data"]["auth"]["extend"]["__typename"]
                == "AuthError",
            "native service authenticated the second fresh profile"
        );
        stage(&mut evidence, "isolation-session-rejected", started);
        Ok(())
    })();
    evidence["passed"] = json!(result.is_ok());
    evidence["error"] = json!(result.as_ref().err().map(|e| format!("{e:#}")));
    let _ = session.set_timeout(Duration::from_secs(5));
    if result.is_err() {
        if let Ok(page) = session.execute(DOM, vec![]) {
            evidence["failure_dom"] = value(page);
        }
    }
    (session, evidence)
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let long_boot = args.len() == 2 && args[1] == "--selected-long-boot";
    let startup_only = args.len() == 2 && args[1] == "--selected-startup";
    let selected = long_boot || startup_only || (args.len() == 2 && args[1] == "--selected");
    ensure!(
        args.len() == 1 || selected,
        "browser helper accepts only --selected, --selected-long-boot or --selected-startup"
    );
    let mut groups = browser_environment::root_setup(
        if selected {
            &["client0", "lo"]
        } else {
            &["eth0", "lo"]
        },
        2,
    )?;
    if selected {
        browser_groups::install_termination_cleanup(&groups)?;
        browser_selected::copy_credentials()?;
    }
    browser_environment::drop_privileges()?;
    let credentials = browser_selected::prepare_profiles(selected)?;
    // Native run250 rendered its login form while normal navigation still
    // waited for load completion. Interact after DOMContentLoaded, then keep
    // the existing complete authenticated page and exact asset assertions.
    let (mut browser, mut session, startup) =
        browser_driver::launch_interactive(0, groups.remove(0))?;
    let (mut other_browser, mut other_session, other_startup) =
        browser_driver::launch_interactive(1, groups.remove(0))?;
    if startup_only {
        return startup_check(
            [&mut browser, &mut other_browser],
            [&mut session, &mut other_session],
            [startup, other_startup],
            credentials,
        );
    }
    let initial_profiles = if selected {
        vec![
            fresh_profile_state(&mut session, 0)?,
            fresh_profile_state(&mut other_session, 1)?,
        ]
    } else {
        vec![]
    };
    let mut evidence = json!({"schema":"md-sim-original-s9-browser/2",
        "verification_order":"native-dom-accepted-work-then-websocket-then-fresh-profile-isolation",
        "selection_wait_seconds":if selected {Some(if long_boot {4900} else {2500})} else {None},
        "startup":startup,"isolation_startup":other_startup,"helper":hash_file(&std::env::current_exe()?)?,
        "source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_browser_check.rs")),
        "separate_uid":browser_groups::UID,"fresh_profile":true,"credentials":credentials,
        "scope":"Actual selected native UI, archived assets, live HTTP and WebSocket telemetry, isolated profiles. Restart persistence and complete dashboard acceptance require separate evidence."});
    println!(
        "MD_SIM_BOSER_BROWSER_READY {}",
        json!({"schema":"md-sim-browser-prepared/1","selected":selected,
            "origin":if selected {None} else {Some("http://10.0.2.15")},
            "helper":evidence["helper"],"startup":evidence["startup"],
            "isolation_startup":evidence["isolation_startup"],"profiles":initial_profiles,
            "firmware_gate":false,"native_dashboard_gate":false})
    );
    let mut result = (|| -> Result<()> {
        let (targets, clock_policy) = if selected {
            let selection = browser_selected::wait_selection(long_boot)?;
            let targets = selection.targets()?;
            let clock_policy = selection.clock_policy()?;
            evidence["selection"] = serde_json::to_value(selection)?;
            (targets, clock_policy)
        } else {
            (vec![Target::reference()], None)
        };
        evidence["origins"] = json!([]);
        for target in targets {
            let (authenticated, receiver) = std::sync::mpsc::channel();
            let other_target = target.clone();
            let isolation =
                std::thread::spawn(move || isolated_profile(other_session, other_target, receiver));
            let mut observation = json!({"origin":target.origin,"target":target});
            let mut checked = dashboard(
                &mut session,
                &target,
                &mut observation,
                authenticated,
                clock_policy.as_ref(),
            );
            session.set_timeout(Duration::from_secs(5))?;
            if checked.is_err() {
                if let Ok(page) = session.execute(DOM, vec![]) {
                    observation["failure_dom"] = value(page);
                }
                other_browser.terminate()?;
            }
            let (returned, isolated) = isolation
                .join()
                .map_err(|_| anyhow::anyhow!("isolation browser thread panicked"))?;
            other_session = returned;
            observation["isolation"] = isolated;
            if checked.is_ok() && observation["isolation"]["passed"] != true {
                checked = Err(anyhow::anyhow!(
                    "native profile isolation failed: {}",
                    observation["isolation"]["error"]
                ));
            }
            if checked.is_ok() {
                checked = (|| -> Result<()> {
                    session.set_timeout(Duration::from_secs(30))?;
                    observation["session_after_isolation"] = graph(
                        &mut session,
                        &target,
                        "mutation { auth { extend { __typename ... on Error { message } } } }",
                    )?;
                    ensure!(
                        observation["session_after_isolation"]["body"]["data"]["auth"]["extend"]
                            ["__typename"]
                            == "VoidResult",
                        "first profile lost its authenticated session during isolation test"
                    );
                    Ok(())
                })();
            }
            observation["passed"] = json!(checked.is_ok());
            observation["error"] = json!(checked.as_ref().err().map(|e| format!("{e:#}")));
            evidence["origins"]
                .as_array_mut()
                .unwrap()
                .push(observation);
            checked?;
        }
        other_session.set_timeout(Duration::from_secs(5))?;
        evidence["isolation_session_close"] = json!(other_session.close().ok());
        Ok(())
    })();
    session.set_timeout(Duration::from_secs(5))?;
    evidence["session_close"] = json!(session.close().ok());
    browser.terminate()?;
    other_browser.terminate()?;
    evidence["cleanup"] = browser.stop()?;
    evidence["isolation_cleanup"] = other_browser.stop()?;
    if evidence["cleanup"]["empty"] != true || evidence["isolation_cleanup"]["empty"] != true {
        result = Err(anyhow::anyhow!("native browser descendants remain"));
    }
    evidence["passed"] = json!(result.is_ok());
    evidence["error"] = json!(result.as_ref().err().map(|e| format!("{e:#}")));
    // Preserve the existing single-origin report contract for the local
    // support-VM cycle. Selected execution retains all origins separately.
    if !selected {
        if let Some(first) = evidence["origins"]
            .as_array()
            .and_then(|a| a.first())
            .cloned()
        {
            for (key, value) in first.as_object().context("browser observation absent")? {
                if !["passed", "error"].contains(&key.as_str()) {
                    evidence[key] = value.clone();
                }
            }
        }
        evidence["schema"] = json!("md-sim-original-s9-browser/1");
    }
    for name in [
        "0-stdout.log",
        "0-stderr.log",
        "1-stdout.log",
        "1-stderr.log",
    ] {
        let path = Path::new("/work/browser").join(name);
        if fs::metadata(&path).is_ok() {
            evidence[name] = browser_driver::bounded_log(&path)?;
        }
    }
    save_json(Path::new("/work/browser/result.json"), &evidence)?;
    println!("MD_SIM_BOSER_BROWSER_RESULT {evidence}");
    result
}

fn fresh_profile_state(session: &mut Session, index: usize) -> Result<Value> {
    let cookies = value(session.cookies()?);
    let page = value(session.execute(
        "return {url:location.href,title:document.title,ready:document.readyState}",
        vec![],
    )?);
    ensure!(
        cookies == json!([]) && page["url"] == "about:blank" && page["ready"] == "complete",
        "fresh startup profile state differs"
    );
    Ok(json!({"index":index,"cookies":cookies,"page":page}))
}

/// This explicit component mode uses the selected client's complete startup
/// path. It never selects a miner, contacts a dashboard, or grants those gates.
fn startup_check(
    browsers: [&mut browser_driver::Browser; 2],
    sessions: [&mut Session; 2],
    startup: [Value; 2],
    credentials: Value,
) -> Result<()> {
    let mut report = json!({"schema":"md-sim-selected-browser-startup/1",
        "firmware_gate":false,"native_dashboard_gate":false,
        "helper":hash_file(&std::env::current_exe()?)?,
        "firefox":hash_file(Path::new("/usr/lib/firefox/firefox"))?,
        "libxul":hash_file(Path::new("/usr/lib/firefox/libxul.so"))?,
        "startup":startup,"credentials":credentials,"profiles":[],"cleanup":[],
        "status":fs::read_to_string("/proc/self/status")?,
        "limits":{"startup_seconds":browser_driver::STARTUP_SECONDS,
            "new_session_seconds":browser_driver::NEW_SESSION_SECONDS,
            "cpu_seconds":browser_driver::CPU_SECONDS,
            "address_space_bytes":browser_driver::ADDRESS_SPACE_BYTES},
        "scope":"Actual selected-client Firefox startup and empty fresh profiles only; no firmware or dashboard request."});
    let mut result = (|| -> Result<()> {
        ensure!(
            report["credentials"]
                .as_array()
                .is_some_and(|a| a.len() == 2),
            "selected startup requires two verified credential profiles"
        );
        for (index, session) in sessions.into_iter().enumerate() {
            report["profiles"]
                .as_array_mut()
                .unwrap()
                .push(fresh_profile_state(session, index)?);
            session.close()?;
        }
        Ok(())
    })();
    for browser in browsers {
        match browser.stop() {
            Ok(cleanup) => {
                if cleanup["empty"] != true {
                    result = Err(anyhow::anyhow!("startup browser descendants remain"));
                }
                report["cleanup"].as_array_mut().unwrap().push(cleanup);
            }
            Err(error) => {
                result = Err(error);
            }
        }
    }
    for name in [
        "0-stdout.log",
        "0-stderr.log",
        "1-stdout.log",
        "1-stderr.log",
    ] {
        report[name] = browser_driver::bounded_log(&Path::new("/work/browser").join(name))?;
    }
    report["passed"] = json!(result.is_ok());
    report["error"] = json!(result.as_ref().err().map(|error| format!("{error:#}")));
    save_json(Path::new("/work/browser/startup-result.json"), &report)?;
    println!("MD_SIM_SELECTED_BROWSER_STARTUP {report}");
    result
}

fn main() -> Result<()> {
    let result = run();
    if let Err(error) = &result {
        let mut failure = json!({"error":format!("{error:#}")});
        for name in [
            "0-stdout.log",
            "0-stderr.log",
            "1-stdout.log",
            "1-stderr.log",
            "start-failure.json",
        ] {
            let path = Path::new("/work/browser").join(name);
            if let Ok(log) = browser_driver::bounded_log(&path) {
                failure[name] = log;
            }
        }
        println!("MD_SIM_BOSER_BROWSER_FAILURE {failure}");
    }
    result
}

//! Native archived S9 services, reached from the separate client namespace.
use super::*;
use std::collections::BTreeSet;
#[path = "native_reboot.rs"]
mod reboot;
#[path = "native_restart.rs"]
mod restart;
#[allow(dead_code)] // Failure collection needs capture; other drivers also format.
#[path = "../support/s9_sd_image.rs"]
mod sd;
#[path = "native_storage.rs"]
mod storage;
const ACTIVE_UHTTPD_PIDS: &str = "for p in $(pidof uhttpd); do if read n c s rest < /proc/$p/stat; then [ \"$s\" = Z ] || echo \"$p\"; fi; done";

pub(super) fn retain_failure_storage(events: &mut Vec<Value>) -> Result<()> {
    let root = Path::new("/work/server-state/native");
    let initial = root.join("storage-initial.json");
    if initial.exists() {
        let bytes = read_evidence(&initial, 3 * 1024 * 1024)?;
        events.push(json!({"native_failure_initial_storage":
            serde_json::from_slice::<Value>(&bytes)?}));
    }
    let disk = root.join("persistent.img");
    if disk.exists() {
        events.push(json!({"native_failure_storage_snapshot":sd::capture(&disk,"failed-run")?,
            "coherence":"Diagnostic bytes after the failed worker was stopped. No filesystem consistency or orderly shutdown claim."}));
    }
    Ok(())
}

fn active_pids(value: &Value) -> Result<BTreeSet<u32>> {
    let text = value
        .as_str()
        .context("native active PID response absent")?;
    let mut pids = BTreeSet::new();
    for part in text.split_whitespace() {
        let pid = part.parse::<u32>()?;
        ensure!(
            pid > 0 && pids.insert(pid),
            "native active PID invalid or duplicated"
        );
    }
    ensure!(!pids.is_empty(), "native service has no active PID");
    Ok(pids)
}
fn verify_uptime(
    before: &Value,
    after: &Value,
    summary_before: &Value,
    summary_after: &Value,
) -> Result<()> {
    let number = |value: &Value| -> Result<f64> {
        // Check integer magnitude before conversion can round 2^53 + 1 down.
        if let Some(integer) = value.as_u64() {
            ensure!(
                integer <= 1u64 << 53,
                "native uptime outside exact numeric range"
            );
        }
        let number = value.as_f64().context("native uptime is not numeric")?;
        ensure!(
            number.is_finite() && (0.0..=9007199254740992.0).contains(&number),
            "native uptime outside exact numeric range"
        );
        Ok(number)
    };
    let (before, after) = (number(before)?, number(after)?);
    ensure!(
        after > before && number(summary_before)? == before && number(summary_after)? == after,
        "native telemetry did not advance or summary differs"
    );
    Ok(())
}

pub(super) struct Native {
    child: Owned,
    identity: ObjectId,
    manifest: Value,
}
pub(super) fn launch(events: &mut Vec<Value>) -> Result<Native> {
    let log = fs::File::create("/work/native-driver.log")?;
    let mut command = Command::new("/s9-native-network");
    let cpu_seconds = if browser_mode() {
        native_browser_budget::SERVICE_CPU_SECONDS
    } else {
        420
    };
    command
        .uid(1000)
        .gid(1000)
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    unsafe {
        command.pre_exec(move || {
            for (resource, limit) in [
                (libc::RLIMIT_CPU, cpu_seconds),
                (libc::RLIMIT_AS, 3 * 1024 * 1024 * 1024),
                (libc::RLIMIT_FSIZE, 64 * 1024 * 1024),
                (libc::RLIMIT_NPROC, 128),
            ] {
                if libc::setrlimit(
                    resource,
                    &libc::rlimit {
                        rlim_cur: limit,
                        rlim_max: limit,
                    },
                ) != 0
                {
                    return Err(std::io::Error::last_os_error());
                }
            }
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = Owned(command.spawn()?);
    let deadline = Instant::now() + Duration::from_secs(200);
    while !Path::new("/worker/native/ready.json").exists() {
        ensure!(
            child.0.try_wait()?.is_none(),
            "native runner exited before service readiness"
        );
        ensure!(
            Instant::now() < deadline,
            "native service readiness deadline"
        );
        thread::sleep(Duration::from_millis(100));
    }
    let ready: Value = serde_json::from_slice(&fs::read("/worker/native/ready.json")?)?;
    let run = hash_file(Path::new("/worker/native/run.json"))?;
    ensure!(
        serde_json::from_value::<ObjectId>(ready["run_manifest"].clone())? == run,
        "native readiness refers to another run manifest"
    );
    let backend: Value = serde_json::from_slice(&fs::read("/worker/native/run.json")?)?;
    let manifest = json!({"schema":"md-sim-selected-native-dashboard-run/1","captured_execution":identity(),"backend_manifest":backend,"backend_manifest_identity":run,"worker_uid":1000,"worker_network":"private in-guest veth without default route","gateway_ip":WORKER,"upstream":"http://127.0.0.1:18080/","native_service":"actual archived uhttpd and LuCI","original_board_boot_gate":false,"mining_gate":false,"exact_hardware_target":null});
    fs::write(
        "/work/native-run.json",
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    let identity = hash_file(Path::new("/work/native-run.json"))?;
    events.push(credentials::create(
        Path::new("/usr/bin/openssl"),
        Path::new("/work/native-credentials"),
        WORKER.parse()?,
        &identity.sha256,
    )?);
    events.push(json!({"native_ready":ready,"selected_native_execution":identity,"selected_native_manifest":manifest}));
    Ok(Native {
        child,
        identity,
        manifest,
    })
}
impl Native {
    pub(super) fn configure_gateway(&self, spec: &mut GatewaySpec) -> Result<()> {
        spec.run_id = self.identity.sha256.clone();
        spec.execution_identity = self.identity.clone();
        spec.dashboard_source = "adapted-native".into();
        spec.certificate = "/work/native-credentials/server.crt".into();
        spec.private_key = "/work/native-credentials/server.key".into();
        spec.client_ca = "/work/native-credentials/ca.crt".into();
        spec.browser_context_id = hex::encode(Sha256::digest(format!(
            "native-browser:{}",
            self.identity.sha256
        )));
        Ok(())
    }
    pub(super) fn publish_endpoint(&self, gateway: &Gateway) -> Result<()> {
        fs::write(
            "/work/native-endpoint.json",
            serde_json::to_vec_pretty(
                &json!({"run_id":self.identity.sha256,"execution_identity":self.identity,"run_manifest":self.manifest,"gateway":gateway.manifest()?}),
            )?,
        )?;
        Ok(())
    }
    pub(super) fn stop(&mut self, events: &mut Vec<Value>) -> Result<()> {
        ensure!(
            unsafe { libc::kill(self.child.0.id() as libc::pid_t, libc::SIGTERM) } == 0,
            "cannot stop owned native runner"
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = self.child.0.try_wait()? {
                ensure!(status.success(), "native runner stop failed");
                break;
            }
            ensure!(Instant::now() < deadline, "native runner stop deadline");
            thread::sleep(Duration::from_millis(20));
        }
        events.push(json!({"native_runner_stopped":true,"selected_run":self.identity.sha256}));
        for name in ["storage-initial", "storage-final", "lifecycle"] {
            let bytes = read_evidence(
                &Path::new("/worker/native").join(format!("{name}.json")),
                3 * 1024 * 1024,
            )?;
            events.push(json!({"native_storage_record":name,
                "record":serde_json::from_slice::<Value>(&bytes)?}));
        }
        Ok(())
    }
}
struct Http {
    base: String,
    cookie: String,
    sequence: u64,
    tls: bool,
    observations: Vec<Value>,
}
impl Http {
    fn persist(&self) -> Result<()> {
        let path = Path::new(&self.cookie)
            .parent()
            .context("cookie context absent")?
            .join("observations.json");
        let bytes = serde_json::to_vec_pretty(
            &json!({"base":self.base,"tls":self.tls,"observations":self.observations}),
        )?;
        ensure!(
            bytes.len() <= 256 * 1024,
            "native observation evidence exceeds bound"
        );
        fs::write(path, bytes)?;
        Ok(())
    }
    fn new(base: &str, name: &str, tls: bool) -> Result<Self> {
        fs::create_dir_all(format!("/work/{name}"))?;
        Ok(Self {
            base: base.into(),
            cookie: format!("/work/{name}/cookies"),
            sequence: 0,
            tls,
            observations: Vec::new(),
        })
    }
    fn request(
        &mut self,
        path: &str,
        body: Option<&str>,
        content_type: &str,
    ) -> Result<(u16, Vec<u8>, String)> {
        self.request_inner(path, body, content_type, false)?
            .context("unexpected transport timeout")
    }
    fn reboot_probe(&mut self) -> Result<Option<(u16, Vec<u8>, String)>> {
        self.request_inner("/cgi-bin/luci/", None, "", true)
    }
    fn request_inner(
        &mut self,
        path: &str,
        body: Option<&str>,
        content_type: &str,
        allow_reboot_timeout: bool,
    ) -> Result<Option<(u16, Vec<u8>, String)>> {
        self.sequence += 1;
        let prefix = format!(
            "{}/http-{}",
            Path::new(&self.cookie)
                .parent()
                .context("cookie context missing")?
                .display(),
            self.sequence
        );
        let mut command = Command::new("/usr/bin/curl");
        command.env_clear().args([
            "--noproxy",
            "*",
            "--silent",
            "--show-error",
            "--max-time",
            "20",
            "--max-filesize",
            "1048576",
            "--cookie",
            &self.cookie,
            "--cookie-jar",
            &self.cookie,
            "--dump-header",
            &format!("{prefix}.headers"),
            "--output",
            &format!("{prefix}.body"),
            "--write-out",
            "%{http_code}",
        ]);
        if self.tls {
            command.args([
                "--cacert",
                "/credentials/ca.crt",
                "--cert",
                "/credentials/client.crt",
                "--key",
                "/credentials/client.key",
            ]);
        }
        if let Some(body) = body {
            fs::write(format!("{prefix}.request"), body)?;
            command.args([
                "--header",
                &format!("Content-Type: {content_type}"),
                "--data-binary",
                &format!("@{prefix}.request"),
            ]);
        }
        unsafe {
            command.pre_exec(|| {
                for (resource, limit) in [
                    (libc::RLIMIT_FSIZE, 1024 * 1024),
                    (libc::RLIMIT_CPU, 5),
                    (libc::RLIMIT_AS, 256 * 1024 * 1024),
                ] {
                    if libc::setrlimit(
                        resource,
                        &libc::rlimit {
                            rlim_cur: limit,
                            rlim_max: limit,
                        },
                    ) != 0
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let status = command
            .arg(format!("{}{path}", self.base))
            .stdin(Stdio::null())
            .stdout(fs::File::create(format!("{prefix}.status"))?)
            .stderr(fs::File::create(format!("{prefix}.stderr"))?)
            .status()?;
        let read = |extension: &str, cap: u64| -> Result<Vec<u8>> {
            let mut bytes = Vec::new();
            fs::File::open(format!("{prefix}.{extension}"))?
                .take(cap + 1)
                .read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() as u64 <= cap,
                "native HTTP {extension} exceeds bound"
            );
            Ok(bytes)
        };
        let stderr = read("stderr", 65536)?;
        if allow_reboot_timeout && status.code() == Some(28) {
            // The gateway can retain a connection while the guest resets.
            // A transport timeout is not an HTTP response or readiness.
            self.observations.push(json!({"path":path,"tls":self.tls,
                "transport_error":"timeout","curl_exit_code":28,
                "stderr_hex":hex::encode(&stderr),
                "partial_http_status":String::from_utf8(read("status",16)?)?}));
            self.persist()?;
            return Ok(None);
        }
        ensure!(
            status.success(),
            "native HTTP request failed: {}",
            String::from_utf8_lossy(&stderr)
        );
        let code = std::str::from_utf8(&read("status", 16)?)?.parse::<u16>()?;
        let bytes = read("body", 1024 * 1024)?;
        let headers = String::from_utf8(read("headers", 65536)?)?;
        self.observations.push(json!({"path":path.split('?').next(),"status":code,"bytes":bytes.len(),"body_sha256":hex::encode(Sha256::digest(&bytes)),"tls":self.tls}));
        self.persist()?;
        Ok(Some((code, bytes, headers)))
    }
    fn rpc(&mut self, session: &str, service: &str, method: &str, params: Value) -> Result<Value> {
        let id = self.sequence + 1;
        let payload = json!({"id":id,"method":method,"params":params}).to_string();
        let (code, body, _) = self.request(
            &format!("/cgi-bin/luci/rpc/{service}?auth={session}"),
            Some(&payload),
            "application/json",
        )?;
        ensure!(code == 200, "native RPC HTTP status {code}");
        let parsed: Value = serde_json::from_slice(&body)?;
        ensure!(
            parsed["id"] == id && parsed["error"].is_null(),
            "native RPC {service}.{method} failed: {parsed}"
        );
        self.observations.push(json!({"rpc_service":service,"rpc_method":method,"request_id":id,"parameters":params,"response":parsed,"response_body_hex":hex::encode(&body)}));
        self.persist()?;
        Ok(parsed["result"].clone())
    }
    fn login(&mut self) -> Result<String> {
        let (code, page, _) = self.request("/cgi-bin/luci/", None, "")?;
        ensure!(
            code == 403 && std::str::from_utf8(&page)?.contains("luci_password"),
            "native login form missing"
        );
        let (code, _, headers) = self.request(
            "/cgi-bin/luci/",
            Some("luci_username=root&luci_password="),
            "application/x-www-form-urlencoded",
        )?;
        ensure!(
            code == 302
                && headers
                    .to_ascii_lowercase()
                    .contains("set-cookie: sysauth="),
            "native login redirect/cookie missing"
        );
        let (code, body, _) = self.request("/cgi-bin/luci/admin/status/overview", None, "")?;
        ensure!(
            code == 200 && body.len() > 1000,
            "authenticated native overview absent"
        );
        let payload = json!({"id":77,"method":"login","params":["root",""]}).to_string();
        let (code, body, _) =
            self.request("/cgi-bin/luci/rpc/auth", Some(&payload), "application/json")?;
        ensure!(code == 200, "native RPC login failed");
        let response: Value = serde_json::from_slice(&body)?;
        response["result"]
            .as_str()
            .map(str::to_string)
            .context("native session ID missing")
    }
}

pub(super) fn retain_http_evidence(events: &mut Vec<Value>) -> Result<()> {
    let mut total = 0usize;
    for context in [
        "native-tunnel-browser",
        "native-tls-browser",
        "native-separate-browser",
        "native-restart-probe",
        "native-reboot-before",
        "native-reboot-after",
        "native-reboot-probe",
    ] {
        let path = Path::new("/work").join(context);
        if !path.exists() {
            continue;
        }
        let mut files = fs::read_dir(path)?
            .map(|entry| entry.map(|e| e.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        files.sort();
        // Forty reboot probes retain body, headers, status and diagnostics.
        ensure!(files.len() <= 256, "native HTTP file count exceeds bound");
        for file in files {
            ensure!(
                fs::symlink_metadata(&file)?.is_file(),
                "unexpected native HTTP evidence entry"
            );
            let mut bytes = Vec::new();
            fs::File::open(&file)?
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            total = total
                .checked_add(bytes.len())
                .context("native evidence size overflow")?;
            ensure!(
                total <= 1024 * 1024,
                "native HTTP evidence exceeds export bound"
            );
            events.push(json!({"native_http_file":file,"hex":hex::encode(&bytes),"identity":{"sha256":hex::encode(Sha256::digest(&bytes)),"size":bytes.len()}}));
        }
    }
    Ok(())
}
pub(super) fn client_checks(events: &mut Vec<Value>) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while TcpStream::connect(format!("{WORKER}:2222")).is_err()
        || TcpStream::connect(format!("{WORKER}:8443")).is_err()
    {
        ensure!(
            Instant::now() < deadline,
            "native SSH/TLS endpoint readiness deadline"
        );
        thread::sleep(Duration::from_millis(50));
    }
    ensure!(
        !Path::new("/worker/native").exists()
            && !Path::new("/s9").exists()
            && !Path::new("/credentials/server.key").exists(),
        "native client can access worker private storage"
    );
    let selected: Value = serde_json::from_slice(&fs::read("/native-endpoint.json")?)?;
    let identity: ObjectId = serde_json::from_value(selected["execution_identity"].clone())?;
    let bytes = serde_json::to_vec_pretty(&selected["run_manifest"])?;
    ensure!(
        identity.size == bytes.len() as u64
            && identity.sha256 == hex::encode(Sha256::digest(&bytes))
            && selected["run_id"] == identity.sha256,
        "selected native endpoint identity differs"
    );
    let mut tunnel = SshTunnel::start(
        ssh_config(),
        "127.0.0.1:18081".parse()?,
        "127.0.0.1:18080".parse()?,
        Path::new("/work/native-tunnel.log"),
    )?;
    let mut local = Http::new("http://127.0.0.1:18081", "native-tunnel-browser", false)?;
    let tunnel_session = local.login()?;
    let tunnel_uptime = local.rpc(&tunnel_session, "sys", "uptime", json!([]))?;
    ensure!(
        tunnel_uptime.as_f64().is_some(),
        "native tunnel telemetry missing"
    );
    let (code, asset, _) = local.request("/luci-static/bootstrap/cascade.css", None, "")?;
    ensure!(
        code == 200
            && hex::encode(Sha256::digest(&asset))
                == "8204c85c4ebe203985adc4b63aa0fd717ce12ffc16f78c4eeb9890063501c477",
        "native tunnel asset differs from archived bytes"
    );
    events.push(json!({"actual_native_ssh_tunnel":true,"selected_run":identity.sha256,"execution_identity":identity,"endpoint":selected,"transport":tunnel.evidence(),"http":local.observations,"live_uptime":tunnel_uptime,"native_asset_sha256":hex::encode(Sha256::digest(&asset))}));
    tunnel.stop()?;
    let unauth = curl("/cgi-bin/luci/", &["--fail"], false)?;
    ensure!(
        !unauth.status.success(),
        "native gateway accepted missing client certificate"
    );
    let mut published = Http::new(
        &format!("https://{WORKER}:8443"),
        "native-tls-browser",
        true,
    )?;
    let session = published.login()?;
    let (code, asset, headers) =
        published.request("/luci-static/bootstrap/cascade.css", None, "")?;
    ensure!(
        code == 200
            && hex::encode(Sha256::digest(&asset))
                == "8204c85c4ebe203985adc4b63aa0fd717ce12ffc16f78c4eeb9890063501c477",
        "native TLS asset differs from archive"
    );
    let ids = headers
        .lines()
        .filter(|line| line.to_ascii_lowercase().starts_with("x-md-sim-run:"))
        .collect::<Vec<_>>();
    ensure!(
        ids.len() == 1 && ids[0].split_whitespace().nth(1) == Some(identity.sha256.as_str()),
        "native publication selected another run"
    );
    let first = published
        .rpc(&session, "sys", "uptime", json!([]))?
        .as_f64()
        .context("native uptime absent")?;
    thread::sleep(Duration::from_secs(2));
    let second = published
        .rpc(&session, "sys", "uptime", json!([]))?
        .as_f64()
        .context("native uptime absent")?;
    ensure!(second > first, "native live uptime did not advance");
    let original = published
        .rpc(&session, "fs", "readfile", json!(["/etc/config/uhttpd"]))?
        .as_str()
        .context("native config bytes absent")?
        .to_string();
    let previous = published.rpc(
        &session,
        "uci",
        "get",
        json!(["uhttpd", "main", "max_connections"]),
    )?;
    ensure!(previous == "100", "native baseline config differs");
    ensure!(
        published.rpc(
            &session,
            "uci",
            "set",
            json!(["uhttpd", "main", "max_connections", "101"])
        )? == true,
        "native configuration set failed"
    );
    ensure!(
        published.rpc(&session, "uci", "commit", json!(["uhttpd"]))? == true,
        "native configuration commit failed"
    );
    ensure!(
        published.rpc(
            &session,
            "uci",
            "get",
            json!(["uhttpd", "main", "max_connections"])
        )? == "101",
        "native config readback failed"
    );
    let before = published.rpc(&session, "sys", "exec", json!([ACTIVE_UHTTPD_PIDS]))?;
    published.rpc(&session,"sys","exec",json!(["(sleep 1; killall uhttpd; /usr/sbin/uhttpd -f -h /www -p 0.0.0.0:8000 -x /cgi-bin -u /ubus) </dev/null >/tmp/native-remote-restart.log 2>&1 &"]))?;
    let mut restart_client = Http::new(&published.base, "native-restart-probe", true)?;
    fs::copy(&published.cookie, &restart_client.cookie)?;
    let restart_observation = restart::wait_ready(&mut restart_client, &session, &before)?;
    let after = published.rpc(&session, "sys", "exec", json!([ACTIVE_UHTTPD_PIDS]))?;
    restart::verify(&restart_observation, &before, &after)?;
    ensure!(
        active_pids(&before)?.is_disjoint(&active_pids(&after)?),
        "native service restart retained an active old PID"
    );
    ensure!(
        published.rpc(
            &session,
            "uci",
            "get",
            json!(["uhttpd", "main", "max_connections"])
        )? == "101",
        "native restart lost committed config"
    );
    ensure!(
        published.rpc(
            &session,
            "fs",
            "writefile",
            json!(["/etc/config/uhttpd", original])
        )? == 1,
        "native exact config restore failed"
    );
    ensure!(
        published.rpc(&session, "fs", "readfile", json!(["/etc/config/uhttpd"]))? == original,
        "native restored config bytes differ"
    );
    ensure!(
        published.rpc(
            &session,
            "uci",
            "get",
            json!(["uhttpd", "main", "max_connections"])
        )? == previous,
        "native restored config value differs"
    );
    let mut other = Http::new(
        &format!("https://{WORKER}:8443"),
        "native-separate-browser",
        true,
    )?;
    let (code, page, _) = other.request("/cgi-bin/luci/admin/status/overview", None, "")?;
    ensure!(
        code == 403 && std::str::from_utf8(&page)?.contains("luci_password"),
        "new isolated cookie jar inherited native authentication"
    );
    events.push(json!({"actual_native_tls_dashboard":true,"selected_run":identity.sha256,"execution_identity":identity,"http":published.observations,"separate_cookie_http":other.observations,"native_asset_sha256":hex::encode(Sha256::digest(&asset)),"uptime_before":first,"uptime_after":second,"configuration_before":"100","configuration_written":"101","configuration_after_service_restart":"101","configuration_restored":"100","original_config_rpc_encoding":"base64","original_config_rpc_sha256":hex::encode(Sha256::digest(original.as_bytes())),"exact_restoration":true,"service_pid_before":before,"service_pid_after":after,"service_restart":restart_observation,"isolated_cookie_context_rejected":true,"no_certificate_rejected":true,"browser_javascript_execution":false,"full_guest_reboot_persistence":false,"original_board_gate":false,"mining_gate":false,"headers":headers}));
    record_progress("native-guest-reboot")?;
    reboot::check(&identity, events)?;
    Ok(())
}

fn one<'a>(events: &'a [Value], key: &str) -> Result<&'a Value> {
    let matches = events
        .iter()
        .filter(|event| !event[key].is_null())
        .collect::<Vec<_>>();
    ensure!(matches.len() == 1, "missing or duplicate native {key}");
    Ok(matches[0])
}
fn matches_identity(value: &Value, identity: &ObjectId, newline: bool) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    if newline {
        bytes.push(b'\n');
    }
    ensure!(
        identity.size == bytes.len() as u64
            && identity.sha256 == hex::encode(Sha256::digest(bytes)),
        "native manifest identity differs"
    );
    Ok(())
}
fn verify_http(events: &[Value], tls: bool) -> Result<Vec<&Value>> {
    let http = events
        .iter()
        .filter(|event| event.get("path").is_some())
        .collect::<Vec<_>>();
    ensure!(http.len() >= 6, "native HTTP observations incomplete");
    for (event, path, status) in [
        (http[0], "/cgi-bin/luci/", 403),
        (http[1], "/cgi-bin/luci/", 302),
        (http[2], "/cgi-bin/luci/admin/status/overview", 200),
        (http[3], "/cgi-bin/luci/rpc/auth", 200),
    ] {
        ensure!(
            event["path"] == path && event["status"] == status && event["tls"] == tls,
            "native login sequence differs"
        );
    }
    ensure!(
        http[2]["bytes"].as_u64().is_some_and(|n| n > 1000),
        "native overview response absent"
    );
    let assets = http
        .iter()
        .filter(|e| e["path"] == "/luci-static/bootstrap/cascade.css")
        .collect::<Vec<_>>();
    ensure!(
        assets.len() == 1
            && assets[0]["status"] == 200
            && assets[0]["body_sha256"]
                == "8204c85c4ebe203985adc4b63aa0fd717ce12ffc16f78c4eeb9890063501c477",
        "native asset response differs"
    );
    let mut rpc = Vec::new();
    for (index, event) in events.iter().enumerate() {
        if event.get("rpc_service").is_none() {
            continue;
        }
        ensure!(index > 0, "RPC lacks HTTP response");
        let response = hex::decode(
            event["response_body_hex"]
                .as_str()
                .context("RPC raw body absent")?,
        )?;
        let preceding = &events[index - 1];
        ensure!(
            preceding["status"] == 200
                && preceding["tls"] == tls
                && preceding["body_sha256"] == hex::encode(Sha256::digest(&response))
                && preceding["bytes"] == response.len(),
            "RPC bytes differ from captured HTTP response"
        );
        ensure!(
            preceding["path"]
                == format!(
                    "/cgi-bin/luci/rpc/{}",
                    event["rpc_service"]
                        .as_str()
                        .context("RPC service absent")?
                ),
            "RPC service differs from response path"
        );
        let parsed: Value = serde_json::from_slice(&response)?;
        ensure!(
            parsed == event["response"]
                && parsed["id"] == event["request_id"]
                && parsed["error"].is_null(),
            "native RPC response identity differs"
        );
        rpc.push(event);
    }
    Ok(rpc)
}

pub(super) fn verify_report(
    client: &[Value],
    server: &[Value],
    captured: &Value,
    component: &ObjectId,
) -> Result<ObjectId> {
    let tunnel = one(client, "actual_native_ssh_tunnel")?;
    let tls = one(client, "actual_native_tls_dashboard")?;
    ensure!(
        tunnel["actual_native_ssh_tunnel"] == true && tls["actual_native_tls_dashboard"] == true,
        "native endpoint checks failed"
    );
    let identity: ObjectId = serde_json::from_value(tunnel["execution_identity"].clone())?;
    reboot::verify(one(client, "native_guest_reboot")?, &identity)?;
    let endpoint = &tunnel["endpoint"];
    ensure!(
        endpoint["execution_identity"] == tunnel["execution_identity"]
            && endpoint["run_id"] == identity.sha256
            && tunnel["selected_run"] == identity.sha256
            && tls["selected_run"] == identity.sha256
            && tls["execution_identity"] == tunnel["execution_identity"],
        "native endpoints select different runs"
    );
    let manifest = &endpoint["run_manifest"];
    matches_identity(manifest, &identity, false)?;
    ensure!(
        serde_json::from_value::<ObjectId>(manifest["captured_execution"].clone())? == *component,
        "native run belongs to another captured environment"
    );
    let ready = one(server, "selected_native_execution")?;
    ensure!(
        ready["selected_native_execution"] == tunnel["execution_identity"]
            && ready["selected_native_manifest"] == *manifest,
        "native server and client identities differ"
    );
    let backend = &manifest["backend_manifest"];
    let backend_identity: ObjectId =
        serde_json::from_value(manifest["backend_manifest_identity"].clone())?;
    matches_identity(backend, &backend_identity, true)?;
    storage::verify(server, backend, &backend_identity, captured)?;
    ensure!(
        ready["native_ready"]["run_manifest"] == manifest["backend_manifest_identity"],
        "readiness refers to another backend"
    );
    ensure!(
        backend["firmware_artifact"] == "braiins-feed-25bba3b4a505ccbc0e54"
            && backend["archive_sha256"]
                == "469743ebcd5faf3445e4f4209ad8af58cc7adeff37c7df28af3c53a5b255b56e"
            && backend["firmware_identity"] == "original-bytes"
            && backend["dashboard_source"] == "adapted-native",
        "native archived firmware identity differs"
    );
    for (name, object) in backend["inputs"]
        .as_object()
        .context("native components absent")?
    {
        ensure!(
            captured["inputs"][format!("/s9/{name}")] == *object,
            "native component was not the captured input"
        );
    }
    ensure!(
        backend["inputs"].as_object().is_some_and(|m| m.len() == 3)
            && backend["qemu"] == captured["inputs"]["/usr/bin/qemu-system-arm"]
            && backend["driver"] == captured["inputs"]["/s9-native-network"],
        "native backend closure differs"
    );
    let gateway = &endpoint["gateway"];
    let gateways = server
        .iter()
        .filter(|e| e["schema"] == "md-sim-dashboard-endpoint/1")
        .collect::<Vec<_>>();
    ensure!(
        gateways.len() == 1
            && gateways[0] == gateway
            && gateway["run_id"] == identity.sha256
            && gateway["execution_identity"] == tunnel["execution_identity"]
            && gateway["dashboard_source"] == "adapted-native"
            && gateway["url"] == format!("https://{WORKER}:8443/")
            && gateway["upstream"] == "http://127.0.0.1:18080"
            && gateway["client_access_rules"] == json!([format!("{CLIENT}/32")]),
        "native published gateway differs from selected endpoint"
    );
    ensure!(
        gateway["nginx"] == captured["inputs"]["/usr/bin/nginx"]
            && gateway["openssl"] == captured["inputs"]["/usr/bin/openssl"],
        "native gateway programs differ"
    );
    let headers = tls["headers"]
        .as_str()
        .context("native response headers absent")?;
    let ids = headers
        .lines()
        .filter(|line| line.to_ascii_lowercase().starts_with("x-md-sim-run:"))
        .collect::<Vec<_>>();
    ensure!(
        ids.len() == 1 && ids[0].split_whitespace().nth(1) == Some(identity.sha256.as_str()),
        "native response selected another run"
    );
    let stopped = one(server, "native_runner_stopped")?;
    ensure!(
        stopped["native_runner_stopped"] == true && stopped["selected_run"] == identity.sha256,
        "selected native runner was not stopped"
    );
    let local = verify_http(
        tunnel["http"]
            .as_array()
            .context("native tunnel HTTP absent")?,
        false,
    )?;
    ensure!(
        local.len() == 1
            && local[0]["rpc_service"] == "sys"
            && local[0]["rpc_method"] == "uptime"
            && local[0]["response"]["result"] == tunnel["live_uptime"]
            && tunnel["live_uptime"].as_f64().is_some(),
        "native tunnel telemetry absent"
    );
    let rpc = verify_http(
        tls["http"].as_array().context("native TLS HTTP absent")?,
        true,
    )?;
    ensure!(
        rpc.len() == 14,
        "native configuration/restart sequence incomplete"
    );
    let original = rpc[2]["response"]["result"]
        .as_str()
        .context("native original config absent")?;
    let expected = [
        ("sys", "uptime", json!([])), ("sys", "uptime", json!([])),
        ("fs", "readfile", json!(["/etc/config/uhttpd"])),
        ("uci", "get", json!(["uhttpd", "main", "max_connections"])),
        ("uci", "set", json!(["uhttpd", "main", "max_connections", "101"])),
        ("uci", "commit", json!(["uhttpd"])),
        ("uci", "get", json!(["uhttpd", "main", "max_connections"])),
        ("sys", "exec", json!([ACTIVE_UHTTPD_PIDS])),
        ("sys", "exec", json!(["(sleep 1; killall uhttpd; /usr/sbin/uhttpd -f -h /www -p 0.0.0.0:8000 -x /cgi-bin -u /ubus) </dev/null >/tmp/native-remote-restart.log 2>&1 &"])),
        ("sys", "exec", json!([ACTIVE_UHTTPD_PIDS])),
        ("uci", "get", json!(["uhttpd", "main", "max_connections"])),
        ("fs", "writefile", json!(["/etc/config/uhttpd", original])),
        ("fs", "readfile", json!(["/etc/config/uhttpd"])),
        ("uci", "get", json!(["uhttpd", "main", "max_connections"])),
    ];
    for (observed, (service, method, params)) in rpc.iter().zip(expected) {
        ensure!(
            observed["rpc_service"] == service
                && observed["rpc_method"] == method
                && observed["parameters"] == params,
            "native RPC command sequence differs"
        );
    }
    let result = |n: usize| &rpc[n]["response"]["result"];
    verify_uptime(
        result(0),
        result(1),
        &tls["uptime_before"],
        &tls["uptime_after"],
    )?;
    for (n, expected) in [
        (3, json!("100")),
        (4, json!(true)),
        (5, json!(true)),
        (6, json!("101")),
        (10, json!("101")),
        (11, json!(1)),
        (12, json!(original)),
        (13, json!("100")),
    ] {
        ensure!(
            *result(n) == expected,
            "native configuration outcome differs at {n}"
        );
    }
    ensure!(
        active_pids(result(7))?.is_disjoint(&active_pids(result(9))?)
            && tls["service_pid_before"] == *result(7)
            && tls["service_pid_after"] == *result(9),
        "native service restart not observed"
    );
    restart::verify(&tls["service_restart"], result(7), result(9))?;
    ensure!(
        tls["original_config_rpc_encoding"] == "base64"
            && tls["original_config_rpc_sha256"]
                == hex::encode(Sha256::digest(original.as_bytes()))
            && tls["exact_restoration"] == true
            && tls["no_certificate_rejected"] == true
            && tls["isolated_cookie_context_rejected"] == true,
        "native restoration or authentication checks absent"
    );
    let separate = tls["separate_cookie_http"]
        .as_array()
        .context("separate cookie context absent")?;
    ensure!(
        separate.len() == 1
            && separate[0]["path"] == "/cgi-bin/luci/admin/status/overview"
            && separate[0]["status"] == 403
            && separate[0]["tls"] == true,
        "separate cookie context inherited authentication"
    );
    for field in [
        "browser_javascript_execution",
        "full_guest_reboot_persistence",
        "original_board_gate",
        "mining_gate",
    ] {
        ensure!(
            tls[field] == false,
            "unsupported native gate claimed: {field}"
        );
    }
    Ok(identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_uptime_and_active_pid_checks_use_observed_values() -> Result<()> {
        verify_uptime(&json!(234), &json!(241), &json!(234.0), &json!(241.0))?;
        assert!(verify_uptime(&json!(234), &json!(234), &json!(234.0), &json!(234.0)).is_err());
        assert!(verify_uptime(&json!(234), &json!(241), &json!(234.0), &json!(242.0)).is_err());
        assert!(verify_uptime(&json!("234"), &json!(241), &json!(234), &json!(241)).is_err());
        let maximum = 1u64 << 53;
        assert!(verify_uptime(
            &json!(maximum - 2),
            &json!(maximum + 1),
            &json!(maximum - 2),
            &json!(maximum as f64),
        )
        .is_err());
        assert!(verify_uptime(
            &json!(maximum - 2),
            &json!(maximum),
            &json!(maximum - 2),
            &json!(maximum + 1),
        )
        .is_err());
        assert!(active_pids(&json!("789\n"))?.is_disjoint(&active_pids(&json!("815\n"))?));
        assert!(!active_pids(&json!("789\n"))?.is_disjoint(&active_pids(&json!("815 789\n"))?));
        for invalid in ["", "0", "4 4", "-1", "unknown"] {
            assert!(active_pids(&json!(invalid)).is_err());
        }
        Ok(())
    }

    #[test]
    fn backend_and_selected_manifest_use_their_actual_serialization() -> Result<()> {
        let value = json!({"input":"exact"});
        let mut raw = serde_json::to_vec_pretty(&value)?;
        let selected = ObjectId {
            sha256: hex::encode(Sha256::digest(&raw)),
            size: raw.len() as u64,
        };
        raw.push(b'\n');
        let saved = ObjectId {
            sha256: hex::encode(Sha256::digest(&raw)),
            size: raw.len() as u64,
        };
        matches_identity(&value, &selected, false)?;
        matches_identity(&value, &saved, true)?;
        assert!(matches_identity(&value, &selected, true).is_err());
        assert!(matches_identity(&value, &saved, false).is_err());
        Ok(())
    }

    #[test]
    fn rpc_verifier_binds_raw_http_bytes_and_rejects_reordered_evidence() -> Result<()> {
        let mut evidence = vec![
            json!({"path":"/cgi-bin/luci/","status":403,"tls":true}),
            json!({"path":"/cgi-bin/luci/","status":302,"tls":true}),
            json!({"path":"/cgi-bin/luci/admin/status/overview","status":200,"tls":true,"bytes":4000}),
            json!({"path":"/cgi-bin/luci/rpc/auth","status":200,"tls":true}),
            json!({"path":"/luci-static/bootstrap/cascade.css","status":200,"tls":true,"body_sha256":"8204c85c4ebe203985adc4b63aa0fd717ce12ffc16f78c4eeb9890063501c477"}),
        ];
        let body = br#"{"id":6,"result":32.5,"error":null}"#;
        evidence.push(json!({"path":"/cgi-bin/luci/rpc/sys","status":200,"tls":true,"bytes":body.len(),"body_sha256":hex::encode(Sha256::digest(body))}));
        evidence.push(json!({"rpc_service":"sys","rpc_method":"uptime","request_id":6,"parameters":[],"response":serde_json::from_slice::<Value>(body)?,"response_body_hex":hex::encode(body)}));
        assert_eq!(verify_http(&evidence, true)?.len(), 1);
        let mut bad = evidence.clone();
        bad[6]["response"]["result"] = json!(99);
        assert!(verify_http(&bad, true).is_err());
        let mut bad = evidence.clone();
        bad[5]["body_sha256"] = json!("00".repeat(32));
        assert!(verify_http(&bad, true).is_err());
        evidence.swap(5, 6);
        assert!(verify_http(&evidence, true).is_err());
        Ok(())
    }
}

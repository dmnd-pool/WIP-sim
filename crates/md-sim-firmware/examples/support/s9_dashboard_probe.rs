//! Native2022 Boser HTTP probe from the separate private Linux VM.
//! Curl handles HTTP framing. This is not a browser or a full dashboard suite.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::store::{hash_file, ObjectId};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    net::TcpStream,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use dashboard_state::{Action, Plan};
use md_sim_firmware::s9_dashboard_state as dashboard_state;
#[path = "s9_luci_probe.rs"]
mod luci;
#[path = "s9_miner_maintenance.rs"]
mod maintenance;
use md_sim_firmware::s9_update_validation as update_validation;

pub const MARKER: &str = "MD_SIM_DASHBOARD_PROBE ";
const ROOT: &str = "/work/dashboard";
const ASSET: &str = "/app.00964ffb2140d6973b03.js";
const ASSET_SHA: &str = "1c318483df06a03e460e35f7419d8562c007f5dc4965a20d412c96392ad36423";
const AUTH: &str = "mutation ($username: String!, $password: String!) { auth { login(username: $username, password: $password) { __typename ... on Error { message } } } }";
const UPTIME: &str = "query { bos { hostname info { version { full } } uptime { since durationS } } storage { public { webTheme dashboardDensity preferredLanguage } } }";
const STORAGE: &str =
    "query { storage { public { webTheme dashboardDensity preferredLanguage } } }";
const UPDATE_STORAGE: &str = "mutation ($input: StorageIn!) { storage { update(input: $input) { __typename ... on StorageData { public { webTheme dashboardDensity preferredLanguage } } ... on IoError { message exception } } } }";
// Read the native contract before implementing writes or subscriptions. The
// archived frontend gives the operations, but does not define null handling.
const SCHEMA: &str = "query { __schema { queryType { name } mutationType { name fields { name type { name kind ofType { name kind } } } } subscriptionType { name fields { name type { name kind ofType { name kind } } } } types { name kind inputFields { name type { name kind ofType { name kind } } } } } }";

fn read(path: &str, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "native HTTP evidence bound");
    Ok(bytes)
}

fn object(bytes: &[u8]) -> ObjectId {
    ObjectId {
        sha256: md_sim_firmware::prepare::hash_bytes(bytes),
        size: bytes.len() as u64,
    }
}

struct Client {
    deadline: Instant,
    observations: Vec<Value>,
    request_limit: usize,
    request_seconds: u64,
    origin: String,
    root: String,
    endpoint: std::net::SocketAddr,
    tls: Option<TlsClient>,
}
/// Credentials stay on the separate client. The expected run comes from the
/// checked worker publication, not from the firmware's HTTP response.
#[allow(dead_code)] // Only selected worker probes use this contract.
pub struct TlsClient {
    pub ca: PathBuf,
    pub certificate: PathBuf,
    pub private_key: PathBuf,
    pub selected_run: String,
}
impl Client {
    fn request(&mut self, path: &str, request: Option<Value>) -> Result<Vec<u8>> {
        let bytes = request.as_ref().map(serde_json::to_vec).transpose()?;
        self.exchange(
            path,
            bytes.as_deref().map(|body| ("application/json", body)),
            request,
            &[200],
        )
    }

    fn exchange(
        &mut self,
        path: &str,
        body: Option<(&str, &[u8])>,
        request: Option<Value>,
        expected_statuses: &[u16],
    ) -> Result<Vec<u8>> {
        self.exchange_with_upload(path, body, None, request, expected_statuses)
    }

    fn update_upload(
        &mut self,
        path: &str,
        token: &str,
        action: update_validation::Action,
    ) -> Result<Vec<u8>> {
        let object = action.checked_image(Path::new(update_validation::GUEST_IMAGE))?;
        self.exchange_with_upload(
            path,
            None,
            Some(token),
            Some(json!({"kind":action.schema(),"image":object,
                "artifact_id":update_validation::ARTIFACT,"commit_requested":false})),
            &[200],
        )
    }

    fn exchange_with_upload(
        &mut self,
        path: &str,
        body: Option<(&str, &[u8])>,
        upload_token: Option<&str>,
        request: Option<Value>,
        expected_statuses: &[u16],
    ) -> Result<Vec<u8>> {
        ensure!(
            path.starts_with('/') && !path.contains(['\r', '\n', '?', '#']),
            "invalid fixed probe path"
        );
        let remaining = self
            .deadline
            .saturating_duration_since(Instant::now())
            .as_secs()
            .min(if upload_token.is_some() {
                update_validation::CLIENT_SECONDS
            } else {
                self.request_seconds
            });
        ensure!(
            remaining > 0 && self.observations.len() < self.request_limit,
            "dashboard probe budget"
        );
        let prefix = format!("{}/http-{}", self.root, self.observations.len());
        let started = Instant::now();
        let mut command = Command::new("/usr/bin/curl");
        command.env_clear().args([
            "--noproxy",
            "*",
            "--silent",
            "--show-error",
            "--proto",
            if self.tls.is_some() {
                "=https"
            } else {
                "=http"
            },
            "--connect-timeout",
            "2",
            "--max-time",
            &remaining.to_string(),
            "--max-filesize",
            "1048576",
            "--cookie",
            &format!("{}/cookies", self.root),
            "--cookie-jar",
            &format!("{}/cookies", self.root),
            "--dump-header",
            &format!("{prefix}.headers"),
            "--output",
            &format!("{prefix}.body"),
            "--write-out",
            "%{http_code}\n%{size_upload}\n%{size_download}\n%{time_total}\n",
        ]);
        if let Some(tls) = &self.tls {
            command
                .arg("--cacert")
                .arg(&tls.ca)
                .arg("--cert")
                .arg(&tls.certificate)
                .arg("--key")
                .arg(&tls.private_key);
        }
        if let Some((content_type, bytes)) = body {
            ensure!(bytes.len() <= 65536, "native request body bound");
            ensure!(
                !content_type.contains(['\r', '\n']),
                "invalid native request content type"
            );
            fs::write(format!("{prefix}.request"), bytes)?;
            command.args([
                "--header",
                &format!("Content-Type: {content_type}"),
                "--data-binary",
                &format!("@{prefix}.request"),
            ]);
        }
        if let Some(token) = upload_token {
            ensure!(
                body.is_none() && token.len() <= 32 && token.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid native upload token or body"
            );
            command.args([
                // Native browser FormData requests do not use Expect.
                // Run061 delivered all19MiB in3s but its100-continue path
                // never returned a final response. Retain that failure.
                "--header",
                "Expect:",
                "--form-string",
                &format!("token={token}"),
                "--form",
                &format!(
                    "image=@{};type=application/octet-stream",
                    update_validation::GUEST_IMAGE
                ),
                "--form-string",
                "keep=on",
            ]);
        }
        let status = command
            .arg(format!("{}{path}", self.origin))
            .stdin(Stdio::null())
            .stdout(fs::File::create_new(format!("{prefix}.status"))?)
            .stderr(fs::File::create_new(format!("{prefix}.stderr"))?)
            .status()?;
        let metrics = String::from_utf8(read(&format!("{prefix}.status"), 256)?)?;
        let fields: Vec<_> = metrics.lines().collect();
        ensure!(fields.len() == 4, "curl transfer metrics missing");
        let status_text = fields[0];
        let upload_bytes: u64 = fields[1].parse()?;
        let download_bytes: u64 = fields[2].parse()?;
        let transfer_seconds: f64 = fields[3].parse()?;
        ensure!(
            transfer_seconds.is_finite() && transfer_seconds >= 0.0,
            "invalid transfer duration"
        );
        let diagnostic = String::from_utf8(read(&format!("{prefix}.stderr"), 65536)?)?;
        let body = if Path::new(&format!("{prefix}.body")).exists() {
            read(&format!("{prefix}.body"), 1048576)?
        } else {
            Vec::new()
        };
        let headers = read(&format!("{prefix}.headers"), 65536)?;
        self.observations.push(
            json!({"path":path,"request":request,"curl_exit_code":status.code(),
            "elapsed_milliseconds":started.elapsed().as_millis(),
            "transfer":{"upload_bytes":upload_bytes,"download_bytes":download_bytes,
                "total_seconds":transfer_seconds,"expect_continue_disabled":upload_token.is_some()},
            "http_status":status_text,"stderr":diagnostic,"headers_hex":hex::encode(&headers),
            "body_hex":hex::encode(&body),"body":object(&body)}),
        );
        println!(
            "MD_SIM_DASHBOARD_STEP {}",
            json!({"index":self.observations.len()-1,
            "path":path,"status":status_text,"curl_exit_code":status.code(),
            "upload_bytes":upload_bytes,"download_bytes":download_bytes,
            "elapsed_milliseconds":started.elapsed().as_millis()})
        );
        ensure!(
            status.success() && expected_statuses.contains(&status_text.parse::<u16>()?),
            "native HTTP request failed: {path}, status{status_text}, curl{status}"
        );
        if let Some(tls) = &self.tls {
            let text = std::str::from_utf8(&headers)?;
            let ids: Vec<_> = text
                .lines()
                .filter_map(|line| line.split_once(':'))
                .filter(|(name, _)| name.eq_ignore_ascii_case("x-md-sim-run"))
                .map(|(_, value)| value.trim())
                .collect();
            ensure!(
                ids == [tls.selected_run.as_str()],
                "native response selected another run"
            );
        }
        Ok(body)
    }

    fn graphql(&mut self, query: &str, variables: Value) -> Result<Value> {
        let response: Value = serde_json::from_slice(&self.request(
            "/graphql",
            Some(json!({"query":query,"variables":variables})),
        )?)?;
        graphql_data(&response).cloned()
    }

    fn ready_index(&mut self) -> Result<Vec<u8>> {
        // An open TCP socket is not HTTP readiness. Original cycle005's
        // native Actix service paused accepts while its workers started.
        // Retry only a refused connection or transport timeout for this
        // initial idempotent GET. Never retry an operation or HTTP error.
        let deadline = Instant::now() + Duration::from_secs(40);
        for attempt in 0..4 {
            let before = self.observations.len();
            let result = self.request("/", None);
            if let Some(record) = self.observations.get_mut(before) {
                record["phase"] = json!("initial-http-readiness");
                record["attempt"] = json!(attempt);
            }
            match result {
                Ok(index) => return Ok(index),
                Err(error) => {
                    let retry = self.observations.last().is_some_and(readiness_retryable);
                    if !retry
                        || attempt == 3
                        || Instant::now() >= deadline
                        || self.deadline.saturating_duration_since(Instant::now())
                            <= Duration::from_secs(30)
                    {
                        return Err(error);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        unreachable!("finite readiness loop returns above")
    }

    fn check(&mut self, plan: Option<&Plan>, check_luci: bool) -> Result<Value> {
        loop {
            if TcpStream::connect_timeout(&self.endpoint, Duration::from_millis(200)).is_ok() {
                break;
            }
            ensure!(
                self.deadline.saturating_duration_since(Instant::now()) > Duration::from_secs(30),
                "native dashboard did not listen within budget"
            );
            std::thread::sleep(Duration::from_millis(200));
        }
        let index = self.ready_index()?;
        ensure!(
            index.len() == 2044
                && object(&index).sha256
                    == "d0d769811cbf2e5290dafb5f9f9a005bcafd45469e94899080353093e1f20947"
                && String::from_utf8(index)?.contains(ASSET.trim_start_matches('/')),
            "native page differs from the pinned archive"
        );
        let asset = self.request(ASSET, None)?;
        ensure!(
            asset.len() == 425409 && object(&asset).sha256 == ASSET_SHA,
            "native application asset differs from archived SquashFS member"
        );
        let before_login = self.graphql(
            "mutation { auth { extend { __typename ... on Error { message } } } }",
            json!({}),
        )?;
        require_type(&before_login, "extend", "AuthError")?;
        let rejected = self.graphql(
            AUTH,
            json!({"username":"md-sim-no-such-user","password":"md-sim-invalid-password-probe"}),
        )?;
        require_type(&rejected, "login", "AuthError")?;
        // The hash-checked archived etc/shadow has root::, with no password.
        let login = self.graphql(AUTH, json!({"username":"root","password":""}))?;
        require_type(&login, "login", "VoidResult")?;
        let authenticated = self.graphql(
            "mutation { auth { extend { __typename ... on Error { message } } } }",
            json!({}),
        )?;
        require_type(&authenticated, "extend", "VoidResult")?;
        let first = self.graphql(UPTIME, json!({}))?;
        // Preserve the existing client deadline and request limit. A
        // whole-second guest value can repeat under instruction timing.
        // Leave seven requests and30seconds for settings and logout checks.
        let attempts = self
            .request_limit
            .saturating_sub(self.observations.len() + 7)
            .min(6);
        let mut second = first.clone();
        for attempt in 0..attempts {
            let remaining = self
                .deadline
                .saturating_duration_since(Instant::now())
                .saturating_sub(Duration::from_secs(30));
            ensure!(!remaining.is_zero(), "native uptime observation deadline");
            let pause = if attempt == 0 {
                Duration::from_secs(3)
            } else {
                remaining / (attempts - attempt) as u32
            };
            std::thread::sleep(pause.min(Duration::from_secs(15)).min(remaining));
            second = self.graphql(UPTIME, json!({}))?;
            if md_sim_firmware::s9_telemetry::uptime_progress(&first["bos"], &second["bos"])? {
                break;
            }
        }
        advancing_uptime(&first, &second)?;
        let schema = self.graphql(SCHEMA, json!({}))?;
        let configuration = self.configuration(&second, plan)?;
        let requested_persistence = plan
            .filter(|plan| matches!(plan.action, Action::Write { .. }))
            .map(|_| &configuration["changed"]);
        let luci = check_luci
            .then(|| luci::check(self, requested_persistence))
            .transpose()?;
        if luci
            .as_ref()
            .is_some_and(|value| value["valid_update"]["commit_requested"] == true)
        {
            // The native updater now owns service shutdown. Its caller must
            // observe the actual reboot and verify the next boot separately.
            return Ok(json!({"before":first,"after":second,"schema":schema,
                "configuration":configuration,"luci":luci,"asset":object(&asset),
                "logout":"not attempted after explicit native update commit"}));
        }
        let logout = self.graphql(
            "mutation { auth { logout { __typename ... on Error { message } } } }",
            json!({}),
        )?;
        require_type(&logout, "logout", "VoidResult")?;
        let after_logout = self.graphql(
            "mutation { auth { extend { __typename ... on Error { message } } } }",
            json!({}),
        )?;
        require_type(&after_logout, "extend", "AuthError")?;
        Ok(
            json!({"before":first,"after":second,"schema":schema,"configuration":configuration,"luci":luci,"asset":object(&asset)}),
        )
    }

    fn configuration(&mut self, initial: &Value, plan: Option<&Plan>) -> Result<Value> {
        let original = public_storage(initial)?.clone();
        if let Some(plan) = plan {
            return self.state_phase(&original, &plan.action);
        }
        let mut changed = original.clone();
        changed["webTheme"] = json!(if original["webTheme"] == "DARK" {
            "LIGHT"
        } else {
            "DARK"
        });
        let write = self.graphql(UPDATE_STORAGE, json!({"input":changed}));
        let observed = self.graphql(STORAGE, json!({}));
        // Always attempt restoration after the write, including bad readback.
        // Its independent readback is required; a mutation response is not proof.
        let restore = self.graphql(UPDATE_STORAGE, json!({"input":original}));
        let restored = self.graphql(STORAGE, json!({}));
        let (write, observed, restore, restored) = (write?, observed?, restore?, restored?);
        ensure!(
            write["storage"]["update"]["__typename"] == "StorageData"
                && restore["storage"]["update"]["__typename"] == "StorageData",
            "native settings update or restoration failed"
        );
        ensure!(
            public_storage(&observed)? == &changed,
            "native settings write did not match independent readback"
        );
        ensure!(
            public_storage(&restored)? == &original,
            "native settings restoration did not match original values"
        );
        Ok(
            json!({"original":original,"changed":changed,"readback":observed,"restored":restored,
            "scope":"Native API write/read/restore in one boot. Restart persistence is not yet tested."}),
        )
    }
    fn update_read(&mut self, expected: &Value) -> Result<Value> {
        let write = self.graphql(UPDATE_STORAGE, json!({"input":expected}))?;
        let readback = self.graphql(STORAGE, json!({}))?;
        ensure!(
            write["storage"]["update"]["__typename"] == "StorageData",
            "native settings update failed"
        );
        ensure!(
            public_storage(&readback)? == expected,
            "native settings readback differs"
        );
        Ok(readback)
    }

    fn state_phase(&mut self, before: &Value, action: &Action) -> Result<Value> {
        let scope = "One phase of a native settings restart test. The complete cycle needs all three linked boot results.";
        match action {
            Action::Write { theme } => {
                let mut changed = before.clone();
                changed["webTheme"] = json!(theme);
                ensure!(changed != *before, "test must change the stored theme");
                let readback = self.update_read(&changed)?;
                Ok(
                    json!({"phase":"write","original":before,"changed":changed,"readback":readback,"scope":scope}),
                )
            }
            Action::Restore { original, changed } => {
                ensure!(
                    before == changed,
                    "native settings did not persist across restart"
                );
                let readback = self.update_read(original)?;
                Ok(
                    json!({"phase":"restore","original":original,"changed":changed,"observed_before":before,"readback":readback,"scope":scope}),
                )
            }
            Action::Verify { original } => {
                ensure!(
                    before == original,
                    "restored native settings did not persist across restart"
                );
                Ok(
                    json!({"phase":"verify","original":original,"observed_before":before,"scope":scope}),
                )
            }
        }
    }
}

fn readiness_retryable(record: &Value) -> bool {
    record["path"] == "/"
        && record["request"].is_null()
        && record["http_status"] == "000"
        && matches!(record["curl_exit_code"].as_i64(), Some(7 | 28))
}

fn public_storage(response: &Value) -> Result<&Value> {
    let public = &response["storage"]["public"];
    dashboard_state::storage(public)?;
    Ok(public)
}

fn graphql_data(response: &Value) -> Result<&Value> {
    ensure!(
        response
            .get("errors")
            .is_none_or(|errors| errors.as_array().is_some_and(Vec::is_empty)),
        "native GraphQL errors"
    );
    response
        .get("data")
        .filter(|data| data.is_object())
        .context("native GraphQL data absent")
}

fn require_type(response: &Value, operation: &str, expected: &str) -> Result<()> {
    ensure!(
        response["auth"][operation]["__typename"] == expected,
        "native auth outcome differs: {operation} expected {expected}"
    );
    Ok(())
}

fn advancing_uptime(first: &Value, second: &Value) -> Result<()> {
    ensure!(
        md_sim_firmware::s9_telemetry::uptime_progress(&first["bos"], &second["bos"])?,
        "native uptime did not advance"
    );
    Ok(())
}

pub fn run(deadline: Instant) -> Result<()> {
    fs::DirBuilder::new().mode(0o700).create(ROOT)?;
    let plan = if Path::new("/dashboard-state.json").exists() {
        Some(dashboard_state::parse(&read(
            "/dashboard-state.json",
            4096,
        )?)?)
    } else {
        None
    };
    let check_luci = Path::new("/dashboard-luci").exists();
    if check_luci {
        ensure!(
            fs::read("/dashboard-luci")? == b"md-sim-original-s9-luci-probe/1\n",
            "unknown LuCI probe contract"
        );
    }
    let mut client = Client {
        deadline,
        origin: "http://10.0.2.15".into(),
        root: ROOT.into(),
        endpoint: "10.0.2.15:80".parse()?,
        tls: None,
        request_seconds: 10,
        observations: Vec::new(),
        // Signed validation includes native stop/start and uptime checks.
        request_limit: if Path::new("/dashboard-update.json").exists() {
            40
        } else if check_luci {
            30
        } else {
            20
        },
    };
    let result = client.check(plan.as_ref(), check_luci);
    if result
        .as_ref()
        .is_ok_and(|value| value["luci"]["valid_update"]["commit_requested"] != true)
    {
        // This small control record follows real logout and its rejection
        // check. Send it before the large retained HTTP evidence, so another
        // client does not wait for the serial transfer of every asset byte.
        println!(
            "MD_SIM_DASHBOARD_SESSION_CLOSED {}",
            json!({
            "origin":"http://10.0.2.15","passed":true,
            "observations_completed":client.observations.len()})
        );
    }
    println!(
        "{MARKER}{}",
        json!({"schema":"md-sim-original-s9-dashboard-probe/1",
        "passed":result.is_ok(),"result":result.as_ref().ok(),"error":result.as_ref().err().map(|e|format!("{e:#}")),
        "client":hash_file(&std::env::current_exe()?)?,"curl":hash_file(Path::new("/usr/bin/curl"))?,
        "observations":client.observations,"state_plan":plan,"luci_selected":check_luci,"request_limit":client.request_limit,"origin":"http://10.0.2.15","fresh_cookie_store":true,
        "scope":"Actual original Boser HTTP/GraphQL from a separate isolated local VM. Full dashboard gate remains incomplete: browser rendering, configuration persistence, uploads, WebSockets, SSH and TLS need their own tests."})
    );
    result.map(|_| ())
}

/// Reuse the native archive/service assertions for the endpoint obtained from
/// the selected running worker. The caller must first observe HTTP readiness.
#[allow(dead_code)] // The pool VM uses run(); the board client uses this path.
pub fn run_selected(
    deadline: Instant,
    endpoint: std::net::SocketAddr,
    root: &Path,
) -> Result<Value> {
    ensure!(
        endpoint.ip() == std::net::Ipv4Addr::LOCALHOST && endpoint.port() >= 1024,
        "selected dashboard must use the verified local bridge or SSH tunnel"
    );
    selected(deadline, endpoint, root, None)
}

#[allow(dead_code)] // The ordinary pool client has no remote TLS publication.
pub fn run_selected_tls(
    deadline: Instant,
    endpoint: std::net::SocketAddr,
    root: &Path,
    tls: TlsClient,
) -> Result<Value> {
    ensure!(
        endpoint.is_ipv4()
            && !endpoint.ip().is_unspecified()
            && !endpoint.ip().is_multicast()
            && endpoint.port() >= 1024,
        "selected TLS address is not an explicit unprivileged IPv4 endpoint"
    );
    ensure!(
        tls.selected_run.len() == 68
            && tls.selected_run.starts_with("run-")
            && tls.selected_run[4..].bytes().all(|b| b.is_ascii_hexdigit()),
        "selected TLS worker identity invalid"
    );
    selected(deadline, endpoint, root, Some(tls))
}

fn selected(
    deadline: Instant,
    endpoint: std::net::SocketAddr,
    root: &Path,
    tls: Option<TlsClient>,
) -> Result<Value> {
    fs::DirBuilder::new().mode(0o700).create(root)?;
    let credentials = tls
        .as_ref()
        .map(|tls| -> Result<Value> {
            Ok(
                json!({"ca":hash_file(&tls.ca)?,"certificate":hash_file(&tls.certificate)?,
                "selected_run":tls.selected_run,"server_identity_verification_required":true}),
            )
        })
        .transpose()?;
    let mut client = Client {
        deadline,
        observations: Vec::new(),
        request_limit: 20,
        // Nested board150 transferred only164816 of425409 asset bytes in10s.
        // This explicit client transfer budget changes no guest network timer,
        // bus clock, hash cap or exact-byte asset assertion.
        request_seconds: 60,
        origin: format!(
            "{}://{endpoint}",
            if tls.is_some() { "https" } else { "http" }
        ),
        root: root
            .to_str()
            .context("dashboard evidence path encoding")?
            .into(),
        endpoint,
        tls,
    };
    let result = client.check(None, false);
    let report = json!({"schema":"md-sim-selected-s9-dashboard-probe/1","passed":result.is_ok(),
        "result":result.as_ref().ok(),"error":result.as_ref().err().map(|e|format!("{e:#}")),
        "origin":client.origin,"observations":client.observations,"fresh_cookie_store":true,"tls":credentials,
        "request_seconds":client.request_seconds,"request_limit":client.request_limit,
        "client":hash_file(&std::env::current_exe()?)?,"curl":hash_file(Path::new("/usr/bin/curl"))?,
        "scope":"Actual selected native login, archived assets, live telemetry and configuration write/read/restore. TLS is recorded only when configured and verified for this endpoint. Browser, WebSocket and restart persistence require separate checks."});
    fs::write(
        root.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    result?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readiness_never_retries_http_errors_or_service_operations() {
        for code in [7, 28] {
            let record =
                json!({"path":"/","request":null,"http_status":"000","curl_exit_code":code});
            assert!(readiness_retryable(&record));
            let mut http = record.clone();
            http["http_status"] = json!("503");
            assert!(!readiness_retryable(&http));
            let mut operation = record.clone();
            operation["path"] = json!("/graphql");
            operation["request"] = json!({"query":"mutation { auth { logout { __typename } } }"});
            assert!(!readiness_retryable(&operation));
        }
        assert!(!readiness_retryable(
            &json!({"path":"/","request":null,"http_status":"000","curl_exit_code":63})
        ));
    }
    #[test]
    fn native_response_assessment_rejects_errors_and_static_or_reset_telemetry() {
        assert!(graphql_data(&json!({"data":{},"errors":[{"message":"failed"}]})).is_err());
        assert!(graphql_data(&json!({"data":null})).is_err());
        assert!(require_type(
            &json!({"auth":{"login":{"__typename":"AuthError"}}}),
            "login",
            "AuthResult"
        )
        .is_err());
        let first = json!({"bos":{"hostname":"miner","uptime":{"since":"boot-a","durationS":10}}});
        let mut second = first.clone();
        assert!(advancing_uptime(&first, &second).is_err());
        second["bos"]["uptime"]["durationS"] = json!(11);
        advancing_uptime(&first, &second).unwrap();
        second["bos"]["uptime"]["since"] = json!("boot-b");
        assert!(advancing_uptime(&first, &second).is_err());
    }

    #[test]
    fn missing_configuration_is_not_an_unset_value() {
        assert!(public_storage(&json!({"storage":{"public":{}}})).is_err());
        public_storage(&json!({"storage":{"public":{"webTheme":null,"dashboardDensity":null,"preferredLanguage":null}}})).unwrap();
        assert!(public_storage(&json!({"storage":{"public":{"webTheme":false,"dashboardDensity":null,"preferredLanguage":null}}})).is_err());
    }
}

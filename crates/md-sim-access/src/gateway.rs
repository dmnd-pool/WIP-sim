use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::save_json,
    store::{hash_file, ObjectId},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    net::{IpAddr, SocketAddr},
    os::unix::{
        fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TlsUpstream {
    pub server_name: String,
    pub ca_file: PathBuf,
}
/// An explicit browser environment adaptation. Original response bodies and
/// guest authentication stay unchanged. This never disables TLS checks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BrowserPolicy {
    #[default]
    Preserve,
    UpgradeInsecureRequests,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GatewaySpec {
    pub run_id: String,
    /// Hash of the complete resolved execution specification, not an image hash.
    pub execution_identity: ObjectId,
    pub dashboard_source: String,
    pub listen: SocketAddr,
    /// A worker-owned guest forward. It must remain on worker loopback.
    pub upstream: SocketAddr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_unix: Option<PathBuf>,
    pub upstream_tls: Option<TlsUpstream>,
    #[serde(default)]
    pub browser_policy: BrowserPolicy,
    pub certificate: PathBuf,
    pub private_key: PathBuf,
    /// A run-specific certificate authority verifies gateway clients. Firmware
    /// cookies and Authorization headers are independent and pass unchanged.
    pub client_ca: PathBuf,
    pub allow_clients: Vec<String>,
    /// An isolated browser profile is mandatory for direct-IP publication.
    /// Ports alone do not isolate cookies.
    pub browser_context_id: String,
    pub nginx: PathBuf,
    pub openssl: PathBuf,
}

fn token(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn path_literal(path: &Path) -> Result<&str> {
    let s = path.to_str().context("path is not UTF-8")?;
    ensure!(
        path.is_absolute()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/_-.:".contains(&b))
            && !path
                .components()
                .any(|p| matches!(p, std::path::Component::ParentDir)),
        "path is not a literal absolute configuration path"
    );
    Ok(s)
}
fn regular(path: &Path, secret: bool) -> Result<()> {
    path_literal(path)?;
    let m = fs::symlink_metadata(path)?;
    ensure!(
        m.is_file() && m.mode() & 0o022 == 0,
        "gateway input must be a regular file without foreign write access"
    );
    if secret {
        ensure!(
            m.uid() == unsafe { libc::geteuid() } && m.mode() & 0o077 == 0,
            "gateway key must be private and owned by this account"
        );
    }
    Ok(())
}
fn cidr(s: &str) -> Result<()> {
    let (address, prefix) = s
        .split_once('/')
        .context("client rule must be an explicit CIDR")?;
    let address: IpAddr = address.parse()?;
    let prefix: u8 = prefix.parse()?;
    ensure!(
        prefix > 0 && prefix <= if address.is_ipv4() { 32 } else { 128 },
        "client prefix is invalid or permits every address"
    );
    Ok(())
}
impl GatewaySpec {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            token(self.run_id.strip_prefix("run-").unwrap_or(&self.run_id)),
            "gateway requires a complete run identity"
        );
        self.execution_identity.validate()?;
        ensure!(
            matches!(
                self.dashboard_source.as_str(),
                "native" | "adapted-native" | "simulator-console"
            ),
            "dashboard source is not declared"
        );
        ensure!(
            self.listen.port() >= 1024
                && !self.listen.ip().is_unspecified()
                && !self.listen.ip().is_multicast(),
            "select an explicit unicast dashboard IP and unprivileged port"
        );
        ensure!(
            self.upstream.ip().is_loopback()
                && self.upstream.port() != 0
                && self.upstream != self.listen,
            "upstream must be a distinct worker loopback endpoint"
        );
        if let Some(path) = &self.upstream_unix {
            path_literal(path)?;
            let metadata = fs::symlink_metadata(path)?;
            let parent = path.parent().context("upstream socket parent absent")?;
            let owner = unsafe { libc::geteuid() };
            let directory = fs::symlink_metadata(parent)?;
            ensure!(
                self.upstream_tls.is_none()
                    && metadata.file_type().is_socket()
                    && metadata.uid() == owner
                    && metadata.nlink() == 1
                    && metadata.mode() & 0o777 == 0o600
                    && directory.is_dir()
                    && directory.uid() == owner
                    && directory.mode() & 0o777 == 0o700
                    && path.canonicalize()? == *path,
                "Unix upstream must be a private, owned run socket without path indirection"
            );
        }
        ensure!(
            token(&self.browser_context_id),
            "direct-IP dashboard requires an isolated browser context identity"
        );
        ensure!(
            !self.allow_clients.is_empty() && self.allow_clients.len() <= 64,
            "client access rule required"
        );
        for rule in &self.allow_clients {
            cidr(rule)?;
        }
        for path in [
            &self.certificate,
            &self.client_ca,
            &self.nginx,
            &self.openssl,
        ] {
            regular(path, false)?;
        }
        regular(&self.private_key, true)?;
        for path in [&self.certificate, &self.client_ca] {
            ensure!(
                fs::metadata(path)?.len() <= 4 * 1024 * 1024,
                "certificate input exceeds 4 MiB"
            );
        }
        ensure!(
            fs::metadata(&self.private_key)?.len() <= 65536,
            "private key input exceeds 64 KiB"
        );
        if let Some(tls) = &self.upstream_tls {
            ensure!(
                !tls.server_name.is_empty()
                    && tls.server_name.len() <= 253
                    && tls
                        .server_name
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b".-".contains(&c)),
                "invalid upstream TLS name"
            );
            regular(&tls.ca_file, false)?;
            ensure!(
                fs::metadata(&tls.ca_file)?.len() <= 4 * 1024 * 1024,
                "upstream trust input exceeds 4 MiB"
            );
        }
        Ok(())
    }
    pub fn public_url(&self) -> String {
        format!("https://{}/", self.listen)
    }
    fn upstream_origin(&self) -> String {
        format!(
            "{}://{}",
            if self.upstream_tls.is_some() {
                "https"
            } else {
                "http"
            },
            self.upstream
        )
    }
    fn proxy_target(&self) -> Result<String> {
        Ok(if let Some(path) = &self.upstream_unix {
            format!("http://unix:{}:", path_literal(path)?)
        } else {
            self.upstream_origin()
        })
    }
    pub fn render(&self, directory: &Path) -> Result<String> {
        self.validate()?;
        let dir = path_literal(directory)?;
        let mut config = format!("daemon off;\nmaster_process off;\nworker_processes 1;\npid {dir}/nginx.pid;\nerror_log {dir}/error.log warn;\nevents {{ worker_connections 128; }}\nhttp {{\n log_format md_safe '$remote_addr $request_method $uri $status $body_bytes_sent';\n access_log {dir}/access.log md_safe;\n client_body_temp_path {dir}/body;\n proxy_temp_path {dir}/proxy;\n fastcgi_temp_path {dir}/fastcgi;\n uwsgi_temp_path {dir}/uwsgi;\n scgi_temp_path {dir}/scgi;\n client_max_body_size 32m;\n client_body_timeout 15s;\n client_header_timeout 15s;\n keepalive_timeout 30s;\n server_tokens off;\n map $http_upgrade $md_connection {{ default upgrade; '' close; }}\n map $http_origin $md_origin {{ default invalid; '' ''; '{}' '{}'; }}\n server {{\n  listen {} ssl;\n  server_name {};\n  ssl_certificate {};\n  ssl_certificate_key {};\n  ssl_client_certificate {};\n  ssl_verify_client on;\n  ssl_verify_depth 1;\n  ssl_protocols TLSv1.2 TLSv1.3;\n  ssl_session_tickets off;\n  ssl_session_cache off;\n", self.public_url().trim_end_matches('/'),self.upstream_origin(),self.listen,self.listen.ip(),path_literal(&self.certificate)?,path_literal(&self.private_key)?,path_literal(&self.client_ca)?);
        for rule in &self.allow_clients {
            config.push_str(&format!("  allow {rule};\n"));
        }
        if self.browser_policy == BrowserPolicy::UpgradeInsecureRequests {
            config.push_str(
                "  add_header Content-Security-Policy 'upgrade-insecure-requests' always;\n",
            );
        }
        config.push_str(&format!("  deny all;\n  if ($http_host != '{}') {{ return 421; }}\n  if ($md_origin = invalid) {{ return 403; }}\n  add_header X-Md-Sim-Run '{}' always;\n  location / {{\n   proxy_pass {};\n   proxy_http_version 1.1;\n   proxy_hide_header X-Md-Sim-Run;\n   proxy_set_header Host '{}';\n   proxy_set_header Origin $md_origin;\n   proxy_set_header Upgrade $http_upgrade;\n   proxy_set_header Connection $md_connection;\n   proxy_set_header Forwarded '';\n   proxy_set_header X-Forwarded-For '';\n   proxy_set_header X-Forwarded-Host '';\n   proxy_set_header X-Forwarded-Proto '';\n   proxy_buffering off;\n   proxy_request_buffering off;\n   proxy_read_timeout 3600s;\n   proxy_send_timeout 60s;\n   proxy_connect_timeout 5s;\n   proxy_redirect {}/ {};\n   proxy_cookie_domain {} {};\n",self.listen,self.run_id,self.proxy_target()?,self.upstream,self.upstream_origin(),self.public_url(),self.upstream.ip(),self.listen.ip()));
        if let Some(tls) = &self.upstream_tls {
            config.push_str(&format!("   proxy_ssl_verify on;\n   proxy_ssl_server_name on;\n   proxy_ssl_name {};\n   proxy_ssl_trusted_certificate {};\n   proxy_ssl_verify_depth 3;\n",tls.server_name,path_literal(&tls.ca_file)?));
        }
        config.push_str("  }\n }\n}\n");
        Ok(config)
    }
}

pub struct Gateway {
    child: Child,
    directory: PathBuf,
}

fn check_certificate(spec: &GatewaySpec) -> Result<()> {
    let mut command = Command::new(&spec.openssl);
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("OPENSSL_CONF", "/dev/null")
        .args(["x509", "-in"])
        .arg(&spec.certificate)
        .args(["-noout", "-checkip"])
        .arg(spec.listen.ip().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let parent = std::process::id() as libc::pid_t;
    unsafe {
        command.pre_exec(move || {
            for (resource, limit) in [
                (libc::RLIMIT_CPU, 3),
                (libc::RLIMIT_AS, 256 * 1024 * 1024),
                (libc::RLIMIT_NOFILE, 32),
                (libc::RLIMIT_CORE, 0),
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
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
                || libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                || libc::getppid() != parent
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    let started = std::time::Instant::now();
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "TLS certificate is invalid or does not cover selected IP"
                    ))
                }
            }
            Ok(None) if started.elapsed() < std::time::Duration::from_secs(5) => {
                std::thread::sleep(std::time::Duration::from_millis(10))
            }
            Ok(None) => break Err(anyhow::anyhow!("certificate preflight deadline exceeded")),
            Err(error) => break Err(error.into()),
        }
    };
    if !matches!(child.try_wait(), Ok(Some(_))) {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}
impl Gateway {
    /// Nginx runs in the caller's worker environment. The caller must provide
    /// the same VM boundary as the guest when that backend requires it.
    pub fn start(spec: &GatewaySpec, directory: &Path) -> Result<Self> {
        spec.validate()?;
        ensure!(!directory.exists(), "gateway output must be new");
        fs::create_dir(directory)?;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        let directory = directory.canonicalize()?;
        check_certificate(spec)?;
        let mut config_file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.join("nginx.conf"))?;
        config_file.write_all(spec.render(&directory)?.as_bytes())?;
        config_file.sync_all()?;
        let mut command = Command::new(&spec.nginx);
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .arg("-p")
            .arg(&directory)
            .arg("-c")
            .arg(directory.join("nginx.conf"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(directory.join("startup.log"))?,
            ));
        let parent = std::process::id() as libc::pid_t;
        unsafe {
            command.pre_exec(move || {
                for (resource, limit) in [
                    (libc::RLIMIT_AS, 512 * 1024 * 1024),
                    (libc::RLIMIT_NOFILE, 512),
                    (libc::RLIMIT_FSIZE, 64 * 1024 * 1024),
                    (libc::RLIMIT_NPROC, 32),
                    (libc::RLIMIT_CORE, 0),
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
                if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
                    || libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                    || libc::getppid() != parent
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn()?;
        let mut gateway = Self { child, directory };
        std::thread::sleep(std::time::Duration::from_millis(100));
        ensure!(
            gateway.child.try_wait()?.is_none(),
            "gateway failed to start; inspect its startup log"
        );
        let manifest = json!({"schema":"md-sim-dashboard-endpoint/1","run_id":spec.run_id,"execution_identity":spec.execution_identity,"dashboard_source":spec.dashboard_source,"url":spec.public_url(),"upstream":spec.upstream_origin(),"upstream_unix":spec.upstream_unix,"nginx":hash_file(&spec.nginx)?,"openssl":hash_file(&spec.openssl)?,"config":hash_file(&gateway.directory.join("nginx.conf"))?,"server_certificate":hash_file(&spec.certificate)?,"client_ca":hash_file(&spec.client_ca)?,"client_access_rules":spec.allow_clients,"gateway_authentication":"mutual TLS with configured client certificate authority; firmware authentication unchanged","cookie_policy":{"mode":"isolated-browser-context","context_id":spec.browser_context_id,"warning":"ports do not isolate cookies; use the recorded isolated client context"},"transformations":["Host to exact loopback guest origin","Origin only from this exact public origin","Location only from exact guest origin","cookie Domain only from exact guest address"],"pid":gateway.child.id(),"access_gate":"requires separate-client test of this endpoint"});
        let mut manifest = manifest;
        manifest["browser_policy"] = serde_json::to_value(spec.browser_policy)?;
        manifest["browser_policy_scope"] = json!("Explicit response-header policy. Original response bodies and guest authentication are unchanged. Browser behavior and native service operation require a separate selected-endpoint test.");
        save_json(&gateway.directory.join("endpoint.json"), &manifest)?;
        Ok(gateway)
    }
    pub fn manifest(&self) -> Result<Value> {
        Ok(serde_json::from_slice(&fs::read(
            self.directory.join("endpoint.json"),
        )?)?)
    }
    pub fn is_running(&mut self) -> Result<bool> {
        Ok(self.child.try_wait()?.is_none())
    }
    pub fn stop(&mut self) -> Result<()> {
        if self.child.try_wait()?.is_none() {
            self.child.kill()?;
        }
        let status = self.child.wait()?;
        save_json(
            &self.directory.join("stopped.json"),
            &json!({"owned_pid":self.child.id(),"exit":status.code(),"stopped":true}),
        )
    }
}
impl Drop for Gateway {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_certificate_preflight_checks_ip_and_size_limits() {
        let temp = tempfile::tempdir().unwrap();
        let credentials = temp.path().join("credentials");
        crate::credentials::create(
            Path::new("/usr/bin/openssl"),
            &credentials,
            "127.0.0.2".parse().unwrap(),
            &"ab".repeat(32),
        )
        .unwrap();
        let mut spec = GatewaySpec {
            run_id: "ab".repeat(32),
            execution_identity: ObjectId {
                sha256: "cd".repeat(32),
                size: 1,
            },
            dashboard_source: "simulator-console".into(),
            listen: "127.0.0.2:8443".parse().unwrap(),
            upstream: "127.0.0.1:8080".parse().unwrap(),
            upstream_unix: None,
            upstream_tls: None,
            browser_policy: BrowserPolicy::Preserve,
            certificate: credentials.join("server.crt"),
            private_key: credentials.join("server.key"),
            client_ca: credentials.join("ca.crt"),
            allow_clients: vec!["127.0.0.3/32".into()],
            browser_context_id: "ef".repeat(32),
            nginx: "/usr/bin/openssl".into(),
            openssl: "/usr/bin/openssl".into(),
        };
        spec.validate().unwrap();
        check_certificate(&spec).unwrap();
        let mut old = serde_json::to_value(&spec).unwrap();
        old.as_object_mut().unwrap().remove("browser_policy");
        assert_eq!(
            serde_json::from_value::<GatewaySpec>(old.clone())
                .unwrap()
                .browser_policy,
            BrowserPolicy::Preserve
        );
        old["browser_policy"] = json!("disable-certificate-checks");
        assert!(serde_json::from_value::<GatewaySpec>(old).is_err());
        spec.listen = "127.0.0.9:8443".parse().unwrap();
        assert!(check_certificate(&spec).is_err());
        fs::OpenOptions::new()
            .write(true)
            .open(&spec.client_ca)
            .unwrap()
            .set_len(4 * 1024 * 1024 + 1)
            .unwrap();
        assert!(spec.validate().is_err());
    }
    #[test]
    fn client_rules_need_real_bounds() {
        for rule in [
            "0.0.0.0/0",
            "127.0.0.1",
            "127.0.0.1/33",
            "::/0",
            "::1/129",
            "127.0.0.1/32; allow all",
        ] {
            assert!(cidr(rule).is_err());
        }
        assert!(cidr("127.0.0.3/32").is_ok());
        assert!(cidr("::1/128").is_ok());
    }
    #[test]
    fn config_paths_reject_variables_and_directives() {
        for p in [
            "/tmp/a;",
            "/tmp/$host",
            "/tmp/a\nlisten",
            "/tmp/../key",
            "relative",
        ] {
            assert!(path_literal(Path::new(p)).is_err());
        }
    }
}

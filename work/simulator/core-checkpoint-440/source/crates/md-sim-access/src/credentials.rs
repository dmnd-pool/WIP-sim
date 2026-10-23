//! Local test certificate provisioning with the installed, identified OpenSSL.
//! Private keys remain in the credential directory, outside run reports.
use anyhow::{ensure, Result};
use md_sim_runtime::{
    isolation::save_json,
    store::{hash_file, ObjectId},
};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    net::IpAddr,
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, PermissionsExt},
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

fn openssl(program: &Path, directory: &Path, args: &[&str]) -> Result<()> {
    bounded_operation(program, directory, args, Duration::from_secs(15))
}

struct OwnedOperation(Child, bool);
impl Drop for OwnedOperation {
    fn drop(&mut self) {
        if self.1 {
            return;
        }
        // The process group is private, including any unexpected descendants.
        unsafe {
            libc::kill(-(self.0.id() as libc::pid_t), libc::SIGKILL);
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn bounded_operation(
    program: &Path,
    directory: &Path,
    args: &[&str],
    deadline: Duration,
) -> Result<()> {
    let mut command = Command::new(program);
    command
        .current_dir(directory)
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("OPENSSL_CONF", "/dev/null")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let parent = std::process::id() as libc::pid_t;
    unsafe {
        command.pre_exec(move || {
            libc::umask(0o077);
            for (resource, limit) in [
                (libc::RLIMIT_CPU, 10),
                (libc::RLIMIT_AS, 256 * 1024 * 1024),
                (libc::RLIMIT_FSIZE, 1024 * 1024),
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
            if libc::setpgid(0, 0) != 0
                || libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0
                || libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                || libc::getppid() != parent
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = OwnedOperation(command.spawn()?, false);
    let mut stderr = child.0.stderr.take().expect("piped stderr");
    let fd = stderr.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    ensure!(
        flags >= 0 && unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == 0,
        "cannot bound certificate diagnostics"
    );
    let started = Instant::now();
    let mut diagnostics = Vec::new();
    let mut buffer = [0u8; 4096];
    let mut eof = false;
    let status = loop {
        ensure!(
            started.elapsed() < deadline,
            "certificate support operation deadline exceeded"
        );
        // Limit work per iteration so a noisy process cannot starve the clock.
        for _ in 0..4 {
            match stderr.read(&mut buffer) {
                Ok(0) => {
                    eof = true;
                    break;
                }
                Ok(n) => {
                    ensure!(
                        diagnostics.len() + n <= 64 * 1024,
                        "certificate support diagnostics exceed limit"
                    );
                    diagnostics.extend_from_slice(&buffer[..n]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }
        }
        // Observe without reaping. The PID must remain reserved until its
        // private group has been stopped, including on the success path.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        ensure!(
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    child.0.id(),
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            } == 0,
            "certificate child status failed"
        );
        if unsafe { info.si_pid() } != 0 && eof {
            unsafe {
                libc::kill(-(child.0.id() as libc::pid_t), libc::SIGKILL);
            }
            let status = child.0.wait()?;
            child.1 = true;
            break status;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    ensure!(
        status.success(),
        "certificate support operation failed: {}",
        String::from_utf8_lossy(&diagnostics)
    );
    Ok(())
}

/// A support executable already captured into the isolated client's inputs.
#[derive(Clone, Debug)]
pub struct BrowserProfileTool {
    pub program: PathBuf,
    pub object: ObjectId,
}

#[derive(Clone, Debug)]
pub struct BrowserProfileTools {
    pub openssl: BrowserProfileTool,
    pub certutil: BrowserProfileTool,
    pub pk12util: BrowserProfileTool,
}

struct ImportFiles(PathBuf);
impl ImportFiles {
    fn cleanup(&self) -> Result<()> {
        for name in [
            "client.p12",
            "ca.der",
            "client.der",
            "nss-ca.der",
            "nss-client.der",
        ] {
            match fs::remove_file(self.0.join(name)) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error.into()),
            }
        }
        match fs::remove_dir(&self.0) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}
impl Drop for ImportFiles {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn checked_profile_tool(tool: &BrowserProfileTool) -> Result<()> {
    tool.object.validate()?;
    ensure!(
        tool.program.is_absolute() && fs::symlink_metadata(&tool.program)?.is_file(),
        "browser credential tool must be an absolute regular file"
    );
    ensure!(
        hash_file(&tool.program)? == tool.object,
        "captured browser credential tool changed"
    );
    Ok(())
}

fn checked_certificate(path: &Path, private: bool) -> Result<ObjectId> {
    let info = fs::symlink_metadata(path)?;
    ensure!(
        path.is_absolute()
            && info.is_file()
            && info.len() > 0
            && info.len() <= if private { 65536 } else { 4 << 20 },
        "browser certificate input bounds"
    );
    if private {
        ensure!(
            info.uid() == unsafe { libc::geteuid() } && info.mode() & 0o077 == 0,
            "browser client private key must be owned and private"
        );
    }
    hash_file(path)
}

/// Initialize a fresh private Firefox NSS profile with only the selected CA
/// and client credential. Run this inside the isolated client, before Firefox.
/// The profile may contain user.js, but must not contain an earlier NSS store.
pub fn prepare_browser_profile(
    tools: &BrowserProfileTools,
    profile: &Path,
    ca: &Path,
    client_certificate: &Path,
    client_key: &Path,
    run_id: &str,
) -> Result<Value> {
    ensure!(
        run_id.len() == 64
            && run_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "browser credential run ID must be lowercase SHA256"
    );
    let info = fs::symlink_metadata(profile)?;
    ensure!(
        profile.is_absolute()
            && info.is_dir()
            && info.uid() == unsafe { libc::geteuid() }
            && info.mode() & 0o777 == 0o700
            && profile.canonicalize()? == profile,
        "browser profile must be a private owned canonical directory"
    );
    for entry in fs::read_dir(profile)? {
        let entry = entry?;
        ensure!(
            entry.file_name() == "user.js" && entry.file_type()?.is_file(),
            "browser profile already contains data"
        );
    }
    for tool in [&tools.openssl, &tools.certutil, &tools.pk12util] {
        checked_profile_tool(tool)?;
    }
    let ca_id = checked_certificate(ca, false)?;
    let client_id = checked_certificate(client_certificate, false)?;
    let key_id = checked_certificate(client_key, true)?;
    let path_text = |p: &Path| -> Result<String> {
        Ok(p.to_str()
            .ok_or_else(|| anyhow::anyhow!("certificate path is not UTF8"))?
            .to_owned())
    };
    let ca_path = path_text(ca)?;
    let client_path = path_text(client_certificate)?;
    let key_path = path_text(client_key)?;
    let database = format!("sql:{}", path_text(profile)?);
    let temporary = profile.join(".md-sim-import");
    fs::create_dir(&temporary)?;
    fs::set_permissions(&temporary, fs::Permissions::from_mode(0o700))?;
    let temporary = ImportFiles(temporary);
    let operation = |tool: &BrowserProfileTool, args: &[&str]| {
        bounded_operation(&tool.program, &temporary.0, args, Duration::from_secs(15))
    };
    operation(
        &tools.openssl,
        &[
            "verify",
            "-CAfile",
            &ca_path,
            "-purpose",
            "sslclient",
            &client_path,
        ],
    )?;
    operation(
        &tools.certutil,
        &["-N", "-d", &database, "--empty-password"],
    )?;
    operation(
        &tools.certutil,
        &[
            "-A",
            "-d",
            &database,
            "-n",
            "md-sim-selected-ca",
            "-t",
            "CT,,",
            "-i",
            &ca_path,
        ],
    )?;
    operation(
        &tools.openssl,
        &[
            "pkcs12",
            "-export",
            "-inkey",
            &key_path,
            "-in",
            &client_path,
            "-certfile",
            &ca_path,
            "-name",
            "md-sim-selected-client",
            "-out",
            "client.p12",
            "-passout",
            "pass:",
        ],
    )?;
    operation(
        &tools.pk12util,
        &["-i", "client.p12", "-d", &database, "-W", "", "-K", ""],
    )?;
    let mut public_readback = serde_json::Map::new();
    for (name, input, nickname) in [
        ("ca", ca_path.as_str(), "md-sim-selected-ca"),
        ("client", client_path.as_str(), "md-sim-selected-client"),
    ] {
        let original = format!("{name}.der");
        let imported = format!("nss-{name}.der");
        operation(
            &tools.openssl,
            &["x509", "-in", input, "-outform", "DER", "-out", &original],
        )?;
        operation(
            &tools.certutil,
            &["-L", "-d", &database, "-n", nickname, "-r", "-o", &imported],
        )?;
        let original = hash_file(&temporary.0.join(original))?;
        ensure!(
            original.size > 0 && original == hash_file(&temporary.0.join(imported))?,
            "NSS certificate readback differs from selected credential"
        );
        public_readback.insert(name.into(), serde_json::to_value(original)?);
    }
    operation(
        &tools.certutil,
        &[
            "-V",
            "-d",
            &database,
            "-n",
            "md-sim-selected-client",
            "-u",
            "C",
        ],
    )?;
    ensure!(
        checked_certificate(ca, false)? == ca_id
            && checked_certificate(client_certificate, false)? == client_id
            && checked_certificate(client_key, true)? == key_id,
        "browser credential changed during import"
    );
    for tool in [&tools.openssl, &tools.certutil, &tools.pk12util] {
        checked_profile_tool(tool)?;
    }
    temporary.cleanup()?;
    for name in ["cert9.db", "key4.db", "pkcs11.txt"] {
        let path = profile.join(name);
        let info = fs::symlink_metadata(&path)?;
        ensure!(
            info.is_file() && info.uid() == unsafe { libc::geteuid() } && info.mode() & 0o077 == 0,
            "NSS profile database is not private"
        );
    }
    Ok(
        json!({"schema":"md-sim-browser-profile/1","run_id":run_id,"profile":profile,
        "ca":ca_id,"client_certificate":client_id,"certificate_der_readback":public_readback,
        "tools":{"openssl":tools.openssl.object,"certutil":tools.certutil.object,"pk12util":tools.pk12util.object},
        "nss_ca_nickname":"md-sim-selected-ca","nss_client_nickname":"md-sim-selected-client",
        "ca_trust":"CT,,","client_certificate_validated":true,"private_import_file_removed":true,
        "scope":"Fresh isolated Firefox profile; private keys remain only in private client storage and NSS database"}),
    )
}

pub fn create(program: &Path, directory: &Path, ip: IpAddr, run_id: &str) -> Result<Value> {
    ensure!(
        run_id.len() == 64
            && run_id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "credential run ID must be 64 lowercase hexadecimal characters"
    );
    ensure!(
        !ip.is_unspecified() && !ip.is_multicast(),
        "credential address must be explicit unicast"
    );
    ensure!(
        program.is_absolute() && fs::symlink_metadata(program)?.is_file(),
        "OpenSSL must be an absolute regular executable"
    );
    ensure!(!directory.exists(), "credential directory must be new");
    fs::create_dir(directory)?;
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
    let directory = directory.canonicalize()?;
    openssl(
        program,
        &directory,
        &[
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:prime256v1",
            "-noenc",
            "-keyout",
            "ca.key",
            "-out",
            "ca.crt",
            "-sha256",
            "-days",
            "1",
            "-subj",
            &format!("/CN=md-sim-test-{}", &run_id[..16]),
            "-addext",
            "basicConstraints=critical,CA:TRUE,pathlen:0",
            "-addext",
            "keyUsage=critical,keyCertSign,cRLSign",
        ],
    )?;
    for (name, purpose, identity) in [
        ("server", "serverAuth", format!("IP:{ip}")),
        (
            "client",
            "clientAuth",
            format!("URI:urn:md-sim:client:{run_id}"),
        ),
    ] {
        openssl(
            program,
            &directory,
            &[
                "req",
                "-new",
                "-newkey",
                "ec",
                "-pkeyopt",
                "ec_paramgen_curve:prime256v1",
                "-noenc",
                "-keyout",
                &format!("{name}.key"),
                "-out",
                &format!("{name}.csr"),
                "-sha256",
                "-subj",
                &format!("/CN=md-sim-{name}-{}", &run_id[..16]),
            ],
        )?;
        fs::write(directory.join(format!("{name}.ext")),format!("basicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature\nextendedKeyUsage={purpose}\nsubjectAltName={identity}\n"))?;
        openssl(
            program,
            &directory,
            &[
                "x509",
                "-req",
                "-in",
                &format!("{name}.csr"),
                "-CA",
                "ca.crt",
                "-CAkey",
                "ca.key",
                "-CAcreateserial",
                "-out",
                &format!("{name}.crt"),
                "-days",
                "1",
                "-sha256",
                "-extfile",
                &format!("{name}.ext"),
            ],
        )?;
    }
    openssl(
        program,
        &directory,
        &[
            "verify",
            "-CAfile",
            "ca.crt",
            "-purpose",
            "sslserver",
            "-verify_ip",
            &ip.to_string(),
            "server.crt",
        ],
    )?;
    openssl(
        program,
        &directory,
        &[
            "verify",
            "-CAfile",
            "ca.crt",
            "-purpose",
            "sslclient",
            "client.crt",
        ],
    )?;
    save_json(
        &directory.join("gateway-auth.json"),
        &json!({"type":"mutual-tls","run_id":run_id,"client_ca":directory.join("ca.crt")}),
    )?;
    save_json(
        &directory.join("client-auth.json"),
        &json!({"type":"mutual-tls","run_id":run_id,"certificate":directory.join("client.crt"),"private_key":directory.join("client.key"),"server_ca":directory.join("ca.crt")}),
    )?;
    let manifest = json!({"schema":"md-sim-test-credentials/1","run_id":run_id,"selected_ip":ip,"expires_after_days":1,"openssl":hash_file(program)?,"ca":hash_file(&directory.join("ca.crt"))?,"server_certificate":hash_file(&directory.join("server.crt"))?,"client_certificate":hash_file(&directory.join("client.crt"))?,"scope":"private local test CA; install its public CA on the selected test client only","gateway_auth_file":directory.join("gateway-auth.json"),"client_auth_file":directory.join("client-auth.json")});
    save_json(&directory.join("manifest.json"), &manifest)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blocked_and_noisy_helpers_are_bounded() {
        let directory = tempfile::tempdir().unwrap();
        let start = Instant::now();
        let error = bounded_operation(
            Path::new("/usr/bin/sleep"),
            directory.path(),
            &["10"],
            Duration::from_millis(50),
        )
        .unwrap_err();
        assert!(error.to_string().contains("deadline"));
        assert!(start.elapsed() < Duration::from_secs(2));
        let error = bounded_operation(
            Path::new("/usr/bin/sh"),
            directory.path(),
            &["-c", "exec /usr/bin/head -c 131072 /dev/zero >&2"],
            Duration::from_secs(2),
        )
        .unwrap_err();
        assert!(error.to_string().contains("diagnostics exceed limit"));
    }
}

//! OpenSSH carries one structured control message to the private worker socket.
//! No worker operation or firmware name becomes a shell fragment.
use crate::WorkerTransport;
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    store::{hash_file, ObjectId},
    worker::{self, Request},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    os::fd::AsRawFd,
    os::unix::{
        fs::{MetadataExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};
mod stream;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SshConfig {
    pub host: String,
    pub user: String,
    pub port: u16,
    pub known_hosts: PathBuf,
    pub identity_file: PathBuf,
    pub remote_relay: String,
    pub worker_socket: String,
    pub ssh_program: PathBuf,
    pub timeout_seconds: u64,
}

fn safe_remote_path(value: &str) -> bool {
    value.starts_with('/')
        && value.len() <= 4096
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"/_.-".contains(&c))
        && value.split('/').all(|part| part != ".." && part != ".")
}
impl SshConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.host.is_empty()
                && self.host.len() <= 253
                && !self.host.starts_with('-')
                && self
                    .host
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b".:-".contains(&c)),
            "invalid SSH host"
        );
        ensure!(
            !self.user.is_empty()
                && self.user.len() <= 64
                && !self.user.starts_with('-')
                && self
                    .user
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)),
            "invalid SSH user"
        );
        ensure!(
            self.port != 0 && (1..=300).contains(&self.timeout_seconds),
            "invalid SSH port or deadline"
        );
        ensure!(
            safe_remote_path(&self.remote_relay) && safe_remote_path(&self.worker_socket),
            "remote relay and socket must be literal absolute paths"
        );
        for path in [&self.known_hosts, &self.identity_file] {
            ensure!(path.to_str().is_some_and(safe_remote_path), "SSH trust and key paths must be literal; expansion tokens and whitespace are forbidden");
        }
        for path in [&self.known_hosts, &self.identity_file, &self.ssh_program] {
            ensure!(path.is_absolute(), "SSH input path must be absolute");
            let metadata = fs::symlink_metadata(path)?;
            ensure!(
                metadata.is_file(),
                "SSH input must be a regular file, without a symlink"
            );
            ensure!(
                metadata.mode() & 0o022 == 0,
                "SSH input is writable by another account"
            );
        }
        let key = fs::symlink_metadata(&self.identity_file)?;
        ensure!(
            key.uid() == unsafe { libc::geteuid() } && key.mode() & 0o077 == 0,
            "SSH private key must be owned by this account and private"
        );
        ensure!(
            fs::metadata(&self.known_hosts)?.len() > 0,
            "pinned known_hosts file is empty"
        );
        Ok(())
    }
}

pub struct SshTransport {
    config: SshConfig,
    ssh_identity: ObjectId,
    host_identity: ObjectId,
    key_identity: ObjectId,
    streaming: bool,
    session: Option<stream::Session>,
    reconnect_count: u64,
}
impl SshTransport {
    pub fn new(config: SshConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            ssh_identity: hash_file(&config.ssh_program)?,
            host_identity: hash_file(&config.known_hosts)?,
            key_identity: hash_file(&config.identity_file)?,
            config,
            streaming: false,
            session: None,
            reconnect_count: 0,
        })
    }
    /// Reuse one private SSH process for bounded request/response frames.
    /// The remote relay must support --stream. An interrupted read-only request
    /// can reconnect once. Mutations still require explicit state recovery.
    pub fn new_streaming(config: SshConfig) -> Result<Self> {
        let mut transport = Self::new(config)?;
        transport.streaming = true;
        Ok(transport)
    }
    pub fn evidence(&self) -> Value {
        json!({"transport":"OpenSSH private worker relay","session":if self.streaming {"bounded framed channel"} else {"one SSH process per request"},"host":self.config.host,"user":self.config.user,"port":self.config.port,"ssh":self.ssh_identity,"host_key_file":self.host_identity,"remote_relay":self.config.remote_relay,"worker_socket":self.config.worker_socket,"host_key_policy":"StrictHostKeyChecking=yes; UpdateHostKeys=no","deadline_seconds":self.config.timeout_seconds,"deadline_scope":"per request attempt","read_only_reconnects":if self.streaming {1}else{0}})
    }
    /// Diagnostic count only; it does not alter the locked transport policy.
    pub fn reconnect_count(&self) -> u64 {
        self.reconnect_count
    }
    fn verify_inputs(&self) -> Result<()> {
        self.config.validate()?;
        ensure!(
            hash_file(&self.config.ssh_program)? == self.ssh_identity
                && hash_file(&self.config.known_hosts)? == self.host_identity
                && hash_file(&self.config.identity_file)? == self.key_identity,
            "SSH executable, trust or credential identity changed"
        );
        Ok(())
    }
    /// Start a support service as the authenticated worker owner. Arguments
    /// have a literal grammar because OpenSSH uses the remote login shell.
    /// The caller owns the returned child and its log/resource bounds.
    pub fn service_command(&self, program: &str, args: &[&str]) -> Result<Command> {
        self.verify_inputs()?;
        let line = service_line(program, args)?;
        let mut command = self.base_command();
        command
            .args(["-o", "ClearAllForwardings=yes"])
            .arg(&self.config.host)
            .arg(line);
        Ok(command)
    }
    fn base_command(&self) -> Command {
        let mut command = Command::new(&self.config.ssh_program);
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LC_ALL", "C")
            .args([
                "-F",
                "/dev/null",
                "-T",
                "-S",
                "none",
                "-o",
                "BatchMode=yes",
                "-o",
                "StrictHostKeyChecking=yes",
                "-o",
                "UpdateHostKeys=no",
                "-o",
                "GlobalKnownHostsFile=/dev/null",
                "-o",
                "IdentityAgent=none",
                "-o",
                "IdentitiesOnly=yes",
                "-o",
                "PreferredAuthentications=publickey",
                "-o",
                "PasswordAuthentication=no",
                "-o",
                "KbdInteractiveAuthentication=no",
                "-o",
                "ForwardAgent=no",
                "-o",
                "ControlMaster=no",
                "-o",
                "ProxyCommand=none",
                "-o",
                "PermitLocalCommand=no",
                "-o",
                "ConnectionAttempts=1",
                "-o",
                "ServerAliveInterval=5",
                "-o",
                "ServerAliveCountMax=2",
            ])
            .arg("-o")
            .arg(format!(
                "ConnectTimeout={}",
                self.config.timeout_seconds.min(30)
            ))
            .arg("-o")
            .arg(format!(
                "UserKnownHostsFile={}",
                self.config.known_hosts.display()
            ))
            .arg("-i")
            .arg(&self.config.identity_file)
            .arg("-p")
            .arg(self.config.port.to_string())
            .arg("-l")
            .arg(&self.config.user);
        command
    }
    fn command(&self) -> Command {
        let mut command = self.base_command();
        command
            .arg("-o")
            .arg("ClearAllForwardings=yes")
            .arg(&self.config.host)
            // Both fields have a narrow literal grammar. OpenSSH passes this
            // fixed command through the remote login shell.
            .arg(format!(
                "{} {}{}",
                self.config.remote_relay,
                self.config.worker_socket,
                if self.streaming { " --stream" } else { "" }
            ));
        command
    }
}

fn service_line(program: &str, args: &[&str]) -> Result<String> {
    ensure!(
        safe_remote_path(program)
            && args.len() <= 16
            && args.iter().all(|arg| !arg.is_empty()
                && arg.len() <= 4096
                && arg
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"/_.:-=".contains(&c))),
        "remote service arguments must be bounded literal tokens"
    );
    Ok(std::iter::once(program)
        .chain(args.iter().copied())
        .collect::<Vec<_>>()
        .join(" "))
}

/// Owns only the SSH process created for one loopback forward. This does not
/// count as a dashboard test; the client must test the selected run's URL.
pub struct SshTunnel {
    child: OwnedSshChild,
    evidence: Value,
}
impl SshTunnel {
    pub fn start(
        config: SshConfig,
        listen: SocketAddr,
        remote: SocketAddr,
        log: &Path,
    ) -> Result<Self> {
        ensure!(
            listen.ip().is_loopback()
                && remote.ip().is_loopback()
                && listen.port() != 0
                && remote.port() != 0,
            "SSH dashboard forward must use explicit loopback endpoints"
        );
        // Do not accept a pre-existing listener as proof that our tunnel started.
        let reservation = std::net::TcpListener::bind(listen)
            .context("requested tunnel port is occupied or unavailable")?;
        let transport = SshTransport::new(config)?;
        let mut command = transport.base_command();
        command
            .args([
                "-N",
                "-o",
                "ExitOnForwardFailure=yes",
                "-o",
                "GatewayPorts=no",
                "-o",
                "ClearAllForwardings=no",
            ])
            .arg("-L")
            .arg(format!("{listen}:{remote}"))
            .arg(&transport.config.host)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(
                fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(log)?,
            ));
        limit_ssh_child(&mut command, None, 64 * 1024 * 1024);
        drop(reservation);
        let tunnel = Self {
            child: OwnedSshChild::new(command.spawn()?),
            evidence: json!({"transport":transport.evidence(),"listen":listen,"remote_loopback":remote,"dashboard_gate":"requires actual selected-run client test"}),
        };
        let start = Instant::now();
        loop {
            ensure!(
                !tunnel.child.exited()?,
                "SSH tunnel failed; inspect its private log"
            );
            if TcpStream::connect_timeout(&listen, Duration::from_millis(50)).is_ok() {
                break;
            }
            ensure!(
                start.elapsed() < Duration::from_secs(transport.config.timeout_seconds),
                "SSH tunnel startup deadline exceeded"
            );
            thread::sleep(Duration::from_millis(20));
        }
        // ExitOnForwardFailure makes a bind race a process failure. The HTTP
        // client must still check the run-specific response after this step.
        thread::sleep(Duration::from_millis(50));
        ensure!(!tunnel.child.exited()?, "SSH tunnel lost its listener");
        Ok(tunnel)
    }
    pub fn evidence(&self) -> Value {
        self.evidence.clone()
    }
    pub fn is_running(&mut self) -> Result<bool> {
        Ok(!self.child.exited()?)
    }
    pub fn stop(&mut self) -> Result<()> {
        self.child.finish()?;
        Ok(())
    }
}
impl Drop for SshTunnel {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Owns an SSH process and its private process group. Exit observation does not
/// reap the leader, so cleanup cannot signal a recycled process group ID.
pub struct OwnedSshChild {
    child: Child,
    status: Option<ExitStatus>,
}
impl OwnedSshChild {
    /// Start a service with the same process-group and parent-death guards as
    /// the control transport. The caller must enforce its own wall deadline.
    pub fn spawn_service(command: &mut Command, cpu_seconds: u64, log_bytes: u64) -> Result<Self> {
        ensure!(
            (1..=3600).contains(&cpu_seconds) && (1..=64 * 1024 * 1024).contains(&log_bytes),
            "SSH service limits exceed the bounded policy"
        );
        limit_ssh_child(command, Some(cpu_seconds), log_bytes);
        Ok(Self::new(
            command.spawn().context("start owned SSH service")?,
        ))
    }

    fn new(child: Child) -> Self {
        Self {
            child,
            status: None,
        }
    }
    pub fn exited(&self) -> std::io::Result<bool> {
        if self.status.is_some() {
            return Ok(true);
        }
        let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
        let rc = unsafe {
            libc::waitid(
                libc::P_PID,
                self.child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if rc != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(unsafe { info.si_pid() } != 0)
    }
    /// Close the pipe owned by this guard, if the caller requested piped stdin.
    pub fn close_stdin(&mut self) {
        self.child.stdin.take();
    }

    /// Stop the private group before reaping its leader. Repeated calls return
    /// the recorded exit status without sending another signal.
    pub fn finish(&mut self) -> std::io::Result<ExitStatus> {
        if let Some(status) = self.status {
            return Ok(status);
        }
        // This is our still-unreaped child and its private process group.
        unsafe {
            libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL);
        }
        let _ = self.child.kill();
        let status = self.child.wait()?;
        self.status = Some(status);
        Ok(status)
    }
}
impl Drop for OwnedSshChild {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

fn limit_ssh_child(command: &mut Command, cpu_seconds: Option<u64>, log_bytes: u64) {
    let parent = std::process::id() as libc::pid_t;
    unsafe {
        command.pre_exec(move || {
            for (resource, limit) in [
                (libc::RLIMIT_AS, 512 * 1024 * 1024),
                (libc::RLIMIT_NOFILE, 256),
                (libc::RLIMIT_FSIZE, log_bytes),
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
            if let Some(limit) = cpu_seconds {
                if libc::setrlimit(
                    libc::RLIMIT_CPU,
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
            {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                return Err(std::io::Error::from_raw_os_error(libc::ECANCELED));
            }
            Ok(())
        });
    }
}
fn nonblocking(file: &impl AsRawFd) -> Result<()> {
    let fd = file.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    ensure!(
        flags >= 0 && unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == 0,
        "cannot bound SSH pipe"
    );
    Ok(())
}
fn drain_pipe(input: &mut impl Read, output: &mut Vec<u8>, limit: usize) -> Result<bool> {
    let mut buffer = [0u8; 8192];
    for _ in 0..4 {
        match input.read(&mut buffer) {
            Ok(0) => return Ok(true),
            Ok(n) => {
                ensure!(
                    n <= limit.saturating_sub(output.len()),
                    "SSH output exceeds limit"
                );
                output.extend_from_slice(&buffer[..n]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e.into()),
        }
    }
    Ok(false)
}
fn exchange(mut command: Command, message: &[u8], timeout: Duration) -> Result<Vec<u8>> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    limit_ssh_child(
        &mut command,
        Some(timeout.as_secs().saturating_add(1)),
        64 * 1024 * 1024,
    );
    let mut child = OwnedSshChild::new(command.spawn().context("start SSH control client")?);
    let mut stdin = Some(child.child.stdin.take().context("SSH stdin missing")?);
    let mut stdout = child.child.stdout.take().context("SSH stdout missing")?;
    let mut stderr = child.child.stderr.take().context("SSH stderr missing")?;
    nonblocking(stdin.as_ref().unwrap())?;
    nonblocking(&stdout)?;
    nonblocking(&stderr)?;
    let start = Instant::now();
    let mut sent = 0;
    let mut received = Vec::new();
    let mut diagnostic = Vec::new();
    let mut stdout_eof = false;
    let mut stderr_eof = false;
    loop {
        ensure!(start.elapsed()<timeout,"SSH request deadline exceeded; reconnect by submission token before retrying execution");
        if let Some(writer) = stdin.as_mut() {
            if sent < message.len() {
                match writer.write(&message[sent..message.len().min(sent + 8192)]) {
                    Ok(0) => anyhow::bail!("SSH input closed before request completion"),
                    Ok(n) => sent += n,
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                        ) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            if sent == message.len() {
                stdin = None;
            }
        }
        if !stdout_eof {
            stdout_eof = drain_pipe(
                &mut stdout,
                &mut received,
                worker::MAX_CONTROL_MESSAGE as usize,
            )?;
        }
        if !stderr_eof {
            stderr_eof = drain_pipe(&mut stderr, &mut diagnostic, 65536)?;
        }
        if child.exited()? && stdout_eof && stderr_eof {
            let status = child.finish()?;
            ensure!(
                status.success(),
                "SSH failed ({status}): {}",
                String::from_utf8_lossy(&diagnostic)
            );
            ensure!(
                sent == message.len(),
                "SSH exited before request completion"
            );
            return Ok(received);
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(test)]
fn read_bounded(mut input: impl Read, limit: usize) -> std::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut oversized = false;
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        if count > limit.saturating_sub(output.len()) {
            oversized = true;
        }
        let keep = count.min(limit.saturating_sub(output.len()));
        output.extend_from_slice(&buffer[..keep]);
    }
    if oversized {
        return Err(std::io::Error::other("SSH output exceeds limit"));
    }
    Ok(output)
}
impl WorkerTransport for SshTransport {
    fn request(&mut self, request: &Request) -> Result<Value> {
        self.verify_inputs()?;
        let mut message = serde_json::to_vec(request)?;
        message.push(b'\n');
        ensure!(
            message.len() as u64 <= worker::MAX_CONTROL_MESSAGE,
            "worker request exceeds limit"
        );
        let timeout = Duration::from_secs(self.config.timeout_seconds);
        if self.streaming {
            let mut attempts = 0;
            let result = read_only_reconnect(request, || {
                attempts += 1;
                self.verify_inputs()?;
                // Taking ownership drops a failed channel before reconnect.
                let mut session = match self.session.take() {
                    Some(session) if !session.exhausted() => session,
                    _ => stream::Session::start(self.command())?,
                };
                let received = session.exchange(&message, timeout)?;
                // Decode failures and worker errors are never retried.
                let response = worker::decode_response(&received)?;
                self.session = Some(session);
                Ok(response)
            });
            self.reconnect_count = self
                .reconnect_count
                .checked_add(u64::from(attempts == 2))
                .context("SSH reconnect counter overflow")?;
            result
        } else {
            worker::decode_response(&exchange(self.command(), &message, timeout)?)
        }
    }
    fn description(&self) -> String {
        self.evidence().to_string()
    }
}

fn read_only_reconnect<T>(request: &Request, mut attempt: impl FnMut() -> Result<T>) -> Result<T> {
    match attempt() {
        Err(error)
            if stream::retryable(&error)
                && matches!(
                    request,
                    Request::Probe
                        | Request::Get { .. }
                        | Request::LookupToken { .. }
                        | Request::PrivateChannelStatus { .. }
                        | Request::Status { .. }
                        | Request::Result { .. }
                        | Request::ResultData { .. }
                        | Request::Logs { .. }
                ) =>
        {
            eprintln!("md-sim SSH: reconnect once for an interrupted read-only request: {error:#}");
            attempt().with_context(|| format!("read-only reconnect failed after: {error:#}"))
        }
        result => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interrupted_readonly_queries_reconnect_once_with_identical_identity_bytes() -> Result<()> {
        for request in [
            Request::Status {
                run_id: "run-selected".into(),
            },
            Request::PrivateChannelStatus {
                token: "a".repeat(64),
                challenge: "b".repeat(64),
            },
        ] {
            let mut message = serde_json::to_vec(&request)?;
            message.push(b'\n');
            let mut attempts = 0;
            let received = read_only_reconnect(&request, || {
                attempts += 1;
                let command = if attempts == 1 {
                    let mut command = Command::new("/usr/bin/sh");
                    command.args(["-c", "read line; exit 0"]);
                    command
                } else {
                    Command::new("/usr/bin/cat")
                };
                stream::Session::start(command)?.exchange(&message, Duration::from_secs(2))
            })?;
            assert_eq!(attempts, 2);
            assert_eq!(received, message);
        }
        Ok(())
    }

    #[test]
    fn reconnect_never_repeats_mutations_or_protocol_errors_and_is_bounded() {
        for request in [
            Request::Stop {
                run_id: "run-selected".into(),
            },
            Request::Shutdown,
        ] {
            let mut attempts = 0;
            let result: Result<()> = read_only_reconnect(&request, || {
                attempts += 1;
                Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe).into())
            });
            assert!(result.is_err());
            assert_eq!(attempts, 1);
        }
        for transient in [true, false] {
            let mut attempts = 0;
            let result: Result<()> = read_only_reconnect(&Request::Probe, || {
                attempts += 1;
                if transient {
                    Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe).into())
                } else {
                    Err(anyhow::anyhow!("invalid worker response"))
                }
            });
            assert!(result.is_err());
            assert_eq!(attempts, if transient { 2 } else { 1 });
        }
    }
    #[test]
    fn remote_service_preserves_literals_and_rejects_shell_expansion() {
        assert_eq!(
            service_line(
                "/md-sim-access",
                &["serve-selected", "/worker/spec.json", "run-abc"]
            )
            .unwrap(),
            "/md-sim-access serve-selected /worker/spec.json run-abc"
        );
        for argument in [
            "", "a b", "$(id)", "`id`", "x;y", "x\ny", "$HOME", "*.json", "'x'", "a>b", "a|b",
            "a&b",
        ] {
            assert!(
                service_line("/md-sim-access", &[argument]).is_err(),
                "{argument}"
            );
        }
        assert!(service_line("/tmp/../tool", &["a"]).is_err());
        assert!(service_line("/tool", &["a"; 17]).is_err());
        assert!(service_line("/tool", &[&"x".repeat(4097)]).is_err());
    }
    #[test]
    fn remote_paths_cannot_inject_shell_syntax() {
        for path in [
            "/bin/relay;true",
            "/tmp/$(id)",
            "/tmp/a b",
            "/tmp/../relay",
            "relative",
            "/tmp/a\n/b",
            "/tmp/'x'",
            "/tmp/%h",
            "/tmp/${HOME}",
            "/tmp/~user",
        ] {
            assert!(!safe_remote_path(path));
        }
        assert!(safe_remote_path("/opt/md-sim/bin/md-sim-worker-relay"));
    }
    #[test]
    fn replies_reject_truncation_multiple_messages_and_protocol_errors() {
        assert!(worker::decode_response(b"{}\n{}\n").is_err());
        assert!(worker::decode_response(b"{}").is_err());
        assert!(worker::decode_response(
            b"{\"protocol\":\"md-sim-worker/2\",\"ok\":false,\"error\":\"refused\"}\n"
        )
        .is_err());
        assert_eq!(worker::decode_response(b"{\"protocol\":\"md-sim-worker/2\",\"ok\":true,\"result\":{\"state\":\"running\"}}\n").unwrap()["state"], "running");
    }
    #[test]
    fn captured_output_is_bounded() {
        assert_eq!(read_bounded(&b"abc"[..], 3).unwrap(), b"abc");
        assert!(read_bounded(&b"abcd"[..], 3).is_err());
    }
    #[test]
    fn owned_service_closes_stdin_and_enforces_file_and_cpu_limits() {
        let root = tempfile::tempdir().unwrap();
        let forbidden = root.path().join("must-not-spawn");
        for (cpu, bytes) in [
            (0, 1),
            (1, 0),
            (3601, 1),
            (1, 64 * 1024 * 1024 + 1),
            (u64::MAX, u64::MAX),
        ] {
            let mut command = Command::new("/usr/bin/touch");
            command.arg(&forbidden);
            assert!(OwnedSshChild::spawn_service(&mut command, cpu, bytes).is_err());
        }
        assert!(!forbidden.exists());
        let report = root.path().join("limits");
        let mut command = Command::new("/usr/bin/sh");
        command
            .args([
                "-c",
                "/usr/bin/cat >/dev/null; /usr/bin/cat /proc/self/limits",
            ])
            .stdin(Stdio::piped())
            .stdout(fs::File::create(&report).unwrap())
            .stderr(Stdio::null());
        let mut child = OwnedSshChild::spawn_service(&mut command, 7, 8192).unwrap();
        assert!(!child.exited().unwrap());
        child.close_stdin();
        let start = Instant::now();
        while !child.exited().unwrap() && start.elapsed() < Duration::from_secs(3) {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(child.exited().unwrap());
        let status = child.finish().unwrap();
        assert!(status.success());
        assert_eq!(child.finish().unwrap(), status);
        let limits = fs::read_to_string(report).unwrap();
        for (name, expected) in [("Max cpu time", "7"), ("Max file size", "8192")] {
            let values: Vec<_> = limits
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .unwrap()
                .split_whitespace()
                .collect();
            assert_eq!(&values[..2], &[expected, expected]);
        }

        let output = root.path().join("bounded-output");
        let mut command = Command::new("/usr/bin/head");
        command
            .args(["-c", "16384", "/dev/zero"])
            .stdin(Stdio::null())
            .stdout(fs::File::create(&output).unwrap())
            .stderr(Stdio::null());
        let mut child = OwnedSshChild::spawn_service(&mut command, 7, 4096).unwrap();
        let start = Instant::now();
        while !child.exited().unwrap() && start.elapsed() < Duration::from_secs(3) {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(child.exited().unwrap());
        assert!(!child.finish().unwrap().success());
        assert_eq!(fs::metadata(output).unwrap().len(), 4096);
    }
    #[test]
    fn exchange_delivers_real_bytes_and_bounds_blocked_or_noisy_children() {
        let data = vec![0x5a; 128 * 1024];
        let reply = exchange(Command::new("/usr/bin/cat"), &data, Duration::from_secs(3)).unwrap();
        assert_eq!(reply, data);
        let mut command = Command::new("/usr/bin/sleep");
        command.arg("10");
        let start = Instant::now();
        let error = exchange(command, &data, Duration::from_millis(60)).unwrap_err();
        assert!(error.to_string().contains("deadline"));
        assert!(start.elapsed() < Duration::from_secs(2));
        let mut command = Command::new("/usr/bin/sh");
        command.args(["-c", "exec /usr/bin/head -c 131072 /dev/zero >&2"]);
        let error = exchange(command, &[], Duration::from_secs(2)).unwrap_err();
        assert!(error.to_string().contains("exceeds limit"));
    }
    #[test]
    fn exchange_deadline_cleans_up_descendant_that_keeps_a_pipe_open() {
        let root = tempfile::tempdir().unwrap();
        let pidfile = root.path().join("owned-child.pid");
        let mut command = Command::new("/usr/bin/sh");
        command
            .args([
                "-c",
                "/usr/bin/sleep 10 & printf '%s' \"$!\" > \"$1\"; exit 0",
                "sh",
            ])
            .arg(&pidfile);
        let start = Instant::now();
        let error = exchange(command, &[], Duration::from_millis(100)).unwrap_err();
        assert!(error.to_string().contains("deadline"));
        assert!(start.elapsed() < Duration::from_secs(2));
        let pid: u32 = fs::read_to_string(pidfile).unwrap().parse().unwrap();
        let mut stopped = false;
        for _ in 0..50 {
            match fs::read_to_string(format!("/proc/{pid}/stat")) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    stopped = true;
                    break;
                }
                Ok(s)
                    if s.split_once(") ")
                        .is_some_and(|(_, rest)| rest.starts_with('Z')) =>
                {
                    stopped = true;
                    break;
                }
                _ => thread::sleep(Duration::from_millis(10)),
            }
        }
        assert!(stopped, "owned inherited-pipe child survived cleanup");
    }
}

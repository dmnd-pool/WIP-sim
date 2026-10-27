use crate::store::{hash_file, ObjectId, Store};
pub use crate::support_virtualization::{SupportVirtualization, SupportVirtualizationReceipt};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{Receiver, RecvTimeoutError, SyncSender},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

/// Hard aggregate limit for retained stdout and stderr. A full firmware
/// upload can require both a complete packet export and a device trace.
/// Each run must request and reserve its own smaller or equal bound.
pub const MAX_TRACE_BYTES: u64 = 128 * 1024 * 1024;

/// A declared byte protocol on standard input. The supervisor sends pipe
/// bytes only, never a host file descriptor. Actual sent bytes enter evidence.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StdinStream {
    pub protocol: String,
    pub max_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mount {
    pub object: ObjectId,
    pub guest_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub wall_seconds: u64,
    pub cpu_seconds: u64,
    pub memory_bytes: u64,
    pub processes: u64,
    pub file_bytes: u64,
    pub trace_bytes: u64,
    pub tmp_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            wall_seconds: 30,
            cpu_seconds: 20,
            memory_bytes: 256 * 1024 * 1024,
            processes: 64,
            file_bytes: 8 * 1024 * 1024,
            trace_bytes: 1024 * 1024,
            tmp_bytes: 16 * 1024 * 1024,
        }
    }
}

impl Limits {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.wall_seconds > 0 && self.wall_seconds <= 86400,
            "wall-time limit out of range"
        );
        ensure!(
            self.cpu_seconds > 0 && self.cpu_seconds <= 86400,
            "CPU-time limit out of range"
        );
        ensure!(
            (16 * 1024 * 1024..=16 * 1024 * 1024 * 1024).contains(&self.memory_bytes),
            "memory limit out of range"
        );
        ensure!(
            (1..=4096).contains(&self.processes),
            "process limit out of range"
        );
        ensure!(
            self.file_bytes > 0 && self.trace_bytes > 0 && self.tmp_bytes > 0,
            "byte limits must be positive"
        );
        ensure!(
            self.tmp_bytes <= self.memory_bytes && self.trace_bytes <= MAX_TRACE_BYTES,
            "storage/trace limit exceeds worker policy"
        );
        Ok(())
    }
}

/// An immutable executable closure. Every guest-visible program and library is
/// a content-addressed input. No host directory is mounted into the guest.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessSpec {
    /// An explicit support-VM capability. Ordinary execution and worker APIs
    /// reject this request; only trusted support launch APIs can grant it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_virtualization: Option<SupportVirtualization>,
    /// One runtime-created framed Ethernet link. A matching explicit provider
    /// is required. No host descriptor or listener may be specified here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_channel: Option<crate::private_channel::Spec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin_stream: Option<StdinStream>,
    /// Map only the invoking host user and group to this identity in the new
    /// namespace. Nonzero identities can host a worker that creates another
    /// private namespace. The default retains earlier process identities.
    #[serde(default, skip_serializing_if = "namespace_root")]
    pub namespace_uid: u32,
    pub schema_version: String,
    pub argv: Vec<String>,
    pub inputs: Vec<Mount>,
    pub limits: Limits,
    pub environment: BTreeMap<String, String>,
    /// Enable only loopback inside the fresh network namespace. The default
    /// leaves it down. No host interface or route is ever inherited.
    #[serde(default, skip_serializing_if = "loopback_disabled")]
    pub loopback: bool,
    /// Optional incoming dashboard bridge. It exposes only a run-owned Unix
    /// socket to the guest and binds the public side to worker loopback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dashboard: Option<crate::dashboard::Spec>,
    /// Optional immutable standard input. Its bytes are part of the run identity.
    #[serde(default)]
    pub stdin_object: Option<ObjectId>,
}

fn loopback_disabled(enabled: &bool) -> bool {
    !enabled
}

fn namespace_root(uid: &u32) -> bool {
    *uid == 0
}

impl ProcessSpec {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == "2.0", "unsupported process schema");
        ensure!(
            self.namespace_uid <= 65534,
            "namespace identity out of range"
        );
        self.limits.validate()?;
        if let Some(channel) = &self.private_channel {
            channel.validate()?;
        }
        if let Some(dashboard) = &self.dashboard {
            dashboard.validate()?;
            ensure!(self.loopback, "dashboard bridge requires private loopback");
            ensure!(
                self.limits.memory_bytes >= 512 << 20 && self.limits.processes >= 32,
                "dashboard bridge requires at least 512 MiB address space and 32 process slots"
            );
        }
        if let Some(input) = &self.stdin_object {
            input.validate()?;
        }
        if let Some(stream) = &self.stdin_stream {
            ensure!(
                self.stdin_object.is_none(),
                "immutable and streamed stdin are exclusive"
            );
            ensure!(
                (1..=65536).contains(&stream.max_bytes),
                "streamed stdin byte limit out of range"
            );
            ensure!(
                !stream.protocol.is_empty()
                    && stream.protocol.len() <= 128
                    && stream
                        .protocol
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"/._-".contains(&b)),
                "invalid streamed stdin protocol"
            );
        }
        ensure!(
            !self.argv.is_empty() && self.argv.len() <= 128,
            "invalid argument count"
        );
        ensure!(
            self.argv
                .iter()
                .all(|s| s.len() <= 8192 && !s.contains('\0')),
            "invalid command argument"
        );
        validate_guest_path(&self.argv[0])?;
        ensure!(
            self.inputs.len() <= 2048,
            "input count exceeds worker policy"
        );
        let mut paths = std::collections::BTreeSet::new();
        for input in &self.inputs {
            input.object.validate()?;
            validate_guest_path(&input.guest_path)?;
            ensure!(
                paths.insert(&input.guest_path),
                "duplicate guest mount path"
            );
        }
        for a in &paths {
            for b in &paths {
                if a != b {
                    ensure!(
                        !Path::new(b).starts_with(Path::new(a)),
                        "nested file mounts are invalid"
                    );
                }
            }
        }
        ensure!(
            paths.contains(&self.argv[0]),
            "executable must be a locked input"
        );
        ensure!(
            self.environment.len() <= 64,
            "environment exceeds worker policy"
        );
        for (k, v) in &self.environment {
            ensure!(
                k != crate::dashboard::TOKEN_ENV && k != crate::dashboard::PORT_ENV,
                "dashboard bridge environment is runtime-owned"
            );
            ensure!(
                !k.is_empty() && k.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
                "invalid environment key"
            );
            ensure!(
                v.len() <= 8192 && !v.contains('\0'),
                "invalid environment value"
            );
        }
        Ok(())
    }
}

fn validate_guest_path(s: &str) -> Result<()> {
    let p = Path::new(s);
    ensure!(
        p.is_absolute() && s.len() <= 4096 && !s.contains('\0') && !s.ends_with('/'),
        "guest path must be an absolute file path"
    );
    ensure!(
        p.components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_))),
        "guest path contains parent/current components"
    );
    ensure!(
        !s.split('/')
            .skip(1)
            .any(|c| c.is_empty() || c == "." || c == ".."),
        "guest path must be normalized"
    );
    for special in [
        "/proc",
        "/dev",
        "/tmp",
        "/work",
        "/sys",
        crate::dashboard::SOCKET_PATH,
    ] {
        ensure!(
            !p.starts_with(special),
            "guest path overlaps a runtime mount"
        );
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub support_virtualization: Option<SupportVirtualizationReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_channel: Option<crate::private_channel::Receipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdin_stream: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dashboard: Option<serde_json::Value>,
    pub status: String,
    pub exit_code: Option<i32>,
    pub wall_milliseconds: u128,
    pub stdout: String,
    pub stderr: String,
    pub trace_truncated: bool,
    pub isolation: String,
    pub limits_scope: String,
    pub tools: BTreeMap<String, ObjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aggregate_limits: Option<serde_json::Value>,
}

/// Host-owned log files. These descriptors are never inherited by the guest.
/// Both streams share the process trace budget. A busy stream can use space
/// that the other stream does not need; their combined output remains bounded.
pub struct TraceFiles {
    pub stdout: File,
    pub stderr: File,
}

fn tool_path(name: &str) -> Result<PathBuf> {
    for base in ["/usr/bin", "/bin"] {
        let p = Path::new(base).join(name);
        if p.is_file() {
            return Ok(p.canonicalize()?);
        }
    }
    anyhow::bail!("required isolation tool {name} is unavailable")
}

/// Runs a closed input set with no external network and no host directory mounts.
/// Only scratch tmpfs storage is writable. Persistent outputs need a separate,
/// bounded export protocol; arbitrary guest paths are never copied to the host.
pub fn execute(
    store: &Store,
    spec: &ProcessSpec,
    cancel: Arc<AtomicBool>,
) -> Result<ProcessResult> {
    execute_with_trace(store, spec, cancel, None)
}

pub fn execute_with_trace(
    store: &Store,
    spec: &ProcessSpec,
    cancel: Arc<AtomicBool>,
    trace: Option<TraceFiles>,
) -> Result<ProcessResult> {
    execute_recorded(store, spec, cancel, trace, None, None)
}

/// Explicit host grant for a trusted support VM. Never call this on a remote
/// worker job or untrusted firmware specification. The only permitted device
/// is a freshly verified KVM node; all other isolation remains unchanged.
pub fn execute_support_vm(
    store: &Store,
    spec: &ProcessSpec,
    cancel: Arc<AtomicBool>,
    trace: Option<TraceFiles>,
) -> Result<ProcessResult> {
    execute_inner(
        store, spec, cancel, trace, None, None, None, None, None, true,
    )
}

/// The same explicit support grant with the existing bounded stdin protocol.
pub fn execute_support_vm_with_streamed_input(
    store: &Store,
    spec: &ProcessSpec,
    cancel: Arc<AtomicBool>,
    trace: Option<TraceFiles>,
    input: Receiver<Vec<u8>>,
) -> Result<ProcessResult> {
    execute_inner(
        store,
        spec,
        cancel,
        trace,
        None,
        None,
        None,
        Some(input),
        None,
        true,
    )
}

/// Run a declared input protocol. A sender must send only bounded frames and
/// drop its channel when finished. The runtime closes the pipe at process end.
pub fn execute_with_streamed_input(
    store: &Store,
    spec: &ProcessSpec,
    cancel: Arc<AtomicBool>,
    trace: Option<TraceFiles>,
    input: Receiver<Vec<u8>>,
) -> Result<ProcessResult> {
    execute_inner(
        store,
        spec,
        cancel,
        trace,
        None,
        None,
        None,
        Some(input),
        None,
        false,
    )
}

/// Run one declared private Ethernet carrier. The runtime creates the socket
/// and offers its host endpoint through `provider.try_send` before launch.
/// Use a bounded channel with capacity one. The provider must own and bound
/// its own processes; the runtime never calls or joins arbitrary provider code.
/// Runtime termination closes the carrier. Only descriptor3 enters the guest.
pub fn execute_with_private_channel(
    store: &Store,
    spec: &ProcessSpec,
    cancel: Arc<AtomicBool>,
    trace: Option<TraceFiles>,
    input: Option<Receiver<Vec<u8>>>,
    provider: SyncSender<File>,
) -> Result<ProcessResult> {
    execute_private_channel_recorded(
        store, spec, cancel, trace, None, None, None, input, provider,
    )
}

/// The private-carrier entry with the worker's durable cgroup and dashboard
/// receipts. The endpoint is still created and owned by this runtime.
#[allow(clippy::too_many_arguments)]
pub fn execute_private_channel_recorded(
    store: &Store,
    spec: &ProcessSpec,
    cancel: Arc<AtomicBool>,
    trace: Option<TraceFiles>,
    cgroup_parent: Option<&Path>,
    cgroup_receipt: Option<&Path>,
    dashboard_receipt: Option<&Path>,
    input: Option<Receiver<Vec<u8>>>,
    provider: SyncSender<File>,
) -> Result<ProcessResult> {
    ensure!(
        spec.private_channel.is_some(),
        "private Ethernet policy is required"
    );
    execute_inner(
        store,
        spec,
        cancel,
        trace,
        cgroup_parent,
        cgroup_receipt,
        dashboard_receipt,
        input,
        Some(provider),
        false,
    )
}

/// Durable cgroup receipt enables cleanup after abrupt supervisor loss.
pub fn execute_recorded(
    store: &Store,
    spec: &ProcessSpec,
    cancel: Arc<AtomicBool>,
    trace: Option<TraceFiles>,
    cgroup_parent: Option<&Path>,
    cgroup_receipt: Option<&Path>,
) -> Result<ProcessResult> {
    execute_dashboard_recorded(
        store,
        spec,
        cancel,
        trace,
        cgroup_parent,
        cgroup_receipt,
        None,
    )
}

/// As execute_recorded, with a host-owned live dashboard binding receipt.
pub fn execute_dashboard_recorded(
    store: &Store,
    spec: &ProcessSpec,
    cancel: Arc<AtomicBool>,
    trace: Option<TraceFiles>,
    cgroup_parent: Option<&Path>,
    cgroup_receipt: Option<&Path>,
    dashboard_receipt: Option<&Path>,
) -> Result<ProcessResult> {
    execute_inner(
        store,
        spec,
        cancel,
        trace,
        cgroup_parent,
        cgroup_receipt,
        dashboard_receipt,
        None,
        None,
        false,
    )
}

// Keep existing public execution contracts intact while sharing all isolation.
#[allow(clippy::too_many_arguments)]
fn execute_inner(
    store: &Store,
    spec: &ProcessSpec,
    cancel: Arc<AtomicBool>,
    trace: Option<TraceFiles>,
    cgroup_parent: Option<&Path>,
    cgroup_receipt: Option<&Path>,
    dashboard_receipt: Option<&Path>,
    input: Option<Receiver<Vec<u8>>>,
    provider: Option<SyncSender<File>>,
    support_grant: bool,
) -> Result<ProcessResult> {
    spec.validate()?;
    ensure!(
        spec.support_virtualization.is_some() == support_grant,
        "support virtualization requires an explicit trusted support-VM grant"
    );
    ensure!(
        !support_grant || (spec.private_channel.is_none() && spec.dashboard.is_none()),
        "support virtualization does not grant a carrier or dashboard capability"
    );
    ensure!(
        spec.private_channel.is_some() == provider.is_some(),
        "private Ethernet requires an explicit policy and matching supervisor provider"
    );
    ensure!(
        spec.stdin_stream.is_some() == input.is_some(),
        "streamed stdin requires an explicit policy and matching supervisor channel"
    );
    let virtualization = if support_grant {
        Some(crate::support_virtualization::Grant::open()?)
    } else {
        None
    };
    let dashboard = spec
        .dashboard
        .as_ref()
        .map(|endpoint| {
            crate::dashboard::Server::start(
                endpoint,
                spec,
                dashboard_receipt.context("dashboard binding receipt is required")?,
            )
        })
        .transpose()?;
    ensure!(
        cgroup_receipt.is_some() == cgroup_parent.is_some(),
        "aggregate limits require both delegation and a durable receipt"
    );
    let mut group = cgroup_parent
        .map(|parent| crate::cgroup::Group::create(parent, &spec.limits, cgroup_receipt))
        .transpose()?;
    let attach_fd = group.as_ref().map(crate::cgroup::Group::attach_fd);
    let unshare = tool_path("unshare")?;
    let bwrap = tool_path("bwrap")?;
    let prlimit = tool_path("prlimit")?;
    let mut tools = BTreeMap::new();
    tools.insert("unshare".into(), hash_file(&unshare)?);
    tools.insert("bubblewrap".into(), hash_file(&bwrap)?);
    tools.insert("prlimit".into(), hash_file(&prlimit)?);
    let mut command = Command::new(unshare);
    command.arg("--user");
    if spec.namespace_uid == 0 {
        command.arg("--map-root-user");
    } else {
        command.args([
            "--map-user",
            &spec.namespace_uid.to_string(),
            "--map-group",
            &spec.namespace_uid.to_string(),
        ]);
    }
    command.arg("--mount");
    if spec.private_channel.is_none() {
        command.args(["--fork", "--kill-child"]);
    }
    // For a carrier, unshare must exec the next tool without a fork. Its optional fork
    // monitor retains FD3 and would conceal guest EOF. Bubblewrap already owns
    // PID1/reaping and parent-death propagation, and closes unrelated monitor
    // descriptors. The default process path remains unchanged.
    if !spec.loopback {
        command.arg("--net");
    }
    // Create the user namespace before lowering NPROC: the new namespace
    // must not inherit a small ancestor allowance already consumed by other
    // host work. prlimit execs Bubblewrap without an extra monitor or fork.
    // The declared process bound applies before any guest program executes.
    command
        .arg(prlimit)
        .arg(format!("--nproc={}", spec.limits.processes))
        .arg("--")
        .arg(bwrap)
        .args([
            "--die-with-parent",
            "--new-session",
            // Bubblewrap must create the PID namespace and its proc mount
            // together. An enclosing unshare PID namespace can leave the
            // Bubblewrap monitor's outside filesystem reachable through proc.
            "--unshare-pid",
            "--unshare-ipc",
            "--unshare-uts",
            "--clearenv",
            "--hostname",
            "md-sim",
            "--cap-drop",
            "ALL",
        ]);
    if spec.loopback {
        // Bubblewrap creates the namespace and enables only its loopback
        // interface before dropping capabilities. Host network policy can
        // still refuse this setup; never fall back to a shared namespace.
        command.arg("--unshare-net");
    }
    // No host files, home directory, management sockets, or physical devices.
    command
        .args(["--proc", "/proc", "--dev", "/dev", "--size"])
        .arg(spec.limits.tmp_bytes.to_string())
        .args(["--tmpfs", "/tmp", "--size"])
        .arg(spec.limits.tmp_bytes.to_string())
        .args(["--tmpfs", "/work", "--chdir", "/work"]);
    if let Some(grant) = &virtualization {
        grant.verify_path()?;
        command.args([
            "--dev-bind",
            crate::support_virtualization::DEVICE,
            crate::support_virtualization::DEVICE,
        ]);
    }
    for input in &spec.inputs {
        command
            .arg("--ro-bind")
            .arg(store.verified_path(&input.object)?)
            .arg(&input.guest_path);
    }
    if let Some(service) = &dashboard {
        command
            .arg("--ro-bind")
            .arg(service.socket())
            .arg(crate::dashboard::SOCKET_PATH)
            .args(["--setenv", crate::dashboard::TOKEN_ENV, service.token()])
            .args(["--setenv", crate::dashboard::PORT_ENV])
            .arg(service.binding.guest_port.to_string());
    }
    // The root is a separate tmpfs. Keep it read-only so writes cannot bypass
    // the size limits of /tmp and /work. This does not remount those children.
    command.args(["--remount-ro", "/"]);
    for (k, v) in &spec.environment {
        command.arg("--setenv").arg(k).arg(v);
    }
    command.arg("--").args(&spec.argv);
    // Never inherit an object-store descriptor. Even a read-only descriptor
    // can expose its host mount through /proc/self/fd. Feed a pipe from the
    // supervisor instead; the guest receives only bytes, not the backing file.
    let stdin_source = spec
        .stdin_object
        .as_ref()
        .map(|input| Ok::<_, anyhow::Error>(File::open(store.verified_path(input)?)?))
        .transpose()?;
    let stdin = if stdin_source.is_some() || input.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    };
    command
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Finish input verification before offering a live carrier to the provider.
    // Its owned relay lifetime starts within the process wall-time budget.
    let start = Instant::now();
    let mut channel = spec
        .private_channel
        .as_ref()
        .zip(provider)
        .map(|(policy, provider)| crate::private_channel::Session::new(policy, provider))
        .transpose()?;
    let limits = spec.limits.clone();
    let supervisor_pid = std::process::id() as libc::pid_t;
    let channel_fd = channel
        .as_ref()
        .map(crate::private_channel::Session::guest_fd);
    // The closure only calls async-signal-safe system calls before exec.
    unsafe {
        command.pre_exec(move || {
            // A killed worker cannot run its Rust cleanup guard. Terminate the
            // namespace supervisor on parent loss; unshare --kill-child and
            // Bubblewrap propagate this to the guest. Close the fork/prctl race.
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != supervisor_pid {
                return Err(std::io::Error::from_raw_os_error(libc::ECANCELED));
            }
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if let Some(fd) = attach_fd {
                // Attach before executing. CLOEXEC keeps the host FD private.
                if libc::write(fd, b"0\n".as_ptr().cast(), 2) != 2 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            // Bubblewrap passes inherited non-CLOEXEC descriptors to the
            // executable. Mark every non-stdio descriptor first, including any
            // unrelated descriptor opened by the embedding application. Keep
            // Rust's exec-status pipe open until exec so spawn errors still work.
            if libc::syscall(
                libc::SYS_close_range,
                3u32,
                u32::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            ) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            if let Some(fd) = channel_fd {
                if libc::dup2(fd, crate::private_channel::GUEST_FD) < 0
                    || libc::fcntl(crate::private_channel::GUEST_FD, libc::F_SETFD, 0) < 0
                {
                    return Err(std::io::Error::last_os_error());
                }
            }
            for (resource, value) in [
                (libc::RLIMIT_CPU, limits.cpu_seconds),
                (libc::RLIMIT_AS, limits.memory_bytes),
                (libc::RLIMIT_FSIZE, limits.file_bytes),
                (libc::RLIMIT_NOFILE, 256),
                (libc::RLIMIT_CORE, 0),
            ] {
                let r = libc::rlimit {
                    rlim_cur: value,
                    rlim_max: value,
                };
                if libc::setrlimit(resource, &r) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut owned_child = ProcessGuard {
        child: command.spawn().context("start isolated process")?,
        reaped: false,
    };
    if let Some(channel) = &mut channel {
        channel.launched();
    }
    let child = &mut owned_child.child;
    let input_finished = Arc::new(AtomicBool::new(false));
    let _input_completion = InputCompletion(input_finished.clone());
    let input_failed = Arc::new(AtomicBool::new(false));
    let stream_writer = input.map(|input| {
        let pipe = child.stdin.take().expect("piped stdin was requested");
        let policy = spec
            .stdin_stream
            .clone()
            .expect("stream policy was checked");
        send_input_stream(
            input,
            pipe,
            policy,
            input_finished.clone(),
            input_failed.clone(),
        )
    });
    let input_writer = stdin_source.map(|mut source| {
        let mut pipe = child.stdin.take().expect("piped stdin was requested");
        thread::spawn(move || match std::io::copy(&mut source, &mut pipe) {
            Ok(_) => Ok(()),
            // A guest may exit without consuming all of its input. Cancellation
            // and time limits also close the namespace's pipe readers.
            Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
            Err(error) => Err(error),
        })
    });
    let truncated = Arc::new(AtomicBool::new(false));
    let trace_remaining = Arc::new(AtomicU64::new(spec.limits.trace_bytes));
    let (stdout_file, stderr_file) = match trace {
        Some(trace) => (Some(trace.stdout), Some(trace.stderr)),
        None => (None, None),
    };
    let out = collect_pipe(
        child.stdout.take().context("missing stdout")?,
        trace_remaining.clone(),
        truncated.clone(),
        stdout_file,
    );
    let err = collect_pipe(
        child.stderr.take().context("missing stderr")?,
        trace_remaining,
        truncated.clone(),
        stderr_file,
    );
    let mut status = "exited";
    let mut private_channel = None;
    let exit = loop {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        if unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        } != 0
        {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(error.into());
        }
        if unsafe { info.si_pid() } != 0 {
            // Stop descendants before reaping while the group ID is reserved.
            if let Some(group) = &group {
                group.kill()?;
            }
            kill_group(child);
            break child.wait()?;
        }
        let channel_failed = channel
            .as_ref()
            .is_some_and(crate::private_channel::Session::failed);
        if cancel.load(Ordering::Acquire) || input_failed.load(Ordering::Acquire) || channel_failed
        {
            private_channel = channel
                .take()
                .map(crate::private_channel::Session::finish)
                .transpose()?;
            status = if channel_failed {
                "channel-failed"
            } else if input_failed.load(Ordering::Acquire) {
                "input-failed"
            } else {
                "cancelled"
            };
            if let Some(group) = &group {
                group.kill()?;
            }
            kill_group(child);
            break child.wait()?;
        }
        if start.elapsed() >= Duration::from_secs(spec.limits.wall_seconds) {
            private_channel = channel
                .take()
                .map(crate::private_channel::Session::finish)
                .transpose()?;
            status = "timed-out";
            if let Some(group) = &group {
                group.kill()?;
            }
            kill_group(child);
            break child.wait()?;
        }
        if group
            .as_ref()
            .map(|group| group.cpu_microseconds())
            .transpose()?
            .is_some_and(|cpu| cpu >= spec.limits.cpu_seconds * 1_000_000)
        {
            status = "cpu-limited";
            private_channel = channel
                .take()
                .map(crate::private_channel::Session::finish)
                .transpose()?;
            group.as_ref().expect("aggregate CPU counter").kill()?;
            kill_group(child);
            break child.wait()?;
        }
        thread::sleep(Duration::from_millis(10));
    };
    owned_child.reaped = true;
    if private_channel.is_none() {
        private_channel = channel
            .map(crate::private_channel::Session::finish)
            .transpose()?;
    }
    if status == "exited"
        && private_channel
            .as_ref()
            .is_some_and(|receipt| receipt.error.is_some())
    {
        status = "channel-failed";
    }
    input_finished.store(true, Ordering::Release);
    let stdin_stream = stream_writer
        .map(|writer| {
            writer
                .join()
                .map_err(|_| anyhow::anyhow!("streamed stdin writer panicked"))
        })
        .transpose()?;
    if input_failed.load(Ordering::Acquire) && status == "exited" {
        status = "input-failed";
    }
    if let Some(writer) = input_writer {
        writer
            .join()
            .map_err(|_| anyhow::anyhow!("stdin writer panicked"))??;
    }
    let stdout = out
        .join()
        .map_err(|_| anyhow::anyhow!("stdout collector panicked"))??;
    let stderr = err
        .join()
        .map_err(|_| anyhow::anyhow!("stderr collector panicked"))??;
    let aggregate_limits = group
        .as_mut()
        .map(|group| group.finish(&spec.limits))
        .transpose()?;
    if status == "exited"
        && aggregate_limits.as_ref().is_some_and(|e| {
            e["memory_events"]["oom_kill"]
                .as_u64()
                .is_some_and(|n| n > 0)
        })
    {
        status = "memory-limited";
    }
    let limits_scope = if aggregate_limits.is_some() {
        "cgroup-v2 aggregate RAM/process count and monitored total CPU budget; one-core bandwidth; per-process rlimits; bounded scratch tmpfs"
    } else {
        "rlimits apply per process; process count applies to mapped user; each scratch tmpfs is bounded; no aggregate cgroup CPU/RAM claim"
    };
    let dashboard = dashboard.map(crate::dashboard::Server::finish);
    let status = if dashboard
        .as_ref()
        .is_some_and(|receipt| receipt["supervisor_healthy"] != true)
    {
        "dashboard-failed"
    } else {
        status
    };
    let isolation = if private_channel.is_some() {
        "user+mount+network+PID+IPC+UTS namespaces; read-only locked files; no inherited host network interface or route; scratch tmpfs; runtime-created private Ethernet carrier; provider topology and lifetime require separate evidence"
    } else {
        "user+mount+network+PID+IPC+UTS namespaces; read-only locked files; no egress; scratch tmpfs; optional run-owned incoming dashboard socket"
    };
    Ok(ProcessResult {
        support_virtualization: virtualization.map(|grant| grant.receipt),
        private_channel,
        stdin_stream,
        dashboard,
        status: status.into(),
        exit_code: exit.code(),
        wall_milliseconds: start.elapsed().as_millis(),
        stdout: hex::encode(stdout),
        stderr: hex::encode(stderr),
        trace_truncated: truncated.load(Ordering::Acquire),
        isolation: isolation.into(),
        limits_scope: limits_scope.into(),
        tools,
        aggregate_limits,
    })
}

struct InputCompletion(Arc<AtomicBool>);
impl Drop for InputCompletion {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

fn send_input_stream(
    input: Receiver<Vec<u8>>,
    mut pipe: impl Write + Send + 'static,
    policy: StdinStream,
    finished: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
) -> thread::JoinHandle<serde_json::Value> {
    thread::spawn(move || {
        let mut sent = Vec::new();
        let result = (|| -> Result<()> {
            while !finished.load(Ordering::Acquire) {
                let frame = match input.recv_timeout(Duration::from_millis(20)) {
                    Ok(frame) => frame,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => break,
                };
                ensure!(
                    !frame.is_empty() && frame.len() as u64 <= policy.max_bytes - sent.len() as u64,
                    "streamed stdin exceeds its declared byte limit"
                );
                let mut offset = 0;
                while offset < frame.len() {
                    match pipe.write(&frame[offset..]) {
                        Ok(0) => anyhow::bail!("streamed stdin write returned zero"),
                        Ok(count) => {
                            sent.extend_from_slice(&frame[offset..offset + count]);
                            offset += count;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(error) => return Err(error.into()),
                    }
                }
            }
            Ok(())
        })();
        if result.is_err() {
            failed.store(true, Ordering::Release);
        }
        serde_json::json!({"protocol":policy.protocol,"max_bytes":policy.max_bytes,
            "sent_hex":hex::encode(sent),"error":result.err().map(|error|format!("{error:#}"))})
    })
}

/// Cancellation and infrastructure errors must release the owned namespace.
/// Never signal a child which has already been reaped, since its PID can be reused.
struct ProcessGuard {
    child: Child,
    reaped: bool,
}
impl Drop for ProcessGuard {
    fn drop(&mut self) {
        if self.reaped {
            return;
        }
        kill_group(&mut self.child);
        let _ = self.child.wait();
    }
}

fn kill_group(child: &mut Child) {
    // The child is in its own group. Never target any unrelated host process.
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
    let _ = child.kill();
}

fn collect_pipe(
    mut pipe: impl Read + Send + 'static,
    remaining: Arc<AtomicU64>,
    truncated: Arc<AtomicBool>,
    mut output: Option<File>,
) -> thread::JoinHandle<Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut saved = Vec::new();
        let mut buffer = [0; 8192];
        loop {
            let n = pipe.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            // Reserve space before writing either representation. Relaxed
            // ordering is sufficient: only the numeric allowance is shared.
            let before = remaining
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |available| {
                    Some(available.saturating_sub(n as u64))
                })
                .expect("reservation always supplies a new value");
            let keep = n.min(before as usize);
            if let Some(file) = &mut output {
                file.write_all(&buffer[..keep])?;
            }
            saved.extend_from_slice(&buffer[..keep]);
            if keep != n {
                truncated.store(true, Ordering::Release);
            }
        }
        if let Some(file) = output {
            file.sync_all()?;
        }
        Ok(saved)
    })
}

pub fn save_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut suffix = [0u8; 12];
    File::open("/dev/urandom")?.read_exact(&mut suffix)?;
    let temp = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        hex::encode(suffix)
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let result = (|| -> Result<()> {
        serde_json::to_writer_pretty(&mut file, value)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_upload_trace_has_an_explicit_hard_ceiling() {
        let mut limits = Limits::default();
        assert_eq!(limits.trace_bytes, 1024 * 1024);
        limits.trace_bytes = 128 * 1024 * 1024;
        assert!(limits.validate().is_ok());
        for bytes in [0, 128 * 1024 * 1024 + 1, u64::MAX] {
            limits.trace_bytes = bytes;
            assert!(limits.validate().is_err());
        }
    }
    #[test]
    fn unequal_trace_streams_share_the_bound_and_still_detect_overflow() {
        for (stdout, stderr, limit, expect_truncation) in [
            (45usize, 10usize, 64u64, false),
            (0, 64, 64, false),
            (64, 0, 64, false),
            (8193, 8193, 8193, true),
            (1, 1, 1, true),
        ] {
            let remaining = Arc::new(AtomicU64::new(limit));
            let truncated = Arc::new(AtomicBool::new(false));
            let a = collect_pipe(
                std::io::Cursor::new(vec![b'a'; stdout]),
                remaining.clone(),
                truncated.clone(),
                None,
            );
            let b = collect_pipe(
                std::io::Cursor::new(vec![b'b'; stderr]),
                remaining.clone(),
                truncated.clone(),
                None,
            );
            let (a, b) = (a.join().unwrap().unwrap(), b.join().unwrap().unwrap());
            assert_eq!(a.len() + b.len(), (stdout + stderr).min(limit as usize));
            assert!(a.iter().all(|byte| *byte == b'a'));
            assert!(b.iter().all(|byte| *byte == b'b'));
            assert_eq!(truncated.load(Ordering::Acquire), expect_truncation);
            assert_eq!(
                remaining.load(Ordering::Relaxed),
                limit - (a.len() + b.len()) as u64
            );
        }
    }

    #[test]
    fn guest_mounts_cannot_replace_runtime_mounts() {
        for s in [
            "/",
            "/tmp",
            "/tmp/a",
            "/proc/self/fd/1",
            "/a/../tmp/a",
            "/a//b",
            "/a/./b",
            "a",
        ] {
            assert!(validate_guest_path(s).is_err(), "{s}");
        }
        assert!(validate_guest_path("/lib64/ld-linux-x86-64.so.2").is_ok());
    }
}

use crate::store::hash_file;
use serde_json::{json, Value};
use std::fs;
use std::net::TcpListener;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn inspect() -> Value {
    inspect_with_core(None)
}

/// Inspect a selected Core executable without running it on the host.
/// Hash availability is separate from the isolated node execution tests.
pub fn inspect_with_core(bitcoin_core: Option<&Path>) -> Value {
    let tcp = match TcpListener::bind("127.0.0.1:0") {
        Ok(s) => json!({"available":true, "address":s.local_addr().map(|a|a.to_string()).ok()}),
        Err(e) => {
            json!({"available":false, "error":e.to_string(), "errno":e.raw_os_error(), "blocks":["TCP test pool", "Bitcoin Core RPC", "SSH transport", "HTTP dashboards", "TLS gateway"]})
        }
    };
    let unix = match UnixStream::pair() {
        Ok((stream, _)) => {
            match stream.set_read_timeout(Some(std::time::Duration::from_millis(10))) {
                Ok(()) => {
                    json!({"pair_creation":true,"standard_socket_timeout":true,"listener_tested":false})
                }
                Err(e) => {
                    json!({"pair_creation":true,"standard_socket_timeout":false,"errno":e.raw_os_error(),"error":e.to_string(),"listener_tested":false,"permitted_transport":"Private inherited file-descriptor IPC is tested separately. It is not a network listener."})
                }
            }
        }
        Err(e) => json!({"available":false,"error":e.to_string()}),
    };
    let namespace = Command::new("/usr/bin/unshare")
        .args([
            "--user",
            "--map-root-user",
            "--net",
            "--mount",
            "--pid",
            "--fork",
            "/usr/bin/true",
        ])
        .stdin(Stdio::null())
        .output();
    let namespace = match namespace {
        Ok(o) => {
            json!({"available":o.status.success(),"stderr":String::from_utf8_lossy(&o.stderr)})
        }
        Err(e) => json!({"available":false,"error":e.to_string()}),
    };
    let mut tools = serde_json::Map::new();
    for (name, path) in [
        ("bubblewrap", Path::new("/usr/bin/bwrap")),
        ("unshare", Path::new("/usr/bin/unshare")),
        ("prlimit", Path::new("/usr/bin/prlimit")),
        ("qemu-arm", Path::new("/usr/bin/qemu-system-arm")),
        ("qemu-xtensa", Path::new("/usr/bin/qemu-system-xtensa")),
        ("ssh", Path::new("/usr/bin/ssh")),
        (
            "bitcoind",
            bitcoin_core.unwrap_or(Path::new("/usr/bin/bitcoind")),
        ),
    ] {
        tools.insert(
            name.into(),
            match hash_file(path) {
                Ok(id) => json!({"path":path,"identity":id}),
                Err(e) => json!({"path":path,"error":e.to_string()}),
            },
        );
    }
    let incomplete = !tcp["available"].as_bool().unwrap_or(false)
        || !namespace["available"].as_bool().unwrap_or(false)
        || ["bubblewrap", "unshare", "prlimit", "bitcoind", "ssh"]
            .iter()
            .any(|name| tools[*name].get("error").is_some());
    let mut disk = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // The constant path is NUL-terminated; statvfs initializes this structure.
    let disk = if unsafe { libc::statvfs(c".".as_ptr(), disk.as_mut_ptr()) } == 0 {
        let disk = unsafe { disk.assume_init() };
        json!({"available_bytes":disk.f_bavail.saturating_mul(disk.f_frsize),"path":"working directory"})
    } else {
        json!({"error":std::io::Error::last_os_error().to_string()})
    };
    json!({"schema_version":"2.0", "platform":std::env::consts::OS, "architecture":std::env::consts::ARCH,
        "infrastructure_incomplete":incomplete, "tcp_loopback":tcp, "unix_sockets":unix, "namespaces":namespace,
        "tools":tools, "memory":fs::read_to_string("/proc/meminfo").unwrap_or_default(), "disk":disk,
        "isolation_limits":"Per-process rlimits and bounded scratch tmpfs are implemented. An explicitly delegated cgroup-v2 parent enables aggregate RAM and process limits, one-core CPU bandwidth and a monitored total CPU budget. This inspection does not establish delegation or run the aggregate fault suite.",
        "firmware_gate":"not-run", "native_dashboard_gate":"not-run", "original_board_gate":"not-run",
        "note":"Installed CPU emulators do not establish support for a miner board. Tool identities are observed hashes, not release authentication."})
}

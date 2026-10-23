use md_sim_runtime::{
    isolation::{execute, execute_with_private_channel, Limits, Mount, ProcessResult, ProcessSpec},
    private_channel::{Protocol, Spec},
    store::{hash_file, Store},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[path = "private_channel/provider.rs"]
mod provider;
#[path = "private_channel/worker.rs"]
mod worker;

fn frame(tag: u8, size: usize) -> Vec<u8> {
    let mut out = (size as u32).to_be_bytes().to_vec();
    out.extend((0..size).map(|n| tag.wrapping_add(n as u8)));
    out
}

#[test]
#[ignore = "guest entry; exercised by private_ethernet_behavior"]
fn guest_default_descriptor_fixture() {
    assert!(std::env::var("HOST_MARKER").is_ok());
    for fd in [3, 64, 222] {
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::EBADF)
        );
    }
}

// This ignored entry is the real guest executable, invoked only through the
// isolated runtime. The host test binary and its ELF libraries are locked inputs.
#[test]
#[ignore = "guest entry; exercised by private_ethernet_behavior"]
fn guest_channel_fixture() {
    let mode = std::env::var("CHANNEL_FIXTURE_MODE").unwrap();
    assert!(!std::path::Path::new(&std::env::var("HOST_MARKER").unwrap()).exists());
    assert!(!std::path::Path::new("/etc/passwd").exists());
    let fd_path = fs::read_link("/proc/self/fd/3").unwrap();
    assert!(fd_path.to_string_lossy().starts_with("socket:["));
    assert!(fs::read_dir("/proc/self/fd/3").is_err());
    assert_eq!(unsafe { libc::fcntl(222, libc::F_GETFD) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EBADF)
    );
    let mut stat: libc::stat = unsafe { std::mem::zeroed() };
    assert_eq!(unsafe { libc::fstat(3, &mut stat) }, 0);
    assert_eq!(stat.st_mode & libc::S_IFMT, libc::S_IFSOCK);
    let mut link = unsafe { File::from_raw_fd(3) };
    match mode.as_str() {
        "duplex" => {
            // Different fragmentation in both directions. This is an Ethernet
            // framing oracle; it does not inspect the relay's implementation.
            let outbound = frame(11, 1200);
            for bytes in outbound.chunks(7) {
                link.write_all(bytes).unwrap();
            }
            let expected = frame(201, 1500);
            let mut actual = vec![0; expected.len()];
            link.read_exact(&mut actual).unwrap();
            assert_eq!(actual, expected);
            link.write_all(&frame(42, 14)).unwrap();
            let mut ack = vec![0; 18];
            link.read_exact(&mut ack).unwrap();
            assert_eq!(ack, frame(43, 14));
        }
        "oversized" => {
            link.write_all(&2049u32.to_be_bytes()).unwrap();
            thread::sleep(Duration::from_secs(10));
        }
        "partial" => {
            link.write_all(&frame(0, 64)[..11]).unwrap();
            drop(link);
            thread::sleep(Duration::from_secs(10));
        }
        "two" => {
            link.write_all(&frame(21, 64)).unwrap();
            link.write_all(&frame(22, 64)).unwrap();
            thread::sleep(Duration::from_secs(10));
        }
        "backpressure" => {
            // A blocked guest write must be stopped by the runtime deadline or
            // cancellation. Keep one descendant in the namespace as well.
            let pid = unsafe { libc::fork() };
            assert!(pid >= 0);
            if pid == 0 {
                loop {
                    thread::sleep(Duration::from_secs(1));
                }
            }
            let bytes = frame(23, 2048);
            loop {
                if link.write_all(&bytes).is_err() {
                    break;
                }
            }
            thread::sleep(Duration::from_secs(10));
        }
        "peer-close" => {
            let mut byte = [0];
            assert_eq!(link.read(&mut byte).unwrap(), 0);
        }
        "no-read" => {
            println!("PRIVATE_NO_READ_LIVE");
            thread::sleep(Duration::from_secs(10));
        }
        "parent-loss" => {
            let pid = unsafe { libc::fork() };
            assert!(pid >= 0);
            if pid != 0 {
                link.write_all(&frame(52, 64)).unwrap();
                println!("PRIVATE_CARRIER_AND_DESCENDANT_LIVE");
            }
            loop {
                thread::sleep(Duration::from_secs(1));
            }
        }
        other => panic!("unknown guest mode: {other}"),
    }
    println!("PRIVATE_ETHERNET_GUEST_PASS {mode}");
}

fn inputs(store: &Store) -> Vec<Mount> {
    let binary = std::env::current_exe().unwrap();
    // ldd is used only on this trusted, locally built test executable.
    let output = std::process::Command::new("/usr/bin/ldd")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(output.status.success());
    let mut files = BTreeMap::from([("/channel-fixture".into(), binary)]);
    for field in String::from_utf8(output.stdout).unwrap().split_whitespace() {
        if field.starts_with('/') {
            files.insert(field.to_string(), PathBuf::from(field));
        }
    }
    files
        .into_iter()
        .map(|(guest_path, source)| {
            let object = hash_file(&source).unwrap();
            store.import(&source, &object).unwrap();
            Mount { object, guest_path }
        })
        .collect()
}

fn policy() -> Spec {
    Spec {
        protocol: Protocol::QemuEthernetV1,
        guest_fd: 3,
        max_frame_bytes: 2048,
        max_wire_bytes: 16 << 20,
        max_packets: 65536,
    }
}

fn spec(store: &Store, host_marker: &str) -> ProcessSpec {
    ProcessSpec {
        private_channel: Some(policy()),
        stdin_stream: None,
        namespace_uid: 0,
        schema_version: "2.0".into(),
        argv: vec![
            "/channel-fixture".into(),
            "--ignored".into(),
            "--exact".into(),
            "guest_channel_fixture".into(),
            "--nocapture".into(),
            "--test-threads=1".into(),
        ],
        inputs: inputs(store),
        limits: Limits {
            wall_seconds: 3,
            memory_bytes: 512 << 20,
            processes: 128,
            trace_bytes: 16384,
            ..Limits::default()
        },
        environment: BTreeMap::from([("HOST_MARKER".into(), host_marker.into())]),
        loopback: false,
        dashboard: None,
        stdin_object: None,
    }
}

fn nonblocking(file: &File) {
    let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
        0
    );
}

// Provider I/O has its own deadline; a broken runtime must fail this test rather
// than leave a blocked test thread. No recv/send/getsockopt/shutdown is needed.
fn transfer(file: &mut File, send: &[u8], receive: usize) -> Vec<u8> {
    nonblocking(file);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut written = 0;
    let mut output = Vec::new();
    loop {
        assert!(Instant::now() < deadline, "provider deadline");
        if written < send.len() {
            match file.write(&send[written..(written + 13).min(send.len())]) {
                Ok(n) => written += n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => panic!("provider write: {e}"),
            }
        }
        let mut buffer = [0; 4096];
        match file.read(&mut buffer) {
            Ok(0) => return output,
            Ok(n) => output.extend_from_slice(&buffer[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("provider read: {e}"),
        }
        if written == send.len() && output.len() >= receive {
            return output;
        }
        thread::sleep(Duration::from_millis(1));
    }
}

fn run<F>(
    store: &Store,
    base: &ProcessSpec,
    mode: &str,
    cancel: Arc<AtomicBool>,
    provider: F,
) -> ProcessResult
where
    F: FnOnce(File) + Send + 'static,
{
    let mut contract = base.clone();
    contract
        .environment
        .insert("CHANNEL_FIXTURE_MODE".into(), mode.into());
    let (sender, receiver) = mpsc::sync_channel(1);
    let handler =
        thread::spawn(move || provider(receiver.recv_timeout(Duration::from_secs(5)).unwrap()));
    let result = execute_with_private_channel(store, &contract, cancel, None, None, sender);
    if let Ok(result) = &result {
        println!(
            "PRIVATE_CHANNEL_EVIDENCE {}",
            serde_json::json!({"mode":mode,"spec":contract,"result":result})
        );
    }
    handler.join().unwrap();
    result.unwrap()
}

fn check_ok(result: &ProcessResult) {
    assert_eq!(result.status, "exited", "{result:?}");
    assert_eq!(
        result.exit_code,
        Some(0),
        "{}",
        String::from_utf8_lossy(&hex::decode(&result.stderr).unwrap())
    );
    assert!(!result.trace_truncated);
    assert!(String::from_utf8(hex::decode(&result.stdout).unwrap())
        .unwrap()
        .contains("PRIVATE_ETHERNET_GUEST_PASS"));
}

#[test]
fn private_ethernet_behavior() {
    let temp = tempfile::tempdir().unwrap();
    let marker = temp.path().join("host-only-marker");
    fs::write(&marker, b"never mount this file").unwrap();
    // An embedding program can have a host file without CLOEXEC. It must not
    // cross either the default or the explicit carrier isolation boundary.
    let marker_file = File::open(&marker).unwrap();
    let inherited = unsafe { libc::fcntl(marker_file.as_raw_fd(), libc::F_DUPFD, 222) };
    assert_eq!(inherited, 222);
    let _inherited = unsafe { File::from_raw_fd(inherited) };
    let store = Store::open(&temp.path().join("store"), 128 << 20).unwrap();
    let mut contract = spec(&store, marker.to_str().unwrap());
    let idle = || Arc::new(AtomicBool::new(false));

    // Existing run identities omit the new field exactly. Ordinary execute
    // must refuse a declared carrier without an explicit supervisor provider.
    assert!(execute(&store, &contract, idle())
        .unwrap_err()
        .to_string()
        .contains("matching supervisor provider"));
    contract.private_channel = None;
    let old = serde_json::to_value(&contract).unwrap();
    assert!(old.get("private_channel").is_none());
    let old_decoded: ProcessSpec = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(old_decoded).unwrap(), old);
    let mut no_channel = contract.clone();
    no_channel.argv = vec![
        "/channel-fixture".into(),
        "--ignored".into(),
        "--exact".into(),
        "guest_default_descriptor_fixture".into(),
        "--nocapture".into(),
        "--test-threads=1".into(),
    ];
    let result = execute(&store, &no_channel, idle()).unwrap();
    assert_eq!(result.exit_code, Some(0), "{result:?}");
    assert!(result.private_channel.is_none());
    let (sender, _receiver) = mpsc::sync_channel(1);
    assert!(execute_with_private_channel(&store, &contract, idle(), None, None, sender).is_err());
    contract.private_channel = Some(policy());
    for invalid in [0, 2, 4, 64] {
        let mut invalid_policy = policy();
        invalid_policy.guest_fd = invalid;
        assert!(invalid_policy.validate().is_err());
    }
    let (sender, receiver) = mpsc::sync_channel(1);
    drop(receiver);
    assert!(execute_with_private_channel(&store, &contract, idle(), None, None, sender).is_err());

    let result = run(&store, &contract, "duplex", idle(), |mut file| {
        let mut sent = frame(201, 1500);
        sent.extend(frame(43, 14));
        let mut expected = frame(11, 1200);
        expected.extend(frame(42, 14));
        let actual = transfer(&mut file, &sent, expected.len());
        assert_eq!(actual.len(), expected.len());
        assert_eq!(Sha256::digest(&actual), Sha256::digest(&expected));
        // Observe a real guest EOF instead of dropping the provider early.
        assert!(transfer(&mut file, &[], usize::MAX).is_empty());
    });
    check_ok(&result);
    let receipt = result.private_channel.unwrap();
    for (direction, tag, size, ack) in [
        (&receipt.guest_to_peer, 11, 1200, 42),
        (&receipt.peer_to_guest, 201, 1500, 43),
    ] {
        let mut wire = frame(tag, size);
        wire.extend(frame(ack, 14));
        assert_eq!(direction.observed_bytes, wire.len() as u64);
        assert_eq!(direction.forwarded_bytes, wire.len() as u64);
        assert_eq!(
            direction.observed_sha256,
            hex::encode(Sha256::digest(&wire))
        );
        assert_eq!(
            direction.forwarded_sha256,
            hex::encode(Sha256::digest(&wire))
        );
        assert_eq!(direction.forwarded_packets, 2);
        assert_eq!(direction.received_packets, 2);
        assert_eq!(direction.pending_bytes, 0);
    }
    assert!(receipt.error.is_none());
    assert_eq!(receipt.end, "guest-eof");

    let result = run(&store, &contract, "peer-close", idle(), drop);
    check_ok(&result);
    let receipt = result.private_channel.unwrap();
    assert_eq!(receipt.end, "peer-eof");
    assert!(receipt.peer_to_guest.eof);

    for (mode, expected_error, max_packets, max_bytes, expected_forwarded) in [
        ("oversized", "oversized", 65536, 16 << 20, 0),
        ("partial", "partial frame", 65536, 16 << 20, 0),
        ("two", "packet limit", 1, 16 << 20, 68),
        ("two", "byte limit", 65536, 68, 68),
    ] {
        let limits = contract.private_channel.as_mut().unwrap();
        limits.max_packets = max_packets;
        limits.max_wire_bytes = max_bytes;
        let result = run(&store, &contract, mode, idle(), move |mut file| {
            assert_eq!(
                transfer(&mut file, &[], usize::MAX).len(),
                expected_forwarded
            );
        });
        assert_eq!(result.status, "channel-failed", "{result:?}");
        let receipt = result.private_channel.unwrap();
        assert!(
            receipt.error.as_ref().unwrap().contains(expected_error),
            "{receipt:?}"
        );
        assert_eq!(
            receipt.guest_to_peer.forwarded_bytes,
            expected_forwarded as u64
        );
        assert!(result.wall_milliseconds < 2500);
    }

    contract.private_channel = Some(policy());
    contract.limits.wall_seconds = 1;
    let result = run(&store, &contract, "no-read", idle(), |mut file| {
        nonblocking(&file);
        let bytes = frame(30, 2048).repeat(1024);
        let deadline = Instant::now() + Duration::from_millis(250);
        let mut written = 0;
        while Instant::now() < deadline && written < bytes.len() {
            match file.write(&bytes[written..]) {
                Ok(n) => written += n,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2))
                }
                Err(e) => panic!("closed-backpressure provider: {e}"),
            }
        }
        assert!(written > 4 * 2052, "must fill more than one relay frame");
        // A closed source with a complete frame blocked toward the guest must
        // not cause a zero-interest POLLHUP poll loop on the host.
        drop(file);
    });
    assert_eq!(result.status, "timed-out", "{result:?}");
    let receipt = result.private_channel.unwrap();
    let cpu_used = receipt
        .relay_cpu_microseconds
        .expect("kernel relay-thread CPU accounting");
    println!("PRIVATE_CHANNEL_CLOSED_BACKPRESSURE_RELAY_CPU_US {cpu_used}");
    assert!(cpu_used < 400_000, "closed carrier spun host CPU");
    assert!(
        receipt.peer_to_guest.pending_bytes > 0,
        "must exercise relay backpressure"
    );
    assert_eq!(
        receipt.peer_to_guest.received_packets,
        receipt.peer_to_guest.forwarded_packets + 1
    );
    assert!(receipt.error.is_none());
    let started = Instant::now();
    let result = run(&store, &contract, "backpressure", idle(), |file| {
        thread::sleep(Duration::from_millis(1400));
        drop(file);
    });
    assert_eq!(result.status, "timed-out", "{result:?}");
    assert!(started.elapsed() < Duration::from_secs(4));
    let receipt = result.private_channel.unwrap();
    assert_eq!(receipt.end, "supervisor-closed");
    assert!(receipt.error.is_none());
    assert!(receipt.guest_to_peer.pending_bytes <= 2052);
    assert!(receipt.guest_to_peer.forwarded_bytes < 16 << 20);

    contract.limits.wall_seconds = 10;
    let cancel = idle();
    let stop = cancel.clone();
    let result = run(&store, &contract, "backpressure", cancel, move |file| {
        thread::sleep(Duration::from_millis(250));
        stop.store(true, Ordering::Release);
        // Hold the provider endpoint past cancellation. Runtime must return
        // without waiting on the provider's lifetime or a blocked writer.
        thread::sleep(Duration::from_millis(350));
        drop(file);
    });
    assert_eq!(result.status, "cancelled", "{result:?}");
    assert!(result.wall_milliseconds < 1500);
    assert_eq!(result.private_channel.unwrap().end, "supervisor-closed");
    assert_eq!(fs::read(marker).unwrap(), b"never mount this file");
}

#[test]
#[ignore = "independent provider entry; exercised by carrier_supervisor_loss"]
fn host_provider_fixture() {
    let root = PathBuf::from(std::env::var_os("CHANNEL_PARENT_LOSS_ROOT").unwrap());
    fs::write(root.join("provider-ready"), std::process::id().to_string()).unwrap();
    let mut file = unsafe { File::from_raw_fd(0) };
    let bytes = transfer(&mut file, &[], 68);
    assert_eq!(bytes, frame(52, 64));
    fs::write(root.join("provider-received"), "actual carrier frame").unwrap();
    assert!(transfer(&mut file, &[], usize::MAX).is_empty());
    fs::write(
        root.join("provider-eof"),
        hex::encode(Sha256::digest(&bytes)),
    )
    .unwrap();
}

fn descendants(pid: u32, output: &mut Vec<u32>) {
    if let Ok(tasks) = fs::read_dir(format!("/proc/{pid}/task")) {
        for task in tasks.flatten() {
            let children = fs::read_to_string(task.path().join("children")).unwrap_or_default();
            for child in children
                .split_whitespace()
                .map(|s| s.parse::<u32>().unwrap())
            {
                if !output.contains(&child) {
                    output.push(child);
                    descendants(child, output);
                }
            }
        }
    }
}

fn process_identity(pid: u32) -> Option<(bool, String)> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields: Vec<_> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    Some((fields.first()? != &"Z", fields.get(19)?.to_string()))
}

#[test]
fn carrier_supervisor_loss() {
    const CHILD_ENV: &str = "CHANNEL_PARENT_LOSS_ROOT";
    if let Some(root) = std::env::var_os(CHILD_ENV) {
        let root = PathBuf::from(root);
        let store = Store::open(&root.join("store"), 128 << 20).unwrap();
        let mut contract = spec(&store, root.join("marker").to_str().unwrap());
        contract
            .environment
            .insert("CHANNEL_FIXTURE_MODE".into(), "parent-loss".into());
        contract.limits.wall_seconds = 30;
        let (sender, receiver) = mpsc::sync_channel(1);
        let provider_root = root.clone();
        thread::spawn(move || {
            let file = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
            // This trusted host test process independently holds the endpoint.
            // It must outlive abrupt loss of the Rust runtime supervisor.
            let provider = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--ignored",
                    "--exact",
                    "host_provider_fixture",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(CHILD_ENV, provider_root)
                .stdin(std::process::Stdio::from(file))
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .unwrap();
            drop(provider);
        });
        let trace = md_sim_runtime::isolation::TraceFiles {
            stdout: File::create(root.join("guest.stdout")).unwrap(),
            stderr: File::create(root.join("guest.stderr")).unwrap(),
        };
        execute_with_private_channel(
            &store,
            &contract,
            Arc::new(AtomicBool::new(false)),
            Some(trace),
            None,
            sender,
        )
        .unwrap();
        panic!("supervisor must be killed while the guest is live");
    }
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join("marker"), "host-only marker").unwrap();
    struct OwnedProcesses {
        supervisor: std::process::Child,
        descendants: Vec<(u32, String)>,
    }
    impl Drop for OwnedProcesses {
        fn drop(&mut self) {
            let _ = self.supervisor.kill();
            let _ = self.supervisor.wait();
            for (pid, identity) in &self.descendants {
                if process_identity(*pid).is_some_and(|(live, start)| live && start == *identity) {
                    unsafe {
                        libc::kill(*pid as i32, libc::SIGKILL);
                    }
                }
            }
        }
    }
    let mut owned = OwnedProcesses {
        supervisor: std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "carrier_supervisor_loss",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD_ENV, temp.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap(),
        descendants: Vec::new(),
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let live = fs::read_to_string(temp.path().join("guest.stdout")).unwrap_or_default();
        if live.contains("PRIVATE_CARRIER_AND_DESCENDANT_LIVE")
            && temp.path().join("provider-received").exists()
        {
            break;
        }
        assert!(
            Instant::now() < deadline && owned.supervisor.try_wait().unwrap().is_none(),
            "guest did not reach live carrier marker"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let provider: u32 = fs::read_to_string(temp.path().join("provider-ready"))
        .unwrap()
        .parse()
        .unwrap();
    let mut pids = Vec::new();
    descendants(owned.supervisor.id(), &mut pids);
    assert!(pids.contains(&provider));
    let guests: Vec<_> = pids
        .iter()
        .copied()
        .filter(|pid| *pid != provider)
        .collect();
    assert!(
        guests.len() >= 4,
        "expected monitors, guest, and descendant: {pids:?}"
    );
    owned.descendants = pids
        .into_iter()
        .filter_map(|pid| process_identity(pid).map(|(_, start)| (pid, start)))
        .collect();
    owned.supervisor.kill().unwrap();
    owned.supervisor.wait().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let live: Vec<_> = guests
            .iter()
            .filter(|pid| process_identity(**pid).is_some_and(|(live, _)| live))
            .collect();
        if live.is_empty() && temp.path().join("provider-eof").exists() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "carrier guest descendants or independent provider survived supervisor loss: {live:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        fs::read_to_string(temp.path().join("provider-eof")).unwrap(),
        hex::encode(Sha256::digest(frame(52, 64)))
    );
    println!(
        "PRIVATE_CHANNEL_SUPERVISOR_LOSS {}",
        serde_json::json!({"guest_processes":guests,"provider_pid":provider,"all_guests_dead":true,"provider_eof_sha256":hex::encode(Sha256::digest(frame(52,64)))})
    );
}

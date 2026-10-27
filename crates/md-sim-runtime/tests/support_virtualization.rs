use md_sim_runtime::{
    isolation::{
        execute, execute_support_vm, execute_support_vm_with_streamed_input,
        execute_with_streamed_input, Limits, Mount, ProcessSpec, StdinStream,
        SupportVirtualization,
    },
    store::{hash_file, Store},
};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Read,
    os::{
        fd::AsRawFd,
        unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, mpsc, Arc},
};

// The child is this trusted local test binary. It never executes acquired code.
fn inputs(store: &Store) -> Vec<Mount> {
    let binary = std::env::current_exe().unwrap();
    let output = std::process::Command::new("/usr/bin/ldd")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(output.status.success());
    let mut files = BTreeMap::from([("/fixture".to_owned(), binary)]);
    for word in String::from_utf8(output.stdout).unwrap().split_whitespace() {
        if word.starts_with('/') {
            files.insert(word.into(), PathBuf::from(word));
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

fn spec(store: &Store, marker: &Path, mode: &str, uid: u32) -> ProcessSpec {
    ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: uid,
        schema_version: "2.0".into(),
        argv: vec![
            "/fixture".into(),
            "--ignored".into(),
            "--exact".into(),
            "guest_support_device_fixture".into(),
            "--nocapture".into(),
        ],
        inputs: inputs(store),
        stdin_object: None,
        dashboard: None,
        loopback: false,
        environment: BTreeMap::from([
            ("SUPPORT_MODE".into(), mode.into()),
            ("SUPPORT_UID".into(), uid.to_string()),
            ("HOST_MARKER".into(), marker.display().to_string()),
        ]),
        limits: Limits {
            processes: 16,
            tmp_bytes: 1 << 20,
            file_bytes: 1 << 20,
            ..Limits::default()
        },
    }
}

#[test]
#[ignore = "isolated child entry, called by the support virtualization tests"]
fn guest_support_device_fixture() {
    for fd in [3, 64, 222] {
        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    }
    for path in [
        "/etc/passwd",
        "/home/user",
        "/sys",
        "/dev/mem",
        "/dev/sda",
        "/dev/dri",
        "/dev/net/tun",
    ] {
        assert!(!Path::new(path).exists(), "unexpected host path {path}");
    }
    assert!(!Path::new(&std::env::var("HOST_MARKER").unwrap()).exists());
    assert!(fs::write("/outside-scratch", b"no").is_err());
    fs::write("/tmp/inside", b"yes").unwrap();
    fs::write("/work/inside", b"yes").unwrap();
    let status = fs::read_to_string("/proc/self/status").unwrap();
    let uid: u32 = std::env::var("SUPPORT_UID").unwrap().parse().unwrap();
    assert_eq!(unsafe { libc::geteuid() }, uid);
    for field in ["CapInh:", "CapPrm:", "CapEff:", "CapAmb:"] {
        let value = status
            .lines()
            .find_map(|line| line.strip_prefix(field))
            .unwrap()
            .trim();
        assert_eq!(u64::from_str_radix(value, 16).unwrap(), 0, "guest {field}");
    }
    let bounding = u64::from_str_radix(
        status
            .lines()
            .find_map(|line| line.strip_prefix("CapBnd:"))
            .unwrap()
            .trim(),
        16,
    )
    .unwrap();
    if uid == 1000 {
        assert_eq!(bounding, 0, "nonroot support bounding set");
    } else {
        assert_eq!(uid, 0);
        if let Ok(expected) = std::env::var("SUPPORT_BOUNDING_BASELINE") {
            assert_eq!(
                bounding,
                expected.parse::<u64>().unwrap(),
                "KVM changed root bounding set"
            );
        }
    }
    println!("BOUNDING_MASK={bounding}");
    assert_eq!(
        status
            .lines()
            .find_map(|line| line.strip_prefix("NoNewPrivs:"))
            .unwrap()
            .trim(),
        "1"
    );
    // A root namespace can retain a nonzero bounding mask while all current
    // privilege sets are empty. Prove it cannot regain CAP_SYS_ADMIN. This
    // also runs for the strict nonroot browser profile.
    #[repr(C)]
    struct Header {
        version: u32,
        pid: i32,
    }
    #[repr(C)]
    struct Caps {
        effective: u32,
        permitted: u32,
        inheritable: u32,
    }
    let header = Header {
        version: 0x20080522,
        pid: 0,
    };
    let caps = [
        Caps {
            effective: 1 << 21,
            permitted: 1 << 21,
            inheritable: 0,
        },
        Caps {
            effective: 0,
            permitted: 0,
            inheritable: 0,
        },
    ];
    assert_eq!(
        unsafe { libc::syscall(libc::SYS_capset, &header as *const Header, caps.as_ptr()) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EPERM)
    );
    assert_eq!(
        unsafe { libc::prctl(libc::PR_CAP_AMBIENT, libc::PR_CAP_AMBIENT_RAISE, 21, 0, 0) },
        -1
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::EPERM)
    );
    let routes = fs::read_to_string("/proc/net/route").unwrap();
    // A protected proc entry can be empty. If a header is present, require
    // that exact header and no route records; never accept a host route.
    if !routes.is_empty() {
        assert_eq!(routes.lines().count(), 1);
        assert_eq!(
            routes.split_whitespace().collect::<Vec<_>>(),
            [
                "Iface",
                "Destination",
                "Gateway",
                "Flags",
                "RefCnt",
                "Use",
                "Metric",
                "Mask",
                "MTU",
                "Window",
                "IRTT"
            ]
        );
    }
    for line in fs::read_to_string("/proc/net/dev").unwrap().lines().skip(2) {
        assert_eq!(line.split_once(':').unwrap().0.trim(), "lo");
    }
    for (kind, expected) in [
        (libc::RLIMIT_NPROC, 16),
        (libc::RLIMIT_AS, 256 << 20),
        (libc::RLIMIT_FSIZE, 1 << 20),
    ] {
        let mut actual = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        assert_eq!(unsafe { libc::getrlimit(kind, &mut actual) }, 0);
        assert_eq!((actual.rlim_cur, actual.rlim_max), (expected, expected));
    }
    let mode = std::env::var("SUPPORT_MODE").unwrap();
    let mounted = fs::read_to_string("/proc/self/mountinfo")
        .unwrap()
        .lines()
        .filter(|line| line.split_whitespace().nth(4) == Some("/dev/kvm"))
        .count();
    if mode == "absent" {
        assert!(!Path::new("/dev/kvm").exists());
        assert_eq!(mounted, 0);
    } else {
        assert_eq!(mounted, 1);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open("/dev/kvm")
            .unwrap();
        let metadata = file.metadata().unwrap();
        assert!(metadata.file_type().is_char_device());
        assert_eq!(
            (libc::major(metadata.rdev()), libc::minor(metadata.rdev())),
            (10, 232)
        );
        assert_eq!(
            unsafe {
                libc::ioctl(
                    file.as_raw_fd(),
                    0xae00 as libc::c_ulong,
                    0 as libc::c_ulong,
                )
            },
            12
        );
        if mode == "stream" {
            let mut bytes = Vec::new();
            std::io::stdin().take(17).read_to_end(&mut bytes).unwrap();
            assert_eq!(bytes, b"kvm-stream\n");
        } else {
            assert_eq!(mode, "kvm");
        }
    }
    println!("SUPPORT_DEVICE_GUEST_PASS {mode}");
}

#[test]
fn default_wire_and_execution_do_not_grant_a_virtualization_device() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(&temp.path().join("store"), 64 << 20).unwrap();
    let marker = temp.path().join("host-marker");
    fs::write(&marker, b"host only").unwrap();
    let mut process = spec(&store, &marker, "absent", 1000);
    let wire = serde_json::to_value(&process).unwrap();
    assert!(wire.get("support_virtualization").is_none());
    let decoded: ProcessSpec = serde_json::from_value(wire.clone()).unwrap();
    assert!(decoded.support_virtualization.is_none());
    assert_eq!(serde_json::to_value(decoded).unwrap(), wire);
    let mut legacy = wire.clone();
    legacy.as_object_mut().unwrap().remove("namespace_uid");
    assert_eq!(
        serde_json::from_value::<ProcessSpec>(legacy)
            .unwrap()
            .namespace_uid,
        0
    );
    for invalid in [
        serde_json::json!("anything"),
        serde_json::json!({"device":"/dev/mem"}),
    ] {
        let mut wrong = wire.clone();
        wrong["support_virtualization"] = invalid;
        assert!(serde_json::from_value::<ProcessSpec>(wrong).is_err());
    }
    let result = execute(&store, &process, Arc::new(AtomicBool::new(false))).unwrap();
    assert_eq!(
        result.exit_code,
        Some(0),
        "{}",
        String::from_utf8_lossy(&hex::decode(&result.stderr).unwrap())
    );
    assert!(result.support_virtualization.is_none());
    assert!(!result.trace_truncated);
    assert!(String::from_utf8(hex::decode(result.stdout).unwrap())
        .unwrap()
        .contains("SUPPORT_DEVICE_GUEST_PASS absent"));
    assert!(execute_support_vm(&store, &process, Arc::new(AtomicBool::new(false)), None).is_err());
    process.support_virtualization = Some(SupportVirtualization::Kvm);
    let denied = execute(&store, &process, Arc::new(AtomicBool::new(false))).unwrap_err();
    assert!(denied
        .to_string()
        .contains("explicit trusted support-VM grant"));
    process.stdin_stream = Some(StdinStream {
        protocol: "test/support/1".into(),
        max_bytes: 16,
    });
    let (_sender, receiver) = mpsc::channel();
    let denied = execute_with_streamed_input(
        &store,
        &process,
        Arc::new(AtomicBool::new(false)),
        None,
        receiver,
    )
    .unwrap_err();
    assert!(denied
        .to_string()
        .contains("explicit trusted support-VM grant"));
}

#[test]
#[ignore = "requires actual /dev/kvm read/write access; run explicitly for the support host"]
fn explicitly_granted_kvm_preserves_isolation_limits_and_stream_protocol() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::open(&temp.path().join("store"), 64 << 20).unwrap();
    let marker = temp.path().join("host-marker");
    fs::write(&marker, b"host only").unwrap();
    let leaked = File::open(&marker).unwrap();
    let fd = unsafe { libc::fcntl(leaked.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 222) };
    assert_eq!(fd, 222);
    // The runtime must close even explicitly inherited host descriptors.
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_SETFD, 0) }, 0);
    struct Close(i32);
    impl Drop for Close {
        fn drop(&mut self) {
            unsafe {
                libc::close(self.0);
            }
        }
    }
    let _close = Close(fd);
    let root_baseline = execute(
        &store,
        &spec(&store, &marker, "absent", 0),
        Arc::new(AtomicBool::new(false)),
    )
    .unwrap();
    assert_eq!(
        root_baseline.exit_code,
        Some(0),
        "{}",
        String::from_utf8_lossy(&hex::decode(&root_baseline.stderr).unwrap())
    );
    assert!(root_baseline.support_virtualization.is_none() && !root_baseline.trace_truncated);
    let baseline_stdout = String::from_utf8(hex::decode(root_baseline.stdout).unwrap()).unwrap();
    assert_eq!(baseline_stdout.matches("BOUNDING_MASK=").count(), 1);
    let baseline: u64 = baseline_stdout
        .split("BOUNDING_MASK=")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    for (uid, mode) in [(1000, "kvm"), (1000, "stream"), (0, "kvm"), (0, "stream")] {
        let mut process = spec(&store, &marker, mode, uid);
        if uid == 0 {
            process
                .environment
                .insert("SUPPORT_BOUNDING_BASELINE".into(), baseline.to_string());
        }
        process.support_virtualization = Some(SupportVirtualization::Kvm);
        let result = if mode == "stream" {
            process.stdin_stream = Some(StdinStream {
                protocol: "test/support/1".into(),
                max_bytes: 16,
            });
            let (sender, receiver) = mpsc::channel();
            sender.send(b"kvm-stream\n".to_vec()).unwrap();
            drop(sender);
            execute_support_vm_with_streamed_input(
                &store,
                &process,
                Arc::new(AtomicBool::new(false)),
                None,
                receiver,
            )
        } else {
            execute_support_vm(&store, &process, Arc::new(AtomicBool::new(false)), None)
        }
        .unwrap();
        assert_eq!(
            result.exit_code,
            Some(0),
            "{}",
            String::from_utf8_lossy(&hex::decode(&result.stderr).unwrap())
        );
        assert!(!result.trace_truncated);
        assert!(String::from_utf8(hex::decode(result.stdout).unwrap())
            .unwrap()
            .contains(&format!("SUPPORT_DEVICE_GUEST_PASS {mode}")));
        let receipt = result.support_virtualization.unwrap();
        assert_eq!(
            (receipt.major, receipt.minor, receipt.api_version),
            (10, 232, 12)
        );
        assert_eq!(receipt.device, "/dev/kvm");
        assert!(!receipt.cache_eligible);
        assert!(!receipt.kernel["release"].is_empty() && receipt.logical_processors > 0);
        assert!(!receipt.cpu_signatures.is_empty() && receipt.boot_id_sha256.len() == 64);
        if mode == "stream" {
            assert_eq!(
                result.stdin_stream.unwrap()["sent_hex"],
                hex::encode(b"kvm-stream\n")
            );
        }
    }
}

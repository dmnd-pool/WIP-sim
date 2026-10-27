//! Probe inherited private QEMU network descriptors. No guest or target gate.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, ProcessSpec},
    store::{hash_file, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs::{self, OpenOptions},
    io::Read,
    os::{
        fd::AsRawFd,
        unix::{fs::OpenOptionsExt, net::UnixStream, process::CommandExt},
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;

struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
fn bounded(path: &Path) -> Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(8193).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 8192, "probe output exceeds bound");
    Ok(String::from_utf8(bytes)?)
}
fn inside() -> Result<()> {
    confinement::require_board()?;
    let mut observations = Vec::new();
    for mode in ["socket", "stream", "guestfwd", "guestfwd-pipe"] {
        let mut pipes = Vec::new();
        if mode == "guestfwd-pipe" {
            for path in ["/work/pool.in", "/work/pool.out"] {
                let name = CString::new(path)?;
                ensure!(
                    unsafe { libc::mkfifo(name.as_ptr(), 0o600) } == 0,
                    "private FIFO creation failed: {}",
                    std::io::Error::last_os_error()
                );
                pipes.push(
                    OpenOptions::new()
                        .read(true)
                        .write(true)
                        .custom_flags(libc::O_NONBLOCK)
                        .open(path)?,
                );
            }
        }
        let (peer, backend) = UnixStream::pair()?;
        let fd = backend.as_raw_fd();
        let mut kind = 0i32;
        let mut size = std::mem::size_of_val(&kind) as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_TYPE,
                (&mut kind as *mut i32).cast(),
                &mut size,
            )
        };
        let get_type = json!({"return":result,"errno":if result == 0 {None} else {std::io::Error::last_os_error().raw_os_error()},"socket_type":if result == 0 {Some(kind)} else {None}});
        let netdev = match mode {
            "socket" => format!("socket,id=private,fd={fd}"),
            "stream" => format!("stream,id=private,addr.type=fd,addr.str={fd}"),
            "guestfwd" | "guestfwd-pipe" => {
                "user,id=private,restrict=on,ipv6=off,guestfwd=tcp:10.0.2.100:3333-chardev:pool"
                    .into()
            }
            _ => unreachable!(),
        };
        let stdout = PathBuf::from(format!("/work/{mode}.stdout"));
        let stderr = PathBuf::from(format!("/work/{mode}.stderr"));
        let mut args = vec![
            "-machine".into(),
            "none".into(),
            "-nodefaults".into(),
            "-no-user-config".into(),
            "-display".into(),
            "none".into(),
            "-monitor".into(),
            "none".into(),
            "-serial".into(),
            "none".into(),
            "-S".into(),
            "-nic".into(),
            "none".into(),
            "-netdev".into(),
            netdev,
            "-sandbox".into(),
            "on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny".into(),
        ];
        if mode == "guestfwd" {
            args.extend(["-chardev".into(), format!("socket,id=pool,fd={fd}")]);
        } else if mode == "guestfwd-pipe" {
            args.extend(["-chardev".into(), "pipe,id=pool,path=/work/pool".into()]);
        }
        let mut command = Command::new("/qemu");
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(fs::File::create(&stdout)?)
            .stderr(fs::File::create(&stderr)?);
        let parent = std::process::id() as libc::pid_t;
        unsafe {
            command.pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                    || libc::getppid() != parent
                    || libc::fcntl(fd, libc::F_SETFD, 0) < 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = Owned(command.spawn()?);
        drop(backend);
        let start = Instant::now();
        let early_exit = loop {
            if let Some(status) = child.0.try_wait()? {
                break Some(status);
            }
            if start.elapsed() >= Duration::from_secs(1) {
                break None;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let stayed_running = early_exit.is_none();
        if stayed_running {
            child.0.kill()?;
            child.0.wait()?;
        }
        drop(peer);
        drop(pipes);
        observations.push(json!({"mode":mode,"argv":args,"getsockopt":get_type,"stayed_running_for_one_second":stayed_running,"early_exit":early_exit.map(|s|s.to_string()),"intentional_stop_after_probe":stayed_running,"stdout":bounded(&stdout)?,"stderr":bounded(&stderr)?,"guest_executed":false,"packet_exchange_verified":false}));
    }
    println!(
        "{}",
        json!({"scope":"private inherited socket backend initialization only; no guest or packet-transfer claim","qemu":hash_file(Path::new("/qemu"))?,"observations":observations})
    );
    Ok(())
}
fn host(out: &Path) -> Result<()> {
    fs::create_dir_all(out)?;
    let store = Store::open(&out.join("input-store"), 64 * 1024 * 1024)?;
    let inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/probe".into(), std::env::current_exe()?),
            ("/qemu".into(), PathBuf::from("/usr/bin/qemu-system-x86_64")),
        ],
    )?;
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec!["/probe".into(), "__inside".into()],
        inputs,
        environment: BTreeMap::new(),
        stdin_object: None,
        limits: Limits {
            wall_seconds: 10,
            cpu_seconds: 5,
            memory_bytes: 3 * 1024 * 1024 * 1024,
            processes: 32,
            file_bytes: 1024 * 1024,
            trace_bytes: 65536,
            tmp_bytes: 16 * 1024 * 1024,
        },
    };
    save_json(&out.join("process-spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated && result.stderr.is_empty(),
        "isolated descriptor probe failed"
    );
    let report: Value = serde_json::from_slice(&hex::decode(result.stdout)?)?;
    save_json(
        &out.join("assessment.json"),
        &json!({"probe_completed":true,"observations":report,"firmware_execution_gate":false,"remote_transport_gate":false,"packet_exchange_gate":false,"process_spec":hash_file(&out.join("process-spec.json"))?}),
    )?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}
fn main() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.get(1).is_some_and(|v| v == "__inside") {
        return inside();
    }
    ensure!(args.len() == 2, "usage: qemu_socket_link_probe NEW_OUTPUT");
    let out = Path::new(&args[1]);
    ensure!(!out.try_exists()?, "output must be new");
    let result = host(out);
    if let Err(error) = &result {
        if out.is_dir() {
            save_json(
                &out.join("failure.json"),
                &json!({"probe_completed":false,"error":format!("{error:#}")}),
            )
            .context("save socket probe failure")?;
        }
    }
    result
}

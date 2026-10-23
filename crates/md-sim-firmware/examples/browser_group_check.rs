//! Adversarial process-lifecycle check in the disconnected Linux test VM.
#[path = "support/browser_groups.rs"]
mod browser_groups;
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{isolation::save_json, store::hash_file};
use serde_json::json;
use std::{
    fs,
    os::unix::{
        fs::PermissionsExt,
        process::{CommandExt, ExitStatusExt},
    },
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("second-leaf") => {
            ensure!(unsafe { libc::setsid() } > 0, "second leaf setsid failed");
            fs::write("/work/browser/second-leaf", std::process::id().to_string())?;
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        Some("detached") => {
            ensure!(
                unsafe { libc::setsid() } > 0,
                "detached helper setsid failed"
            );
            fs::write("/work/browser/detached", std::process::id().to_string())?;
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        Some("child") => {
            let parent = args.get(2).context("parent PID absent")?.parse::<u32>()?;
            let error = fs::OpenOptions::new()
                .write(true)
                .open("/sys/fs/cgroup/md-sim-browser/profile-1/cgroup.procs")
                .unwrap_err();
            ensure!(
                error.raw_os_error() == Some(libc::EROFS),
                "sibling migration path is writable: {error}"
            );
            let error = fs::read_dir(format!("/proc/{parent}/fd"))
                .err()
                .context("harness descriptors exposed")?;
            ensure!(
                error.raw_os_error() == Some(libc::EACCES),
                "unexpected descriptor denial: {error}"
            );
            for entry in fs::read_dir("/proc/self/fd")? {
                if let Ok(target) = fs::read_link(entry?.path()) {
                    ensure!(
                        !target.starts_with("/sys/fs/cgroup"),
                        "cgroup writer leaked through exec"
                    );
                }
            }
            let _detached = Command::new(std::env::current_exe()?)
                .arg("detached")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            fs::write(
                "/work/browser/denials",
                "read-only-path,protected-harness-fds,no-inherited-writers",
            )?;
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
        None => (),
        _ => anyhow::bail!("unsupported check mode"),
    }
    ensure!(
        unsafe { libc::geteuid() } == 0
            && fs::read_to_string("/proc/cmdline")?
                .split_whitespace()
                .any(|w| w == "md_sim_private_guest=1"),
        "requires private VM root"
    );
    ensure!(
        fs::read_dir("/sys/class/net")?.count() == 1,
        "unexpected network device"
    );
    fs::create_dir("/work/browser")?;
    fs::set_permissions("/work/browser", fs::Permissions::from_mode(0o700))?;
    ensure!(
        unsafe {
            libc::chown(
                c"/work/browser".as_ptr(),
                browser_groups::UID,
                browser_groups::UID,
            )
        } == 0,
        "ownership failed"
    );
    let mut groups = browser_groups::setup(1)?;
    ensure!(
        unsafe { libc::setgroups(0, std::ptr::null()) } == 0
            && unsafe { libc::setgid(browser_groups::UID) } == 0
            && unsafe { libc::setuid(browser_groups::UID) } == 0
            && unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } == 0
            && unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } == 0,
        "privilege drop failed"
    );
    let fd = groups[0].attach_fd();
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("child")
        .arg(std::process::id().to_string())
        .stdin(Stdio::null());
    unsafe {
        command.pre_exec(move || {
            if libc::write(fd, b"0\n".as_ptr().cast(), 2) != 2 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    let second_fd = groups[1].attach_fd();
    let mut second_command = Command::new(std::env::current_exe()?);
    second_command.arg("second-leaf").stdin(Stdio::null());
    unsafe {
        second_command.pre_exec(move || {
            if libc::write(second_fd, b"0\n".as_ptr().cast(), 2) != 2 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut second_child = second_command.spawn()?;
    let start = Instant::now();
    while !Path::new("/work/browser/detached").exists()
        || !Path::new("/work/browser/denials").exists()
        || !Path::new("/work/browser/second-leaf").exists()
    {
        ensure!(
            start.elapsed() < Duration::from_secs(10),
            "descendant readiness deadline"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let detached: u32 = fs::read_to_string("/work/browser/detached")?.parse()?;
    let detached_before = fs::read_to_string(format!("/proc/{detached}/stat"))?;
    // A distinct single-threaded supervisor holds the same cgroup writers.
    // Terminating it must kill both leaves, including the setsid descendant.
    let supervisor = unsafe { libc::fork() };
    ensure!(supervisor >= 0, "fork cancellation supervisor");
    if supervisor == 0 {
        let ready = browser_groups::install_termination_cleanup(&groups)
            .and_then(|_| Ok(fs::write("/work/browser/signal-ready", b"ready")?));
        if ready.is_err() {
            unsafe {
                libc::_exit(111);
            }
        }
        loop {
            unsafe {
                libc::pause();
            }
        }
    }
    let ready_at = Instant::now();
    while !Path::new("/work/browser/signal-ready").exists() {
        ensure!(
            ready_at.elapsed() < Duration::from_secs(5),
            "signal supervisor readiness"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    ensure!(
        unsafe { libc::kill(supervisor, libc::SIGTERM) } == 0,
        "signal supervisor"
    );
    let mut supervisor_status = 0;
    ensure!(
        unsafe { libc::waitpid(supervisor, &mut supervisor_status, 0) } == supervisor
            && libc::WIFEXITED(supervisor_status)
            && libc::WEXITSTATUS(supervisor_status) == 143,
        "supervisor did not complete its owned-group termination handler"
    );
    let status = child.wait()?;
    ensure!(
        status.signal() == Some(libc::SIGKILL),
        "child was not killed by owned group"
    );
    ensure!(
        second_child.wait()?.signal() == Some(libc::SIGKILL),
        "second browser leaf survived cancellation"
    );
    let first = groups[0].finish()?;
    let second = groups[1].finish()?;
    ensure!(
        !Path::new(&format!("/proc/{detached}")).exists(),
        "detached process remains after cleanup"
    );
    drop(groups);
    let report = json!({"schema":"md-sim-browser-group-check/1","passed":true,
        "driver":hash_file(&std::env::current_exe()?)?,"source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("browser_group_check.rs")),
        "group_source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("support/browser_groups.rs")),
        "denials":fs::read_to_string("/work/browser/denials")?,"detached_before":detached_before,
        "detached_removed":true,"signal_cleanup_both_leaves":true,"supervisor_exit":143,
        "groups":[first,second],"firmware_gate":false,"browser_gate":false});
    save_json(Path::new("/work/browser/group-report.json"), &report)?;
    println!("MD_SIM_BROWSER_GROUP_REPORT {report}");
    Ok(())
}

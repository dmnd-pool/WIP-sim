//! Private VM mounts and unprivileged account for the captured browser.
use crate::browser_groups;
use anyhow::{ensure, Result};
use std::{
    collections::BTreeSet, ffi::CString, fs, io::Write, os::unix::fs::PermissionsExt, path::Path,
};

pub fn root_setup(interfaces: &[&str], browser_cpus: u8) -> Result<Vec<browser_groups::Group>> {
    ensure!(
        unsafe { libc::geteuid() } == 0
            && fs::read_to_string("/proc/cmdline")?
                .split_whitespace()
                .any(|word| word == "md_sim_private_guest=1"),
        "browser setup requires private VM root"
    );
    // /proc reports this network namespace. A bind-mounted sysfs can retain
    // the network view of the namespace in which it was first mounted.
    let devices = fs::read_to_string("/proc/net/dev")?;
    let actual = devices
        .lines()
        .skip(2)
        .map(|line| {
            line.split_once(':')
                .map(|(name, _)| name.trim().to_owned())
                .ok_or_else(|| anyhow::anyhow!("invalid network device record"))
        })
        .collect::<Result<BTreeSet<_>>>()?;
    ensure!(
        actual == interfaces.iter().map(|name| (*name).to_owned()).collect(),
        "browser network interfaces differ: {actual:?}"
    );
    ensure!(
        unsafe { libc::unshare(libc::CLONE_NEWNS) } == 0
            && unsafe {
                libc::mount(
                    std::ptr::null(),
                    c"/".as_ptr(),
                    std::ptr::null(),
                    libc::MS_REC | libc::MS_PRIVATE,
                    std::ptr::null(),
                )
            } == 0,
        "browser mount namespace setup failed"
    );
    fs::create_dir("/work/browser")?;
    fs::set_permissions("/work/browser", fs::Permissions::from_mode(0o700))?;
    chown(Path::new("/work/browser"))?;
    fs::create_dir_all("/dev/shm")?;
    ensure!(
        unsafe {
            libc::mount(
                c"tmpfs".as_ptr(),
                c"/dev/shm".as_ptr(),
                c"tmpfs".as_ptr(),
                libc::MS_NOSUID | libc::MS_NODEV,
                c"size=256M,mode=1777".as_ptr().cast(),
            )
        } == 0,
        "browser shared memory mount failed"
    );
    let uid = browser_groups::UID;
    for (name,content) in [
        ("passwd",format!("root:x:0:0:root:/:/bin/false\nmdsim:x:{uid}:{uid}:isolated browser:/work/browser/home:/bin/false\n")),
        ("group",format!("root:x:0:\nmdsim:x:{uid}:\n")),
    ] {
        let source = format!("/work/browser/{name}");
        let target = format!("/etc/{name}");
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&source)?;
        file.write_all(content.as_bytes())?;
        fs::set_permissions(&source,fs::Permissions::from_mode(0o444))?;
        if !Path::new(&target).try_exists()? {
            fs::OpenOptions::new().write(true).create_new(true).open(&target)?;
        }
        let source = CString::new(source)?;
        let target = CString::new(target)?;
        ensure!(unsafe { libc::mount(source.as_ptr(),target.as_ptr(),std::ptr::null(),
            libc::MS_BIND,std::ptr::null()) } == 0,"private browser account mount failed");
        ensure!(unsafe { libc::mount(std::ptr::null(),target.as_ptr(),std::ptr::null(),
            libc::MS_REMOUNT | libc::MS_BIND | libc::MS_RDONLY | libc::MS_NOSUID |
            libc::MS_NODEV | libc::MS_NOEXEC,std::ptr::null()) } == 0,"browser account read-only mount failed");
    }
    browser_groups::setup(browser_cpus)
}

pub fn chown(path: &Path) -> Result<()> {
    let path = CString::new(path.as_os_str().as_encoded_bytes())?;
    ensure!(
        unsafe { libc::chown(path.as_ptr(), browser_groups::UID, browser_groups::UID) } == 0,
        "browser ownership failed"
    );
    Ok(())
}

pub fn drop_privileges() -> Result<()> {
    ensure!(
        unsafe { libc::setgroups(0, std::ptr::null()) } == 0
            && unsafe { libc::setgid(browser_groups::UID) } == 0
            && unsafe { libc::setuid(browser_groups::UID) } == 0
            && unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } == 0
            && unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } == 0,
        "browser privilege drop failed"
    );
    ensure!(
        unsafe { libc::getuid() } == browser_groups::UID
            && unsafe { libc::geteuid() } == browser_groups::UID
            && unsafe { libc::getgid() } == browser_groups::UID
            && unsafe { libc::getegid() } == browser_groups::UID
            && fs::read_to_string("/proc/self/status")?.contains("CapEff:\t0000000000000000"),
        "browser account or capabilities differ"
    );
    for directory in ["tmp", "cache", "runtime", "home", "config"] {
        let path = format!("/work/browser/{directory}");
        fs::create_dir(&path)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

//! Own all browser descendants, including crash helpers which call setsid().
//! This setup runs only in the private test VM, before the harness drops UID.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    ffi::CString,
    fs::{self, File},
    os::fd::AsRawFd,
    path::{Path, PathBuf},
    sync::atomic::{AtomicI32, Ordering},
    time::{Duration, Instant},
};

const ROOT: &str = "/sys/fs/cgroup/md-sim-browser";
// PID and network namespaces do not separate RLIMIT_NPROC accounting.
// Keep browser tasks off the worker's UID1000 (native022 exhausted its128
// task limit while Firefox was below its own256-task cgroup limit).
pub const UID: libc::uid_t = 1001;
// Linux counts threads as tasks. Trace021 failed process creation at126 leaf
// tasks plus two harness tasks under RLIMIT_NPROC128, before this controller.
pub const TASK_LIMIT: usize = 256;

#[allow(dead_code)] // Used by the selected-run helper; other browser drivers share Group.
static STOP_FDS: [AtomicI32; 2] = [AtomicI32::new(-1), AtomicI32::new(-1)];
#[allow(dead_code)]
extern "C" fn terminate_groups(_: libc::c_int) {
    for descriptor in &STOP_FDS {
        let fd = descriptor.load(Ordering::Relaxed);
        if fd >= 0 {
            // write and _exit are async-signal-safe. These dedicated CLOEXEC
            // descriptors live until process exit, so no reused fd is touched.
            unsafe {
                libc::write(fd, b"1".as_ptr().cast(), 1);
            }
        }
    }
    unsafe {
        libc::_exit(143);
    }
}

/// The selected-run parent can stop this helper and all its browser leaves.
/// Keep dedicated descriptors alive even after ordinary Group cleanup.
#[allow(dead_code)]
pub fn install_termination_cleanup(groups: &[Group]) -> Result<()> {
    ensure!(
        groups.len() == 2 && STOP_FDS.iter().all(|fd| fd.load(Ordering::Relaxed) == -1),
        "browser termination cleanup already installed or groups absent"
    );
    for (slot, group) in STOP_FDS.iter().zip(groups) {
        let fd = unsafe { libc::fcntl(group.kill.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3) };
        ensure!(fd >= 0, "duplicate browser cleanup descriptor");
        slot.store(fd, Ordering::Relaxed);
    }
    let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
    action.sa_sigaction = terminate_groups as *const () as usize;
    ensure!(
        unsafe { libc::sigemptyset(&mut action.sa_mask) } == 0
            && unsafe { libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut()) } == 0,
        "install owned browser termination cleanup"
    );
    Ok(())
}

pub fn setup(cpu_count: u8) -> Result<Vec<Group>> {
    ensure!(
        (1..=2).contains(&cpu_count),
        "browser CPU count outside bound"
    );
    ensure!(
        unsafe { libc::geteuid() } == 0,
        "browser group setup requires VM root"
    );
    fs::create_dir_all("/sys/fs/cgroup")?;
    ensure!(
        unsafe {
            libc::mount(
                c"cgroup2".as_ptr(),
                c"/sys/fs/cgroup".as_ptr(),
                c"cgroup2".as_ptr(),
                libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC,
                std::ptr::null(),
            )
        } == 0,
        "mount private browser cgroups: {}",
        std::io::Error::last_os_error()
    );
    fs::write(
        "/sys/fs/cgroup/cgroup.subtree_control",
        "+cpu +memory +pids",
    )?;
    let parent = Path::new(ROOT);
    fs::create_dir(parent)?;
    fs::write(parent.join("cgroup.subtree_control"), "+cpu +memory +pids")?;
    fs::create_dir(parent.join("supervisor"))?;
    fs::write(
        parent.join("supervisor/cgroup.procs"),
        std::process::id().to_string(),
    )?;
    chown(&parent.join("cgroup.procs"))?;
    let mut groups = Vec::new();
    let task_limit = TASK_LIMIT.to_string();
    let cpu_quota = format!("{} 100000", u32::from(cpu_count) * 100_000);
    for index in 0..2 {
        let path = parent.join(format!("profile-{index}"));
        fs::create_dir(&path)?;
        for (name, value) in [
            ("memory.max", "805306368"),
            ("memory.swap.max", "0"),
            ("memory.oom.group", "1"),
            ("pids.max", task_limit.as_str()),
            ("cpu.max", cpu_quota.as_str()),
        ] {
            fs::write(path.join(name), value)?;
            ensure!(
                fs::read_to_string(path.join(name))?.trim() == value,
                "browser group limit readback differs: {name}"
            );
        }
        for name in ["cgroup.procs", "cgroup.kill"] {
            chown(&path.join(name))?;
        }
        groups.push(Group::open(index)?);
    }
    // Keep CLOEXEC writers on the original mount, then cover it with a
    // read-only bind mount. The browser cannot migrate to another leaf.
    ensure!(
        unsafe {
            libc::mount(
                c"/sys/fs/cgroup".as_ptr(),
                c"/sys/fs/cgroup".as_ptr(),
                std::ptr::null(),
                libc::MS_BIND | libc::MS_REC,
                std::ptr::null(),
            )
        } == 0,
        "bind browser cgroup view failed"
    );
    ensure!(
        unsafe {
            libc::mount(
                std::ptr::null(),
                c"/sys/fs/cgroup".as_ptr(),
                std::ptr::null(),
                libc::MS_REMOUNT
                    | libc::MS_BIND
                    | libc::MS_RDONLY
                    | libc::MS_NOSUID
                    | libc::MS_NODEV
                    | libc::MS_NOEXEC,
                std::ptr::null(),
            )
        } == 0,
        "make browser cgroup view read-only failed"
    );
    // Reap orphaned helpers after killing their owned cgroup. Other services
    // in the VM are not descendants of this test harness.
    ensure!(
        unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0) } == 0,
        "browser subreaper setup failed"
    );
    Ok(groups)
}

fn chown(path: &Path) -> Result<()> {
    let path = CString::new(path.as_os_str().as_encoded_bytes())?;
    ensure!(
        unsafe { libc::chown(path.as_ptr(), UID, UID) } == 0,
        "browser cgroup delegation failed"
    );
    Ok(())
}

pub struct Group {
    path: PathBuf,
    attach: File,
    kill: File,
    index: usize,
    cleanup: Option<Value>,
}
impl Group {
    fn open(index: usize) -> Result<Self> {
        ensure!(index < 2, "browser profile index exceeds bound");
        let path = Path::new(ROOT).join(format!("profile-{index}"));
        ensure!(
            fs::read_to_string(path.join("cgroup.procs"))?
                .trim()
                .is_empty(),
            "browser group is already populated"
        );
        let attach = fs::OpenOptions::new()
            .write(true)
            .open(path.join("cgroup.procs"))?;
        let kill = fs::OpenOptions::new()
            .write(true)
            .open(path.join("cgroup.kill"))?;
        Ok(Self {
            path,
            attach,
            kill,
            index,
            cleanup: None,
        })
    }
    pub fn attach_fd(&self) -> libc::c_int {
        self.attach.as_raw_fd()
    }
    pub fn kill(&self) -> Result<()> {
        use std::io::Write;
        (&self.kill)
            .write_all(b"1")
            .context("kill owned browser descendants")
    }
    pub fn finish(&mut self) -> Result<Value> {
        if let Some(cleanup) = &self.cleanup {
            return Ok(cleanup.clone());
        }
        self.kill()?;
        let start = Instant::now();
        loop {
            let events = fs::read_to_string(self.path.join("cgroup.events"))?;
            if events.lines().any(|l| l == "populated 0") {
                break;
            }
            ensure!(
                start.elapsed() < Duration::from_secs(3),
                "browser descendants did not stop"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        let mut reaped = 0;
        loop {
            let pid = unsafe { libc::waitpid(-1, std::ptr::null_mut(), libc::WNOHANG) };
            if pid <= 0 {
                break;
            }
            reaped += 1;
            ensure!(
                reaped <= TASK_LIMIT,
                "orphan helper count exceeds task bound"
            );
        }
        let evidence = json!({"group":self.path,"empty":true,"reaped_helpers":reaped,
            "cpu_limit":fs::read_to_string(self.path.join("cpu.max"))?,
            "cpu":fs::read_to_string(self.path.join("cpu.stat"))?,
            "memory_peak":fs::read_to_string(self.path.join("memory.peak"))?,
            "memory_events":fs::read_to_string(self.path.join("memory.events"))?,
            "pids_peak":fs::read_to_string(self.path.join("pids.peak"))?,
            "pids_events":fs::read_to_string(self.path.join("pids.events"))?});
        self.cleanup = Some(evidence.clone());
        Ok(evidence)
    }
}
impl Drop for Group {
    fn drop(&mut self) {
        let record = match self.finish() {
            Ok(record) => record,
            Err(error) => json!({"group":self.path,"empty":false,"error":format!("{error:#}")}),
        };
        let _ = md_sim_runtime::isolation::save_json(
            Path::new(&format!("/work/browser/cleanup-{}.json", self.index)),
            &record,
        );
    }
}

//! Aggregate Linux cgroup-v2 limits for an explicitly delegated parent.
//! The runtime creates, controls and removes only its new child group.
use crate::isolation::Limits;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs::{self, File},
    io::Read,
    os::{
        fd::AsRawFd,
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: String,
    parent: PathBuf,
    name: String,
    inode: Option<u64>,
    state: String,
}

/// Call only after acquiring the worker's exclusive state lock. A reservation
/// without an inode was never allowed to execute; it can remove only an empty
/// group. A populated group is killed only after its saved inode matches.
pub fn recover(receipt: &Path, expected_parent: &Path) -> Result<()> {
    validate_parent(expected_parent)?;
    let mut saved: Receipt = serde_json::from_str(&bounded_text(receipt)?)?;
    ensure!(
        saved.schema == "md-sim-cgroup/1"
            && saved.parent == expected_parent
            && saved.name.starts_with("md-sim-")
            && saved.name.len() == 39
            && saved.name[7..]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "cgroup receipt identity differs"
    );
    ensure!(
        ["reserved", "created", "removed", "collision"].contains(&saved.state.as_str()),
        "unknown cgroup receipt state"
    );
    if saved.state == "collision" || saved.state == "removed" {
        return Ok(());
    }
    let path = expected_parent.join(&saved.name);
    if path.try_exists()? {
        let info = fs::symlink_metadata(&path)?;
        ensure!(info.is_dir(), "cgroup receipt points to a non-directory");
        if saved.state == "created" {
            ensure!(
                saved.inode == Some(info.ino()),
                "cgroup inode changed; preserve unrelated group"
            );
            fs::write(path.join("cgroup.kill"), b"1\n")?;
            wait_empty(&path)?;
        } else {
            ensure!(
                saved.inode.is_none()
                    && counters(&path.join("cgroup.events"))?.get("populated") == Some(&0),
                "uncommitted cgroup reservation is populated; cannot claim ownership"
            );
        }
        fs::remove_dir(path)?;
    }
    saved.state = "removed".into();
    crate::isolation::save_json(receipt, &saved)
}

fn wait_empty(path: &Path) -> Result<()> {
    let start = Instant::now();
    loop {
        if counters(&path.join("cgroup.events"))?.get("populated") == Some(&0) {
            return Ok(());
        }
        ensure!(
            start.elapsed() < Duration::from_secs(3),
            "run-owned cgroup did not empty after stop"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn validate_parent(parent: &Path) -> Result<()> {
    ensure!(
        parent.is_absolute() && parent.canonicalize()? == parent,
        "cgroup parent must be an absolute canonical path"
    );
    ensure!(
        fs::symlink_metadata(parent)?.is_dir(),
        "cgroup parent must be a real directory"
    );
    let name = CString::new(parent.as_os_str().as_bytes())?;
    let mut info: libc::statfs = unsafe { std::mem::zeroed() };
    ensure!(
        unsafe { libc::statfs(name.as_ptr(), &mut info) } == 0 && info.f_type == 0x63677270,
        "aggregate limits require a delegated cgroup-v2 filesystem"
    );
    let controllers = bounded_text(&parent.join("cgroup.subtree_control"))?;
    for required in ["cpu", "memory", "pids"] {
        ensure!(
            controllers.split_whitespace().any(|c| c == required),
            "delegated cgroup parent has not enabled {required}"
        );
    }
    Ok(())
}

fn bounded_text(path: &Path) -> Result<String> {
    let mut text = String::new();
    File::open(path)?.take(8193).read_to_string(&mut text)?;
    ensure!(text.len() <= 8192, "cgroup counter exceeds bound");
    Ok(text)
}
fn counters(path: &Path) -> Result<BTreeMap<String, u64>> {
    let mut result = BTreeMap::new();
    for line in bounded_text(path)?.lines() {
        let mut fields = line.split_whitespace();
        let key = fields.next().context("missing cgroup counter name")?;
        let value = fields.next().context("missing cgroup counter")?.parse()?;
        ensure!(
            fields.next().is_none() && result.insert(key.into(), value).is_none(),
            "invalid cgroup counter"
        );
    }
    Ok(result)
}

pub(crate) struct Group {
    path: PathBuf,
    attach: File,
    cleaned: bool,
    receipt: Option<PathBuf>,
}
impl Group {
    pub(crate) fn create(parent: &Path, limits: &Limits, receipt: Option<&Path>) -> Result<Self> {
        validate_parent(parent)?;
        let mut random = [0u8; 16];
        File::open("/dev/urandom")?.read_exact(&mut random)?;
        let mut record = Receipt {
            schema: "md-sim-cgroup/1".into(),
            parent: parent.to_path_buf(),
            name: format!("md-sim-{}", hex::encode(random)),
            inode: None,
            state: "reserved".into(),
        };
        let path = parent.join(&record.name);
        if let Some(receipt) = receipt {
            ensure!(!receipt.try_exists()?, "cgroup receipt must be new");
            crate::isolation::save_json(receipt, &record)?;
        }
        if let Err(error) = fs::create_dir(&path) {
            if let Some(receipt) = receipt {
                record.state = "collision".into();
                crate::isolation::save_json(receipt, &record)?;
            }
            return Err(error).context("create run-owned delegated cgroup");
        }
        // Open before a child exists. This FD is CLOEXEC; the guest never gets
        // a descriptor for a host cgroup mount.
        let attach = match fs::OpenOptions::new()
            .write(true)
            .open(path.join("cgroup.procs"))
        {
            Ok(file) => file,
            Err(error) => {
                let _ = fs::remove_dir(&path);
                return Err(error.into());
            }
        };
        let group = Self {
            path,
            attach,
            cleaned: false,
            receipt: receipt.map(Path::to_path_buf),
        };
        record.inode = Some(fs::symlink_metadata(&group.path)?.ino());
        record.state = "created".into();
        if let Some(receipt) = receipt {
            crate::isolation::save_json(receipt, &record)?;
        }
        for (name, value) in [
            ("memory.max", limits.memory_bytes.to_string()),
            ("memory.swap.max", "0".into()),
            ("memory.oom.group", "1".into()),
            ("pids.max", limits.processes.to_string()),
            // Bound scheduler overshoot of the monitored total CPU budget.
            ("cpu.max", "100000 100000".into()),
        ] {
            fs::write(group.path.join(name), value.as_bytes())
                .with_context(|| format!("set aggregate {name}"))?;
            ensure!(
                bounded_text(&group.path.join(name))?.trim() == value,
                "aggregate {name} readback differs"
            );
        }
        ensure!(
            bounded_text(&group.path.join("cgroup.procs"))?
                .trim()
                .is_empty(),
            "new cgroup is populated"
        );
        Ok(group)
    }
    pub(crate) fn attach_fd(&self) -> libc::c_int {
        self.attach.as_raw_fd()
    }
    pub(crate) fn cpu_microseconds(&self) -> Result<u64> {
        counters(&self.path.join("cpu.stat"))?
            .get("usage_usec")
            .copied()
            .context("aggregate CPU counter unavailable")
    }
    pub(crate) fn kill(&self) -> Result<()> {
        fs::write(self.path.join("cgroup.kill"), b"1\n").context("stop run-owned aggregate cgroup")
    }
    pub(crate) fn finish(&mut self, limits: &Limits) -> Result<Value> {
        self.kill()?;
        wait_empty(&self.path)?;
        let evidence = json!({"mechanism":"linux-cgroup-v2","cpu_total_budget_usec":limits.cpu_seconds * 1_000_000,"cpu_budget_poll_interval_ms":10,"cpu_bandwidth":"100000 100000","memory_max":limits.memory_bytes,"swap_max":0,"pids_max":limits.processes,"cpu":counters(&self.path.join("cpu.stat"))?,"memory_events":counters(&self.path.join("memory.events"))?,"memory_peak":bounded_text(&self.path.join("memory.peak"))?.trim().parse::<u64>()?,"pids_events":counters(&self.path.join("pids.events"))?,"empty_before_removal":true,"cleanup":"removed only the new run-owned group"});
        fs::remove_dir(&self.path).context("remove empty run-owned cgroup")?;
        self.cleaned = true;
        if let Some(receipt) = &self.receipt {
            recover(receipt, self.path.parent().expect("cgroup parent"))?;
        }
        Ok(evidence)
    }
}
impl Drop for Group {
    fn drop(&mut self) {
        if !self.cleaned {
            let _ = self.kill();
            // Best-effort on infrastructure failure; never remove its parent.
            for _ in 0..100 {
                if fs::remove_dir(&self.path).is_ok() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_directory_cannot_impersonate_delegation() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("cgroup.subtree_control"),
            "cpu memory pids",
        )
        .unwrap();
        assert!(validate_parent(directory.path())
            .unwrap_err()
            .to_string()
            .contains("cgroup-v2 filesystem"));
        assert!(!directory.path().join("cgroup.procs").exists());
    }
}

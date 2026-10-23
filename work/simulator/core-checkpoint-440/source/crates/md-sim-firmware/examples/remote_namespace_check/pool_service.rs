//! Owned independent pool process in the SSH server's existing network.
use super::{
    shared_manifest::Manifest,
    shared_pool::{self, Lease, Selection},
    shared_retention::{Bytes, PoolBundle},
    worker_broker::bounded,
};
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{isolation::save_json, store::ObjectId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::{
            fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
            process::ExitStatusExt,
        },
    },
    path::Path,
    process::{Child, Command, ExitStatus, Stdio},
    time::{Duration, Instant},
};

pub const OUTCOME: &str = "/work/pool-outcome.json";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LogIdentity {
    device: u64,
    inode: u64,
}

impl LogIdentity {
    fn read(file: &File) -> Result<Self> {
        let metadata = file.metadata()?;
        ensure!(
            metadata.is_file()
                && metadata.uid() == 0
                && metadata.nlink() == 1
                && metadata.mode() & 0o022 == 0,
            "pool log must be a private root-owned regular file"
        );
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    fn matches(&self, file: &File) -> Result<()> {
        let actual = Self::read(file)?;
        ensure!(
            self.device == actual.device && self.inode == actual.inode,
            "pool log inode changed"
        );
        Ok(())
    }
}

pub struct Service {
    selected: Selection,
    manifest: Manifest,
    child: Option<Child>,
    started: Option<u64>,
    pid: Option<u32>,
    stdout_reader: Option<File>,
    offset: u64,
    pending: Vec<u8>,
    ready: bool,
    finished: bool,
    cancelled: bool,
}

impl Service {
    pub fn new() -> Result<Self> {
        let (selected, manifest) = shared_pool::selected()?;
        fs::create_dir("/work/pool")?;
        fs::create_dir("/work/pool-control")?;
        fs::set_permissions("/work/pool-control", fs::Permissions::from_mode(0o700))?;
        Ok(Self {
            selected,
            manifest,
            child: None,
            started: None,
            pid: None,
            stdout_reader: None,
            offset: 0,
            pending: Vec::new(),
            ready: false,
            finished: false,
            cancelled: false,
        })
    }
    fn lease(&self) -> Result<Lease> {
        let lease: Lease =
            serde_json::from_slice(&broker_bytes(Path::new(shared_pool::LEASE), 65536)?)?;
        ensure!(
            lease.schema_version == "md-sim-shared-pool-lease/1"
                && lease.process == self.selected.process
                && lease.locked_run_hash == self.selected.locked_run_hash
                && lease.manifest == self.selected.manifest
                && lease.pool_input == self.selected.pool_input
                && lease.pool_verifier == self.manifest.pool_verifier,
            "pool start request differs from the frozen selection"
        );
        Ok(lease)
    }
    fn start(&mut self) -> Result<()> {
        self.lease()?;
        let stdout = create_log(Path::new("/work/pool.stdout"))?;
        let stderr = create_log(Path::new("/work/pool.stderr"))?;
        let stdout_identity = LogIdentity::read(&stdout)?;
        let stderr_identity = LogIdentity::read(&stderr)?;
        // A separate open description avoids sharing the child's write offset.
        let stdout_reader = open_regular(Path::new("/work/pool.stdout"))?;
        stdout_identity.matches(&stdout_reader)?;
        self.stdout_reader = Some(stdout_reader);
        let mut command = Command::new("/usr/bin/bwrap");
        command.args([
            "--unshare-pid",
            "--unshare-ipc",
            "--unshare-uts",
            "--die-with-parent",
            "--new-session",
            "--cap-add",
            "ALL",
            "--tmpfs",
            "/",
            "--ro-bind",
            "/usr",
            "/usr",
            "--ro-bind",
            "/lib64",
            "/lib64",
            "--ro-bind",
            "/etc",
            "/etc",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--ro-bind",
            "/sys",
            "/sys",
            "--tmpfs",
            "/tmp",
            "--dir",
            "/run",
            "--bind",
            "/work/pool",
            "/work",
        ]);
        for file in ["/pool-guest", "/shared-pool.json", "/private-pool.json"] {
            command.args(["--ro-bind", file, file]);
        }
        command
            .args(["--chdir", "/work", "--", "/pool-guest", "--shared-server"])
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr);
        self.started = Some(shared_pool::monotonic_ns()?);
        let child = command.spawn()?;
        self.pid = Some(child.id());
        self.child = Some(child);
        save_json(
            Path::new("/work/pool-start.json"),
            &json!({"pool_pid":self.pid,"started_monotonic_ns":self.started,
            "pool_verifier":self.manifest.pool_verifier,"pool_input":self.selected.pool_input,"manifest":self.selected.manifest,
            "stdout":stdout_identity,"stderr":stderr_identity,
            "work":"/work/pool","network":"existing SSH server namespace; no new NIC or network namespace"}),
        )?;
        Ok(())
    }
    fn readiness(&mut self) -> Result<()> {
        let file = self
            .stdout_reader
            .as_mut()
            .context("pool stdout reader absent")?;
        let total = file.metadata()?.len();
        ensure!(
            total <= 2 << 20 && total >= self.offset,
            "pool stdout bounds differ"
        );
        file.seek(SeekFrom::Start(self.offset))?;
        let mut bytes = Vec::new();
        file.take(total - self.offset).read_to_end(&mut bytes)?;
        self.offset = total;
        self.pending.extend_from_slice(&bytes);
        ensure!(
            self.pending.len() <= 2 << 20,
            "pool readiness pending line bound"
        );
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line = String::from_utf8(self.pending.drain(..=end).collect())?;
            if let Some(record) = line.trim_end().strip_prefix("MD_SIM_POOL_READY ") {
                ensure!(!self.ready, "duplicate actual pool ready marker");
                let ready: Value = serde_json::from_str(record)?;
                ensure!(
                    ready["endpoint"] == self.manifest.pool_endpoint
                        && ready["input"] == serde_json::to_value(&self.selected.pool_input)?
                        && ready["verifier"] == serde_json::to_value(&self.manifest.pool_verifier)?,
                    "pool actual readiness identity differs"
                );
                let lease = self.lease()?;
                save_json(
                    Path::new(shared_pool::READY),
                    &json!({"token":lease.token,"process":lease.process,"ready":ready,"error":null}),
                )?;
                self.ready = true;
            }
        }
        Ok(())
    }
    fn outcome(&mut self, status: ExitStatus, cancelled: bool) -> Result<()> {
        save_json(
            Path::new(OUTCOME),
            &json!({"schema_version":"md-sim-shared-pool-outcome/1","pool_pid":self.pid,
            "started_monotonic_ns":self.started,"finished_monotonic_ns":shared_pool::monotonic_ns()?,
            "exit_code":status.code(),"signal":status.signal(),"cancelled":cancelled,"complete":!cancelled}),
        )?;
        self.finished = true;
        Ok(())
    }
    pub fn poll(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        if self.child.is_none() {
            if !Path::new(shared_pool::LEASE).exists() {
                return Ok(());
            }
            if let Err(error) = self.start() {
                self.publish_error(&error)?;
                return Err(error);
            }
        }
        if let Err(error) = self.readiness() {
            self.publish_error(&error)?;
            return Err(error);
        }
        if Path::new("/work/broker/pool-cancel-request.json").exists() && !self.cancelled {
            let request: Value = serde_json::from_slice(&broker_bytes(
                Path::new("/work/broker/pool-cancel-request.json"),
                65536,
            )?)?;
            let lease = self.lease()?;
            ensure!(
                request["token"] == lease.token
                    && request["run_id"] == serde_json::to_value(&lease.run_id)?,
                "pool cancellation belongs to another run"
            );
            self.cancel()?;
        }
        if let Some(status) = self
            .child
            .as_mut()
            .context("pool child absent")?
            .try_wait()?
        {
            self.child.take();
            self.outcome(status, self.cancelled)?;
            if !self.ready {
                self.publish_error(&anyhow::anyhow!("pool exited before actual readiness"))?;
            }
        }
        Ok(())
    }
    fn publish_error(&self, error: &anyhow::Error) -> Result<()> {
        if !self.ready {
            let lease = self.lease()?;
            save_json(
                Path::new(shared_pool::READY),
                &json!({"token":lease.token,"process":lease.process,"ready":null,"error":format!("{error:#}")}),
            )?;
        }
        Ok(())
    }
    pub fn finish(&mut self) -> Result<()> {
        self.poll()?;
        if self.child.is_some() {
            self.cancel()?;
            let deadline = Instant::now() + Duration::from_secs(10);
            while self.child.is_some() && Instant::now() < deadline {
                self.poll()?;
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        if let Some(mut child) = self.child.take() {
            child.kill()?;
            let status = child.wait()?;
            self.outcome(status, true)?;
        }
        Ok(())
    }
    fn cancel(&mut self) -> Result<()> {
        if self.cancelled {
            return Ok(());
        }
        publish_cancel(
            Path::new("/work/pool-control"),
            Path::new("/work/pool"),
            1000,
            1000,
        )?;
        self.cancelled = true;
        Ok(())
    }
}

fn create_log(path: &Path) -> Result<File> {
    Ok(OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?)
}

fn open_regular(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "pool artifact must be a regular file"
    );
    Ok(file)
}

fn owned_bytes(path: &Path, limit: u64, owner: u32) -> Result<Vec<u8>> {
    let file = open_regular(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.uid() == owner
            && metadata.nlink() == 1
            && metadata.mode() & 0o022 == 0
            && metadata.len() <= limit,
        "broker control input ownership, links, mode or size differs"
    );
    read_bytes(file, limit)
}

fn broker_bytes(path: &Path, limit: u64) -> Result<Vec<u8>> {
    owned_bytes(path, limit, 1000)
}

fn publish_cancel(control: &Path, pool: &Path, uid: u32, gid: u32) -> Result<()> {
    let temp = control.join("cancel-pending");
    let mut file = create_log(&temp)?;
    ensure!(
        unsafe { libc::fchown(file.as_raw_fd(), uid, gid) } == 0,
        "cannot set cancellation owner: {}",
        std::io::Error::last_os_error()
    );
    file.write_all(b"cancel\n")?;
    file.sync_all()?;
    // The control directory and the pool's parent /work are root-owned.
    // rename replaces a destination symlink itself and never follows it.
    fs::rename(temp, pool.join("cancel"))?;
    File::open(control)?.sync_all()?;
    File::open(pool)?.sync_all()?;
    Ok(())
}
impl Drop for Service {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            if let Ok(status) = child.wait() {
                let _ = self.outcome(status, true);
            }
        }
    }
}

fn bytes(path: &Path, limit: u64) -> Result<Bytes> {
    file_bytes(open_regular(path)?, limit)
}

fn file_bytes(file: File, limit: u64) -> Result<Bytes> {
    let raw = read_bytes(file, limit)?;
    let object = ObjectId {
        sha256: md_sim_firmware::prepare::hash_bytes(&raw),
        size: raw.len() as u64,
    };
    Ok(Bytes {
        object,
        hex: hex::encode(raw),
    })
}

fn read_bytes(file: File, limit: u64) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    file.take(limit + 1).read_to_end(&mut raw)?;
    ensure!(
        raw.len() as u64 <= limit,
        "pool artifact exceeds its byte bound"
    );
    Ok(raw)
}

fn log_bytes(root: &Path, name: &str, start: &Value) -> Result<Bytes> {
    let expected: LogIdentity = serde_json::from_value(start[name].clone())?;
    let file = open_regular(&root.join(format!("pool.{name}")))?;
    expected.matches(&file)?;
    file_bytes(file, 2 << 20)
}

pub fn bundle(root: &Path, native: &md_sim_coordinator::RetentionIdentity) -> Result<PoolBundle> {
    let selected: Selection =
        serde_json::from_slice(&bounded(Path::new(shared_pool::SELECTION), 65536)?)?;
    let (manifest, _) = super::shared_manifest::read(Path::new("/shared-pool.json"))?;
    let lease: Lease = serde_json::from_slice(&broker_bytes(
        &root.join("broker/shared-lease.json"),
        65536,
    )?)?;
    ensure!(
        lease.locked_run_hash == native.locked_run_hash
            && lease.token == native.token
            && lease.run_id.as_deref() == Some(native.run_id.as_str())
            && lease.process == selected.process,
        "pool lease differs from selected native retention request"
    );
    let input = super::pool_input::read(Path::new("/private-pool.json"))?;
    let start: Value = serde_json::from_slice(&bounded(&root.join("pool-start.json"), 65536)?)?;
    let mut sessions = BTreeMap::new();
    for entry in fs::read_dir(root.join("pool"))? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("pool session filename is not text"))?;
        if !name.starts_with("session-") {
            continue;
        }
        let number: usize = name
            .strip_prefix("session-")
            .and_then(|name| name.strip_suffix(".jsonl"))
            .context("pool session filename differs")?
            .parse()?;
        ensure!(
            number < input.session_limit()? && name == format!("session-{number}.jsonl"),
            "pool session identity differs"
        );
        ensure!(
            entry.file_type()?.is_file(),
            "pool session must be a regular file"
        );
        sessions.insert(name, bytes(&entry.path(), 512 << 10)?);
    }
    let bundle = PoolBundle {
        schema_version: super::shared_retention::POOL_SCHEMA.into(),
        locked_run_hash: native.locked_run_hash.clone(),
        token: native.token.clone(),
        run_id: native.run_id.clone(),
        process: selected.process,
        manifest: selected.manifest,
        pool_input: selected.pool_input,
        pool_verifier: manifest.pool_verifier,
        support_runner: manifest.support_runner,
        support_backend: manifest.support_backend,
        support_kernel: manifest.support_kernel,
        lease: serde_json::to_value(lease)?,
        outcome: serde_json::from_slice(&bounded(&root.join("pool-outcome.json"), 65536)?)?,
        stdout: log_bytes(root, "stdout", &start)?,
        stderr: log_bytes(root, "stderr", &start)?,
        sessions,
    };
    ensure!(
        serde_json::to_vec(&bundle)?.len() as u64 <= super::shared_retention::POOL_BUNDLE_BYTES,
        "complete pool artifact bundle exceeds declared bound"
    );
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn retained_log_reader_has_independent_offset_and_inode() -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("stdout");
        let mut writer = create_log(&path)?;
        let mut reader = open_regular(&path)?;
        writer.write_all(b"first\n")?;
        reader.seek(SeekFrom::Start(0))?;
        let mut first = [0; 2];
        reader.read_exact(&mut first)?;
        writer.write_all(b"second\n")?;
        reader.seek(SeekFrom::Start(0))?;
        let mut actual = String::new();
        reader.read_to_string(&mut actual)?;
        assert_eq!(actual, "first\nsecond\n");
        fs::rename(&path, root.path().join("original"))?;
        fs::write(&path, b"forged readiness\n")?;
        reader.seek(SeekFrom::Start(0))?;
        actual.clear();
        reader.read_to_string(&mut actual)?;
        assert_eq!(actual, "first\nsecond\n");
        assert_ne!(reader.metadata()?.ino(), fs::metadata(path)?.ino());
        Ok(())
    }

    #[test]
    fn cancel_publication_replaces_symlink_without_changing_its_target() -> Result<()> {
        let root = tempfile::tempdir()?;
        let control = root.path().join("control");
        let pool = root.path().join("pool");
        fs::create_dir(&control)?;
        fs::create_dir(&pool)?;
        let protected = root.path().join("protected");
        fs::write(&protected, b"unchanged")?;
        symlink(&protected, pool.join("cancel"))?;
        let before = fs::metadata(&protected)?;
        publish_cancel(&control, &pool, unsafe { libc::geteuid() }, unsafe {
            libc::getegid()
        })?;
        let after = fs::metadata(&protected)?;
        assert_eq!(fs::read(&protected)?, b"unchanged");
        assert_eq!((before.uid(), before.gid()), (after.uid(), after.gid()));
        assert!(fs::symlink_metadata(pool.join("cancel"))?.is_file());
        assert_eq!(fs::read(pool.join("cancel"))?, b"cancel\n");
        assert!(!control.join("cancel-pending").exists());
        Ok(())
    }

    #[test]
    fn artifact_open_rejects_symlink_and_fifo_without_a_writer() -> Result<()> {
        let root = tempfile::tempdir()?;
        let target = root.path().join("target");
        fs::write(&target, b"target")?;
        let link = root.path().join("link");
        symlink(&target, &link)?;
        assert!(open_regular(&link).is_err());
        let fifo = root.path().join("fifo");
        let name = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes())?;
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        assert!(open_regular(&fifo).is_err());
        Ok(())
    }

    #[test]
    fn broker_control_read_checks_owner_links_mode_size_and_file_type() -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("control");
        let owner = unsafe { libc::geteuid() };
        fs::write(&path, b"control")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        assert_eq!(owned_bytes(&path, 7, owner)?, b"control");
        assert!(owned_bytes(&path, 6, owner).is_err());
        assert!(owned_bytes(&path, 7, owner.wrapping_add(1)).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o622))?;
        assert!(owned_bytes(&path, 7, owner).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        let link = root.path().join("link");
        fs::hard_link(&path, &link)?;
        assert!(owned_bytes(&path, 7, owner).is_err());
        fs::remove_file(&link)?;
        symlink(&path, &link)?;
        assert!(owned_bytes(&link, 7, owner).is_err());
        fs::remove_file(&link)?;
        let fifo = std::ffi::CString::new(link.as_os_str().as_encoded_bytes())?;
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(owned_bytes(&link, 7, owner).is_err());
        Ok(())
    }
}

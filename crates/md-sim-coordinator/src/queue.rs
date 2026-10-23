use crate::{
    executor::{Executor, ExecutorIdentity},
    plan::*,
};
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    file_lock::ExclusiveFileLock,
    isolation::save_json,
    store::{hash_file, ObjectId, Store},
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

pub fn read_json<T: DeserializeOwned>(path: &Path, limit: u64) -> Result<T> {
    let file = File::options()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.nlink() == 1 && metadata.len() <= limit,
        "unsafe or oversized JSON input"
    );
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= limit,
        "JSON input grew past its budget"
    );
    Ok(serde_json::from_slice(&bytes)?)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttemptResult {
    pub schema_version: String,
    pub run_hash: String,
    pub resume_key: String,
    pub executor: ExecutorIdentity,
    pub status: String,
    pub gates: Vec<GateResult>,
    pub capability_limits: Vec<String>,
    pub errors: Vec<String>,
    pub evidence: BTreeMap<String, ObjectId>,
    pub scope: ExecutionScope,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct QueueRow {
    pub name: String,
    pub run_hash: String,
    pub status: String,
    pub reused: bool,
    pub attempt: String,
    pub result: ObjectId,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct QueueReport {
    pub schema_version: String,
    pub plan_sha256: String,
    pub rows: Vec<QueueRow>,
    pub declared_suite_complete: bool,
    pub exact_physical_targets_executed: u64,
    pub complete_hardware_matrix: bool,
}

pub struct Queue {
    root: PathBuf,
    pub plan: LockedPlan,
    store: Store,
    _lock: ExclusiveFileLock,
}

/// The verifier can export only bounded regular files into its owned evidence
/// directory. It cannot request arbitrary guest or host paths as destinations.
pub struct EvidenceWriter {
    root: PathBuf,
    limit: u64,
    used: u64,
    files: BTreeMap<String, ObjectId>,
}
impl EvidenceWriter {
    fn new(root: PathBuf, limit: u64) -> Result<Self> {
        fs::create_dir(&root)?;
        Ok(Self {
            root,
            limit,
            used: 0,
            files: BTreeMap::new(),
        })
    }
    pub fn write(&mut self, name: &str, bytes: &[u8]) -> Result<()> {
        ensure!(
            !name.is_empty()
                && name.len() <= 128
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                && name != "."
                && name != "..",
            "unsafe evidence filename"
        );
        ensure!(
            self.files.len() < 64 && !self.files.contains_key(name),
            "duplicate or excessive evidence files"
        );
        let next = self
            .used
            .checked_add(bytes.len() as u64)
            .context("evidence size overflow")?;
        ensure!(next <= self.limit, "verifier evidence byte budget exceeded");
        self.used = next;
        let path = self.root.join(name);
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)?;
        use std::io::Write;
        file.write_all(bytes)?;
        file.sync_all()?;
        self.files.insert(name.into(), hash_file(&path)?);
        Ok(())
    }
    pub fn json(&mut self, name: &str, value: &impl Serialize) -> Result<()> {
        let mut bytes = serde_json::to_vec_pretty(value)?;
        bytes.push(b'\n');
        self.write(name, &bytes)
    }
    pub fn copy(&mut self, name: &str, store: &Store, object: &ObjectId) -> Result<()> {
        ensure!(
            object.size <= self.limit - self.used,
            "copied evidence exceeds byte budget"
        );
        let mut bytes = Vec::new();
        File::open(store.verified_path(object)?)?
            .take(object.size + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 == object.size,
            "copied input changed size"
        );
        self.write(name, &bytes)
    }
    pub fn files(&self) -> &BTreeMap<String, ObjectId> {
        &self.files
    }
    pub fn reference(&self, name: &str) -> Result<String> {
        ensure!(
            self.files.contains_key(name),
            "evidence reference is absent"
        );
        Ok(format!("evidence/{name}"))
    }
}
impl Queue {
    pub fn input_store(&self) -> &Store {
        &self.store
    }
    pub fn create(root: &Path, plan: LockedPlan, source: &Store) -> Result<Self> {
        plan.validate()?;
        ensure!(!root.exists(), "queue directory must be new");
        ensure!(
            serde_json::to_vec_pretty(&plan)?.len() <= 16 * 1024 * 1024,
            "execution plan exceeds file bound"
        );
        fs::create_dir_all(root)?;
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
        save_json(&root.join("plan.json"), &plan)?;
        fs::create_dir(root.join("jobs"))?;
        let queue = Self::open(root)?;
        let mut unique = BTreeMap::new();
        for run in &queue.plan.content.runs {
            for id in run.inputs.values() {
                unique.insert(id.sha256.clone(), id.clone());
            }
        }
        for id in unique.values() {
            queue.store.import(&source.verified_path(id)?, id)?;
        }
        Ok(queue)
    }
    pub fn open(root: &Path) -> Result<Self> {
        ensure!(
            fs::symlink_metadata(root)?.is_dir(),
            "queue root must be a real directory"
        );
        let root = root.canonicalize()?;
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            let name = entry.file_name();
            let m = fs::symlink_metadata(entry.path())?;
            ensure!(
                (name == "plan.json" || name == "queue.lock") && m.is_file() && m.nlink() == 1
                    || (name == "jobs" || name == "input-store") && m.is_dir(),
                "unexpected or unsafe queue entry"
            );
        }
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join("queue.lock"))?;
        ensure!(lock.metadata()?.len() == 0, "unsafe queue lock");
        let lock = ExclusiveFileLock::acquire(lock).context("queue already has a writer")?;
        let plan: LockedPlan = read_json(&root.join("plan.json"), 16 * 1024 * 1024)?;
        plan.validate()?;
        ensure!(
            fs::symlink_metadata(root.join("jobs"))?.is_dir(),
            "jobs must be a real directory"
        );
        let store = Store::open(&root.join("input-store"), 8 * 1024 * 1024 * 1024)?;
        Ok(Self {
            root,
            plan,
            store,
            _lock: lock,
        })
    }
    pub fn run(
        &mut self,
        executor: &mut impl Executor,
        suite: &impl SuiteVerifier,
        resume: bool,
        cancel: Arc<AtomicBool>,
    ) -> Result<QueueReport> {
        let identity = executor.identity()?;
        let mut rows = Vec::new();
        for run in &self.plan.content.runs {
            let hash = run.hash()?;
            let key = crate::identity(&("md-sim-resume/2", run, &identity))?;
            for id in run.inputs.values() {
                self.store.verified_path(id)?;
            }
            let job = self.root.join("jobs").join(&hash);
            fs::create_dir_all(&job)?;
            ensure!(
                fs::symlink_metadata(&job)?.is_dir(),
                "job must be a real directory"
            );
            let attempts = attempts(&job)?;
            let mut cached = None;
            for old in &attempts {
                if !old.join("completion.json").exists() {
                    let recovery = executor.recover(old);
                    let mut errors=match recovery{Ok(())=>vec!["Previous coordinator ended before a complete result commit; fresh scratch is required.".into()],Err(e)=>vec![format!("Interrupted execution recovery failed: {e:#}")]};
                    let old_identity: AttemptLock =
                        read_json(&old.join("lock.json"), 16 * 1024 * 1024).unwrap_or_else(|e| {
                            errors.push(format!("Initialization lock was not committed or is invalid: {e:#}; no prior execution identity is claimed."));
                            AttemptLock {
                                run: run.clone(),
                                run_hash: hash.clone(),
                                resume_key: key.clone(),
                                executor: identity.clone(),
                            }
                        });
                    seal(
                        old,
                        AttemptResult {
                            schema_version: "2.0".into(),
                            run_hash: hash.clone(),
                            resume_key: old_identity.resume_key,
                            executor: old_identity.executor,
                            status: "interrupted".into(),
                            gates: vec![],
                            capability_limits: run.capability_limits.clone(),
                            errors,
                            evidence: BTreeMap::new(),
                            scope: run.scope.clone(),
                        },
                    )?;
                }
                if resume {
                    if let Ok(result) = verified_attempt(old, &key, run) {
                        if result.status == "pass" && required_pass(run, &result.gates) {
                            cached = Some(old.clone());
                        }
                    }
                }
            }
            if let Some(old) = cached {
                rows.push(row(run, &hash, &old, true)?);
                continue;
            }
            let (count, used) = storage_usage(&self.root.join("jobs"))?;
            let reserve = run.process.as_ref().map_or(0, |p| p.limits.trace_bytes * 6)
                + (serde_json::to_vec(run)?.len() as u64) * 3
                + 2 * 1024 * 1024;
            ensure!(
                count < self.plan.content.limits.max_attempts
                    && used
                        .checked_add(reserve)
                        .is_some_and(|n| n <= self.plan.content.limits.max_evidence_bytes),
                "coordinator evidence budget exhausted; prior attempts are retained"
            );
            let next = attempts
                .last()
                .map(|p| p.file_name().unwrap().to_str().unwrap()[8..].parse::<u32>())
                .transpose()?
                .unwrap_or(0)
                .checked_add(1)
                .context("attempt number overflow")?;
            ensure!(next <= 999999, "attempt name limit reached");
            let path = job.join(format!("attempt-{next:06}"));
            fs::create_dir(&path)?;
            save_json(
                &path.join("lock.json"),
                &AttemptLock {
                    run: run.clone(),
                    run_hash: hash.clone(),
                    resume_key: key.clone(),
                    executor: identity.clone(),
                },
            )?;
            File::open(&job)?.sync_all()?;
            let mut result = AttemptResult {
                schema_version: "2.0".into(),
                run_hash: hash.clone(),
                resume_key: key,
                executor: identity.clone(),
                status: "blocked".into(),
                gates: vec![],
                capability_limits: run.capability_limits.clone(),
                errors: vec![],
                evidence: BTreeMap::new(),
                scope: run.scope.clone(),
            };
            if run.process.is_none() {
                result.errors.push("Unresolved target has no executable path. This is not an incompatibility exclusion.".into());
            } else if run.suite != suite.suite() {
                result.errors.push(
                    "The selected suite or verifier is not implemented at the locked identity."
                        .into(),
                );
            } else if cancel.load(Ordering::Acquire) {
                result.status = "cancelled".into();
            } else {
                let local_cancel = Arc::new(AtomicBool::new(false));
                let watcher = CancellationWatch::start(
                    cancel.clone(),
                    local_cancel.clone(),
                    path.join("cancel.request"),
                );
                let execute = (|| -> Result<Vec<GateResult>> {
                    let process = executor.run(&self.store, run, local_cancel.clone(), &path)?;
                    let trace_limit = run
                        .process
                        .as_ref()
                        .context("missing process")?
                        .limits
                        .trace_bytes;
                    ensure!(
                        (process.stdout.len() + process.stderr.len()) as u64 <= trace_limit * 2,
                        "process result exceeds the locked trace budget"
                    );
                    ensure!(
                        process
                            .stdout
                            .bytes()
                            .chain(process.stderr.bytes())
                            .all(|b| b.is_ascii_hexdigit()),
                        "process result contains invalid trace encoding"
                    );
                    save_json(&path.join("process-result.json"), &process)?;
                    ensure!(
                        process.tools == run.backend_tools,
                        "executed isolation tool identity differs"
                    );
                    let mut retained = BTreeMap::new();
                    walk(&path, "", &mut retained)?;
                    let occupied: u64 = retained.values().map(|id| id.size).sum();
                    let budget = reserve
                        .checked_sub(occupied + 1024 * 1024)
                        .context("executor exceeded its evidence reservation")?;
                    let mut output = EvidenceWriter::new(path.join("evidence"), budget)?;
                    let gates = suite.verify(&self.store, run, &process, &mut output)?;
                    validate_gates(run, &gates)?;
                    let mut retained = BTreeMap::new();
                    walk(&path, "", &mut retained)?;
                    validate_references(&gates, &retained)?;
                    Ok(gates)
                })();
                drop(watcher);
                match execute {
                    Ok(gates) => {
                        result.status = if required_pass(run, &gates) {
                            "pass"
                        } else {
                            "fail"
                        }
                        .into();
                        result.gates = gates;
                    }
                    Err(e) => {
                        result.status = "fail".into();
                        result.errors.push(format!("{e:#}"));
                    }
                }
                if local_cancel.load(Ordering::Acquire) {
                    result.status = "cancelled".into();
                }
            }
            seal(&path, result)?;
            rows.push(row(run, &hash, &path, false)?);
        }
        let declared_suite_complete = rows.iter().all(|r| r.status == "pass");
        Ok(QueueReport {
            schema_version: "2.0".into(),
            plan_sha256: self.plan.sha256.clone(),
            rows,
            declared_suite_complete,
            exact_physical_targets_executed: 0,
            complete_hardware_matrix: false,
        })
    }
}

#[derive(Serialize, Deserialize)]
struct AttemptLock {
    run: LockedRun,
    run_hash: String,
    resume_key: String,
    executor: ExecutorIdentity,
}
fn attempts(job: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(job)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_str().context("non-UTF8 attempt name")?;
        ensure!(
            name.len() == 14
                && name.starts_with("attempt-")
                && name[8..].bytes().all(|b| b.is_ascii_digit())
                && fs::symlink_metadata(entry.path())?.is_dir(),
            "unsafe attempt entry"
        );
        paths.push(entry.path());
        ensure!(paths.len() <= 65536, "attempt count exceeded");
    }
    paths.sort();
    Ok(paths)
}
fn walk(path: &Path, relative: &str, out: &mut BTreeMap<String, ObjectId>) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("non-UTF8 evidence name"))?;
        if relative.is_empty() && ["result.json", "completion.json"].contains(&name.as_str()) {
            continue;
        }
        ensure!(
            !name.contains('/') && name.len() <= 128,
            "invalid evidence name"
        );
        let key = format!("{relative}{name}");
        let m = fs::symlink_metadata(entry.path())?;
        if m.is_dir() {
            ensure!(
                relative.is_empty() && name == "evidence",
                "unexpected evidence directory"
            );
            walk(&entry.path(), "evidence/", out)?;
        } else {
            ensure!(m.is_file() && m.nlink() == 1, "unsafe evidence file");
            ensure!(out.len() < 128, "evidence file count exceeded");
            out.insert(key, hash_file(&entry.path())?);
        }
    }
    Ok(())
}
pub(crate) fn move_without_replace(source: &Path, target: &Path) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    let source = std::ffi::CString::new(source.as_os_str().as_bytes())?;
    let target = std::ffi::CString::new(target.as_os_str().as_bytes())?;
    let result = unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            target.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}
fn seal(path: &Path, mut result: AttemptResult) -> Result<()> {
    // An interrupted result commit is evidence. Never replace its bytes when
    // sealing the coordinator's later interrupted disposition.
    let prior = path.join("result.json");
    if prior.exists() {
        let target = (0..64)
            .map(|n| path.join(format!("result-before-seal-{n:02}.json")))
            .find(|p| !p.exists())
            .context("result history limit reached")?;
        move_without_replace(&prior, &target)?;
        File::open(path)?.sync_all()?;
    }
    walk(path, "", &mut result.evidence)?;
    validate_references(&result.gates, &result.evidence)?;
    ensure!(
        serde_json::to_vec_pretty(&result)?.len() < 1024 * 1024 - 4096,
        "attempt result metadata exceeds budget"
    );
    save_json(&path.join("result.json"), &result)?;
    save_json(
        &path.join("completion.json"),
        &hash_file(&path.join("result.json"))?,
    )
}
fn verified_attempt(path: &Path, key: &str, run: &LockedRun) -> Result<AttemptResult> {
    let expected: ObjectId = read_json(&path.join("completion.json"), 4096)?;
    ensure!(
        hash_file(&path.join("result.json"))? == expected,
        "saved result identity changed"
    );
    let result: AttemptResult = read_json(&path.join("result.json"), 1024 * 1024)?;
    ensure!(result.resume_key == key, "cached run identity differs");
    let mut actual = BTreeMap::new();
    walk(path, "", &mut actual)?;
    ensure!(
        actual == result.evidence,
        "saved execution evidence changed"
    );
    validate_gates(run, &result.gates)?;
    validate_references(&result.gates, &actual)?;
    Ok(result)
}
fn row(run: &LockedRun, hash: &str, path: &Path, reused: bool) -> Result<QueueRow> {
    let result: AttemptResult = read_json(&path.join("result.json"), 1024 * 1024)?;
    Ok(QueueRow {
        name: run.name.clone(),
        run_hash: hash.into(),
        status: result.status,
        reused,
        attempt: path.to_string_lossy().into_owned(),
        result: hash_file(&path.join("result.json"))?,
    })
}
fn validate_references(gates: &[GateResult], files: &BTreeMap<String, ObjectId>) -> Result<()> {
    for gate in gates {
        for name in &gate.evidence {
            ensure!(
                files.contains_key(name),
                "gate references missing retained evidence: {name}"
            );
        }
    }
    Ok(())
}
fn validate_gates(run: &LockedRun, gates: &[GateResult]) -> Result<()> {
    ensure!(gates.len() <= 64, "gate count limit");
    let mut names = std::collections::BTreeSet::new();
    for gate in gates {
        ensure!(
            names.insert(&gate.id)
                && !gate.clock_domain.is_empty()
                && !gate.test_version.is_empty(),
            "invalid or duplicate gate"
        );
        ensure!(
            gate.expected != serde_json::Value::Null && gate.actual != serde_json::Value::Null,
            "gate lacks expected/actual values"
        );
        if gate.status == GateStatus::Pass {
            ensure!(!gate.evidence.is_empty(), "passed gate lacks evidence");
        }
    }
    ensure!(
        run.suite.required_gates.iter().all(|id| names.contains(id)),
        "suite omitted a required gate"
    );
    Ok(())
}
fn required_pass(run: &LockedRun, gates: &[GateResult]) -> bool {
    run.suite.required_gates.iter().all(|id| {
        gates
            .iter()
            .any(|g| g.id == *id && g.status == GateStatus::Pass)
    })
}
fn storage_usage(root: &Path) -> Result<(u32, u64)> {
    let mut count = 0u32;
    let mut bytes = 0u64;
    for job in fs::read_dir(root)? {
        let job = job?;
        ensure!(
            fs::symlink_metadata(job.path())?.is_dir(),
            "unsafe job storage"
        );
        for path in attempts(&job.path())? {
            count = count.checked_add(1).context("attempt count overflow")?;
            let mut files = BTreeMap::new();
            walk(&path, "", &mut files)?;
            for name in ["result.json", "completion.json"] {
                let p = path.join(name);
                if p.exists() {
                    let m = fs::symlink_metadata(&p)?;
                    ensure!(m.is_file() && m.nlink() == 1, "unsafe result file");
                    bytes = bytes
                        .checked_add(m.len())
                        .context("evidence byte overflow")?;
                }
            }
            for id in files.values() {
                bytes = bytes
                    .checked_add(id.size)
                    .context("evidence byte overflow")?;
            }
        }
    }
    Ok((count, bytes))
}
struct CancellationWatch {
    done: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl CancellationWatch {
    fn start(global: Arc<AtomicBool>, local: Arc<AtomicBool>, request: PathBuf) -> Self {
        let done = Arc::new(AtomicBool::new(false));
        let end = done.clone();
        let thread = thread::spawn(move || {
            while !end.load(Ordering::Acquire) {
                if global.load(Ordering::Acquire) || request.exists() {
                    local.store(true, Ordering::Release);
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
        });
        Self {
            done,
            thread: Some(thread),
        }
    }
}
impl Drop for CancellationWatch {
    fn drop(&mut self) {
        self.done.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub fn request_cancel(root: &Path, run_hash: &str) -> Result<bool> {
    let plan: LockedPlan = read_json(&root.join("plan.json"), 16 * 1024 * 1024)?;
    plan.validate()?;
    ensure!(
        plan.content
            .runs
            .iter()
            .any(|r| r.hash().ok().as_deref() == Some(run_hash)),
        "run is not in the locked plan"
    );
    let paths = attempts(&root.join("jobs").join(run_hash))?;
    let Some(path) = paths.last() else {
        return Ok(false);
    };
    if path.join("completion.json").exists() {
        return Ok(false);
    }
    let target = path.join("cancel.request");
    if target.exists() {
        return Ok(true);
    }
    let mut file = File::options()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(target)?;
    use std::io::Write;
    file.write_all(b"cancel current attempt\n")?;
    file.sync_all()?;
    File::open(path)?.sync_all()?;
    Ok(true)
}

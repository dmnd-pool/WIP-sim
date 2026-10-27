//! Exact duplicate CAS copies from superseded preparation462 and failed run469.
//! This mode preserves their verdicts. It does not imply firmware acceptance.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    file_lock::ExclusiveFileLock,
    isolation::ProcessSpec,
    store::{ObjectId, StoreLimits},
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{BufReader, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Component, Path, PathBuf},
};

const TARGETS: &[&str] = &[
    "work/simulator/qemu-s9/native-browser-prepared-462/input-store",
    "work/simulator/network-vm/native-browser-469/input-store",
    "work/simulator/network-vm/native-browser-469/execution/input-store",
    "work/simulator/network-vm/native-browser-469/client-image/input-store",
    "work/simulator/network-vm/native-browser-469/execution/shared-access/client/inspection/input-store",
    "work/simulator/network-vm/native-browser-469/execution/shared-access/client/gateway/input-store",
];
const KEEPERS: &[&str] = &[
    "work/simulator/qemu-s9/native-browser-prepared-478/input-store",
    "work/simulator/qemu-s9/native-browser-prepared-468/input-store",
    "work/simulator/network-vm/native-worker-channel-342/input-store",
    "work/simulator/network-vm/browser-startup-prepared-456/input-store",
    "work/simulator/network-vm/native-browser-453/client-image/input-store",
];
const OBJECT_LIMIT: u64 = 512 << 20;

#[derive(Deserialize)]
struct Inventory {
    schema: String,
    candidates: Vec<Candidate>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Candidate {
    path: PathBuf,
    sha256_name: String,
    size: u64,
    allocated_bytes: u64,
    device: u64,
    inode: u64,
    links: u64,
    restore_copy_from: PathBuf,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct State {
    device: u64,
    inode: u64,
    size: u64,
    links: u64,
    uid: u32,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl State {
    fn from(metadata: &fs::Metadata) -> Result<Self> {
        ensure!(
            metadata.is_file()
                && metadata.nlink() == 1
                && metadata.uid() == unsafe { libc::geteuid() },
            "cleanup file must be owned, regular, and have one link"
        );
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            links: metadata.nlink(),
            uid: metadata.uid(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        })
    }
}

struct HashedRead {
    file: File,
    digest: Sha256,
    bytes: u64,
    limit: u64,
}
impl Read for HashedRead {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let available = (self.limit + 1).saturating_sub(self.bytes);
        let count = buffer.len().min(available as usize);
        if count == 0 {
            return Err(std::io::Error::other("cleanup input exceeds byte bound"));
        }
        let read = self.file.read(&mut buffer[..count])?;
        self.digest.update(&buffer[..read]);
        self.bytes += read as u64;
        Ok(read)
    }
}
fn reader(path: &Path, limit: u64) -> Result<(BufReader<HashedRead>, State)> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let state = State::from(&file.metadata()?)?;
    ensure!(state.size <= limit, "cleanup input size bound");
    Ok((
        BufReader::with_capacity(
            65536,
            HashedRead {
                file,
                digest: Sha256::new(),
                bytes: 0,
                limit,
            },
        ),
        state,
    ))
}
fn finish(reader: &BufReader<HashedRead>, before: &State) -> Result<ObjectId> {
    let reader = reader.get_ref();
    ensure!(
        State::from(&reader.file.metadata()?)? == *before && reader.bytes == before.size,
        "cleanup input changed while being read"
    );
    Ok(ObjectId {
        sha256: hex::encode(reader.digest.clone().finalize()),
        size: reader.bytes,
    })
}
fn json_file<T: DeserializeOwned>(path: &Path, limit: u64) -> Result<(T, ObjectId)> {
    let (mut input, before) = reader(path, limit)?;
    let value = serde_json::from_reader(&mut input)?;
    Ok((value, finish(&input, &before)?))
}
fn hash_owned(path: &Path, limit: u64) -> Result<(ObjectId, State)> {
    let (mut input, before) = reader(path, limit)?;
    std::io::copy(&mut input, &mut std::io::sink())?;
    Ok((finish(&input, &before)?, before))
}

fn object_store(path: &Path, allowed: &[&str]) -> Result<PathBuf> {
    ensure!(
        !path.is_absolute() && path.components().all(|c| matches!(c, Component::Normal(_))),
        "cleanup path must be normalized and repository-relative"
    );
    let objects = path.parent().context("cleanup object directory absent")?;
    ensure!(
        objects.file_name().is_some_and(|name| name == "objects"),
        "only direct CAS objects are eligible"
    );
    let store = objects.parent().context("cleanup store absent")?;
    ensure!(
        allowed.iter().any(|allowed| store == Path::new(allowed)),
        "cleanup path is outside reviewed browser scope"
    );
    Ok(store.to_owned())
}

#[derive(Serialize)]
struct Pair {
    path: PathBuf,
    restore_copy_from: PathBuf,
    object: ObjectId,
    before: State,
    restore_before: State,
}
#[derive(Serialize)]
struct Report {
    schema: &'static str,
    plan_only: bool,
    completed: bool,
    inventory: ObjectId,
    tool: ObjectId,
    planned_bytes: u64,
    removed_bytes: u64,
    objects: Vec<Pair>,
    retained_objects: BTreeMap<PathBuf, ObjectId>,
    evidence: BTreeMap<PathBuf, ObjectId>,
    original_run_assessment_passed: Option<bool>,
    restore_rule: &'static str,
}

fn evidence(
    repository: &Path,
    stores: &BTreeSet<PathBuf>,
) -> Result<(BTreeMap<PathBuf, ObjectId>, Option<bool>)> {
    let mut inputs = BTreeMap::new();
    if stores.contains(Path::new(TARGETS[0])) {
        let path = Path::new(TARGETS[0])
            .parent()
            .unwrap()
            .join("process-spec.json");
        let (spec, identity): (ProcessSpec, _) = json_file(&repository.join(&path), 1 << 20)?;
        spec.validate()?;
        inputs.insert(path, identity);
    }
    let run = Path::new("work/simulator/network-vm/native-browser-469");
    let passed = if stores.iter().any(|store| store.starts_with(run)) {
        #[derive(Deserialize)]
        struct Terminal {
            status: String,
            exit_code: Option<i32>,
        }
        #[derive(Deserialize)]
        struct Assessment {
            passed: bool,
        }
        let result = run.join("result.json");
        let (terminal, identity): (Terminal, _) = json_file(&repository.join(&result), 256 << 20)?;
        ensure!(
            terminal.status == "exited" && terminal.exit_code.is_some(),
            "browser run is not terminal"
        );
        inputs.insert(result, identity);
        let path = run.join("assessment.json");
        let (assessment, identity): (Assessment, _) = json_file(&repository.join(&path), 64 << 20)?;
        inputs.insert(path, identity);
        let path = run.join("process-spec.json");
        let (spec, identity): (ProcessSpec, _) = json_file(&repository.join(&path), 1 << 20)?;
        spec.validate()?;
        inputs.insert(path, identity);
        let log = run.join("guest.log");
        inputs.insert(log.clone(), hash_owned(&repository.join(log), 128 << 20)?.0);
        Some(assessment.passed)
    } else {
        None
    };
    Ok((inputs, passed))
}

fn verify_file(repository: &Path, path: &Path, expected: &ObjectId) -> Result<State> {
    let (actual, state) = hash_owned(&repository.join(path), OBJECT_LIMIT)?;
    ensure!(
        actual == *expected,
        "cleanup input or restore hash differs: {}",
        path.display()
    );
    Ok(state)
}

fn run(
    repository: &Path,
    inventory_path: &Path,
    journal: &Path,
    plan_only: bool,
) -> Result<Report> {
    let journal = if journal.is_absolute() {
        journal.to_owned()
    } else {
        repository.join(journal)
    };
    super::real_directories(
        journal.parent().context("journal parent absent")?,
        repository,
    )?;
    ensure!(
        !journal.try_exists()? && !journal.with_extension("completed.json").try_exists()?,
        "cleanup journal already exists"
    );
    ensure!(
        TARGETS
            .iter()
            .chain(KEEPERS)
            .all(|store| !journal.starts_with(repository.join(store))),
        "journal cannot be inside a CAS store"
    );
    let (inventory, identity): (Inventory, _) = json_file(inventory_path, 4 << 20)?;
    ensure!(
        inventory.schema == "md-sim-readonly-duplicate-store-inventory/1"
            && !inventory.candidates.is_empty()
            && inventory.candidates.len() <= 4096,
        "cleanup inventory schema or count bound"
    );
    let mut stores = BTreeSet::new();
    let mut targets = BTreeSet::new();
    let mut selected = BTreeSet::new();
    let mut total = 0u64;
    for item in &inventory.candidates {
        let object = ObjectId {
            sha256: item.sha256_name.clone(),
            size: item.size,
        };
        object.validate()?;
        ensure!(
            item.path
                .file_name()
                .is_some_and(|n| n == object.sha256.as_str())
                && item
                    .restore_copy_from
                    .file_name()
                    .is_some_and(|n| n == object.sha256.as_str())
                && item.size <= OBJECT_LIMIT
                && item.links == 1
                && item.allocated_bytes <= OBJECT_LIMIT + (1 << 20),
            "cleanup candidate identity or byte bound"
        );
        let target = object_store(&item.path, TARGETS)?;
        let keeper = object_store(&item.restore_copy_from, KEEPERS)?;
        ensure!(
            selected.insert(item.path.clone()),
            "duplicate cleanup candidate path"
        );
        targets.insert(target.clone());
        stores.insert(target);
        stores.insert(keeper);
        total = total
            .checked_add(item.size)
            .context("cleanup total overflow")?;
    }
    ensure!(total <= 4 << 30, "cleanup aggregate byte bound");
    let mut locks = Vec::new();
    for store in &stores {
        let path = repository.join(store);
        super::real_directories(&path.join("objects"), repository)?;
        super::real_directories(&path.join("incoming"), repository)?;
        let lock = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path.join("store.lock"))?;
        ensure!(
            State::from(&lock.metadata()?)?.size == 0,
            "store lock must be empty"
        );
        locks.push(ExclusiveFileLock::acquire(lock).context("browser cleanup store is active")?);
        ensure!(
            fs::read_dir(path.join("incoming"))?.next().is_none(),
            "incomplete store transfers remain"
        );
    }
    let (mut evidence, original_run_assessment_passed) = evidence(repository, &targets)?;
    for store in &stores {
        match fs::symlink_metadata(repository.join(store).join("limits.pending.json")) {
            Ok(_) => anyhow::bail!("store policy update is incomplete"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let policy = store.join("limits.json");
        let (limits, identity): (StoreLimits, _) = json_file(&repository.join(&policy), 4096)?;
        limits.validate()?;
        evidence.insert(policy, identity);
    }
    let mut objects = Vec::new();
    let mut keepers = BTreeMap::<PathBuf, (ObjectId, State)>::new();
    for item in inventory.candidates {
        let object = ObjectId {
            sha256: item.sha256_name,
            size: item.size,
        };
        let before = verify_file(repository, &item.path, &object)?;
        ensure!(
            before.device == item.device && before.inode == item.inode,
            "candidate inode changed since inventory"
        );
        let restored = if let Some((expected, state)) = keepers.get(&item.restore_copy_from) {
            ensure!(*expected == object, "conflicting restore identities");
            state.clone()
        } else {
            let state = verify_file(repository, &item.restore_copy_from, &object)?;
            keepers.insert(
                item.restore_copy_from.clone(),
                (object.clone(), state.clone()),
            );
            state
        };
        ensure!(
            (before.device, before.inode) != (restored.device, restored.inode),
            "restore copy is the candidate inode"
        );
        objects.push(Pair {
            path: item.path,
            restore_copy_from: item.restore_copy_from,
            object,
            before,
            restore_before: restored,
        });
    }
    let mut retained_objects = BTreeMap::new();
    for target in &targets {
        for entry in fs::read_dir(repository.join(target).join("objects"))? {
            let relative = target.join("objects").join(entry?.file_name());
            if !selected.contains(&relative) {
                let object = hash_owned(&repository.join(&relative), OBJECT_LIMIT)?.0;
                object.validate()?;
                ensure!(
                    relative
                        .file_name()
                        .is_some_and(|name| name == object.sha256.as_str()),
                    "retained CAS filename hash differs"
                );
                retained_objects.insert(relative, object);
            }
        }
    }
    let mut report = Report { schema: "md-sim-browser-duplicate-cleanup/1", plan_only, completed: false,
        inventory: identity, tool: md_sim_runtime::store::hash_file(Path::new("/proc/self/exe"))?,
        planned_bytes: total, removed_bytes: 0, objects, retained_objects, evidence,
        original_run_assessment_passed, restore_rule: "Copy each retained restore_copy_from into its absent original path, verify object hash/size, never hard-link or overwrite a different file." };
    for (path, expected) in &report.evidence {
        ensure!(
            hash_owned(&repository.join(path), 256 << 20)?.0 == *expected,
            "original browser evidence changed before journal"
        );
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&journal)?;
    serde_json::to_writer_pretty(&mut file, &report)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    File::open(journal.parent().unwrap_or(Path::new(".")))?.sync_all()?;
    if plan_only {
        return Ok(report);
    }
    // Reserve the completion path before any removal. A collision must not
    // leave a partial cleanup. The original journal remains the restore map.
    let mut completed = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(journal.with_extension("completed.json"))?;
    File::open(journal.parent().unwrap())?.sync_all()?;
    for pair in &report.objects {
        ensure!(
            verify_file(repository, &pair.path, &pair.object)? == pair.before
                && verify_file(repository, &pair.restore_copy_from, &pair.object)?
                    == pair.restore_before,
            "cleanup pair changed after durable journal"
        );
        fs::remove_file(repository.join(&pair.path))?;
        report.removed_bytes += pair.object.size;
    }
    for target in &targets {
        File::open(repository.join(target).join("objects"))?.sync_all()?;
    }
    for (path, expected) in &report.retained_objects {
        verify_file(repository, path, expected)?;
    }
    for (path, (expected, state)) in &keepers {
        ensure!(
            verify_file(repository, path, expected)? == *state,
            "retained restore changed"
        );
    }
    for (path, expected) in &report.evidence {
        ensure!(
            hash_owned(&repository.join(path), 256 << 20)?.0 == *expected,
            "original browser evidence changed"
        );
    }
    report.completed = true;
    serde_json::to_writer_pretty(&mut completed, &report)?;
    completed.write_all(b"\n")?;
    completed.sync_all()?;
    File::open(journal.parent().unwrap_or(Path::new(".")))?.sync_all()?;
    Ok(report)
}

#[cfg(test)]
#[path = "browser_tests.rs"]
mod tests;

pub(super) fn dispatch(args: &[OsString]) -> Result<bool> {
    let Some(flag) = args.get(1) else {
        return Ok(false);
    };
    if flag != "--browser-plan" && flag != "--browser-reclaim" {
        return Ok(false);
    }
    ensure!(
        args.len() == 4,
        "usage: reclaim_qemu_inputs --browser-plan|--browser-reclaim INVENTORY NEW_JOURNAL"
    );
    let repository = std::env::current_dir()?.canonicalize()?;
    let report = run(
        &repository,
        Path::new(&args[2]),
        Path::new(&args[3]),
        flag == "--browser-plan",
    )?;
    println!("Browser duplicates: {} objects, {} planned bytes, {} removed bytes; original failed verdict remains {:?}",
        report.objects.len(), report.planned_bytes, report.removed_bytes, report.original_run_assessment_passed);
    Ok(true)
}

//! Fixed, reviewed two-run storage relocation. No archive payload is a target.
//! Each phase is separate so another reviewer can inspect copied bytes before
//! publication, and the retained local backup before retirement.
use anyhow::{bail, ensure, Context, Result};
use md_sim_runtime::{
    file_lock::ExclusiveFileLock,
    store::{ObjectId, StoreLimits},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::CString,
    fs::{self, File, Metadata, OpenOptions},
    io::{Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{symlink, MetadataExt, OpenOptionsExt, PermissionsExt},
    },
    path::{Component, Path, PathBuf},
};

#[path = "support/bounded_file.rs"]
mod bounded_file;

const INVENTORY: &str = "work/simulator/reviews/runtime-relocation-inventory-517.json";
const INVENTORY_SHA: &str = "d43823ade46c8e282e8642dddd4c27d4f473b9babb4de68ecd9627371c145a1d";
const STORAGE_SHA: &str = "f2e2227a50ebf678fcb1138242204ee0a0b8dd62a372fe31826ad64e7ccbb98d";
const EXTERNAL: &str = "/home/user/mnt/small/md-sim-archive-20261019";
const MAX_FILE: u64 = 2 << 30;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Node {
    device: u64,
    inode: u64,
}
impl Node {
    fn of(m: &Metadata) -> Self {
        Self {
            device: m.dev(),
            inode: m.ino(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Attributes {
    mode: u32,
    uid: u32,
    gid: u32,
    mtime: i64,
    mtime_nsec: i64,
}
impl Attributes {
    fn of(m: &Metadata) -> Self {
        Self {
            mode: m.mode() & 0o7777,
            uid: m.uid(),
            gid: m.gid(),
            mtime: m.mtime(),
            mtime_nsec: m.mtime_nsec(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SavedFile {
    node: Node,
    attributes: Attributes,
    ctime: i64,
    ctime_nsec: i64,
    object: ObjectId,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct SavedDirectory {
    node: Node,
    attributes: Attributes,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    files: BTreeMap<String, SavedFile>,
    directories: BTreeMap<String, SavedDirectory>,
}

struct Plan {
    source: PathBuf,
    destination: PathBuf,
    backup: PathBuf,
    journal: PathBuf,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CopyReceipt {
    schema: u32,
    source: PathBuf,
    destination: PathBuf,
    backup: PathBuf,
    inventory: String,
    source_snapshot: Snapshot,
    destination_snapshot: Snapshot,
}

fn directory(path: &Path) -> Result<Metadata> {
    let m = fs::symlink_metadata(path)?;
    ensure!(
        m.is_dir(),
        "directory must be a real directory: {}",
        path.display()
    );
    Ok(m)
}

/// Check every ancestor, including an external mount point. A final run link
/// is inspected separately and never used for recursive deletion.
fn real_ancestors(path: &Path) -> Result<()> {
    ensure!(path.is_absolute(), "absolute path required");
    let mut at = PathBuf::from("/");
    for part in path.components() {
        match part {
            Component::RootDir => (),
            Component::Normal(p) => {
                at.push(p);
                directory(&at)?;
            }
            _ => bail!("non-normal path component"),
        }
    }
    Ok(())
}

fn create_parents(path: &Path) -> Result<()> {
    ensure!(path.is_absolute(), "absolute destination required");
    let mut at = PathBuf::from("/");
    for part in path.components() {
        match part {
            Component::RootDir => (),
            Component::Normal(p) => {
                let parent = at.clone();
                at.push(p);
                match fs::symlink_metadata(&at) {
                    Ok(m) => ensure!(m.is_dir(), "unsafe destination ancestor"),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        fs::create_dir(&at)?;
                        fs::set_permissions(&at, fs::Permissions::from_mode(0o700))?;
                        File::open(&parent)?.sync_all()?;
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            _ => bail!("non-normal destination component"),
        }
    }
    Ok(())
}

fn unchanged(a: &Metadata, b: &Metadata) -> bool {
    Node::of(a) == Node::of(b)
        && Attributes::of(a) == Attributes::of(b)
        && a.len() == b.len()
        && a.nlink() == b.nlink()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}

fn open_regular(path: &Path) -> Result<File> {
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let m = f.metadata()?;
    ensure!(
        m.is_file() && m.nlink() == 1 && m.len() <= MAX_FILE,
        "unsafe file type/link/size"
    );
    ensure!(
        m.mode() & 0o7000 == 0,
        "special permission bits are not supported"
    );
    no_extended_attributes(&f)?;
    ensure!(
        unchanged(&m, &fs::symlink_metadata(path)?),
        "file path changed"
    );
    Ok(f)
}

fn no_extended_attributes(file: &File) -> Result<()> {
    // Extended ACLs or capabilities cannot be silently discarded. This pilot
    // copies plain files only; a nonempty list needs a separately reviewed path.
    let size = unsafe { libc::flistxattr(file.as_raw_fd(), std::ptr::null_mut(), 0) };
    ensure!(
        size == 0,
        "extended attributes are unsupported: {}",
        std::io::Error::last_os_error()
    );
    Ok(())
}

fn inspect(path: &Path, mut destination: Option<&mut File>) -> Result<SavedFile> {
    let mut f = open_regular(path)?;
    let before = f.metadata()?;
    let mut hash = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0u8; 65536];
    loop {
        let n = f.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        count = count.checked_add(n as u64).context("byte count overflow")?;
        ensure!(count <= before.len(), "input grew during copy");
        hash.update(&buffer[..n]);
        if let Some(out) = destination.as_mut() {
            out.write_all(&buffer[..n])?;
        }
    }
    ensure!(
        count == before.len()
            && unchanged(&before, &f.metadata()?)
            && unchanged(&before, &fs::symlink_metadata(path)?),
        "input changed during read"
    );
    Ok(SavedFile {
        node: Node::of(&before),
        attributes: Attributes::of(&before),
        ctime: before.ctime(),
        ctime_nsec: before.ctime_nsec(),
        object: ObjectId {
            sha256: hex::encode(hash.finalize()),
            size: count,
        },
    })
}

fn tree(path: &Path) -> Result<Snapshot> {
    real_ancestors(path)?;
    let mut out = Snapshot {
        files: BTreeMap::new(),
        directories: BTreeMap::new(),
    };
    fn visit(root: &Path, rel: &str, out: &mut Snapshot) -> Result<()> {
        ensure!(rel.matches('/').count() <= 3, "unexpected run nesting");
        let path = root.join(rel);
        let meta = directory(&path)?;
        let fd = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(&path)?;
        ensure!(unchanged(&meta, &fd.metadata()?), "directory substituted");
        no_extended_attributes(&fd)?;
        out.directories.insert(
            rel.to_owned(),
            SavedDirectory {
                node: Node::of(&meta),
                attributes: Attributes::of(&meta),
            },
        );
        let names = fs::read_dir(&path)?
            .map(|e| e.map(|x| x.file_name()))
            .collect::<std::io::Result<BTreeSet<_>>>()?;
        for name in names {
            let name = name.to_str().context("non-UTF8 run path")?;
            ensure!(
                !name.is_empty() && name != "." && name != ".." && !name.contains('/'),
                "unsafe relative path"
            );
            let next = if rel.is_empty() {
                name.to_owned()
            } else {
                format!("{rel}/{name}")
            };
            let p = root.join(&next);
            let m = fs::symlink_metadata(&p)?;
            if m.is_dir() {
                visit(root, &next, out)?;
            } else {
                out.files.insert(next, inspect(&p, None)?);
            }
            ensure!(
                out.files.len() <= 128 && out.directories.len() <= 8,
                "run tree bound"
            );
        }
        let after = directory(&path)?;
        ensure!(unchanged(&meta, &after), "directory changed during scan");
        Ok(())
    }
    visit(path, "", &mut out)?;
    Ok(out)
}

fn lease(path: &Path) -> Result<ExclusiveFileLock> {
    real_ancestors(&path.join("inputs"))?;
    let p = path.join("inputs/store.lock");
    let f = open_regular(&p)?;
    let m = f.metadata()?;
    ensure!(m.len() == 0, "store lock must be empty");
    let lock = ExclusiveFileLock::acquire(f).context("store is active")?;
    ensure!(
        unchanged(&m, &fs::symlink_metadata(p)?),
        "store lock substituted"
    );
    ensure!(
        fs::read_dir(path.join("inputs/incoming"))?.next().is_none(),
        "pending transfer"
    );
    let limits: StoreLimits = serde_json::from_slice(&bounded_file::bounded(
        &path.join("inputs/limits.json"),
        4096,
    )?)?;
    limits.validate()?;
    ensure!(
        !path.join("inputs/limits.pending.json").try_exists()?,
        "pending store policy"
    );
    Ok(lock)
}

fn equal_contents(a: &Snapshot, b: &Snapshot) -> Result<()> {
    ensure!(
        a.files.keys().eq(b.files.keys()) && a.directories.keys().eq(b.directories.keys()),
        "tree members differ"
    );
    for (name, old) in &a.files {
        let new = &b.files[name];
        ensure!(
            old.object == new.object && old.attributes == new.attributes,
            "copied file differs: {name}"
        );
        ensure!(old.node != new.node, "copy shares original inode: {name}");
    }
    for (name, old) in &a.directories {
        ensure!(
            old.attributes == b.directories[name].attributes,
            "copied directory differs: {name}"
        );
    }
    Ok(())
}

fn apply_attributes(file: &File, attrs: &Attributes) -> Result<()> {
    ensure!(
        file.metadata()?.uid() == attrs.uid && file.metadata()?.gid() == attrs.gid,
        "copy owner/group differs; implicit ownership changes are forbidden"
    );
    file.set_permissions(fs::Permissions::from_mode(attrs.mode))?;
    let times = [
        libc::timespec {
            tv_sec: 0,
            tv_nsec: libc::UTIME_OMIT,
        },
        libc::timespec {
            tv_sec: attrs.mtime,
            tv_nsec: attrs.mtime_nsec,
        },
    ];
    // SAFETY: live owned descriptor and exactly two valid timestamps.
    ensure!(
        unsafe { libc::futimens(file.as_raw_fd(), times.as_ptr()) } == 0,
        "set copied mtime: {}",
        std::io::Error::last_os_error()
    );
    file.sync_all()?;
    Ok(())
}

fn save_new(path: &Path, value: &impl Serialize) -> Result<()> {
    real_ancestors(path.parent().context("journal parent")?)?;
    let bytes = serde_json::to_vec_pretty(value)?;
    ensure!(bytes.len() <= 512 * 1024, "journal bound");
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    f.write_all(&bytes)?;
    f.write_all(b"\n")?;
    f.sync_all()?;
    File::open(path.parent().unwrap())?.sync_all()?;
    Ok(())
}

fn copy(plan: &Plan, expected: &Snapshot) -> Result<()> {
    ensure!(
        fs::symlink_metadata(&plan.destination)
            .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
        "destination must be absent"
    );
    ensure!(
        fs::symlink_metadata(&plan.backup).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
        "backup must be absent"
    );
    create_parents(plan.destination.parent().context("destination parent")?)?;
    create_parents(&plan.journal)?;
    save_new(&plan.journal.join("copy-intent.json"), expected)?;
    fs::create_dir(&plan.destination)?;
    fs::set_permissions(&plan.destination, fs::Permissions::from_mode(0o700))?;
    File::open(plan.destination.parent().unwrap())?.sync_all()?;
    for name in expected.directories.keys().filter(|x| !x.is_empty()) {
        fs::create_dir(plan.destination.join(name))?;
        fs::set_permissions(
            plan.destination.join(name),
            fs::Permissions::from_mode(0o700),
        )?;
    }
    for (name, want) in &expected.files {
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(plan.destination.join(name))?;
        ensure!(
            inspect(&plan.source.join(name), Some(&mut out))? == *want,
            "source differs from copy intent"
        );
        apply_attributes(&out, &want.attributes)?;
    }
    // Children change directory mtimes. Restore deepest first, after all writes.
    for (name, want) in expected.directories.iter().rev() {
        apply_attributes(&File::open(plan.destination.join(name))?, &want.attributes)?;
    }
    let _destination_lock = lease(&plan.destination)?;
    let actual = tree(&plan.destination)?;
    equal_contents(expected, &actual)?;
    ensure!(
        tree(&plan.source)? == *expected,
        "source changed while copying"
    );
    save_new(
        &plan.journal.join("copy.json"),
        &CopyReceipt {
            schema: 1,
            source: plan.source.clone(),
            destination: plan.destination.clone(),
            backup: plan.backup.clone(),
            inventory: INVENTORY_SHA.into(),
            source_snapshot: expected.clone(),
            destination_snapshot: actual,
        },
    )
}

fn receipt(plan: &Plan) -> Result<CopyReceipt> {
    real_ancestors(&plan.journal)?;
    let r: CopyReceipt = serde_json::from_slice(&bounded_file::bounded(
        &plan.journal.join("copy.json"),
        512 * 1024,
    )?)?;
    ensure!(
        r.schema == 1
            && r.source == plan.source
            && r.destination == plan.destination
            && r.backup == plan.backup
            && r.inventory == INVENTORY_SHA,
        "copy receipt binding differs"
    );
    Ok(r)
}

fn link_target(plan: &Plan) -> Result<()> {
    ensure!(
        fs::symlink_metadata(&plan.source)?.file_type().is_symlink()
            && fs::read_link(&plan.source)? == plan.destination,
        "workspace link differs"
    );
    real_ancestors(&plan.destination)?;
    Ok(())
}

fn exchange(a: &Path, b: &Path) -> Result<()> {
    ensure!(a.parent() == b.parent(), "exchange must use one parent");
    real_ancestors(a.parent().context("exchange parent")?)?;
    let a_c = CString::new(a.as_os_str().as_encoded_bytes())?;
    let b_c = CString::new(b.as_os_str().as_encoded_bytes())?;
    // SAFETY: paths are checked, nul-terminated and remain alive. Atomic exchange
    // is mandatory; no two-rename fallback creates a missing workspace path.
    ensure!(
        unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                a_c.as_ptr(),
                libc::AT_FDCWD,
                b_c.as_ptr(),
                libc::RENAME_EXCHANGE,
            )
        } == 0,
        "atomic exchange: {}",
        std::io::Error::last_os_error()
    );
    File::open(a.parent().unwrap())?.sync_all()?;
    Ok(())
}

fn verify_pair(plan: &Plan, r: &CopyReceipt, published: bool) -> Result<()> {
    let local = if published {
        link_target(plan)?;
        &plan.backup
    } else {
        &plan.source
    };
    ensure!(tree(local)? == r.source_snapshot, "local copy changed");
    ensure!(
        tree(&plan.destination)? == r.destination_snapshot,
        "external copy changed"
    );
    equal_contents(&r.source_snapshot, &r.destination_snapshot)
}

fn publish(plan: &Plan, r: &CopyReceipt) -> Result<()> {
    verify_pair(plan, r, false)?;
    ensure!(
        fs::symlink_metadata(&plan.backup).is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound),
        "backup slot occupied"
    );
    save_new(&plan.journal.join("publish-intent.json"), r)?;
    symlink(&plan.destination, &plan.backup)?;
    File::open(plan.source.parent().unwrap())?.sync_all()?;
    exchange(&plan.source, &plan.backup)?;
    // A failed postcheck leaves both copies and durable intent for rollback.
    verify_pair(plan, r, true)?;
    save_new(&plan.journal.join("published.json"), r)
}

fn rollback(plan: &Plan, r: &CopyReceipt) -> Result<()> {
    // Also works after exchange succeeded but published.json was not written.
    verify_pair(plan, r, true)?;
    save_new(&plan.journal.join("rollback-intent.json"), r)?;
    exchange(&plan.source, &plan.backup)?;
    verify_pair(plan, r, false)?;
    ensure!(
        fs::read_link(&plan.backup)? == plan.destination,
        "rollback link differs"
    );
    save_new(&plan.journal.join("rolled-back.json"), r)
}

fn retire(plan: &Plan, r: &CopyReceipt) -> Result<()> {
    let published: CopyReceipt = serde_json::from_slice(&bounded_file::bounded(
        &plan.journal.join("published.json"),
        512 * 1024,
    )?)?;
    ensure!(
        serde_json::to_value(&published)? == serde_json::to_value(r)?,
        "publication receipt differs"
    );
    verify_pair(plan, r, true)?;
    // Both byte copies have just been rehashed under their store leases.
    // A partial retirement retains this complete restoration map; it cannot
    // be silently retried with an incomplete local source.
    save_new(&plan.journal.join("retire-intent.json"), r)?;
    for (name, want) in &r.source_snapshot.files {
        let path = plan.backup.join(name);
        ensure!(
            inspect(&path, None)? == *want,
            "local file changed before removal"
        );
        fs::remove_file(&path)?;
        File::open(path.parent().unwrap())?.sync_all()?;
    }
    for (name, want) in r.source_snapshot.directories.iter().rev() {
        let path = plan.backup.join(name);
        ensure!(
            Node::of(&directory(&path)?) == want.node,
            "backup directory substituted"
        );
        fs::remove_dir(&path)?;
        File::open(path.parent().unwrap())?.sync_all()?;
    }
    link_target(plan)?;
    ensure!(
        tree(&plan.destination)? == r.destination_snapshot,
        "external copy changed after retirement"
    );
    save_new(&plan.journal.join("retired.json"), r)
}

fn load_plan(root: &Path, name: &str) -> Result<(Plan, serde_json::Value)> {
    ensure!(
        matches!(
            name,
            "backend-build-513c-ubsan" | "backend-build-513d-ubsan"
        ),
        "run outside reviewed pilot"
    );
    real_ancestors(root)?;
    let inventory = bounded_file::capture(&root.join(INVENTORY), 128 * 1024)?;
    ensure!(
        inventory.object.sha256 == INVENTORY_SHA,
        "reviewed inventory differs"
    );
    let storage = bounded_file::capture(&root.join("catalog/storage.json"), 4096)?;
    ensure!(
        storage.object.sha256 == STORAGE_SHA,
        "registered archive policy differs"
    );
    let source = root.join("work/simulator/qemu-s9").join(name);
    let destination = Path::new(EXTERNAL)
        .join("runtime-work/md-sim/work/simulator/qemu-s9")
        .join(name);
    real_ancestors(Path::new(EXTERNAL))?;
    // Bind the visible registered mount, not an underlying shadowed mount at
    // the same pathname. This is the reviewed517 host topology only.
    ensure!(
        directory(Path::new(EXTERNAL))?.dev() == libc::makedev(8, 33),
        "registered mount device changed"
    );
    let mountinfo = fs::read_to_string("/proc/self/mountinfo")?;
    ensure!(
        mountinfo.lines().any(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            let Some(split) = fields.iter().position(|s| *s == "-") else {
                return false;
            };
            fields.len() > split + 3
                && fields[2] == "8:33"
                && fields[4] == "/home/user/mnt/small"
                && fields[split + 1] == "ext4"
                && !fields[5].split(',').any(|s| s == "noexec")
                && !fields[split + 3].split(',').any(|s| s == "noexec")
        }),
        "registered executable ext4 mount is absent"
    );
    let plan = Plan {
        backup: source.with_file_name(format!(".{name}.relocation-517")),
        source,
        destination,
        journal: root.join("work/simulator/relocation-517").join(name),
    };
    Ok((plan, serde_json::from_slice(&inventory.bytes)?))
}

fn validate_original(plan: &Plan, inventory: &serde_json::Value, actual: &Snapshot) -> Result<()> {
    let name = plan.source.file_name().unwrap().to_str().unwrap();
    let run = inventory["runs"]
        .as_array()
        .context("inventory runs")?
        .iter()
        .find(|x| {
            x["source"]
                .as_str()
                .is_some_and(|s| s.ends_with(&format!("/{name}")))
        })
        .context("run absent")?;
    ensure!(
        run["destination"].as_str() == plan.destination.to_str(),
        "inventory destination differs"
    );
    ensure!(
        actual.files.len() == 92 && actual.directories.len() == 4,
        "pilot inventory shape differs"
    );
    let meta = &run["source_metadata"];
    ensure!(
        actual.directories[""].node.device == meta["device"].as_u64().context("device")?
            && actual.directories[""].node.inode == meta["inode"].as_u64().context("inode")?
            && u64::from(actual.directories[""].attributes.mode)
                == meta["mode"].as_u64().context("mode")?,
        "run root substituted"
    );
    let expected = inventory["files"]
        .as_array()
        .context("file inventory")?
        .iter()
        .filter(|x| x["run"] == name)
        .collect::<Vec<_>>();
    ensure!(expected.len() == actual.files.len(), "file count differs");
    for entry in expected {
        let rel = entry["relative"].as_str().context("relative path")?;
        let have = actual.files.get(rel).context("inventoried file missing")?;
        ensure!(
            entry["sha256"] == have.object.sha256
                && entry["size"] == have.object.size
                && entry["device"] == have.node.device
                && entry["inode"] == have.node.inode
                && entry["nlink"] == 1
                && entry["mode"] == have.attributes.mode
                && entry["uid"] == have.attributes.uid
                && entry["gid"] == have.attributes.gid
                && entry["mtime_ns"].as_i64()
                    == Some(have.attributes.mtime * 1_000_000_000 + have.attributes.mtime_nsec)
                && entry["ctime_ns"].as_i64() == Some(have.ctime * 1_000_000_000 + have.ctime_nsec),
            "file identity differs: {rel}"
        );
    }
    for d in run["directories"].as_array().context("directories")? {
        let have = actual
            .directories
            .get(d["relative"].as_str().context("directory relative")?)
            .context("directory missing")?;
        ensure!(
            d["mode"] == have.attributes.mode
                && d["uid"] == have.attributes.uid
                && d["gid"] == have.attributes.gid,
            "directory permissions differ"
        );
    }
    let result: serde_json::Value = serde_json::from_slice(&bounded_file::bounded(
        &plan.source.join("result.json"),
        64 << 20,
    )?)?;
    ensure!(
        result["status"] == "exited"
            && result["exit_code"] == 0
            && result["trace_truncated"] == false,
        "pilot result is not the reviewed terminal build"
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 4,
        "usage: relocate_runtime_run ROOT RUN copy|verify|publish|rollback|retire"
    );
    let root = Path::new(&args[1]);
    let name = args[2].to_str().context("run name")?;
    let phase = args[3].to_str().context("phase")?;
    ensure!(
        matches!(phase, "copy" | "verify" | "publish" | "rollback" | "retire"),
        "unknown phase"
    );
    let (plan, inventory) = load_plan(root, name)?;
    if phase == "copy" {
        let _source = lease(&plan.source)?;
        let before = tree(&plan.source)?;
        validate_original(&plan, &inventory, &before)?;
        copy(&plan, &before)?;
    } else {
        let published = fs::symlink_metadata(&plan.source)?.file_type().is_symlink();
        let r = receipt(&plan)?;
        validate_original(&plan, &inventory, &r.source_snapshot)?;
        if phase == "verify"
            && published
            && fs::symlink_metadata(&plan.backup)
                .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        {
            link_target(&plan)?;
            let _destination = lease(&plan.destination)?;
            let retired: CopyReceipt = serde_json::from_slice(&bounded_file::bounded(
                &plan.journal.join("retired.json"),
                512 * 1024,
            )?)?;
            ensure!(
                serde_json::to_value(&retired)? == serde_json::to_value(&r)?,
                "retirement receipt differs"
            );
            ensure!(
                tree(&plan.destination)? == r.destination_snapshot,
                "retired destination differs"
            );
            println!(
                "{}",
                serde_json::json!({"phase":phase,"run":name,"passed":true,"retired":true,"journal":plan.journal})
            );
            return Ok(());
        }
        let local = if published {
            link_target(&plan)?;
            &plan.backup
        } else {
            &plan.source
        };
        let _source = lease(local)?;
        let _destination = lease(&plan.destination)?;
        match phase {
            "verify" => verify_pair(&plan, &r, published)?,
            "publish" => {
                ensure!(!published, "already published");
                publish(&plan, &r)?;
            }
            "rollback" => {
                ensure!(published, "not published");
                rollback(&plan, &r)?;
            }
            "retire" => {
                ensure!(published, "not published");
                retire(&plan, &r)?;
            }
            _ => unreachable!(),
        }
    }
    println!(
        "{}",
        serde_json::json!({"phase":phase,"run":name,"passed":true,"journal":plan.journal,
        "scope":"Verified storage transaction only; original run verdict and execution identities unchanged"})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use md_sim_runtime::store::Store;

    fn fixture() -> (tempfile::TempDir, Plan, ObjectId) {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("local/run");
        fs::create_dir_all(&source).unwrap();
        let payload = root.path().join("payload");
        fs::write(&payload, b"retained real bytes\n").unwrap();
        let id = md_sim_runtime::store::hash_file(&payload).unwrap();
        let store = Store::open(&source.join("inputs"), 16 << 20).unwrap();
        store.import(&payload, &id).unwrap();
        drop(store);
        fs::write(
            source.join("result.json"),
            b"{\"status\":\"exited\",\"exit_code\":1}\n",
        )
        .unwrap();
        fs::write(source.join("executable"), b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(source.join("executable"), fs::Permissions::from_mode(0o755)).unwrap();
        let plan = Plan {
            backup: root.path().join("local/.run.backup"),
            source,
            destination: root.path().join("external/runtime/run"),
            journal: root.path().join("receipts"),
        };
        (root, plan, id)
    }

    #[test]
    fn copy_publish_store_access_and_atomic_rollback_preserve_bytes_and_modes() {
        let (_root, p, id) = fixture();
        let source_lock = lease(&p.source).unwrap();
        let before = tree(&p.source).unwrap();
        copy(&p, &before).unwrap();
        let r = receipt(&p).unwrap();
        let destination_lock = lease(&p.destination).unwrap();
        assert!(Store::open(&p.source.join("inputs"), 16 << 20).is_err());
        assert!(Store::open(&p.destination.join("inputs"), 16 << 20).is_err());
        publish(&p, &r).unwrap();
        assert!(fs::symlink_metadata(&p.source)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(p.backup.is_dir());
        drop(destination_lock);
        drop(source_lock);
        let store = Store::open(&p.source.join("inputs"), 16 << 20).unwrap();
        assert_eq!(
            fs::read(store.verified_path(&id).unwrap()).unwrap(),
            b"retained real bytes\n"
        );
        drop(store);
        let _source = lease(&p.backup).unwrap();
        let _destination = lease(&p.destination).unwrap();
        rollback(&p, &r).unwrap();
        assert!(!fs::symlink_metadata(&p.source)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(tree(&p.source).unwrap(), before);
        assert_eq!(fs::read_link(&p.backup).unwrap(), p.destination);
        assert!(publish(&p, &r).is_err());
    }

    #[test]
    fn retirement_removes_only_the_verified_local_backup() {
        let (root, p, _) = fixture();
        let unrelated = root.path().join("local/unrelated");
        fs::write(&unrelated, b"keep").unwrap();
        let _source = lease(&p.source).unwrap();
        let before = tree(&p.source).unwrap();
        copy(&p, &before).unwrap();
        let r = receipt(&p).unwrap();
        let _destination = lease(&p.destination).unwrap();
        publish(&p, &r).unwrap();
        retire(&p, &r).unwrap();
        assert!(fs::symlink_metadata(&p.backup).is_err());
        assert_eq!(fs::read(unrelated).unwrap(), b"keep");
        assert_eq!(tree(&p.destination).unwrap(), r.destination_snapshot);
        assert_eq!(
            fs::read(p.source.join("result.json")).unwrap(),
            b"{\"status\":\"exited\",\"exit_code\":1}\n"
        );
        assert!(p.journal.join("retired.json").is_file());
    }

    #[test]
    fn changed_extra_linked_pending_or_active_inputs_fail_before_switch() {
        let (_root, p, _) = fixture();
        let source = lease(&p.source).unwrap();
        assert!(lease(&p.source).is_err());
        let before = tree(&p.source).unwrap();
        copy(&p, &before).unwrap();
        let r = receipt(&p).unwrap();
        fs::write(p.destination.join("result.json"), b"changed").unwrap();
        assert!(publish(&p, &r).is_err());
        assert!(!p.backup.exists());
        assert!(!p.journal.join("publish-intent.json").exists());
        fs::write(p.source.join("extra"), b"not inventoried").unwrap();
        assert_ne!(tree(&p.source).unwrap(), before);
        fs::remove_file(p.source.join("extra")).unwrap();
        let link = p.source.join("alias");
        fs::hard_link(p.source.join("result.json"), &link).unwrap();
        assert!(tree(&p.source).is_err());
        fs::remove_file(&link).unwrap();
        symlink("result.json", &link).unwrap();
        assert!(tree(&p.source).is_err());
        fs::remove_file(&link).unwrap();
        drop(source);
        fs::write(p.source.join("inputs/incoming/partial"), b"pending").unwrap();
        assert!(lease(&p.source).is_err());
    }

    #[test]
    fn journal_collision_and_post_exchange_interruption_keep_restore_path() {
        let (_root, p, _) = fixture();
        let _source = lease(&p.source).unwrap();
        let before = tree(&p.source).unwrap();
        create_parents(&p.journal).unwrap();
        save_new(&p.journal.join("copy-intent.json"), &before).unwrap();
        assert!(copy(&p, &before).is_err());
        assert!(!p.destination.exists());
        fs::remove_file(p.journal.join("copy-intent.json")).unwrap();
        copy(&p, &before).unwrap();
        let r = receipt(&p).unwrap();
        let _destination = lease(&p.destination).unwrap();
        // Simulate a crash after exchange and before the completion receipt.
        save_new(&p.journal.join("publish-intent.json"), &r).unwrap();
        symlink(&p.destination, &p.backup).unwrap();
        exchange(&p.source, &p.backup).unwrap();
        assert!(!p.journal.join("published.json").exists());
        rollback(&p, &r).unwrap();
        assert_eq!(tree(&p.source).unwrap(), before);
    }

    #[test]
    fn store_and_object_symlinks_are_rejected_without_new_store_state() {
        let (root, p, id) = fixture();
        let alias = root.path().join("alias");
        symlink(p.source.join("inputs"), &alias).unwrap();
        assert!(Store::open(&alias, 16 << 20).is_err());
        let objects = p.source.join("inputs/objects");
        let object = objects.join(id.sha256);
        let saved = root.path().join("saved-object");
        fs::rename(&object, &saved).unwrap();
        symlink(&saved, &object).unwrap();
        assert!(tree(&p.source).is_err());
        assert!(Store::open(&p.source.join("inputs"), 16 << 20).is_err());
    }

    #[test]
    #[ignore = "requires Linux user/mount/PID namespaces; actual isolated read through relocated parent"]
    fn isolated_read_uses_real_store_beneath_published_run_symlink() {
        use md_sim_runtime::isolation::{execute, Limits, Mount, ProcessSpec};
        use std::sync::{atomic::AtomicBool, Arc};
        let (_root, p, id) = fixture();
        let store = Store::open(&p.source.join("inputs"), 16 << 20).unwrap();
        let mut mounts = md_sim_firmware::lab::elf_closure(
            &store,
            vec![("/cat".into(), PathBuf::from("/usr/bin/cat"))],
        )
        .unwrap();
        mounts.push(Mount {
            guest_path: "/payload".into(),
            object: id,
        });
        drop(store);
        let source = lease(&p.source).unwrap();
        copy(&p, &tree(&p.source).unwrap()).unwrap();
        let r = receipt(&p).unwrap();
        let destination = lease(&p.destination).unwrap();
        publish(&p, &r).unwrap();
        drop(destination);
        drop(source);
        let store = Store::open(&p.source.join("inputs"), 16 << 20).unwrap();
        let spec = ProcessSpec {
            support_virtualization: None,
            private_channel: None,
            stdin_stream: None,
            namespace_uid: 0,
            dashboard: None,
            loopback: false,
            schema_version: "2.0".into(),
            argv: vec!["/cat".into(), "/payload".into()],
            inputs: mounts,
            stdin_object: None,
            environment: BTreeMap::new(),
            limits: Limits {
                wall_seconds: 10,
                cpu_seconds: 2,
                memory_bytes: 128 << 20,
                processes: 16,
                file_bytes: 1 << 20,
                trace_bytes: 1 << 20,
                tmp_bytes: 1 << 20,
            },
        };
        let result = execute(&store, &spec, Arc::new(AtomicBool::new(false))).unwrap();
        assert_eq!(result.status, "exited");
        assert_eq!(result.exit_code, Some(0));
        assert!(!result.trace_truncated);
        assert_eq!(result.stderr, "");
        assert_eq!(
            hex::decode(result.stdout).unwrap(),
            b"retained real bytes\n"
        );
    }
}

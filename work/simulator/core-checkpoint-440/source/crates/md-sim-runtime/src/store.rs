use crate::file_lock::ExclusiveFileLock;
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

pub const MAX_CHUNK: usize = 256 * 1024;
pub const DEFAULT_TOTAL_BYTES: u64 = 16 * 1024 * 1024 * 1024;
pub const DEFAULT_MAX_OBJECTS: u64 = 4096;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StoreLimits {
    pub max_object_bytes: u64,
    pub max_total_bytes: u64,
    pub max_objects: u64,
}
impl StoreLimits {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.max_object_bytes > 0 && self.max_total_bytes > 0 && self.max_objects > 0,
            "store limits must be positive"
        );
        ensure!(
            self.max_object_bytes <= self.max_total_bytes,
            "object limit exceeds aggregate store byte limit"
        );
        Ok(())
    }
}

/// Logical saved bytes and distinct hash slots, including incomplete transfers.
/// A published object's temporary second hard link does not count twice.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoreUsage {
    pub bytes: u64,
    pub objects: u64,
}

struct Inventory {
    usage: StoreUsage,
    hashes: BTreeSet<String>,
    published: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ObjectId {
    pub sha256: String,
    pub size: u64,
}

impl ObjectId {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.sha256.len() == 64
                && self
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "SHA-256 must contain 64 lowercase hexadecimal characters"
        );
        Ok(())
    }
}

pub fn hash_file(path: &Path) -> Result<ObjectId> {
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    ensure!(file.metadata()?.is_file(), "input is not a regular file");
    let mut digest = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
        size = size.checked_add(n as u64).context("file size overflow")?;
    }
    Ok(ObjectId {
        sha256: hex::encode(digest.finalize()),
        size,
    })
}

/// A private, single-writer store. The worker serializes all transfer requests.
/// Final objects are always verified again before execution or export.
pub struct Store {
    root: PathBuf,
    limits: StoreLimits,
    mutation: Mutex<()>,
    // Holding this descriptor excludes other Store handles and processes.
    _lock: ExclusiveFileLock,
}

impl Store {
    pub fn open(root: &Path, max_object_bytes: u64) -> Result<Self> {
        Self::open_with_limits(
            root,
            StoreLimits {
                max_object_bytes,
                max_total_bytes: DEFAULT_TOTAL_BYTES,
                max_objects: DEFAULT_MAX_OBJECTS,
            },
        )
    }

    pub fn open_with_limits(root: &Path, limits: StoreLimits) -> Result<Self> {
        limits.validate()?;
        fs::create_dir_all(root)?;
        ensure!(
            fs::symlink_metadata(root)?.is_dir(),
            "store root must be a real directory"
        );
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let name = entry.file_name();
            let metadata = fs::symlink_metadata(entry.path())?;
            if name == "objects" || name == "incoming" {
                ensure!(metadata.is_dir(), "store area must be a real directory");
            } else if name == "store.lock" {
                ensure!(
                    metadata.is_file() && metadata.nlink() == 1,
                    "unexpected or unsafe store root entry"
                );
            } else {
                ensure!(
                    (name == "limits.json" || name == "limits.pending.json")
                        && metadata.is_file()
                        && (1..=2).contains(&metadata.nlink())
                        && metadata.len() <= 4096,
                    "unexpected or unsafe store root entry"
                );
            }
        }
        fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
        let root = root.canonicalize()?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join("store.lock"))?;
        ensure!(
            lock.metadata()?.is_file()
                && lock.metadata()?.nlink() == 1
                && lock.metadata()?.len() == 0,
            "store lock must be an empty owned regular file"
        );
        let lock = ExclusiveFileLock::acquire(lock).context("store already has a writer")?;
        for dir in ["objects", "incoming"] {
            fs::create_dir_all(root.join(dir))?;
            ensure!(
                fs::symlink_metadata(root.join(dir))?.is_dir(),
                "store area must be a real directory"
            );
            fs::set_permissions(root.join(dir), fs::Permissions::from_mode(0o700))?;
        }
        let store = Self {
            root,
            limits,
            mutation: Mutex::new(()),
            _lock: lock,
        };
        store.inventory()?;
        store.initialize_policy()?;
        Ok(store)
    }

    fn read_policy(path: &Path) -> Result<(fs::Metadata, Vec<u8>)> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        let metadata = file.metadata()?;
        ensure!(metadata.is_file(), "store limits must be a regular file");
        let mut bytes = Vec::new();
        file.take(4097).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 4096, "store limits file exceeds its bound");
        Ok((metadata, bytes))
    }

    /// The reserved pending file is never authoritative until publication.
    /// Recovery can complete a full pending policy, or replace an incomplete
    /// serialization. No transfer is allowed before this operation succeeds.
    fn initialize_policy(&self) -> Result<()> {
        let policy = self.root.join("limits.json");
        let pending = self.root.join("limits.pending.json");
        let require_same = |bytes: &[u8]| -> Result<()> {
            let saved: StoreLimits = serde_json::from_slice(bytes)?;
            ensure!(
                saved == self.limits,
                "store limits differ from persisted policy; use the same limits or a new store"
            );
            Ok(())
        };
        if policy.exists() {
            let (metadata, bytes) = Self::read_policy(&policy)?;
            require_same(&bytes)?;
            if pending.exists() {
                let (partial, _) = Self::read_policy(&pending)?;
                ensure!(
                    metadata.dev() == partial.dev()
                        && metadata.ino() == partial.ino()
                        && metadata.nlink() == 2
                        && partial.nlink() == 2,
                    "store limits have an unexpected pending or external hard link"
                );
                fs::remove_file(pending)?;
                File::open(&self.root)?.sync_all()?;
            } else {
                ensure!(
                    metadata.nlink() == 1,
                    "store limits have an external hard link"
                );
            }
            return Ok(());
        }
        let needs_write = if pending.exists() {
            let (metadata, bytes) = Self::read_policy(&pending)?;
            ensure!(
                metadata.nlink() == 1,
                "pending limits have an external hard link"
            );
            if serde_json::from_slice::<serde_json::Value>(&bytes).is_ok() {
                require_same(&bytes)?;
                false
            } else {
                true
            }
        } else {
            true
        };
        if needs_write {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(&pending)?;
            serde_json::to_writer_pretty(&mut file, &self.limits)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
        }
        File::open(&pending)?.sync_all()?;
        // hard_link never overwrites a committed policy. A crash after this
        // step leaves two names for the same inode, which recovery recognizes.
        fs::hard_link(&pending, &policy)?;
        File::open(&self.root)?.sync_all()?;
        fs::remove_file(pending)?;
        File::open(&self.root)?.sync_all()?;
        Ok(())
    }

    pub fn limits(&self) -> &StoreLimits {
        &self.limits
    }

    pub fn usage(&self) -> Result<StoreUsage> {
        let _guard = self.writer()?;
        Ok(self.inventory()?.usage)
    }

    fn writer(&self) -> Result<MutexGuard<'_, ()>> {
        self.mutation
            .lock()
            .map_err(|_| anyhow::anyhow!("store writer lock poisoned"))
    }

    /// Recount disk state instead of trusting a counter that can become stale
    /// after a short write, interruption, abort or restart. The private writer
    /// lock makes the check and following mutation one atomic operation.
    fn inventory(&self) -> Result<Inventory> {
        let mut hashes = BTreeSet::new();
        let mut published = BTreeSet::new();
        let mut files = BTreeMap::<(u64, u64), (String, u64, u64, u64)>::new();
        let mut names = BTreeMap::new();
        for area in ["incoming", "objects"] {
            let dir = self.root.join(area);
            ensure!(
                fs::symlink_metadata(&dir)?.is_dir(),
                "store area is not a real directory"
            );
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("store name is not UTF-8"))?;
                let metadata = fs::symlink_metadata(entry.path())?;
                ensure!(metadata.is_file(), "store contains a non-regular object");
                let id = ObjectId {
                    sha256: name.clone(),
                    size: metadata.len(),
                };
                self.check(&id)?;
                let inode = (metadata.dev(), metadata.ino());
                if let Some(previous) = names.insert(name.clone(), inode) {
                    ensure!(
                        previous == inode,
                        "incoming and published hash refer to different files"
                    );
                }
                let file = files.entry(inode).or_insert((
                    name.clone(),
                    metadata.len(),
                    metadata.nlink(),
                    0,
                ));
                ensure!(
                    file.0 == name && file.1 == metadata.len() && file.2 == metadata.nlink(),
                    "store has an aliased or changing object"
                );
                file.3 += 1;
                hashes.insert(name.clone());
                ensure!(
                    hashes.len() as u64 <= self.limits.max_objects,
                    "store object-count limit exceeded"
                );
                if area == "objects" {
                    published.insert(name);
                }
            }
        }
        let mut bytes = 0u64;
        for (_, (_, length, links, local_links)) in files {
            ensure!(
                links == local_links,
                "store object has an external hard link"
            );
            bytes = bytes
                .checked_add(length)
                .context("store byte count overflow")?;
            ensure!(
                bytes <= self.limits.max_total_bytes,
                "store aggregate byte limit exceeded"
            );
        }
        Ok(Inventory {
            usage: StoreUsage {
                bytes,
                objects: hashes.len() as u64,
            },
            hashes,
            published,
        })
    }

    fn check(&self, id: &ObjectId) -> Result<()> {
        id.validate()?;
        ensure!(
            id.size <= self.limits.max_object_bytes,
            "object exceeds configured byte limit"
        );
        Ok(())
    }

    fn path(&self, id: &ObjectId, area: &str) -> PathBuf {
        self.root.join(area).join(&id.sha256)
    }

    pub fn verified_path(&self, id: &ObjectId) -> Result<PathBuf> {
        self.check(id)?;
        let p = self.path(id, "objects");
        ensure!(
            fs::symlink_metadata(&p)?.is_file(),
            "object is not a regular file"
        );
        ensure!(
            hash_file(&p)? == *id,
            "stored object identity differs from requested identity"
        );
        Ok(p)
    }

    pub fn begin(&self, id: &ObjectId) -> Result<u64> {
        let _guard = self.writer()?;
        self.check(id)?;
        let inventory = self.inventory()?;
        if self.path(id, "objects").exists() {
            self.verified_path(id)?;
            return Ok(id.size);
        }
        if !inventory.hashes.contains(&id.sha256) {
            ensure!(
                inventory.usage.objects < self.limits.max_objects,
                "store object-count quota exhausted"
            );
        }
        let path = self.path(id, "incoming");
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW);
        let file = options.open(path)?;
        ensure!(
            file.metadata()?.is_file(),
            "partial object is not a regular file"
        );
        let size = file.metadata()?.len();
        ensure!(size <= id.size, "partial object exceeds declared size");
        File::open(self.root.join("incoming"))?.sync_all()?;
        Ok(size)
    }

    pub fn chunk(&self, id: &ObjectId, offset: u64, bytes: &[u8]) -> Result<u64> {
        let _guard = self.writer()?;
        self.check(id)?;
        let inventory = self.inventory()?;
        ensure!(
            !inventory.published.contains(&id.sha256),
            "published objects cannot be changed"
        );
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_CHUNK,
            "invalid chunk length"
        );
        let end = offset
            .checked_add(bytes.len() as u64)
            .context("offset overflow")?;
        ensure!(end <= id.size, "chunk exceeds declared size");
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(self.path(id, "incoming"))?;
        ensure!(
            f.metadata()?.is_file(),
            "partial object is not a regular file"
        );
        let current = f.metadata()?.len();
        if offset < current {
            ensure!(end <= current, "retry overlaps the append boundary");
            f.seek(SeekFrom::Start(offset))?;
            let mut existing = vec![0; bytes.len()];
            f.read_exact(&mut existing)?;
            ensure!(existing == bytes, "retry differs from saved bytes");
            return Ok(current);
        }
        ensure!(
            offset == current,
            "chunk offset does not match saved length"
        );
        ensure!(
            inventory
                .usage
                .bytes
                .checked_add(bytes.len() as u64)
                .is_some_and(|total| total <= self.limits.max_total_bytes),
            "store aggregate byte quota exhausted"
        );
        f.seek(SeekFrom::End(0))?;
        f.write_all(bytes)?;
        f.sync_data()?;
        Ok(end)
    }

    pub fn finish(&self, id: &ObjectId) -> Result<PathBuf> {
        let _guard = self.writer()?;
        self.check(id)?;
        self.inventory()?;
        let dest = self.path(id, "objects");
        if dest.exists() {
            let verified = self.verified_path(id)?;
            let incoming = self.path(id, "incoming");
            if incoming.exists() {
                // Inventory permits this only for the same inode and hash.
                // Recover an interruption after publication but before unlink.
                fs::remove_file(incoming)?;
                File::open(self.root.join("incoming"))?.sync_all()?;
            }
            return Ok(verified);
        }
        let src = self.path(id, "incoming");
        ensure!(
            fs::symlink_metadata(&src)?.is_file(),
            "partial object is not a regular file"
        );
        ensure!(
            hash_file(&src)? == *id,
            "destination hash or size mismatch; partial object retained"
        );
        // Inputs can include guest programs. The private store permits read and
        // execution, never guest writes. Byte identity does not depend on mode.
        fs::set_permissions(&src, fs::Permissions::from_mode(0o555))?;
        File::open(&src)?.sync_all()?;
        fs::hard_link(&src, &dest).context("publish immutable object without overwriting")?;
        // Commit the destination before removing the only incoming name.
        File::open(self.root.join("objects"))?.sync_all()?;
        fs::remove_file(src)?;
        File::open(self.root.join("incoming"))?.sync_all()?;
        Ok(dest)
    }

    /// Cancel only an incomplete transfer. Published objects remain immutable.
    pub fn abort(&self, id: &ObjectId) -> Result<()> {
        let _guard = self.writer()?;
        self.check(id)?;
        self.inventory()?;
        let partial = self.path(id, "incoming");
        match fs::symlink_metadata(&partial) {
            Ok(metadata) => {
                ensure!(metadata.is_file(), "partial object is not a regular file");
                fs::remove_file(partial)?;
                File::open(self.root.join("incoming"))?.sync_all()?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }

    pub fn import(&self, path: &Path, expected: &ObjectId) -> Result<PathBuf> {
        self.check(expected)?;
        // Resolve a workspace link to its bytes, never copy the link itself.
        let actual = hash_file(path)?;
        ensure!(actual == *expected, "source hash or size mismatch");
        let mut offset = self.begin(expected)?;
        if offset == expected.size && self.path(expected, "objects").exists() {
            return self.verified_path(expected);
        }
        let mut f = File::open(path)?;
        f.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0; MAX_CHUNK];
        while offset < expected.size {
            let n = f.read(&mut bytes)?;
            if n == 0 {
                bail!("source changed during import");
            }
            offset = self.chunk(expected, offset, &bytes[..n])?;
        }
        self.finish(expected)
    }

    pub fn read_chunk(&self, id: &ObjectId, offset: u64, count: usize) -> Result<Vec<u8>> {
        ensure!(count <= MAX_CHUNK, "download chunk exceeds limit");
        ensure!(offset <= id.size, "download offset exceeds object");
        let mut file = File::open(self.verified_path(id)?)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0; count.min((id.size - offset) as usize)];
        file.read_exact(&mut bytes)?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transfer_resume_retry_and_destination_verification() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::open(root.path(), 100).unwrap();
        let id = ObjectId {
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
            size: 3,
        };
        assert_eq!(store.begin(&id).unwrap(), 0);
        assert_eq!(store.chunk(&id, 0, b"ab").unwrap(), 2);
        assert!(store.finish(&id).is_err());
        assert_eq!(store.begin(&id).unwrap(), 2);
        assert_eq!(store.chunk(&id, 0, b"ab").unwrap(), 2);
        assert!(store.chunk(&id, 0, b"xy").is_err());
        assert!(store.chunk(&id, 1, b"bc").is_err());
        assert!(store.chunk(&id, 3, b"c").is_err());
        store.chunk(&id, 2, b"c").unwrap();
        store.finish(&id).unwrap();
        assert_eq!(store.read_chunk(&id, 1, 100).unwrap(), b"bc");
        let final_path = store.verified_path(&id).unwrap();
        fs::set_permissions(&final_path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::write(final_path, b"abd").unwrap();
        assert!(store.verified_path(&id).is_err());
    }

    #[test]
    fn paths_and_budgets_are_not_controlled_by_hash_text() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::open(root.path(), 2).unwrap();
        assert!(store
            .begin(&ObjectId {
                sha256: "../escape".into(),
                size: 0
            })
            .is_err());
        assert!(store
            .begin(&ObjectId {
                sha256: "0".repeat(64),
                size: 3
            })
            .is_err());
    }

    #[test]
    fn rejected_transfer_can_restart_without_deleting_published_bytes() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::open(root.path(), 100).unwrap();
        let id = ObjectId {
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
            size: 3,
        };
        store.begin(&id).unwrap();
        store.chunk(&id, 0, b"bad").unwrap();
        assert!(store.finish(&id).is_err());
        store.abort(&id).unwrap();
        store.abort(&id).unwrap();
        assert_eq!(store.begin(&id).unwrap(), 0);
        store.chunk(&id, 0, b"abc").unwrap();
        store.finish(&id).unwrap();
        store.abort(&id).unwrap();
        assert_eq!(store.read_chunk(&id, 0, 3).unwrap(), b"abc");
    }
}

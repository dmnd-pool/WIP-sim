//! Bounded reads of a completed result. Hash once per unchanged file version.
//! The receiver must still hash the complete retrieved bytes before cleanup.
use crate::{isolation::MAX_TRACE_BYTES, store::ObjectId};
use anyhow::{ensure, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::{Metadata, OpenOptions},
    io::{Read, Seek, SeekFrom},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

#[derive(PartialEq, Eq)]
struct Stamp {
    device: u64,
    inode: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl Stamp {
    fn new(metadata: Metadata) -> Result<Self> {
        ensure!(
            metadata.is_file() && metadata.nlink() == 1,
            "result must be a private regular file"
        );
        ensure!(
            metadata.len() <= 2 * MAX_TRACE_BYTES + 1024 * 1024,
            "result file exceeds the complete trace envelope"
        );
        Ok(Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        })
    }
}

#[derive(Default)]
pub(super) struct Reader {
    // One entry bounds memory and descriptors regardless of retained run count.
    saved: Option<(PathBuf, Stamp, ObjectId)>,
    #[cfg(test)]
    bytes_hashed: u64,
}
impl Reader {
    pub(super) fn read(
        &mut self,
        path: &Path,
        expected: &ObjectId,
        offset: u64,
        count: usize,
    ) -> Result<Vec<u8>> {
        expected.validate()?;
        ensure!(
            count > 0 && count <= crate::store::MAX_CHUNK,
            "result chunk size exceeds bound"
        );
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        let stamp = Stamp::new(file.metadata()?)?;
        ensure!(
            offset <= expected.size && stamp.size == expected.size,
            "result extent changed"
        );
        let cached = self
            .saved
            .as_ref()
            .is_some_and(|(saved_path, saved_stamp, _)| {
                saved_path == path && *saved_stamp == stamp
            });
        if !cached {
            let mut hash = Sha256::new();
            let mut bytes = [0u8; 65536];
            let mut total = 0u64;
            loop {
                let count = file.read(&mut bytes)?;
                if count == 0 {
                    break;
                }
                total += count as u64;
                ensure!(total <= stamp.size, "result grew during hashing");
                hash.update(&bytes[..count]);
            }
            ensure!(
                total == stamp.size && Stamp::new(file.metadata()?)? == stamp,
                "result changed during hashing"
            );
            #[cfg(test)]
            {
                self.bytes_hashed += total;
            }
            self.saved = Some((
                path.into(),
                stamp,
                ObjectId {
                    sha256: hex::encode(hash.finalize()),
                    size: total,
                },
            ));
        }
        let (_, stamp, identity) = self.saved.as_ref().expect("verified result version");
        ensure!(identity == expected, "saved result identity changed");
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = vec![0u8; (expected.size - offset).min(count as u64) as usize];
        file.read_exact(&mut bytes)?;
        ensure!(
            Stamp::new(file.metadata()?)? == *stamp,
            "result changed during range read"
        );
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, io::Write, os::unix::fs::symlink};
    #[test]
    fn repeated_ranges_hash_once_and_reject_same_length_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("result.json");
        fs::write(&path, vec![b'a'; 1_000_000]).unwrap();
        // Standard SHA-256 million-'a' vector, independent of this reader.
        let id = ObjectId {
            sha256: "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0".into(),
            size: 1_000_000,
        };
        let mut reader = Reader::default();
        assert_eq!(reader.read(&path, &id, 0, 17).unwrap(), vec![b'a'; 17]);
        assert_eq!(
            reader.read(&path, &id, 999990, 256).unwrap(),
            vec![b'a'; 10]
        );
        assert_eq!(reader.bytes_hashed, 1_000_000);
        let mut wrong = id.clone();
        wrong.sha256 = "00".repeat(32);
        assert!(reader.read(&path, &wrong, 0, 1).is_err());
        let mut file = OpenOptions::new().write(true).open(&path).unwrap();
        file.write_all(b"b").unwrap();
        file.sync_all().unwrap();
        assert!(reader.read(&path, &id, 0, 1).is_err());
        assert_eq!(reader.bytes_hashed, 2_000_000);
        assert!(reader.read(&path, &id, 1_000_001, 1).is_err());
    }
    #[test]
    fn replacement_inode_links_and_growth_cannot_reuse_an_old_verification() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("result.json");
        fs::write(&path, b"abc").unwrap();
        let id = ObjectId {
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
            size: 3,
        };
        let mut reader = Reader::default();
        assert_eq!(reader.read(&path, &id, 0, 3).unwrap(), b"abc");
        let replacement = dir.path().join("replacement");
        fs::write(&replacement, b"abc").unwrap();
        fs::rename(&replacement, &path).unwrap();
        assert_eq!(reader.read(&path, &id, 0, 3).unwrap(), b"abc");
        assert_eq!(reader.bytes_hashed, 6);
        fs::hard_link(&path, &replacement).unwrap();
        assert!(reader.read(&path, &id, 0, 3).is_err());
        fs::remove_file(&replacement).unwrap();
        fs::rename(&path, &replacement).unwrap();
        symlink(&replacement, &path).unwrap();
        assert!(reader.read(&path, &id, 0, 3).is_err());
        fs::remove_file(&path).unwrap();
        fs::write(&path, b"abcd").unwrap();
        assert!(reader.read(&path, &id, 0, 3).is_err());
    }
}

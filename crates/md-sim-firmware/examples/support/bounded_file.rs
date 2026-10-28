//! Bounded regular-file input shared by preparation and result readers.
use anyhow::{ensure, Result};
use md_sim_runtime::store::ObjectId;
use std::{
    fs,
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

pub struct CapturedFile {
    pub bytes: Vec<u8>,
    pub object: ObjectId,
}

pub fn bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    ensure!(limit < u64::MAX, "file input limit overflow");
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let before = file.metadata()?;
    ensure!(
        before.is_file() && before.len() <= limit,
        "input file type or size bound"
    );
    let mut bytes = Vec::new();
    (&mut file).take(limit + 1).read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let identity = |m: &fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    ensure!(
        identity(&before) == identity(&after) && bytes.len() as u64 == before.len(),
        "input file changed while reading"
    );
    ensure!(bytes.len() as u64 <= limit, "input file grew beyond bound");
    Ok(bytes)
}

pub fn capture(path: &Path, limit: u64) -> Result<CapturedFile> {
    let bytes = bounded(path, limit)?;
    let object = ObjectId {
        sha256: md_sim_firmware::prepare::hash_bytes(&bytes),
        size: bytes.len() as u64,
    };
    Ok(CapturedFile { bytes, object })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::CString, os::unix::fs::symlink};

    #[test]
    fn type_alias_and_size_fail_without_opening_a_blocking_stream() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("input");
        let link = dir.path().join("link");
        let fifo = dir.path().join("fifo");
        fs::write(&file, b"abc").unwrap();
        symlink(&file, &link).unwrap();
        let name = CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        for bad in [link.as_path(), fifo.as_path(), dir.path()] {
            assert!(capture(bad, 100).is_err());
        }
        assert!(capture(&file, 2).is_err());
        assert!(capture(&file, u64::MAX).is_err());
    }

    #[test]
    fn identity_describes_consumed_bytes_after_path_changes() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("input");
        fs::write(&file, b"abc").unwrap();
        let captured = capture(&file, 3).unwrap();
        fs::write(&file, b"xyz").unwrap();
        assert_eq!(captured.bytes, b"abc");
        assert_eq!(captured.object.size, 3);
        assert_eq!(
            captured.object.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_ne!(captured.object, capture(&file, 3).unwrap().object);
    }
}

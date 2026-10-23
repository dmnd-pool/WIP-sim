//! A private listener can recover only the exact socket inode it published.
//! The worker root lock and this endpoint lease remain held during service.
use crate::{file_lock::ExclusiveFileLock, isolation::save_json};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::Read,
    os::unix::{
        fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt},
        net::UnixListener,
    },
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema_version: u32,
    path: PathBuf,
    device: u64,
    inode: u64,
    owner: u32,
}

fn endpoint_path(path: &Path) -> Result<PathBuf> {
    let name = path
        .file_name()
        .context("worker socket needs a file name")?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = parent.canonicalize()?;
    let metadata = fs::metadata(&parent)?;
    // A writable shared parent needs sticky-directory deletion protection.
    ensure!(
        metadata.is_dir() && (metadata.mode() & 0o022 == 0 || metadata.mode() & 0o1000 != 0),
        "worker socket parent is not protected against replacement by other accounts"
    );
    let path = parent.join(name);
    ensure!(
        path.as_os_str().len() <= 4096,
        "worker socket path exceeds limit"
    );
    Ok(path)
}

fn safe_file(metadata: &Metadata) -> bool {
    metadata.is_file()
        && metadata.nlink() == 1
        && metadata.uid() == unsafe { libc::geteuid() }
        && metadata.mode() & 0o022 == 0
}

fn read_receipt(root: &Path) -> Result<Option<Receipt>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(root.join("endpoint.json"))
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        safe_file(&file.metadata()?) && file.metadata()?.len() <= 8192,
        "worker endpoint receipt is unsafe or oversized"
    );
    let mut bytes = Vec::new();
    file.take(8193).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 8192, "worker endpoint receipt exceeds limit");
    let receipt: Receipt = serde_json::from_slice(&bytes)?;
    ensure!(
        receipt.schema_version == 1 && receipt.owner == unsafe { libc::geteuid() },
        "worker endpoint receipt version or owner differs"
    );
    Ok(Some(receipt))
}

fn socket_receipt(path: &Path) -> Result<Option<Receipt>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        metadata.file_type().is_socket()
            && metadata.nlink() == 1
            && metadata.uid() == unsafe { libc::geteuid() },
        "worker endpoint is not an owned socket inode"
    );
    Ok(Some(Receipt {
        schema_version: 1,
        path: path.to_owned(),
        device: metadata.dev(),
        inode: metadata.ino(),
        owner: metadata.uid(),
    }))
}

fn remove_recorded_socket(receipt: &Receipt) -> Result<()> {
    if let Some(actual) = socket_receipt(&receipt.path)? {
        ensure!(
            actual == *receipt,
            "worker socket differs from recorded inode; retained"
        );
        fs::remove_file(&receipt.path)?;
        File::open(receipt.path.parent().context("socket has no parent")?)?.sync_all()?;
    }
    Ok(())
}

fn recover(root: &Path, path: &Path) -> Result<()> {
    let saved = read_receipt(root)?;
    if let Some(receipt) = saved {
        ensure!(
            receipt.path == path,
            "worker endpoint path differs from retained lease"
        );
        remove_recorded_socket(&receipt)?;
        fs::remove_file(root.join("endpoint.json"))?;
        File::open(root)?.sync_all()?;
    } else {
        ensure!(
            socket_receipt(path)?.is_none(),
            "unrecorded socket retained; ownership cannot be established"
        );
    }
    Ok(())
}

pub(super) struct Endpoint {
    root: PathBuf,
    receipt: Receipt,
    listener: Option<UnixListener>,
    _lease: ExclusiveFileLock,
    closed: bool,
}
impl Endpoint {
    pub(super) fn bind(root: &Path, socket: &Path) -> Result<Self> {
        let lease = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join("endpoint.lock"))?;
        ensure!(
            safe_file(&lease.metadata()?),
            "worker endpoint lock is unsafe"
        );
        let lease =
            ExclusiveFileLock::acquire(lease).context("worker endpoint is already leased")?;
        let path = endpoint_path(socket)?;
        recover(root, &path)?;
        let listener = UnixListener::bind(&path).context("bind worker socket")?;
        let receipt = socket_receipt(&path)?.context("bound worker socket disappeared")?;
        let mut endpoint = Self {
            root: root.to_owned(),
            receipt,
            listener: Some(listener),
            _lease: lease,
            closed: false,
        };
        // No request is accepted before the exact inode identity is durable.
        // Abrupt loss between bind and publication is deliberately fail-closed:
        // an unrecorded socket is never deleted on a later start.
        if let Err(error) = (|| -> Result<()> {
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
            save_json(&root.join("endpoint.json"), &endpoint.receipt)?;
            File::open(path.parent().context("socket has no parent")?)?.sync_all()?;
            Ok(())
        })() {
            endpoint.listener.take();
            remove_recorded_socket(&endpoint.receipt)?;
            endpoint.closed = true;
            return Err(error);
        }
        Ok(endpoint)
    }
    pub(super) fn listener(&self) -> &UnixListener {
        self.listener.as_ref().expect("live endpoint")
    }
    fn cleanup(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.listener.take();
        ensure!(
            read_receipt(&self.root)?.as_ref() == Some(&self.receipt),
            "worker endpoint receipt changed; retained"
        );
        remove_recorded_socket(&self.receipt)?;
        fs::remove_file(self.root.join("endpoint.json"))?;
        File::open(&self.root)?.sync_all()?;
        self.closed = true;
        Ok(())
    }
    pub(super) fn close(mut self) -> Result<()> {
        self.cleanup()
    }
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStrExt;

    // This creates only a filesystem inode. It does not bind a listener or
    // establish network restart behavior. The separate guest test does that.
    fn socket_inode(path: &Path) {
        let path = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(
            unsafe { libc::mknod(path.as_ptr(), libc::S_IFSOCK | 0o600, 0) },
            0,
            "{}",
            std::io::Error::last_os_error()
        );
    }
    #[test]
    fn only_exact_recorded_socket_is_recovered() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("worker.sock");
        socket_inode(&path);
        assert!(recover(temp.path(), &path).is_err());
        assert!(path.exists());
        let receipt = socket_receipt(&path).unwrap().unwrap();
        save_json(&temp.path().join("endpoint.json"), &receipt).unwrap();
        recover(temp.path(), &path).unwrap();
        assert!(!path.exists());
        recover(temp.path(), &path).unwrap();

        socket_inode(&path);
        let receipt = socket_receipt(&path).unwrap().unwrap();
        save_json(&temp.path().join("endpoint.json"), &receipt).unwrap();
        let old = temp.path().join("old.sock");
        fs::rename(&path, &old).unwrap();
        socket_inode(&path);
        assert!(recover(temp.path(), &path).is_err());
        assert!(path.exists() && old.exists());
        fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&old, &path).unwrap();
        assert!(recover(temp.path(), &path).is_err());
        assert!(old.exists());
    }
    #[test]
    fn foreign_files_and_receipts_are_retained() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("worker.sock");
        fs::write(&path, b"unrelated").unwrap();
        assert!(recover(temp.path(), &path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"unrelated");
        std::os::unix::fs::symlink(&path, temp.path().join("endpoint.json")).unwrap();
        assert!(read_receipt(temp.path()).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"unrelated");
    }
}

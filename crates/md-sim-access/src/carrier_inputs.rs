//! Fetch small captured access inputs through the real worker protocol.
//! The worker's private store path is never mounted or inferred by the caller.
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::WorkerTransport;
use md_sim_runtime::{
    store::{ObjectId, MAX_CHUNK},
    worker::Request,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::Path,
};

pub const MAX_BYTES: u64 = 4 << 20;

/// Parse the canonical CLI form. No whitespace, alternate integer spelling,
/// uppercase digest, extra field, or path is accepted.
pub fn parse_object(text: &str) -> Result<ObjectId> {
    ensure!(text.len() <= 85, "object token exceeds bound");
    let (sha256, size) = text
        .split_once(':')
        .context("object token must be SHA256:SIZE")?;
    let parsed: u64 = size.parse().context("invalid object size")?;
    ensure!(
        parsed.to_string() == size,
        "object size is not canonical decimal"
    );
    let object = ObjectId {
        sha256: sha256.into(),
        size: parsed,
    };
    object.validate()?;
    Ok(object)
}

fn response(value: Value, object: &ObjectId, offset: u64, expected: usize) -> Result<Vec<u8>> {
    ensure!(
        value.as_object().is_some_and(|fields| fields.len() == 3),
        "unexpected worker Get response fields"
    );
    ensure!(
        serde_json::from_value::<ObjectId>(value["object"].clone())? == *object
            && value["offset"].as_u64() == Some(offset),
        "worker Get object or offset differs"
    );
    let encoded = value["hex"].as_str().context("worker Get bytes absent")?;
    ensure!(
        encoded.len() == expected * 2
            && encoded
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "worker Get byte count, EOF or hex differs"
    );
    Ok(hex::decode(encoded)?)
}

struct Pending {
    directory: File,
    file: File,
    name: CString,
}

impl Pending {
    fn new(directory: File) -> Result<Self> {
        let mut random = [0; 16];
        File::open("/dev/urandom")?.read_exact(&mut random)?;
        let name = CString::new(format!(".carrier-input-{}.pending", hex::encode(random)))?;
        let descriptor = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            )
        };
        ensure!(
            descriptor >= 0,
            "create private input staging file: {}",
            std::io::Error::last_os_error()
        );
        Ok(Self {
            directory,
            file: unsafe { File::from_raw_fd(descriptor) },
            name,
        })
    }

    fn owns_name(&self) -> bool {
        let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe {
            libc::fstatat(
                self.directory.as_raw_fd(),
                self.name.as_ptr(),
                metadata.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            return false;
        }
        let metadata = unsafe { metadata.assume_init() };
        self.file.metadata().is_ok_and(|file| {
            file.dev() == metadata.st_dev
                && file.ino() == metadata.st_ino
                && metadata.st_mode & libc::S_IFMT == libc::S_IFREG
                && metadata.st_nlink == 1
        })
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if self.owns_name() {
            let _ = unsafe { libc::unlinkat(self.directory.as_raw_fd(), self.name.as_ptr(), 0) };
            let _ = self.directory.sync_all();
        }
    }
}

/// Fetch and verify one small object into a new file in an owner-only directory.
/// Each Get is bounded by the transport. Publication never replaces an existing
/// destination. Partial bytes are removed on failure; the archive is untouched.
pub fn fetch(
    transport: &mut impl WorkerTransport,
    object: &ObjectId,
    destination: &Path,
    max_bytes: u64,
) -> Result<()> {
    object.validate()?;
    ensure!(
        (1..=MAX_BYTES).contains(&max_bytes) && object.size <= max_bytes,
        "captured access input exceeds its byte bound"
    );
    ensure!(
        destination.is_absolute(),
        "captured access destination must be absolute"
    );
    let name = CString::new(
        destination
            .file_name()
            .context("input destination filename absent")?
            .as_encoded_bytes(),
    )?;
    let parent = destination
        .parent()
        .context("input destination parent absent")?;
    let directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(parent)?;
    let metadata = directory.metadata()?;
    ensure!(
        metadata.is_dir()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o077 == 0,
        "captured access inputs require an owner-only directory"
    );
    let mut existing = std::mem::MaybeUninit::<libc::stat>::uninit();
    ensure!(
        unsafe {
            libc::fstatat(
                directory.as_raw_fd(),
                name.as_ptr(),
                existing.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
            && std::io::Error::last_os_error().raw_os_error() == Some(libc::ENOENT),
        "input destination already exists or cannot be checked"
    );
    let mut pending = Pending::new(directory)?;
    let mut offset = 0;
    let mut digest = Sha256::new();
    while offset < object.size {
        let count = (object.size - offset).min(MAX_CHUNK as u64) as usize;
        let value = transport.request(&Request::Get {
            object: object.clone(),
            offset,
            count,
        })?;
        let bytes = response(value, object, offset, count)?;
        pending.file.write_all(&bytes)?;
        digest.update(&bytes);
        offset += bytes.len() as u64;
    }
    let eof = transport.request(&Request::Get {
        object: object.clone(),
        offset: object.size,
        count: 1,
    })?;
    response(eof, object, object.size, 0)?;
    ensure!(
        hex::encode(digest.finalize()) == object.sha256
            && pending.file.metadata()?.len() == object.size,
        "captured access input hash or size differs"
    );
    pending.file.sync_all()?;
    ensure!(
        pending.owns_name(),
        "input staging entry changed before publication"
    );
    ensure!(
        unsafe {
            libc::linkat(
                pending.directory.as_raw_fd(),
                pending.name.as_ptr(),
                pending.directory.as_raw_fd(),
                name.as_ptr(),
                0,
            )
        } == 0,
        "publish captured input without overwriting: {}",
        std::io::Error::last_os_error()
    );
    // After publication this exact inode has two links. Remove our staging
    // name explicitly; Drop only removes an unpublished one-link file.
    ensure!(
        unsafe { libc::unlinkat(pending.directory.as_raw_fd(), pending.name.as_ptr(), 0) } == 0,
        "remove published input staging link: {}",
        std::io::Error::last_os_error()
    );
    pending.directory.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use md_sim_coordinator::executor::ConnectedTransport;
    use md_sim_runtime::{store::Store, worker};
    use serde_json::json;
    use std::{
        fs,
        os::unix::{fs::PermissionsExt, net::UnixStream},
    };

    #[test]
    fn object_token_is_strict_and_canonical() {
        let good = format!("{}:123", "a".repeat(64));
        assert_eq!(parse_object(&good).unwrap().size, 123);
        for bad in [
            format!("{}:01", "a".repeat(64)),
            format!("{}:+1", "a".repeat(64)),
            format!("{}:1", "A".repeat(64)),
            format!("{}:1:2", "a".repeat(64)),
            format!("{}: 1", "a".repeat(64)),
            format!("{}:18446744073709551616", "a".repeat(64)),
        ] {
            assert!(parse_object(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn actual_worker_get_publishes_exact_multi_chunk_bytes_and_checks_eof() -> Result<()> {
        let root = tempfile::tempdir()?;
        let worker_root = root.path().join("worker");
        fs::create_dir(&worker_root)?;
        let store = Store::open_with_limits(
            &worker_root.join("store"),
            worker::WorkerLimits::default().store,
        )?;
        let bytes = (0..MAX_CHUNK + 17)
            .map(|n| (n % 251) as u8)
            .collect::<Vec<_>>();
        let object = ObjectId {
            sha256: hex::encode(Sha256::digest(&bytes)),
            size: bytes.len() as u64,
        };
        store.begin(&object)?;
        for (index, chunk) in bytes.chunks(MAX_CHUNK).enumerate() {
            store.chunk(&object, (index * MAX_CHUNK) as u64, chunk)?;
        }
        store.finish(&object)?;
        drop(store);
        let (client, server) = UnixStream::pair()?;
        let service = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            let incoming = std::iter::from_fn(move || {
                while std::time::Instant::now() < deadline {
                    let mut event = libc::pollfd {
                        fd: server.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    let ready = unsafe { libc::poll(&mut event, 1, 50) };
                    if ready < 0 {
                        return Some(Err(std::io::Error::last_os_error().into()));
                    }
                    if event.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
                        return None;
                    }
                    if event.revents & libc::POLLIN != 0 {
                        return Some(server.try_clone().map_err(Into::into));
                    }
                }
                None
            });
            worker::serve_connections(&worker_root, incoming)
        });
        let mut transport = ConnectedTransport::new(client);
        let output = root.path().join("inputs");
        fs::create_dir(&output)?;
        fs::set_permissions(&output, fs::Permissions::from_mode(0o700))?;
        let path = output.join("captured.json");
        let fetched = fetch(&mut transport, &object, &path, MAX_BYTES);
        let shutdown = transport.request(&Request::Shutdown);
        let ended = service
            .join()
            .map_err(|_| anyhow::anyhow!("worker service panic"))?;
        ended.context("actual worker Get service")?;
        fetched?;
        shutdown?;
        assert_eq!(fs::read(&path)?, bytes);
        assert_eq!(fs::read_dir(&output)?.count(), 1);
        assert_eq!(fs::metadata(&path)?.mode() & 0o777, 0o600);
        Ok(())
    }

    struct Bad {
        kind: u8,
        bytes: Vec<u8>,
    }
    impl WorkerTransport for Bad {
        fn request(&mut self, request: &Request) -> Result<Value> {
            let Request::Get {
                object,
                offset,
                count,
            } = request
            else {
                anyhow::bail!("unexpected request")
            };
            let end = (*offset as usize + count).min(self.bytes.len());
            let data = &self.bytes[*offset as usize..end];
            let mut response = json!({"object":object,"offset":offset,"hex":hex::encode(data)});
            match self.kind {
                0 => response["object"]["size"] = json!(999),
                1 => response["offset"] = json!(offset + 1),
                2 => response["hex"] = json!("zz"),
                3 => response["hex"] = json!(""),
                4 if *offset == object.size => response["hex"] = json!("00"),
                5 if !data.is_empty() => response["hex"] = json!("00".repeat(data.len())),
                6 => response["unexpected"] = json!(true),
                _ => {}
            }
            Ok(response)
        }
        fn description(&self) -> String {
            "invalid Get response regression".into()
        }
    }

    #[test]
    fn changed_ranges_hex_eof_or_hash_never_publish_and_existing_files_survive() -> Result<()> {
        let root = tempfile::tempdir()?;
        let bytes = b"captured policy".to_vec();
        let object = ObjectId {
            sha256: hex::encode(Sha256::digest(&bytes)),
            size: bytes.len() as u64,
        };
        let path = root.path().join("input.json");
        for kind in 0..7 {
            assert!(fetch(
                &mut Bad {
                    kind,
                    bytes: bytes.clone()
                },
                &object,
                &path,
                MAX_BYTES
            )
            .is_err());
            assert_eq!(fs::read_dir(root.path())?.count(), 0);
        }
        fs::write(&path, b"preserve")?;
        assert!(fetch(
            &mut Bad {
                kind: 6,
                bytes: bytes.clone()
            },
            &object,
            &path,
            MAX_BYTES
        )
        .is_err());
        assert_eq!(fs::read(&path)?, b"preserve");
        fs::remove_file(&path)?;
        let protected = root.path().join("protected");
        fs::write(&protected, b"original")?;
        std::os::unix::fs::symlink(&protected, &path)?;
        assert!(fetch(
            &mut Bad {
                kind: 7,
                bytes: bytes.clone()
            },
            &object,
            &path,
            MAX_BYTES
        )
        .is_err());
        assert_eq!(fs::read(&protected)?, b"original");
        assert!(fs::symlink_metadata(&path)?.file_type().is_symlink());
        fs::remove_file(&path)?;
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755))?;
        assert!(fetch(&mut Bad { kind: 6, bytes }, &object, &path, MAX_BYTES).is_err());
        Ok(())
    }
}

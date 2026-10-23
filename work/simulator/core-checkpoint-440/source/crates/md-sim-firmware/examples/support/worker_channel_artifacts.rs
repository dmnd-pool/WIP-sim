//! Bounded transport of complete worker artifacts through the VM console.
//! Callers supply run-owned paths and limits. Import publishes only verified
//! bytes, without replacing an existing destination. It accepts one gzip member.
//! The caller must bound the JSON envelope before deserializing Export.
use anyhow::{ensure, Context, Result};
use flate2::{bufread::GzDecoder, write::GzEncoder, Compression};
use md_sim_runtime::store::{hash_file, ObjectId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, Metadata, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub object: ObjectId,
    pub gzip_hex: String,
}

#[derive(PartialEq, Eq)]
struct Stamp {
    device: u64,
    inode: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

impl Stamp {
    fn read(metadata: Metadata) -> Result<Self> {
        ensure!(
            metadata.is_file() && metadata.nlink() == 1,
            "artifact must be a private regular file"
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

struct BoundedBuffer {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for BoundedBuffer {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(buffer.len())
            .is_none_or(|size| size > self.limit)
        {
            return Err(io::Error::other("compressed artifact exceeds bound"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn export_file(path: &Path, max_bytes: u64, max_compressed: u64) -> Result<Export> {
    let mut file = OpenOptions::new()
        .read(true)
        // A FIFO must not block before metadata can reject it.
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let before = Stamp::read(file.metadata()?)?;
    ensure!(before.size <= max_bytes, "artifact exceeds source bound");
    let sink = BoundedBuffer {
        bytes: Vec::new(),
        limit: max_compressed
            .try_into()
            .context("compressed bound overflow")?,
    };
    let mut encoder = GzEncoder::new(sink, Compression::fast());
    let mut digest = Sha256::new();
    let mut count = 0u64;
    let mut buffer = [0; 65536];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        count = count
            .checked_add(read as u64)
            .context("artifact size overflow")?;
        ensure!(
            count <= max_bytes && count <= before.size,
            "artifact grew during export"
        );
        digest.update(&buffer[..read]);
        encoder.write_all(&buffer[..read])?;
    }
    ensure!(
        count == before.size
            && Stamp::read(file.metadata()?)? == before
            && Stamp::read(fs::symlink_metadata(path)?)? == before,
        "artifact changed during export"
    );
    let compressed = encoder.finish()?.bytes;
    Ok(Export {
        object: ObjectId {
            sha256: hex::encode(digest.finalize()),
            size: count,
        },
        gzip_hex: hex::encode(compressed),
    })
}

struct Pending {
    path: PathBuf,
    remove: bool,
}
impl Drop for Pending {
    fn drop(&mut self) {
        if self.remove {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub fn import_file(
    export: &Export,
    path: &Path,
    max_bytes: u64,
    max_compressed: u64,
) -> Result<()> {
    export.object.validate()?;
    ensure!(
        export.object.size <= max_bytes,
        "artifact exceeds decoded bound"
    );
    ensure!(
        export.gzip_hex.len() % 2 == 0 && export.gzip_hex.len() as u64 / 2 <= max_compressed,
        "compressed artifact exceeds bound or has odd hex length"
    );
    let compressed = hex::decode(&export.gzip_hex)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    ensure!(
        fs::symlink_metadata(parent)?.is_dir(),
        "artifact parent must be a real directory"
    );
    match fs::symlink_metadata(path) {
        Ok(_) => anyhow::bail!("artifact destination already exists"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut nonce = [0; 16];
    File::open("/dev/urandom")?.read_exact(&mut nonce)?;
    let pending_path = parent.join(format!(".md-sim-artifact-{}", hex::encode(nonce)));
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&pending_path)?;
    // Only own the cleanup path after exclusive creation has succeeded.
    let mut pending = Pending {
        path: pending_path,
        remove: true,
    };
    let mut decoder = GzDecoder::new(compressed.as_slice());
    let mut count = 0u64;
    let mut buffer = [0; 65536];
    loop {
        // Read at most one byte beyond the declared extent. Even a small gzip
        // stream cannot make us expand an unbounded artifact before checking it.
        let wanted = (export.object.size - count)
            .saturating_add(1)
            .min(buffer.len() as u64) as usize;
        let read = decoder.read(&mut buffer[..wanted])?;
        if read == 0 {
            break;
        }
        count = count
            .checked_add(read as u64)
            .context("decoded artifact size overflow")?;
        ensure!(
            count <= export.object.size,
            "decoded artifact exceeds its declared size"
        );
        output.write_all(&buffer[..read])?;
    }
    ensure!(
        count == export.object.size,
        "decoded artifact is shorter than declared"
    );
    ensure!(
        decoder.into_inner().is_empty(),
        "trailing bytes or additional gzip member"
    );
    output.sync_all()?;
    ensure!(
        hash_file(&pending.path)? == export.object,
        "decoded artifact identity differs"
    );
    // A hard link is an atomic no-overwrite publication, as in the input store.
    // On a later sync error, any destination already present contains only the
    // fully verified bytes. Existing destinations are never removed or changed.
    fs::hard_link(&pending.path, path).context("publish verified artifact without overwriting")?;
    fs::remove_file(&pending.path)?;
    pending.remove = false;
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::CString,
        os::unix::{ffi::OsStrExt, fs::symlink},
        process::{Command, Stdio},
        time::{Duration, Instant},
    };

    fn abc() -> Export {
        Export {
            object: ObjectId {
                sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
                size: 3,
            },
            // Independent gzip fixture for "abc"; fixed header and CRC32.
            gzip_hex: "1f8b08000000000000034b4c4a0600c241243503000000".into(),
        }
    }

    fn rejects_without_publication(value: &Export, max: u64, compressed: u64) {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("result");
        assert!(import_file(value, &output, max, compressed).is_err());
        assert!(!output.exists());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn independent_gzip_and_sha256_fixture_imports_exact_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("result");
        let fixture = abc();
        import_file(&fixture, &output, 3, 23).unwrap();
        assert_eq!(fs::read(&output).unwrap(), b"abc");
        assert_eq!(hash_file(&output).unwrap(), fixture.object);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn binary_roundtrip_preserves_known_digest_and_source() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("source");
        let output = directory.path().join("restored");
        let bytes: Vec<u8> = (0..=255).collect();
        fs::write(&input, &bytes).unwrap();
        let value = export_file(&input, 256, 1024).unwrap();
        assert_eq!(value.object.size, 256);
        assert_eq!(
            value.object.sha256,
            "40aff2e9d2d8922e47afd4648e6967497158785fbd1da870e7110266bf944880"
        );
        import_file(&value, &output, 256, 1024).unwrap();
        assert_eq!(fs::read(&input).unwrap(), bytes);
        assert_eq!(fs::read(&output).unwrap(), bytes);
        assert_eq!(hash_file(&output).unwrap(), value.object);
    }

    #[test]
    fn content_hash_and_declared_extents_are_required() {
        let mut wrong = abc();
        wrong.object.sha256 = "0".repeat(64);
        rejects_without_publication(&wrong, 3, 23);
        for size in [0, 2, 4] {
            let mut wrong = abc();
            wrong.object.size = size;
            rejects_without_publication(&wrong, 4, 23);
        }
        rejects_without_publication(&abc(), 2, 23);
        rejects_without_publication(&abc(), 3, 22);
    }

    #[test]
    fn truncated_malformed_and_bad_crc_inputs_do_not_publish() {
        let fixture = abc();
        for malformed in [
            "".to_owned(),
            "z0".to_owned(),
            "f".to_owned(),
            "00000000".to_owned(),
            fixture.gzip_hex[..fixture.gzip_hex.len() - 2].to_owned(),
        ] {
            let mut value = fixture.clone();
            value.gzip_hex = malformed;
            rejects_without_publication(&value, 3, 23);
        }
        // All plaintext bytes can be decoded before this footer error. The
        // decoder must still read through EOF and validate the gzip checksum.
        let mut bytes = hex::decode(&fixture.gzip_hex).unwrap();
        let checksum = bytes.len() - 8;
        bytes[checksum] ^= 1;
        let mut wrong = fixture;
        wrong.gzip_hex = hex::encode(bytes);
        rejects_without_publication(&wrong, 3, 23);
    }

    #[test]
    fn trailing_bytes_and_extra_members_are_rejected() {
        for suffix in ["00".to_owned(), abc().gzip_hex] {
            let mut wrong = abc();
            wrong.gzip_hex.push_str(&suffix);
            rejects_without_publication(&wrong, 3, 128);
        }
    }

    #[test]
    fn a_small_compressed_stream_cannot_exceed_the_declared_extent() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("large");
        fs::write(&source, vec![0; 1024 * 1024]).unwrap();
        let mut encoded = export_file(&source, 1024 * 1024, 8192).unwrap();
        encoded.object.size = 8;
        rejects_without_publication(&encoded, 8, 8192);
    }

    #[test]
    fn export_limits_apply_during_compression_and_empty_files_work() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source");
        fs::write(&source, b"abc").unwrap();
        assert!(export_file(&source, 2, 1024).is_err());
        assert!(export_file(&source, 3, 1).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"abc");
        fs::write(&source, []).unwrap();
        let empty = export_file(&source, 0, 64).unwrap();
        assert_eq!(
            empty.object.sha256,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let output = directory.path().join("empty");
        import_file(&empty, &output, 0, 64).unwrap();
        assert_eq!(fs::metadata(output).unwrap().len(), 0);
    }

    #[test]
    fn existing_files_and_links_are_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("existing");
        fs::write(&output, b"keep these bytes").unwrap();
        assert!(import_file(&abc(), &output, 3, 23).is_err());
        assert_eq!(fs::read(&output).unwrap(), b"keep these bytes");
        let link = directory.path().join("link");
        symlink(&output, &link).unwrap();
        assert!(export_file(&link, 1024, 1024).is_err());
        assert!(import_file(&abc(), &link, 3, 23).is_err());
        assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
        assert_eq!(fs::read(output).unwrap(), b"keep these bytes");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
    }

    #[test]
    fn unknown_wire_fields_are_rejected() {
        let mut value = serde_json::to_value(abc()).unwrap();
        value["trusted"] = serde_json::json!(true);
        assert!(serde_json::from_value::<Export>(value).is_err());
    }

    #[test]
    fn fifo_export_rejects_without_waiting_for_writer() {
        const CHILD: &str = "MD_SIM_CODEC_FIFO_CHILD";
        if let Some(path) = std::env::var_os(CHILD) {
            assert!(export_file(Path::new(&path), 1024, 1024).is_err());
            fs::write(Path::new(&path).with_extension("checked"), b"rejected").unwrap();
            return;
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("unopened-fifo");
        let name = CString::new(path.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        let test = format!(
            "{}::fifo_export_rejects_without_waiting_for_writer",
            module_path!().split_once("::").unwrap().1
        );
        // Run the possible blocking open in an owned child. A regression must
        // fail this bounded test, not leave the entire test suite hanging.
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &test, "--nocapture"])
            .env(CHILD, &path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let result = (|| -> Result<_> {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                if let Some(status) = child.try_wait()? {
                    return Ok(status);
                }
                ensure!(
                    Instant::now() < deadline,
                    "FIFO export blocked waiting for a writer"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
        })();
        if result.is_err() {
            let _ = child.kill();
            let _ = child.wait();
        }
        assert!(result.unwrap().success());
        assert_eq!(
            fs::read(path.with_extension("checked")).unwrap(),
            b"rejected"
        );
    }
}

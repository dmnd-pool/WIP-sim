//! Snapshot only the completed access service and client evidence. No browser
//! profiles, TLS keys, archive objects, or pool scratch directories are included.
use super::{shared_retention as wire, worker_broker};
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::RetentionIdentity;
use md_sim_runtime::{isolation::save_json, store::ObjectId};
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Read,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

struct Snapshot {
    files: BTreeMap<String, wire::Bytes>,
    bytes: usize,
    entries: usize,
}
impl Snapshot {
    fn file(&mut self, path: &Path, name: String) -> Result<()> {
        wire::access_name(&name)?;
        ensure!(self.files.len() < 512, "too many access evidence files");
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?;
        let metadata = file.metadata()?;
        ensure!(
            metadata.is_file() && metadata.nlink() == 1 && metadata.len() <= 8 << 20,
            "access evidence is not a bounded regular file"
        );
        let mut bytes = Vec::new();
        file.take((8 << 20) + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 8 << 20 && self.bytes + bytes.len() <= 15 << 20,
            "access evidence total bound"
        );
        self.bytes += bytes.len();
        let value = wire::Bytes {
            object: ObjectId {
                sha256: md_sim_firmware::prepare::hash_bytes(&bytes),
                size: bytes.len() as u64,
            },
            hex: hex::encode(bytes),
        };
        ensure!(
            self.files.insert(name, value).is_none(),
            "duplicate access evidence path"
        );
        Ok(())
    }
    fn directory(&mut self, root: &Path, prefix: &str, depth: usize) -> Result<()> {
        ensure!(
            depth <= 6 && fs::symlink_metadata(root)?.is_dir(),
            "access directory scope or depth"
        );
        let mut entries = Vec::new();
        for entry in fs::read_dir(root)? {
            self.entries += 1;
            ensure!(self.entries <= 512, "too many access directory entries");
            entries.push(entry?);
        }
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("access filename encoding"))?;
            let selected = format!("{prefix}/{name}");
            wire::access_name(&selected)?;
            if entry.file_type()?.is_dir() {
                self.directory(&entry.path(), &selected, depth + 1)?;
            } else {
                self.file(&entry.path(), selected)?;
            }
        }
        Ok(())
    }
}

pub fn export(native: &RetentionIdentity) -> Result<Option<super::channel_artifacts::Export>> {
    let run = worker_broker::run()?;
    let Some(policy) = run.policies.get("carrier_dashboard") else {
        return Ok(None);
    };
    let mut snapshot = Snapshot {
        files: BTreeMap::new(),
        bytes: 0,
        entries: 0,
    };
    snapshot.directory(Path::new("/work/client-root/carrier-client"), "client", 0)?;
    snapshot.directory(Path::new("/work/server-root/access"), "server", 0)?;
    for name in [
        "carrier-launch.json",
        "carrier-start.json",
        "carrier-outcome.json",
        "carrier.stdout",
        "carrier.stderr",
    ] {
        let path = Path::new("/work/server-root").join(name);
        // On a failed startup, retain every existing record. Completion is
        // checked after durable retention; a missing record cannot become a pass.
        if path.try_exists()? {
            snapshot.file(&path, format!("server-root/{name}"))?;
        }
    }
    snapshot.file(Path::new("/provider.json"), "metadata/provider.json".into())?;
    snapshot.file(
        Path::new("/carrier-dashboard.json"),
        "metadata/policy.json".into(),
    )?;
    snapshot.file(
        Path::new("/work/client-root/carrier-observer.json"),
        "client-observer/report.json".into(),
    )?;
    for name in [
        "board-browser.stdout",
        "board-browser.stderr",
        "board-browser-process.json",
    ] {
        let path = Path::new("/work/client-root").join(name);
        if path.try_exists()? {
            snapshot.file(&path, format!("client-observer/{name}"))?;
        }
    }
    let provider = snapshot
        .files
        .get("metadata/provider.json")
        .context("provider snapshot absent")?
        .object
        .clone();
    let policy: ObjectId = serde_json::from_value(policy.clone())?;
    ensure!(
        snapshot.files["metadata/policy.json"].object == policy,
        "retained policy differs from selected identity"
    );
    let bundle = wire::AccessBundle {
        schema: "md-sim-shared-carrier-artifacts/1".into(),
        native: native.clone(),
        policy,
        provider,
        browser_requested: super::board_browser::enabled(),
        browser_driver: if super::board_browser::enabled() {
            Some(md_sim_runtime::store::hash_file(Path::new(
                "/s9-browser-check",
            ))?)
        } else {
            None
        },
        files: snapshot.files,
    };
    ensure!(
        serde_json::to_vec_pretty(&bundle)?.len() as u64 <= wire::ACCESS_BUNDLE_BYTES,
        "access bundle exceeds bound"
    );
    let path = Path::new("/work/shared-access-bundle.json");
    save_json(path, &bundle)?;
    Ok(Some(super::channel_artifacts::export_file(
        path,
        wire::ACCESS_BUNDLE_BYTES,
        wire::ACCESS_COMPRESSED_BYTES,
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_retains_exact_bytes_and_rejects_links_and_devices() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::write(root.path().join("body"), b"abc")?;
        let mut snapshot = Snapshot {
            files: BTreeMap::new(),
            bytes: 0,
            entries: 0,
        };
        snapshot.file(&root.path().join("body"), "client/body".into())?;
        let saved = &snapshot.files["client/body"];
        assert_eq!(
            saved.object.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(saved.hex, "616263");
        assert_eq!(saved.object.size, 3);
        std::os::unix::fs::symlink(root.path().join("body"), root.path().join("link"))?;
        assert!(snapshot
            .file(&root.path().join("link"), "server/link".into())
            .is_err());
        let pipe = std::ffi::CString::new(root.path().join("pipe").as_os_str().as_encoded_bytes())?;
        assert_eq!(unsafe { libc::mkfifo(pipe.as_ptr(), 0o600) }, 0);
        assert!(snapshot
            .file(&root.path().join("pipe"), "server/pipe".into())
            .is_err());
        assert!(snapshot
            .file(&root.path().join("body"), "server/client.key".into())
            .is_err());
        Ok(())
    }
    #[test]
    fn empty_directory_fanout_counts_toward_the_snapshot_bound() -> Result<()> {
        let root = tempfile::tempdir()?;
        for index in 0..513 {
            fs::create_dir(root.path().join(format!("dir-{index}")))?;
        }
        let mut snapshot = Snapshot {
            files: BTreeMap::new(),
            bytes: 0,
            entries: 0,
        };
        let error = snapshot.directory(root.path(), "client", 0).unwrap_err();
        assert!(error.to_string().contains("directory entries"));
        assert!(snapshot.files.is_empty());
        Ok(())
    }
}

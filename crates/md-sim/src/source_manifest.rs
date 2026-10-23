use anyhow::{ensure, Result};
use md_sim_runtime::store::{hash_file, ObjectId};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceManifest {
    pub schema_version: String,
    pub identity: String,
    pub files: BTreeMap<String, ObjectId>,
}

pub fn collect(root: &Path) -> Result<SourceManifest> {
    let mut files = BTreeMap::new();
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        ".cargo/config.toml",
        "vendor",
        "README.md",
        "AGENTS.md",
        "docs/simulator-specification.md",
        "crates",
        "simulator",
        "profiles",
        "scenarios",
        "fixtures",
        "backends",
    ] {
        let p = root.join(path);
        if p.exists() {
            scan(root, &p, &mut files)?;
        }
    }
    Ok(SourceManifest {
        schema_version: "2.0".into(),
        identity: md_sim_catalog::content_hash(&files)?,
        files,
    })
}

fn scan(root: &Path, path: &Path, files: &mut BTreeMap<String, ObjectId>) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "source manifest must not follow symbolic links"
    );
    if metadata.is_dir() {
        for child in fs::read_dir(path)? {
            scan(root, &child?.path(), files)?;
        }
    } else {
        ensure!(
            metadata.is_file(),
            "source manifest input is not a regular file"
        );
        files.insert(
            path.strip_prefix(root)?.to_string_lossy().into_owned(),
            hash_file(path)?,
        );
    }
    Ok(())
}

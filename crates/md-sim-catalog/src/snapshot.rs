use crate::{content_hash, hash_file, valid_sha256};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader},
    path::{Component, Path, PathBuf},
};

const REQUIRED_FILES: &[&str] = &[
    "catalog/entities.jsonl",
    "catalog/relationships.jsonl",
    "catalog/artifacts.jsonl",
    "catalog/sources.jsonl",
    "catalog/gaps.jsonl",
    "catalog/storage.json",
    "reports/device-matrix.csv",
    "reports/coverage.md",
    "docs/simulator-specification.md",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FrozenFile {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotManifest {
    pub schema_version: String,
    /// Catalogue data identity excludes profile/scenario/backend registries.
    /// Targets may pin this identity without a circular hash of their own file.
    pub catalogue_data_hash: String,
    pub snapshot_hash: String,
    pub files: Vec<FrozenFile>,
    /// Payload bytes are checked at preparation, not by metadata import.
    pub artifact_payloads_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedSnapshot {
    pub directory: PathBuf,
    pub manifest: SnapshotManifest,
}

/// Original records remain intact. This retains revision, uncertainty, and
/// artifact-specific constraints even when newer fields are not understood.
#[derive(Debug, Clone)]
pub struct Catalog {
    pub snapshot: ImportedSnapshot,
    pub entities: BTreeMap<String, Value>,
    pub artifacts: BTreeMap<String, Value>,
    pub relationships: BTreeMap<String, Value>,
    pub sources: BTreeMap<String, Value>,
    pub gaps: BTreeMap<String, Value>,
}

fn collect_files(root: &Path, relative: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
    let directory = root.join(relative);
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(&directory)? {
        let entry = entry?;
        let name = relative.join(entry.file_name());
        let kind = entry.file_type()?;
        ensure!(
            !kind.is_symlink(),
            "snapshot evidence must not be a link: {}",
            name.display()
        );
        if kind.is_dir() {
            collect_files(root, &name, paths)?;
        } else if kind.is_file() {
            paths.push(name);
        }
    }
    Ok(())
}

fn relative_path(value: &str) -> Result<&Path> {
    let path = Path::new(value);
    ensure!(
        !value.is_empty() && path.components().all(|c| matches!(c, Component::Normal(_))),
        "unsafe snapshot path: {value}"
    );
    Ok(path)
}

/// Copy metadata and profile bytes into a content-addressed snapshot directory.
/// Do not read the 150 GiB payload archive as a side effect of a catalogue import.
/// Each selected payload must pass `verify_artifact` before preparation.
pub fn import_catalog(repository: &Path, store: &Path) -> Result<ImportedSnapshot> {
    let mut paths: Vec<PathBuf> = REQUIRED_FILES.iter().map(PathBuf::from).collect();
    for directory in ["catalog/parts", "profiles", "scenarios", "backends"] {
        collect_files(repository, Path::new(directory), &mut paths)?;
    }
    paths.sort();
    paths.dedup();
    let mut files = Vec::new();
    for path in &paths {
        let (sha256, size_bytes) = hash_file(&repository.join(path))?;
        files.push(FrozenFile {
            path: path.to_string_lossy().into_owned(),
            sha256,
            size_bytes,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let data_hash = catalogue_data_hash(&files)?;
    let snapshot_hash = content_hash(&("md-sim-catalog-snapshot-v2", &data_hash, &files))?;
    let manifest = SnapshotManifest {
        schema_version: "2.0".into(),
        catalogue_data_hash: data_hash,
        snapshot_hash: snapshot_hash.clone(),
        files,
        artifact_payloads_verified: false,
    };
    fs::create_dir_all(store)?;
    let directory = store.join(&snapshot_hash);
    if directory.exists() {
        return verify_snapshot(&directory);
    }
    // create_dir provides an exclusive claim. A concurrent importer may retry.
    let staging = store.join(format!(".import-{}-{snapshot_hash}", std::process::id()));
    fs::create_dir(&staging).context("create snapshot staging directory")?;
    let write_result = (|| -> Result<()> {
        for file in &manifest.files {
            let relative = relative_path(&file.path)?;
            let destination = staging.join(relative);
            fs::create_dir_all(destination.parent().context("missing parent")?)?;
            fs::copy(repository.join(relative), &destination)?;
            let actual = hash_file(&destination)?;
            ensure!(
                actual == (file.sha256.clone(), file.size_bytes),
                "input changed during import: {}",
                file.path
            );
            let mut permissions = fs::metadata(&destination)?.permissions();
            permissions.set_readonly(true);
            fs::set_permissions(destination, permissions)?;
        }
        fs::write(
            staging.join("snapshot.json"),
            serde_json::to_vec_pretty(&manifest)?,
        )?;
        // Parse before publishing so duplicate or malformed records cannot create
        // a frozen snapshot which later appears usable.
        load_catalog(&staging)?;
        fs::rename(&staging, &directory)?;
        Ok(())
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    Ok(ImportedSnapshot {
        directory,
        manifest,
    })
}

pub fn verify_snapshot(directory: &Path) -> Result<ImportedSnapshot> {
    let canonical_directory = fs::canonicalize(directory)?;
    let manifest: SnapshotManifest =
        serde_json::from_reader(fs::File::open(directory.join("snapshot.json"))?)?;
    ensure!(
        manifest.schema_version == "2.0",
        "unsupported catalogue snapshot version"
    );
    ensure!(
        !manifest.artifact_payloads_verified,
        "metadata snapshot cannot assert payload verification"
    );
    let expected = content_hash(&(
        "md-sim-catalog-snapshot-v2",
        &manifest.catalogue_data_hash,
        &manifest.files,
    ))?;
    ensure!(
        manifest.snapshot_hash == expected,
        "snapshot manifest hash mismatch"
    );
    ensure!(
        manifest.catalogue_data_hash == catalogue_data_hash(&manifest.files)?,
        "catalogue data identity mismatch"
    );
    let mut prior = None;
    for file in &manifest.files {
        ensure!(valid_sha256(&file.sha256), "invalid snapshot file hash");
        if let Some(previous) = prior {
            ensure!(
                previous < &file.path,
                "snapshot files must be unique and sorted"
            );
        }
        prior = Some(&file.path);
        let path = directory.join(relative_path(&file.path)?);
        ensure!(
            !fs::symlink_metadata(&path)?.file_type().is_symlink(),
            "snapshot file is a link"
        );
        ensure!(
            fs::canonicalize(&path)?.starts_with(&canonical_directory),
            "snapshot parent link escapes frozen directory"
        );
        ensure!(
            hash_file(&path)? == (file.sha256.clone(), file.size_bytes),
            "snapshot file hash mismatch: {}",
            file.path
        );
    }
    for required in REQUIRED_FILES {
        ensure!(
            manifest.files.iter().any(|f| f.path == *required),
            "missing required frozen input: {required}"
        );
    }
    Ok(ImportedSnapshot {
        directory: directory.to_path_buf(),
        manifest,
    })
}

fn catalogue_data_hash(files: &[FrozenFile]) -> Result<String> {
    let catalogue: Vec<_> = files
        .iter()
        .filter(|file| {
            !["profiles/", "scenarios/", "backends/"]
                .iter()
                .any(|prefix| file.path.starts_with(prefix))
        })
        .collect();
    content_hash(&("md-sim-catalog-data-v2", catalogue))
}

fn read_records(path: &Path) -> Result<BTreeMap<String, Value>> {
    let mut records = BTreeMap::new();
    for (line, data) in BufReader::new(fs::File::open(path)?).lines().enumerate() {
        let data = data?;
        if data.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&data)
            .with_context(|| format!("{}:{}", path.display(), line + 1))?;
        let id = value["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .context("record lacks ID")?
            .to_owned();
        ensure!(
            records.insert(id.clone(), value).is_none(),
            "duplicate record ID {id} in {}",
            path.display()
        );
    }
    Ok(records)
}

pub fn load_catalog(directory: &Path) -> Result<Catalog> {
    let snapshot = verify_snapshot(directory)?;
    let records = |name: &str| read_records(&directory.join(format!("catalog/{name}.jsonl")));
    Ok(Catalog {
        snapshot,
        entities: records("entities")?,
        artifacts: records("artifacts")?,
        relationships: records("relationships")?,
        sources: records("sources")?,
        gaps: records("gaps")?,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedArtifact {
    pub artifact_id: String,
    pub path: PathBuf,
    pub sha256: String,
    pub size_bytes: u64,
    pub record_hash: String,
}

impl Catalog {
    /// Resolve workspace links only through the archive roots recorded in the
    /// snapshot. Transfer callers must copy the returned bytes, not the link.
    pub fn verify_artifact(&self, repository: &Path, id: &str) -> Result<VerifiedArtifact> {
        let record = self
            .artifacts
            .get(id)
            .with_context(|| format!("unknown artifact {id}"))?;
        ensure!(
            record["status"] == "downloaded",
            "artifact {id} is not downloaded"
        );
        ensure!(
            record["content_role"] != "git-lfs-pointer",
            "artifact {id} is a pointer; select its resolved payload explicitly"
        );
        if record["content_validity"].as_str().is_some() {
            bail!(
                "artifact {id} has a recorded content defect: {}",
                record["content_validity"]
            );
        }
        ensure!(
            record["integrity_review_required"] != true,
            "artifact {id} requires integrity review"
        );
        let expected = record["sha256"]
            .as_str()
            .filter(|h| valid_sha256(h))
            .context("missing artifact SHA-256")?;
        let relative = relative_path(
            record["local_path"]
                .as_str()
                .context("missing artifact path")?,
        )?;
        ensure!(
            relative.starts_with("artifacts/files"),
            "artifact path is outside archive"
        );
        let path = fs::canonicalize(repository.join(relative))
            .context("resolve archived artifact bytes")?;
        let root = fs::canonicalize(repository)?;
        let storage: Value = serde_json::from_reader(fs::File::open(
            self.snapshot.directory.join("catalog/storage.json"),
        )?)?;
        let mut allowed = path.starts_with(root.join("artifacts/files"));
        if let Some(roots) = storage["artifact_roots"].as_array() {
            for value in roots {
                if let Some(root) = value.as_str() {
                    if let Ok(root) = fs::canonicalize(root) {
                        allowed |= path.starts_with(root);
                    }
                }
            }
        }
        ensure!(allowed, "artifact link escapes recorded archive roots");
        let (sha256, size_bytes) = hash_file(&path)?;
        ensure!(sha256 == expected, "artifact hash mismatch: {id}");
        ensure!(
            record["size_bytes"].as_u64() == Some(size_bytes),
            "artifact size mismatch: {id}"
        );
        Ok(VerifiedArtifact {
            artifact_id: id.into(),
            path,
            sha256,
            size_bytes,
            record_hash: content_hash(record)?,
        })
    }
}

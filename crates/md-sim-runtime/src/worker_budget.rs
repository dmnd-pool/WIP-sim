use crate::{isolation::ProcessSpec, store::StoreLimits};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

/// Host evidence storage and admission limits. These are separate from the
/// per-process guest limits and do not claim aggregate cgroup enforcement.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkerLimits {
    pub store: StoreLimits,
    pub max_active_runs: u64,
    pub max_retained_runs: u64,
    pub max_retained_bytes: u64,
    /// Explicit delegation; this policy is part of executor/cache identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cgroup_parent: Option<PathBuf>,
}

impl Default for WorkerLimits {
    fn default() -> Self {
        Self {
            store: StoreLimits {
                max_object_bytes: 8 * 1024 * 1024 * 1024,
                max_total_bytes: 16 * 1024 * 1024 * 1024,
                max_objects: 4096,
            },
            max_active_runs: 2,
            max_retained_runs: 1024,
            max_retained_bytes: 4 * 1024 * 1024 * 1024,
            cgroup_parent: None,
        }
    }
}

impl WorkerLimits {
    pub(crate) fn validate(&self) -> Result<()> {
        self.store.validate()?;
        if let Some(parent) = &self.cgroup_parent {
            crate::cgroup::validate_parent(parent)?;
        }
        ensure!(
            (1..=64).contains(&self.max_active_runs)
                && (1..=65536).contains(&self.max_retained_runs)
                && self.max_active_runs <= self.max_retained_runs,
            "invalid worker run count limits"
        );
        ensure!(
            self.max_retained_bytes > 0,
            "retained byte limit must be positive"
        );
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct RetainedUsage {
    pub runs: u64,
    /// Includes the full reservation of every active run, even before it writes.
    pub charged_bytes: u64,
}

/// Raw logs use at most trace_bytes. The result repeats them as hexadecimal,
/// requiring another two bytes per byte. The remaining reservation bounds the
/// immutable specification, lock, state and fixed result/error fields, including
/// atomic temporary files. Guest output cannot write host evidence directly.
pub(crate) fn reservation(spec: &ProcessSpec) -> Result<u64> {
    let specification_bytes = serde_json::to_vec_pretty(spec)?.len() as u64 + 1;
    spec.limits
        .trace_bytes
        .checked_mul(3)
        .and_then(|n| n.checked_add(specification_bytes))
        .and_then(|n| n.checked_add(64 * 1024))
        .context("run storage reservation overflow")
}

pub(crate) fn retained_usage(
    root: &Path,
    active_reservations: &BTreeMap<String, u64>,
) -> Result<RetainedUsage> {
    let mut result = RetainedUsage {
        runs: 0,
        charged_bytes: 0,
    };
    for entry in fs::read_dir(root.join("runs"))? {
        let entry = entry?;
        ensure!(
            entry.file_type()?.is_dir(),
            "run storage contains a non-directory"
        );
        result.runs = result.runs.checked_add(1).context("run count overflow")?;
        ensure!(result.runs <= 65536, "run storage exceeds scan limit");
        let name = entry.file_name();
        let name = name.to_str().context("run directory name is not UTF-8")?;
        super::worker::validate_run_id(name)?;
        let mut bytes = 0u64;
        for (count, file) in fs::read_dir(entry.path())?.enumerate() {
            ensure!(count < 32, "run storage exceeds file count limit");
            let file = file?;
            // Do not follow symbolic links or charge an unrelated file through
            // a hard link. A failed scan prevents admission, never deletes data.
            let metadata = fs::symlink_metadata(file.path())?;
            ensure!(
                metadata.is_file() && metadata.nlink() == 1,
                "run storage contains an unsafe file"
            );
            bytes = bytes
                .checked_add(metadata.len())
                .context("run byte count overflow")?;
        }
        let charged = bytes.max(active_reservations.get(name).copied().unwrap_or(0));
        result.charged_bytes = result
            .charged_bytes
            .checked_add(charged)
            .context("retained byte count overflow")?;
    }
    Ok(result)
}

pub(crate) fn admit(
    limits: &WorkerLimits,
    root: &Path,
    reservations: &BTreeMap<String, u64>,
    spec: &ProcessSpec,
) -> Result<u64> {
    ensure!(
        (reservations.len() as u64) < limits.max_active_runs,
        "worker active run limit reached"
    );
    let retained = retained_usage(root, reservations)?;
    ensure!(
        retained.runs < limits.max_retained_runs,
        "worker retained run limit reached (includes token receipts); cleanup releases evidence bytes but preserves idempotency receipts"
    );
    let required = reservation(spec)?;
    let total = retained
        .charged_bytes
        .checked_add(required)
        .context("retained byte count overflow")?;
    ensure!(
        total <= limits.max_retained_bytes,
        "worker retained byte limit exceeded; retrieve and acknowledge cleanup first"
    );
    Ok(required)
}

//! Recover observations in a new evidence root; do not change a run verdict.
use anyhow::{ensure, Result};
use std::path::Path;

#[allow(dead_code)]
#[path = "support/worker_channel_artifacts.rs"]
mod channel_artifacts;
#[path = "support/s9_external_pool.rs"]
mod external_pool;
#[allow(dead_code)]
#[path = "support/s9_pool_input.rs"]
mod pool_input;
#[path = "support/s9_pool_report.rs"]
mod pool_report;
#[path = "support/prepared_vm_image.rs"]
mod prepared_vm_image;
#[path = "support/s9_shared_recovery.rs"]
mod recovery;
#[allow(dead_code)]
#[path = "support/worker_retention_frame.rs"]
mod retention_frame;
#[allow(dead_code)]
#[path = "support/s9_shared_pool.rs"]
mod shared_manifest;
#[allow(dead_code)]
#[path = "support/worker_shared_retention.rs"]
mod shared_retention;

fn main() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    ensure!(args.len() == 4,
        "usage: s9_retain_shared_evidence plan SOURCE_EXECUTION NEW_PLAN | retain SOURCE_EXECUTION NEW_ROOT");
    let source = Path::new(&args[2]);
    let output = Path::new(&args[3]);
    match args[1].to_str() {
        Some("plan") => recovery::save_plan(source, output),
        Some("retain") => recovery::retain(source, output),
        _ => anyhow::bail!("unknown shared evidence recovery action"),
    }
}

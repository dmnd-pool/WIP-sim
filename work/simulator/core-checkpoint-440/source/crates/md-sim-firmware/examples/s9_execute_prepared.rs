//! Execute one frozen board-reference process through the common local/worker contract.
//! This does not resolve its physical target or grant a mining/dashboard gate.
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::{
    executor::PrivateWorker, Executor, LocalExecutor, LockedRun, SshConfig, SshTransport,
    WorkerExecutor,
};
use md_sim_runtime::{
    isolation::save_json,
    store::{hash_file, Store},
};
use serde_json::json;
use std::{
    fs,
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

#[allow(dead_code)] // The common pool bundle contract uses this artifact codec.
#[path = "support/worker_channel_artifacts.rs"]
mod channel_artifacts;
#[path = "support/prepared_board.rs"]
mod prepared;
#[allow(dead_code)]
#[path = "support/worker_retention_frame.rs"]
mod retention_frame;
#[allow(dead_code)]
#[path = "support/s9_shared_pool.rs"]
mod shared_manifest;
#[allow(dead_code)]
#[path = "support/worker_shared_retention.rs"]
mod shared_retention;
use prepared::{bounded, freeze};

// This driver uses the same report and storage code as the boot preparation CLI.
#[allow(dead_code)]
#[path = "support/s9_boot_storage.rs"]
mod boot_storage;
#[path = "support/execution_cancellation.rs"]
mod cancellation;
#[path = "support/s9_execution_report.rs"]
mod execution_report;
#[allow(dead_code)]
#[path = "support/s9_pool_input.rs"]
mod pool_input;
#[path = "support/s9_pool_report.rs"]
mod pool_report;
#[allow(dead_code)]
#[path = "support/pool_vm.rs"]
mod pool_vm;

fn execute(
    executor: &mut dyn Executor,
    store: &Store,
    run: &LockedRun,
    out: &Path,
    cancel: Arc<AtomicBool>,
) -> Result<()> {
    let before = executor.identity()?;
    save_json(&out.join("executor.json"), &before)?;
    let spec = run.process.as_ref().context("prepared process absent")?;
    let result = executor.run(store, run, cancel, out)?;
    // Keep the exact execution envelope even if a report or identity check fails.
    save_json(&out.join("result.json"), &result)?;
    fs::write(out.join("guest.log"), hex::decode(&result.stdout)?)?;
    fs::write(out.join("backend.log"), hex::decode(&result.stderr)?)?;
    ensure!(
        result.tools == run.backend_tools,
        "executed isolation tools differ from the frozen run"
    );
    ensure!(
        md_sim_coordinator::identity(&before)?
            == md_sim_coordinator::identity(&executor.identity()?)?,
        "executor identity changed during execution"
    );
    execution_report::retain(out, spec, &result)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(
        args.len() == 4,
        "usage: s9_execute_prepared PREPARED NEW_OUTPUT local|worker-ipc=PROGRAM|worker-ssh=CONFIG"
    );
    let source = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    ensure!(!out.exists(), "output must be new");
    ensure!(
        args[3] == "local"
            || args[3].starts_with("worker-ipc=")
            || args[3].starts_with("worker-ssh="),
        "unknown executor"
    );
    fs::create_dir_all(out)?;
    let store = Store::open(&out.join("input-store"), 512 * 1024 * 1024)?;
    let run = freeze(source, out, &store)?;
    save_json(&out.join("locked-run.json"), &run)?;
    save_json(
        &out.join("execution-identity.json"),
        &json!({"locked_run_hash":run.hash()?,"prepared_process":hash_file(&source.join("process-spec.json"))?,"scope":"Complete process transport identity; exact physical target is unresolved."}),
    )?;
    let cancellation = cancellation::Cancellation::start(out)?;
    let cancel = cancellation.requested.clone();
    let result = if let Some(program) = args[3].strip_prefix("worker-ipc=") {
        let mut worker = PrivateWorker::spawn(Path::new(program), &out.join("worker-state"))?;
        execute(&mut worker.executor, &store, &run, out, cancel.clone())
    } else if let Some(config) = args[3].strip_prefix("worker-ssh=") {
        let config: SshConfig = serde_json::from_slice(&bounded(Path::new(config), 65536)?)?;
        let mut worker = WorkerExecutor {
            transport: SshTransport::new_streaming(config)?,
        };
        execute(&mut worker, &store, &run, out, cancel.clone())
    } else {
        execute(&mut LocalExecutor, &store, &run, out, cancel.clone())
    };
    let cancellation_result = cancellation.finish();
    save_json(
        &out.join("transport-assessment.json"),
        &json!({"process_completed":result.is_ok(),"error":result.as_ref().err().map(|e|format!("{e:#}")),"cancellation_requested":cancel.load(std::sync::atomic::Ordering::Acquire),"cancellation_record_error":cancellation_result.as_ref().err().map(|e|format!("{e:#}")),"locked_run_hash":run.hash()?,"original_board_gate":"incomplete","mining_gate":"requires independent audit and Core acceptance","dashboard_gate":"requires declared native suite"}),
    )?;
    cancellation_result?;
    result
}

use anyhow::{ensure, Context, Result};
use md_sim_coordinator::{
    executor::PrivateWorker, LocalExecutor, Queue, SshConfig, SshTransport, WorkerExecutor,
};
use md_sim_firmware::{
    execution_suite::{self, SourceSuite},
    lab::SourceRunOptions,
};
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};
fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().collect();
    ensure!(
        a.len() >= 2,
        "usage: cgminer-coordinator plan|run|cancel ..."
    );
    match a[1].as_str() {
        "plan" | "plan-regtest" => {
            ensure!((7..=11).contains(&a.len()),"usage: cgminer-coordinator plan PREPARED TOOLCHAIN_SNAPSHOT SOURCE_ARCHIVE PREPARATION_RUNNER NEW_QUEUE [CAP_HPS] [SEED] [EPOCH] [EXPONENT]");
            let cap = a.get(7).map(|s| s.parse()).transpose()?.unwrap_or(10000.0);
            let q = execution_suite::create_plan(
                Path::new(&a[2]),
                Path::new(&a[3]),
                Path::new(&a[4]),
                Path::new(&a[5]),
                Path::new(&a[6]),
                SourceRunOptions {
                    cap,
                    seed: a.get(8).cloned(),
                    epoch: a.get(9).map(|s| s.parse()).transpose()?,
                    exponent: a
                        .get(10)
                        .map(|s| s.parse())
                        .transpose()?
                        .unwrap_or_else(|| md_sim_firmware::schedule::automatic_exponent(cap)),
                    regtest: a[1] == "plan-regtest",
                },
            )?;
            println!("{}", serde_json::to_string_pretty(&q.plan)?);
        }
        "run" => {
            ensure!(
                a.len() == 4 || a.len() == 5,
                "usage: cgminer-coordinator run QUEUE local | run QUEUE worker WORKER_ROOT | run QUEUE ssh SSH_CONFIG_JSON"
            );
            let mut q = Queue::open(Path::new(&a[2]))?;
            let suite = SourceSuite::current()?;
            let cancel = Arc::new(AtomicBool::new(false));
            let report = match a[3].as_str() {
                "local" => {
                    ensure!(a.len() == 4, "local run has no worker root");
                    q.run(&mut LocalExecutor, &suite, true, cancel)?
                }
                "worker" => {
                    let program = std::env::current_exe()?
                        .parent()
                        .context("missing binary directory")?
                        .join("md-sim-worker-ipc");
                    let mut worker = PrivateWorker::spawn(
                        &program,
                        Path::new(a.get(4).context("missing worker root")?),
                    )?;
                    q.run(&mut worker.executor, &suite, true, cancel)?
                }
                "ssh" => {
                    let path = Path::new(a.get(4).context("missing SSH configuration")?);
                    let bytes = std::fs::read(path)?;
                    ensure!(bytes.len() <= 65536, "SSH configuration exceeds limit");
                    let config: SshConfig = serde_json::from_slice(&bytes)?;
                    let mut worker = WorkerExecutor {
                        transport: SshTransport::new(config)?,
                    };
                    q.run(&mut worker, &suite, true, cancel)?
                }
                _ => anyhow::bail!("executor must be local, worker or ssh"),
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
            ensure!(
                report.declared_suite_complete,
                "declared source fixture suite failed or incomplete; inspect preserved attempts"
            );
        }
        "cancel" => {
            ensure!(
                a.len() == 4,
                "usage: cgminer-coordinator cancel QUEUE RUN_HASH"
            );
            println!(
                "{}",
                md_sim_coordinator::request_cancel(Path::new(&a[2]), &a[3])?
            );
        }
        _ => anyhow::bail!("unknown command"),
    }
    Ok(())
}

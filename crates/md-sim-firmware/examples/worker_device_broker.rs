//! Owner-only worker bridge and a real-worker regression inside a private VM.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::store::hash_file;
use std::{
    fs::{self, File, OpenOptions},
    os::{fd::FromRawFd, unix::fs::OpenOptionsExt},
    path::Path,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};
#[path = "support/worker_device_broker.rs"]
mod broker;
#[path = "support/worker_device_transport.rs"]
mod device_transport;
#[path = "support/worker_broker_regression.rs"]
mod regression;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|s| s == "--prepare-test") {
        ensure!(args.len() == 4, "--prepare-test NEW_OUTPUT WORKER_BINARY");
        return regression::prepare(Path::new(&args[2]), Path::new(&args[3]));
    }
    if args.get(1).is_some_and(|s| s == "__job") {
        let challenge = hash_file(Path::new("/challenge.bin"))?;
        for sequence in 0..150 {
            println!(
                "MD_SIM_BROKER_JOB {}",
                serde_json::json!({"sequence":sequence,"challenge":challenge})
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        return Ok(());
    }
    ensure!(
        fs::read_to_string("/proc/cmdline")?.contains("md_sim_private_guest=1"),
        "broker requires the declared disconnected test VM"
    );
    if args.get(1).is_some_and(|s| s.starts_with("__test")) {
        return regression::guest(args.get(1).context("test mode absent")?);
    }
    ensure!(
        args.len() == 5 && args[1] == "--serve",
        "usage: worker_device_broker --serve SOCKET INHERITED_CHECKED_DEVICE_FD AUDIT_FILE"
    );
    let descriptor: i32 = args[3].parse()?;
    ensure!(
        descriptor > 2,
        "broker device must not replace standard I/O"
    );
    // The trusted VM orchestrator checks the virtual-port name and device
    // identity, then transfers this descriptor exactly once to this process.
    let device = unsafe { File::from_raw_fd(descriptor) };
    let transport = device_transport::DeviceTransport::new(device)?;
    let log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&args[4])?;
    let report = broker::serve(
        Path::new(&args[2]),
        transport,
        Arc::new(AtomicBool::new(false)),
        log,
    )?;
    println!("MD_SIM_WORKER_BROKER {report}");
    Ok(())
}

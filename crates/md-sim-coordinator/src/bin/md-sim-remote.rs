use anyhow::{ensure, Result};
use md_sim_coordinator::{SshConfig, SshTransport, SshTunnel, WorkerTransport};
use md_sim_runtime::{
    isolation::save_json,
    worker::{Request, MAX_CONTROL_MESSAGE},
};
use std::{
    fs::File,
    io::{Read, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn stop(_: libc::c_int) {
    STOP.store(true, Ordering::Release);
}
fn bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "remote input exceeds limit");
    Ok(bytes)
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    ensure!(args.len()>=3,"usage: md-sim-remote probe CONFIG | request CONFIG REQUEST_JSON | tunnel CONFIG LISTEN REMOTE_LOOPBACK NEW_OUTPUT_DIRECTORY");
    let config: SshConfig = serde_json::from_slice(&bounded(Path::new(&args[2]), 65536)?)?;
    match args[1].as_str() {
        "probe" | "request" => {
            let request = if args[1] == "probe" {
                ensure!(args.len() == 3, "probe CONFIG");
                Request::Probe
            } else {
                ensure!(args.len() == 4, "request CONFIG REQUEST_JSON");
                serde_json::from_slice(&bounded(Path::new(&args[3]), MAX_CONTROL_MESSAGE)?)?
            };
            let mut transport = SshTransport::new(config)?;
            let result = transport.request(&request)?;
            println!(
                "{}",
                serde_json::json!({"transport":transport.evidence(),"result":result})
            );
        }
        "tunnel" => {
            ensure!(
                args.len() == 6,
                "tunnel CONFIG LISTEN REMOTE_LOOPBACK NEW_OUTPUT_DIRECTORY"
            );
            let directory = Path::new(&args[5]);
            ensure!(!directory.exists(), "tunnel output must be new");
            std::fs::create_dir(directory)?;
            unsafe {
                libc::signal(libc::SIGINT, stop as *const () as libc::sighandler_t);
                libc::signal(libc::SIGTERM, stop as *const () as libc::sighandler_t);
            }
            let mut tunnel = SshTunnel::start(
                config,
                args[3].parse()?,
                args[4].parse()?,
                &directory.join("ssh.log"),
            )?;
            save_json(&directory.join("tunnel.json"), &tunnel.evidence())?;
            println!("{}", tunnel.evidence());
            std::io::stdout().flush()?;
            while !STOP.load(Ordering::Acquire) {
                ensure!(
                    tunnel.is_running()?,
                    "SSH tunnel exited; inspect its private log"
                );
                std::thread::sleep(Duration::from_millis(100));
            }
            tunnel.stop()?;
            save_json(
                &directory.join("stopped.json"),
                &serde_json::json!({"owned_tunnel_stopped":true}),
            )?;
        }
        _ => anyhow::bail!("unknown remote operation"),
    }
    Ok(())
}

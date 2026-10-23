use anyhow::{ensure, Result};
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 2 || (args.len() == 3 && args[2] == "--stream"),
        "usage: md-sim-worker-relay PRIVATE_WORKER_SOCKET [--stream]"
    );
    if args.len() == 3 {
        return md_sim_runtime::worker::relay_stream(
            Path::new(&args[1]),
            std::io::stdin().lock(),
            std::io::stdout().lock(),
        );
    }
    md_sim_runtime::worker::relay(
        Path::new(&args[1]),
        std::io::stdin().lock(),
        std::io::stdout().lock(),
    )
}

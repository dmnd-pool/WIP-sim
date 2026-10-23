//! Close a real SSH relay after one read-only reply to test transport recovery.
use anyhow::{ensure, Result};
use md_sim_runtime::worker::{self, Request};
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::Path,
};

pub fn run(socket: &Path) -> Result<()> {
    ensure!(
        matches!(
            socket.to_str(),
            Some("/worker.sock" | "/worker/worker.sock")
        ) && fs::read_to_string("/proc/cmdline")?.contains("md_sim_private_guest=1"),
        "one-reply relay requires the isolated test VM and its worker socket"
    );
    let mut frame = Vec::new();
    BufReader::new(std::io::stdin().take(worker::MAX_CONTROL_MESSAGE + 1))
        .read_until(b'\n', &mut frame)?;
    ensure!(
        frame.last() == Some(&b'\n') && frame.len() as u64 <= worker::MAX_CONTROL_MESSAGE,
        "one-reply test relay frame bound"
    );
    let request: Request = serde_json::from_slice(&frame)?;
    ensure!(
        matches!(
            request,
            Request::Probe | Request::Status { .. } | Request::Logs { .. }
        ),
        "one-reply test relay permits only probe, status and logs"
    );
    worker::relay(socket, &frame[..], std::io::stdout())
}

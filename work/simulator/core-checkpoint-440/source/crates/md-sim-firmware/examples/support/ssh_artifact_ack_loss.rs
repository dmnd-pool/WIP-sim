//! Test-only relay: forward real artifact requests, then withhold one reply.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    store::MAX_CHUNK,
    worker::{self, Request},
};
use serde_json::json;
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
};

pub const MARKER: &str = "MD_SIM_WITHHELD_ARTIFACT_REPLY ";
pub const SIZE: u64 = MAX_CHUNK as u64 + 8192;

pub fn run(socket: &Path, operation: &str) -> Result<()> {
    ensure!(
        matches!(
            socket.to_str(),
            Some("/worker.sock" | "/worker/worker.sock")
        ) && matches!(operation, "chunk" | "finish")
            && fs::read_to_string("/proc/cmdline")?.contains("md_sim_private_guest=1"),
        "artifact reply-loss relay requires the isolated test VM"
    );
    let mut input = BufReader::new(std::io::stdin());
    let mut selected = None;
    for _ in 0..8 {
        let mut frame = Vec::new();
        (&mut input)
            .take(worker::MAX_CONTROL_MESSAGE + 1)
            .read_until(b'\n', &mut frame)?;
        ensure!(
            frame.last() == Some(&b'\n') && frame.len() as u64 <= worker::MAX_CONTROL_MESSAGE,
            "artifact reply-loss frame bound"
        );
        let request: Request = serde_json::from_slice(&frame)?;
        let (object, name, offset) = match &request {
            Request::Begin { object } => (object, "begin", None),
            Request::Chunk { object, offset, .. } => (object, "chunk", Some(*offset)),
            Request::Finish { object } => (object, "finish", None),
            _ => anyhow::bail!("artifact reply-loss relay permits only artifact writes"),
        };
        object.validate()?;
        ensure!(
            object.size == SIZE && selected.as_ref().is_none_or(|old| old == object),
            "artifact reply-loss object differs"
        );
        selected = Some(object.clone());
        let mut response = Vec::new();
        worker::relay(socket, &frame[..], &mut response)?;
        ensure!(response.len() <= 4096, "artifact reply bound");
        let value: serde_json::Value = serde_json::from_slice(&response)?;
        ensure!(
            value["ok"] == true,
            "worker rejected artifact request: {}",
            value["error"]
        );
        if name == operation {
            // Stderr is evidence, never a successful control response. The
            // client must observe an interrupted channel and query state.
            eprintln!(
                "{MARKER}{}",
                json!({"operation":name,"object":object,
                "offset":offset,"worker_response":value,"response_withheld":true})
            );
            std::io::stderr().flush()?;
            return Ok(());
        }
        std::io::stdout().write_all(&response)?;
        std::io::stdout().flush()?;
    }
    anyhow::bail!("artifact reply-loss target operation was not reached")
}

pub fn observation(error: &str) -> Result<serde_json::Value> {
    let records: Vec<_> = error
        .lines()
        .filter_map(|line| line.find(MARKER).map(|at| &line[at + MARKER.len()..]))
        .collect();
    ensure!(
        records.len() == 1,
        "withheld artifact response evidence absent or repeated"
    );
    serde_json::from_str(records[0]).context("withheld artifact response encoding")
}

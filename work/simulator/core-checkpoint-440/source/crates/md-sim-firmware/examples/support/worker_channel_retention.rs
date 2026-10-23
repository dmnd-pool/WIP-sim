//! Identity-bound receipt over a separate pipe/virtio port. The host owns the
//! retained files; the guest receives bytes and no host filesystem descriptor.
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::RetentionIdentity;
use md_sim_runtime::{
    isolation::{self, ProcessResult, ProcessSpec, TraceFiles},
    store::{hash_file, Store},
};
use serde_json::{json, Value};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

use super::retention_frame as frame;

pub const REQUEST: &str = "MD_SIM_WORKER_RETENTION_REQUEST ";
pub const PROTOCOL: &str = "md-sim-worker-retention/1";
const ACK_BOUND: usize = 4096;

/// Each FIFO belongs to the current isolated process. O_RDWR|O_NONBLOCK avoids
/// an unbounded open while QEMU starts or after it exits. No outside path exists.
pub fn start_relay() -> Result<()> {
    for suffix in ["in", "out"] {
        let path = std::ffi::CString::new(format!("/work/retention.{suffix}"))?;
        ensure!(
            unsafe { libc::mkfifo(path.as_ptr(), 0o600) } == 0,
            "create private receipt FIFO: {}",
            std::io::Error::last_os_error()
        );
    }
    let mut input = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open("/work/retention.in")?;
    // This thread has only the runtime's bounded pipe and a private FIFO. Main
    // never joins it: normal exit or the parent death guard ends the relay.
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = (|| -> Result<()> {
            std::io::stdin()
                .take((ACK_BOUND + 1) as u64)
                .read_to_end(&mut bytes)?;
            ensure!(
                !bytes.is_empty() && bytes.len() <= ACK_BOUND,
                "receipt relay frame bound"
            );
            ensure!(
                bytes.last() == Some(&b'\n')
                    && bytes[..bytes.len() - 1].iter().all(|b| *b != b'\n'),
                "receipt relay requires exactly one frame"
            );
            // The maximum frame is PIPE_BUF, so nonblocking write is atomic.
            let count = input.write(&bytes)?;
            ensure!(count == bytes.len(), "receipt relay write was partial");
            Ok(())
        })();
        if let Err(error) = result {
            eprintln!("receipt relay failed: {error:#}");
        }
    });
    Ok(())
}

pub fn read_ack(device: &mut File, expected: &RetentionIdentity) -> Result<RetentionIdentity> {
    let actual: RetentionIdentity = serde_json::from_slice(&frame::read(device, ACK_BOUND, 180)?)?;
    ensure!(actual == *expected, "outer receipt identity differs");
    Ok(actual)
}

pub(super) fn receive(record: &Value, out: &Path) -> Result<Vec<u8>> {
    let identity: RetentionIdentity = serde_json::from_value(record["identity"].clone())?;
    super::execution::restore(&record["artifacts"], out)?.context("retention result absent")?;
    let request: Value = serde_json::from_slice(&super::prepared::bounded(
        &out.join("worker-request.json"),
        65536,
    )?)?;
    let run: Value = serde_json::from_slice(&super::prepared::bounded(
        &out.join("worker-run.json"),
        65536,
    )?)?;
    ensure!(
        identity.result == hash_file(&out.join("worker-result.json"))?
            && request["token"] == identity.token
            && request["locked_run_hash"] == identity.locked_run_hash
            && run["run_id"] == identity.run_id
            && record["artifacts"]["locked_run_hash"] == identity.locked_run_hash,
        "retention request differs from exact imported execution evidence"
    );
    isolation::save_json(&out.join("host-retention.json"), &identity)?;
    File::open(out)?.sync_all()?;
    let mut ack = serde_json::to_vec(&identity)?;
    ack.push(b'\n');
    ensure!(
        ack.len() <= ACK_BOUND,
        "retention acknowledgment exceeds bound"
    );
    Ok(ack)
}

/// Poll only the run-owned trace written by the isolation supervisor. Import
/// exact bytes and fsync the receipt before sending its acknowledgment.
pub fn execute(
    store: &Store,
    spec: &ProcessSpec,
    out: &Path,
    execution_out: &Path,
    bad_ack: bool,
) -> Result<ProcessResult> {
    execute_with_receiver(
        store,
        spec,
        out,
        execution_out,
        REQUEST,
        spec.limits.trace_bytes as usize,
        |bytes| {
            let record: Value = serde_json::from_slice(bytes)?;
            let mut ack = receive(&record, execution_out)?;
            if bad_ack {
                let mut wrong: RetentionIdentity = serde_json::from_slice(&ack)?;
                wrong.locked_run_hash = "0".repeat(64);
                isolation::save_json(&execution_out.join("injected-wrong-receipt.json"), &wrong)?;
                ack = serde_json::to_vec(&wrong)?;
                ack.push(b'\n');
            }
            Ok(ack)
        },
    )
}

/// The receiver verifies and durably saves one declared result envelope.
/// Only its bounded acknowledgment enters the runtime's private input pipe.
pub fn execute_with_receiver(
    store: &Store,
    spec: &ProcessSpec,
    out: &Path,
    execution_out: &Path,
    marker: &str,
    maximum: usize,
    mut receive: impl FnMut(&[u8]) -> Result<Vec<u8>>,
) -> Result<ProcessResult> {
    ensure!(
        !marker.is_empty()
            && marker.len() <= 128
            && maximum > 0
            && maximum as u64 <= spec.limits.trace_bytes,
        "retention receiver bounds differ"
    );
    let cancel = Arc::new(AtomicBool::new(false));
    let (send, input) = mpsc::channel();
    let trace = TraceFiles {
        stdout: File::create(out.join("guest.log"))?,
        stderr: File::create(out.join("backend.log"))?,
    };
    let mut reader = File::open(out.join("guest.log"))?;
    std::thread::scope(|scope| {
        let run_cancel = cancel.clone();
        let run = scope.spawn(move || {
            isolation::execute_with_streamed_input(store, spec, run_cancel, Some(trace), input)
        });
        let observed = (|| -> Result<()> {
            let mut pending = Vec::new();
            let mut total = 0u64;
            let mut send = Some(send);
            let mut received = false;
            loop {
                let finished = run.is_finished();
                let mut buffer = [0; 65536];
                loop {
                    let count = reader.read(&mut buffer)?;
                    if count == 0 {
                        break;
                    }
                    total += count as u64;
                    ensure!(total <= spec.limits.trace_bytes, "receipt trace bound");
                    for byte in &buffer[..count] {
                        if *byte == b'\n' {
                            if let Some(record) = pending.strip_prefix(marker.as_bytes()) {
                                ensure!(!received, "duplicate retention request");
                                ensure!(record.len() <= maximum, "retention request exceeds bound");
                                let ack = receive(record)?;
                                ensure!(
                                    !ack.is_empty() && ack.len() <= ACK_BOUND,
                                    "retention acknowledgment exceeds bound"
                                );
                                send.take().context("retention sender absent")?.send(ack)?;
                                received = true;
                            }
                            pending.clear();
                        } else {
                            pending.push(*byte);
                            ensure!(
                                pending.len() <= maximum + marker.len(),
                                "retention trace line exceeds request bound"
                            );
                        }
                    }
                }
                if finished {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            ensure!(received, "complete retention request absent");
            Ok(())
        })();
        if let Err(error) = observed {
            cancel.store(true, Ordering::Release);
            isolation::save_json(
                &execution_out.join("host-retention-error.json"),
                &json!({"error":format!("{error:#}")}),
            )?;
        }
        run.join()
            .map_err(|_| anyhow::anyhow!("retention process thread panicked"))?
    })
}

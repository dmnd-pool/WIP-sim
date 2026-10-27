//! Live output is provisional until it matches the completely verified result.
use crate::executor::WorkerTransport;
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::ProcessResult,
    store::MAX_CHUNK,
    worker::{LogStream, Request},
};
use serde_json::Value;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
};

const STREAMS: [&str; 2] = ["stdout", "stderr"];

pub(crate) struct RemoteTrace {
    files: [File; 2],
    offsets: [u64; 2],
    observed: [u64; 2],
    bound: u64,
}

impl RemoteTrace {
    pub(crate) fn create(out: &Path, bound: u64) -> Result<Self> {
        Ok(Self {
            files: [
                File::create_new(out.join("worker-stdout.log"))?,
                File::create_new(out.join("worker-stderr.log"))?,
            ],
            offsets: [0; 2],
            observed: [0; 2],
            bound,
        })
    }

    // At most one bounded request per nonempty stream and status poll. A large
    // backlog must not prevent status checks or cancellation.
    pub(crate) fn poll(
        &mut self,
        transport: &mut impl WorkerTransport,
        id: &str,
        status: &Value,
    ) -> Result<()> {
        ensure!(status["run_id"] == id, "log status run differs");
        let lengths = [
            status["saved_logs"]["stdout"]
                .as_u64()
                .context("stdout length absent")?,
            status["saved_logs"]["stderr"]
                .as_u64()
                .context("stderr length absent")?,
        ];
        ensure!(
            lengths[0] <= self.bound && lengths[1] <= self.bound - lengths[0],
            "remote trace exceeds locked bound"
        );
        for (index, name) in STREAMS.iter().enumerate() {
            let available = lengths[index];
            let offset = self.offsets[index];
            ensure!(
                available >= self.observed[index] && available >= offset,
                "remote log shrank"
            );
            self.observed[index] = available;
            if available == offset {
                continue;
            }
            let count = (available - offset).min(MAX_CHUNK as u64) as usize;
            let response = transport.request(&Request::Logs {
                run_id: id.into(),
                stream: if index == 0 {
                    LogStream::Stdout
                } else {
                    LogStream::Stderr
                },
                offset,
                count,
            })?;
            let encoded = response["hex"].as_str().context("log bytes absent")?;
            ensure!(encoded.len() == count * 2, "log chunk length differs");
            let bytes = hex::decode(encoded)?;
            let next = offset + count as u64;
            let saved = response["saved_bytes"]
                .as_u64()
                .context("log saved length absent")?;
            ensure!(
                response["run_id"] == id
                    && response["stream"] == *name
                    && response["offset"].as_u64() == Some(offset)
                    && response["next_offset"].as_u64() == Some(next)
                    && saved >= available
                    && saved <= self.bound,
                "log response range or identity differs"
            );
            ensure!(
                self.files[index].metadata()?.len() == offset,
                "local live log changed"
            );
            self.files[index].write_all(&bytes)?;
            self.offsets[index] = next;
        }
        Ok(())
    }
}

/// Called before acknowledging remote cleanup, including after reconnect.
/// Preserve an inconsistent prefix as failure evidence. Never overwrite it.
pub(crate) fn finish(out: &Path, result: &ProcessResult, bound: u64) -> Result<()> {
    ensure!(
        bound <= md_sim_runtime::isolation::MAX_TRACE_BYTES,
        "invalid trace bound"
    );
    ensure!(
        result.stdout.len() as u64 <= bound * 2
            && result.stderr.len() as u64 <= bound * 2 - result.stdout.len() as u64,
        "result trace exceeds locked bound"
    );
    for (name, encoded) in STREAMS.iter().zip([&result.stdout, &result.stderr]) {
        let bytes = hex::decode(encoded)?;
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(out.join(format!("worker-{name}.log")))?;
        let metadata = file.metadata()?;
        ensure!(
            metadata.is_file() && metadata.nlink() == 1 && metadata.len() <= bytes.len() as u64,
            "unsafe or excessive live log"
        );
        let mut prefix = Vec::new();
        (&mut file)
            .take(bytes.len() as u64 + 1)
            .read_to_end(&mut prefix)?;
        ensure!(
            prefix.len() as u64 == metadata.len() && bytes.starts_with(&prefix),
            "live log differs from verified process result"
        );
        file.write_all(&bytes[prefix.len()..])?;
        file.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, fs};

    fn result(stdout: &[u8], stderr: &[u8]) -> ProcessResult {
        ProcessResult {
            support_virtualization: None,
            private_channel: None,
            stdin_stream: None,
            dashboard: None,
            status: "exited".into(),
            exit_code: Some(0),
            wall_milliseconds: 1,
            stdout: hex::encode(stdout),
            stderr: hex::encode(stderr),
            trace_truncated: false,
            isolation: "test record only".into(),
            limits_scope: "test".into(),
            tools: BTreeMap::new(),
            aggregate_limits: None,
        }
    }

    #[test]
    fn final_output_matches_retained_prefix_and_rejects_changed_or_linked_files() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let stdout = root.join("worker-stdout.log");
        let stderr = root.join("worker-stderr.log");
        fs::write(&stdout, b"first").unwrap();
        let record = result(b"first\0last\n", b"error\n");
        finish(root, &record, 64).unwrap();
        assert_eq!(fs::read(&stdout).unwrap(), b"first\0last\n");
        assert_eq!(fs::read(&stderr).unwrap(), b"error\n");
        finish(root, &record, 64).unwrap();
        assert!(finish(root, &record, 15).is_err());
        assert!(finish(root, &record, u64::MAX).is_err());
        fs::write(&stdout, b"other").unwrap();
        assert!(finish(root, &record, 64).is_err());
        assert_eq!(fs::read(&stdout).unwrap(), b"other");
        fs::remove_file(&stdout).unwrap();
        let outside = root.join("preserved.txt");
        fs::write(&outside, b"first").unwrap();
        std::os::unix::fs::symlink(&outside, &stdout).unwrap();
        assert!(finish(root, &record, 64).is_err());
        fs::remove_file(&stdout).unwrap();
        fs::hard_link(&outside, &stdout).unwrap();
        assert!(finish(root, &record, 64).is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"first");
    }

    struct Response {
        value: Value,
        requests: usize,
    }
    impl WorkerTransport for Response {
        fn description(&self) -> String {
            "protocol validation fixture, no execution".into()
        }
        fn request(&mut self, _: &Request) -> Result<Value> {
            self.requests += 1;
            Ok(self.value.clone())
        }
    }

    #[test]
    fn live_protocol_rejects_wrong_run_stream_range_and_budget_without_empty_polls() {
        let status = serde_json::json!({"run_id":"selected", "saved_logs":{"stdout":4,"stderr":0}});
        let response = serde_json::json!({"run_id":"selected","stream":"stdout","offset":0,"next_offset":4,"saved_bytes":4,"hex":"00010203"});
        for (field, wrong) in [
            ("run_id", serde_json::json!("other")),
            ("stream", serde_json::json!("stderr")),
            ("offset", serde_json::json!(1)),
            ("next_offset", serde_json::json!(5)),
            ("saved_bytes", serde_json::json!(3)),
            ("saved_bytes", serde_json::json!(17)),
            ("hex", serde_json::json!("000102")),
            ("hex", serde_json::json!("not-hex!")),
        ] {
            let directory = tempfile::tempdir().unwrap();
            let mut trace = RemoteTrace::create(directory.path(), 16).unwrap();
            let mut value = response.clone();
            value[field] = wrong;
            let mut transport = Response { value, requests: 0 };
            assert!(
                trace.poll(&mut transport, "selected", &status).is_err(),
                "{field}"
            );
            assert!(fs::read(directory.path().join("worker-stdout.log"))
                .unwrap()
                .is_empty());
        }
        let directory = tempfile::tempdir().unwrap();
        let mut trace = RemoteTrace::create(directory.path(), 16).unwrap();
        let mut transport = Response {
            value: response,
            requests: 0,
        };
        trace.poll(&mut transport, "selected", &status).unwrap();
        trace.poll(&mut transport, "selected", &status).unwrap();
        assert_eq!(
            transport.requests, 1,
            "unchanged lengths must not trigger empty log requests"
        );
        let mut shrunk = status.clone();
        shrunk["saved_logs"]["stdout"] = serde_json::json!(3);
        assert!(trace.poll(&mut transport, "selected", &shrunk).is_err());
        let mut excess = status;
        excess["saved_logs"]["stderr"] = serde_json::json!(13);
        assert!(trace.poll(&mut transport, "selected", &excess).is_err());
    }
}

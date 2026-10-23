//! Bounded observations only. These records never establish an acceptance gate.
use anyhow::{ensure, Result};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::Path,
    process::Child,
    time::{Duration, Instant},
};

const READ_BYTES: u64 = 64 << 10;
const MARKER: &str = "MD_SIM_SHARED_POOL_PROGRESS ";

fn open(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "diagnostic input is not regular"
    );
    Ok(file)
}

fn document(path: &Path) -> Result<Value> {
    let file = open(path)?;
    let mut bytes = Vec::new();
    file.take(READ_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= READ_BYTES,
        "diagnostic JSON exceeds bound"
    );
    Ok(serde_json::from_slice(&bytes)?)
}

fn audit(path: &Path) -> Result<Value> {
    let mut file = open(path)?;
    let size = file.metadata()?.len();
    let start = size.saturating_sub(READ_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(READ_BYTES).read_to_end(&mut bytes)?;
    // Ignore a partial first or final record during concurrent append.
    let lines = bytes.split_inclusive(|byte| *byte == b'\n');
    let latest = lines
        .enumerate()
        .filter(|(index, line)| (start == 0 || *index != 0) && line.ends_with(b"\n"))
        .filter_map(|(_, line)| serde_json::from_slice::<Value>(line).ok())
        .last();
    Ok(json!({"bytes":size,"last_complete_event":latest}))
}

fn observed(result: Result<Value>) -> Value {
    result.unwrap_or_else(|error| json!({"observation_error":format!("{error:#}")}))
}

fn access_report(path: &Path) -> Value {
    // Finished reports may contain large HTTP bodies. Read only bounded JSON
    // and never include those bodies, cookies, or publication inputs in progress.
    observed(document(path).map(|report| {
        json!({"passed":report["passed"].as_bool(),
            "error":report["error"].as_str().map(|message|message.chars().take(512).collect::<String>())})
    }))
}

fn files(root: &Path) -> Value {
    let mut records = serde_json::Map::new();
    for name in [
        "client-root/attempt/artifact-transfer.json",
        "client-root/attempt/worker-run.json",
        "client-root/attempt/worker-stdout.log",
        "client-root/attempt/worker-stderr.log",
        "client-root/attempt/worker-result.json",
        "client-root/native-retention-request.json",
        "client-root/shared-retention.json",
        "client-root/attempt/worker-cleanup.json",
        "server-root/pool.stdout",
        "server-root/pool.stderr",
        "server-root/pool-outcome.json",
        "server-root/carrier-start.json",
        "server-root/carrier-outcome.json",
        "server-root/carrier.stderr",
        "server-root/access/ready.json",
        "server-root/access/service-outcome.json",
        "client-root/carrier-client/report.json",
        "client-root/carrier-observer.json",
    ] {
        let value = match fs::symlink_metadata(root.join(name)) {
            Ok(metadata) => {
                json!({"regular":metadata.is_file(),"bytes":metadata.len(),"uid":metadata.uid()})
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({"present":false}),
            Err(error) => json!({"observation_error":error.to_string()}),
        };
        records.insert(name.into(), value);
    }
    let transfer = observed(document(
        &root.join("client-root/attempt/artifact-transfer.json"),
    ));
    let run = observed(document(&root.join("client-root/attempt/worker-run.json")));
    json!({"files":records,
        "destination_verified_objects":transfer["verified_at_destination"].as_object().map(|objects|objects.len()),
        "native_run_id":run["run_id"].as_str().filter(|id|id.len()<=256),
        "access_client":access_report(&root.join("client-root/carrier-client/report.json")),
        "broker":observed(audit(&root.join("server-root/broker/broker.jsonl")))})
}

fn child(process: &mut Child) -> Value {
    let state = match process.try_wait() {
        Ok(None) => json!({"state":"running"}),
        Ok(Some(status)) => json!({"state":"exited","status":status.to_string()}),
        Err(error) => json!({"observation_error":error.to_string()}),
    };
    json!({"pid":process.id(),"observation":state})
}

pub struct Progress {
    started: Instant,
    next: Instant,
}

impl Progress {
    pub fn new() -> Self {
        let started = Instant::now();
        Self {
            started,
            next: started,
        }
    }

    pub fn emit(&mut self, stage: &str, server: &mut Child, client: Option<&mut Child>) {
        if Instant::now() < self.next {
            return;
        }
        self.next = Instant::now() + Duration::from_secs(15);
        let record = json!({"schema_version":"md-sim-shared-pool-progress/1",
            "diagnostic_only":true,"stage":stage,"elapsed_ms":self.started.elapsed().as_millis(),
            "server":child(server),"client":client.map(child),"observed":files(Path::new("/work"))});
        if let Ok(encoded) = serde_json::to_vec(&record) {
            if encoded.len() <= 8192 {
                let mut out = std::io::stdout().lock();
                let _ = out.write_all(MARKER.as_bytes());
                let _ = out.write_all(&encoded);
                let _ = out.write_all(b"\n");
                let _ = out.flush();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_reads_actual_complete_events_and_existing_result_bytes() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::create_dir_all(root.path().join("server-root/broker"))?;
        fs::create_dir_all(root.path().join("client-root/attempt"))?;
        let log = root.path().join("server-root/broker/broker.jsonl");
        fs::write(
            &log,
            b"{\"connection\":1,\"operation\":\"chunk\"}\n{\"connection\":2",
        )?;
        fs::write(
            root.path().join("client-root/attempt/worker-result.json"),
            b"partial",
        )?;
        let observed = files(root.path());
        assert_eq!(observed["broker"]["last_complete_event"]["connection"], 1);
        assert_eq!(
            observed["files"]["client-root/attempt/worker-result.json"]["bytes"],
            7
        );
        assert!(observed["native_run_id"].is_null());
        assert!(observed["destination_verified_objects"].is_null());
        assert_eq!(
            observed["files"]["client-root/shared-retention.json"]["present"],
            false
        );
        Ok(())
    }

    #[test]
    fn progress_nonregular_and_oversized_inputs_do_not_block_or_become_events() -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("input");
        let fifo = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())?;
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(audit(&path).is_err());
        assert!(access_report(&path)["passed"].is_null());
        fs::remove_file(&path)?;
        fs::write(&path, vec![b' '; READ_BYTES as usize + 1])?;
        assert!(document(&path).is_err());
        assert!(access_report(&path)["passed"].is_null());
        assert!(audit(&path)?["last_complete_event"].is_null());
        Ok(())
    }

    #[test]
    fn progress_access_report_is_bounded_and_omits_response_data() -> Result<()> {
        let root = tempfile::tempdir()?;
        let path = root.path().join("report.json");
        assert!(access_report(&path)["passed"].is_null());
        fs::write(
            &path,
            serde_json::to_vec(&json!({"passed":false,"error":"é".repeat(600),
                "observations":[{"body_hex":"never emit response bodies"}],
                "result":{"credentials":"never emit input data"}}))?,
        )?;
        let report = access_report(&path);
        assert_eq!(report["passed"], false);
        assert_eq!(report["error"].as_str().unwrap().chars().count(), 512);
        assert_eq!(report.as_object().unwrap().len(), 2);
        fs::write(&path, b"{\"passed\":true,\"error\":null}")?;
        assert_eq!(access_report(&path), json!({"passed":true,"error":null}));
        Ok(())
    }
}

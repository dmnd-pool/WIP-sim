//! Frozen archived board process through the local or actual SSH executor.
//! The gateway fixture is separate. It is not this board's dashboard.
use super::*;
use flate2::{bufread::GzDecoder, write::GzEncoder, Compression};
use md_sim_runtime::isolation::{save_json, ProcessResult, ProcessSpec};
use std::{
    io::{Seek, SeekFrom},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
};

const ROOT: &str = "/etc/md-sim-board";
const RESULT_LIMIT: u64 = 256 << 20;
const COMPRESSED_LIMIT: usize = 48 << 20;
fn live_sample(path: &Path) -> Result<Value> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.nlink() == 1 && metadata.len() <= 128 << 20,
        "live sample file bound"
    );
    let start = metadata.len().saturating_sub(2048);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(metadata.len() - start).read_to_end(&mut bytes)?;
    Ok(json!({"offset":start,"saved_bytes":metadata.len(),"hex":hex::encode(bytes)}))
}
pub(super) fn local_mode() -> bool {
    std::env::var("MD_SIM_BOARD_LOCAL").as_deref() == Ok("1")
}
pub(super) fn extended_mode() -> bool {
    std::env::var("MD_SIM_BOARD_EXTENDED").as_deref() == Ok("1")
}
pub(super) fn long_mode() -> bool {
    std::env::var("MD_SIM_BOARD_LONG").as_deref() == Ok("1")
}
pub(super) fn slow_mode() -> bool {
    std::env::var("MD_SIM_BOARD_SLOW").as_deref() == Ok("1")
}
pub(super) fn long_boot_mode() -> bool {
    std::env::var("MD_SIM_BOARD_LONG_BOOT").as_deref() == Ok("1")
}
pub(super) fn server_seconds() -> u64 {
    if long_boot_mode() {
        5280
    } else if slow_mode() {
        2880
    } else if long_mode() {
        2280
    } else if extended_mode() {
        1680
    } else {
        840
    }
}
const FILES: &[&str] = &[
    "adaptation.json",
    "configuration-reference.bit",
    "artifact-transfer.json",
    "console.json",
    "execution-identity.json",
    "executor.json",
    "locked-run.json",
    "pool-input.json",
    "process-spec.json",
    "selected-dashboard.json",
    "transport-assessment.json",
    "worker-cleanup.json",
    "worker-request.json",
    "worker-run.json",
];

fn selected_run(bytes: &[u8]) -> Option<String> {
    let record: Value = serde_json::from_slice(bytes).ok()?;
    let id = record["run_id"].as_str()?;
    (id.len() == 68 && id.starts_with("run-") && id[4..].bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| id.to_owned())
}

pub(super) fn prepare(
    source: &Path,
    build: &Path,
    template: &Path,
    out: &Path,
    local: bool,
    browser_payload: Option<&Path>,
) -> Result<()> {
    ensure!(!out.exists(), "board deployment output must be new");
    fs::create_dir_all(out)?;
    let out = out.canonicalize()?;
    let mut files = BTreeMap::new();
    let mut symlinks = json!({});
    if let Some(path) = browser_payload {
        let payload: Value = serde_json::from_slice(&read_evidence(path, 4 << 20)?)?;
        ensure!(
            payload["schema"] == "md-sim-vm-payload/1",
            "browser payload schema"
        );
        for (name, value) in payload["files"]
            .as_object()
            .context("browser files absent")?
        {
            ensure!(
                name.starts_with("/usr/") || name.starts_with("/etc/"),
                "browser payload outside support tools"
            );
            ensure!(
                !name.starts_with("/etc/md-sim-board/"),
                "browser payload contains board inputs"
            );
            files.insert(name.clone(), value.clone());
        }
        symlinks = payload["symlinks"].clone();
    }
    for name in [
        "process-spec.json",
        "adaptation.json",
        "console.json",
        "pool-input.json",
    ] {
        let path = source.join(name).canonicalize()?;
        let bytes = read_evidence(&path, 4 << 20)?;
        let _: Value = serde_json::from_slice(&bytes)?;
        files.insert(
            format!("{ROOT}/{name}"),
            json!({"source":path,"object":hash_file(&path)?,"mode":0o444}),
        );
    }
    let spec: ProcessSpec =
        serde_json::from_slice(&read_evidence(&source.join("process-spec.json"), 4 << 20)?)?;
    spec.validate()?;
    for object in spec
        .inputs
        .iter()
        .map(|input| &input.object)
        .chain(spec.stdin_object.iter())
    {
        let path = source.join("input-store/objects").join(&object.sha256);
        ensure!(
            fs::symlink_metadata(&path)?.is_file() && hash_file(&path)? == *object,
            "prepared input identity differs"
        );
        files.insert(
            format!("{ROOT}/input-store/objects/{}", object.sha256),
            json!({"source":path.canonicalize()?,"object":object,"mode":0o444}),
        );
    }
    save_json(
        &out.join("payload.json"),
        &json!({"schema":"md-sim-vm-payload/1","files":files,"symlinks":symlinks}),
    )?;
    let mut scenario: Value = serde_json::from_slice(&read_evidence(template, 65536)?)?;
    ensure!(
        scenario["environment"] == json!({}) && scenario.get("payload_manifest").is_none(),
        "use the plain namespace scenario template"
    );
    for (guest, binary) in [
        ("/namespace-check", "examples/remote_namespace_check"),
        ("/check", "examples/remote_transport_check"),
        ("/md-sim", "md-sim"),
        ("/md-sim-worker-relay", "md-sim-worker-relay"),
        ("/md-sim-access", "md-sim-access"),
        ("/s9-execute-prepared", "examples/s9_execute_prepared"),
    ] {
        scenario["programs"][guest] = json!(build.join(binary).canonicalize()?);
    }
    scenario["environment"] = json!({"MD_SIM_BOARD_PROCESS":"1","MD_SIM_PROCESS_VM":"1"});
    if browser_payload.is_some() {
        ensure!(
            spec.dashboard.is_some(),
            "browser needs a selected native dashboard"
        );
        scenario["programs"]["/s9-browser-check"] =
            json!(build.join("examples/s9_browser_check").canonicalize()?);
        scenario["environment"]["MD_SIM_BOARD_BROWSER"] = json!("1");
    }
    if local {
        scenario["environment"]["MD_SIM_BOARD_LOCAL"] = json!("1");
    }
    let profile = spec
        .environment
        .get(confinement::MINING_PROFILE_ENVIRONMENT)
        .map(String::as_str);
    if let Some(wall) = confinement::mining_profile_wall(profile)? {
        ensure!(
            spec.limits.wall_seconds == wall
                && Some(spec.limits.cpu_seconds) == confinement::mining_profile_cpu(profile)?,
            "nested mining resource contract differs"
        );
        scenario["environment"]["MD_SIM_BOARD_EXTENDED"] = json!("1");
        if wall >= confinement::NESTED_MINING_LONG_WALL_SECONDS {
            scenario["environment"]["MD_SIM_BOARD_LONG"] = json!("1");
        }
        if wall >= confinement::NESTED_MINING_SLOW_WALL_SECONDS {
            scenario["environment"]["MD_SIM_BOARD_SLOW"] = json!("1");
        }
        if wall == confinement::NESTED_MINING_LONG_BOOT_SECONDS {
            scenario["environment"]["MD_SIM_BOARD_LONG_BOOT"] = json!("1");
        }
    }
    scenario["payload_manifest"] = json!(out.join("payload.json"));
    save_json(&out.join("scenario.json"), &scenario)?;
    save_json(
        &out.join("deployment.json"),
        &json!({"schema":"md-sim-board-process-deployment/2","board_executor":if local {"local"} else {"ssh"},"prepared_process":hash_file(&source.join("process-spec.json"))?,"payload":hash_file(&out.join("payload.json"))?,"scenario":hash_file(&out.join("scenario.json"))?,"scope":"Selected local or SSH executor inside one disconnected local VM. The inner process, seed, clock and budgets are unchanged. SSH/TLS fixture tests are separate from this board's execution and dashboard."}),
    )?;
    Ok(())
}

pub(super) fn client(events: &mut Vec<Value>) -> Result<()> {
    let local = local_mode();
    super::record_progress(if local {
        "board-local-execution"
    } else {
        "board-artifact-transfer-and-execution"
    })?;
    ensure!(
        Path::new(ROOT).join("process-spec.json").is_file(),
        "client prepared input absent"
    );
    save_json(Path::new("/work/board-ssh.json"), &super::ssh_config())?;
    let log = fs::File::create("/work/board-driver.log")?;
    let out = Path::new(if local {
        "/work/board-local/result"
    } else {
        "/work/board-result"
    });
    let mut command = Command::new("/s9-execute-prepared");
    if local {
        let parent = out.parent().context("local result parent")?;
        fs::create_dir(parent)?;
        std::os::unix::fs::chown(parent, Some(1000), Some(1000))?;
        command.uid(1000).gid(1000);
    }
    let mut child = Owned(
        command
            .args([
                ROOT,
                out.to_str().context("output path")?,
                if local {
                    "local"
                } else {
                    "worker-ssh=/work/board-ssh.json"
                },
            ])
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?,
    );
    let began = Instant::now();
    let process: ProcessSpec = serde_json::from_slice(&read_evidence(
        &Path::new(ROOT).join("process-spec.json"),
        4 << 20,
    )?)?;
    let mut dashboard = super::board_dashboard::Probe::new(process, local)?;
    let mut execution_reported = false;
    let mut selected_id = None;
    let mut progress_at = Instant::now();
    let mut sample_at = Instant::now();
    let outcome = loop {
        let selected = out.join("worker-run.json");
        if !local && !execution_reported {
            // This diagnostic can race the coordinator's create/write. Wait
            // for a complete record. Final evidence validation stays strict.
            if let Some(id) = read_evidence(&selected, 65536)
                .ok()
                .and_then(|bytes| selected_run(&bytes))
            {
                super::record_progress(&format!("board-execution:{id}"))?;
                selected_id = Some(id);
                execution_reported = true;
            }
        }
        if local && selected_id.is_none() {
            if let Ok(bytes) = read_evidence(&out.join("execution-identity.json"), 65536) {
                if let Ok(value) = serde_json::from_slice::<Value>(&bytes) {
                    if let Some(hash) = value["locked_run_hash"].as_str().filter(|hash| {
                        hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                    }) {
                        selected_id = Some(format!("local-{hash}"));
                    }
                }
            }
        }
        if progress_at.elapsed() >= Duration::from_secs(5) {
            if let Some(id) = &selected_id {
                let prefix = if local { "local" } else { "worker" };
                if let (Ok(stdout), Ok(stderr)) = (
                    fs::metadata(out.join(format!("{prefix}-stdout.log"))),
                    fs::metadata(out.join(format!("{prefix}-stderr.log"))),
                ) {
                    // Provisional byte counts are diagnostics, not a boot or
                    // mining gate. Final evidence still requires exact bytes.
                    super::record_progress(&format!(
                        "board-live:{id}:o={}:e={}",
                        stdout.len(),
                        stderr.len()
                    ))?;
                }
            }
            progress_at = Instant::now();
        }
        if sample_at.elapsed() >= Duration::from_secs(30) {
            if let Some(id) = &selected_id {
                let prefix = if local { "local" } else { "worker" };
                if let (Ok(stdout), Ok(stderr)) = (
                    live_sample(&out.join(format!("{prefix}-stdout.log"))),
                    live_sample(&out.join(format!("{prefix}-stderr.log"))),
                ) {
                    save_json(
                        Path::new("/work/board-live.tmp"),
                        &json!({"selected_run":id,"diagnostic_only":true,"provisional":true,
                            "dashboard":dashboard.diagnostic(),"stdout":stdout,"stderr":stderr}),
                    )?;
                    fs::rename("/work/board-live.tmp", "/work/board-live.json")?;
                }
            }
            sample_at = Instant::now();
        }
        if let Some(status) = child.0.try_wait()? {
            break Ok(status);
        }
        dashboard.poll(out, selected_id.as_deref());
        if began.elapsed()
            >= Duration::from_secs(if long_boot_mode() {
                5160
            } else if slow_mode() {
                2760
            } else if long_mode() {
                2160
            } else if extended_mode() {
                1560
            } else {
                720
            })
        {
            break Err(anyhow::anyhow!("board process coordinator deadline"));
        }
        thread::sleep(Duration::from_millis(100));
    };
    if outcome.is_err() {
        // Give the coordinator's cancellation watcher time to stop the actual
        // selected worker and retrieve its result. This owned child is not yet
        // reaped, so its PID cannot have been reused by another process.
        unsafe {
            libc::kill(child.0.id() as libc::pid_t, libc::SIGTERM);
        }
        let grace = Instant::now() + Duration::from_secs(35);
        while child.0.try_wait()?.is_none() && Instant::now() < grace {
            thread::sleep(Duration::from_millis(100));
        }
        if child.0.try_wait()?.is_none() {
            let _ = child.0.kill();
            let _ = child.0.wait();
        }
    }
    let dashboard = dashboard.finish(out)?;
    let mut metadata = BTreeMap::new();
    for name in FILES {
        let path = out.join(name);
        if path.exists() {
            let bytes = read_evidence(&path, 4 << 20)?;
            metadata.insert(
                *name,
                json!({"object":hash_file(&path)?,"hex":hex::encode(bytes)}),
            );
        }
    }
    let result_path = out.join(if local {
        "result.json"
    } else {
        "worker-result.json"
    });
    let result = if result_path.exists() {
        let raw = read_evidence(&result_path, RESULT_LIMIT)?;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&raw)?;
        let compressed = encoder.finish()?;
        ensure!(
            compressed.len() <= COMPRESSED_LIMIT,
            "compressed board evidence exceeds bound"
        );
        json!({"object":hash_file(&result_path)?,"encoding":"gzip","compressed_sha256":hex::encode(Sha256::digest(&compressed)),"hex":hex::encode(compressed)})
    } else {
        Value::Null
    };
    let driver = read_evidence(Path::new("/work/board-driver.log"), 256 << 10)?;
    let success = outcome.as_ref().is_ok_and(|status| status.success());
    events.push(json!({"board_process":{"schema":"md-sim-board-process-execution/2","executor":if local {"local"} else {"ssh"},"coordinator_exited_successfully":success,"elapsed_ms":began.elapsed().as_millis(),"error":outcome.as_ref().err().map(|e|format!("{e:#}")),"metadata":metadata,"process_result":result,"driver_log_hex":hex::encode(driver),"native_dashboard_access_gate":false,"original_board_gate":false,"external_physical_host":false}}));
    super::record_progress("board-evidence-retained")?;
    ensure!(
        success,
        "board process coordinator failed; exact output retained"
    );
    ensure!(
        dashboard.is_none_or(|report| report["passed"] == true),
        "selected native HTTP failed; exact output retained"
    );
    Ok(())
}

fn decode(entry: &Value, cap: u64, gzip: bool) -> Result<Vec<u8>> {
    let object: ObjectId = serde_json::from_value(entry["object"].clone())?;
    object.validate()?;
    ensure!(object.size <= cap, "board evidence object exceeds bound");
    let encoded = entry["hex"]
        .as_str()
        .context("missing board evidence bytes")?;
    ensure!(
        encoded.len() as u64 <= 2 * if gzip { COMPRESSED_LIMIT as u64 } else { cap },
        "encoded evidence exceeds bound"
    );
    let compressed = hex::decode(encoded)?;
    let bytes = if gzip {
        ensure!(
            entry["encoding"] == "gzip"
                && entry["compressed_sha256"] == hex::encode(Sha256::digest(&compressed)),
            "compressed board evidence identity differs"
        );
        let mut decoder = GzDecoder::new(compressed.as_slice());
        let mut bytes = Vec::new();
        decoder.by_ref().take(cap + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 <= cap,
            "expanded board evidence exceeds bound"
        );
        ensure!(
            decoder.into_inner().is_empty(),
            "trailing compressed board evidence"
        );
        bytes
    } else {
        compressed
    };
    ensure!(
        bytes.len() as u64 == object.size && hex::encode(Sha256::digest(&bytes)) == object.sha256,
        "board evidence identity differs"
    );
    Ok(bytes)
}

fn record_local(record: &Value) -> Result<bool> {
    match (record["schema"].as_str(), record["executor"].as_str()) {
        (Some("md-sim-board-ssh-execution/1"), None) => Ok(false),
        (Some("md-sim-board-process-execution/2"), Some("ssh")) => Ok(false),
        (Some("md-sim-board-process-execution/2"), Some("local")) => Ok(true),
        _ => anyhow::bail!("unsupported board execution schema or executor"),
    }
}
fn result_entry(record: &Value) -> &Value {
    if record["schema"] == "md-sim-board-ssh-execution/1" {
        &record["worker_result"]
    } else {
        &record["process_result"]
    }
}
fn result_filename(local: bool) -> &'static str {
    if local {
        "local-result.json"
    } else {
        "worker-result.json"
    }
}

fn record(client_events: &[Value]) -> Result<&Value> {
    let records: Vec<_> = client_events
        .iter()
        .filter_map(|event| event.get("board_process"))
        .collect();
    ensure!(
        records.len() == 1,
        "missing or duplicate board execution report"
    );
    record_local(records[0])?;
    Ok(records[0])
}

fn export(directory: &Path, record: &Value) -> Result<std::path::PathBuf> {
    let local = record_local(record)?;
    let out = directory.join("board-result");
    ensure!(!out.exists(), "board result output already exists");
    fs::create_dir(&out)?;
    for name in FILES {
        if !record["metadata"][name].is_null() {
            fs::write(
                out.join(name),
                decode(&record["metadata"][name], 4 << 20, false)?,
            )?;
        }
    }
    let diagnostic = record["driver_log_hex"]
        .as_str()
        .context("board diagnostic absent")?;
    ensure!(
        diagnostic.len() <= 512 << 10,
        "board diagnostic exceeds bound"
    );
    fs::write(out.join("driver.log"), hex::decode(diagnostic)?)?;
    if !result_entry(record).is_null() {
        let raw = decode(result_entry(record), RESULT_LIMIT, true)?;
        fs::write(out.join(result_filename(local)), &raw)?;
        let result: ProcessResult = serde_json::from_slice(&raw)?;
        // Keep the transport object's exact bytes. Re-serializing a valid
        // result can change key order and invalidate its exported identity.
        fs::write(out.join("result.json"), &raw)?;
        fs::write(out.join("guest.log"), hex::decode(&result.stdout)?)?;
        fs::write(out.join("backend.log"), hex::decode(&result.stderr)?)?;
    }
    save_json(
        &out.join("export.json"),
        &json!({"scope":"Bounded exact evidence extraction only; no execution or target gate granted.","executor":if local {"local"} else {"ssh"},"coordinator_exited_successfully":record["coordinator_exited_successfully"],"process_result":result_entry(record)["object"],"outer_result":hash_file(&directory.join("result.json"))?}),
    )?;
    Ok(out)
}

pub(super) fn extract_failure(directory: &Path) -> Result<()> {
    let process: ProcessResult =
        serde_json::from_slice(&read_evidence(&directory.join("result.json"), 257 << 20)?)?;
    let raw = read_evidence(&directory.join("guest.log"), 128 << 20)?;
    ensure!(
        hex::decode(&process.stdout)? == raw,
        "outer guest log differs from captured output"
    );
    let text = std::str::from_utf8(&raw)?;
    let frames: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("MD_SIM_NAMESPACE_EVIDENCE "))
        .collect();
    ensure!(frames.len() == 1, "missing or duplicate namespace evidence");
    let report: Value = serde_json::from_str(frames[0])?;
    ensure!(
        report["role"] == "orchestrator" && report["passed"] == false,
        "failure extraction requires a failed orchestrator result"
    );
    let clients: Vec<_> = report["events"]
        .as_array()
        .context("missing namespace events")?
        .iter()
        .filter_map(|event| event.get("role_report"))
        .filter(|role| role["role"] == "client")
        .collect();
    ensure!(clients.len() == 1, "missing or duplicate client report");
    export(
        directory,
        record(
            clients[0]["events"]
                .as_array()
                .context("client events absent")?,
        )?,
    )?;
    Ok(())
}

pub(super) fn restore(directory: &Path, client_events: &[Value], spec: &Value) -> Result<ObjectId> {
    let record = record(client_events)?;
    let local = record_local(record)?;
    ensure!(
        local == (spec["board_local"] == true),
        "board executor differs from captured scenario"
    );
    ensure!(
        record["coordinator_exited_successfully"] == true,
        "board process execution failed"
    );
    let out = export(directory, record)?;
    let result: ProcessResult =
        serde_json::from_slice(&fs::read(out.join(result_filename(local)))?)?;
    let run: md_sim_coordinator::LockedRun =
        serde_json::from_slice(&fs::read(out.join("locked-run.json"))?)?;
    run.validate()?;
    let process = run.process.as_ref().context("board process absent")?;
    ensure!(
        serde_json::from_slice::<Value>(&fs::read(out.join("process-spec.json"))?)?
            == serde_json::to_value(process)?,
        "board process differs from locked run"
    );
    let selected: Value = serde_json::from_slice(&fs::read(out.join("execution-identity.json"))?)?;
    ensure!(
        selected["locked_run_hash"] == run.hash()?
            && selected["prepared_process"] == spec["inputs"][format!("{ROOT}/process-spec.json")]
            && selected["prepared_process"]
                == serde_json::to_value(hash_file(&out.join("process-spec.json"))?)?,
        "selected board input identity differs"
    );
    let reporter: ObjectId =
        serde_json::from_value(spec["inputs"]["/s9-execute-prepared"].clone())?;
    ensure!(
        run.suite.suite == reporter
            && run.suite.verifier == reporter
            && run.inputs.get("execution-and-report-build") == Some(&reporter),
        "board coordinator build differs from captured input"
    );
    for object in process
        .inputs
        .iter()
        .map(|input| &input.object)
        .chain(process.stdin_object.iter())
    {
        ensure!(
            spec["inputs"][format!("{ROOT}/input-store/objects/{}", object.sha256)]
                == serde_json::to_value(object)?,
            "board input not in captured VM payload"
        );
    }
    ensure!(
        result.tools == run.backend_tools,
        "worker isolation tool identity differs"
    );
    for (name, path) in [
        ("bubblewrap", "/usr/bin/bwrap"),
        ("unshare", "/usr/bin/unshare"),
        ("prlimit", "/usr/bin/prlimit"),
    ] {
        ensure!(
            serde_json::to_value(run.backend_tools.get(name).context("board tool absent")?)?
                == spec["inputs"][path],
            "board tool differs from VM input"
        );
    }
    if local {
        let executor: Value = serde_json::from_slice(&fs::read(out.join("executor.json"))?)?;
        ensure!(
            executor["kind"] == "local-isolated-process"
                && executor["build"] == serde_json::to_value(&reporter)?
                && executor["tools"] == serde_json::to_value(&run.backend_tools)?,
            "local executor identity differs"
        );
        ensure!(
            !out.join("worker-run.json").exists() && !out.join("worker-cleanup.json").exists(),
            "local execution has conflicting worker evidence"
        );
    } else {
        let worker: Value = serde_json::from_slice(&fs::read(out.join("worker-run.json"))?)?;
        ensure!(
            worker["identity"]["build"] == spec["inputs"]["/md-sim"],
            "board worker build differs from captured input"
        );
        let clean: Value = serde_json::from_slice(&fs::read(out.join("worker-cleanup.json"))?)?;
        ensure!(
            clean["removed"] == true
                && clean["acknowledged_result"] == result_entry(record)["object"]
                && clean["run_id"] == worker["run_id"],
            "board worker cleanup acknowledgement differs"
        );
    }
    let assessment: Value =
        serde_json::from_slice(&fs::read(out.join("transport-assessment.json"))?)?;
    ensure!(
        assessment["process_completed"] == true
            && assessment["cancellation_requested"] == false
            && assessment["error"].is_null(),
        "board process assessment failed"
    );
    fs::write(out.join("guest.log"), hex::decode(&result.stdout)?)?;
    fs::write(out.join("backend.log"), hex::decode(&result.stderr)?)?;
    super::execution_report::retain(&out, process, &result)?;
    let browser_driver: Option<ObjectId> = if spec["board_browser"] == true {
        Some(serde_json::from_value(
            spec["inputs"]["/s9-browser-check"].clone(),
        )?)
    } else {
        None
    };
    super::board_dashboard::verify(&out, process, &result, local, browser_driver.as_ref())?;
    hash_file(&out.join("locked-run.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_preserves_worker_bytes_including_json_layout() {
        let raw = br#"{ "tools":{}, "limits_scope":"test", "isolation":"test", "trace_truncated":false, "stderr":"", "stdout":"68690a", "wall_milliseconds":3, "exit_code":0, "status":"exited" }"#;
        let mut compressor = GzEncoder::new(Vec::new(), Compression::fast());
        compressor.write_all(raw).unwrap();
        let compressed = compressor.finish().unwrap();
        let entry = json!({"object":{"size":raw.len(),"sha256":hex::encode(Sha256::digest(raw))},"encoding":"gzip","compressed_sha256":hex::encode(Sha256::digest(&compressed)),"hex":hex::encode(compressed)});
        for executor in ["local", "ssh"] {
            let directory = tempfile::tempdir().unwrap();
            fs::write(directory.path().join("result.json"), b"{}").unwrap();
            let record = json!({"schema":"md-sim-board-process-execution/2","executor":executor,"driver_log_hex":"","process_result":entry,"coordinator_exited_successfully":true});
            let out = export(directory.path(), &record).unwrap();
            assert_eq!(fs::read(out.join("result.json")).unwrap(), raw);
            assert_eq!(fs::read(out.join("guest.log")).unwrap(), b"hi\n");
            let exported: Value =
                serde_json::from_slice(&fs::read(out.join("export.json")).unwrap()).unwrap();
            assert_eq!(
                exported["process_result"],
                serde_json::to_value(hash_file(&out.join("result.json")).unwrap()).unwrap()
            );
        }
    }
    #[test]
    fn executor_scope_cannot_be_relabelled_or_inferred_from_a_result_name() {
        assert!(!record_local(&json!({"schema":"md-sim-board-ssh-execution/1"})).unwrap());
        assert!(
            record_local(&json!({"schema":"md-sim-board-ssh-execution/1","executor":"local"}))
                .is_err()
        );
        assert!(record_local(&json!({"schema":"md-sim-board-process-execution/2"})).is_err());
        assert!(record_local(
            &json!({"schema":"md-sim-board-process-execution/2","executor":"local"})
        )
        .unwrap());
        assert!(!record_local(
            &json!({"schema":"md-sim-board-process-execution/2","executor":"ssh"})
        )
        .unwrap());
        assert!(record_local(
            &json!({"schema":"md-sim-board-process-execution/2","executor":"worker-ipc"})
        )
        .is_err());
    }
    #[test]
    fn progress_waits_for_a_complete_selected_run_record() {
        assert!(selected_run(b"").is_none());
        assert!(selected_run(b"{\"run_id\":").is_none());
        assert!(selected_run(br#"{"run_id":"run-../../wrong"}"#).is_none());
        let id = format!("run-{}", "a1".repeat(32));
        assert_eq!(
            selected_run(&serde_json::to_vec(&json!({"run_id":id})).unwrap()),
            Some(id)
        );
    }
    #[test]
    fn retained_results_require_exact_uncompressed_bytes_and_bounded_expansion() {
        let raw = b"independent result bytes";
        let mut compressor = GzEncoder::new(Vec::new(), Compression::fast());
        compressor.write_all(raw).unwrap();
        let bytes = compressor.finish().unwrap();
        let mut entry = json!({"object":{"size":raw.len(),"sha256":hex::encode(Sha256::digest(raw))},"encoding":"gzip","compressed_sha256":hex::encode(Sha256::digest(&bytes)),"hex":hex::encode(bytes)});
        assert_eq!(decode(&entry, raw.len() as u64, true).unwrap(), raw);
        assert!(decode(&entry, raw.len() as u64 - 1, true).is_err());
        let mut expanded = entry.clone();
        expanded["object"]["size"] = json!(1);
        assert!(decode(&expanded, 2, true).is_err());
        let mut trailing = hex::decode(entry["hex"].as_str().unwrap()).unwrap();
        trailing.extend(b"unexpected trailing bytes");
        let mut appended = entry.clone();
        appended["hex"] = json!(hex::encode(&trailing));
        appended["compressed_sha256"] = json!(hex::encode(Sha256::digest(&trailing)));
        assert!(decode(&appended, 100, true).is_err());
        entry["object"]["sha256"] = json!("00".repeat(32));
        assert!(decode(&entry, 100, true).is_err());
    }
}

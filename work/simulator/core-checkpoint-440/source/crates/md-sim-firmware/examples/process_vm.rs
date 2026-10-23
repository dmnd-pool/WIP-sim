//! Run an unchanged captured process contract in a disconnected Linux VM.
use anyhow::{ensure, Context, Result};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use md_sim_runtime::{
    isolation::{execute_with_trace, save_json, ProcessResult, ProcessSpec, TraceFiles},
    store::{hash_file, ObjectId, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::CString,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
const SPEC: &str = "/usr/md-sim-process.json";
const OBJECTS: &str = "/usr/md-sim-inputs";
const MAX_INPUT: u64 = 512 * 1024 * 1024;
const MAX_EXPORT: usize = 16 * 1024 * 1024;
const MARKER: &str = "MD_SIM_PROCESS_VM_CHUNK ";

fn guest() -> Result<()> {
    ensure!(
        fs::read_to_string("/proc/cmdline")?
            .split_whitespace()
            .any(|s| s == "md_sim_private_guest=1"),
        "captured Linux VM required"
    );
    ensure!(unsafe { libc::geteuid() } == 0, "guest setup requires root");
    let work = CString::new("/work")?;
    ensure!(
        unsafe { libc::chown(work.as_ptr(), 1000, 1000) } == 0,
        "guest work ownership failed"
    );
    ensure!(
        unsafe { libc::setgroups(0, std::ptr::null()) } == 0
            && unsafe { libc::setgid(1000) } == 0
            && unsafe { libc::setuid(1000) } == 0
            && unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } == 0,
        "guest privilege drop failed"
    );
    let status = fs::read_to_string("/proc/self/status")?;
    ensure!(
        status.lines().any(|s| s == "CapEff:\t0000000000000000"),
        "guest retained capabilities"
    );
    let spec: ProcessSpec = serde_json::from_reader(File::open(SPEC)?)?;
    spec.validate()?;
    let store = Store::open(Path::new("/work/input-store"), MAX_INPUT)?;
    for id in spec
        .inputs
        .iter()
        .map(|i| &i.object)
        .chain(spec.stdin_object.iter())
    {
        store.import(&Path::new(OBJECTS).join(&id.sha256), id)?;
    }
    println!(
        "MD_SIM_PROCESS_VM_START {}",
        json!({
            "specification":hash_file(Path::new(SPEC))?,
            "uid":unsafe{libc::geteuid()},"driver":hash_file(&std::env::current_exe()?)?,
            "external_physical_host":false,"inner_contract_unchanged":true
        })
    );
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let heartbeat = std::thread::spawn(move || {
        let mut ticks = 0;
        while !signal.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(250));
            ticks += 1;
            if ticks % 80 == 0 {
                println!(
                    "MD_SIM_PROCESS_VM_PROGRESS {}",
                    json!({
                        "elapsed_seconds":ticks/4,
                        "stdout_bytes":fs::metadata("/work/stdout.log").ok().map(|m|m.len()),
                        "stderr_bytes":fs::metadata("/work/stderr.log").ok().map(|m|m.len())
                    })
                );
            }
        }
    });
    let result = execute_with_trace(
        &store,
        &spec,
        Arc::new(AtomicBool::new(false)),
        Some(TraceFiles {
            stdout: File::create("/work/stdout.log")?,
            stderr: File::create("/work/stderr.log")?,
        }),
    );
    stop.store(true, Ordering::Release);
    heartbeat
        .join()
        .map_err(|_| anyhow::anyhow!("VM heartbeat failed"))?;
    let result = result?;
    let mut gzip = GzEncoder::new(Vec::new(), Compression::fast());
    serde_json::to_writer(&mut gzip, &result)?;
    let compressed = gzip.finish()?;
    ensure!(
        compressed.len() <= MAX_EXPORT,
        "compressed process evidence exceeds16MiB"
    );
    let identity = ObjectId {
        sha256: md_sim_firmware::prepare::hash_bytes(&compressed),
        size: compressed.len() as u64,
    };
    for (index, chunk) in compressed.chunks(16384).enumerate() {
        println!(
            "{MARKER}{}",
            json!({"index":index,"hex":hex::encode(chunk)})
        );
    }
    println!(
        "MD_SIM_PROCESS_VM_RESULT {}",
        json!({
            "compressed":identity,"process_specification":hash_file(Path::new(SPEC))?,
            "status":result.status,"exit_code":result.exit_code,
            "trace_truncated":result.trace_truncated,"firmware_gate_inferred":false
        })
    );
    std::io::stdout().flush()?;
    Ok(())
}

fn recover(out: &Path, spec: &Path) -> Result<ProcessResult> {
    let mut text = String::new();
    File::open(out.join("vm/guest.log"))?
        .take(64 * 1024 * 1024 + 1)
        .read_to_string(&mut text)?;
    ensure!(text.len() <= 64 * 1024 * 1024, "outer VM log bound");
    let mut compressed = Vec::new();
    let mut expected = 0;
    let mut reports = Vec::<Value>::new();
    for line in text.lines().map(|s| s.trim_end_matches('\r')) {
        if let Some(data) = line.strip_prefix(MARKER) {
            ensure!(reports.is_empty(), "evidence chunk follows final result");
            let chunk: Value = serde_json::from_str(data)?;
            ensure!(
                chunk["index"] == expected,
                "process evidence chunk out of order"
            );
            let bytes = hex::decode(chunk["hex"].as_str().context("missing evidence bytes")?)?;
            ensure!(
                bytes.len() <= 16384 && compressed.len() + bytes.len() <= MAX_EXPORT,
                "process evidence byte bound"
            );
            compressed.extend(bytes);
            expected += 1;
        }
        if let Some(data) = line.strip_prefix("MD_SIM_PROCESS_VM_RESULT ") {
            reports.push(serde_json::from_str(data)?);
        }
    }
    ensure!(reports.len() == 1, "process result missing or duplicate");
    ensure!(
        reports[0]["process_specification"] == serde_json::to_value(hash_file(spec)?)?,
        "inner process specification differs"
    );
    let identity = ObjectId {
        sha256: md_sim_firmware::prepare::hash_bytes(&compressed),
        size: compressed.len() as u64,
    };
    ensure!(
        reports[0]["compressed"] == serde_json::to_value(&identity)?,
        "process evidence hash differs"
    );
    fs::write(out.join("process-result.json.gz"), &compressed)?;
    let mut decoded = Vec::new();
    GzDecoder::new(&compressed[..])
        .take(140 * 1024 * 1024 + 1)
        .read_to_end(&mut decoded)?;
    ensure!(
        decoded.len() <= 140 * 1024 * 1024,
        "decoded process evidence bound"
    );
    let result: ProcessResult = serde_json::from_slice(&decoded)?;
    ensure!(
        reports[0]["status"] == result.status
            && reports[0]["exit_code"] == serde_json::to_value(result.exit_code)?
            && reports[0]["trace_truncated"] == result.trace_truncated,
        "process result summary differs from decoded evidence"
    );
    let contract: ProcessSpec = serde_json::from_reader(File::open(spec)?)?;
    let stdout = hex::decode(&result.stdout)?;
    let stderr = hex::decode(&result.stderr)?;
    ensure!(
        stdout.len() as u64 <= contract.limits.trace_bytes / 2
            && stderr.len() as u64 <= contract.limits.trace_bytes / 2,
        "recovered process traces exceed declared limits"
    );
    fs::write(out.join("process-result.json"), decoded)?;
    fs::write(out.join("process.stdout"), stdout)?;
    fs::write(out.join("process.stderr"), stderr)?;
    save_json(
        &out.join("assessment.json"),
        &json!({
            "transported_complete_process_result":true,"inner_status":result.status,"inner_exit_code":result.exit_code,
            "inner_trace_truncated":result.trace_truncated,
            "process_specification":hash_file(spec)?,"result":hash_file(&out.join("process-result.json"))?,
            "topology":"Captured local x86 Linux VM without a network adapter; unchanged inner ProcessSpec in private namespaces.",
            "external_physical_host":false,"firmware_gate_inferred":false
        }),
    )?;
    Ok(result)
}

fn host(spec_path: &Path, source_store: &Path, out: &Path) -> Result<()> {
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(out)?;
    let mut spec_bytes = Vec::new();
    File::open(spec_path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut spec_bytes)?;
    ensure!(spec_bytes.len() <= 1024 * 1024, "process metadata bound");
    let spec: ProcessSpec = serde_json::from_slice(&spec_bytes)?;
    spec.validate()?;
    fs::write(out.join("process-spec.json"), spec_bytes)?;
    let mut total = 0;
    let mut files = BTreeMap::new();
    for id in spec
        .inputs
        .iter()
        .map(|i| &i.object)
        .chain(spec.stdin_object.iter())
    {
        let path = source_store
            .join("objects")
            .join(&id.sha256)
            .canonicalize()?;
        ensure!(hash_file(&path)? == *id, "captured process input differs");
        let name = format!("{OBJECTS}/{}", id.sha256);
        if !files.contains_key(&name) {
            total += id.size;
        }
        files.insert(name, json!({"source":path,"object":id,"mode":365}));
    }
    ensure!(
        total <= MAX_INPUT,
        "captured process input set exceeds512MiB"
    );
    save_json(
        &out.join("payload.json"),
        &json!({
            "schema":"md-sim-vm-payload/1","files":files,"symlinks":{}
        }),
    )?;
    let mut programs = BTreeMap::from([("/process-vm".to_owned(), std::env::current_exe()?)]);
    for name in ["unshare", "bwrap"] {
        programs.insert(
            format!("/usr/bin/{name}"),
            PathBuf::from(format!("/usr/bin/{name}")),
        );
    }
    save_json(
        &out.join("scenario.json"),
        &json!({
            "programs":programs,"files":{
                SPEC:{"path":out.join("process-spec.json").canonicalize()?,"mode":292}
            },"argv":["/process-vm","__guest"],"environment":{"MD_SIM_PROCESS_VM":"1"},
            "payload_manifest":out.join("payload.json").canonicalize()?
        }),
    )?;
    let executable = std::env::current_exe()?
        .parent()
        .context("builder directory missing")?
        .join("isolated_network_vm");
    let status = Command::new(executable)
        .arg(out.join("vm"))
        .arg(out.join("scenario.json"))
        .stdin(Stdio::null())
        .stdout(File::create(out.join("driver.stdout"))?)
        .stderr(File::create(out.join("driver.stderr"))?)
        .status()?;
    let result = recover(out, &out.join("process-spec.json"))?;
    ensure!(
        status.success(),
        "outer VM failed despite retained inner evidence"
    );
    println!(
        "{}",
        json!({"output":out,"inner_status":result.status,"inner_exit_code":result.exit_code})
    );
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated,
        "inner process failed; evidence retained"
    );
    Ok(())
}
fn main() -> Result<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.get(1).is_some_and(|s| s == "__guest") {
        return guest();
    }
    ensure!(
        args.len() == 4,
        "usage: process_vm PROCESS_SPEC INPUT_STORE NEW_OUTPUT"
    );
    let out = Path::new(&args[3]);
    let result = host(Path::new(&args[1]), Path::new(&args[2]), out);
    if let Err(error) = &result {
        if out.is_dir() {
            save_json(
                &out.join("failure.json"),
                &json!({"error":format!("{error:#}"),"completed":false}),
            )?;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(trace_bytes: u64) -> (tempfile::TempDir, PathBuf, String) {
        let directory = tempfile::tempdir().unwrap();
        fs::create_dir(directory.path().join("vm")).unwrap();
        let path = directory.path().join("spec.json");
        let contract = ProcessSpec {
            private_channel: None,
            stdin_stream: None,
            namespace_uid: 0,
            dashboard: None,
            loopback: false,
            schema_version: "2.0".into(),
            argv: vec!["/fixture".into()],
            inputs: vec![],
            environment: BTreeMap::new(),
            stdin_object: None,
            limits: md_sim_runtime::isolation::Limits {
                trace_bytes,
                ..Default::default()
            },
        };
        save_json(&path, &contract).unwrap();
        // Literal failed result: recovery must retain failure, never infer success.
        let bytes=br#"{"status":"timed_out","exit_code":null,"wall_milliseconds":1500,"stdout":"6572726f72","stderr":"","trace_truncated":false,"isolation":"fixture only","limits_scope":"fixture only","tools":{}}"#;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(bytes).unwrap();
        let gzip = encoder.finish().unwrap();
        let chunks = gzip
            .chunks(80)
            .enumerate()
            .map(|(index, bytes)| {
                format!(
                    "{MARKER}{}\n",
                    json!({"index":index,"hex":hex::encode(bytes)})
                )
            })
            .collect::<String>();
        let report = json!({
            "compressed":{"sha256":md_sim_firmware::prepare::hash_bytes(&gzip),"size":gzip.len()},
            "process_specification":hash_file(&path).unwrap(),
            "status":"timed_out","exit_code":null,"trace_truncated":false
        });
        (
            directory,
            path,
            format!("{chunks}MD_SIM_PROCESS_VM_RESULT {report}\n"),
        )
    }

    #[test]
    fn complete_transport_retains_failed_execution_and_exact_output() {
        let (directory, path, log) = fixture(128);
        fs::write(directory.path().join("vm/guest.log"), log).unwrap();
        let result = recover(directory.path(), &path).unwrap();
        assert_eq!(result.status, "timed_out");
        assert_eq!(result.exit_code, None);
        assert_eq!(
            fs::read(directory.path().join("process.stdout")).unwrap(),
            b"error"
        );
        let assessment: Value =
            serde_json::from_reader(File::open(directory.path().join("assessment.json")).unwrap())
                .unwrap();
        assert_eq!(assessment["firmware_gate_inferred"], false);
        assert_eq!(assessment["inner_status"], "timed_out");
    }

    #[test]
    fn recovery_rejects_missing_reordered_duplicate_and_conflicting_evidence() {
        for case in 0..7 {
            let (directory, path, log) = fixture(if case == 6 { 2 } else { 128 });
            let mut lines = log.lines().map(str::to_owned).collect::<Vec<_>>();
            match case {
                0 => {
                    lines.pop();
                }
                1 => lines.swap(0, 1),
                2 => lines.push(lines.last().unwrap().clone()),
                3 => {
                    let last = lines.last_mut().unwrap();
                    *last = last.replace("\"exit_code\":null", "\"exit_code\":0");
                }
                4 => lines.push(lines[0].clone()),
                5 => {
                    // A valid JSON chunk with changed bytes must fail the digest.
                    let mut chunk: Value =
                        serde_json::from_str(lines[0].strip_prefix(MARKER).unwrap()).unwrap();
                    let text = chunk["hex"].as_str().unwrap();
                    chunk["hex"] = json!(format!("00{}", &text[2..]));
                    lines[0] = format!("{MARKER}{chunk}");
                }
                6 => {}
                _ => unreachable!(),
            }
            fs::write(directory.path().join("vm/guest.log"), lines.join("\n")).unwrap();
            assert!(
                recover(directory.path(), &path).is_err(),
                "case{case} must fail"
            );
            assert!(!directory.path().join("assessment.json").exists());
        }
    }
}

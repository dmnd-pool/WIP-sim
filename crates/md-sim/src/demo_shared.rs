//! Run an adapted S9 suite with fresh work through its isolated helpers.
//! This command neither resolves an exact target nor implements a second miner.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::save_json,
    store::{hash_file, ObjectId},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufReader, Read},
    os::unix::{
        fs::{DirBuilderExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

const MANIFEST: &str = include_str!("../../../simulator/demos/s9-shared.json");
const HELPERS: &str = "work/simulator/channel-target-499/release";
const PREPARED: &str = "work/simulator/qemu-s9/native-browser-prepared-494";
const BACKEND: &str = "work/simulator/qemu-s9/backend-build-485-pool-kvm";
const PAYLOAD: &str = "work/simulator/network-vm/native-driver-024/payload.json";
const CORE: &str = "work/simulator/bitcoin-core/source-build-002/bitcoind";
const DRIVER_HASH: &str = "72bef33479774d5979b79d5e86b723f4d6cb618e34543d1e46462b78bfd4cf4f";
static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn stop(_: libc::c_int) {
    STOP.store(true, Ordering::Release);
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema: String,
    inputs: BTreeMap<PathBuf, ObjectId>,
    host_inputs: BTreeMap<PathBuf, ObjectId>,
    limits: Vec<String>,
}

fn helper(name: &str) -> PathBuf {
    Path::new(HELPERS).join(name)
}

fn check(manifest: &Manifest) -> Result<()> {
    ensure!(
        manifest.schema == "md-sim-shared-s9-demo/1",
        "unknown shared demo contract"
    );
    ensure!(
        manifest
            .inputs
            .get(&helper("examples/worker_channel_probe"))
            .is_some_and(|input| input.sha256 == DRIVER_HASH),
        "unrecognized shared simulator runner"
    );
    for required in [
        helper("md-sim-worker-ipc"),
        helper("examples/remote_namespace_check"),
        helper("examples/s9_fresh_prepared"),
        helper("examples/s9_asic_audit"),
        helper("examples/node_prerequisite_probe"),
        Path::new(PREPARED).join("process-spec.json"),
        Path::new(BACKEND).join("build-report.json"),
        PAYLOAD.into(),
        CORE.into(),
    ] {
        ensure!(
            manifest.inputs.contains_key(&required),
            "missing required demo input {}",
            required.display()
        );
    }
    for (path, expected) in &manifest.inputs {
        ensure!(
            !path.as_os_str().is_empty()
                && path.components().all(|c| matches!(c, Component::Normal(_))),
            "demo input is not repository-relative: {}",
            path.display()
        );
        expected.validate()?;
        ensure!(
            hash_file(path)
                .with_context(|| format!("missing demo prerequisite {}", path.display()))?
                == *expected,
            "demo prerequisite changed: {}",
            path.display()
        );
    }
    for (path, expected) in &manifest.host_inputs {
        ensure!(
            path.is_absolute()
                && path.starts_with("/usr")
                && path
                    .components()
                    .all(|c| matches!(c, Component::RootDir | Component::Normal(_))),
            "host prerequisite must be a fixed /usr file"
        );
        expected.validate()?;
        ensure!(
            hash_file(path)? == *expected,
            "host prerequisite changed: {}",
            path.display()
        );
    }
    Ok(())
}

fn read(path: &Path) -> Result<Value> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= 16 << 20,
        "demo report type or size bound"
    );
    let mut bytes = Vec::new();
    file.take((16 << 20) + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 16 << 20, "demo report grew beyond bound");
    Ok(serde_json::from_slice(&bytes)?)
}

// Native assessments contain bounded, embedded artifact exports. The observed
// report exceeds the small-report reader's 16 MiB bound. Stream its verdict
// fields and skip payload values; the independent audits read retained files.
#[derive(Deserialize)]
struct NativeAssessment {
    passed: bool,
    process_specification: ObjectId,
    execution: ObjectId,
    firmware_reference: bool,
    shared_pool: bool,
    carrier_browser_requested: bool,
    complete_result_exported: bool,
    fault: Option<String>,
    fault_gate_passed: bool,
}

fn native_assessment(path: &Path) -> Result<NativeAssessment> {
    const LIMIT: u64 = 128 << 20;
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= LIMIT,
        "native assessment type or size bound"
    );
    let mut bounded = file.take(LIMIT + 1);
    let report = serde_json::from_reader(BufReader::new(&mut bounded))?;
    ensure!(bounded.limit() > 0, "native assessment grew beyond bound");
    Ok(report)
}

fn native_evidence(run: &Path) -> Result<Value> {
    let report = native_assessment(&run.join("assessment.json"))?;
    ensure!(
        report.passed
            && report.firmware_reference
            && report.shared_pool
            && report.carrier_browser_requested
            && report.complete_result_exported
            && report.fault.is_none()
            && !report.fault_gate_passed,
        "native suite did not pass all requested capabilities"
    );
    ensure!(
        report.process_specification == hash_file(&run.join("process-spec.json"))?
            && report.execution == hash_file(&run.join("result.json"))?,
        "native assessment refers to different support execution"
    );
    let execution = run.join("execution");
    let locked = read(&execution.join("locked-run.json"))?;
    let process = read(&execution.join("process-spec.json"))?;
    ensure!(
        locked["process"] == process && process["support_virtualization"].is_null(),
        "selected native process differs from its lock or grants host virtualization"
    );
    // The support accelerator and execution host are outside the native lock.
    // Retain both complete identities. This command always executes a new run;
    // neither a native lock nor a previous host receipt grants result reuse.
    Ok(json!({
        "assessment":hash_file(&run.join("assessment.json"))?,
        "support_process_specification":report.process_specification,
        "support_execution":report.execution,
        "native_locked_run":hash_file(&execution.join("locked-run.json"))?,
        "native_process_specification":hash_file(&execution.join("process-spec.json"))?,
        "worker_run":hash_file(&execution.join("worker-run.json"))?,
        "cache_eligible":false
    }))
}

fn fresh_evidence(
    parent: &Path,
    prepared: &Path,
    execution: &Path,
    helper: &ObjectId,
) -> Result<Value> {
    let path = prepared.join("fresh-work.json");
    let receipt = read(&path)?;
    let identity = hash_file(&path)?;
    let locked = read(&execution.join("locked-run.json"))?;
    ensure!(
        receipt["schema"] == "md-sim-s9-fresh-preparation/1"
            && receipt["helper"] == serde_json::to_value(helper)?
            && locked["inputs"]["fresh-preparation-build"] == receipt["helper"]
            && locked["inputs"]["evidence:fresh-work.json"] == serde_json::to_value(&identity)?
            && locked["policies"]["fresh_preparation"] == serde_json::to_value(&identity)?
            && hash_file(&execution.join("fresh-work.json"))? == identity,
        "selected worker did not execute the bound fresh preparation"
    );
    for (field, name) in [
        ("process", "process-spec.json"),
        ("pool", "pool-input.json"),
    ] {
        let original = hash_file(&parent.join(name))?;
        let child = hash_file(&prepared.join(name))?;
        ensure!(
            original != child
                && receipt[format!("parent_{field}")] == serde_json::to_value(original)?
                && receipt[format!("child_{field}")] == serde_json::to_value(&child)?
                && hash_file(&execution.join(name))? == child,
            "fresh {field} receipt differs from selected inputs"
        );
    }
    for (key, field) in [
        ("process-specification", "child_process"),
        ("evidence:pool-input.json", "child_pool"),
        ("runtime:/private-pool.json", "child_pool"),
    ] {
        ensure!(
            locked["inputs"][key] == receipt[field],
            "fresh child input is not bound in selected lock: {key}"
        );
    }
    for field in ["lock", "process", "pool"] {
        let name = format!("fresh-work-parent-{field}.json");
        let object = hash_file(&prepared.join(&name))?;
        ensure!(
            receipt[format!("parent_{field}")] == serde_json::to_value(&object)?
                && locked["inputs"][format!("evidence:{name}")] == serde_json::to_value(&object)?
                && hash_file(&execution.join(&name))? == object,
            "fresh parent {field} evidence is not bound to the selected run"
        );
    }
    Ok(
        json!({"receipt":identity,"parent_process":receipt["parent_process"],
        "parent_pool":receipt["parent_pool"],"parent_lock":receipt["parent_lock"],"process":receipt["child_process"],
        "pool":receipt["child_pool"],"helper":helper,"cache_eligible":false}),
    )
}

struct Running(Child, bool);
impl Running {
    fn exited_without_reaping(&self) -> std::io::Result<bool> {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                self.0.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(unsafe { info.si_pid() } != 0)
    }

    fn finish(&mut self) -> std::io::Result<std::process::ExitStatus> {
        // Keep the unreaped leader until cleanup, so its process-group ID
        // cannot be reused for an unrelated process between these operations.
        let killed = unsafe { libc::kill(-(self.0.id() as i32), libc::SIGKILL) };
        if killed != 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
            return Err(std::io::Error::last_os_error());
        }
        let status = self.0.wait()?;
        self.1 = true;
        Ok(status)
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if !self.1 {
            let _ = self.finish();
        }
    }
}

fn stage(
    out: &Path,
    name: &str,
    program: &Path,
    arguments: &[String],
    manifest: &Manifest,
) -> Result<()> {
    // Enclose preparation and retention as well as the fixed guest budgets.
    let seconds = if name == "native-browser" { 1800 } else { 300 };
    stage_inner(
        out,
        name,
        program,
        manifest
            .inputs
            .get(program)
            .context("stage helper is absent from frozen inputs")?,
        arguments,
        Duration::from_secs(seconds),
        &STOP,
    )
}

fn stage_inner(
    out: &Path,
    name: &str,
    program: &Path,
    expected: &ObjectId,
    arguments: &[String],
    limit: Duration,
    cancel: &AtomicBool,
) -> Result<()> {
    ensure!(
        !cancel.load(Ordering::Acquire),
        "demo cancelled before {name}"
    );
    ensure!(
        hash_file(program)? == *expected,
        "{name} helper changed before execution"
    );
    save_json(
        &out.join(format!("{name}-invocation.json")),
        &json!({
        "program":program,"program_identity":expected,"arguments":arguments}),
    )?;
    eprintln!("Adapted S9: {name}. Evidence: {}", out.display());
    let mut command = Command::new(program);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .process_group(0)
        .stdout(fs::File::create(out.join(format!("{name}.stdout.log")))?)
        .stderr(fs::File::create(out.join(format!("{name}.stderr.log")))?);
    let parent = unsafe { libc::getpid() };
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != parent {
                return Err(std::io::Error::other("demo parent exited before launch"));
            }
            Ok(())
        });
    }
    let mut child = Running(command.spawn()?, false);
    let deadline = Instant::now() + limit;
    let status = loop {
        if child.exited_without_reaping()? {
            break child.finish()?;
        }
        if cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
            save_json(
                &out.join(format!("{name}-exit.json")),
                &json!({"success":false,"deadline_milliseconds":limit.as_millis(),
                    "pid":child.0.id(),"cancelled":cancel.load(Ordering::Acquire),
                    "error":"supervisor cancellation or deadline"}),
            )?;
            anyhow::bail!("{name} was cancelled or exceeded its supervisor deadline");
        }
        thread::sleep(Duration::from_millis(50));
    };
    save_json(
        &out.join(format!("{name}-exit.json")),
        &json!({"success":status.success(),"exit_code":status.code()}),
    )?;
    ensure!(
        status.success(),
        "{name} failed; its logs and original result remain in {}",
        out.display()
    );
    Ok(())
}

fn verify_candidate(node: &Value, candidate: &Value) -> Result<()> {
    let expected = candidate["block_hash_display"]
        .as_str()
        .context("candidate hash absent")?;
    ensure!(
        expected.len() == 64 && expected.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid candidate hash"
    );
    ensure!(
        node["accepted"] == true
            && node["persisted_after_restart"] == true
            && node["node_generated_block"] == false
            && node["clean_stop_after_each_phase"] == true
            && node["node_result_status"] == "exited"
            && node["expected_block_hash"] == expected
            && node["observed_height_before"] == 0
            && node["observed_height_after"] == 1
            && node["observed_height_after_restart"] == 1
            && node["node_exit_code"] == 0,
        "Core did not accept and retain the selected firmware-produced block"
    );
    Ok(())
}

pub fn run(check_only: bool, output: Option<PathBuf>) -> Result<(Value, i32)> {
    ensure!(
        check_only || output.is_some(),
        "use --check or --out NEW_DIRECTORY"
    );
    let manifest: Manifest = serde_json::from_str(MANIFEST)?;
    check(&manifest)?;
    if check_only {
        return Ok((
            json!({"passed":true,"checked_inputs":manifest.inputs.len(),"checked_host_inputs":manifest.host_inputs.len(),
            "scope":"prepared workspace inputs only; no firmware execution",
            "full_specification_complete":false,"limits":manifest.limits}),
            0,
        ));
    }
    let out = output.context("demo output absent")?;
    fs::create_dir_all(
        out.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?;
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&out)
        .context("demo output must be new")?;
    fs::write(out.join("input-manifest.json"), MANIFEST)?;
    unsafe {
        ensure!(
            libc::signal(libc::SIGINT, stop as *const () as libc::sighandler_t) != libc::SIG_ERR,
            "cannot install interrupt handler"
        );
        ensure!(
            libc::signal(libc::SIGTERM, stop as *const () as libc::sighandler_t) != libc::SIG_ERR,
            "cannot install termination handler"
        );
    }
    save_json(
        &out.join("cli.json"),
        &json!({"executable":hash_file(&std::env::current_exe()?)?,
        "working_directory":std::env::current_dir()?,"manifest":hash_file(&out.join("input-manifest.json"))?}),
    )?;
    let result = (|| -> Result<Value> {
        let execution = out.join("run/execution");
        let prepared = out.join("prepared");
        stage(
            &out,
            "fresh-pool",
            &helper("examples/s9_fresh_prepared"),
            &[PREPARED.into(), prepared.display().to_string()],
            &manifest,
        )?;
        stage(
            &out,
            "native-browser",
            &helper("examples/worker_channel_probe"),
            &[
                helper("md-sim-worker-ipc").display().to_string(),
                BACKEND.into(),
                out.join("run").display().to_string(),
                format!("prepared={}", prepared.display()),
                format!("ssh-helper={HELPERS}/examples/remote_namespace_check"),
                format!("ssh-browser-payload={PAYLOAD}"),
                "support-accel=kvm".into(),
            ],
            &manifest,
        )?;
        let native = native_evidence(&out.join("run"))?;
        let fresh = fresh_evidence(
            Path::new(PREPARED),
            &prepared,
            &execution,
            manifest
                .inputs
                .get(&helper("examples/s9_fresh_prepared"))
                .context("fresh preparation helper identity absent")?,
        )?;
        let rates = execution.join("demo-rate-audit.json");
        let audit = execution.join("demo-regtest-audit.json");
        for (name, mode, report) in [
            ("rates", "rates-shared", &rates),
            ("mining", "regtest-shared", &audit),
        ] {
            stage(
                &out,
                name,
                &helper("examples/s9_asic_audit"),
                &[
                    mode.into(),
                    execution.display().to_string(),
                    report.display().to_string(),
                ],
                &manifest,
            )?;
        }
        let mining = read(&audit)?;
        ensure!(
            mining["result"]["fresh_jobs"]
                .as_u64()
                .is_some_and(|n| n >= 2)
                && mining["result"]["clean_job_transition"] == true
                && mining["result"]["difficulty_change"] == true,
            "fresh-work transition evidence absent"
        );
        let candidates = mining["result"]["regtest_blocks"]
            .as_array()
            .filter(|blocks| blocks.len() >= 2)
            .context("two firmware block candidates required")?;
        ensure!(
            candidates[0]["block_hash_display"] != candidates[1]["block_hash_display"],
            "duplicate candidate blocks"
        );
        let mut nodes = Vec::new();
        for (index, candidate) in candidates.iter().take(2).enumerate() {
            let node = out.join(format!("core-{index}"));
            stage(
                &out,
                &format!("core-{index}"),
                &helper("examples/node_prerequisite_probe"),
                &[
                    CORE.into(),
                    node.display().to_string(),
                    "shared".into(),
                    audit.display().to_string(),
                    index.to_string(),
                ],
                &manifest,
            )?;
            let report = node.join("block-acceptance.json");
            verify_candidate(&read(&report)?, candidate)?;
            nodes.push(json!({"path":report,"identity":hash_file(&report)?}));
        }
        check(&manifest)?;
        Ok(json!({"native":native,"fresh_preparation":fresh,
            "rate_audit":hash_file(&rates)?,"mining_audit":hash_file(&audit)?,"core":nodes}))
    })();
    let summary = json!({"passed":result.is_ok(),"classification":"original-firmware-adapted-reference-environment",
        "full_specification_complete":false,"result":result.as_ref().ok(),
        "error":result.as_ref().err().map(|error| format!("{error:#}")),"limits":manifest.limits});
    save_json(&out.join("assessment.json"), &summary)?;
    Ok((summary, if result.is_ok() { 0 } else { 1 }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_receipt_must_join_the_selected_execution_and_helper() -> Result<()> {
        let out = tempfile::tempdir()?;
        let parent = out.path().join("parent");
        let prepared = out.path().join("prepared");
        let execution = out.path().join("execution");
        for dir in [&parent, &prepared, &execution] {
            fs::create_dir(dir)?;
        }
        let helper = ObjectId {
            sha256: "1".repeat(64),
            size: 123,
        };
        // Join regression only. These metadata bytes do not claim firmware
        // execution, valid preparation, or a mining result.
        let mut receipt = json!({"schema":"md-sim-s9-fresh-preparation/1","helper":helper});
        for (field, name) in [
            ("process", "process-spec.json"),
            ("pool", "pool-input.json"),
        ] {
            fs::write(parent.join(name), format!("parent {field}"))?;
            fs::write(prepared.join(name), format!("child {field}"))?;
            fs::copy(prepared.join(name), execution.join(name))?;
            receipt[format!("parent_{field}")] =
                serde_json::to_value(hash_file(&parent.join(name))?)?;
            receipt[format!("child_{field}")] =
                serde_json::to_value(hash_file(&prepared.join(name))?)?;
            let retained = format!("fresh-work-parent-{field}.json");
            fs::copy(parent.join(name), prepared.join(&retained))?;
            fs::copy(parent.join(name), execution.join(retained))?;
        }
        let parent_lock = "fresh-work-parent-lock.json";
        fs::write(prepared.join(parent_lock), b"parent lock")?;
        fs::copy(prepared.join(parent_lock), execution.join(parent_lock))?;
        receipt["parent_lock"] = serde_json::to_value(hash_file(&prepared.join(parent_lock))?)?;
        save_json(&prepared.join("fresh-work.json"), &receipt)?;
        fs::copy(
            prepared.join("fresh-work.json"),
            execution.join("fresh-work.json"),
        )?;
        let identity = hash_file(&prepared.join("fresh-work.json"))?;
        let mut lock = json!({"inputs":{"fresh-preparation-build":helper,
            "evidence:fresh-work.json":identity,
            "process-specification":receipt["child_process"],
            "runtime:/private-pool.json":receipt["child_pool"],
            "evidence:pool-input.json":receipt["child_pool"]},
            "policies":{"fresh_preparation":identity}});
        for field in ["lock", "process", "pool"] {
            lock["inputs"][format!("evidence:fresh-work-parent-{field}.json")] =
                receipt[format!("parent_{field}")].clone();
        }
        save_json(&execution.join("locked-run.json"), &lock)?;
        assert!(fresh_evidence(&parent, &prepared, &execution, &helper).is_ok());
        let other_helper = ObjectId {
            sha256: "2".repeat(64),
            size: 123,
        };
        assert!(fresh_evidence(&parent, &prepared, &execution, &other_helper).is_err());
        for name in [
            "process-spec.json",
            "pool-input.json",
            "fresh-work.json",
            "fresh-work-parent-lock.json",
            "fresh-work-parent-process.json",
            "fresh-work-parent-pool.json",
        ] {
            fs::write(execution.join(name), b"other run")?;
            assert!(fresh_evidence(&parent, &prepared, &execution, &helper).is_err());
            fs::copy(prepared.join(name), execution.join(name))?;
        }
        let mut unbound = lock.clone();
        unbound["policies"]["fresh_preparation"] = Value::Null;
        save_json(&execution.join("locked-run.json"), &unbound)?;
        assert!(fresh_evidence(&parent, &prepared, &execution, &helper).is_err());
        save_json(&execution.join("locked-run.json"), &lock)?;
        for key in [
            "process-specification",
            "runtime:/private-pool.json",
            "evidence:pool-input.json",
            "evidence:fresh-work-parent-lock.json",
        ] {
            let mut unbound = lock.clone();
            unbound["inputs"][key] = Value::Null;
            save_json(&execution.join("locked-run.json"), &unbound)?;
            assert!(fresh_evidence(&parent, &prepared, &execution, &helper).is_err());
        }
        save_json(&execution.join("locked-run.json"), &lock)?;
        fs::write(parent.join("pool-input.json"), b"different baseline")?;
        assert!(fresh_evidence(&parent, &prepared, &execution, &helper).is_err());
        Ok(())
    }

    fn assessment_fixture() -> Value {
        json!({"passed":true,"process_specification":{"sha256":"0".repeat(64),"size":0},
            "execution":{"sha256":"0".repeat(64),"size":0},
            "firmware_reference":true,"shared_pool":true,"carrier_browser_requested":true,
            "complete_result_exported":true,"fault":null,"fault_gate_passed":false})
    }

    #[test]
    fn native_reader_streams_large_artifacts_and_rejects_ambiguous_verdicts() -> Result<()> {
        use std::io::Write;
        let out = tempfile::tempdir()?;
        let path = out.path().join("assessment.json");
        let mut file = fs::File::create(&path)?;
        file.write_all(b"{\"observations\":[\"")?;
        // The real native report is about 49 MiB. This regression exceeds the
        // small-report limit without allocating a payload-sized JSON value.
        let chunk = [b'a'; 65536];
        for _ in 0..272 {
            file.write_all(&chunk)?;
        }
        file.write_all(b"\"],")?;
        file.write_all(&serde_json::to_vec(&assessment_fixture())?[1..])?;
        drop(file);
        assert!(native_assessment(&path)?.passed);
        assert!(read(&path).is_err());
        fs::write(&path, b"{\"passed\":true,\"passed\":false}")?;
        assert!(native_assessment(&path).is_err());
        fs::write(&path, b"{\"passed\":true}")?;
        assert!(native_assessment(&path).is_err());
        Ok(())
    }

    #[test]
    fn native_reader_rejects_links_and_excessive_reports() -> Result<()> {
        use std::os::unix::fs::symlink;
        let out = tempfile::tempdir()?;
        let path = out.path().join("assessment.json");
        fs::write(&path, serde_json::to_vec(&assessment_fixture())?)?;
        let link = out.path().join("link");
        symlink(&path, &link)?;
        assert!(native_assessment(&link).is_err());
        fs::OpenOptions::new()
            .write(true)
            .open(&path)?
            .set_len((128 << 20) + 1)?;
        assert!(native_assessment(&path).is_err());
        Ok(())
    }

    #[test]
    #[ignore = "requires retained native479 evidence; checks parsing, never grants a suite pass"]
    fn retained_native479_report_streams_and_preserves_failure() -> Result<()> {
        let run = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../work/simulator/network-vm/native-browser-479");
        assert_eq!(hash_file(&run.join("assessment.json"))?.size, 49_409_766);
        let report = native_assessment(&run.join("assessment.json"))?;
        assert!(!report.passed);
        assert!(report.carrier_browser_requested);
        assert_eq!(
            report.process_specification,
            hash_file(&run.join("process-spec.json"))?
        );
        assert_eq!(report.execution, hash_file(&run.join("result.json"))?);
        assert!(native_evidence(&run).is_err());
        Ok(())
    }

    #[test]
    fn native_evidence_joins_support_identity_and_rejects_changed_execution() -> Result<()> {
        let out = tempfile::tempdir()?;
        let run = out.path();
        let execution = run.join("execution");
        fs::create_dir(&execution)?;
        fs::write(
            run.join("process-spec.json"),
            b"{\"support_virtualization\":\"kvm\"}",
        )?;
        fs::write(run.join("result.json"), b"{\"status\":\"exited\"}")?;
        fs::write(
            execution.join("process-spec.json"),
            b"{\"argv\":[\"/firmware\"]}",
        )?;
        fs::write(
            execution.join("locked-run.json"),
            b"{\"process\":{\"argv\":[\"/firmware\"]}}",
        )?;
        fs::write(
            execution.join("worker-run.json"),
            b"{\"run_id\":\"unit-fixture\"}",
        )?;
        let mut report = assessment_fixture();
        report["process_specification"] =
            serde_json::to_value(hash_file(&run.join("process-spec.json"))?)?;
        report["execution"] = serde_json::to_value(hash_file(&run.join("result.json"))?)?;
        save_json(&run.join("assessment.json"), &report)?;
        let joined = native_evidence(run)?;
        assert_eq!(joined["cache_eligible"], false);
        assert_eq!(
            joined["support_process_specification"],
            report["process_specification"]
        );
        assert!(joined["native_locked_run"]["sha256"].is_string());
        fs::write(run.join("result.json"), b"{\"status\":\"changed\"}")?;
        assert!(native_evidence(run).is_err());
        report["execution"] = serde_json::to_value(hash_file(&run.join("result.json"))?)?;
        save_json(&run.join("assessment.json"), &report)?;
        fs::write(
            execution.join("process-spec.json"),
            b"{\"argv\":[\"/different\"]}",
        )?;
        assert!(native_evidence(run).is_err());
        Ok(())
    }

    #[test]
    fn report_reader_rejects_symlinks_special_files_and_oversized_files() -> Result<()> {
        use std::os::unix::{ffi::OsStrExt, fs::symlink};
        let out = tempfile::tempdir()?;
        let regular = out.path().join("regular");
        fs::write(&regular, b"{\"accepted\":false}")?;
        assert_eq!(read(&regular)?["accepted"], false);
        let link = out.path().join("link");
        symlink(&regular, &link)?;
        assert!(read(&link).is_err());
        let fifo = out.path().join("fifo");
        let name = std::ffi::CString::new(fifo.as_os_str().as_bytes())?;
        ensure!(
            unsafe { libc::mkfifo(name.as_ptr(), 0o600) } == 0,
            "FIFO fixture failed"
        );
        assert!(read(&fifo).is_err());
        assert!(read(out.path()).is_err());
        fs::File::create(&regular)?.set_len((16 << 20) + 1)?;
        assert!(read(&regular).is_err());
        Ok(())
    }
    #[test]
    fn failed_and_expired_tools_keep_failure_evidence_and_reap_owned_children() -> Result<()> {
        let out = tempfile::tempdir()?;
        let cancel = AtomicBool::new(false);
        stage_inner(
            out.path(),
            "success",
            Path::new("/usr/bin/true"),
            &hash_file(Path::new("/usr/bin/true"))?,
            &[],
            Duration::from_secs(2),
            &cancel,
        )?;
        assert!(stage_inner(
            out.path(),
            "failed",
            Path::new("/usr/bin/false"),
            &hash_file(Path::new("/usr/bin/false"))?,
            &[],
            Duration::from_secs(2),
            &cancel
        )
        .is_err());
        assert_eq!(read(&out.path().join("failed-exit.json"))?["exit_code"], 1);
        let start = Instant::now();
        assert!(stage_inner(
            out.path(),
            "expired",
            Path::new("/usr/bin/sleep"),
            &hash_file(Path::new("/usr/bin/sleep"))?,
            &["30".into()],
            Duration::from_millis(100),
            &cancel
        )
        .is_err());
        assert!(start.elapsed() < Duration::from_secs(5));
        let result = read(&out.path().join("expired-exit.json"))?;
        assert_eq!(result["success"], false);
        let pid = result["pid"].as_u64().unwrap() as i32;
        assert_eq!(
            unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) },
            -1
        );
        assert_eq!(
            std::io::Error::last_os_error().raw_os_error(),
            Some(libc::ECHILD)
        );
        cancel.store(true, Ordering::Release);
        assert!(stage_inner(
            out.path(),
            "cancelled",
            Path::new("/usr/bin/true"),
            &hash_file(Path::new("/usr/bin/true"))?,
            &[],
            Duration::from_secs(2),
            &cancel
        )
        .is_err());
        assert!(!out.path().join("cancelled-invocation.json").exists());
        Ok(())
    }
    #[test]
    fn failed_helper_cannot_leave_a_running_group_child_or_use_changed_bytes() -> Result<()> {
        let out = tempfile::tempdir()?;
        let cancel = AtomicBool::new(false);
        let shell = Path::new("/usr/bin/sh");
        let identity = hash_file(shell)?;
        let pid_path = out.path().join("child.pid");
        let args = [
            "-c".into(),
            "/usr/bin/sleep 30 & printf '%s\\n' \"$!\" > \"$1\"; exit 7".into(),
            "owned-child-test".into(),
            pid_path.display().to_string(),
        ];
        assert!(stage_inner(
            out.path(),
            "owned",
            shell,
            &identity,
            &args,
            Duration::from_secs(2),
            &cancel
        )
        .is_err());
        assert_eq!(read(&out.path().join("owned-exit.json"))?["exit_code"], 7);
        let pid: u32 = fs::read_to_string(pid_path)?.trim().parse()?;
        let until = Instant::now() + Duration::from_secs(2);
        loop {
            match fs::read_to_string(format!("/proc/{pid}/stat")) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
                Ok(stat)
                    if stat
                        .rsplit_once(") ")
                        .is_some_and(|(_, fields)| fields.starts_with('Z')) =>
                {
                    break
                }
                _ => ensure!(Instant::now() < until, "owned descendant remained running"),
            }
            thread::sleep(Duration::from_millis(10));
        }
        let wrong = ObjectId {
            sha256: "00".repeat(32),
            size: identity.size,
        };
        assert!(stage_inner(
            out.path(),
            "changed",
            shell,
            &wrong,
            &args,
            Duration::from_secs(2),
            &cancel
        )
        .is_err());
        assert!(!out.path().join("changed-invocation.json").exists());
        Ok(())
    }
    #[test]
    fn core_result_must_bind_the_actual_candidate_and_restart_chain() {
        let hash = "12".repeat(32);
        let candidate = json!({"block_hash_display":hash});
        let node = json!({"accepted":true,"persisted_after_restart":true,"node_generated_block":false,
            "clean_stop_after_each_phase":true,"node_result_status":"exited",
            "expected_block_hash":hash,"observed_height_before":0,"observed_height_after":1,
            "observed_height_after_restart":1,"node_exit_code":0});
        verify_candidate(&node, &candidate).unwrap();
        for (field, value) in [
            ("accepted", json!(false)),
            ("persisted_after_restart", json!(false)),
            ("node_generated_block", json!(true)),
            ("expected_block_hash", json!("34".repeat(32))),
            ("observed_height_before", json!(1)),
            ("observed_height_after_restart", json!(0)),
            ("node_exit_code", json!(1)),
        ] {
            let mut wrong = node.clone();
            wrong[field] = value;
            assert!(verify_candidate(&wrong, &candidate).is_err(), "{field}");
        }
        assert!(verify_candidate(&node, &json!({})).is_err());
    }
}

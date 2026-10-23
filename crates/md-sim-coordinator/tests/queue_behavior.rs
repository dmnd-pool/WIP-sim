//! Behavioral checks use actual isolated installed shell processes and workers.
//! They make no firmware, miner or external-host capability claim.
use anyhow::{ensure, Result};
use md_sim_coordinator::{
    executor::{
        local_tools, Executor, ExecutorIdentity, PrivateWorker, WorkerExecutor, WorkerTransport,
    },
    queue::{AttemptResult, EvidenceWriter},
    ExecutionScope, GateResult, GateStatus, LocalExecutor, LockedPlan, LockedRun, PlanContent,
    Queue, QueueLimits, RetentionIdentity, RetentionRequest, SuiteIdentity, SuiteVerifier,
};
use md_sim_runtime::{
    isolation::{self, Limits, Mount, ProcessResult, ProcessSpec, TraceFiles},
    store::{hash_file, ObjectId, Store},
    worker::Request,
};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
    thread,
    time::{Duration, Instant},
};

const EXPECTED: &[u8] = b"actual isolated queue process\n";
#[path = "support/elf.rs"]
mod elf;
// These real processes share the enclosing UID's process accounting. Running
// independent fixtures together can consume one another's rlimit and startup
// deadline. Each test still exercises its own concurrent worker/control paths.
static PROCESS_FIXTURE: Mutex<()> = Mutex::new(());

fn import(store: &Store, path: &Path) -> ObjectId {
    let object = hash_file(path).unwrap();
    store.import(path, &object).unwrap();
    object
}

fn shell_inputs(store: &Store) -> Vec<Mount> {
    elf::inputs(store, Path::new("/usr/bin/sh"), "/usr/bin/sh")
}

#[derive(Clone, Copy)]
enum OracleMode {
    Check,
    MissingEvidence,
    ExcessiveExport,
}
struct Oracle {
    identity: SuiteIdentity,
    mode: OracleMode,
}
impl SuiteVerifier for Oracle {
    fn suite(&self) -> SuiteIdentity {
        self.identity.clone()
    }
    fn verify(
        &self,
        _store: &Store,
        _run: &LockedRun,
        process: &ProcessResult,
        out: &mut EvidenceWriter,
    ) -> Result<Vec<GateResult>> {
        let stdout = hex::decode(&process.stdout)?;
        let correct = process.status == "exited"
            && process.exit_code == Some(0)
            && !process.trace_truncated
            && stdout == EXPECTED
            && process.stderr.is_empty();
        out.write("stdout.txt", &stdout)?;
        if matches!(self.mode, OracleMode::ExcessiveExport) {
            out.write("oversized.bin", &vec![7; 16 * 1024 * 1024])?;
        }
        let reference = if matches!(self.mode, OracleMode::MissingEvidence) {
            "evidence/absent.txt".to_string()
        } else {
            out.reference("stdout.txt")?
        };
        Ok(vec![GateResult {
            id: "shell-output".into(),
            status: if correct {
                GateStatus::Pass
            } else {
                GateStatus::Fail
            },
            evidence: BTreeSet::from([reference]),
            actual: json!(String::from_utf8_lossy(&stdout)),
            expected: json!(String::from_utf8_lossy(EXPECTED)),
            tolerance: json!("exact bytes"),
            clock_domain: "process".into(),
            test_version: "1".into(),
        }])
    }
}

struct Fixture {
    _temporary: tempfile::TempDir,
    source: Store,
    plan: LockedPlan,
    oracle: Oracle,
    root: PathBuf,
    _process_fixture: MutexGuard<'static, ()>,
}
fn fixture(mode: OracleMode) -> Fixture {
    // Retain failures from one test without preventing the remaining checks.
    let process_fixture = PROCESS_FIXTURE.lock().unwrap_or_else(|e| e.into_inner());
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().to_path_buf();
    let source = Store::open(&root.join("source"), 32 * 1024 * 1024).unwrap();
    let runtime_inputs = shell_inputs(&source);
    let suite_path = root.join("suite.json");
    fs::write(&suite_path, b"{\"suite\":\"exact shell stdout v1\"}\n").unwrap();
    let suite = import(&source, &suite_path);
    let verifier = import(
        &source,
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/queue_behavior.rs"),
    );
    let identity = SuiteIdentity {
        id: "isolated-shell-behavior".into(),
        version: "1".into(),
        suite: suite.clone(),
        verifier: verifier.clone(),
        required_gates: BTreeSet::from(["shell-output".into()]),
    };
    let mut inputs: BTreeMap<_, _> = runtime_inputs
        .iter()
        .map(|m| (m.guest_path.clone(), m.object.clone()))
        .collect();
    inputs.insert("suite".into(), suite);
    inputs.insert("verifier".into(), verifier);
    let process=ProcessSpec{private_channel: None, stdin_stream:None,namespace_uid:0,dashboard:None,loopback:false,schema_version:"2.0".into(),argv:vec!["/usr/bin/sh".into(),"-c".into(),"test ! -e /home/user && test ! -e /etc/passwd && printf 'actual isolated queue process\\n'".into()],inputs:runtime_inputs,stdin_object:None,environment:BTreeMap::new(),limits:Limits{trace_bytes:4096,..Limits::default()}};
    let run = LockedRun {
        name: "actual-process".into(),
        scope: ExecutionScope::ProcessPrimitive {
            description: "A real bounded shell validates immutable queue evidence, not firmware."
                .into(),
        },
        process: Some(process),
        suite: identity.clone(),
        inputs,
        backend_tools: local_tools().unwrap(),
        policies: json!({"seed":1,"hash_cap":10000,"clock":"wall","scope":"test process only"}),
        capability_limits: vec!["No firmware or exact hardware gate".into()],
    };
    let plan = LockedPlan::new(PlanContent {
        schema_version: "2.0".into(),
        catalogue_snapshot: None,
        limits: QueueLimits {
            max_attempts: 16,
            max_evidence_bytes: 32 * 1024 * 1024,
        },
        runs: vec![run],
    })
    .unwrap();
    Fixture {
        _temporary: temporary,
        source,
        plan,
        oracle: Oracle { identity, mode },
        root,
        _process_fixture: process_fixture,
    }
}
fn cancel() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

// The cancellation test exposes only bounded, run-owned trace files. This lets
// its controller observe actual child output before asking the queue to stop.
struct ObservedLocal;
impl Executor for ObservedLocal {
    fn identity(&mut self) -> Result<ExecutorIdentity> {
        LocalExecutor.identity()
    }
    fn run(
        &mut self,
        store: &Store,
        run: &LockedRun,
        cancel: Arc<AtomicBool>,
        out: &Path,
    ) -> Result<ProcessResult> {
        isolation::execute_with_trace(
            store,
            run.process.as_ref().unwrap(),
            cancel,
            Some(TraceFiles {
                stdout: fs::File::create(out.join("observed-stdout.log"))?,
                stderr: fs::File::create(out.join("observed-stderr.log"))?,
            }),
        )
    }
}
fn result(path: &str) -> AttemptResult {
    serde_json::from_slice(&fs::read(Path::new(path).join("result.json")).unwrap()).unwrap()
}

#[test]
fn local_executor_retains_output_before_the_process_stops() {
    let fixture = fixture(OracleMode::Check);
    let mut run = fixture.plan.content.runs[0].clone();
    let spec = run.process.as_mut().unwrap();
    spec.argv[2] =
        "printf 'live stdout\\n'; printf 'live stderr\\n' >&2; while :; do :; done".into();
    spec.limits.cpu_seconds = 10;
    spec.limits.wall_seconds = 15;
    let out = fixture.root.join("live-execution");
    fs::create_dir(&out).unwrap();
    let stop = cancel();
    let (seen, result) = thread::scope(|scope| {
        let execution =
            scope.spawn(|| LocalExecutor.run(&fixture.source, &run, stop.clone(), &out));
        let deadline = Instant::now() + Duration::from_secs(5);
        let seen = loop {
            if fs::read(out.join("local-stdout.log")).ok().as_deref() == Some(b"live stdout\n")
                && fs::read(out.join("local-stderr.log")).ok().as_deref() == Some(b"live stderr\n")
            {
                break true;
            }
            if Instant::now() >= deadline {
                break false;
            }
            thread::sleep(Duration::from_millis(10));
        };
        stop.store(true, Ordering::Release);
        (seen, execution.join().unwrap().unwrap())
    });
    assert!(seen, "actual output was not visible while the guest ran");
    assert_eq!(result.status, "cancelled");
    assert!(!result.trace_truncated);
    assert_eq!(
        fs::read(out.join("local-stdout.log")).unwrap(),
        hex::decode(result.stdout).unwrap()
    );
    assert_eq!(
        fs::read(out.join("local-stderr.log")).unwrap(),
        hex::decode(result.stderr).unwrap()
    );
}

#[test]
fn proc_view_uses_the_executing_process_pid_namespace() -> Result<()> {
    let fixture = fixture(OracleMode::Check);
    let mut run = fixture.plan.content.runs[0].clone();
    let marker = fixture.root.join("outside-process-marker");
    fs::write(&marker, b"must not be visible through any process root")?;
    let spec = run.process.as_mut().unwrap();
    spec.environment.insert(
        "MD_SIM_OUTSIDE_MARKER".into(),
        marker.to_str().unwrap().into(),
    );
    spec.argv[2] = "set -e; read observed rest < /proc/self/stat; printf 'shell=%s proc=%s\\n' \"$$\" \"$observed\"; test \"$$\" = \"$observed\"; for process in /proc/[0-9]*; do test ! -e \"$process/root/$MD_SIM_OUTSIDE_MARKER\"; done".into();
    let out = fixture.root.join("proc-local");
    fs::create_dir(&out)?;
    let local = LocalExecutor.run(&fixture.source, &run, cancel(), &out)?;
    assert_eq!(
        local.exit_code,
        Some(0),
        "local: {}",
        String::from_utf8_lossy(&hex::decode(local.stdout)?)
    );
    let mut worker = PrivateWorker::spawn(
        Path::new(env!("CARGO_BIN_EXE_md-sim-worker-ipc")),
        &fixture.root.join("worker"),
    )?;
    let out = fixture.root.join("proc-worker");
    fs::create_dir(&out)?;
    let remote = worker.executor.run(&fixture.source, &run, cancel(), &out)?;
    assert_eq!(
        remote.exit_code,
        Some(0),
        "worker: {}",
        String::from_utf8_lossy(&hex::decode(remote.stdout)?)
    );
    Ok(())
}

#[test]
fn worker_live_output_is_visible_and_checked_before_cleanup() -> Result<()> {
    for corrupt in [false, true] {
        let fixture = fixture(OracleMode::Check);
        let mut run = fixture.plan.content.runs[0].clone();
        let spec = run.process.as_mut().unwrap();
        spec.argv[2] = "printf 'live worker stdout\\n'; printf 'live worker stderr\\n' >&2; while :; do :; done".into();
        spec.limits.cpu_seconds = 10;
        spec.limits.wall_seconds = 15;
        let out = fixture.root.join("live-worker");
        fs::create_dir(&out)?;
        let mut worker = PrivateWorker::spawn(
            Path::new(env!("CARGO_BIN_EXE_md-sim-worker-ipc")),
            &fixture.root.join("worker"),
        )?;
        let stop = cancel();
        let (seen, result) = thread::scope(|scope| {
            let execution = scope.spawn(|| {
                worker
                    .executor
                    .run(&fixture.source, &run, stop.clone(), &out)
            });
            // Transfer and executable identity checks precede execution. Give
            // those their own bound; observe live output for at most five
            // seconds after the coordinator creates its live trace files.
            let setup_deadline = Instant::now() + Duration::from_secs(15);
            let mut live_deadline = None;
            let seen = loop {
                if live_deadline.is_none() && out.join("worker-stdout.log").exists() {
                    live_deadline = Some(Instant::now() + Duration::from_secs(5));
                }
                if fs::read(out.join("worker-stdout.log")).ok().as_deref()
                    == Some(b"live worker stdout\n")
                    && fs::read(out.join("worker-stderr.log")).ok().as_deref()
                        == Some(b"live worker stderr\n")
                {
                    break true;
                }
                if Instant::now() >= live_deadline.unwrap_or(setup_deadline)
                    || execution.is_finished()
                {
                    break false;
                }
                thread::sleep(Duration::from_millis(10));
            };
            if corrupt {
                fs::write(out.join("worker-stdout.log"), b"changed live stdout\n").unwrap();
            }
            stop.store(true, Ordering::Release);
            (seen, execution.join().unwrap())
        });
        assert!(seen, "worker output was not visible before cancellation: {result:?}; stdout={:?}; stderr={:?}", fs::read(out.join("worker-stdout.log")), fs::read(out.join("worker-stderr.log")));
        if corrupt {
            assert!(result.is_err(), "a changed live prefix must fail");
            assert!(!out.join("worker-cleanup.json").exists());
            assert_eq!(
                fs::read(out.join("worker-stdout.log"))?,
                b"changed live stdout\n"
            );
            let saved: serde_json::Value =
                serde_json::from_slice(&fs::read(out.join("worker-run.json"))?)?;
            let id = saved["run_id"].as_str().unwrap();
            assert!(fixture
                .root
                .join("worker/runs")
                .join(id)
                .join("result.json")
                .is_file());
        } else {
            let result = result?;
            assert_eq!(result.status, "cancelled");
            assert!(!result.trace_truncated);
            assert_eq!(
                fs::read(out.join("worker-stdout.log"))?,
                hex::decode(result.stdout)?
            );
            assert_eq!(
                fs::read(out.join("worker-stderr.log"))?,
                hex::decode(result.stderr)?
            );
            assert!(out.join("worker-cleanup.json").exists());
        }
    }
    Ok(())
}

#[test]
fn local_resume_requires_unchanged_retained_evidence() {
    let fixture = fixture(OracleMode::Check);
    let mut queue =
        Queue::create(&fixture.root.join("queue"), fixture.plan, &fixture.source).unwrap();
    let first = queue
        .run(&mut LocalExecutor, &fixture.oracle, false, cancel())
        .unwrap();
    assert!(first.declared_suite_complete);
    assert_eq!(first.exact_physical_targets_executed, 0);
    assert!(!first.complete_hardware_matrix);
    let old = first.rows[0].attempt.clone();
    assert!(!first.rows[0].reused);
    let second = queue
        .run(&mut LocalExecutor, &fixture.oracle, true, cancel())
        .unwrap();
    assert!(second.rows[0].reused);
    assert_eq!(second.rows[0].attempt, old);
    fs::write(
        Path::new(&old).join("evidence/stdout.txt"),
        b"tampered evidence",
    )
    .unwrap();
    let third = queue
        .run(&mut LocalExecutor, &fixture.oracle, true, cancel())
        .unwrap();
    assert!(third.declared_suite_complete);
    assert!(!third.rows[0].reused);
    assert_ne!(third.rows[0].attempt, old);
    assert_eq!(
        fs::read(Path::new(&old).join("evidence/stdout.txt")).unwrap(),
        b"tampered evidence"
    );
}

#[test]
fn nonexistent_gate_evidence_and_excessive_export_cannot_pass() {
    for mode in [OracleMode::MissingEvidence, OracleMode::ExcessiveExport] {
        let fixture = fixture(mode);
        let mut queue =
            Queue::create(&fixture.root.join("queue"), fixture.plan, &fixture.source).unwrap();
        let report = queue
            .run(&mut LocalExecutor, &fixture.oracle, false, cancel())
            .unwrap();
        assert_eq!(report.rows[0].status, "fail");
        assert!(!report.declared_suite_complete);
        let saved = result(&report.rows[0].attempt);
        assert!(!saved.errors.is_empty());
        assert!(!Path::new(&report.rows[0].attempt)
            .join("evidence/oversized.bin")
            .exists());
    }
}

#[test]
fn interrupted_attempt_before_lock_is_retained_then_reexecuted() {
    let fixture = fixture(OracleMode::Check);
    let run_hash = fixture.plan.content.runs[0].hash().unwrap();
    let root = fixture.root.join("queue");
    let queue = Queue::create(&root, fixture.plan, &fixture.source).unwrap();
    let interrupted = root.join("jobs").join(run_hash).join("attempt-000001");
    fs::create_dir_all(&interrupted).unwrap();
    let unsealed_result = b"{\"status\":\"uncommitted prior bytes\"}\n";
    fs::write(interrupted.join("result.json"), unsealed_result).unwrap();
    drop(queue);
    let mut reopened = Queue::open(&root).unwrap();
    let report = reopened
        .run(&mut LocalExecutor, &fixture.oracle, true, cancel())
        .unwrap();
    assert!(report.declared_suite_complete);
    assert!(!report.rows[0].reused);
    assert!(report.rows[0].attempt.ends_with("attempt-000002"));
    let prior = result(interrupted.to_str().unwrap());
    assert_eq!(prior.status, "interrupted");
    assert!(!prior.errors.is_empty());
    assert!(interrupted.join("completion.json").is_file());
    assert_eq!(
        fs::read(interrupted.join("result-before-seal-00.json")).unwrap(),
        unsealed_result
    );
}

#[test]
fn a_private_worker_executes_transfers_reuses_and_cleans_actual_results() -> Result<()> {
    let fixture = fixture(OracleMode::Check);
    let mut queue = Queue::create(&fixture.root.join("queue"), fixture.plan, &fixture.source)?;
    let worker_root = fixture.root.join("worker");
    let mut worker = PrivateWorker::spawn(
        Path::new(env!("CARGO_BIN_EXE_md-sim-worker-ipc")),
        &worker_root,
    )?;
    let report = queue.run(&mut worker.executor, &fixture.oracle, false, cancel())?;
    ensure!(
        report.declared_suite_complete,
        "worker queue did not pass: {:?}",
        result(&report.rows[0].attempt).errors
    );
    let attempt = Path::new(&report.rows[0].attempt);
    assert!(attempt.join("artifact-transfer.json").is_file());
    let cleanup: serde_json::Value =
        serde_json::from_slice(&fs::read(attempt.join("worker-cleanup.json"))?)?;
    assert_eq!(cleanup["removed"], true);
    let original = report.rows[0].attempt.clone();
    drop(worker);
    let mut reconnected = PrivateWorker::spawn(
        Path::new(env!("CARGO_BIN_EXE_md-sim-worker-ipc")),
        &worker_root,
    )?;
    let resumed = queue.run(&mut reconnected.executor, &fixture.oracle, true, cancel())?;
    assert!(resumed.rows[0].reused);
    assert_eq!(resumed.rows[0].attempt, original);
    assert!(!resumed.complete_hardware_matrix);
    assert_eq!(resumed.exact_physical_targets_executed, 0);
    Ok(())
}

#[test]
fn changed_policy_and_limits_have_different_locked_identity() {
    let fixture = fixture(OracleMode::Check);
    let original = fixture.plan.content.runs[0].clone();
    let identity = original.hash().unwrap();
    for (key, value) in [
        ("seed", json!(2)),
        ("hash_cap", json!(100)),
        ("clock", json!("logical")),
    ] {
        let mut changed = original.clone();
        changed.policies[key] = value;
        assert_ne!(changed.hash().unwrap(), identity);
    }
    let mut changed = original.clone();
    changed.process.as_mut().unwrap().limits.cpu_seconds += 1;
    assert_ne!(changed.hash().unwrap(), identity);
    let legacy = serde_json::to_value(original.process.as_ref().unwrap()).unwrap();
    assert!(legacy.get("loopback").is_none());
    let restored: ProcessSpec = serde_json::from_value(legacy.clone()).unwrap();
    assert!(!restored.loopback);
    assert_eq!(serde_json::to_value(restored).unwrap(), legacy);
    let mut changed = original.clone();
    changed.process.as_mut().unwrap().loopback = true;
    assert_ne!(changed.hash().unwrap(), identity);
    let mut changed = original;
    changed.suite.version = "2".into();
    assert_ne!(changed.hash().unwrap(), identity);
}

#[test]
fn cancellation_stops_actual_local_and_worker_processes() -> Result<()> {
    for remote in [false, true] {
        let mut fixture = fixture(OracleMode::Check);
        let mut content = fixture.plan.content.clone();
        let process = content.runs[0].process.as_mut().unwrap();
        process.argv[2] = "printf 'actual isolated queue process\\n'; while :; do :; done".into();
        process.limits.wall_seconds = 10;
        process.limits.cpu_seconds = 10;
        fixture.plan = LockedPlan::new(content)?;
        let run_hash = fixture.plan.content.runs[0].hash()?;
        let mut queue = Queue::create(&fixture.root.join("queue"), fixture.plan, &fixture.source)?;
        let stop = cancel();
        let request = stop.clone();
        let watch_root = fixture.root.clone();
        let timer = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(7);
            let mut observed = false;
            while Instant::now() < deadline {
                let paths = if remote {
                    fs::read_dir(watch_root.join("worker/runs"))
                        .into_iter()
                        .flatten()
                        .filter_map(Result::ok)
                        .map(|e| e.path().join("stdout.log"))
                        .collect::<Vec<_>>()
                } else {
                    vec![watch_root
                        .join("queue/jobs")
                        .join(&run_hash)
                        .join("attempt-000001/observed-stdout.log")]
                };
                if paths
                    .iter()
                    .any(|p| fs::read(p).is_ok_and(|bytes| bytes == EXPECTED))
                {
                    observed = true;
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
            request.store(true, Ordering::Release);
            observed
        });
        let started = Instant::now();
        let report = if remote {
            let mut worker = PrivateWorker::spawn(
                Path::new(env!("CARGO_BIN_EXE_md-sim-worker-ipc")),
                &fixture.root.join("worker"),
            )?;
            queue.run(&mut worker.executor, &fixture.oracle, false, stop)?
        } else {
            queue.run(&mut ObservedLocal, &fixture.oracle, false, stop)?
        };
        assert!(
            timer.join().unwrap(),
            "actual process never emitted its start marker"
        );
        assert!(started.elapsed() < Duration::from_secs(8));
        assert_eq!(report.rows[0].status, "cancelled");
        assert!(!report.declared_suite_complete);
        let process: ProcessResult = serde_json::from_slice(&fs::read(
            Path::new(&report.rows[0].attempt).join("process-result.json"),
        )?)?;
        // This output proves the child ran before cancellation. A cancelled
        // queue row without actual process execution does not satisfy this test.
        assert_eq!(hex::decode(&process.stdout)?, EXPECTED);
        assert_eq!(process.status, "cancelled");
    }
    Ok(())
}

#[test]
fn exhausted_queue_budget_rejects_before_creating_a_new_attempt() -> Result<()> {
    for byte_budget in [false, true] {
        let fixture = fixture(OracleMode::Check);
        let mut content = fixture.plan.content.clone();
        content.limits.max_attempts = 1;
        if byte_budget {
            content.limits.max_evidence_bytes = 1;
        }
        let plan = LockedPlan::new(content)?;
        let run_hash = plan.content.runs[0].hash()?;
        let root = fixture.root.join("queue");
        let mut queue = Queue::create(&root, plan, &fixture.source)?;
        let retained = if byte_budget {
            None
        } else {
            let first = queue.run(&mut LocalExecutor, &fixture.oracle, false, cancel())?;
            assert!(first.declared_suite_complete);
            Some((first.rows[0].attempt.clone(), first.rows[0].result.clone()))
        };
        let error = queue
            .run(&mut LocalExecutor, &fixture.oracle, false, cancel())
            .unwrap_err();
        assert!(error.to_string().contains("budget exhausted"));
        assert_eq!(
            fs::read_dir(root.join("jobs").join(run_hash))?.count(),
            usize::from(!byte_budget)
        );
        if let Some((attempt, identity)) = retained {
            assert_eq!(
                hash_file(&Path::new(&attempt).join("result.json"))?,
                identity
            );
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum LostReply {
    Execute,
    Cleanup,
    ResultChunk,
    WrongCleanup,
}

#[test]
fn unsupported_worker_is_rejected_before_transfer_or_execution() -> Result<()> {
    struct WithoutLiveStatus<'a, T>(&'a mut T, usize);
    impl<T: WorkerTransport> WorkerTransport for WithoutLiveStatus<'_, T> {
        fn request(&mut self, request: &Request) -> Result<serde_json::Value> {
            self.1 += 1;
            ensure!(
                matches!(request, Request::Probe),
                "preflight issued a mutating operation"
            );
            let mut response = self.0.request(request)?;
            response.as_object_mut().unwrap().remove("capabilities");
            Ok(response)
        }
        fn description(&self) -> String {
            self.0.description()
        }
    }
    let fixture = fixture(OracleMode::Check);
    let mut worker = PrivateWorker::spawn(
        Path::new(env!("CARGO_BIN_EXE_md-sim-worker-ipc")),
        &fixture.root.join("worker"),
    )?;
    let out = fixture.root.join("unsupported-worker");
    fs::create_dir(&out)?;
    let mut executor = WorkerExecutor {
        transport: WithoutLiveStatus(&mut worker.executor.transport, 0),
    };
    let error = executor
        .run(
            &fixture.source,
            &fixture.plan.content.runs[0],
            cancel(),
            &out,
        )
        .unwrap_err();
    assert!(error.to_string().contains("live-log status support"));
    assert_eq!(executor.transport.1, 1);
    assert_eq!(fs::read_dir(out)?.count(), 0);
    Ok(())
}
struct LoseReply<'a, T> {
    transport: &'a mut T,
    once: Option<LostReply>,
}
impl<T: WorkerTransport> WorkerTransport for LoseReply<'_, T> {
    fn request(&mut self, request: &Request) -> Result<serde_json::Value> {
        let mut actual = self.transport.request(request)?;
        if matches!(
            (&self.once, request),
            (Some(LostReply::WrongCleanup), Request::Cleanup { .. })
        ) {
            self.once = None;
            actual["acknowledged_result"]["sha256"] = json!("00".repeat(32));
            return Ok(actual);
        }
        let lose = matches!(
            (&self.once, request),
            (Some(LostReply::Execute), Request::ExecuteToken { .. })
                | (Some(LostReply::Cleanup), Request::Cleanup { .. })
                | (Some(LostReply::ResultChunk), Request::ResultData { .. })
        );
        if lose {
            self.once = None;
            anyhow::bail!("test discarded a real worker reply after the operation");
        }
        Ok(actual)
    }
    fn description(&self) -> String {
        self.transport.description()
    }
}

#[test]
fn lost_real_worker_replies_recover_tokens_cleanup_and_partial_history() -> Result<()> {
    for fault in [
        LostReply::Execute,
        LostReply::Cleanup,
        LostReply::ResultChunk,
        LostReply::WrongCleanup,
    ] {
        let fixture = fixture(OracleMode::Check);
        let worker_root = fixture.root.join("worker");
        let mut worker = PrivateWorker::spawn(
            Path::new(env!("CARGO_BIN_EXE_md-sim-worker-ipc")),
            &worker_root,
        )?;
        let attempt = fixture.root.join("interrupted-attempt");
        fs::create_dir(&attempt)?;
        let error = {
            let mut executor = WorkerExecutor {
                transport: LoseReply {
                    transport: &mut worker.executor.transport,
                    once: Some(fault),
                },
            };
            executor
                .run(
                    &fixture.source,
                    &fixture.plan.content.runs[0],
                    cancel(),
                    &attempt,
                )
                .unwrap_err()
        };
        if matches!(fault, LostReply::WrongCleanup) {
            assert!(error
                .to_string()
                .contains("cleanup acknowledgement differs"));
            assert!(!attempt.join("worker-cleanup.json").exists());
            assert!(attempt.join("worker-cleanup-response.json").exists());
        } else {
            assert!(error.to_string().contains("discarded a real worker reply"));
        }
        assert!(attempt.join("worker-request.json").is_file());
        if matches!(fault, LostReply::Execute) {
            assert!(!attempt.join("worker-run.json").exists());
        }
        // A failed exchange can close the old transport. Recovery uses a fresh
        // actual worker channel and the retained root, never the uncertain wire.
        drop(worker);
        let mut worker = PrivateWorker::spawn(
            Path::new(env!("CARGO_BIN_EXE_md-sim-worker-ipc")),
            &worker_root,
        )?;
        if matches!(fault, LostReply::ResultChunk) {
            // A second interrupted retrieval must retain the first fragment.
            {
                let mut executor = WorkerExecutor {
                    transport: LoseReply {
                        transport: &mut worker.executor.transport,
                        once: Some(LostReply::ResultChunk),
                    },
                };
                assert!(executor.recover(&attempt).is_err());
            }
            assert!(attempt.join("worker-result-loss-00.json").is_file());
            assert!(!attempt.join("worker-result-loss-01.json").exists());
            drop(worker);
            worker = PrivateWorker::spawn(
                Path::new(env!("CARGO_BIN_EXE_md-sim-worker-ipc")),
                &worker_root,
            )?;
        }
        worker.executor.recover(&attempt)?;
        assert!(attempt.join("worker-cleanup.json").is_file());
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(attempt.join("worker-request.json"))?)?;
        let token = saved["token"].as_str().unwrap();
        let lookup = worker.executor.transport.request(&Request::LookupToken {
            token: token.into(),
        })?;
        assert_eq!(lookup["state"], "cleaned");
        let acknowledged: ObjectId = serde_json::from_value(lookup["acknowledged_result"].clone())?;
        assert_eq!(
            hash_file(&attempt.join("worker-result.json"))?,
            acknowledged
        );
        if matches!(fault, LostReply::ResultChunk) {
            assert!(attempt.join("worker-result-loss-01.json").is_file());
        }
    }
    Ok(())
}

#[test]
fn external_retention_requires_exact_ack_and_recovers_over_a_fresh_worker_channel() -> Result<()> {
    let fixture = fixture(OracleMode::Check);
    let worker_root = fixture.root.join("worker");
    let program = Path::new(env!("CARGO_BIN_EXE_md-sim-worker-ipc"));
    let mut worker = PrivateWorker::spawn(program, &worker_root)?;
    let attempt = fixture.root.join("retention-attempt");
    fs::create_dir(&attempt)?;
    let run = &fixture.plan.content.runs[0];
    let mut observed = None;
    let error = worker
        .executor
        .run_with_retention(
            &fixture.source,
            run,
            cancel(),
            &attempt,
            &mut |request: &RetentionRequest| {
                assert_eq!(request.identity.locked_run_hash, run.hash()?);
                assert_eq!(hash_file(&request.result_path)?, request.identity.result);
                let process: ProcessResult =
                    serde_json::from_slice(&fs::read(&request.result_path)?)?;
                assert_eq!(process.status, "exited");
                assert_eq!(process.exit_code, Some(0));
                assert_eq!(hex::decode(process.stdout)?, EXPECTED);
                observed = Some(request.identity.clone());
                anyhow::bail!("test withheld durable retention acknowledgment")
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("withheld durable retention"));
    let expected: RetentionIdentity =
        observed.expect("actual completed result reached the barrier");
    let remote_run = worker_root.join("runs").join(&expected.run_id);
    let original_bytes = fs::read(remote_run.join("result.json"))?;
    assert_eq!(
        fs::read(attempt.join("worker-result.json"))?,
        original_bytes
    );
    assert!(!remote_run.join("cleanup.json").exists());
    assert!(!attempt.join("worker-cleanup.json").exists());
    assert!(!attempt.join("worker-retention.json").exists());
    let error = worker.executor.recover(&attempt).unwrap_err();
    assert!(error.to_string().contains("requires external retention"));
    assert_eq!(fs::read(remote_run.join("result.json"))?, original_bytes);
    assert!(!remote_run.join("cleanup.json").exists());

    // Old attempts remain available to the original local recovery contract,
    // but cannot authorize an external retention acknowledgment without a lock.
    let request_path = attempt.join("worker-request.json");
    let original_request = fs::read(&request_path)?;
    let mut legacy: serde_json::Value = serde_json::from_slice(&original_request)?;
    legacy.as_object_mut().unwrap().remove("locked_run_hash");
    isolation::save_json(&request_path, &legacy)?;
    let mut called = false;
    let error = worker
        .executor
        .recover_with_retention(&attempt, &mut |request| {
            called = true;
            Ok(request.identity.clone())
        })
        .unwrap_err();
    assert!(error.to_string().contains("saved locked-run hash"));
    assert!(!called);
    fs::write(&request_path, original_request)?;
    fs::File::open(&request_path)?.sync_all()?;

    // Each identity field is required. A callback that returns success with a
    // mismatched acknowledgment must not remove the actual worker result.
    for field in ["lock", "token", "run", "hash", "size"] {
        let error = worker
            .executor
            .recover_with_retention(&attempt, &mut |request| {
                assert_eq!(request.identity, expected);
                assert_eq!(hash_file(&request.result_path)?, expected.result);
                let mut wrong = request.identity.clone();
                match field {
                    "lock" => wrong.locked_run_hash.push('0'),
                    "token" => wrong.token.push('0'),
                    "run" => wrong.run_id.push('0'),
                    "hash" => wrong.result.sha256.push('0'),
                    "size" => wrong.result.size += 1,
                    _ => unreachable!(),
                }
                Ok(wrong)
            })
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("retention acknowledgment differs"));
        assert_eq!(fs::read(remote_run.join("result.json"))?, original_bytes);
        assert!(!remote_run.join("cleanup.json").exists());
        assert!(!attempt.join("worker-retention.json").exists());
    }

    drop(worker);
    let mut worker = PrivateWorker::spawn(program, &worker_root)?;
    let durable = fixture.root.join("durable-receiver");
    fs::create_dir(&durable)?;
    let mut acknowledged = false;
    worker
        .executor
        .recover_with_retention(&attempt, &mut |request| {
            assert_eq!(request.identity, expected);
            assert!(!remote_run.join("cleanup.json").exists());
            assert!(!attempt.join("worker-cleanup.json").exists());
            let copy = durable.join("worker-result.json");
            fs::copy(&request.result_path, &copy)?;
            assert_eq!(hash_file(&copy)?, expected.result);
            fs::File::open(&copy)?.sync_all()?;
            isolation::save_json(&durable.join("receipt.json"), &request.identity)?;
            fs::File::open(&durable)?.sync_all()?;
            acknowledged = true;
            Ok(request.identity.clone())
        })?;
    assert!(acknowledged);
    assert_eq!(
        fs::read(durable.join("worker-result.json"))?,
        original_bytes
    );
    assert!(!remote_run.join("result.json").exists());
    assert!(remote_run.join("cleanup.json").is_file());
    let receipt: RetentionIdentity =
        serde_json::from_slice(&fs::read(attempt.join("worker-retention.json"))?)?;
    assert_eq!(receipt, expected);
    let lookup = worker.executor.transport.request(&Request::LookupToken {
        token: expected.token.clone(),
    })?;
    assert_eq!(lookup["state"], "cleaned");
    assert_eq!(
        lookup["acknowledged_result"],
        serde_json::to_value(&expected.result)?
    );

    // Neither the local cleanup fast path nor the worker's retained cleanup
    // receipt can bypass a requested external barrier on a later recovery.
    for lost_local_ack in [false, true] {
        if lost_local_ack {
            fs::rename(
                attempt.join("worker-cleanup.json"),
                attempt.join("worker-cleanup-before-client-loss.json"),
            )?;
        }
        let mut calls = 0;
        worker
            .executor
            .recover_with_retention(&attempt, &mut |request| {
                calls += 1;
                assert_eq!(request.identity, expected);
                assert_eq!(fs::read(&request.result_path)?, original_bytes);
                assert_eq!(
                    hash_file(&durable.join("worker-result.json"))?,
                    expected.result
                );
                Ok(request.identity.clone())
            })?;
        assert_eq!(calls, 1);
        assert!(attempt.join("worker-cleanup.json").is_file());
    }
    Ok(())
}

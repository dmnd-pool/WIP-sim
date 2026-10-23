//! Exercise coordinator log retrieval through actual OpenSSH, before stopping
//! an actual isolated process. This fixture establishes no firmware gate.
use super::*;
use md_sim_coordinator::{ExecutionScope, Executor, LockedRun, SuiteIdentity, WorkerExecutor};
use std::{collections::BTreeSet, sync::atomic::Ordering};

fn reconnect(out: &Path) -> Result<Value> {
    let worker: Value = serde_json::from_slice(&fs::read(out.join("worker-run.json"))?)?;
    let run_id = worker["run_id"]
        .as_str()
        .context("live worker run absent")?;
    let mut config = config();
    config.remote_relay = "/check".into();
    let mut transport = SshTransport::new_streaming(config)?;
    let mut reads = Vec::new();
    for request in [
        Request::Status {
            run_id: run_id.into(),
        },
        Request::Logs {
            run_id: run_id.into(),
            stream: LogStream::Stdout,
            offset: 0,
            count: 4096,
        },
        Request::Logs {
            run_id: run_id.into(),
            stream: LogStream::Stderr,
            offset: 0,
            count: 4096,
        },
        Request::Status {
            run_id: run_id.into(),
        },
    ] {
        let response = transport.request(&request)?;
        reads.push(json!({"request":request,"response":response}));
    }
    let proof = json!({"run_id":run_id,"reads":reads,"reconnects":transport.reconnect_count(),
        "forced_server_channel_close_after_each_reply":true,"transport":transport.evidence()});
    isolation::save_json(&out.join("live-reconnect.json"), &proof)?;
    dashboard_transport_report::verify_live_reconnect(&proof, run_id)?;
    Ok(proof)
}

pub(super) fn check(store: &Store, baseline: &ProcessSpec, events: &mut Vec<Value>) -> Result<()> {
    let out = Path::new("/work/coordinator-live");
    fs::create_dir(out)?;
    let mut executor = WorkerExecutor {
        transport: SshTransport::new_streaming(config())?,
    };
    let identity = executor.identity()?;
    let mut spec = baseline.clone();
    spec.argv[2] = "printf 'coordinator live stdout\\n'; printf 'coordinator live stderr\\n' >&2; while :; do :; done".into();
    spec.limits.cpu_seconds = 15;
    spec.limits.wall_seconds = 20;
    let suite = hash_file(Path::new("/fixture-inputs.json"))?;
    store.import(Path::new("/fixture-inputs.json"), &suite)?;
    let program = std::env::current_exe()?;
    let verifier = hash_file(&program)?;
    store.import(&program, &verifier)?;
    let mut inputs: BTreeMap<_, _> = spec
        .inputs
        .iter()
        .map(|input| (input.guest_path.clone(), input.object.clone()))
        .collect();
    inputs.insert("suite".into(), suite.clone());
    inputs.insert("verifier".into(), verifier.clone());
    let run = LockedRun {
        name: "live-coordinator-ssh-fixture".into(),
        scope: ExecutionScope::ProcessPrimitive {
            description:
                "Actual isolated shell output through the SSH coordinator; no firmware gate".into(),
        },
        process: Some(spec),
        suite: SuiteIdentity {
            id: "ssh-coordinator-live-output".into(),
            version: "2".into(),
            suite,
            verifier,
            required_gates: BTreeSet::from([
                "live-reconnect-before-stop-and-final-byte-match".into()
            ]),
        },
        inputs,
        backend_tools: identity.tools.clone(),
        policies: json!({"output":"live prefix must match full verified result", "stop":"only after both actual streams are observed"}),
        capability_limits: vec![
            "Protocol fixture only; no firmware or native dashboard gate".into(),
        ],
    };
    let run_hash = run.hash()?;
    isolation::save_json(&out.join("locked-run.json"), &run)?;
    let stop = Arc::new(AtomicBool::new(false));
    let (observed, reconnect, result) = thread::scope(|scope| {
        let execution = scope.spawn(|| executor.run(store, &run, stop.clone(), out));
        let setup_deadline = Instant::now() + Duration::from_secs(30);
        let mut live_deadline = None;
        let observed = loop {
            if live_deadline.is_none() && out.join("worker-stdout.log").exists() {
                live_deadline = Some(Instant::now() + Duration::from_secs(10));
            }
            if fs::read(out.join("worker-stdout.log")).ok().as_deref()
                == Some(b"coordinator live stdout\n")
                && fs::read(out.join("worker-stderr.log")).ok().as_deref()
                    == Some(b"coordinator live stderr\n")
            {
                break true;
            }
            if Instant::now() >= live_deadline.unwrap_or(setup_deadline) || execution.is_finished()
            {
                break false;
            }
            thread::sleep(Duration::from_millis(20));
        };
        let reconnect = if observed {
            reconnect(out)
        } else {
            Err(anyhow::anyhow!("live logs absent before reconnect"))
        };
        stop.store(true, Ordering::Release);
        (
            observed,
            reconnect,
            execution
                .join()
                .map_err(|_| anyhow::anyhow!("coordinator test panicked")),
        )
    });
    let result = result??;
    let reconnect = reconnect?;
    ensure!(
        observed && result.status == "cancelled" && !result.trace_truncated,
        "coordinator did not observe live streams before actual cancellation"
    );
    ensure!(
        hex::decode(&result.stdout)? == fs::read(out.join("worker-stdout.log"))?
            && hex::decode(&result.stderr)? == fs::read(out.join("worker-stderr.log"))?,
        "live and verified final bytes differ"
    );
    let worker_run: Value = serde_json::from_slice(&fs::read(out.join("worker-run.json"))?)?;
    let cleanup: Value = serde_json::from_slice(&fs::read(out.join("worker-cleanup.json"))?)?;
    events.push(json!({"actual_coordinator_live_ssh":{"locked_run":run,"locked_run_hash":run_hash,"worker_run":worker_run,"executor":identity,"observed_both_streams_before_stop":observed,"live_reconnect":reconnect,"final_process":result,"verified_result":hash_file(&out.join("worker-result.json"))?,"cleanup":cleanup,"firmware_gate":false}}));
    Ok(())
}

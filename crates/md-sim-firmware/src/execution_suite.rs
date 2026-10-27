//! A real archived-source execution suite for the documented S2 controller
//! interface fixture. This is not an exact physical-board target or matrix pass.
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::{
    queue::{read_json, EvidenceWriter},
    *,
};
use md_sim_mining::{stratum::PoolReport, PoolVerifier, Verdict};
use md_sim_runtime::{
    isolation::{ProcessResult, ProcessSpec},
    store::{hash_file, ObjectId, Store, MAX_CHUNK},
};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
const SUITE: &[u8] = include_bytes!("execution_suite.rs");
const FIXTURE:&[u8]=br#"{"schema":1,"type":"source-interface-fixture","chains":8,"chips_per_chain":32,"interface":"cgminer S2 controller serial protocol","exact_physical_board":false}"#;
const GATES: &[&str] = &[
    "G0.locked-inputs",
    "G1.source-execution",
    "G2.controller-interface",
    "G3.fresh-work",
    "G3.verifier-rejection",
];
fn bytes(store: &Store, data: &[u8]) -> Result<ObjectId> {
    let id = ObjectId {
        sha256: crate::prepare::hash_bytes(data),
        size: data.len() as u64,
    };
    let mut offset = store.begin(&id)?;
    while offset < id.size {
        let end = (offset as usize + MAX_CHUNK).min(data.len());
        offset = store.chunk(&id, offset, &data[offset as usize..end])?;
    }
    store.finish(&id)?;
    Ok(id)
}
fn import(store: &Store, path: &Path) -> Result<ObjectId> {
    let id = hash_file(path)?;
    store.import(path, &id)?;
    Ok(id)
}
pub struct SourceSuite {
    identity: SuiteIdentity,
    harness: ObjectId,
}
impl SourceSuite {
    pub fn current() -> Result<Self> {
        let executable = std::env::current_exe()?;
        let harness = hash_file(
            &executable
                .parent()
                .context("missing binary directory")?
                .join("cgminer-fresh-work"),
        )?;
        Ok(Self {
            harness,
            identity: SuiteIdentity {
                id: "cgminer-source-controller-fresh-work".into(),
                version: "2.0".into(),
                suite: ObjectId {
                    sha256: crate::prepare::hash_bytes(SUITE),
                    size: SUITE.len() as u64,
                },
                verifier: hash_file(&std::env::current_exe()?)?,
                required_gates: GATES.iter().map(|s| s.to_string()).collect(),
            },
        })
    }
}
/// Freeze the actual prepared firmware, its captured build closure and every
/// runtime input. All paths name existing evidence; no acquired program runs here.
pub fn create_plan(
    prepared: &Path,
    snapshot: &Path,
    source: &Path,
    recipe: &Path,
    output: &Path,
    options: crate::lab::SourceRunOptions,
) -> Result<Queue> {
    ensure!(!output.exists(), "queue output must be new");
    let stage = output.with_extension("inputs");
    crate::lab::lock_recipe(prepared, &stage, options)?;
    // The lab store has a 64 MiB per-object policy. Use a separate transport
    // store for the complete captured toolchain, which is one bounded archive.
    let source_store = Store::open(&stage.join("input-store"), 64 * 1024 * 1024)?;
    let store = Store::open(&stage.join("transport-store"), 2 * 1024 * 1024 * 1024)?;
    let spec: ProcessSpec = read_json(&stage.join("run-spec.json"), 16 * 1024 * 1024)?;
    let mut inputs = BTreeMap::new();
    for mount in &spec.inputs {
        store.import(&source_store.verified_path(&mount.object)?, &mount.object)?;
        inputs.insert(
            format!("runtime:{}", mount.guest_path),
            mount.object.clone(),
        );
    }
    let prep: Value = read_json(&prepared.join("preparation-inputs.json"), 1024 * 1024)?;
    let manifest: Value = read_json(&prepared.join("input-manifest.json"), 1024 * 1024)?;
    let status: Value = read_json(&prepared.join("preparation-status.json"), 65536)?;
    ensure!(
        status["code"] == 0 && status["evidence_complete"] == true,
        "preparation did not pass"
    );
    ensure!(
        manifest["c_regression"]["passed"] == true && manifest["toolchain_limit"].is_null(),
        "preparation lacks closed inputs or C regression"
    );
    ensure!(
        prep["source"]["sha256"] == manifest["source_sha256"]
            && prep["source"]["sha256"] == crate::prepare::SOURCE_SHA256,
        "preparation source records disagree"
    );
    ensure!(
        hash_file(&prepared.join("preparation-stderr.log"))?.sha256 == status["stderr"],
        "preparation stderr identity differs"
    );
    // Restore the preparation envelope before its saved closure annotation.
    // Its captured stdout hash binds the original build log and C output bytes.
    let mut original_manifest = manifest.clone();
    let object = original_manifest
        .as_object_mut()
        .context("preparation manifest must be an object")?;
    for name in [
        "toolchain_closure",
        "toolchain_scope",
        "matches_v10_firmware_bytes",
    ] {
        object.remove(name);
    }
    object.insert("toolchain_limit".into(),json!("Read-only installed /usr supplies headers and build dependencies. A frozen toolchain image is still required for a reproducible release."));
    let mut envelope = serde_json::to_vec(
        &json!({"binary_hex":hex::encode(fs::read(prepared.join("cgminer"))?),"build_log":fs::read_to_string(prepared.join("build.log"))?,"manifest":original_manifest}),
    )?;
    envelope.push(b'\n');
    ensure!(
        crate::prepare::hash_bytes(&envelope) == status["stdout"],
        "preparation export identity differs"
    );
    for (name, path, key) in [
        ("source-archive", source, "source"),
        ("preparation-runner", recipe, "recipe_build"),
    ] {
        let expected: ObjectId = serde_json::from_value(prep[key].clone())?;
        ensure!(
            hash_file(path)? == expected,
            "preparation {name} identity differs"
        );
        inputs.insert(name.into(), import(&store, path)?);
    }
    let closure: ObjectId = serde_json::from_value(prep["toolchain_closure"].clone())?;
    ensure!(
        serde_json::from_value::<ObjectId>(manifest["toolchain_closure"].clone())? == closure,
        "closure records disagree"
    );
    let bundle = stage.join("toolchain-inputs.tar");
    let bundle_id = crate::closure_bundle::pack(snapshot, &closure, &bundle)?;
    store.import(&bundle, &bundle_id)?;
    inputs.insert("toolchain-bundle".into(), bundle_id.clone());
    inputs.insert(
        "toolchain-manifest".into(),
        import(&store, &snapshot.join("manifest.json"))?,
    );
    for name in [
        "input-manifest.json",
        "preparation-inputs.json",
        "preparation-status.json",
        "build.log",
        "preparation-stderr.log",
        "closure-manifest.json",
        "adaptation.patch",
        "adaptation-manifest.json",
    ] {
        inputs.insert(
            format!("prepared:{name}"),
            import(&store, &prepared.join(name))?,
        );
    }
    for name in [
        "run-spec.json",
        "input-manifest.json",
        "adaptation-manifest.json",
        "execution-identity.json",
    ] {
        inputs.insert(format!("lab:{name}"), import(&store, &stage.join(name))?);
    }
    let fixture = bytes(&store, FIXTURE)?;
    inputs.insert("fixture".into(), fixture.clone());
    let suite = SourceSuite::current()?;
    inputs.insert("suite-source".into(), bytes(&store, SUITE)?);
    inputs.insert(
        "verifier-build".into(),
        import(&store, &std::env::current_exe()?)?,
    );
    for (name, data) in [
        ("model-source", include_bytes!("bitmain_s2.rs").as_slice()),
        (
            "harness-source",
            include_bytes!("bin/cgminer-fresh-work.rs").as_slice(),
        ),
        (
            "verifier-source",
            include_bytes!("../../md-sim-mining/src/pool.rs").as_slice(),
        ),
        (
            "Cargo.lock",
            include_bytes!("../../../Cargo.lock").as_slice(),
        ),
    ] {
        inputs.insert(name.into(), bytes(&store, data)?);
    }
    let run=LockedRun{name:"cgminer-source-interface".into(),scope:ExecutionScope::SourceInterfaceFixture{fixture,firmware_identity:"rebuilt-source".into()},process:Some(spec),suite:suite.suite(),inputs,backend_tools:md_sim_coordinator::executor::local_tools()?,policies:json!({"hashing":"functional SHA256d","transport":"private connected IPC plus PTY","preparation":"exact captured toolchain bytes transferred; build evidence retained","scratch":"fresh isolated tmpfs for every attempt","resume":"complete locked execution, suite, verifier, inputs and executor identity"}),capability_limits:vec!["Source-derived controller interface fixture; no exact physical board target.".into(),"No original-board boot, calibrated physics, native dashboard, SSH, TLS, or external physical worker claim.".into(),"Regtest block export is a candidate; this suite does not run Bitcoin Core.".into()]};
    let queue = Queue::create(
        output,
        LockedPlan::new(PlanContent {
            schema_version: "2.0".into(),
            catalogue_snapshot: None,
            limits: QueueLimits::default(),
            runs: vec![run],
        })?,
        &store,
    )?;
    let retained = queue.input_store().verified_path(&bundle_id)?;
    ensure!(
        fs::symlink_metadata(&bundle)?.is_file() && hash_file(&bundle)? == bundle_id,
        "temporary bundle changed before cleanup"
    );
    md_sim_runtime::isolation::save_json(
        &stage.join("bundle-cleanup.json"),
        &json!({"removed_duplicate":bundle,"object":bundle_id,"retained_verified_object":retained,"reason":"The complete input archive is retained in the locked queue content store."}),
    )?;
    fs::remove_file(bundle)?;
    fs::File::open(&stage)?.sync_all()?;
    Ok(queue)
}
fn validate_recipe(spec: &ProcessSpec) -> Result<()> {
    ensure!(
        spec.argv.len() == 9
            && spec.argv[0] == "/harness"
            && spec.argv[1] == "/firmware"
            && spec.argv[2] == "/work/results"
            && spec.argv[8] == "--export"
            && ["random-parent", "regtest-genesis"].contains(&spec.argv[7].as_str()),
        "process does not match source suite recipe"
    );
    Ok(())
}
fn submission_params(share: &md_sim_mining::Share) -> Value {
    json!([
        "simulator",
        share.job_id,
        hex::encode(&share.extra_nonce2),
        format!("{:08x}", share.time),
        format!("{:08x}", share.nonce)
    ])
}
fn wire_difficulty_target(value: &Value) -> Result<md_sim_mining::Target> {
    use md_sim_mining::stratum::{difficulty_target_scaled, DifficultyScale};
    let difficulty = value.as_f64().context("wire difficulty is not a number")?;
    // The declared source suite supports these exact binary-representable
    // difficulties. Reject other values rather than round or guess a target.
    for exponent in -31..=32 {
        for (factor, scale) in [
            (1.0, DifficultyScale::One),
            (1.5, DifficultyScale::ThreeHalves),
        ] {
            if difficulty.to_bits() == (factor * 2f64.powi(exponent)).to_bits() {
                return difficulty_target_scaled(exponent, scale).map_err(anyhow::Error::msg);
            }
        }
    }
    anyhow::bail!("wire difficulty is outside the declared exact supported values")
}
fn notify_params(job: &md_sim_mining::PoolJob, clean: bool) -> Value {
    let mut previous = job.previous;
    for word in previous.chunks_exact_mut(4) {
        word.reverse();
    }
    json!([
        job.id,
        hex::encode(previous),
        hex::encode(&job.coinbase_prefix),
        hex::encode(&job.coinbase_suffix),
        job.merkle_branch
            .iter()
            .map(hex::encode)
            .collect::<Vec<_>>(),
        format!("{:08x}", job.version),
        format!("{:08x}", job.bits),
        format!("{:08x}", job.time_min),
        clean
    ])
}
fn ordered_pool_verdicts(
    report: &PoolReport,
    messages: &[Value],
    max_elapsed_ns: u64,
) -> Result<Vec<Verdict>> {
    let mut verifier = PoolVerifier::default();
    let mut checked = Vec::new();
    let mut previous = 0u64;
    let mut subscribe_id = None;
    let mut subscription = None;
    let mut target = None;
    let mut notified = BTreeSet::new();
    for message in messages {
        let now = message["elapsed_ns"]
            .as_str()
            .context("pool timestamp absent")?
            .parse::<u64>()?;
        ensure!(
            now >= previous && now <= max_elapsed_ns,
            "pool trace timestamps are outside the ordered execution budget"
        );
        previous = now;
        let packet = &message["message"];
        let direction = message["direction"]
            .as_str()
            .context("pool trace direction absent")?;
        ensure!(
            ["pool-to-firmware", "firmware-to-pool"].contains(&direction),
            "unknown pool trace direction"
        );
        if direction == "firmware-to-pool" && packet["method"] == "mining.subscribe" {
            ensure!(
                subscribe_id.is_none() && subscription.is_none() && !packet["id"].is_null(),
                "unsupported repeated or uncorrelated subscription"
            );
            subscribe_id = Some(packet["id"].clone());
        } else if direction == "pool-to-firmware"
            && subscribe_id.as_ref().is_some_and(|id| *id == packet["id"])
        {
            ensure!(
                packet["error"].is_null()
                    && packet["result"].as_array().is_some_and(|r| r.len() == 3),
                "subscription failed or malformed"
            );
            let extra1 = hex::decode(
                packet["result"][1]
                    .as_str()
                    .context("subscription extra nonce absent")?,
            )?;
            let extra2 = usize::try_from(
                packet["result"][2]
                    .as_u64()
                    .context("extra nonce size absent")?,
            )?;
            ensure!(
                extra1.len() <= 32 && extra2 <= 32,
                "subscription exceeds declared bounds"
            );
            subscription = Some((extra1, extra2));
            subscribe_id = None;
        } else if direction == "pool-to-firmware" && packet["method"] == "mining.set_difficulty" {
            ensure!(
                packet["params"].as_array().is_some_and(|p| p.len() == 1),
                "invalid difficulty packet"
            );
            target = Some(wire_difficulty_target(&packet["params"][0])?);
        } else if direction == "pool-to-firmware" && packet["method"] == "mining.notify" {
            let job = report
                .jobs
                .iter()
                .find(|job| packet["params"][0] == job.id)
                .context("notified job absent from report")?;
            let clean = packet["params"][8]
                .as_bool()
                .context("clean-job flag absent")?;
            let (extra1, extra2) = subscription
                .as_ref()
                .context("work precedes subscription response")?;
            ensure!(
                job.extra_nonce1 == *extra1 && job.extra_nonce2_size == *extra2,
                "report extra nonce differs from subscription"
            );
            ensure!(
                Some(job.share_target) == target,
                "report target differs from preceding wire difficulty"
            );
            ensure!(
                packet["params"] == notify_params(job, clean),
                "report job differs from complete wire notification"
            );
            ensure!(notified.insert(job.id.clone()), "job notification repeated");
            verifier
                .install(job.clone(), clean)
                .map_err(anyhow::Error::msg)?;
        } else if direction == "firmware-to-pool" && packet["method"] == "mining.submit" {
            let submission = report
                .submissions
                .get(checked.len())
                .context("firmware submission omitted from report")?;
            ensure!(
                packet["params"] == submission_params(&submission.share),
                "report does not preserve original submission order or parameters"
            );
            let verdict = verifier
                .verify(&submission.share)
                .map_err(anyhow::Error::msg)?;
            ensure!(
                verdict == submission.verdict,
                "independent ordered reconstruction disagrees with saved verdict"
            );
            checked.push(verdict);
        }
    }
    ensure!(
        checked.len() == report.submissions.len(),
        "report contains an untraced submission"
    );
    ensure!(
        notified.len() == report.jobs.len() && subscription.is_some() && subscribe_id.is_none(),
        "report contains untraced jobs or incomplete subscription"
    );
    Ok(checked)
}
fn gate(
    id: &str,
    ok: bool,
    actual: Value,
    expected: Value,
    names: &[&str],
    out: &EvidenceWriter,
) -> Result<GateResult> {
    Ok(GateResult {
        id: id.into(),
        status: if ok {
            GateStatus::Pass
        } else {
            GateStatus::Fail
        },
        evidence: names
            .iter()
            .map(|s| out.reference(s))
            .collect::<Result<BTreeSet<_>>>()?,
        actual,
        expected,
        tolerance: json!(0),
        clock_domain: "wall-monotonic; hashing work count separate from ordinary interface timing"
            .into(),
        test_version: "2.0".into(),
    })
}
impl SuiteVerifier for SourceSuite {
    fn suite(&self) -> SuiteIdentity {
        self.identity.clone()
    }
    fn verify(
        &self,
        store: &Store,
        run: &LockedRun,
        process: &ProcessResult,
        out: &mut EvidenceWriter,
    ) -> Result<Vec<GateResult>> {
        ensure!(run.suite == self.identity, "source suite identity differs");
        let expected_fixture = ObjectId {
            sha256: crate::prepare::hash_bytes(FIXTURE),
            size: FIXTURE.len() as u64,
        };
        ensure!(
            matches!(&run.scope,ExecutionScope::SourceInterfaceFixture{fixture,firmware_identity} if *fixture==expected_fixture && firmware_identity=="rebuilt-source"),
            "source suite is limited to its exact interface fixture identity"
        );
        for (key, name) in [
            ("lab:run-spec.json", "run-spec.json"),
            ("lab:input-manifest.json", "input-manifest.json"),
            ("lab:adaptation-manifest.json", "adaptation-manifest.json"),
            ("lab:execution-identity.json", "execution-identity.json"),
            (
                "prepared:preparation-inputs.json",
                "preparation-inputs.json",
            ),
            (
                "prepared:preparation-status.json",
                "preparation-status.json",
            ),
        ] {
            out.copy(
                name,
                store,
                run.inputs
                    .get(key)
                    .context("locked preparation evidence missing")?,
            )?;
        }
        let files = crate::lab::exported_evidence(process)?;
        for (name, data) in &files {
            out.write(name, data)?;
        }
        let session: Value = serde_json::from_slice(
            files
                .get("session.json")
                .context("firmware session absent")?,
        )?;
        let report: PoolReport = serde_json::from_slice(
            files
                .get("pool-report.json")
                .context("pool report absent")?,
        )?;
        let spec = run.process.as_ref().context("source process absent")?;
        validate_recipe(spec)?;
        let input = |key: &str| run.inputs.get(key).context("required input absent");
        let saved_spec: ProcessSpec = read_json(
            &store.verified_path(input("lab:run-spec.json")?)?,
            16 * 1024 * 1024,
        )?;
        ensure!(
            md_sim_coordinator::identity(&saved_spec)? == md_sim_coordinator::identity(spec)?,
            "saved recipe differs from executed specification"
        );
        let manifest: Value = read_json(
            &store.verified_path(input("lab:input-manifest.json")?)?,
            1024 * 1024,
        )?;
        let firmware = spec
            .inputs
            .iter()
            .find(|m| m.guest_path == "/firmware")
            .context("firmware mount absent")?;
        let harness = spec
            .inputs
            .iter()
            .find(|m| m.guest_path == "/harness")
            .context("harness mount absent")?;
        ensure!(
            harness.object == self.harness
                && serde_json::from_value::<ObjectId>(manifest["firmware_binary"].clone())?
                    == firmware.object,
            "source binary or harness differs from suite/preparation identity"
        );
        ensure!(
            manifest["source_sha256"] == crate::prepare::SOURCE_SHA256
                && manifest["patch_sha256"] == crate::prepare::hash_bytes(crate::prepare::PATCH)
                && manifest["adaptation_manifest_sha256"]
                    == crate::prepare::hash_bytes(crate::prepare::ADAPTATION),
            "source preparation identity differs from supported adaptation"
        );

        let run_inputs: Value = serde_json::from_slice(
            files
                .get("run-inputs.json")
                .context("run input evidence absent")?,
        )?;
        for (guest, key) in [("/firmware", "firmware"), ("/harness", "harness")] {
            ensure!(
                serde_json::from_value::<ObjectId>(run_inputs[key].clone())?
                    == spec
                        .inputs
                        .iter()
                        .find(|m| m.guest_path == guest)
                        .context("runtime input absent")?
                        .object,
                "executed binary identity differs"
            );
        }
        ensure!(
            run_inputs["seed"] == spec.argv[4]
                && run_inputs["pool_epoch"].as_u64() == Some(spec.argv[5].parse()?)
                && run_inputs["hash_cap_hps"].as_f64() == Some(spec.argv[3].parse()?),
            "executed run settings differ"
        );
        ensure!(
            run_inputs["difficulty_exponents"][0].as_i64() == Some(spec.argv[6].parse()?)
                && run_inputs["pool_anchor"] == spec.argv[7],
            "executed difficulty or anchor differs"
        );
        let schedule =
            crate::schedule::mining_schedule(spec.argv[3].parse()?, spec.argv[6].parse()?)?;
        ensure!(
            run_inputs["mining_schedule"] == serde_json::to_value(&schedule)?
                && run_inputs["difficulty_exponents"] == json!(schedule.exponents)
                && run_inputs["difficulty_scale"] == "three-halves"
                && run_inputs["clock"] == "wall-monotonic",
            "reported schedule or clock differs from locked policy"
        );
        let seed: [u8; 32] = hex::decode(&spec.argv[4])?
            .try_into()
            .map_err(|_| anyhow::anyhow!("seed must be 32 bytes"))?;
        let epoch: u32 = spec.argv[5].parse()?;
        let time_max = epoch
            .checked_add(u32::try_from(schedule.timeout_seconds)?)
            .and_then(|t| t.checked_add(120))
            .context("job time bound overflow")?;
        let anchor = (spec.argv[7] == "regtest-genesis")
            .then(md_sim_mining::regtest::RegtestAnchor::genesis);
        ensure!(
            serde_json::to_value(&report.regtest_anchor)? == serde_json::to_value(&anchor)?
                && report.jobs.len() == 2,
            "pool anchor or job count differs from locked suite"
        );
        for (i, job) in report.jobs.iter().enumerate() {
            let mut parent = seed;
            parent[0] ^= i as u8;
            if let Some(anchor) = &anchor {
                parent = anchor.previous_hash;
            }
            ensure!(
                job.id == format!("{}-{i}", hex::encode(&seed[..8]))
                    && job.previous == parent
                    && job.extra_nonce1 == seed[..4]
                    && job.extra_nonce2_size == 4
                    && job.version == 0x20000000
                    && job.version_mask == 0
                    && job.bits == 0x207fffff
                    && job.time_min == epoch
                    && job.time_max == time_max
                    && job.nonce_min == 0
                    && job.nonce_max == u32::MAX
                    && job.share_target
                        == wire_difficulty_target(&json!(1.5 * 2f64.powi(schedule.exponents[i])))?,
                "reported job restrictions differ from the locked suite policy"
            );
        }
        let max_elapsed_ns = spec
            .limits
            .wall_seconds
            .checked_mul(1_000_000_000)
            .context("wall budget overflow")?;
        let replay = md_sim_mining::replay_s2::replay(
            &report,
            files
                .get("device-transactions.jsonl")
                .context("device trace absent")?,
            max_elapsed_ns,
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        out.json("independent-device-replay.json", &replay)?;
        let trace = files
            .get("pool-transactions.jsonl")
            .context("Stratum trace absent")?;
        let mut messages = Vec::new();
        for line in trace.split(|b| *b == b'\n').filter(|s| !s.is_empty()) {
            ensure!(messages.len() < 4096, "trace message count exceeded");
            messages.push(serde_json::from_slice::<Value>(line)?);
        }
        let ordered_verdicts = ordered_pool_verdicts(&report, &messages, max_elapsed_ns)?;
        let mut checked = Vec::new();
        let mut jobs = BTreeSet::new();
        let mut rejects = Vec::new();
        for (i, submission) in report.submissions.iter().enumerate() {
            let job = report
                .jobs
                .iter()
                .find(|j| j.id == submission.share.job_id)
                .context("submitted job absent")?;
            let mut verifier = PoolVerifier::default();
            verifier
                .install(job.clone(), false)
                .map_err(anyhow::Error::msg)?;
            let verdict = ordered_verdicts[i].clone();
            if let Verdict::Accepted { .. } = verdict {
                ensure!(
                    verifier
                        .verify(&submission.share)
                        .map_err(anyhow::Error::msg)?
                        == verdict,
                    "accepted header independently differs"
                );
                let s = &submission.share;
                let params = json!([
                    "simulator",
                    s.job_id,
                    hex::encode(&s.extra_nonce2),
                    format!("{:08x}", s.time),
                    format!("{:08x}", s.nonce)
                ]);
                ensure!(
                    messages.iter().any(|v| v["direction"] == "firmware-to-pool"
                        && v["message"]["method"] == "mining.submit"
                        && v["message"]["params"] == params),
                    "accepted share lacks original firmware submission"
                );
                let header = serde_json::to_value(&verdict)?["Accepted"]["header_hex"].clone();
                ensure!(replay["results"].as_array().context("missing replay results")?.iter().any(|r|r["header_hex"]==header && r["firmware_pool_submission_present"]==true && r["pool_target_met"]==true),"accepted pool result lacks matching serial work and computed hardware response");
                jobs.insert(job.id.clone());
                checked.push(json!({"submission_index":i,"share":s,"verdict":verdict}));
                ensure!(
                    verifier.verify(s).map_err(anyhow::Error::msg)? == Verdict::Duplicate,
                    "duplicate was accepted"
                );
                let mut invalid = s.clone();
                invalid.extra_nonce2.push(0);
                ensure!(
                    verifier.verify(&invalid).map_err(anyhow::Error::msg)?
                        == Verdict::InvalidExtraNonce,
                    "invalid extranonce was accepted"
                );
                invalid = s.clone();
                invalid.time = job.time_max.checked_add(1).context("time range overflow")?;
                ensure!(
                    verifier.verify(&invalid).map_err(anyhow::Error::msg)? == Verdict::OutOfRange,
                    "out of range was accepted"
                );
                let mut low = None;
                for step in 1..=1024 {
                    let mut candidate = s.clone();
                    candidate.nonce = s.nonce.wrapping_add(step);
                    if verifier.verify(&candidate).map_err(anyhow::Error::msg)?
                        == Verdict::LowDifficulty
                    {
                        low = Some(candidate);
                        break;
                    }
                }
                ensure!(
                    low.is_some(),
                    "negative-share test found no above-target result within its bound"
                );
                let mut next = job.clone();
                next.id = format!("negative-clean-{i}");
                verifier.install(next, true).map_err(anyhow::Error::msg)?;
                ensure!(
                    verifier.verify(s).map_err(anyhow::Error::msg)? == Verdict::Stale,
                    "stale was accepted"
                );
                rejects.push(json!({"submission_index":i,"duplicate":"rejected","invalid_extra_nonce":"rejected","out_of_range":"rejected","stale_after_clean":"rejected","above_target_share":low,"above_target":"rejected"}));
            }
        }
        let notify: Vec<_> = messages
            .iter()
            .filter(|m| {
                m["direction"] == "pool-to-firmware" && m["message"]["method"] == "mining.notify"
            })
            .collect();
        let difficulty: BTreeSet<_> = messages
            .iter()
            .filter(|m| {
                m["direction"] == "pool-to-firmware"
                    && m["message"]["method"] == "mining.set_difficulty"
            })
            .map(|m| m["message"]["params"].to_string())
            .collect();
        let transitions = notify.len() >= 2
            && notify
                .iter()
                .skip(1)
                .any(|m| m["message"]["params"][8] == true)
            && difficulty.len() >= 2;
        out.json("independent-verifier.json",&json!({"implementation":"bitcoin_hashes independent from SHA256 engine","accepted":checked,"negative_submissions":rejects,"clean_job_and_difficulty_change":transitions,"negative_scope":"independent verifier exercise; not an injected firmware negative-submission test"}))?;
        if report.regtest_anchor.is_some() {
            let blocks =
                md_sim_mining::regtest::regtest_blocks(&report).map_err(anyhow::Error::msg)?;
            let exported: Value = serde_json::from_slice(
                files
                    .get("regtest-blocks.json")
                    .context("regtest export missing")?,
            )?;
            ensure!(
                exported == serde_json::to_value(&blocks)?,
                "regtest candidate differs on independent reconstruction"
            );
        }
        let gates = vec![
            gate(GATES[0], true, json!({"locked_inputs":run.inputs.len()}),
                json!("saved input identities and executed binary/settings match"),
                &["run-spec.json", "input-manifest.json", "preparation-inputs.json"], out)?,
            gate(GATES[1], process.status == "exited" && process.exit_code == Some(0)
                && !process.trace_truncated && session["execution_error"].is_null(),
                json!({"process":process.status,"exit":process.exit_code,"session":session}),
                json!("actual rebuilt archived source executes through isolated device path"),
                &["execution.json", "guest.log", "session.json"], out)?,
            gate(GATES[2], session["crc_errors"] == 0 && session["configured"] == true
                && session["chains"] == 8 && session["chips_per_chain"] == 32
                && session["work_generations"].as_u64().unwrap_or(0) >= 2
                && session["computed_hashes"].as_u64().unwrap_or(0) > 0
                && !session["trace_truncated"].as_bool().unwrap_or(true),
                session.clone(), json!("8 chains,32 chips; configured; at least2work generations; actual hash computation"),
                &["session.json", "device-transactions.jsonl", "independent-device-replay.json"], out)?,
            gate(GATES[3], report.completed && !report.timed_out && !report.budget_exhausted
                && report.failure.is_none() && jobs.len() >= 2 && transitions,
                json!({"accepted_jobs":jobs,"clean_and_difficulty":transitions}),
                json!("two fresh jobs accepted with independent headers, clean transition and difficulty change"),
                &["pool-report.json", "pool-transactions.jsonl", "independent-verifier.json"], out)?,
            gate(GATES[4], !rejects.is_empty(), json!(rejects),
                json!("duplicate, invalid, stale, out-of-range and above-target rejected by independent verifier"),
                &["independent-verifier.json"], out)?,
        ];
        let checksums = out.files().clone();
        out.json("evidence-checksums.json", &checksums)?;
        Ok(gates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clean_job_replay_preserves_stale_and_duplicate_verdicts() {
        use md_sim_mining::{
            stratum::{StratumLimits, SubmissionEvidence},
            PoolJob, Share,
        };
        let first = PoolJob {
            id: "a".into(),
            previous: [0; 32],
            coinbase_prefix: vec![1],
            extra_nonce1: vec![],
            extra_nonce2_size: 4,
            coinbase_suffix: vec![],
            merkle_branch: vec![],
            version: 1,
            version_mask: 0,
            bits: 0x207fffff,
            time_min: 1,
            time_max: 2,
            nonce_min: 0,
            nonce_max: u32::MAX,
            share_target: wire_difficulty_target(&json!(2f64.powi(-31))).unwrap(),
        };
        let mut second = first.clone();
        second.id = "b".into();
        second.coinbase_prefix = vec![2];
        let mut one = Share {
            job_id: "a".into(),
            extra_nonce2: vec![0; 4],
            nonce: 1,
            time: 1,
            version: 1,
        };
        let mut oracle = PoolVerifier::default();
        oracle.install(first.clone(), true).unwrap();
        let accepted_one = loop {
            let verdict = oracle.verify(&one).unwrap();
            if matches!(verdict, Verdict::Accepted { .. }) {
                break verdict;
            }
            one.nonce += 1;
            assert!(one.nonce < 100);
        };
        let mut two = one.clone();
        two.job_id = "b".into();
        oracle.install(second.clone(), true).unwrap();
        let accepted_two = loop {
            let verdict = oracle.verify(&two).unwrap();
            if matches!(verdict, Verdict::Accepted { .. }) {
                break verdict;
            }
            two.nonce += 1;
            assert!(two.nonce < 100);
        };
        let report = PoolReport {
            response_policy: md_sim_mining::stratum::ResponsePolicy::JsonRpc1StratumV1,
            connection_policy: md_sim_mining::stratum::CompletionPolicy::CloseAfterWork,
            connection_end: None,
            regtest_anchor: None,
            transport: "unit vector".into(),
            hash_implementation: "independent".into(),
            jobs: vec![first, second],
            submissions: vec![
                SubmissionEvidence {
                    share: one.clone(),
                    verdict: accepted_one,
                },
                SubmissionEvidence {
                    share: one.clone(),
                    verdict: Verdict::Stale,
                },
                SubmissionEvidence {
                    share: two.clone(),
                    verdict: accepted_two,
                },
                SubmissionEvidence {
                    share: two.clone(),
                    verdict: Verdict::Duplicate,
                },
            ],
            completed: true,
            stopped: false,
            timed_out: false,
            limits: StratumLimits::default(),
            messages_received: 4,
            budget_exhausted: false,
            failure: None,
        };
        let notify = |job: &PoolJob| json!({"direction":"pool-to-firmware","elapsed_ns":"1","message":{"method":"mining.notify","params":notify_params(job,true)}});
        let submit = |share: &Share| json!({"direction":"firmware-to-pool","elapsed_ns":"1","message":{"method":"mining.submit","params":submission_params(share)}});
        let mut messages = vec![
            json!({"direction":"firmware-to-pool","elapsed_ns":"1","message":{"id":0,"method":"mining.subscribe","params":[]}}),
            json!({"direction":"pool-to-firmware","elapsed_ns":"1","message":{"id":0,"error":null,"result":[[],"",4]}}),
            json!({"direction":"pool-to-firmware","elapsed_ns":"1","message":{"method":"mining.set_difficulty","params":[2f64.powi(-31)]}}),
            notify(&report.jobs[0]),
            submit(&one),
            notify(&report.jobs[1]),
            submit(&one),
            submit(&two),
            submit(&two),
        ];
        let result = ordered_pool_verdicts(&report, &messages, 10).unwrap();
        assert_eq!(result[1], Verdict::Stale);
        assert_eq!(result[3], Verdict::Duplicate);
        for field in 1..=7 {
            let mut tampered = messages.clone();
            tampered[3]["message"]["params"][field] = json!("changed wire field");
            assert!(
                ordered_pool_verdicts(&report, &tampered, 10).is_err(),
                "notify field {field}"
            );
        }
        let mut tampered = messages.clone();
        tampered[1]["message"]["result"][1] = json!("ff");
        assert!(ordered_pool_verdicts(&report, &tampered, 10).is_err());
        tampered = messages.clone();
        tampered[1]["message"]["result"][2] = json!(5);
        assert!(ordered_pool_verdicts(&report, &tampered, 10).is_err());
        tampered = messages.clone();
        tampered[1]["message"]["id"] = json!(999);
        assert!(ordered_pool_verdicts(&report, &tampered, 10).is_err());
        tampered = messages.clone();
        tampered[2]["message"]["params"][0] = json!(2f64.powi(-30));
        assert!(ordered_pool_verdicts(&report, &tampered, 10).is_err());
        tampered = messages.clone();
        tampered.last_mut().unwrap()["elapsed_ns"] = json!("11");
        assert!(ordered_pool_verdicts(&report, &tampered, 10).is_err());
        messages.swap(5, 6);
        assert!(ordered_pool_verdicts(&report, &messages, 10).is_err());
    }
    #[test]
    fn malformed_recipe_is_an_error_not_a_panic() {
        let mut spec = ProcessSpec {
            support_virtualization: None,
            private_channel: None,
            stdin_stream: None,
            namespace_uid: 0,
            dashboard: None,
            loopback: false,
            schema_version: "2.0".into(),
            stdin_object: None,
            argv: vec![],
            inputs: vec![],
            environment: BTreeMap::new(),
            limits: md_sim_runtime::isolation::Limits::default(),
        };
        for args in [
            vec![],
            vec!["/harness"],
            vec![
                "/harness",
                "/firmware",
                "/work/results",
                "10000",
                "seed",
                "1",
                "-20",
                "regtest-genesis",
            ],
        ] {
            spec.argv = args.iter().map(|s| s.to_string()).collect();
            assert!(validate_recipe(&spec).is_err());
        }
    }
}

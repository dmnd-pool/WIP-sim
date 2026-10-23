//! Capture and verify support inputs for native-worker SSH modes. A shared
//! pool run does not establish native dashboard access.
use super::{execution, prepared, shared_pool};
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{save_json, Mount, ProcessSpec},
    store::hash_file,
};
use serde_json::{json, Value};
use std::{fs, path::Path, process::Command};

pub const MARKER: &str = "MD_SIM_NATIVE_SSH_BROKER ";
const VETH: &str = "/usr/lib/modules/7.2.9-arch1-1/kernel/drivers/net/veth.ko.zst";
const E1000: &str =
    "/usr/lib/modules/7.2.9-arch1-1/kernel/drivers/net/ethernet/intel/e1000/e1000.ko.zst";

pub fn prepare(
    helper: &Path,
    out: &Path,
    shared_backend: Option<&Path>,
    browser_payload: Option<&Path>,
) -> Result<()> {
    let relay = helper
        .parent()
        .context("helper directory")?
        .parent()
        .context("release directory")?
        .join("md-sim-worker-relay");
    let scenario = out.join("scenario.json");
    let execution = out.join("execution");
    let mut command = Command::new(helper);
    command
        .arg(if shared_backend.is_some() {
            "prepare-shared-pool"
        } else {
            "prepare-worker-broker"
        })
        .arg(&scenario)
        .arg(execution.join("locked-run.json"))
        .arg(execution.join("input-store/objects"))
        .arg(&relay)
        .arg(VETH);
    let mut proof = None;
    let mut carrier_dashboard = false;
    let mut access_proof = None;
    if let Some(backend) = shared_backend {
        let manifest_path = execution.join("shared-pool.json");
        let (manifest, _) = shared_pool::read(&manifest_path)?;
        let pool_guest = helper
            .parent()
            .context("pool helper directory")?
            .join("s9_pool_guest");
        let process: ProcessSpec = serde_json::from_slice(&prepared::bounded(
            &execution.join("process-spec.json"),
            4 << 20,
        )?)?;
        process.validate()?;
        let pool_input = hash_file(&execution.join("pool-input.json"))?;
        let identity = hash_file(&manifest_path)?;
        ensure!(
            process
                .inputs
                .iter()
                .any(|input| input.guest_path == "/shared-pool.json" && input.object == identity)
                && process
                    .inputs
                    .iter()
                    .any(|input| input.guest_path == "/private-pool.json"
                        && input.object == pool_input)
                && serde_json::to_value(&process.private_channel)?
                    == serde_json::to_value(&manifest.carrier)?,
            "selected shared process input or carrier differs"
        );
        ensure!(
            manifest.pool_verifier == hash_file(&pool_guest)?
                && manifest.support_runner == hash_file(helper)?
                && manifest.support_backend == hash_file(&backend.join("qemu-system-x86_64"))?
                && manifest.support_kernel == hash_file(Path::new(super::KERNEL))?,
            "selected support component differs from its manifest"
        );
        let mut provider = json!({
            "schema":"md-sim-worker-private-carrier/1","process":process,
            "support_identity":identity,"handoff_seconds":300,
            "wall_seconds":process.limits.wall_seconds + 30
        });
        let locked: md_sim_coordinator::LockedRun = serde_json::from_slice(&prepared::bounded(
            &execution.join("locked-run.json"),
            4 << 20,
        )?)?;
        locked.validate()?;
        ensure!(
            serde_json::to_value(&locked.process)? == serde_json::to_value(&process)?,
            "provider lock and captured process specification differ"
        );
        if let Some(policy) = locked.policies.get("carrier_dashboard") {
            let policy: md_sim_runtime::store::ObjectId = serde_json::from_value(policy.clone())?;
            ensure!(
                hash_file(&execution.join("carrier-dashboard.json"))? == policy,
                "captured carrier dashboard policy differs"
            );
            provider["lease"] = json!({"locked_run":locked,"policy":policy});
            carrier_dashboard = true;
            access_proof = Some(json!({"policy":policy,
                "locked_run_file":hash_file(&execution.join("locked-run.json"))?,
                "access_program":hash_file(&relay.with_file_name("md-sim-access"))?,
                "browser_requested":browser_payload.is_some(),
                "browser_payload":browser_payload.map(hash_file).transpose()?,
                "browser_driver":if browser_payload.is_some() {Some(hash_file(&helper.with_file_name("s9_browser_check"))?)} else {None}}));
        }
        save_json(&out.join("provider.json"), &provider)?;
        proof = Some(json!({"pool_verifier":manifest.pool_verifier,
            "support_runner":manifest.support_runner,"support_backend":manifest.support_backend,
            "support_kernel":manifest.support_kernel,"manifest":identity,"pool_input":pool_input,
            "e1000_module":hash_file(Path::new(E1000))?,"carrier_dashboard":access_proof}));
        command.arg(pool_guest).arg(E1000);
        if let Some(payload) = browser_payload {
            ensure!(
                carrier_dashboard,
                "browser capture requires the selected carrier dashboard policy"
            );
            command.arg(payload);
        }
    }
    ensure!(
        browser_payload.is_none() || shared_backend.is_some(),
        "browser capture requires shared firmware execution"
    );
    let status = command.output()?;
    fs::write(out.join("ssh-preparation.stdout"), &status.stdout)?;
    fs::write(out.join("ssh-preparation.stderr"), &status.stderr)?;
    ensure!(status.status.success(), "SSH scenario preparation failed");
    let mut scenario_value: Value =
        serde_json::from_slice(&prepared::bounded(&scenario, 4 << 20)?)?;
    let files = scenario_value["files"]
        .as_object_mut()
        .context("scenario files absent")?;
    ensure!(
        !files.contains_key("/worker-identity.json"),
        "scenario contains worker identity already"
    );
    files.insert(
        "/worker-identity.json".into(),
        json!({"path":out.join("worker-identity.json").canonicalize()?,"mode":292}),
    );
    if carrier_dashboard {
        ensure!(
            !files.contains_key("/provider.json"),
            "scenario contains a provider configuration already"
        );
        files.insert(
            "/provider.json".into(),
            json!({"path":out.join("provider.json").canonicalize()?,"mode":292}),
        );
    }
    save_json(&scenario, &scenario_value)?;
    save_json(
        &out.join("ssh-inputs.json"),
        &json!({
            "helper":hash_file(helper)?,"relay":hash_file(&relay)?,"veth":hash_file(Path::new(VETH))?,
            "scenario":hash_file(&scenario)?,"shared_support":proof,
            "scope":"Captured SSH transport and selected shared support inputs. Execution and mining require separate outcome evidence."
        }),
    )
}

pub fn support_proof(out: &Path, preparation: &Path) -> Result<()> {
    let inputs: Value =
        serde_json::from_slice(&prepared::bounded(&out.join("ssh-inputs.json"), 65536)?)?;
    let mut proof = inputs["shared_support"].clone();
    ensure!(proof.is_object(), "shared support input proof absent");
    proof["scenario"] = serde_json::to_value(hash_file(&out.join("scenario.json"))?)?;
    proof["initramfs"] = serde_json::to_value(hash_file(&preparation.join("initramfs.cpio.gz"))?)?;
    proof["root_manifest"] =
        serde_json::to_value(hash_file(&preparation.join("root-manifest.json"))?)?;
    proof["outer_process"] = serde_json::to_value(hash_file(&out.join("process-spec.json"))?)?;
    proof["provider_config"] = serde_json::to_value(hash_file(&out.join("provider.json"))?)?;
    let outer: ProcessSpec =
        serde_json::from_slice(&prepared::bounded(&out.join("process-spec.json"), 4 << 20)?)?;
    outer.validate()?;
    let root: Vec<Mount> = serde_json::from_slice(&prepared::bounded(
        &preparation.join("root-manifest.json"),
        4 << 20,
    )?)?;
    for (mounts, guest, identity) in [
        (&outer.inputs, "/qemu", "support_backend"),
        (&outer.inputs, "/vmlinuz", "support_kernel"),
        (&outer.inputs, "/initramfs.cpio.gz", "initramfs"),
        (&outer.inputs, "/provider.json", "provider_config"),
        (&root, "/namespace-check", "support_runner"),
        (&root, "/pool-guest", "pool_verifier"),
        (&root, "/shared-pool.json", "manifest"),
        (&root, "/private-pool.json", "pool_input"),
    ] {
        let selected: Vec<_> = mounts
            .iter()
            .filter(|input| input.guest_path == guest)
            .collect();
        ensure!(
            selected.len() == 1 && proof[identity] == serde_json::to_value(&selected[0].object)?,
            "captured support input differs from the selected identity"
        );
    }
    if let Some(access) = proof["carrier_dashboard"].as_object() {
        for (guest, expected) in [
            ("/provider.json", proof["provider_config"].clone()),
            ("/carrier-dashboard.json", access["policy"].clone()),
            (
                "/carrier-locked-run.json",
                access["locked_run_file"].clone(),
            ),
            ("/md-sim-access", access["access_program"].clone()),
        ] {
            let selected: Vec<_> = root
                .iter()
                .filter(|mount| mount.guest_path == guest)
                .collect();
            ensure!(
                selected.len() == 1 && serde_json::to_value(&selected[0].object)? == expected,
                "captured carrier support input differs from selected identity"
            );
        }
    }
    if proof["carrier_dashboard"].is_object() {
        let scenario: Value =
            serde_json::from_slice(&prepared::bounded(&out.join("scenario.json"), 4 << 20)?)?;
        check_browser_capture(&proof["carrier_dashboard"], &scenario, &root)?;
    }
    let directory = out.join("execution/shared-pool");
    fs::create_dir_all(&directory)?;
    save_json(&directory.join("support-proof.json"), &proof)
}

fn check_browser_capture(access: &Value, scenario: &Value, root: &[Mount]) -> Result<()> {
    let requested = access["browser_requested"]
        .as_bool()
        .context("browser request absent")?;
    ensure!(
        requested == (scenario["environment"]["MD_SIM_CARRIER_BROWSER"] == "1"),
        "prepared browser mode differs from original request"
    );
    for (guest, field) in [
        ("/s9-browser-check", "browser_driver"),
        ("/carrier-browser-payload.json", "browser_payload"),
    ] {
        let selected: Vec<_> = root
            .iter()
            .filter(|mount| mount.guest_path == guest)
            .collect();
        if requested {
            ensure!(
                selected.len() == 1 && serde_json::to_value(&selected[0].object)? == access[field],
                "prepared browser input differs from original request"
            );
        } else {
            ensure!(
                selected.is_empty() && access[field].is_null(),
                "unexpected browser input without a request"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_capture_cannot_downgrade_or_substitute_the_original_request() -> Result<()> {
        let driver = md_sim_runtime::store::ObjectId {
            sha256: "11".repeat(32),
            size: 1,
        };
        let payload = md_sim_runtime::store::ObjectId {
            sha256: "22".repeat(32),
            size: 2,
        };
        let access =
            json!({"browser_requested":true,"browser_driver":driver,"browser_payload":payload});
        let scenario = json!({"environment":{"MD_SIM_CARRIER_BROWSER":"1"}});
        let root = vec![
            Mount {
                guest_path: "/s9-browser-check".into(),
                object: driver,
            },
            Mount {
                guest_path: "/carrier-browser-payload.json".into(),
                object: payload,
            },
        ];
        check_browser_capture(&access, &scenario, &root)?;
        assert!(check_browser_capture(&access, &json!({"environment":{}}), &root).is_err());
        for index in 0..root.len() {
            let mut changed = root.clone();
            changed[index].object.size += 1;
            assert!(check_browser_capture(&access, &scenario, &changed).is_err());
        }
        assert!(check_browser_capture(&access, &scenario, &root[..1]).is_err());
        let basic = json!({"browser_requested":false,"browser_driver":null,"browser_payload":null});
        check_browser_capture(&basic, &json!({"environment":{}}), &[])?;
        assert!(check_browser_capture(&basic, &scenario, &root).is_err());
        Ok(())
    }
}

pub fn restore(record: &Value, out: &Path, payload: &Path) -> Result<()> {
    // Retain any verified result before checking later namespace observations.
    // A failed observation must not discard bytes already retrieved by SSH.
    let result = execution::restore(&record["artifacts"], out)?;
    ensure!(
        record["passed"] == true,
        "SSH broker did not pass: {}",
        record["error"]
    );
    let result = result.context("SSH worker result absent")?;
    ensure!(
        result.status == "cancelled" && !result.trace_truncated,
        "selected SSH recovery did not stop its actual job"
    );
    ensure!(
        record["recovery"]["result"]
            == serde_json::to_value(hash_file(&out.join("worker-result.json"))?)?,
        "SSH recovery result identity differs"
    );
    let run: Value =
        serde_json::from_slice(&prepared::bounded(&out.join("worker-run.json"), 65536)?)?;
    ensure!(
        record["recovery"]["run_id"] == run["run_id"],
        "SSH recovery selected a different run"
    );
    let before = record["recovery"]["stdout_before_loss"]
        .as_u64()
        .context("prior stdout absent")?;
    let after = record["recovery"]["stdout_after_loss"]
        .as_u64()
        .context("recovered stdout absent")?;
    let reconnected = record["recovery"]["stdout_at_reconnect"]
        .as_u64()
        .context("reconnect stdout absent")?;
    ensure!(
        after > reconnected && reconnected >= before && before > 0,
        "SSH job did not advance after client loss"
    );
    for (name, expected) in [
        ("status_at_reconnect", reconnected),
        ("status_before_recovery", after),
    ] {
        let status = &record["recovery"][name];
        ensure!(
            status["run_id"] == run["run_id"]
                && status["state"]["state"] == "booting"
                && status["saved_logs"]["stdout"] == expected,
            "SSH post-loss status selected another job or log boundary"
        );
    }
    let expected = serde_json::to_value(hash_file(payload)?)?;
    let stdout = String::from_utf8(hex::decode(&result.stdout)?)?;
    let pulses: Vec<Value> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("MD_SIM_CHANNEL_PROCESS "))
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    ensure!(
        pulses.len() >= 2,
        "support process produced too few observations"
    );
    for (index, pulse) in pulses.iter().enumerate() {
        ensure!(
            pulse["challenge"] == expected && pulse["sequence"] == index,
            "executed support process challenge or sequence differs"
        );
    }
    let cleanup: Value =
        serde_json::from_slice(&prepared::bounded(&out.join("worker-cleanup.json"), 65536)?)?;
    ensure!(
        record["recovery"]["cleanup"] == cleanup
            && cleanup["removed"] == true
            && cleanup["run_id"] == run["run_id"]
            && cleanup["acknowledged_result"] == record["recovery"]["result"],
        "SSH cleanup evidence differs"
    );
    save_json(
        &out.join("ssh-recovery.json"),
        &json!({
            "passed":true,"run_id":run["run_id"],"challenge":expected,"observations":pulses.len(),
            "result":hash_file(&out.join("worker-result.json"))?,"stdout_before_loss":before,"stdout_at_reconnect":reconnected,"stdout_after_loss":after,
            "retention":"VM-local result before cleanup; outer export after cleanup. No whole-VM-loss retention claim."
        }),
    )
}

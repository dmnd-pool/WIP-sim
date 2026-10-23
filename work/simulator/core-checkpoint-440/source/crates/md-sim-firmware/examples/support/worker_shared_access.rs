//! Durable access evidence and independent identity checks for the joined run.
use super::{channel_artifacts, prepared, retain_bytes, wire};
use anyhow::{ensure, Context, Result};
use md_sim_access::carrier::{CarrierEndpoint, ProviderBinding, Selection};
use md_sim_coordinator::LockedRun;
use md_sim_runtime::{
    isolation::save_json,
    store::{hash_file, ObjectId},
};
use serde_json::Value;
use std::{
    fs::{self, File},
    path::Path,
};

#[allow(dead_code)] // Retention uses the same selected-browser report oracle.
#[path = "s9_browser_target.rs"]
mod browser_target;

pub fn retain(request: &wire::Request, run: &LockedRun, proof: &Value, out: &Path) -> Result<()> {
    let selected = run.policies.get("carrier_dashboard");
    ensure!(
        selected.is_some() == request.access_bundle.is_some(),
        "selected carrier access evidence absent or unrequested"
    );
    let Some(export) = &request.access_bundle else {
        return Ok(());
    };
    let policy: ObjectId =
        serde_json::from_value(selected.context("selected access policy absent")?.clone())?;
    let directory = out.join("shared-access");
    fs::create_dir_all(&directory)?;
    let path = directory.join("access-bundle.json");
    channel_artifacts::import_file(
        export,
        &path,
        wire::ACCESS_BUNDLE_BYTES,
        wire::ACCESS_COMPRESSED_BYTES,
    )?;
    let bundle: wire::AccessBundle =
        serde_json::from_slice(&prepared::bounded(&path, wire::ACCESS_BUNDLE_BYTES)?)?;
    ensure!(
        bundle.schema == "md-sim-shared-carrier-artifacts/1"
            && bundle.native == request.identity.native
            && bundle.policy == policy
            && serde_json::to_value(&bundle.provider)? == proof["provider_config"]
            && bundle.browser_requested
                == (proof["carrier_dashboard"]["browser_requested"] == true)
            && serde_json::to_value(&bundle.browser_driver)?
                == proof["carrier_dashboard"]["browser_driver"]
            && bundle.files.len() <= 512,
        "carrier access bundle belongs to another selected execution"
    );
    let mut total = 0u64;
    let mut directories = std::collections::BTreeSet::new();
    for (name, bytes) in &bundle.files {
        wire::access_name(name)?;
        total = total
            .checked_add(bytes.object.size)
            .context("access evidence size overflow")?;
        ensure!(total <= 15 << 20, "access evidence total bound");
        let target = directory.join(name);
        let parent = target.parent().context("access evidence parent absent")?;
        fs::create_dir_all(parent)?;
        retain_bytes(bytes, &target, 8 << 20)?;
        let mut current = parent;
        while current.starts_with(&directory) {
            directories.insert(current.to_path_buf());
            let Some(parent) = current.parent() else {
                break;
            };
            current = parent;
        }
    }
    let policy_bytes = prepared::bounded(&directory.join("metadata/policy.json"), 65536)?;
    let selected = Selection::new(run, policy, &policy_bytes, &request.identity.native.token)?;
    let provider_bytes = prepared::bounded(&directory.join("metadata/provider.json"), 4 << 20)?;
    ProviderBinding::from_config(selected, bundle.provider, &provider_bytes)?;
    save_json(&directory.join("retention-receipt.json"), &request.identity)?;
    for path in directories.iter().rev() {
        File::open(path)?.sync_all()?;
    }
    File::open(&directory)?.sync_all()?;
    Ok(())
}

fn read(path: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&prepared::bounded(path, 8 << 20)?)?)
}

fn verify_files(
    root: &Path,
    files: &std::collections::BTreeMap<String, wire::Bytes>,
) -> Result<()> {
    for (name, bytes) in files {
        wire::access_name(name)?;
        ensure!(
            hash_file(&root.join(name))? == bytes.object,
            "extracted carrier evidence differs from retained bundle: {name}"
        );
    }
    Ok(())
}

pub fn check(out: &Path, receipt: &wire::Receipt) -> Result<()> {
    let Some(identity) = &receipt.access_bundle else {
        return Ok(());
    };
    let root = out.join("shared-access");
    ensure!(
        hash_file(&root.join("access-bundle.json"))? == *identity,
        "retained carrier bundle differs"
    );
    let bundle: wire::AccessBundle = serde_json::from_slice(&prepared::bounded(
        &root.join("access-bundle.json"),
        wire::ACCESS_BUNDLE_BYTES,
    )?)?;
    ensure!(
        bundle.native == receipt.native,
        "access bundle retention identity differs"
    );
    verify_files(&root, &bundle.files)?;
    let run: LockedRun = serde_json::from_value(read(&out.join("locked-run.json"))?)?;
    let policy: ObjectId = serde_json::from_value(run.policies["carrier_dashboard"].clone())?;
    let selected = Selection::new(
        &run,
        policy,
        &prepared::bounded(&root.join("metadata/policy.json"), 65536)?,
        &receipt.native.token,
    )?;
    let provider = root.join("metadata/provider.json");
    let binding = ProviderBinding::from_config(
        selected,
        hash_file(&provider)?,
        &prepared::bounded(&provider, 4 << 20)?,
    )?;
    let endpoint: CarrierEndpoint =
        serde_json::from_value(read(&root.join("server/bridge/endpoint.json"))?)?;
    endpoint.verify(&binding)?;
    let client = read(&root.join("client/report.json"))?;
    let observer = read(&root.join("client-observer/report.json"))?;
    let service = read(&root.join("server/service-outcome.json"))?;
    let supervisor = read(&root.join("server-root/carrier-outcome.json"))?;
    ensure!(
        client == observer
            && client["passed"] == true
            && client["result"]["selected_run"] == receipt.native.run_id
            && client["result"]["endpoint"] == serde_json::to_value(&endpoint)?
            && client["result"]["http"]["passed"] == true
            && client["result"]["tls"]["passed"] == true
            && service["identity"] == serde_json::to_value(binding.selection().identity())?
            && service["receipt"]["listeners_closed"] == true
            && supervisor["success"] == true
            && supervisor["exit_code"] == 0,
        "selected carrier native access or service closure did not pass"
    );
    if bundle.browser_requested {
        let retained = &client["result"]["browser"];
        let browser = &retained["report"];
        let driver = bundle
            .browser_driver
            .as_ref()
            .context("requested browser driver absent")?;
        let selection: browser_target::CarrierSelection =
            serde_json::from_value(browser["selection"].clone())?;
        ensure!(
            client["result"]["browser_tested"] == true
                && retained["exit_code"] == 0
                && retained["error"].is_null()
                && retained["report_count"] == 1
                && selection.selected_run == receipt.native.run_id
                && selection.run.object()? == run.object()?
                && selection.policy == bundle.policy
                && selection.provider == bundle.provider
                && selection.endpoint == endpoint
                && selection.publication == client["result"]["publication"]
                && selection.policy_bytes.as_bytes()
                    == prepared::bounded(&root.join("metadata/policy.json"), 65536)?
                && selection.provider_bytes.as_bytes() == prepared::bounded(&provider, 4 << 20)?,
            "selected carrier browser metadata differs from retained execution"
        );
        browser_target::verify_report(browser, &browser["selection"], driver)?;
        for (filename, field) in [
            ("board-browser.stdout", "stdout"),
            ("board-browser.stderr", "stderr"),
        ] {
            let artifact = bundle
                .files
                .get(&format!("client-observer/{filename}"))
                .context("requested browser raw log absent")?;
            ensure!(
                retained[field] == serde_json::to_value(&artifact.object)?,
                "browser raw log identity differs"
            );
        }
        ensure!(
            bundle
                .files
                .contains_key("client-observer/board-browser-process.json")
                && read(&root.join("client-observer/board-browser-process.json"))? == *retained,
            "browser process evidence differs from client result"
        );
        let stdout =
            prepared::bounded(&root.join("client-observer/board-browser.stdout"), 8 << 20)?;
        let reports = std::str::from_utf8(&stdout)?
            .lines()
            .filter_map(|line| line.strip_prefix("MD_SIM_BOSER_BROWSER_RESULT "))
            .map(serde_json::from_str::<Value>)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(
            reports.len() == 1 && reports[0] == *browser,
            "browser raw report differs or is duplicated"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resumed_assessment_rejects_edited_extracted_evidence() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join("client"))?;
        let path = root.path().join("client/report.json");
        fs::write(&path, b"abc")?;
        let files = std::collections::BTreeMap::from([(
            "client/report.json".into(),
            wire::Bytes {
                object: ObjectId {
                    sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
                        .into(),
                    size: 3,
                },
                hex: "616263".into(),
            },
        )]);
        verify_files(root.path(), &files)?;
        fs::write(&path, b"abd")?;
        assert!(verify_files(root.path(), &files).is_err());
        fs::remove_file(path)?;
        assert!(verify_files(root.path(), &files).is_err());
        Ok(())
    }
}

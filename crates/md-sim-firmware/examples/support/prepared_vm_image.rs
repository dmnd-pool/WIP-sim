//! Bind the selected support image to its preparation and original browser request.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::Mount,
    store::{hash_file, ObjectId},
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

fn read(path: &Path) -> Result<Value> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "image metadata is not a regular file"
    );
    let mut bytes = Vec::new();
    file.take((4 << 20) + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 4 << 20, "image metadata exceeds bound");
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn browser_request(
    environment: &BTreeMap<String, String>,
    access: &Value,
) -> Result<Option<ObjectId>> {
    let requested = match environment
        .get("MD_SIM_CARRIER_BROWSER_SUPPORT")
        .map(String::as_str)
    {
        None => false,
        Some("1") => true,
        _ => anyhow::bail!("invalid captured browser support mode"),
    };
    if access.is_null() {
        ensure!(!requested, "browser support lacks its original request");
        return Ok(None);
    }
    ensure!(
        access["browser_requested"].as_bool() == Some(requested),
        "outer browser mode differs from original request"
    );
    if requested {
        let object: ObjectId = serde_json::from_value(access["browser_payload"].clone())?;
        object.validate()?;
        Ok(Some(object))
    } else {
        ensure!(
            access["browser_payload"].is_null() && access["browser_driver"].is_null(),
            "unexpected browser identity without a request"
        );
        Ok(None)
    }
}

pub struct Selected {
    pub path: PathBuf,
    pub object: ObjectId,
}

pub fn select(
    preparation: &Path,
    scenario: &Value,
    kernel: &ObjectId,
    browser: Option<&ObjectId>,
) -> Result<Selected> {
    ensure!(
        match scenario["environment"].get("MD_SIM_CARRIER_BROWSER") {
            None => browser.is_none(),
            Some(Value::String(value)) => value == "1" && browser.is_some(),
            _ => false,
        },
        "prepared browser mode differs from original request"
    );
    let path = preparation.join(if browser.is_some() {
        "combined-initramfs.cpio.gz"
    } else {
        "initramfs.cpio.gz"
    });
    let object = hash_file(&path)?;
    let prepared = read(&preparation.join("prepared-image.json"))?;
    ensure!(
        prepared["schema"] == "md-sim-private-vm-preparation/1"
            && prepared["image"] == serde_json::to_value(&object)?
            && prepared["kernel"] == serde_json::to_value(kernel)?
            && prepared["root_manifest"]
                == serde_json::to_value(hash_file(&preparation.join("root-manifest.json"))?)?,
        "prepared client image identity differs"
    );
    if let Some(expected) = browser {
        expected.validate()?;
        let original = scenario["payload_manifest"]
            .as_str()
            .filter(|path| !path.is_empty())
            .context("browser payload path absent")?;
        let report = read(&preparation.join("payload-report.json"))?;
        ensure!(
            scenario["files"]["/carrier-browser-payload.json"]["path"] == original
                && hash_file(&preparation.join("payload-manifest.json"))? == *expected
                && report["schema"] == "md-sim-vm-payload-result/1"
                && report["manifest"] == serde_json::to_value(expected)?
                && report["combined_initramfs"] == serde_json::to_value(&object)?,
            "appended browser payload differs from original request"
        );
    } else {
        ensure!(
            scenario["payload_manifest"].is_null()
                && !preparation.join("payload-report.json").try_exists()?,
            "unexpected appended payload without a browser request"
        );
    }
    Ok(Selected { path, object })
}

pub fn check_browser_capture(access: &Value, scenario: &Value, root: &[Mount]) -> Result<()> {
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
#[path = "prepared_vm_image_tests.rs"]
mod tests;

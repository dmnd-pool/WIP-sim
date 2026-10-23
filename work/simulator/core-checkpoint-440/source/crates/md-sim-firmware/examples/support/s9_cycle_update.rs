//! Native keep-file configuration for the exact signed2022 SD update cycle.
//! These are explicit reference-environment adaptations, not board evidence.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::save_json,
    store::{hash_file, ObjectId},
};
use serde_json::{json, Value};
use std::path::Path;

#[path = "s9_monitor_adaptation.rs"]
mod monitor;

const KEEP_ORIGINAL: &str = "9e87b05fe2f12c84d1c465b396322748479f64bc40e5bab061887f0bd95020a6";
const KEEP_CONFIGURED: &str = "d7116cbec060fe94c01d21da5575e66d3141d8b0f20c5a53a854b0d524392cb9";

pub struct Prepared {
    pub initial: String,
    pub resumed: String,
    pub manifest: Value,
}

fn boot_checks(inspection: &Value) -> Result<String> {
    let mut script = String::new();
    for (member, name) in [
        ("spl", "boot.bin"),
        ("uboot", "u-boot.img"),
        ("fit", "fit.itb"),
        ("fpga", "system.bit.gz"),
    ] {
        let comparison = &inspection["baseline_comparisons"][member];
        ensure!(
            comparison["identical"] == true && comparison["baseline_file"] == name,
            "signed update does not match selected baseline: {member}"
        );
        let object: ObjectId = serde_json::from_value(comparison["baseline_object"].clone())?;
        ensure!(
            object.size > 0
                && object.sha256.len() == 64
                && hex::encode(hex::decode(&object.sha256)?) == object.sha256,
            "invalid baseline member identity"
        );
        script.push_str(&format!(
            "[ \"$(sha256sum /tmp/mnt/boot/{name} | cut -d ' ' -f 1)\" = '{}' ]\n",
            object.sha256
        ));
    }
    script.push_str("printf '\\nMD_SIM_POST_UPDATE_ARCHIVED_FILES_VERIFIED\\n'\n");
    Ok(script)
}

pub fn prepare(source: &Path, inspection: &Value, baseline: &Path, out: &Path) -> Result<Prepared> {
    ensure!(
        inspection["archive"]
            == json!({"sha256":super::update::SHA256,"size":super::update::BYTES})
            && inspection["baseline_manifest"] == serde_json::to_value(hash_file(baseline)?)?,
        "signed update inspection input differs"
    );
    let files = boot_checks(inspection)?;
    std::fs::create_dir(out)?;
    let adaptation = monitor::prepare(source, out)?;
    let original = adaptation["original"]["sha256"]
        .as_str()
        .context("monitor identity absent")?;
    let initial = format!(
        r#"[ "$(sha256sum /etc/sysupgrade.conf | cut -d ' ' -f 1)" = '{KEEP_ORIGINAL}' ]
printf '/etc/web_storage.json\n' >> /etc/sysupgrade.conf
[ "$(sha256sum /etc/sysupgrade.conf | cut -d ' ' -f 1)" = '{KEEP_CONFIGURED}' ]
printf '\nMD_SIM_NATIVE_KEEP_POLICY_CONFIGURED\n'
"#
    );
    let resumed = format!(
        r#"[ "$(sha256sum /etc/sysupgrade.conf | cut -d ' ' -f 1)" = '{KEEP_CONFIGURED}' ]
printf '\nMD_SIM_NATIVE_KEEP_POLICY_PERSISTED\n'
[ "$(sha256sum /usr/sbin/bosminer_monitor.lua | cut -d ' ' -f 1)" = '{original}' ]
{files}"#
    );
    let manifest = json!({"schema":"md-sim-s9-native-update-cycle-adaptation/1",
        "keep_file":{"path":"/etc/sysupgrade.conf","original":{"sha256":KEEP_ORIGINAL,"size":128},
            "configured":{"sha256":KEEP_CONFIGURED,"size":150},"append":"/etc/web_storage.json\n",
            "reason":"The native default backup omits the dashboard theme file. Use its explicit keep-file configuration before installing."},
        "monitor":adaptation,"signed_update_inspection":inspection,
        "initial_script_sha256":md_sim_firmware::prepare::hash_bytes(initial.as_bytes()),
        "resumed_script_sha256":md_sim_firmware::prepare::hash_bytes(resumed.as_bytes()),
        "scope":"Keep-file configuration persists. The monitor adaptation applies only before installation. Later boots require the original monitor and four archived boot files."});
    save_json(&out.join("manifest.json"), &manifest)?;
    Ok(Prepared {
        initial,
        resumed,
        manifest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn untrusted_member_names_and_hash_text_cannot_become_shell_commands() {
        let mut inspection = json!({"baseline_comparisons":{}});
        for (member, name) in [
            ("spl", "boot.bin"),
            ("uboot", "u-boot.img"),
            ("fit", "fit.itb"),
            ("fpga", "system.bit.gz"),
        ] {
            inspection["baseline_comparisons"][member] = json!({"identical":true,"baseline_file":name,
                "baseline_object":{"sha256":"ab".repeat(32),"size":1}});
        }
        assert!(boot_checks(&inspection).is_ok());
        for bad in ["../boot.bin", "boot.bin; reboot", "boot.bin\nreboot"] {
            let mut changed = inspection.clone();
            changed["baseline_comparisons"]["spl"]["baseline_file"] = json!(bad);
            assert!(boot_checks(&changed).is_err());
        }
        for bad in ["'$(reboot)'", &"AB".repeat(32), &"a".repeat(63)] {
            let mut changed = inspection.clone();
            changed["baseline_comparisons"]["spl"]["baseline_object"]["sha256"] = json!(bad);
            assert!(boot_checks(&changed).is_err());
        }
        inspection["baseline_comparisons"]["spl"]["identical"] = json!(false);
        assert!(boot_checks(&inspection).is_err());
    }
}

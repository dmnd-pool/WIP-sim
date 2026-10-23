//! Explicit native-script adaptation for the two-PWM reference environment.
//! The archived monitor writes offsets20/24. This model exposes offsets16/20.
//! This does not establish the register map of the selected2022 FPGA image.
use anyhow::{ensure, Result};
use md_sim_runtime::{isolation::save_json, store::hash_file};
use serde_json::{json, Value};
use std::{fs, path::Path};

const ORIGINAL: &str = "5dd1adc64ed54033c2b9ed6c1998de31c744d133fbb4bfc77f11bef30b3f2a1f";
const BEFORE: &str = "for i = 1, N_FANS do";
const AFTER: &str = "for i = 0, N_FANS - 1 do";
const MEMBER: &str = "usr/sbin/bosminer_monitor.lua";

fn adapt(bytes: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        bytes.len() == 11880 && md_sim_firmware::prepare::hash_bytes(bytes) == ORIGINAL,
        "native monitor source identity differs"
    );
    let source = std::str::from_utf8(bytes)?;
    ensure!(
        source.matches(BEFORE).count() == 1 && !source.contains(AFTER),
        "monitor patch context differs"
    );
    Ok(source.replacen(BEFORE, AFTER, 1).into_bytes())
}

pub fn prepare(source: &Path, out: &Path) -> Result<Value> {
    ensure!(
        fs::metadata(source)?.len() == 11880,
        "native monitor input size differs"
    );
    let bytes = fs::read(source)?;
    let changed = adapt(&bytes)?;
    let derived = out.join("adapted-bosminer-monitor.lua");
    fs::write(&derived, &changed)?;
    let output = hash_file(&derived)?;
    let patch = out.join("monitor-fan-index.patch");
    fs::write(&patch, format!("--- original/{MEMBER}\n+++ adapted/{MEMBER}\n@@ -212,3 +212,3 @@\n \tfunction self:set_fans(duty)\n-\t\t{BEFORE}\n+\t\t{AFTER}\n \t\t\tfan_set_duty(i, duty)\n"))?;
    let script = format!(
        r#"(
set -eu
[ -z "$(pidof bosminer || true)" ]
identity=$(sha256sum /{MEMBER})
case "$identity" in
  "{ORIGINAL}  /{MEMBER}")
    sed 's/{BEFORE}/{AFTER}/' /{MEMBER} > /tmp/md-sim-monitor-adapted
    [ "$(sha256sum /tmp/md-sim-monitor-adapted)" = "{sha}  /tmp/md-sim-monitor-adapted" ]
    cp /tmp/md-sim-monitor-adapted /{MEMBER}
    chmod 755 /{MEMBER}
    ;;
  "{sha}  /{MEMBER}") ;;
  *) printf '\nMD_SIM_MONITOR_SOURCE_IDENTITY_FAILED\n'; exit 1 ;;
esac
[ "$(sha256sum /{MEMBER})" = "{sha}  /{MEMBER}" ]
printf '\nMD_SIM_MONITOR_FAN_INDEX_ADAPTATION original={ORIGINAL} adapted={sha} physical_fpga_map=unverified\n'
sha256sum /{MEMBER}
)
"#,
        sha = output.sha256
    );
    fs::write(out.join("monitor-adaptation-script.sh"), script)?;
    let manifest = json!({"schema":"md-sim-s9-monitor-fan-index-adaptation/1",
        "original":{"sha256":ORIGINAL,"size":11880},"member":MEMBER,
        "parent_ramdisk":{"sha256":"2bd49c70b915b7e25f6c01013d95a5ffea8d499b73c6c6ec5539fda4247154b3","size":15491980},
        "derived":output,"patch":hash_file(&patch)?,"application_script":hash_file(&out.join("monitor-adaptation-script.sh"))?,
        "reason":"Original073 stopped the reference machine when the native safety monitor wrote unsupported offset24 after a real native miner stop.",
        "original_offsets":[20,24],"adapted_offsets":[16,20],"native_script_bytes_changed":true,
        "scope":"Patched native safety script in the explicit two-PWM reference environment. Monitoring and protective fan writes still run. This is not original-script execution, selected2022 FPGA equivalence, or a physical thermal-protection pass."});
    save_json(&out.join("monitor-fan-adaptation.json"), &manifest)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_or_missing_source_cannot_be_adapted() {
        assert!(adapt(&[]).is_err());
        let mut bytes = vec![b' '; 11880];
        bytes[..BEFORE.len()].copy_from_slice(BEFORE.as_bytes());
        assert!(adapt(&bytes).is_err());
    }
}

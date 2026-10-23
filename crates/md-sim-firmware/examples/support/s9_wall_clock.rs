//! Explicit wall-clock adaptation for a private miner and its support VM.
//! This never changes CLOCK_MONOTONIC, instruction clocks or device timers.
use anyhow::{ensure, Result};
use serde_json::{json, Value};
use std::time::Instant;

pub const ENVIRONMENT: &str = "MD_SIM_S9_WALL_CLOCK_EPOCH";

pub fn validate(epoch: u64) -> Result<u64> {
    // The archived ARM userspace has a signed 32-bit time_t. Leave room for
    // the complete bounded run, including support VM startup.
    ensure!(
        (1_231_006_505..=2_147_482_000).contains(&epoch),
        "wall-clock epoch is outside the supported ARM time range"
    );
    Ok(epoch)
}

pub fn identity(epoch: u64) -> Value {
    json!({"kind":"explicit-shared-wall-clock-environment-adaptation",
        "epoch_unix_seconds":epoch,
        "support":"Captured QEMU RTC starts at the selected UTC epoch and uses its host clock.",
        "miner":"The original guest date command sets CLOCK_REALTIME after console activation to the epoch plus supervisor monotonic elapsed time. The guest must read back that value.",
        "monotonic_and_device_clocks_changed":false,"firmware_bytes_changed":false,
        "ntp_tested":false,"physical_rtc_claim":false,
        "limits":"Serial delivery, guest scheduling and emulation can produce measured clock skew. This is a recorded environment adaptation, not a model of a physical RTC or network time service."})
}

pub struct Clock {
    epoch: u64,
    started: Instant,
}
impl Clock {
    pub fn new(epoch: u64) -> Result<Self> {
        Ok(Self {
            epoch: validate(epoch)?,
            started: Instant::now(),
        })
    }

    pub fn rtc(&self) -> Result<String> {
        let seconds = libc::time_t::try_from(self.epoch)?;
        let mut utc: libc::tm = unsafe { std::mem::zeroed() };
        ensure!(
            !unsafe { libc::gmtime_r(&seconds, &mut utc) }.is_null(),
            "UTC conversion failed"
        );
        Ok(format!(
            "base={:04}-{:02}-{:02}T{:02}:{:02}:{:02},clock=host,driftfix=none",
            utc.tm_year + 1900,
            utc.tm_mon + 1,
            utc.tm_mday,
            utc.tm_hour,
            utc.tm_min,
            utc.tm_sec
        ))
    }

    pub fn script(&self) -> Result<Vec<u8>> {
        let elapsed = self.started.elapsed().as_secs();
        ensure!(elapsed <= 1024, "clock adaptation exceeded run bound");
        let target = self.epoch + elapsed;
        // All interpolated values are bounded integers. BusyBox executes in
        // the original guest, never on the host. Readback failure is fatal.
        let script = format!(
            "(\nset -eu\n[ -z \"$(pidof boser bosminer || true)\" ]\nprintf '\\nMD_SIM_WALL_CLOCK_BEFORE\\n'\ndate -u +%s\ncat /proc/uptime\ndate -u -s @{target}\nactual=$(date -u +%s)\n[ \"$actual\" -ge {target} ]\n[ \"$actual\" -le {} ]\nprintf '\\nMD_SIM_WALL_CLOCK_APPLIED epoch={} elapsed={elapsed} target={target} actual=%s\\n' \"$actual\"\ncat /proc/uptime\n)\n",
            target + 5, self.epoch
        );
        println!(
            "MD_SIM_WALL_CLOCK_REQUEST {}",
            json!({"policy":identity(self.epoch),
            "elapsed_seconds":elapsed,"requested_unix_seconds":target,
            "script_sha256":md_sim_firmware::prepare::hash_bytes(script.as_bytes()),
            "readback_maximum_seconds":target+5,"applied":false})
        );
        Ok(script.into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_epoch_is_explicit_utc_and_bounded_by_original_userspace() {
        assert_eq!(
            Clock::new(1_231_006_505).unwrap().rtc().unwrap(),
            "base=2009-01-03T18:15:05,clock=host,driftfix=none"
        );
        assert_eq!(
            Clock::new(1_709_164_800).unwrap().rtc().unwrap(),
            "base=2024-02-29T00:00:00,clock=host,driftfix=none"
        );
        for epoch in [0, 1_231_006_504, 2_147_482_001, u64::MAX] {
            assert!(Clock::new(epoch).is_err());
        }
        let record = identity(1_709_164_800);
        assert_eq!(record["monotonic_and_device_clocks_changed"], false);
        assert_eq!(record["ntp_tested"], false);
    }
}

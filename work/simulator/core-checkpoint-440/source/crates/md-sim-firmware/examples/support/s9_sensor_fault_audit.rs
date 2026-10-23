//! Native sensor error detection and explicit service-restart recovery.
use super::{Chain, InputEvent, SensorSample};
use anyhow::{ensure, Context, Result};
use md_sim_firmware::tmp421::Diode;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Case {
    OpenDiode,
    ShortDiode,
    InvalidSupply,
}

pub(super) fn scenario(events: &[InputEvent]) -> Result<Option<Case>> {
    ensure!(events.len() == 2, "requires a fault and a restoration edge");
    let first = events[0].inputs;
    let last = events[1].inputs;
    ensure!(
        first.local_millicelsius == 25000
            && last.local_millicelsius == 25000
            && last.remote_millicelsius == 65000
            && last.supply_valid
            && last.diode == Diode::Connected,
        "unsupported sensor baseline or restoration"
    );
    if first.supply_valid
        && first.diode == Diode::Connected
        && [105000, 115000].contains(&first.remote_millicelsius)
    {
        return Ok(None);
    }
    ensure!(
        first.remote_millicelsius == 65000,
        "fault case changes the temperature input"
    );
    ensure!(
        events[0].at_ns == 150_000_000_000 && events[1].at_ns == 180_000_000_000,
        "manual recovery suite requires the declared 150s/180s input edges"
    );
    match (first.diode, first.supply_valid) {
        (Diode::Open, true) => Ok(Some(Case::OpenDiode)),
        (Diode::Short, true) => Ok(Some(Case::ShortDiode)),
        (Diode::Connected, false) => Ok(Some(Case::InvalidSupply)),
        _ => anyhow::bail!("unsupported sensor fault combination"),
    }
}

fn marker(guest: &str, prefix: &str) -> Result<(u64, u32)> {
    framed_marker(guest, prefix, "MD_SIM_SENSOR_RECOVERY_")
}

pub(super) fn framed_marker(guest: &str, prefix: &str, family: &str) -> Result<(u64, u32)> {
    let lines: Vec<_> = guest
        .lines()
        .filter_map(|l| l.strip_prefix(prefix))
        .collect();
    ensure!(
        lines.len() == 1,
        "missing or repeated manual recovery marker"
    );
    // The guest console also carries logread output. A log line can follow
    // the record before its newline, as captured in diode-open180. Require
    // an explicit end delimiter; never guess where a damaged PID ends.
    let (record, suffix) = lines[0]
        .split_once(&format!(":{family}MARKER_END"))
        .context("recovery marker end absent")?;
    ensure!(
        !suffix.contains(family),
        "multiple recovery records on one line"
    );
    let (time, pid) = record.split_once(':').context("recovery PID absent")?;
    let (seconds, fraction) = time.split_once('.').context("recovery uptime format")?;
    ensure!(
        !seconds.is_empty()
            && seconds.bytes().all(|b| b.is_ascii_digit())
            && !pid.is_empty()
            && pid.bytes().all(|b| b.is_ascii_digit()),
        "invalid recovery seconds or PID"
    );
    ensure!(
        !fraction.is_empty() && fraction.len() <= 9 && fraction.bytes().all(|b| b.is_ascii_digit()),
        "invalid uptime fraction"
    );
    let ns = seconds
        .parse::<u64>()?
        .checked_mul(1_000_000_000)
        .and_then(|v| {
            v.checked_add(fraction.parse::<u64>().ok()? * 10_u64.pow(9 - fraction.len() as u32))
        })
        .context("recovery uptime overflow")?;
    let pid = pid.parse::<u32>()?;
    ensure!(pid > 0, "recovery process is absent");
    Ok((ns, pid))
}

/// Tie native chain initialization to the new process, in console order.
/// Guest uptime and QEMU device timestamps have distinct clock origins.
pub(super) fn restarted_chains(guest: &str, family: &str, pid: u32) -> Result<Vec<Value>> {
    let lines: Vec<_> = guest.lines().collect();
    let begin = lines
        .iter()
        .position(|l| l.starts_with(&format!("{family}RESTART_BEGIN:")))
        .context("restart begin absent")?;
    let end = lines
        .iter()
        .position(|l| l.starts_with(&format!("{family}RESTART_END:")))
        .context("restart end absent")?;
    ensure!(end > begin, "restart markers are out of console order");
    let process = format!(" daemon.err bosminer[{pid}]: ");
    (6..=8)
        .map(|link| {
            let text = format!(
                " INFO bosminer_am1_s9::hashchain: Initializing hash chain {link}, (difficulty 64)"
            );
            let (index, line) = lines
                .iter()
                .enumerate()
                .skip(begin + 1)
                .find(|(_, line)| line.contains(&process) && line.ends_with(&text))
                .with_context(|| format!("new native process did not initialize chain{link}"))?;
            Ok(json!({"link":link,"line":index+1,"message":line}))
        })
        .collect()
}

fn healthy(sample: &SensorSample, channel: u8) -> bool {
    sample.channel == channel
        && sample.valid()
        && sample.units_sixteenths_celsius == if channel == 0 { 25 * 16 } else { 65 * 16 }
}

fn fault(sample: &SensorSample, case: Case) -> bool {
    match case {
        Case::OpenDiode => sample.channel == 1 && sample.open && !sample.invalid && !sample.short,
        Case::ShortDiode => {
            sample.channel == 1
                && sample.short
                && !sample.invalid
                && !sample.open
                && sample.units_sixteenths_celsius == -64 * 16
        }
        Case::InvalidSupply => sample.invalid && !sample.open && !sample.short,
    }
}

pub(super) fn assess(
    chains: &BTreeMap<u32, Chain>,
    events: &[InputEvent],
    guest: &str,
    case: Case,
) -> Result<Value> {
    let (restart_ns, old_pid) = marker(guest, "MD_SIM_SENSOR_RECOVERY_RESTART_BEGIN:")?;
    let (restarted_ns, new_pid) = marker(guest, "MD_SIM_SENSOR_RECOVERY_RESTART_END:")?;
    let start = events[0].at_ns;
    let restored = events[1].at_ns;
    ensure!(
        restart_ns >= 185_000_000_000 && restarted_ns >= restart_ns && old_pid != new_pid,
        "native recovery did not restart after its declared uptime threshold"
    );
    let native_restart = restarted_chains(guest, "MD_SIM_SENSOR_RECOVERY_", new_pid)?;
    ensure!(
        chains.len() == 3,
        "sensor recovery requires all three reference chains"
    );
    let mut results = Vec::new();
    // The transmitted shell source can also contain the marker text. Only
    // an emitted record at the start of a line establishes this boundary.
    let before_restart: Vec<_> = guest
        .lines()
        .take_while(|line| !line.starts_with("MD_SIM_SENSOR_RECOVERY_RESTART_BEGIN:"))
        .collect();
    for (&link, chain) in chains {
        let expected = format!("bosminer_hal::sensor: Sensor hb{link}[ii_hwmon::tmp42x::TMP42x]: Too many sensor errors, disabling sensor");
        let errors: Vec<_> = before_restart
            .iter()
            .copied()
            .filter(|l| l.contains(&expected))
            .collect();
        ensure!(
            !errors.is_empty(),
            "native sensor error response absent for chain{link}"
        );
        let cut = chain
            .power_edges
            .iter()
            .find(|&&(ns, on)| !on && ns > restored)
            .context("recovery did not cut chain power after input restoration")?;
        let powered = chain
            .power_edges
            .iter()
            .rev()
            .find(|&&(ns, on)| on && ns > cut.0)
            .context("recovery did not restore chain power")?;
        let mut channels = Vec::new();
        for channel in 0..=1 {
            let before = chain
                .sensor_samples
                .iter()
                .find(|s| s.low_ns < start && healthy(s, channel))
                .context("initial healthy sensor read absent")?;
            let during = chain
                .sensor_samples
                .iter()
                .find(|s| {
                    s.channel == channel
                        && s.high_ns >= start
                        && s.low_ns < restored
                        && if matches!(case, Case::OpenDiode | Case::ShortDiode) && channel == 0 {
                            healthy(s, channel)
                        } else {
                            fault(s, case)
                        }
                })
                .context("programmed sensor fault was not returned to the native driver")?;
            let after = chain
                .sensor_samples
                .iter()
                .find(|s| s.high_ns > powered.0 && healthy(s, channel))
                .context("healthy paired sensor read after manual restart absent")?;
            ensure!(
                chain.hashes.iter().any(|&ns| ns < start)
                    && chain.hashes.iter().any(|&ns| ns > after.low_ns),
                "no computation before fault and after recovery"
            );
            channels.push(
                json!({"channel":channel,"before":before,"fault_interval":during,"after":after}),
            );
        }
        let automatic_reads = chain
            .sensor_samples
            .iter()
            .filter(|s| {
                s.channel == 1 && s.high_ns >= restored && s.low_ns < cut.0 && healthy(s, 1)
            })
            .count();
        results.push(
            json!({"link":link,"channels":channels,"native_error_messages":errors,
            "healthy_remote_reads_between_input_restore_and_recovery_power_cut":automatic_reads,
            "power_edges":chain.power_edges,"computed_hashes":chain.hashes.len(),
            "paired_sensor_reads":chain.sensor_samples,"no_computation_while_inhibited":true}),
        );
    }
    Ok(
        json!({"passed":true,"case":case,"recovery":"explicit-native-service-restart",
        "restart_begin_guest_uptime_ns":restart_ns,"restart_end_guest_uptime_ns":restarted_ns,"old_pid":old_pid,"new_pid":new_pid,
        "new_process_initialization":native_restart,
        "clock_scope":"Restart markers use native Linux uptime. Device events use QEMU virtual time. The verifier does not treat their origins as equal. The exact script requires uptime185s before restart; device recovery requires a power cycle and healthy paired reads after the180s input restoration.",
        "chains":results,"scope":"Actual archived native code in the adapted reference environment. CRC-checked paired sensor reads include the fault bits. The original service command restarts the miner after external input restoration. This tests error detection and manual recovery. It does not grant automatic recovery, protective shutdown, physical calibration, exact-board, mining or Core acceptance."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native_initialization(pid: u32) -> String {
        (6..=8).map(|link|format!("Wed Oct 21 20:13:40 2026 daemon.err bosminer[{pid}]: Oct 21 20:13:40.401  INFO bosminer_am1_s9::hashchain: Initializing hash chain {link}, (difficulty 64)\n")).collect()
    }

    #[test]
    fn recovery_logs_must_follow_restart_and_belong_to_the_new_process() -> Result<()> {
        let begin = "MD_SIM_FAN_RECOVERY_RESTART_BEGIN:200.00:42:MD_SIM_FAN_RECOVERY_MARKER_END\n";
        let end = "MD_SIM_FAN_RECOVERY_RESTART_END:201.00:43:MD_SIM_FAN_RECOVERY_MARKER_END\n";
        let family = "MD_SIM_FAN_RECOVERY_";
        let logs = native_initialization(43);
        assert_eq!(
            restarted_chains(&format!("{begin}{end}{logs}"), family, 43)?.len(),
            3
        );
        // Initialization can start before the shell observes the replacement.
        restarted_chains(&format!("{begin}{logs}{end}"), family, 43)?;
        for invalid in [
            format!("{end}{begin}{logs}"),
            format!("{logs}{begin}{end}"),
            format!("{begin}{end}{}", native_initialization(42)),
            format!("{begin}{end}{}", logs.replace("chain 7,", "chain 9,")),
        ] {
            assert!(restarted_chains(&invalid, family, 43).is_err());
        }
        Ok(())
    }

    #[test]
    fn recovery_markers_require_bounded_time_and_one_process() -> Result<()> {
        assert_eq!(
            marker(
                "MD_SIM_SENSOR_RECOVERY_RESTART_BEGIN:185.01:42:MD_SIM_SENSOR_RECOVERY_MARKER_END\n",
                "MD_SIM_SENSOR_RECOVERY_RESTART_BEGIN:"
            )
            .unwrap(),
            (185_010_000_000, 42)
        );
        for body in [
            "185:42",
            "185.xx:42",
            "185.0000000001:42",
            "185.01:0",
            "185.01:42 43",
            "+185.01:42",
            "185.01:+42",
            "18446744073709551615.1:42",
        ] {
            assert!(marker(
                &format!("begin:{body}:MD_SIM_SENSOR_RECOVERY_MARKER_END"),
                "begin:"
            )
            .is_err());
        }
        assert!(marker("begin:185.00:42:MD_SIM_SENSOR_RECOVERY_MARKER_END\nbegin:185.00:42:MD_SIM_SENSOR_RECOVERY_MARKER_END", "begin:").is_err());
        assert!(marker("begin:185.00:42", "begin:").is_err());
        assert!(marker(
            "begin:211.41:1964Wed Oct 21 20:13:37 2026 daemon.err bosminer[1964]: message",
            "begin:"
        )
        .is_err());
        assert_eq!(marker("begin:185.01:42:MD_SIM_SENSOR_RECOVERY_MARKER_ENDWed Oct 21 20:13:37 2026 daemon.err bosminer[42]: message", "begin:")?, (185_010_000_000,42));
        assert!(marker(
            "begin:185.01:42:MD_SIM_SENSOR_RECOVERY_MARKER_END:MD_SIM_SENSOR_RECOVERY_MARKER_END",
            "begin:"
        )
        .is_err());
        Ok(())
    }

    #[test]
    fn fault_flags_cannot_be_treated_as_valid_temperature() {
        let mut read = SensorSample {
            high_ns: 1,
            low_ns: 2,
            channel: 1,
            units_sixteenths_celsius: 65 * 16,
            invalid: false,
            open: false,
            short: false,
            ..SensorSample::default()
        };
        assert!(healthy(&read, 1));
        read.open = true;
        assert!(!healthy(&read, 1));
        assert!(fault(&read, Case::OpenDiode));
        read.open = false;
        read.invalid = true;
        assert!(!healthy(&read, 1));
        assert!(fault(&read, Case::InvalidSupply));
        assert!(!fault(&read, Case::OpenDiode));
        read.invalid = false;
        read.short = true;
        assert!(!healthy(&read, 1));
        assert!(!fault(&read, Case::ShortDiode));
        read.units_sixteenths_celsius = -64 * 16;
        assert!(fault(&read, Case::ShortDiode));
        read.open = true;
        assert!(!fault(&read, Case::ShortDiode));
        assert!(!fault(&read, Case::OpenDiode));
    }

    #[test]
    fn manual_recovery_requires_each_native_error_fault_read_and_power_cycle() -> Result<()> {
        let events: Vec<InputEvent> = serde_json::from_str(
            r#"[
            {"at_ns":150000000000,"inputs":{"local_millicelsius":25000,"remote_millicelsius":65000,"diode":"Open","supply_valid":true}},
            {"at_ns":180000000000,"inputs":{"local_millicelsius":25000,"remote_millicelsius":65000,"diode":"Connected","supply_valid":true}}
        ]"#,
        )?;
        assert!(matches!(scenario(&events)?, Some(Case::OpenDiode)));
        let guest = (6..=8).map(|link|format!("bosminer_hal::sensor: Sensor hb{link}[ii_hwmon::tmp42x::TMP42x]: Too many sensor errors, disabling sensor\n")).collect::<String>()
            + "MD_SIM_SENSOR_RECOVERY_RESTART_BEGIN:200.00:42:MD_SIM_SENSOR_RECOVERY_MARKER_END\nMD_SIM_SENSOR_RECOVERY_RESTART_END:201.00:43:MD_SIM_SENSOR_RECOVERY_MARKER_END\n"
            + &native_initialization(43);
        let sample = |seconds: u64, channel, open| SensorSample {
            high_ns: seconds * 1_000_000_000,
            low_ns: seconds * 1_000_000_000 + 1,
            channel,
            units_sixteenths_celsius: if channel == 0 { 25 * 16 } else { 65 * 16 },
            invalid: false,
            open,
            short: false,
            ..SensorSample::default()
        };
        let mut chains: BTreeMap<_, _> = (6..=8)
            .map(|link| {
                (
                    link,
                    Chain {
                        sensor_samples: vec![
                            sample(100, 0, false),
                            sample(100, 1, false),
                            sample(155, 0, false),
                            sample(155, 1, true),
                            sample(220, 0, false),
                            sample(220, 1, false),
                        ],
                        hashes: vec![110_000_000_000, 221_000_000_000],
                        power_edges: vec![
                            (90_000_000_000, true),
                            (202_000_000_000, false),
                            (210_000_000_000, true),
                        ],
                        ..Chain::default()
                    },
                )
            })
            .collect();
        assert!(assess(&chains, &events, &guest, Case::OpenDiode).is_ok());
        let echoed = "printf 'MD_SIM_SENSOR_RECOVERY_RESTART_BEGIN:%s:%s'\n".to_owned() + &guest;
        assert!(assess(&chains, &events, &echoed, Case::OpenDiode).is_ok());
        let mut short_events = events.clone();
        short_events[0].inputs.diode = Diode::Short;
        assert!(matches!(scenario(&short_events)?, Some(Case::ShortDiode)));
        // A shorted diode is encoded as high=0, low=0 in extended range.
        // It does not set the open-diode or invalid-supply flag.
        for chain in chains.values_mut() {
            chain.sensor_samples[3].open = false;
            chain.sensor_samples[3].short = true;
            chain.sensor_samples[3].units_sixteenths_celsius = -64 * 16;
        }
        assert!(assess(&chains, &short_events, &guest, Case::ShortDiode).is_ok());
        assert!(assess(&chains, &short_events, &guest, Case::OpenDiode).is_err());
        for chain in chains.values_mut() {
            chain.sensor_samples[3].open = true;
            chain.sensor_samples[3].short = false;
            chain.sensor_samples[3].units_sixteenths_celsius = 65 * 16;
        }
        assert!(assess(
            &chains,
            &events,
            &guest.replace("201.00:43", "201.00:42"),
            Case::OpenDiode
        )
        .is_err());
        assert!(assess(
            &chains,
            &events,
            &guest.replace("Sensor hb7", "Sensor hb9"),
            Case::OpenDiode
        )
        .is_err());
        chains.get_mut(&7).unwrap().sensor_samples[3].open = false;
        assert!(assess(&chains, &events, &guest, Case::OpenDiode).is_err());
        chains.get_mut(&7).unwrap().sensor_samples[3].open = true;
        chains.get_mut(&7).unwrap().hashes.pop();
        assert!(assess(&chains, &events, &guest, Case::OpenDiode).is_err());
        chains.get_mut(&7).unwrap().hashes.push(221_000_000_000);
        chains.get_mut(&7).unwrap().power_edges[1].0 = 179_000_000_000;
        assert!(assess(&chains, &events, &guest, Case::OpenDiode).is_err());
        Ok(())
    }
}

//! Native fan fault detection, initial power shutdown, and explicit recovery.
use super::{fan_audit, sensor_fault, traffic};
use anyhow::{ensure, Context, Result};
use md_sim_firmware::fan::{validate_input_plan, InputEvent};
use md_sim_runtime::store::hash_file;
use serde_json::{json, Value};
use std::path::Path;

fn healthy_sensor_reads(chain: &super::Chain) -> Result<Value> {
    // TI SBOS398C p11: range encoding changes at the next conversion.
    // This independent arithmetic check uses only the pinned reference
    // timing and observed power/configuration transactions, not Sensor.
    let profile: Value = serde_json::from_slice(include_bytes!(
        "../../../../simulator/backends/qemu/s9-tmp421-reference.json"
    ))?;
    ensure!(
        profile["bus"]["clock_hz"] == 100000
            && profile["bus"]["timing"]
                == json!({"power_start_ns":50000,
            "conversion_ns":115000000,"channel_slot_ns":125000000})
            && profile["bus"]["inputs"]
                == json!({"local_millicelsius":25000,
            "remote_millicelsius":65000,"diode":"Connected","supply_valid":true}),
        "reference profile differs from the independent conversion oracle"
    );
    ensure!(
        chain.sensor_write_commands.len() == chain.sensor_writes.len(),
        "sensor write lacks a completed readback"
    );
    for ((at, word), write) in chain.sensor_write_commands.iter().zip(&chain.sensor_writes) {
        ensure!(
            *at == write.applied_ns
                && *word == (0x0199_0000 | u32::from(write.register) << 8 | u32::from(write.value))
                && matches!((write.register, write.value), (9, 4) | (10, 0x1c)),
            "fan test changed the sensor conversion policy"
        );
    }
    let mut transitions = Vec::new();
    for sample in &chain.sensor_samples {
        ensure!(
            sample.channel <= 1 && sample.valid() && sample.low_byte == 0,
            "unrelated sensor fault or fractional input"
        );
        let power = chain
            .power_edges
            .iter()
            .rev()
            .find(|&&(ns, _)| ns <= sample.high_acquired_ns)
            .context("sensor sample lacks power history")?;
        ensure!(power.1, "sensor read while unpowered");
        let first = power
            .0
            .checked_add(50_000 + u64::from(sample.channel) * 125_000_000 + 115_000_000)
            .context("sensor conversion clock overflow")?;
        ensure!(
            sample.high_acquired_ns >= first,
            "sensor read before initial conversion"
        );
        let start =
            first - 115_000_000 + (sample.high_acquired_ns - first) / 250_000_000 * 250_000_000;
        let extended_at = |ns| {
            chain
                .sensor_writes
                .iter()
                .any(|w| w.register == 9 && w.completed_ns >= power.0 && w.completed_ns <= ns)
        };
        let degrees = if sample.channel == 0 { 25 } else { 65 };
        let expected_byte = degrees + if extended_at(start) { 64 } else { 0 };
        let interpreted = i16::from(expected_byte)
            - if extended_at(sample.high_acquired_ns) {
                64
            } else {
                0
            };
        ensure!(
            sample.high_byte == expected_byte
                && sample.units_sixteenths_celsius == interpreted * 16,
            "sensor bytes differ from completed conversion: {}",
            serde_json::to_string(sample)?
        );
        if interpreted != i16::from(degrees) {
            transitions.push(json!({"sample":sample,"conversion_start_ns":start,
                "physical_input_celsius":degrees,"native_interpretation_celsius":interpreted}));
        }
    }
    Ok(
        json!({"checked_samples":chain.sensor_samples.len(),"range_transition_samples":transitions,
        "writes":chain.sensor_writes,
        "timing_scope":"Pinned reference: power startup50us, conversion115ms, two125ms channel slots. This is not physical calibration.",
        "source":"TI TMP421 SBOS398C page11: range format takes effect on conversion. The native transient is preserved."}),
    )
}

fn scenario(events: &[InputEvent]) -> Result<&'static str> {
    validate_input_plan(events, 2).map_err(|e| anyhow::anyhow!("fan input: {e:?}"))?;
    ensure!(
        events.len() == 2 && events.iter().all(|e| e.motor == 1),
        "requires fault and restore on the connected reference motor"
    );
    match (
        events[0].powered,
        events[0].stalled,
        events[1].powered,
        events[1].stalled,
    ) {
        (None, Some(true), None, Some(false)) => Ok("locked-rotor"),
        (Some(false), None, Some(true), None) => Ok("motor-power-loss"),
        _ => anyhow::bail!("unsupported fan fault combination"),
    }
}

fn applied(log: &str, events: &[InputEvent]) -> Result<()> {
    let expected: Vec<_> = events
        .iter()
        .map(|e| {
            let mask = u32::from(e.powered.is_some()) | (u32::from(e.stalled.is_some()) << 1);
            let values = u32::from(e.powered.unwrap_or(false))
                | (u32::from(e.stalled.unwrap_or(false)) << 1);
            format!(
                "md_sim_fan_input ns={} motor={} mask={mask} values={values}",
                e.at_ns, e.motor
            )
        })
        .collect();
    ensure!(
        log.lines()
            .filter(|l| l.starts_with("md_sim_fan_input "))
            .eq(expected.iter().map(String::as_str)),
        "applied fan input edges differ"
    );
    Ok(())
}

pub(super) fn analyze(run: &Path) -> Result<Value> {
    let (backend, guest) = super::super::submission_audit::completed_logs(run)?;
    let read = |name: &str| -> Result<Value> {
        Ok(serde_json::from_str(&super::super::read(&run.join(name))?)?)
    };
    let adaptation = read("adaptation.json")?;
    let spec = read("process-spec.json")?;
    let console = read("console.json")?;
    let legacy = &adaptation["legacy_io"];
    let events: Vec<InputEvent> = serde_json::from_str(&super::super::read_bounded(
        &run.join("fan-plan-original.json"),
        4096,
    )?)?;
    let case = scenario(&events)?;
    let plan = &legacy["fan_input_plan"];
    ensure!(
        plan["schema"] == "md-sim-fan-input-plan/1"
            && plan["source"]
                == serde_json::to_value(hash_file(&run.join("fan-plan-original.json"))?)?
            && plan["events"] == serde_json::to_value(&events)?,
        "fan plan identity differs"
    );
    let argv = spec["argv"]
        .as_array()
        .context("process arguments absent")?;
    let wanted = format!("md-sim-fan.input-plan={}", serde_json::to_string(&events)?);
    let plans: Vec<_> = argv
        .iter()
        .filter_map(Value::as_str)
        .filter(|s| s.starts_with("md-sim-fan.input-plan="))
        .collect();
    ensure!(plans == [wanted.as_str()], "executed fan plan differs");
    let without_plan: Vec<_> = argv
        .iter()
        .filter(|v| v.as_str() != Some(wanted.as_str()))
        .cloned()
        .collect();
    fan_audit::fixture(legacy, &without_plan)?;
    ensure!(
        legacy["temperature_fixture"] == true
            && legacy["asic_contract"]["temperature_fixture"]["profile"]["sha256"]
                == md_sim_firmware::prepare::hash_bytes(include_bytes!(
                    "../../../../simulator/backends/qemu/s9-tmp421-reference.json"
                ))
            && !argv
                .iter()
                .filter_map(Value::as_str)
                .any(|s| s.starts_with("md-sim-s9io.temperature-input-plan=")),
        "fan recovery requires unchanged healthy sensor inputs"
    );
    let script = include_bytes!("../../../../simulator/scenarios/s9-fan-manual-recovery.sh");
    let identity =
        json!({"sha256":md_sim_firmware::prepare::hash_bytes(script),"size":script.len()});
    let scripts: Vec<_> = spec["inputs"]
        .as_array()
        .context("process inputs absent")?
        .iter()
        .filter(|v| v["guest_path"] == "/console-script")
        .collect();
    ensure!(
        console["script"] == identity && scripts.len() == 1 && scripts[0]["object"] == identity,
        "manual fan recovery script differs"
    );
    applied(&backend, &events)?;
    let accesses = fan_audit::accesses(&backend)?;
    let start = events[0].at_ns;
    let restore = events[1].at_ns;
    let before = accesses
        .iter()
        .rev()
        .find(|a| !a.write && a.address == 4 && a.ns < start && a.value > 0)
        .context("initial motor motion absent")?;
    let zero = accesses
        .iter()
        .find(|a| !a.write && a.address == 4 && a.ns >= start && a.ns < restore && a.value == 0)
        .context("native firmware did not read the stopped tachometer")?;
    ensure!(
        accesses
            .iter()
            .filter(|a| !a.write
                && a.address == 4
                && a.ns >= start + 2_000_000_000
                && a.ns < restore)
            .all(|a| a.value == 0),
        "stopped motor returned motion after its sample window settled"
    );
    let after = accesses
        .iter()
        .find(|a| !a.write && a.address == 4 && a.ns >= restore + 2_000_000_000 && a.value > 0)
        .context("native firmware did not read restored motor motion")?;
    let marker = |suffix| {
        sensor_fault::framed_marker(
            &guest,
            &format!("MD_SIM_FAN_RECOVERY_{suffix}:"),
            "MD_SIM_FAN_RECOVERY_",
        )
    };
    let (restart_ns, old_pid) = marker("RESTART_BEGIN")?;
    let (restarted_ns, new_pid) = marker("RESTART_END")?;
    ensure!(
        restart_ns >= 185_000_000_000 && restarted_ns >= restart_ns && old_pid != new_pid,
        "native manual restart did not meet its uptime and PID contract"
    );
    let native_restart = sensor_fault::restarted_chains(&guest, "MD_SIM_FAN_RECOVERY_", new_pid)?;
    let messages: Vec<_> = guest
        .lines()
        .take_while(|l| !l.starts_with("MD_SIM_FAN_RECOVERY_RESTART_BEGIN:"))
        .filter(|l| {
            l.contains("daemon.err bosminer[")
                && l.contains(
                    "bosminer_hal::monitor: HALT reason: Shutdown: not enough fans (0 < 1)",
                )
        })
        .collect();
    ensure!(
        !messages.is_empty()
            && guest.lines().any(|l| l.contains("Resolved monitor config:")
                && l.contains("min_fans: 1, min_fan_rpm: 0")),
        "native fan threshold or shutdown absent"
    );
    let chains = traffic(&backend)?;
    let mut reports = Vec::new();
    for (link, chain) in &chains {
        let sensor_reads = healthy_sensor_reads(chain)?;
        let first_cut = chain
            .power_edges
            .iter()
            .find(|&&(ns, on)| !on && ns > zero.ns && ns < restore)
            .context("native fan shutdown did not cut every chain during the fault")?;
        let last_cut = chain
            .power_edges
            .iter()
            .rev()
            .find(|&&(ns, on)| !on && ns > restore)
            .context("recovery power cycle after restored motor absent")?;
        let powered = chain
            .power_edges
            .iter()
            .rev()
            .find(|&&(ns, on)| on && ns > last_cut.0)
            .context("chain power after recovery absent")?;
        let healthy = chain
            .sensor_samples
            .iter()
            .find(|s| {
                s.channel == 1
                    && s.high_acquired_ns > powered.0
                    && s.valid()
                    && s.units_sixteenths_celsius == 65 * 16
            })
            .context("healthy sensor read after recovery absent")?;
        ensure!(
            chain.hashes.iter().any(|&ns| ns < start)
                && chain.hashes.iter().any(|&ns| ns > healthy.low_ns),
            "all chains must compute before fault and after recovery"
        );
        reports.push(json!({"link":link,"initial_shutdown":first_cut,"recovery_cut":last_cut,
            "recovery_power":powered,"healthy_after_recovery":healthy,"power_edges":chain.power_edges,
            "computed_hashes":chain.hashes.len(),"no_computation_while_inhibited":true,
            "sensor_reads":sensor_reads,
            "pic_to_asic_transfers":chain.power_trace.finish()?}));
    }
    let mut report = json!({"passed":true,"case":case,"recovery":"explicit-native-service-restart",
        "tach_before":before,"stopped_tach":zero,"restored_tach":after,"chains":reports,"native_shutdown_messages":messages,
        "restart_begin_guest_uptime_ns":restart_ns,"restart_end_guest_uptime_ns":restarted_ns,"old_pid":old_pid,"new_pid":new_pid,
        "new_process_initialization":native_restart,
        "clock_scope":"Fan, ASIC and PIC events use QEMU virtual nanoseconds. Restart markers use guest uptime; their clock origins are not treated as equal.",
        "scope":"Initial native fan-fault shutdown and explicit manual recovery in the reference environment. Native automatic retries remain visible. No continuous safety interlock, automatic recovery, calibrated fan, exact-board, mining or Core gate is granted."});
    for name in [
        "result.json",
        "process-spec.json",
        "adaptation.json",
        "backend.log",
        "guest.log",
        "fan-plan-original.json",
        "console.json",
    ] {
        report["inputs"][name] = serde_json::to_value(hash_file(&run.join(name))?)?;
    }
    report["verifier_source"] = json!({"sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_fan_fault_audit.rs")),"size":include_bytes!("s9_fan_fault_audit.rs").len()});
    report["sensor_transaction_verifier_source"] = json!({"sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_temperature_audit.rs")),"size":include_bytes!("s9_temperature_audit.rs").len()});
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range_change_chain() -> super::super::Chain {
        use super::super::{SensorSample, SensorWrite};
        super::super::Chain {
            power_edges: vec![(0, true)],
            sensor_write_commands: vec![(199_710_000, 0x0199_0904)],
            sensor_writes: vec![SensorWrite {
                applied_ns: 199_710_000,
                completed_ns: 200_000_000,
                register: 9,
                value: 4,
            }],
            sensor_samples: vec![SensorSample {
                high_ns: 301_000_000,
                low_ns: 302_000_000,
                high_acquired_ns: 300_000_000,
                high_byte: 65,
                low_byte: 0,
                channel: 1,
                units_sixteenths_celsius: 16,
                ..SensorSample::default()
            }],
            ..super::super::Chain::default()
        }
    }

    #[test]
    fn range_change_keeps_the_actual_prior_conversion_until_next_completion() -> Result<()> {
        // Remote conversions start at125.05ms and375.05ms, and finish
        // at240.05ms and490.05ms. The range changes at200ms. No model used.
        let mut chain = range_change_chain();
        let report = healthy_sensor_reads(&chain)?;
        assert_eq!(
            report["range_transition_samples"].as_array().unwrap().len(),
            1
        );
        chain.sensor_samples[0].high_acquired_ns = 490_050_000 - 1;
        healthy_sensor_reads(&chain)?;
        chain.sensor_samples[0].high_acquired_ns += 1;
        assert!(healthy_sensor_reads(&chain).is_err());
        chain.sensor_samples[0].high_byte = 129;
        chain.sensor_samples[0].units_sixteenths_celsius = 65 * 16;
        assert!(healthy_sensor_reads(&chain)?["range_transition_samples"]
            .as_array()
            .unwrap()
            .is_empty());
        Ok(())
    }

    #[test]
    fn range_change_cannot_hide_faults_wrong_inputs_or_stale_values() -> Result<()> {
        let mut chain = range_change_chain();
        chain.sensor_samples[0].invalid = true;
        assert!(healthy_sensor_reads(&chain).is_err());
        chain.sensor_samples[0].invalid = false;
        chain.sensor_samples[0].high_byte = 66;
        assert!(healthy_sensor_reads(&chain).is_err());
        chain = range_change_chain();
        chain.sensor_writes.clear();
        assert!(healthy_sensor_reads(&chain).is_err());
        chain.sensor_write_commands.clear();
        assert!(healthy_sensor_reads(&chain).is_err());
        chain = range_change_chain();
        chain.sensor_write_commands.push((599_710_000, 0x0199_0904));
        chain.sensor_writes.push(super::super::SensorWrite {
            applied_ns: 599_710_000,
            completed_ns: 600_000_000,
            register: 9,
            value: 4,
        });
        chain.sensor_samples[0].high_acquired_ns = 610_000_000;
        // Rewriting the same range does not renew the old-format interval.
        assert!(healthy_sensor_reads(&chain).is_err());
        chain = range_change_chain();
        chain.sensor_write_commands[0].1 = 0x0199_0b00;
        chain.sensor_writes[0].register = 11;
        chain.sensor_writes[0].value = 0;
        assert!(healthy_sensor_reads(&chain).is_err());
        Ok(())
    }
    #[test]
    fn fan_fault_plan_and_applied_edges_cannot_silently_change() -> Result<()> {
        let events = [
            InputEvent {
                at_ns: 1,
                motor: 1,
                powered: None,
                stalled: Some(true),
            },
            InputEvent {
                at_ns: 2,
                motor: 1,
                powered: None,
                stalled: Some(false),
            },
        ];
        assert_eq!(scenario(&events)?, "locked-rotor");
        let trace="md_sim_fan_input ns=1 motor=1 mask=2 values=2\nmd_sim_fan_input ns=2 motor=1 mask=2 values=0\n";
        applied(trace, &events)?;
        assert!(applied(&trace.replace("values=2", "values=0"), &events).is_err());
        assert!(applied(&format!("{trace}{trace}"), &events).is_err());
        let mut changed = events;
        changed[1].motor = 0;
        assert!(scenario(&changed).is_err());
        changed = events;
        changed[1].stalled = Some(true);
        assert!(scenario(&changed).is_err());
        Ok(())
    }
}

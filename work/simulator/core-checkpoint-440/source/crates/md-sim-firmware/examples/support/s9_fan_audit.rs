//! Timed native fan commands and actual reference tachometer readback.
use anyhow::{ensure, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub(super) fn fixture(legacy: &Value, argv: &[Value]) -> Result<()> {
    let fan = &legacy["fan_fixture"];
    let binding = include_bytes!("../../../../simulator/backends/qemu/md-sim-fan.c");
    ensure!(
        fan["binding"]
            == json!({"sha256":md_sim_firmware::prepare::hash_bytes(binding),"size":binding.len()})
            && fan["motors_connected"] == true
            && fan["sample_nanoseconds"] == 1_000_000_000u64
            && fan["motors"]
                == json!({"maximum_rpm":6000,"minimum_rpm":900,"pulses_per_revolution":2,"pwm_channels":[0,1],"slew_rpm_per_second":12000,"tach_channels":[null,1]}),
        "fan fixture identity differs"
    );
    let options: Vec<_> = argv
        .iter()
        .filter_map(Value::as_str)
        .filter(|s| s.starts_with("md-sim-fan."))
        .collect();
    ensure!(
        options == ["md-sim-fan.reference-motors=on"],
        "executed fan fixture differs"
    );
    Ok(())
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct Access {
    pub(super) ns: u64,
    pub(super) line: usize,
    pub(super) write: bool,
    pub(super) address: u32,
    pub(super) value: u32,
}

pub(super) fn accesses(log: &str) -> Result<Vec<Access>> {
    let mut events: Vec<Access> = Vec::new();
    for (index, line) in log.lines().enumerate() {
        let Some(body) = line.strip_prefix("md_sim_fan_mmio ") else {
            continue;
        };
        let mut fields = BTreeMap::new();
        for word in body.split_whitespace() {
            let (key, value) = word.split_once('=').context("fan field has no value")?;
            ensure!(fields.insert(key, value).is_none(), "repeated fan field");
        }
        ensure!(
            fields.keys().copied().collect::<Vec<_>>()
                == ["addr", "error", "ns", "size", "value", "write"],
            "fan trace fields differ or have no clock"
        );
        ensure!(
            fields["error"] == "0" && fields["size"] == "4",
            "failed or unsupported native fan access"
        );
        let ns = fields["ns"].parse()?;
        ensure!(
            events.last().is_none_or(|e| e.ns <= ns),
            "fan clock reversed"
        );
        let write = match fields["write"] {
            "0" => false,
            "1" => true,
            _ => anyhow::bail!("invalid fan operation"),
        };
        let number = |key| -> Result<u32> {
            Ok(u32::from_str_radix(
                fields[key]
                    .strip_prefix("0x")
                    .context("fan hexadecimal field")?,
                16,
            )?)
        };
        let address = number("addr")?;
        let value = number("value")?;
        ensure!(
            if write {
                matches!(address, 16 | 20) && value <= 100
            } else {
                matches!(address, 0 | 4 | 8 | 12 | 16 | 20)
            },
            "native fan register or duty differs"
        );
        if !write && matches!(address, 0 | 8 | 12) {
            ensure!(
                value == 0,
                "disconnected reference tachometer returned motion"
            );
        }
        events.push(Access {
            ns,
            line: index + 1,
            write,
            address,
            value,
        });
    }
    ensure!(!events.is_empty(), "fan accesses absent");
    Ok(events)
}

pub(super) fn assess(
    log: &str,
    high_read: u64,
    restored_input: u64,
    healthy_read: u64,
) -> Result<Value> {
    ensure!(
        high_read < restored_input && restored_input <= healthy_read,
        "fan sensor observation order differs"
    );
    let events = accesses(log)?;
    let mut channels = Vec::new();
    let mut full_at = 0;
    let mut reduced_at = 0;
    for address in [16, 20] {
        let writes: Vec<_> = events
            .iter()
            .filter(|e| e.write && e.address == address)
            .collect();
        let before = writes
            .iter()
            .rev()
            .find(|e| e.ns < high_read)
            .context("initial native duty absent")?;
        ensure!(
            before.value < 100,
            "fan was already at full duty before hot input"
        );
        let full = writes
            .iter()
            .find(|e| e.ns >= high_read && e.ns < restored_input && e.value == 100)
            .context("hot sensor did not cause full native duty")?;
        ensure!(
            writes
                .iter()
                .filter(|e| e.ns >= full.ns && e.ns < restored_input)
                .all(|e| e.value == 100),
            "native duty dropped during hot interval"
        );
        let reduced = writes
            .iter()
            .find(|e| e.ns >= healthy_read && e.value < 100)
            .context("native fan duty did not fall after healthy sensor read")?;
        full_at = full_at.max(full.ns);
        reduced_at = reduced_at.max(reduced.ns);
        channels.push(json!({"pwm_channel":(address - 16) / 4,"before":before,"full":full,"reduced":reduced,"hot_response_ns":full.ns-high_read,"recovery_response_ns":reduced.ns-healthy_read}));
    }
    // The explicit fixture has6000RPM maximum,12000RPM/s slew and one-second
    // tach windows. Two full seconds cover the largest ramp and one complete
    // window. This checks the settled result, not the model's integration code.
    let settled = full_at
        .checked_add(2_000_000_000)
        .context("fan time overflow")?;
    let full_tach = events
        .iter()
        .find(|e| {
            !e.write && e.address == 4 && e.ns >= settled && e.ns < restored_input && e.value == 100
        })
        .context("settled reference tachometer did not report6000RPM")?;
    let reduced = reduced_at
        .checked_add(2_000_000_000)
        .context("fan time overflow")?;
    let reduced_tach = events
        .iter()
        .find(|e| !e.write && e.address == 4 && e.ns >= reduced && (15..100).contains(&e.value))
        .context("reference tachometer did not slow after duty reduction")?;
    Ok(
        json!({"passed":true,"channels":channels,"high_sensor_read_ns":high_read,"restored_input_ns":restored_input,"healthy_sensor_read_ns":healthy_read,"settled_full_tach":full_tach,"reduced_tach":reduced_tach,"access_count":events.len(),"clock":"QEMU virtual nanoseconds at motor synchronization before MMIO","scope":"Native hot-input fan response in the explicit two-motor reference fixture. Reports measured response intervals without claiming a physical deadline, calibrated motor, thermal plant, exact FPGA or original-board pass."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACE: &str =
        "md_sim_fan_mmio ns=149000000000 write=1 addr=0x10 size=4 value=0x1 error=0\n\
md_sim_fan_mmio ns=149000000001 write=1 addr=0x14 size=4 value=0x1 error=0\n\
md_sim_fan_mmio ns=152000000000 write=1 addr=0x10 size=4 value=0x64 error=0\n\
md_sim_fan_mmio ns=152000000001 write=1 addr=0x14 size=4 value=0x64 error=0\n\
md_sim_fan_mmio ns=155000000000 write=0 addr=0x4 size=4 value=0x64 error=0\n\
md_sim_fan_mmio ns=182000000000 write=1 addr=0x10 size=4 value=0x58 error=0\n\
md_sim_fan_mmio ns=182000000001 write=1 addr=0x14 size=4 value=0x58 error=0\n\
md_sim_fan_mmio ns=185000000000 write=0 addr=0x4 size=4 value=0x59 error=0\n";

    fn check(log: &str) -> Result<Value> {
        assess(log, 151_000_000_000, 180_000_000_000, 181_000_000_000)
    }

    #[test]
    fn fixture_checks_executed_option_and_motor_parameters() -> Result<()> {
        let binding = include_bytes!("../../../../simulator/backends/qemu/md-sim-fan.c");
        let mut legacy = json!({"fan_fixture":{"binding":{"sha256":md_sim_firmware::prepare::hash_bytes(binding),"size":binding.len()},"motors_connected":true,"sample_nanoseconds":1000000000u64,"motors":{"maximum_rpm":6000,"minimum_rpm":900,"pulses_per_revolution":2,"pwm_channels":[0,1],"slew_rpm_per_second":12000,"tach_channels":[null,1]}}});
        let argv = [json!("md-sim-fan.reference-motors=on")];
        fixture(&legacy, &argv)?;
        assert!(fixture(&legacy, &[]).is_err());
        assert!(fixture(&legacy, &[argv[0].clone(), argv[0].clone()]).is_err());
        legacy["fan_fixture"]["motors"]["maximum_rpm"] = json!(6001);
        assert!(fixture(&legacy, &argv).is_err());
        Ok(())
    }

    #[test]
    fn both_native_commands_and_settled_motion_are_required() -> Result<()> {
        assert_eq!(
            check(TRACE)?["channels"][0]["hot_response_ns"],
            1_000_000_000u64
        );
        for (old, new) in [
            ("value=0x64", "value=0x63"),
            ("ns=155000000000", "ns=153000000000"),
            ("value=0x59", "value=0x64"),
            ("value=0x1", "value=0x64"),
            ("addr=0x14", "addr=0x10"),
            ("error=0", "error=1"),
            ("ns=152000000001", "ns=148000000000"),
        ] {
            assert!(check(&TRACE.replace(old, new)).is_err(), "{old} -> {new}");
        }
        assert!(check(&TRACE.replace("ns=149000000000 ", "")).is_err());
        assert!(check(&TRACE.replace("write=1", "write=1 write=1")).is_err());
        assert!(check(&TRACE.replace("addr=0x4 ", "addr=0x0 ")).is_err());
        Ok(())
    }
}

//! Check actual sensor replies, power transitions and native safety messages.
//! This is a reference-environment fault test, not a calibrated thermal plant.
use anyhow::{ensure, Context, Result};
use md_sim_firmware::bm1387::{self, i2c::InputEvent, Command, Packet};
use md_sim_runtime::store::hash_file;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, VecDeque},
    path::Path,
};
#[path = "s9_fan_audit.rs"]
mod fan_audit;
#[path = "s9_fan_fault_audit.rs"]
mod fan_fault;
#[path = "s9_power_trace.rs"]
mod power_trace;
#[path = "s9_sensor_fault_audit.rs"]
mod sensor_fault;

#[derive(Default, serde::Serialize)]
struct SensorSample {
    high_ns: u64,
    low_ns: u64,
    high_acquired_ns: u64,
    high_byte: u8,
    low_byte: u8,
    channel: u8,
    units_sixteenths_celsius: i16,
    invalid: bool,
    open: bool,
    short: bool,
}

#[derive(serde::Serialize)]
struct SensorWrite {
    applied_ns: u64,
    completed_ns: u64,
    register: u8,
    value: u8,
}

impl SensorSample {
    fn valid(&self) -> bool {
        !self.invalid && !self.open && !self.short
    }
}

#[derive(Default)]
struct Chain {
    power_trace: power_trace::PowerTrace,
    bytes: Vec<u8>,
    queries: VecDeque<Option<(u64, u32, u64)>>,
    i2c: u32,
    i2c_applied_ns: u64,
    i2c_sequence: u64,
    completed_i2c: Option<(u64, u32)>,
    extended: bool,
    powered: bool,
    released: bool,
    last_ns: u64,
    temperatures: Vec<(u64, i16)>,
    sensor_high: Option<(u64, u64, u8, u8, bool)>,
    sensor_samples: Vec<SensorSample>,
    sensor_write_commands: Vec<(u64, u32)>,
    sensor_writes: Vec<SensorWrite>,
    power_edges: Vec<(u64, bool)>,
    hashes: Vec<u64>,
}

impl Chain {
    fn sensor_register(
        &mut self,
        ns: u64,
        acquired_ns: u64,
        register: u8,
        value: u8,
    ) -> Result<()> {
        let high = self.sensor_high.take();
        match register {
            0..=1 => self.sensor_high = Some((ns, acquired_ns, register, value, self.extended)),
            0x10..=0x11 => {
                let (high_ns, high_acquired_ns, channel, byte, extended) =
                    high.context("sensor low byte lacks a paired high read")?;
                ensure!(
                    channel == register - 0x10 && high_ns <= ns,
                    "sensor read pair differs"
                );
                ensure!(value & 0x0c == 0, "sensor low byte has reserved flags");
                let degrees = if extended {
                    i16::from(byte) - 64
                } else {
                    i16::from(byte as i8)
                };
                let sample = SensorSample {
                    high_ns,
                    low_ns: ns,
                    high_acquired_ns,
                    high_byte: byte,
                    low_byte: value,
                    channel,
                    units_sixteenths_celsius: degrees * 16 + i16::from(value >> 4),
                    invalid: value & 2 != 0,
                    open: value & 1 != 0,
                    short: extended && channel == 1 && byte == 0,
                };
                if channel == 1 && sample.valid() && value >> 4 == 0 {
                    self.temperatures.push((high_ns, degrees));
                }
                self.sensor_samples.push(sample);
            }
            _ => {}
        }
        Ok(())
    }

    fn command(&mut self, ns: u64, packet: Packet) -> Result<()> {
        if let Packet::Command { broadcast, command } = packet {
            match command {
                Command::Write {
                    address: 244,
                    register: 0x20,
                    value,
                } if !broadcast => {
                    self.i2c = value;
                    self.i2c_applied_ns = ns;
                    if value & 0xffff_0000 == 0x0199_0000 {
                        self.sensor_write_commands.push((ns, value));
                    }
                    self.i2c_sequence = self
                        .i2c_sequence
                        .checked_add(1)
                        .context("I2C transaction counter overflow")?;
                }
                Command::Read { address, register } => {
                    let request = (address == 244 && register == 0x20 && !broadcast).then_some((
                        self.i2c_sequence,
                        self.i2c,
                        self.i2c_applied_ns,
                    ));
                    self.queries
                        .extend(std::iter::repeat_n(request, if broadcast { 63 } else { 1 }));
                    ensure!(self.queries.len() <= 1024, "sensor audit query queue bound");
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn state(&mut self, ns: u64, kind: u32, value: u32) -> Result<()> {
        if matches!(kind, 8 | 9) {
            ensure!(value <= 1, "invalid ASIC pin level");
            if kind == 8 {
                if self.powered != (value != 0) {
                    self.power_edges.push((ns, value != 0));
                }
                self.powered = value != 0;
                if !self.powered {
                    self.extended = false;
                }
            } else {
                self.released = value != 0;
            }
            if value == 0 {
                self.bytes.clear();
                self.queries.clear();
                self.i2c = 0;
                self.sensor_high = None;
                self.completed_i2c = None;
            }
        }
        if kind == 10 {
            ensure!(
                self.powered && self.released,
                "hash computed while power or reset inhibits the ASIC"
            );
            self.hashes.push(ns);
        }
        if kind != 2 {
            return Ok(());
        }
        self.bytes.push(u8::try_from(value)?);
        if self.bytes.len() != 7 {
            return Ok(());
        }
        ensure!(
            super::submission_audit::valid_reply(&self.bytes),
            "sensor trace reply checksum"
        );
        if self.bytes[6] & 0x80 == 0 {
            let requested = self
                .queries
                .pop_front()
                .context("register reply lacks an applied guest query")?;
            if let Some((sequence, command, applied_ns)) = requested {
                let word = u32::from_be_bytes(self.bytes[..4].try_into()?);
                ensure!(
                    word & 0x7fff_ff00 == command & 0x7fff_ff00,
                    "sensor controller reply differs from guest transaction"
                );
                if command & 0xffff_0000 == 0x0199_0000 {
                    ensure!(
                        word & 0x7fff_ffff == command,
                        "sensor write readback changed the guest data byte"
                    );
                }
                if word & 0x8000_0000 == 0 {
                    if let Some((previous, returned)) = self.completed_i2c {
                        ensure!(
                            sequence >= previous,
                            "completed I2C transaction order reversed"
                        );
                        if sequence == previous {
                            ensure!(
                                word == returned,
                                "completed I2C readback changed without a guest transaction"
                            );
                        }
                    }
                    if self
                        .completed_i2c
                        .is_none_or(|(previous, _)| sequence != previous)
                    {
                        // The pinned reference bus uses 100kHz. Write-byte
                        // transfers use 29 slots; read-byte transfers use39.
                        // Both include the declared START/STOP slot policy.
                        let transfer_ns = if command & 0x10000 != 0 {
                            290_000
                        } else {
                            390_000
                        };
                        let completed_ns = applied_ns
                            .checked_add(transfer_ns)
                            .context("sensor transaction clock overflow")?;
                        ensure!(completed_ns <= ns, "sensor reply precedes bus completion");
                        if word & 0xffff_0000 == 0x0199_0000 {
                            self.sensor_writes.push(SensorWrite {
                                applied_ns,
                                completed_ns,
                                register: (word >> 8) as u8,
                                value: word as u8,
                            });
                        }
                        if word & 0xffff_ff00 == 0x0199_0900 {
                            self.extended = word & 4 != 0;
                        }
                        if word & 0xffff_0000 == 0x0198_0000 {
                            self.sensor_register(ns, completed_ns, (word >> 8) as u8, word as u8)?;
                        }
                        self.completed_i2c = Some((sequence, word));
                    }
                }
            }
        }
        self.bytes.clear();
        Ok(())
    }
}

fn traffic(log: &str) -> Result<BTreeMap<u32, Chain>> {
    let mut chains = BTreeMap::<u32, Chain>::new();
    for (index, line) in log.lines().enumerate() {
        if let Some(body) = line.strip_prefix("md_sim_pic_power ") {
            let fields: BTreeMap<_, _> = body
                .split_whitespace()
                .filter_map(|f| f.split_once('='))
                .collect();
            let address = super::hex_field(&fields, "address")?;
            ensure!((0x55..=0x57).contains(&address), "unexpected reference PIC");
            let ns = fields.get("ns").context("PIC clock absent")?.parse()?;
            let on = super::hex_field(&fields, "state")? & 1 != 0;
            chains
                .entry(address - 0x55 + 6)
                .or_default()
                .power_trace
                .producer(ns, index + 1, on)?;
            continue;
        }
        let packet = line.starts_with("md_sim_bm1387_packet ");
        let Some(body) = line
            .strip_prefix("md_sim_bm1387_packet ")
            .or_else(|| line.strip_prefix("md_sim_bm1387_state "))
        else {
            continue;
        };
        let fields: BTreeMap<_, _> = body
            .split_whitespace()
            .filter_map(|f| f.split_once('='))
            .collect();
        let link: u32 = fields.get("link").context("ASIC link absent")?.parse()?;
        ensure!((6..=8).contains(&link), "unexpected reference chain");
        let ns: u64 = fields
            .get("virtual_ns")
            .context("ASIC clock absent")?
            .parse()?;
        let kind: u32 = fields
            .get("kind")
            .context("ASIC event kind absent")?
            .parse()?;
        let chain = chains.entry(link).or_default();
        ensure!(ns >= chain.last_ns, "ASIC trace time reversed");
        chain.last_ns = ns;
        if packet {
            if kind == 4 {
                let bytes = hex::decode(fields.get("frame_hex").context("ASIC frame absent")?)?;
                ensure!(bytes.len() <= 150, "ASIC packet bound");
                chain.command(
                    ns,
                    bm1387::decode(&bytes).map_err(|e| anyhow::anyhow!("guest packet: {e:?}"))?,
                )?;
            }
        } else {
            let value = super::hex_field(&fields, "value")?;
            if kind == 8 {
                chain.power_trace.consumer(ns, index + 1, value != 0)?;
            }
            chain.state(ns, kind, value)?;
        }
    }
    ensure!(chains.len() == 3, "three chains were not observed");
    for chain in chains.values() {
        chain.power_trace.finish()?;
    }
    Ok(chains)
}

fn assess(
    chains: &BTreeMap<u32, Chain>,
    start: u64,
    restored: u64,
    hot: i16,
    guest: &str,
) -> Result<Value> {
    let dangerous = hot == 115;
    let mut results = Vec::new();
    ensure!(
        guest.lines().any(|l| l.contains("Resolved monitor config:")
            && l.contains("dangerous_temp: 110.0, hot_temp: 100.0")),
        "native thermal thresholds were not observed"
    );
    let shutdowns: Vec<_> = guest.lines().filter(|l| l.contains("bosminer_hal::monitor: HALT reason: Shutdown: temperature 115°C is above DANGEROUS")).collect();
    ensure!(
        dangerous != shutdowns.is_empty(),
        "native dangerous-temperature result differs"
    );
    if !dangerous {
        ensure!(
            !guest
                .lines()
                .any(|l| l.contains("bosminer_hal::monitor: HALT reason:")),
            "native miner halted during the hot-only test"
        );
    }
    for (&link, chain) in chains {
        let before = chain
            .temperatures
            .iter()
            .find(|&&(ns, t)| ns < start && t == 65)
            .context("no initial65C sensor read")?;
        let high = chain
            .temperatures
            .iter()
            .find(|&&(ns, t)| ns >= start && ns < restored && t == hot)
            .context("no changed sensor reading during the input interval")?;
        let after = chain
            .temperatures
            .iter()
            .find(|&&(ns, t)| ns >= restored && t == 65)
            .context("no restored65C sensor read")?;
        ensure!(
            chain.hashes.iter().any(|&ns| ns < start)
                && chain.hashes.iter().any(|&ns| ns > after.0),
            "hashing was not observed before the fault and after sensor recovery"
        );
        let cuts: Vec<_> = chain
            .power_edges
            .iter()
            .filter(|&&(ns, on)| !on && ns > high.0 && ns < restored)
            .collect();
        if dangerous {
            ensure!(
                !cuts.is_empty(),
                "native shutdown did not cut this chain's power during the fault"
            );
            ensure!(
                chain
                    .power_edges
                    .iter()
                    .any(|&(ns, on)| on && ns >= restored && ns < after.0),
                "chain was not repowered after recovery"
            );
        } else {
            ensure!(
                !chain.power_edges.iter().any(|&(ns, on)| !on && ns >= start),
                "hot-only test unexpectedly cut power"
            );
        }
        results.push(json!({"link":link,"initial":before,"changed":high,"restored":after,
            "power_edges":chain.power_edges,"power_cuts_during_fault":cuts,"computed_hashes":chain.hashes.len(),
            "sensor_reads":chain.temperatures,"paired_sensor_reads":chain.sensor_samples,"no_computation_while_inhibited":true}));
    }
    Ok(
        json!({"passed":true,"case":if dangerous {"dangerous-temperature-shutdown-and-recovery"} else {"hot-temperature-readback-and-continued-operation"},"chains":results,"native_shutdown_messages":shutdowns,
        "scope":"Actual archived native code in the declared adapted reference environment. CRC-checked sensor replies are joined to applied guest queries. Power and ASIC reset inhibit every observed hash. Hot-only mode does not establish a fan-control threshold pass. No calibrated thermal response, exact physical-board, mining or Core gate is granted by this audit."}),
    )
}

pub fn analyze(run: &Path) -> Result<Value> {
    analyze_inner(run, false)
}

pub fn analyze_cooling(run: &Path) -> Result<Value> {
    analyze_inner(run, true)
}

pub fn analyze_fan_recovery(run: &Path) -> Result<Value> {
    fan_fault::analyze(run)
}

fn analyze_inner(run: &Path, cooling: bool) -> Result<Value> {
    let (backend, guest) = super::submission_audit::completed_logs(run)?;
    let adaptation: Value = serde_json::from_str(&super::read(&run.join("adaptation.json"))?)?;
    let spec: Value = serde_json::from_str(&super::read(&run.join("process-spec.json"))?)?;
    let raw = super::read_bounded(&run.join("temperature-plan-original.json"), 4096)?;
    let events: Vec<InputEvent> = serde_json::from_str(&raw)?;
    ensure!(
        events.len() == 2 && events[0].at_ns > 0 && events[0].at_ns < events[1].at_ns,
        "requires two ordered temperature input edges"
    );
    let case = sensor_fault::scenario(&events)?;
    let legacy = &adaptation["legacy_io"];
    let plan = &legacy["temperature_input_plan"];
    ensure!(
        plan["schema"] == "md-sim-temperature-input-plan/1"
            && plan["source"]
                == serde_json::to_value(hash_file(&run.join("temperature-plan-original.json"))?)?
            && plan["events"] == serde_json::to_value(&events)?
            && legacy["temperature_fixture"] == true
            && legacy["asic_contract"]["temperature_fixture"]["profile"]["sha256"]
                == md_sim_firmware::prepare::hash_bytes(include_bytes!(
                    "../../../../simulator/backends/qemu/s9-tmp421-reference.json"
                )),
        "temperature input identity differs"
    );
    let wanted = format!(
        "md-sim-s9io.temperature-input-plan={}",
        serde_json::to_string(&events)?
    );
    let argv = spec["argv"]
        .as_array()
        .context("process arguments absent")?;
    let plans: Vec<_> = argv
        .iter()
        .filter_map(Value::as_str)
        .filter(|s| s.starts_with("md-sim-s9io.temperature-input-plan="))
        .collect();
    ensure!(
        plans == [wanted.as_str()],
        "executed temperature plan differs"
    );
    let chains = traffic(&backend)?;
    if cooling {
        fan_audit::fixture(legacy, argv)?;
        ensure!(
            case.is_none() && events[0].inputs.remote_millicelsius == 105000,
            "cooling audit requires the hot-only scenario"
        );
    }
    let mut report = if let Some(case) = case {
        let console: Value = serde_json::from_str(&super::read(&run.join("console.json"))?)?;
        let script = include_bytes!("../../../../simulator/scenarios/s9-sensor-manual-recovery.sh");
        let identity =
            json!({"sha256":md_sim_firmware::prepare::hash_bytes(script),"size":script.len()});
        let inputs = spec["inputs"].as_array().context("process inputs absent")?;
        let scripts: Vec<_> = inputs
            .iter()
            .filter(|v| v["guest_path"] == "/console-script")
            .collect();
        ensure!(
            console["script"] == identity && scripts.len() == 1 && scripts[0]["object"] == identity,
            "manual recovery script differs from this suite"
        );
        sensor_fault::assess(&chains, &events, &guest, case)?
    } else {
        assess(
            &chains,
            events[0].at_ns,
            events[1].at_ns,
            (events[0].inputs.remote_millicelsius / 1000) as i16,
            &guest,
        )?
    };
    for (&link, chain) in &chains {
        report["pic_to_asic_power_transfers"][link.to_string()] =
            serde_json::to_value(chain.power_trace.finish()?)?;
    }
    if cooling {
        let first_read = |temperature: i16, after: u64| -> Result<u64> {
            chains
                .values()
                .flat_map(|c| &c.sensor_samples)
                .filter(|s| {
                    s.channel == 1
                        && s.valid()
                        && s.high_ns >= after
                        && s.units_sixteenths_celsius == temperature * 16
                })
                .map(|s| s.low_ns)
                .min()
                .context("fan sensor observation absent")
        };
        report["cooling"] = fan_audit::assess(
            &backend,
            first_read(105, events[0].at_ns)?,
            events[1].at_ns,
            first_read(65, events[1].at_ns)?,
        )?;
        report["cooling_verifier_source"] = json!({"sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_fan_audit.rs")),"size":include_bytes!("s9_fan_audit.rs").len()});
    }
    for name in [
        "result.json",
        "process-spec.json",
        "adaptation.json",
        "backend.log",
        "guest.log",
        "temperature-plan-original.json",
        "console.json",
    ] {
        report["inputs"][name] = serde_json::to_value(hash_file(&run.join(name))?)?;
    }
    report["verifier_source"] = json!({"sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_temperature_audit.rs")),"size":include_bytes!("s9_temperature_audit.rs").len()});
    report["power_trace_verifier_source"] = json!({"sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_power_trace.rs")),"size":include_bytes!("s9_power_trace.rs").len()});
    report["sensor_fault_verifier_source"] = json!({"sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_sensor_fault_audit.rs")),"size":include_bytes!("s9_sensor_fault_audit.rs").len()});
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_sensor_write_requires_exact_data_and_elapsed_bus_time() -> Result<()> {
        // Actual CRC-protected config09=04 reply from fan-power193.
        let frame = [1, 153, 9, 4, 0, 0, 21];
        for (command, applied, observed, success) in [
            (0x0199_0904, 100, 290_100, true),
            (0x0199_0900, 100, 290_100, false),
            (0x0199_0904, 100, 290_099, false),
        ] {
            let mut chain = Chain::default();
            chain.queries.push_back(Some((1, command, applied)));
            for byte in &frame[..6] {
                chain.state(observed, 2, *byte)?;
            }
            assert_eq!(chain.state(observed, 2, frame[6]).is_ok(), success);
            if success {
                assert!(chain.extended);
                assert_eq!(chain.sensor_writes[0].completed_ns, 290_100);
            }
        }
        Ok(())
    }

    #[test]
    fn captured_sensor_reply_requires_a_guest_query_and_the_selected_range() -> Result<()> {
        // Literal complete reply from temperature-hot-170. No model encoder.
        let frame = [1, 152, 1, 169, 0, 0, 21];
        let mut missing = Chain::default();
        for byte in &frame[..6] {
            missing.state(1 + 1_000_000, 2, *byte)?;
        }
        assert!(missing.state(1 + 1_000_000, 2, frame[6]).is_err());
        let mut chain = Chain {
            extended: true,
            ..Chain::default()
        };
        chain.queries.push_back(Some((1, 0x0198_0100, 0)));
        for byte in frame {
            chain.state(1 + 1_000_000, 2, byte)?;
        }
        assert!(chain.temperatures.is_empty());
        // Actual low-byte reply from both temperature and sensor-fault runs.
        chain.queries.push_back(Some((2, 0x0198_1100, 0)));
        for byte in [1, 152, 17, 0, 0, 0, 13] {
            chain.state(2 + 1_000_000, 2, byte)?;
        }
        assert_eq!(chain.temperatures, [(1_000_001, 105)]);
        chain.queries.push_back(Some((3, 0x0198_0000, 0)));
        for byte in &frame[..6] {
            chain.state(2 + 1_000_000, 2, *byte)?;
        }
        assert!(chain.state(2 + 1_000_000, 2, frame[6]).is_err());
        let mut inhibited = Chain::default();
        assert!(inhibited.state(3 + 1_000_000, 10, 0).is_err());
        inhibited.state(4 + 1_000_000, 8, 1)?;
        assert!(inhibited.state(5 + 1_000_000, 10, 0).is_err());
        inhibited.state(6 + 1_000_000, 9, 1)?;
        inhibited.state(7 + 1_000_000, 10, 0)?;
        inhibited.state(8 + 1_000_000, 8, 0)?;
        assert!(inhibited.state(9 + 1_000_000, 10, 0).is_err());
        Ok(())
    }

    #[test]
    fn captured_fault_flags_invalidate_the_paired_temperature() -> Result<()> {
        for (register, high, low) in [
            (0x11, [1, 152, 1, 129, 0, 0, 23], [1, 152, 17, 1, 0, 0, 19]),
            (0x11, [1, 152, 1, 129, 0, 0, 23], [1, 152, 17, 2, 0, 0, 20]),
            (0x10, [1, 152, 0, 89, 0, 0, 27], [1, 152, 16, 2, 0, 0, 22]),
        ] {
            // Literal CRC-protected frames from diode-open175 and supply-invalid175.
            let mut chain = Chain {
                extended: true,
                ..Chain::default()
            };
            chain
                .queries
                .push_back(Some((1, 0x0198_0000 | (register - 0x10) << 8, 0)));
            for byte in high {
                chain.state(1 + 1_000_000, 2, byte)?;
            }
            chain
                .queries
                .push_back(Some((2, 0x0198_0000 | register << 8, 0)));
            for byte in low {
                chain.state(2 + 1_000_000, 2, byte)?;
            }
            assert!(chain.temperatures.is_empty());
            assert_eq!(chain.sensor_samples.len(), 1);
            assert!(!chain.sensor_samples[0].valid());
        }
        let mut unpaired = Chain::default();
        assert!(unpaired.sensor_register(1, 1, 0x11, 0).is_err());
        unpaired.sensor_register(2, 2, 0, 89)?;
        assert!(unpaired.sensor_register(3, 3, 0x11, 0).is_err());
        unpaired.sensor_register(4, 4, 1, 129)?;
        assert!(unpaired.sensor_register(5, 5, 0x11, 4).is_err());
        Ok(())
    }

    #[test]
    fn repeated_controller_readback_is_one_sensor_transaction() -> Result<()> {
        let mut chain = Chain {
            extended: true,
            ..Chain::default()
        };
        let high = [1, 152, 1, 129, 0, 0, 23];
        let low = [1, 152, 17, 0, 0, 0, 13];
        for (ns, sequence, register, frame) in [
            (1, 1, 1, high),
            (2, 1, 1, high),
            (3, 2, 0x11, low),
            (4, 2, 0x11, low),
        ] {
            chain
                .queries
                .push_back(Some((sequence, 0x0198_0000 | register << 8, 0)));
            for byte in frame {
                chain.state(ns + 1_000_000, 2, byte)?;
            }
        }
        assert_eq!(chain.sensor_samples.len(), 1);
        assert_eq!(chain.sensor_samples[0].high_ns, 1_000_001);
        assert_eq!(chain.sensor_samples[0].low_ns, 1_000_003);
        assert_eq!(chain.temperatures, [(1_000_001, 65)]);
        // A valid CRC does not permit a changed cached result without a write.
        chain.queries.push_back(Some((2, 0x0198_1100, 0)));
        for byte in [1, 152, 17, 1, 0, 0] {
            chain.state(5 + 1_000_000, 2, byte)?;
        }
        assert!(chain.state(5 + 1_000_000, 2, 19).is_err());
        Ok(())
    }

    #[test]
    fn shutdown_needs_changed_sensor_power_cut_and_actual_recovery() -> Result<()> {
        let guest = "Resolved monitor config: dangerous_temp: 110.0, hot_temp: 100.0\nbosminer_hal::monitor: HALT reason: Shutdown: temperature 115°C is above DANGEROUS";
        let mut chains: BTreeMap<_, _> = (6..=8)
            .map(|link| {
                (
                    link,
                    Chain {
                        temperatures: vec![(1, 65), (11, 115), (25, 65)],
                        power_edges: vec![(0, true), (12, false), (22, true)],
                        hashes: vec![2, 26],
                        ..Chain::default()
                    },
                )
            })
            .collect();
        assert!(assess(&chains, 10, 20, 115, guest).is_ok());
        let chain = chains.get_mut(&7).unwrap();
        chain.power_edges[1].0 = 9;
        assert!(assess(&chains, 10, 20, 115, guest).is_err());
        chains.get_mut(&7).unwrap().power_edges[1].0 = 12;
        chains.get_mut(&7).unwrap().hashes.pop();
        assert!(assess(&chains, 10, 20, 115, guest).is_err());
        chains.get_mut(&7).unwrap().hashes.push(26);
        chains.get_mut(&7).unwrap().temperatures[2].1 = 115;
        assert!(assess(&chains, 10, 20, 115, guest).is_err());
        chains.get_mut(&7).unwrap().temperatures[2].1 = 65;
        assert!(assess(
            &chains,
            10,
            20,
            115,
            "Resolved monitor config: dangerous_temp: 110.0, hot_temp: 100.0"
        )
        .is_err());
        Ok(())
    }
}

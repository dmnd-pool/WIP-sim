//! Freeze external sensor inputs without changing the guest's configuration.
use anyhow::{ensure, Result};
use md_sim_firmware::bm1387::i2c::{Controller, InputEvent, Profile};
use serde_json::{json, Value};
use std::{fs, io::Read, path::Path};

pub struct Plan {
    raw: Vec<u8>,
    pub json: String,
    pub manifest: Value,
}
impl Plan {
    pub fn read(path: &Path) -> Result<Self> {
        let file = fs::File::open(path)?;
        ensure!(
            file.metadata()?.is_file(),
            "temperature plan must be a regular file"
        );
        let mut raw = Vec::new();
        file.take(4097).read_to_end(&mut raw)?;
        Self::parse(raw)
    }
    fn parse(raw: Vec<u8>) -> Result<Self> {
        ensure!(
            !raw.is_empty() && raw.len() <= 4096,
            "temperature plan exceeds bound"
        );
        let events: Vec<InputEvent> = serde_json::from_slice(&raw)?;
        let reference: Value = serde_json::from_slice(include_bytes!(
            "../../../../simulator/backends/qemu/s9-tmp421-reference.json"
        ))?;
        let profile: Profile = serde_json::from_value(reference["bus"].clone())?;
        let mut controller = Controller::new(profile).map_err(|e| anyhow::anyhow!("{e:?}"))?;
        controller
            .install_input_plan(events.clone())
            .map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let encoded = serde_json::to_string(&events)?;
        ensure!(
            encoded.len() <= 4096,
            "canonical temperature plan exceeds backend bound"
        );
        let manifest = json!({"schema":"md-sim-temperature-input-plan/1",
            "source":{"sha256":md_sim_firmware::prepare::hash_bytes(&raw),"size":raw.len()},
            "events":events,"timeline":"absolute emulator virtual nanoseconds; conversions and bus completion precede an equal-time input edge",
            "application":"all three reference chains at the declared attached sensor",
            "scope":"Explicit external sensor test inputs; no calibrated thermal plant or exact board claim"});
        Ok(Self {
            raw,
            json: encoded,
            manifest,
        })
    }
    pub fn retain(&self, out: &Path) -> Result<()> {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(out.join("temperature-plan-original.json"))?
            .write_all(&self.raw)?;
        Ok(())
    }
}
use std::io::Write;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_bytes_and_order_are_pinned_before_backend_selection() -> Result<()> {
        let raw = br#"[{"at_ns":300000000,"inputs":{"local_millicelsius":25000,"remote_millicelsius":105000,"diode":"Connected","supply_valid":true}}]"#;
        let plan = Plan::parse(raw.to_vec())?;
        let mut spaced = raw.to_vec();
        spaced.push(b'\n');
        let other = Plan::parse(spaced)?;
        assert_eq!(plan.json, other.json);
        assert_ne!(plan.manifest["source"], other.manifest["source"]);
        assert!(Plan::parse(Vec::new()).is_err());
        assert!(Plan::parse(vec![b' '; 4097]).is_err());
        let mut events: Value = serde_json::from_slice(raw)?;
        let event = events[0].clone();
        events.as_array_mut().unwrap().push(event);
        assert!(Plan::parse(serde_json::to_vec(&events)?).is_err());
        events.as_array_mut().unwrap().pop();
        events[0]["at_ns"] = json!(0);
        assert!(Plan::parse(serde_json::to_vec(&events)?).is_err());
        Ok(())
    }
}

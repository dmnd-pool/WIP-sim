//! Frozen external motor inputs for the explicit two-motor S9 reference fixture.
use anyhow::{ensure, Result};
use md_sim_firmware::fan::{validate_input_plan, InputEvent};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

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
            "fan plan must be a regular file"
        );
        let mut raw = Vec::new();
        file.take(4097).read_to_end(&mut raw)?;
        Self::parse(raw)
    }

    fn parse(raw: Vec<u8>) -> Result<Self> {
        ensure!(
            !raw.is_empty() && raw.len() <= 4096,
            "fan plan exceeds bound"
        );
        let events: Vec<InputEvent> = serde_json::from_slice(&raw)?;
        validate_input_plan(&events, 2).map_err(|e| anyhow::anyhow!("fan plan: {e:?}"))?;
        let encoded = serde_json::to_string(&events)?;
        ensure!(
            encoded.len() <= 4096,
            "canonical fan plan exceeds backend bound"
        );
        let manifest = json!({"schema":"md-sim-fan-input-plan/1",
            "source":{"sha256":md_sim_firmware::prepare::hash_bytes(&raw),"size":raw.len()},
            "events":events,"timeline":"absolute emulator virtual nanoseconds; preceding motion and equal-time tach sample precede an input edge; equal-time motors use index order",
            "scope":"External power and rotor-lock inputs for the explicit two-motor fixture. No physical fan, calibrated response or exact board claim."});
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
            .open(out.join("fan-plan-original.json"))?
            .write_all(&self.raw)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn freeze_bytes_without_accepting_unknown_wires_or_reordered_inputs() -> Result<()> {
        let raw = br#"[{"at_ns":150000000000,"motor":1,"powered":null,"stalled":true}]"#;
        let a = Plan::parse(raw.to_vec())?;
        let b = Plan::parse([raw.as_slice(), b"\n"].concat())?;
        assert_eq!(a.json, b.json);
        assert_ne!(a.manifest["source"], b.manifest["source"]);
        let mut v: Value = serde_json::from_slice(raw)?;
        v[0]["motor"] = json!(2);
        assert!(Plan::parse(serde_json::to_vec(&v)?).is_err());
        assert!(Plan::parse(Vec::new()).is_err());
        assert!(Plan::parse(vec![b' '; 4097]).is_err());
        Ok(())
    }
}

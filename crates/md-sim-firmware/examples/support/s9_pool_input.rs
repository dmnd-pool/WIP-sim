//! Frozen entropy, difficulty and timing inputs shared by pool transports.
use anyhow::{ensure, Result};
use bitcoin_hashes::{sha256, Hash};
use md_sim_mining::regtest::RegtestAnchor;
use md_sim_mining::stratum::{CompletionPolicy, ResponsePolicy};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Read,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolInput {
    pub schema: String,
    pub seed: [u8; 32],
    pub time: u32,
    pub seconds: u64,
    pub difficulty_exponents: [i32; 2],
    #[serde(default)]
    pub regtest_anchor: Option<RegtestAnchor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_limit: Option<usize>,
}

impl PoolInput {
    pub fn session_limit(&self) -> Result<usize> {
        self.response_policy()?;
        if matches!(
            self.schema.as_str(),
            "md-sim-s9-pool-input/7" | "md-sim-s9-pool-input/8"
        ) {
            let limit = self
                .connection_limit
                .ok_or_else(|| anyhow::anyhow!("explicit pool connection limit absent"))?;
            ensure!(
                (16..=128).contains(&limit),
                "pool connection limit out of range"
            );
            Ok(limit)
        } else {
            ensure!(
                self.connection_limit.is_none(),
                "legacy pool input cannot change its connection limit"
            );
            Ok(16)
        }
    }

    pub fn response_policy(&self) -> Result<ResponsePolicy> {
        match self.schema.as_str() {
            "md-sim-s9-pool-input/2" | "md-sim-s9-pool-input/3" | "md-sim-s9-pool-input/4" => {
                Ok(ResponsePolicy::LegacyFalseCode23)
            }
            "md-sim-s9-pool-input/5"
            | "md-sim-s9-pool-input/6"
            | "md-sim-s9-pool-input/7"
            | "md-sim-s9-pool-input/8" => Ok(ResponsePolicy::JsonRpc1StratumV1),
            _ => anyhow::bail!("unknown pool input schema"),
        }
    }

    /// Per-connection command policy. Older TCP support VMs have their own
    /// explicit persistent-session contract; their frozen bytes stay intact.
    pub fn completion_policy(&self) -> Result<CompletionPolicy> {
        self.response_policy()?;
        Ok(
            if matches!(
                self.schema.as_str(),
                "md-sim-s9-pool-input/6" | "md-sim-s9-pool-input/8"
            ) {
                CompletionPolicy::KeepOpen
            } else {
                CompletionPolicy::CloseAfterWork
            },
        )
    }

    /// Each new connection receives new work. Older frozen inputs retain their
    /// recorded behavior; their reports must not acquire a new seed silently.
    pub fn session_seed(&self, session: usize) -> Result<[u8; 32]> {
        ensure!(session < self.session_limit()?, "pool session seed bound");
        self.response_policy()?;
        if matches!(
            self.schema.as_str(),
            "md-sim-s9-pool-input/2" | "md-sim-s9-pool-input/3"
        ) {
            return Ok(self.seed);
        }
        let mut material = b"md-sim-s9-pool-session-seed/1\0".to_vec();
        material.extend_from_slice(&self.seed);
        material.extend_from_slice(&(session as u64).to_le_bytes());
        Ok(sha256::Hash::hash(&material).to_byte_array())
    }
}

pub fn prepare(
    seconds: u64,
    difficulty_exponents: [i32; 2],
    regtest_anchor: Option<RegtestAnchor>,
) -> Result<serde_json::Value> {
    validate(seconds, difficulty_exponents)?;
    if let Some(anchor) = &regtest_anchor {
        anchor.validate().map_err(anyhow::Error::msg)?;
    }
    let mut seed = [0; 32];
    File::open("/dev/urandom")?.read_exact(&mut seed)?;
    Ok(serde_json::to_value(PoolInput {
        schema: "md-sim-s9-pool-input/7".into(),
        seed,
        time: u32::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())?,
        seconds,
        difficulty_exponents,
        regtest_anchor,
        // Native protocol probes and reconnects count too. A16-connection
        // lifetime limit exhausted during diode-open181's270-second window.
        // Keep the larger lifetime budget explicit in the hashed input.
        connection_limit: Some(128),
    })?)
}

pub fn prepare_window(
    seconds: u64,
    exponents: [i32; 2],
    anchor: Option<RegtestAnchor>,
) -> Result<serde_json::Value> {
    let mut input = prepare(seconds, exponents, anchor)?;
    input["schema"] = serde_json::json!("md-sim-s9-pool-input/8");
    Ok(input)
}

fn validate(seconds: u64, exponents: [i32; 2]) -> Result<()> {
    ensure!(
        (20..=900).contains(&seconds),
        "private pool deadline out of range"
    );
    ensure!(
        exponents.iter().all(|value| (-31..=0).contains(value)) && exponents[0] != exponents[1],
        "pool must use two distinct supported difficulty exponents"
    );
    Ok(())
}

pub fn read(path: &Path) -> Result<PoolInput> {
    ensure!(fs::metadata(path)?.len() <= 4096, "pool input bound");
    let input: PoolInput = serde_json::from_reader(File::open(path)?.take(4097))?;
    input.response_policy()?;
    input.session_limit()?;
    validate(input.seconds, input.difficulty_exponents)?;
    if let Some(anchor) = &input.regtest_anchor {
        ensure!(
            input.schema != "md-sim-s9-pool-input/2",
            "old pool schema has no regtest anchor"
        );
        anchor.validate().map_err(anyhow::Error::msg)?;
        ensure!(
            input.time > anchor.genesis_time,
            "job time precedes regtest genesis"
        );
    }
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connection_budget_is_explicit_and_cannot_reinterpret_legacy_inputs() {
        let mut input: PoolInput =
            serde_json::from_value(prepare(270, [-31, -30], None).unwrap()).unwrap();
        assert_eq!(input.session_limit().unwrap(), 128);
        for limit in [0, 15, 129, usize::MAX] {
            input.connection_limit = Some(limit);
            assert!(input.session_limit().is_err());
            assert!(input.session_seed(0).is_err());
        }
        input.connection_limit = None;
        assert!(input.session_limit().is_err());
        for version in 2..=6 {
            input.schema = format!("md-sim-s9-pool-input/{version}");
            input.connection_limit = None;
            assert_eq!(input.session_limit().unwrap(), 16);
            assert!(input.session_seed(15).is_ok());
            assert!(input.session_seed(16).is_err());
            assert!(serde_json::to_value(&input)
                .unwrap()
                .get("connection_limit")
                .is_none());
            input.connection_limit = Some(16);
            assert!(input.session_limit().is_err());
        }
    }

    #[test]
    fn reconnect_changes_work_entropy_and_legacy_inputs_keep_their_identity() {
        let mut input: PoolInput =
            serde_json::from_value(prepare(20, [-28, -27], None).unwrap()).unwrap();
        let seeds = (0..128)
            .map(|id| input.session_seed(id).unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(seeds.len(), 128);
        assert!(!seeds.contains(&input.seed));
        let frozen =
            serde_json::from_slice::<PoolInput>(&serde_json::to_vec(&input).unwrap()).unwrap();
        assert_eq!(
            frozen.session_seed(3).unwrap(),
            input.session_seed(3).unwrap()
        );
        assert!(input.session_seed(128).is_err());
        assert_eq!(
            input.response_policy().unwrap(),
            ResponsePolicy::JsonRpc1StratumV1
        );
        input.schema = "md-sim-s9-pool-input/4".into();
        input.connection_limit = None;
        assert_eq!(
            input.session_seed(3).unwrap(),
            frozen.session_seed(3).unwrap()
        );
        assert_eq!(
            input.response_policy().unwrap(),
            ResponsePolicy::LegacyFalseCode23
        );
        for schema in ["md-sim-s9-pool-input/2", "md-sim-s9-pool-input/3"] {
            input.schema = schema.into();
            assert_eq!(input.session_seed(0).unwrap(), input.seed);
            assert_eq!(input.session_seed(15).unwrap(), input.seed);
            assert_eq!(
                input.response_policy().unwrap(),
                ResponsePolicy::LegacyFalseCode23
            );
        }
        assert_eq!(
            input.completion_policy().unwrap(),
            CompletionPolicy::CloseAfterWork
        );
        input.schema = "md-sim-s9-pool-input/9".into();
        assert!(input.response_policy().is_err());
        assert!(input.session_seed(0).is_err());
    }
    #[test]
    fn persistent_pool_policy_is_a_separate_frozen_identity() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("input.json");
        let value = prepare_window(900, [-31, -30], Some(RegtestAnchor::genesis())).unwrap();
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        let input = read(&path).unwrap();
        assert_eq!(input.schema, "md-sim-s9-pool-input/8");
        assert_eq!(
            input.completion_policy().unwrap(),
            CompletionPolicy::KeepOpen
        );
        assert_eq!(
            input.response_policy().unwrap(),
            ResponsePolicy::JsonRpc1StratumV1
        );
        assert_ne!(
            input.session_seed(0).unwrap(),
            input.session_seed(1).unwrap()
        );
    }
    #[test]
    fn explicit_anchor_roundtrips_and_rejects_changed_or_old_schema_identity() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("input.json");
        let value = prepare(230, [-28, -27], Some(RegtestAnchor::genesis())).unwrap();
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(
            read(&path).unwrap().regtest_anchor,
            Some(RegtestAnchor::genesis())
        );
        for mode in 0..3 {
            let mut bad = value.clone();
            match mode {
                0 => bad["schema"] = serde_json::json!("md-sim-s9-pool-input/2"),
                1 => bad["regtest_anchor"]["candidate_height"] = serde_json::json!(2),
                _ => bad["time"] = serde_json::json!(1),
            }
            fs::write(&path, serde_json::to_vec(&bad).unwrap()).unwrap();
            assert!(read(&path).is_err());
        }
        let mut old = prepare(20, [-28, -27], None).unwrap();
        old["schema"] = serde_json::json!("md-sim-s9-pool-input/2");
        old.as_object_mut().unwrap().remove("connection_limit");
        old.as_object_mut().unwrap().remove("regtest_anchor");
        fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        assert!(read(&path).unwrap().regtest_anchor.is_none());
    }
}

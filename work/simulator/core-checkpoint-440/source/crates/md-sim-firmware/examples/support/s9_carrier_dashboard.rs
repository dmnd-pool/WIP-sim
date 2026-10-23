//! Capture the selected S9 native service policy before execution. This does
//! not create a listener or establish native HTTP readiness.
use anyhow::{ensure, Result};
use md_sim_access::carrier::Policy;
use md_sim_runtime::{dashboard::Source, store::ObjectId};
use std::{fs::OpenOptions, io::Read, os::unix::fs::OpenOptionsExt, path::Path};

pub const FILE: &str = "carrier-dashboard.json";
pub const GUEST: &str = "/carrier-dashboard.json";

pub fn read(path: &Path, support: &ObjectId, wall_seconds: u64) -> Result<(Policy, Vec<u8>)> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.len() <= 16384,
        "dashboard policy file bound"
    );
    let mut bytes = Vec::new();
    file.take(16385).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 16384, "dashboard policy grew beyond bound");
    let policy: Policy = serde_json::from_slice(&bytes)?;
    policy.validate()?;
    ensure!(
        policy.source == Source::AdaptedNative
            && policy.destination == "10.0.2.15:80".parse()?
            && policy.support_identity == *support
            && policy.wall_seconds <= wall_seconds,
        "dashboard policy differs from the selected adapted S9 carrier"
    );
    Ok((policy, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    #[test]
    fn selected_policy_cannot_change_target_source_support_or_wall_budget() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join(FILE);
        let support = ObjectId {
            sha256: "a".repeat(64),
            size: 930,
        };
        let policy = json!({
            "schema":"md-sim-carrier-dashboard-policy/1", "source":"adapted-native",
            "support_identity":support, "destination":"10.0.2.15:80", "listen_port":0,
            "connection_limit":8, "connection_attempt_limit":1024,
            "connection_bytes":67108864, "run_bytes":268435456,
            "connect_milliseconds":1000, "idle_seconds":60, "wall_seconds":300,
            "lease_milliseconds":10000
        });
        let bytes = serde_json::to_vec(&policy)?;
        fs::write(&path, &bytes)?;
        assert_eq!(read(&path, &support, 360)?.1, bytes);
        for (field, value) in [
            ("destination", json!("10.0.2.99:80")),
            ("source", json!("native")),
            (
                "support_identity",
                json!({"sha256":"b".repeat(64),"size":930}),
            ),
            ("wall_seconds", json!(361)),
            ("unknown", json!(true)),
        ] {
            let mut changed = policy.clone();
            changed[field] = value;
            fs::write(&path, serde_json::to_vec(&changed)?)?;
            assert!(
                read(&path, &support, 360).is_err(),
                "accepted changed {field}"
            );
        }
        fs::write(&path, &bytes)?;
        let link = directory.path().join("link");
        std::os::unix::fs::symlink(&path, &link)?;
        assert!(read(&link, &support, 360).is_err());
        assert!(read(directory.path(), &support, 360).is_err());
        Ok(())
    }
}

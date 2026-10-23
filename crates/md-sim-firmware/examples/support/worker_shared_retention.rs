//! The shared pool receipt binds exact native and independent pool artifacts.
//! Host import and guest supervision are separate callers of this contract.
use anyhow::{ensure, Result};
use md_sim_coordinator::RetentionIdentity;
use md_sim_runtime::store::ObjectId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fs::File};

use super::retention_frame as frame;

pub const SCHEMA: &str = "md-sim-shared-pool-retention/1";
pub const ACCESS_SCHEMA: &str = "md-sim-shared-access-retention/2";
pub const REQUEST: &str = "MD_SIM_SHARED_POOL_RETENTION_REQUEST ";
pub const POOL_SCHEMA: &str = "md-sim-shared-pool-artifacts/1";
pub const ACK_BYTES: usize = 4096;
pub const ACK_SECONDS: u64 = 180;
pub const POOL_BUNDLE_BYTES: u64 = 16 << 20;
pub const POOL_COMPRESSED_BYTES: u64 = 17 << 20;
pub const REQUEST_BYTES: usize = 112 << 20;
pub const ACCESS_BUNDLE_BYTES: u64 = 32 << 20;
pub const ACCESS_COMPRESSED_BYTES: u64 = 17 << 20;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub native: RetentionIdentity,
    pub pool_bundle: ObjectId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_bundle: Option<ObjectId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema_version: String,
    pub identity: Receipt,
    pub native_artifacts: Value,
    pub pool_bundle: super::channel_artifacts::Export,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_bundle: Option<super::channel_artifacts::Export>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessBundle {
    pub schema: String,
    pub native: RetentionIdentity,
    pub policy: ObjectId,
    pub provider: ObjectId,
    pub browser_requested: bool,
    pub browser_driver: Option<ObjectId>,
    pub files: BTreeMap<String, Bytes>,
}

pub fn access_name(name: &str) -> Result<()> {
    let parts: Vec<_> = name.split('/').collect();
    ensure!(
        name.len() <= 256
            && parts.len() >= 2
            && parts.len() <= 8
            && [
                "client",
                "server",
                "server-root",
                "metadata",
                "client-observer"
            ]
            .contains(&parts[0])
            && parts.iter().all(|part| !part.is_empty()
                && *part != "."
                && *part != ".."
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte)))
            && !parts
                .iter()
                .any(|part| part.ends_with(".key") || part.ends_with(".p12")),
        "unsafe access evidence filename"
    );
    Ok(())
}

impl Request {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.pool_bundle.object == self.identity.pool_bundle,
            "shared pool bundle identity differs"
        );
        match (&self.access_bundle, &self.identity.access_bundle) {
            (None, None) => ensure!(
                self.schema_version == SCHEMA,
                "shared retention schema differs"
            ),
            (Some(bundle), Some(identity)) => ensure!(
                self.schema_version == ACCESS_SCHEMA && bundle.object == *identity,
                "shared access retention schema or identity differs"
            ),
            _ => anyhow::bail!("shared access retention bundle or identity absent"),
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bytes {
    pub object: ObjectId,
    pub hex: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolBundle {
    pub schema_version: String,
    pub locked_run_hash: String,
    pub token: String,
    pub run_id: String,
    pub process: ObjectId,
    pub manifest: ObjectId,
    pub pool_input: ObjectId,
    pub pool_verifier: ObjectId,
    pub support_runner: ObjectId,
    pub support_backend: ObjectId,
    pub support_kernel: ObjectId,
    pub lease: Value,
    pub outcome: Value,
    pub stdout: Bytes,
    pub stderr: Bytes,
    pub sessions: BTreeMap<String, Bytes>,
}

pub fn read_ack(device: &mut File, expected: &Receipt) -> Result<Receipt> {
    let actual: Receipt = serde_json::from_slice(&frame::read(device, ACK_BYTES, ACK_SECONDS)?)?;
    ensure!(
        actual == *expected,
        "outer shared-pool receipt identity differs"
    );
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Write,
        os::{fd::OwnedFd, unix::net::UnixStream},
        time::Instant,
    };

    fn channel() -> (File, File) {
        let (left, right) = UnixStream::pair().unwrap();
        let left: OwnedFd = left.into();
        let right: OwnedFd = right.into();
        (File::from(left), File::from(right))
    }
    fn expected() -> Receipt {
        Receipt {
            native: RetentionIdentity {
                locked_run_hash: "11".repeat(32),
                token: "22".repeat(32),
                run_id: format!("run-{}", "22".repeat(32)),
                result: ObjectId {
                    sha256: "33".repeat(32),
                    size: 7,
                },
            },
            pool_bundle: ObjectId {
                sha256: "44".repeat(32),
                size: 9,
            },
            access_bundle: None,
        }
    }
    fn wire(receipt: &Receipt) -> Vec<u8> {
        let mut bytes = serde_json::to_vec(receipt).unwrap();
        bytes.push(b'\n');
        bytes
    }
    #[test]
    fn actual_channel_accepts_only_the_complete_joined_identity() {
        let expected = expected();
        let (mut reader, mut writer) = channel();
        writer.write_all(&wire(&expected)).unwrap();
        assert_eq!(read_ack(&mut reader, &expected).unwrap(), expected);
        for wrong_pool in [false, true] {
            let (mut reader, mut writer) = channel();
            let mut wrong = expected.clone();
            if wrong_pool {
                wrong.pool_bundle.size += 1;
            } else {
                wrong.native.result.size += 1;
            }
            writer.write_all(&wire(&wrong)).unwrap();
            assert!(read_ack(&mut reader, &expected)
                .unwrap_err()
                .to_string()
                .contains("identity differs"));
        }
    }
    #[test]
    fn extra_oversized_and_partial_frames_are_rejected() {
        let expected = expected();
        for bytes in [wire(&expected).repeat(2), vec![b'x'; ACK_BYTES + 1]] {
            let (mut reader, mut writer) = channel();
            writer.write_all(&bytes).unwrap();
            assert!(read_ack(&mut reader, &expected).is_err());
        }
        let (mut reader, mut writer) = channel();
        writer.write_all(b"{").unwrap();
        drop(writer);
        assert!(read_ack(&mut reader, &expected).is_err());
    }
    #[test]
    fn silent_actual_peer_hits_the_declared_frame_deadline() {
        let (mut reader, _writer) = channel();
        let began = Instant::now();
        assert!(frame::read(&mut reader, ACK_BYTES, 1)
            .unwrap_err()
            .to_string()
            .contains("deadline"));
        assert!(began.elapsed().as_millis() >= 900);
    }
    #[test]
    fn access_ack_binds_the_additional_bundle_and_never_accepts_missing_identity() {
        let mut expected = expected();
        expected.access_bundle = Some(ObjectId {
            sha256: "55".repeat(32),
            size: 123,
        });
        let (mut reader, mut writer) = channel();
        let mut missing = expected.clone();
        missing.access_bundle = None;
        writer.write_all(&wire(&missing)).unwrap();
        assert!(read_ack(&mut reader, &expected).is_err());
        let (mut reader, mut writer) = channel();
        writer.write_all(&wire(&expected)).unwrap();
        assert_eq!(read_ack(&mut reader, &expected).unwrap(), expected);
    }
    #[test]
    fn access_paths_reject_escape_and_secret_material() {
        for name in [
            "../client/a",
            "server/../../a",
            "server//a",
            "/server/a",
            "server/a.key",
            "metadata/client.p12",
            "server/a\n",
            "pool/session.json",
        ] {
            assert!(access_name(name).is_err(), "{name}");
        }
        for name in [
            "client/http/report.json",
            "server/bridge/endpoint.json",
            "metadata/provider.json",
        ] {
            access_name(name).unwrap();
        }
    }
}

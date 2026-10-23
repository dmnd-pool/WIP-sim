//! Exact signed SD update for the selected archived2022 reference firmware.
use anyhow::{ensure, Result};
use md_sim_runtime::store::{hash_file, ObjectId};
use serde_json::Value;
use std::path::Path;

pub const ENVIRONMENT: &str = "MD_SIM_S9_UPDATE_VALIDATION";
pub const SCHEMA: &str = "md-sim-s9-sd-update-validation/3";
pub const MAINTENANCE: &str = "native-stop-validate-cancel-start/1";
pub const INSTALL_SCHEMA: &str = "md-sim-s9-sd-update-install/2";
pub const INSTALL_MAINTENANCE: &str = "native-stop-validate-install-keep-reboot/1";
pub const REJECTION_SCHEMA: &str = "md-sim-s9-sd-update-payload-rejection/1";
pub const REJECTION_MAINTENANCE: &str = "native-stop-reject-payload-fault-start/1";
// Reproducible output of s9_update_fault: one XOR01 at archive offset9168902.
// The exact original signature trailer and every other byte are retained.
pub const FAULT_SHA256: &str = "98a5748d67fd98e4be0e54fb752f82620163f16b848cbdd0c6ee1ddf89b6ef02";
pub const FAULT_OFFSET: u64 = 9_168_902;
pub const RESTART_SECONDS: u64 = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    ValidateCancel,
    InstallKeep,
    RejectTampered,
}
impl Action {
    pub fn schema(self) -> &'static str {
        match self {
            Self::ValidateCancel => SCHEMA,
            Self::InstallKeep => INSTALL_SCHEMA,
            Self::RejectTampered => REJECTION_SCHEMA,
        }
    }
    pub fn maintenance(self) -> &'static str {
        match self {
            Self::ValidateCancel => MAINTENANCE,
            Self::InstallKeep => INSTALL_MAINTENANCE,
            Self::RejectTampered => REJECTION_MAINTENANCE,
        }
    }
    pub fn environment(self) -> &'static str {
        match self {
            Self::ValidateCancel => "1",
            Self::InstallKeep => "install-keep",
            Self::RejectTampered => "reject-tampered",
        }
    }
    pub fn operation(self) -> &'static str {
        match self {
            Self::ValidateCancel => "observe two mining jobs, native stop, upload, native signature/format validation, checksum confirmation, cancel, native start, observe fresh mining jobs; no flash",
            Self::InstallKeep => "observe two mining jobs, native stop, upload, native signature/format validation, checksum confirmation, native step2 with keep=1, observe actual kernel restart and emulator exit; post-update boot/readback remains a separate required test",
            Self::RejectTampered => "observe two mining jobs, native stop, upload exact single-bit damaged signed image, require native rejection without install controls, native start, observe fresh mining jobs; no flash",
        }
    }
    pub fn from_schema(schema: &str) -> Result<Self> {
        match schema {
            SCHEMA => Ok(Self::ValidateCancel),
            INSTALL_SCHEMA => Ok(Self::InstallKeep),
            REJECTION_SCHEMA => Ok(Self::RejectTampered),
            _ => anyhow::bail!("unknown native update contract"),
        }
    }
    pub fn result_field(self) -> &'static str {
        match self {
            Self::RejectTampered => "rejected_update",
            _ => "valid_update",
        }
    }
    pub fn checked_image(self, path: &Path) -> Result<ObjectId> {
        let image = hash_file(path)?;
        let expected = if self == Self::RejectTampered {
            FAULT_SHA256
        } else {
            SHA256
        };
        ensure!(
            image.sha256 == expected && image.size == BYTES,
            "selected2022 SD update identity differs"
        );
        Ok(image)
    }
}
pub const GUEST_IMAGE: &str = "/dashboard-update.tar";
pub const ARTIFACT: &str = "braiins-feed-25bba3b4a505ccbc0e54";
pub const ARCHIVE_IMAGE: &str = "artifacts/files/firmware/braiins-feed-25bba3b4a505ccbc0e54/sd-firmware_2022-04-29-0-5eb2c5fe-22.02.4-plus_arm_cortex-a9_neon.tar";
pub const SHA256: &str = "469743ebcd5faf3445e4f4209ad8af58cc7adeff37c7df28af3c53a5b255b56e";
pub const BYTES: u64 = 19_179_888;
// This bounds the complete client operation, including native parsing,
// signature checks and checksums of the 19 MiB image. Runs 061–063 and 067
// received the complete image but reached the former 60-second client bound
// while the original validator was still running. Firmware deadlines,
// ordinary HTTP limits and the connection timeout are not changed.
pub const CLIENT_SECONDS: u64 = 120;
pub const POOL_SECONDS: u64 = 300;
// Include the complete hexadecimal packet export and ASIC transaction trace.
// Run 070 exceeded the old aggregate 64 MiB bound; per-file limits stay 64 MiB.
pub const TRACE_BYTES: u64 = 128 * 1024 * 1024;
// A complete upload plus Ethernet/IP/TCP headers, native HTTP and pool traffic.
// No packet payload is truncated.
pub const PACKET_BYTES: usize = 24 * 1024 * 1024;

pub fn checked_image(path: &Path) -> Result<ObjectId> {
    Action::ValidateCancel.checked_image(path)
}

pub fn selected_environment() -> Result<bool> {
    Ok(selected_action()?.is_some())
}

pub fn selected_action() -> Result<Option<Action>> {
    match std::env::var(ENVIRONMENT) {
        Ok(value) if value == "1" => Ok(Some(Action::ValidateCancel)),
        Ok(value) if value == "install-keep" => Ok(Some(Action::InstallKeep)),
        Ok(value) if value == "reject-tampered" => Ok(Some(Action::RejectTampered)),
        Ok(_) => anyhow::bail!("unknown update action"),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Validate the native HTTP observation. Kernel restart and disk readback
/// remain separate gates; this function cannot establish installation alone.
pub fn verify_completion(record: &Value, action: Action) -> Result<()> {
    let update = &record["result"]["luci"][action.result_field()];
    ensure!(
        record["passed"] == true
            && record["origin"] == "http://10.0.2.15"
            && update["contract"]["schema"] == action.schema()
            && update["contract"]["maintenance"] == action.maintenance()
            && update["maintenance"]["stop"]["port"]["connection_refused"] == true,
        "native upload validation failed or missing: {}",
        record["error"]
    );
    let matches = match action {
        Action::ValidateCancel => {
            update["cancelled"] == true
                && update["commit_requested"] == false
                && update["maintenance"]["start"]["port"]["listening"] == true
        }
        Action::InstallKeep => {
            update["cancelled"] == false
                && update["commit_requested"] == true
                && update["keep_settings"] == true
                && update["step"] == 2
                && update["reported_sha256"] == SHA256
                && update["contract"]["restart_observation_seconds"] == RESTART_SECONDS
                && update["native_commit_response"]["size"]
                    .as_u64()
                    .is_some_and(|size| size > 0)
        }
        Action::RejectTampered => {
            update["rejected"] == true
                && update["commit_requested"] == false
                && update["install_controls_present"] == false
                && update["contract"]["object"]["sha256"] == FAULT_SHA256
                && update["contract"]["object"]["size"] == BYTES
                && update["contract"]["parent"]["sha256"] == SHA256
                && update["contract"]["parent"]["size"] == BYTES
                && update["contract"]["fault"]
                    == serde_json::json!({"archive_offset":FAULT_OFFSET,"xor_mask":1})
                && update["maintenance"]["start"]["port"]["listening"] == true
                && update["native_rejection"]["size"]
                    .as_u64()
                    .is_some_and(|size| size > 0)
        }
    };
    ensure!(
        matches,
        "native update result does not match the requested operation"
    );
    Ok(())
}

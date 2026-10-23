use crate::{
    artifact_constraints, content_hash, valid_sha256, Catalog, PlanState, Reason, VerifiedArtifact,
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionMode {
    BoardEmulation,
    ProcessEmulation,
    SourceAdapter,
    ApiModel,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FirmwareIdentity {
    OriginalBytes,
    RebuiltSource,
    PatchedBinary,
    NotExecuted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BootEntry {
    Reset,
    Bootloader,
    Kernel,
    Process,
    None,
}

/// The grouped values hold board-specific fields. Required fields are checked
/// below. Unknown values must be JSON null and have an entry in `unknowns`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HardwareProfileV2 {
    pub schema_version: String,
    pub id: String,
    pub revision: Option<String>,
    pub device_aliases: Vec<String>,
    pub evidence_ids: Vec<String>,
    pub controller: Value,
    pub memory: Value,
    pub storage: Value,
    pub hashboards: Value,
    pub auxiliary_controllers: Value,
    pub power: Value,
    pub cooling: Value,
    pub interfaces: Value,
    pub unknowns: Vec<Reason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleComponent {
    pub id: String,
    pub parent_artifact_id: String,
    pub parent_sha256: String,
    pub sha256: String,
    pub archive_member: Option<String>,
    pub byte_range: Option<[u64; 2]>,
    pub role: String,
    pub destination: Value,
    pub preparation_tool_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirmwareBundleV2 {
    pub schema_version: String,
    pub id: String,
    pub artifact_ids: Vec<String>,
    pub components: Vec<BundleComponent>,
    pub installation_path: Vec<Value>,
    pub starting_state_sha256: String,
    pub missing_inputs: Vec<Reason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstraintReview {
    pub artifact_id: String,
    pub constraints_sha256: String,
    pub state: PlanState,
    pub evidence_ids: Vec<String>,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetDefinitionV2 {
    pub schema_version: String,
    pub id: String,
    /// Pin SnapshotManifest.catalogue_data_hash. The full snapshot additionally
    /// freezes this target and registry files, without a circular self identity.
    pub snapshot_hash: String,
    pub device_id: String,
    pub firmware_entity_id: String,
    pub hardware: HardwareProfileV2,
    pub firmware: FirmwareBundleV2,
    pub execution_mode: ExecutionMode,
    pub firmware_identity: FirmwareIdentity,
    pub environment_identity: Vec<Value>,
    pub boot_entry: BootEntry,
    pub adaptation_manifest: Value,
    pub backend_build_sha256: String,
    pub required_models: BTreeMap<String, String>,
    pub constraint_reviews: Vec<ConstraintReview>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendRegistry {
    pub builds: BTreeMap<String, BTreeSet<ExecutionMode>>,
    pub model_builds: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetResolution {
    pub target_id: String,
    pub target_hash: String,
    pub hardware_profile_hash: String,
    pub firmware_bundle_hash: String,
    pub adaptation_hash: String,
    pub state: PlanState,
    pub reasons: Vec<Reason>,
    pub unresolved_fields: Vec<String>,
    pub required_input_hashes: BTreeMap<String, String>,
}

fn known(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(v) => {
            !v.trim().is_empty() && !["unknown", "unspecified", "TODO"].contains(&v.as_str())
        }
        Value::Number(_) => true,
        Value::Object(v) => !v.is_empty(),
        Value::Array(_) => true,
        Value::Bool(_) => true,
    }
}

impl HardwareProfileV2 {
    pub fn unresolved_fields(&self) -> Result<Vec<String>> {
        ensure!(
            self.schema_version == "2.0",
            "version 1 profiles must use explicit migration"
        );
        let value = serde_json::to_value(self)?;
        let mut missing = Vec::new();
        for path in [
            "/id",
            "/revision",
            "/device_aliases",
            "/evidence_ids",
            "/controller/board_id",
            "/controller/pcb_revision",
            "/controller/soc",
            "/controller/cpu/type",
            "/controller/cpu/count",
            "/controller/isa",
            "/controller/endian",
            "/controller/reset_entry",
            "/controller/address_map",
            "/memory/ram_layout",
            "/memory/rom_layout",
            "/memory/reserved_ranges",
            "/storage/bus",
            "/storage/chip_id",
            "/storage/size_bytes",
            "/storage/geometry",
            "/storage/ecc_oob",
            "/storage/partitions",
            "/storage/boot_order",
            "/storage/write_protection",
            "/hashboards/slots",
            "/auxiliary_controllers",
            "/power/psu_id",
            "/power/revision",
            "/power/control_interface",
            "/power/rails",
            "/power/startup_sequence",
            "/cooling/mode",
            "/cooling/sensor_layout",
            "/cooling/actuators",
            "/interfaces/bindings",
            "/interfaces/timing",
            "/interfaces/implementation_hashes",
        ] {
            if !value.pointer(path).is_some_and(known) {
                missing.push(path.to_string());
            }
        }
        for path in [
            "/id",
            "/revision",
            "/controller/board_id",
            "/controller/pcb_revision",
            "/controller/soc",
            "/controller/cpu/type",
            "/controller/isa",
            "/controller/endian",
            "/storage/bus",
            "/storage/chip_id",
            "/power/psu_id",
            "/power/revision",
            "/power/control_interface",
            "/cooling/mode",
        ] {
            if value
                .pointer(path)
                .and_then(Value::as_str)
                .is_none_or(|v| v.trim().is_empty())
            {
                missing.push(format!("{path} (must be a nonempty string)"));
            }
        }
        if self.controller["reset_entry"].as_u64().is_none() {
            missing.push("/controller/reset_entry (must be an unsigned address)".into());
        }
        for (path, allow_empty) in [
            ("/controller/address_map", false),
            ("/memory/ram_layout", false),
            ("/memory/rom_layout", true),
            ("/memory/reserved_ranges", true),
        ] {
            match value.pointer(path).and_then(Value::as_array) {
                Some(ranges) => {
                    if !allow_empty && ranges.is_empty() {
                        missing.push(format!("{path} (empty layout)"));
                    }
                    for (index, range) in ranges.iter().enumerate() {
                        let base = range["base"].as_u64();
                        let size = range["size"].as_u64();
                        if !base.zip(size).is_some_and(|(base, size)| {
                            size > 0 && base.checked_add(size).is_some()
                        }) {
                            missing.push(format!("{path}/{index} (invalid base or size)"));
                        }
                    }
                }
                None => missing.push(format!("{path} (must be an array of address ranges)")),
            }
        }
        if !self.storage["geometry"]
            .as_object()
            .is_some_and(|geometry| {
                ["erase_bytes", "block_bytes"].iter().any(|key| {
                    geometry
                        .get(*key)
                        .and_then(Value::as_u64)
                        .is_some_and(|n| n > 0)
                }) && geometry.values().all(|v| v.as_u64().is_some_and(|n| n > 0))
            })
        {
            missing.push("/storage/geometry (requires positive erase_bytes or block_bytes and positive numeric geometry)".into());
        }
        if !self.storage["partitions"]
            .as_array()
            .is_some_and(|partitions| {
                !partitions.is_empty()
                    && partitions.iter().all(|p| {
                        p["offset"].as_u64().zip(p["size"].as_u64()).is_some_and(
                            |(offset, size)| {
                                size > 0
                                    && offset.checked_add(size).is_some_and(|end| {
                                        self.storage["size_bytes"]
                                            .as_u64()
                                            .is_some_and(|total| end <= total)
                                    })
                            },
                        )
                    })
            })
        {
            missing.push("/storage/partitions (invalid or missing partition ranges)".into());
        }
        if !self.storage["write_protection"].is_boolean() {
            missing.push("/storage/write_protection (must be boolean)".into());
        }
        for path in ["/storage/boot_order", "/power/startup_sequence"] {
            if !value
                .pointer(path)
                .and_then(Value::as_array)
                .is_some_and(|a| {
                    !a.is_empty() && a.iter().all(|v| v.as_str().is_some_and(|v| !v.is_empty()))
                })
            {
                missing.push(format!("{path} (must be a nonempty string array)"));
            }
        }
        if let Some(slots) = self.hashboards["slots"].as_array() {
            if slots.is_empty() {
                missing.push("/hashboards/slots (no hashboards)".into());
            }
            for (index, slot) in slots.iter().enumerate() {
                for key in [
                    "board_id",
                    "revision",
                    "asic_type",
                    "asic_revision",
                    "chip_count",
                    "chip_addresses",
                    "chain_links",
                    "reset_domain",
                ] {
                    if !slot.get(key).is_some_and(known) {
                        missing.push(format!("/hashboards/slots/{index}/{key}"));
                    }
                }
                for key in [
                    "board_id",
                    "revision",
                    "asic_type",
                    "asic_revision",
                    "reset_domain",
                ] {
                    if slot[key].as_str().is_none_or(|s| s.is_empty()) {
                        missing.push(format!(
                            "/hashboards/slots/{index}/{key} (must be a nonempty string)"
                        ));
                    }
                }
                if slot["chip_count"].as_u64().is_none_or(|count| count == 0) {
                    missing.push(format!(
                        "/hashboards/slots/{index}/chip_count (must be positive)"
                    ));
                }
                let addresses = slot["chip_addresses"]
                    .as_array()
                    .map(|a| a.iter().map(Value::as_u64).collect::<Option<BTreeSet<_>>>());
                if !matches!(&addresses, Some(Some(a)) if Some(a.len() as u64)==slot["chip_count"].as_u64() && Some(a.len())==slot["chip_addresses"].as_array().map(Vec::len))
                {
                    missing.push(format!("/hashboards/slots/{index}/chip_addresses (count, type, or uniqueness mismatch)"));
                }
                if !slot["chain_links"].as_array().is_some_and(|links| links.iter().all(|link| {
                    let endpoints=link["from"].as_u64().zip(link["to"].as_u64());
                    matches!((&addresses,endpoints), (Some(Some(a)),Some((from,to))) if from != to && a.contains(&from) && a.contains(&to))
                })) { missing.push(format!("/hashboards/slots/{index}/chain_links (invalid link endpoints)")); }
            }
        } else {
            missing.push("/hashboards/slots (must be an array)".into());
        }
        if self.controller["cpu"]["count"]
            .as_u64()
            .is_none_or(|count| count == 0)
        {
            missing.push("/controller/cpu/count (must be positive)".into());
        }
        if self.storage["size_bytes"]
            .as_u64()
            .is_none_or(|size| size == 0)
        {
            missing.push("/storage/size_bytes (must be positive)".into());
        }
        if self.evidence_ids.is_empty() {
            missing.push("/evidence_ids (empty)".into());
        }
        for (path, keys, allow_empty) in [
            ("/power/rails", &["name"][..], false),
            (
                "/cooling/sensor_layout",
                &["id", "type", "location"][..],
                false,
            ),
            ("/cooling/actuators", &["id", "type"][..], true),
            (
                "/interfaces/bindings",
                &["id", "kind", "endpoint"][..],
                false,
            ),
        ] {
            if !value
                .pointer(path)
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    (allow_empty || !items.is_empty())
                        && items.iter().all(|item| {
                            keys.iter()
                                .all(|key| item[*key].as_str().is_some_and(|v| !v.is_empty()))
                        })
                })
            {
                missing.push(format!("{path} (invalid typed entries)"));
            }
        }
        if !self.auxiliary_controllers.as_array().is_some_and(|items| {
            items.iter().all(|item| {
                item["mcu_type"].as_str().is_some_and(|v| !v.is_empty())
                    && ["program_sha256", "model_build_sha256"]
                        .iter()
                        .any(|key| item[*key].as_str().is_some_and(valid_sha256))
            })
        }) {
            missing.push("/auxiliary_controllers (invalid MCU identity)".into());
        }
        if !self.interfaces["implementation_hashes"]
            .as_object()
            .is_some_and(|entries| {
                !entries.is_empty()
                    && entries
                        .values()
                        .all(|hash| hash.as_str().is_some_and(valid_sha256))
            })
        {
            missing.push("/interfaces/implementation_hashes (invalid model hashes)".into());
        }
        missing.extend(
            self.unknowns
                .iter()
                .map(|r| format!("declared: {}", r.detail)),
        );
        Ok(missing)
    }
}

impl Catalog {
    /// Hash every relationship and per-artifact constraint that a target author
    /// must review. A family support claim alone cannot satisfy this review.
    pub fn target_constraints_hash(
        &self,
        device: &str,
        firmware: &str,
        artifact: &str,
    ) -> Result<String> {
        let record = self.artifacts.get(artifact).context("unknown artifact")?;
        let relations: Vec<_> = self
            .relationships
            .values()
            .filter(|r| {
                (r["from_id"] == firmware && r["to_id"] == device)
                    || (r["from_id"] == device && r["to_id"] == firmware)
            })
            .collect();
        let device = self.entities.get(device).context("unknown device")?;
        let firmware = self.entities.get(firmware).context("unknown firmware")?;
        content_hash(&(
            device,
            firmware,
            record,
            artifact_constraints(record),
            relations,
        ))
    }

    /// Resolve an explicit v2 target. This never derives compatibility from its
    /// filename, chipset, or a firmware family's list of attached artifacts.
    pub fn resolve_target(
        &self,
        target: &TargetDefinitionV2,
        registry: &BackendRegistry,
        verified: &BTreeMap<String, VerifiedArtifact>,
    ) -> Result<TargetResolution> {
        ensure!(
            target.schema_version == "2.0" && target.firmware.schema_version == "2.0",
            "unsupported target or bundle schema"
        );
        ensure!(
            target.snapshot_hash == self.snapshot.manifest.catalogue_data_hash,
            "target references another frozen catalogue data identity"
        );
        let device = self
            .entities
            .get(&target.device_id)
            .context("unknown device ID")?;
        ensure!(
            device["kind"] == "device",
            "target device ID has the wrong role"
        );
        ensure!(
            target.hardware.device_aliases.contains(&target.device_id),
            "profile does not name selected device"
        );
        let firmware = self
            .entities
            .get(&target.firmware_entity_id)
            .context("unknown firmware entity")?;
        ensure!(
            firmware["kind"] == "firmware",
            "target firmware entity has the wrong role"
        );
        ensure!(
            valid_sha256(&target.backend_build_sha256),
            "invalid backend build hash"
        );
        match target.execution_mode {
            ExecutionMode::ApiModel => ensure!(
                target.firmware_identity == FirmwareIdentity::NotExecuted
                    && target.boot_entry == BootEntry::None,
                "API models must declare firmware not executed and no boot entry"
            ),
            ExecutionMode::SourceAdapter => ensure!(
                target.firmware_identity == FirmwareIdentity::RebuiltSource
                    && target.boot_entry != BootEntry::None,
                "source adapter must identify rebuilt source and an execution entry"
            ),
            ExecutionMode::ProcessEmulation => ensure!(
                target.boot_entry == BootEntry::Process
                    && target.firmware_identity != FirmwareIdentity::NotExecuted,
                "process execution identity is inconsistent"
            ),
            ExecutionMode::BoardEmulation => ensure!(
                matches!(
                    target.boot_entry,
                    BootEntry::Reset | BootEntry::Bootloader | BootEntry::Kernel
                ) && target.firmware_identity != FirmwareIdentity::NotExecuted,
                "board execution identity is inconsistent"
            ),
        }
        ensure!(
            target.adaptation_manifest.is_array() || target.adaptation_manifest.is_object(),
            "adaptation manifest must explicitly record changes or their absence"
        );
        if target.execution_mode == ExecutionMode::SourceAdapter {
            ensure!(
                target.adaptation_manifest["changes"].is_array()
                    && target.adaptation_manifest["reason"]
                        .as_str()
                        .is_some_and(|s| !s.trim().is_empty()),
                "source adapter requires an explicit changes list and reason"
            );
        }
        let mut reasons = Vec::new();
        let unresolved_fields = target.hardware.unresolved_fields()?;
        let mut unknown = !unresolved_fields.is_empty();
        let mut conflict = false;
        if unknown {
            reasons.push(Reason::new(
                "hardware-identity-incomplete",
                "Exact hardware fields or evidence remain unresolved.",
                "Resolve every listed field before selecting an executable target.",
            ));
        }
        for evidence in &target.hardware.evidence_ids {
            if !self.sources.contains_key(evidence)
                && !self.relationships.contains_key(evidence)
                && !self.artifacts.contains_key(evidence)
            {
                unknown = true;
                reasons.push(Reason::new(
                    "hardware-evidence-unresolved",
                    &format!(
                        "Hardware evidence ID {evidence} does not exist in the frozen catalogue."
                    ),
                    "Supply and freeze the actual source evidence.",
                ));
            }
        }
        ensure!(
            target
                .firmware
                .artifact_ids
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
                == target.firmware.artifact_ids.len(),
            "duplicate bundle artifact ID"
        );
        ensure!(
            target
                .firmware
                .components
                .iter()
                .map(|c| &c.id)
                .collect::<BTreeSet<_>>()
                .len()
                == target.firmware.components.len(),
            "duplicate bundle component ID"
        );
        let mut input_hashes = BTreeMap::new();
        if target.firmware.artifact_ids.is_empty() {
            reasons.push(Reason::new(
                "bundle-has-no-artifacts",
                "The firmware bundle has no source artifacts.",
                "Select the exact archived inputs.",
            ));
        }
        let mut reviewed = BTreeSet::new();
        for review in &target.constraint_reviews {
            ensure!(
                reviewed.insert(&review.artifact_id),
                "duplicate artifact constraint review"
            );
        }
        for artifact_id in &target.firmware.artifact_ids {
            let record = self
                .artifacts
                .get(artifact_id)
                .context("bundle references unknown artifact")?;
            let hash = record["sha256"].as_str().unwrap_or("");
            if valid_sha256(hash) {
                input_hashes.insert(artifact_id.clone(), hash.into());
            }
            let valid_input = verified.get(artifact_id).is_some_and(|v| {
                v.sha256 == hash && v.record_hash == content_hash(record).unwrap_or_default()
            });
            if !valid_input {
                reasons.push(Reason::new(
                    "artifact-bytes-not-verified",
                    &format!("Artifact {artifact_id} has no matching byte verification."),
                    "Verify archived bytes against the frozen record before preparation.",
                ));
            }
            let expected = self.target_constraints_hash(
                &target.device_id,
                &target.firmware_entity_id,
                artifact_id,
            )?;
            match target
                .constraint_reviews
                .iter()
                .find(|r| &r.artifact_id == artifact_id)
            {
                Some(review)
                    if review.constraints_sha256 == expected
                        && !review.explanation.trim().is_empty()
                        && !review.evidence_ids.is_empty() =>
                {
                    for evidence in &review.evidence_ids {
                        ensure!(
                            self.sources.contains_key(evidence)
                                || self.relationships.contains_key(evidence)
                                || self.artifacts.contains_key(evidence),
                            "constraint review references unknown evidence {evidence}"
                        );
                    }
                    match review.state {
                        PlanState::Runnable => {}
                        PlanState::Incompatible => {
                            conflict = true;
                            reasons.push(Reason {code: "evidenced-artifact-conflict".into(), detail: review.explanation.clone(), evidence_ids: review.evidence_ids.clone(), next_action: "Select a different exact target; retain this exclusion evidence.".into()});
                        }
                        _ => {
                            unknown = true;
                            reasons.push(Reason::new(
                                "artifact-constraints-unresolved",
                                &review.explanation,
                                "Resolve the artifact-specific constraints.",
                            ));
                        }
                    }
                }
                _ => {
                    unknown = true;
                    reasons.push(Reason::new("artifact-review-missing-or-stale", &format!("No current evidence-backed constraint review exists for {artifact_id}."), "Review the artifact against this exact profile and frozen constraints."));
                }
            }
        }
        if target.firmware.components.is_empty() {
            reasons.push(Reason::new(
                "component-map-missing",
                "The firmware bundle has no component map.",
                "Prepare and hash each required component in isolation.",
            ));
        }
        for component in &target.firmware.components {
            ensure!(
                valid_sha256(&component.sha256) && valid_sha256(&component.preparation_tool_sha256),
                "invalid component or preparation-tool hash"
            );
            ensure!(
                input_hashes.get(&component.parent_artifact_id) == Some(&component.parent_sha256),
                "component parent identity mismatch"
            );
            ensure!(
                component.archive_member.is_some() || component.byte_range.is_some(),
                "component lacks parent location"
            );
            ensure!(
                !component.role.is_empty() && known(&component.destination),
                "component role or destination is unresolved"
            );
            let component_key = format!("component:{}", component.id);
            if !verified.get(&component_key).is_some_and(|v| {
                v.sha256 == component.sha256
                    && v.record_hash == content_hash(component).unwrap_or_default()
            }) {
                reasons.push(Reason::new(
                    "component-bytes-not-verified",
                    &format!(
                        "Prepared component {} has no matching byte verification.",
                        component.id
                    ),
                    "Verify the prepared component bytes and their recorded parent mapping.",
                ));
            }
            if let Some([offset, length]) = component.byte_range {
                let size = self.artifacts[&component.parent_artifact_id]["size_bytes"]
                    .as_u64()
                    .context("component parent lacks size")?;
                ensure!(
                    length > 0 && offset.checked_add(length).is_some_and(|end| end <= size),
                    "component range escapes parent"
                );
            }
        }
        if !valid_sha256(&target.firmware.starting_state_sha256) {
            reasons.push(Reason::new(
                "starting-state-missing",
                "No exact starting storage fixture is selected.",
                "Prepare the baseline and installation path.",
            ));
        }
        reasons.extend(target.firmware.missing_inputs.clone());
        if !registry
            .builds
            .get(&target.backend_build_sha256)
            .is_some_and(|modes| modes.contains(&target.execution_mode))
        {
            reasons.push(Reason::new(
                "backend-not-implemented",
                "The selected backend build does not provide the declared execution mode.",
                "Implement and verify the backend; do not classify this as incompatible hardware.",
            ));
        }
        if target.required_models.is_empty() {
            reasons.push(Reason::new(
                "device-models-unresolved",
                "No exact guest-facing model builds are selected.",
                "Declare and implement the required device interfaces.",
            ));
        }
        for (model, hash) in &target.required_models {
            ensure!(valid_sha256(hash), "invalid model build hash");
            if registry.model_builds.get(model) != Some(hash) {
                reasons.push(Reason::new(
                    "device-model-not-implemented",
                    &format!("Required model {model} is unavailable at the selected build."),
                    "Implement and verify the exact interface.",
                ));
            }
        }
        let state = if conflict {
            PlanState::Incompatible
        } else if unknown {
            PlanState::Unknown
        } else if !reasons.is_empty() {
            PlanState::Blocked
        } else {
            PlanState::Runnable
        };
        Ok(TargetResolution {
            target_id: target.id.clone(),
            target_hash: content_hash(target)?,
            hardware_profile_hash: content_hash(&target.hardware)?,
            firmware_bundle_hash: content_hash(&target.firmware)?,
            adaptation_hash: content_hash(&target.adaptation_manifest)?,
            state,
            reasons,
            unresolved_fields,
            required_input_hashes: input_hashes,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeIdentity {
    /// Complete resolved specification; never just the selected target name.
    pub resolved_run_specification: Value,
    pub suite_sha256: String,
    pub verifier_sha256: String,
    pub referenced_inputs: BTreeMap<String, String>,
}

impl ResumeIdentity {
    pub fn hash(&self) -> Result<String> {
        ensure!(
            valid_sha256(&self.suite_sha256) && valid_sha256(&self.verifier_sha256),
            "invalid suite or verifier identity"
        );
        ensure!(
            !self.referenced_inputs.is_empty()
                && self.referenced_inputs.values().all(|h| valid_sha256(h)),
            "invalid referenced input identities"
        );
        for name in [
            "target_hash",
            "firmware_bundle_hash",
            "backend_build_hash",
            "adaptation_hash",
            "scenario_hash",
            "seed",
            "hashing",
            "low_rate",
            "clock",
            "network",
            "resources",
            "timeouts",
            "persistent_state_fixture_hash",
            "dashboard",
        ] {
            ensure!(
                self.resolved_run_specification
                    .get(name)
                    .is_some_and(|v| !v.is_null()),
                "resolved run specification lacks {name}"
            );
        }
        ensure!(
            self.resolved_run_specification.get("run_id").is_none()
                && self.resolved_run_specification.get("created_at").is_none(),
            "attempt metadata must not enter the resume identity"
        );
        content_hash(&("md-sim-test-identity-v2", self))
    }
}

/// Preserve the old validation claim as historical data. It cannot be converted
/// into independent capability passes without the original gate evidence.
pub fn migrate_v1_profile(source: &Value) -> Result<Value> {
    ensure!(
        source["schema_version"] == "1.0",
        "migration input must be version 1"
    );
    Ok(json!({
        "schema_version": "2.0", "migration_status": "requires-evidence-resolution",
        "source_profile_sha256": content_hash(source)?, "legacy_profile": source,
        "capability_results": {}, "reasons": [Reason::new("legacy-validation-not-a-gate", "Version 1 validation.level is preserved as a historical claim. It does not pass a version 2 gate.", "Resolve the exact v2 hardware, bundle, and execution contract and run the required gates.")]
    }))
}

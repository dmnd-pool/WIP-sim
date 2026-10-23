use crate::{content_hash, Catalog};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum PlanState {
    Runnable,
    Blocked,
    Unknown,
    Incompatible,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Pairing {
    Relevant,
    Cartesian,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reason {
    pub code: String,
    pub detail: String,
    pub evidence_ids: Vec<String>,
    pub next_action: String,
}

impl Reason {
    pub fn new(code: &str, detail: &str, next_action: &str) -> Self {
        Self {
            code: code.into(),
            detail: detail.into(),
            evidence_ids: vec![],
            next_action: next_action.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRow {
    pub id: String,
    pub record_kind: String,
    pub record_id: String,
    pub record_hash: String,
    pub device_id: Option<String>,
    pub firmware_id: Option<String>,
    pub artifact_id: Option<String>,
    pub state: PlanState,
    pub reasons: Vec<Reason>,
    pub evidence_ids: Vec<String>,
    pub constraints: Vec<Value>,
    pub required_fields: Vec<String>,
}

/// Every Cartesian pair has an explicit default rule. `pair` rows override that
/// rule. This represents millions of unknown pairs without silently dropping rows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartesianDomain {
    pub device_ids: Vec<String>,
    pub firmware_artifact_ids: Vec<String>,
    pub total_pairs: u64,
    pub explicit_pairs: u64,
    pub default_state: PlanState,
    pub default_reason: Reason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatrixPlan {
    pub schema_version: String,
    pub plan_hash: String,
    pub snapshot_hash: String,
    pub pairing: Pairing,
    pub selection_rule: String,
    pub inventory_entities: u64,
    pub inventory_artifacts: u64,
    pub rows: Vec<PlanRow>,
    pub cartesian: Option<CartesianDomain>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSummary {
    pub inventory_entities: u64,
    pub inventory_artifacts: u64,
    pub explicit_rows: u64,
    pub implicit_cartesian_pairs: u64,
    pub states: BTreeMap<PlanState, u64>,
    pub executed_tests: u64,
    pub passing_firmware_targets: u64,
    pub complete: bool,
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn row_for(record: &Value, record_kind: &str) -> Result<PlanRow> {
    let id = record["id"].as_str().unwrap_or_default();
    let kind = record["kind"].as_str().unwrap_or("unknown");
    let is_artifact = record_kind == "artifact";
    let (state, reason) = if is_artifact && kind == "firmware" {
        artifact_state(record)
    } else if !is_artifact
        && [
            "device",
            "firmware",
            "hashboard",
            "control-board",
            "chip",
            "psu",
            "platform",
        ]
        .contains(&kind)
    {
        (
            PlanState::Unknown,
            Reason::new(
                "exact-target-unresolved",
                "Catalogue evidence is not a complete execution profile.",
                "Resolve exact component revisions, bundle inputs, and implemented interfaces.",
            ),
        )
    } else {
        (
            PlanState::NotApplicable,
            Reason::new(
                "supporting-record",
                "This record is a support input, not a standalone miner execution target.",
                "Retain this record when resolving the input closure of a target.",
            ),
        )
    };
    Ok(PlanRow {
        id: format!("{record_kind}:{id}"),
        record_kind: record_kind.into(),
        record_id: id.into(),
        record_hash: content_hash(record)?,
        device_id: (kind == "device").then(|| id.into()),
        firmware_id: (!is_artifact && kind == "firmware").then(|| id.into()),
        artifact_id: is_artifact.then(|| id.into()),
        state,
        reasons: vec![reason],
        evidence_ids: strings(&record["source_ids"]),
        constraints: artifact_constraints(record),
        required_fields: if state == PlanState::Unknown {
            exact_fields()
        } else {
            vec![]
        },
    })
}

fn exact_fields() -> Vec<String> {
    [
        "controller.pcb_revision",
        "controller.soc",
        "controller.cpu",
        "memory.layout",
        "storage.geometry",
        "hashboards.revisions",
        "hashboards.chip_counts",
        "auxiliary_controllers",
        "psu.revision",
        "cooling.sensor_layout",
        "firmware.bundle",
        "installation.path",
        "backend.build",
        "models.implementations",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

pub fn artifact_constraints(record: &Value) -> Vec<Value> {
    let mut constraints = Vec::new();
    for field in [
        "model_labels",
        "vendor_models",
        "publisher_metadata",
        "software_version",
        "package_version",
        "content_role",
        "content_validity",
        "integrity_review_required",
        "dependency_status",
        "payload_available",
        "notes",
    ] {
        if let Some(value) = record.get(field) {
            constraints.push(serde_json::json!({"field": field, "value": value}));
        }
    }
    constraints
}

fn artifact_state(record: &Value) -> (PlanState, Reason) {
    if record["status"] != "downloaded" {
        return (
            PlanState::Blocked,
            Reason::new(
                "payload-not-downloaded",
                "The frozen artifact record does not contain a downloaded payload.",
                "Resolve the recorded acquisition error and import a new snapshot.",
            ),
        );
    }
    if record["content_role"] == "git-lfs-pointer" {
        return (PlanState::Blocked, Reason::new("lfs-reference-not-payload", "This record stores a Git LFS pointer. It is not executable firmware.", "Select and verify the recorded resolved payload without dropping the pointer alias."));
    }
    if record["content_validity"].is_string() || record["integrity_review_required"] == true {
        return (
            PlanState::Blocked,
            Reason::new(
                "artifact-integrity-unresolved",
                "The artifact has a recorded content defect or pending integrity review.",
                "Resolve the exact integrity finding before preparation.",
            ),
        );
    }
    if [
        "documentation-image",
        "html-download-redirect",
        "javascript-shell",
        "installer-link-page",
    ]
    .contains(&record["content_role"].as_str().unwrap_or(""))
    {
        return (
            PlanState::Blocked,
            Reason::new(
                "firmware-record-has-no-payload",
                "The saved file has a non-firmware content role.",
                "Acquire the actual firmware payload and retain this source record.",
            ),
        );
    }
    (PlanState::Unknown, Reason::new("artifact-target-unresolved", "A saved filename and hash do not establish a complete executable target.", "Verify the bytes, artifact constraints, boot dependencies, and exact hardware profile."))
}

impl Catalog {
    pub fn plan(&self, pairing: Pairing) -> Result<MatrixPlan> {
        let mut rows = Vec::new();
        let mut entity_rows = BTreeMap::new();
        let mut artifact_hashes = BTreeMap::new();
        for record in self.entities.values() {
            let row = row_for(record, "entity")?;
            entity_rows.insert(row.record_id.clone(), row.clone());
            rows.push(row);
        }
        for record in self.artifacts.values() {
            let row = row_for(record, "artifact")?;
            artifact_hashes.insert(row.record_id.clone(), row.record_hash.clone());
            rows.push(row);
        }
        let devices: BTreeSet<_> = self
            .entities
            .iter()
            .filter(|(_, v)| v["kind"] == "device")
            .map(|(id, _)| id.clone())
            .collect();
        let firmware_artifacts: BTreeSet<_> = self
            .artifacts
            .iter()
            .filter(|(_, v)| v["kind"] == "firmware")
            .map(|(id, _)| id.clone())
            .collect();
        // Relationship IDs are retained. Two aliases or constraint-bearing edges
        // cannot erase each other, even if their payload bytes are identical.
        let mut explicit_pairs = BTreeSet::new();
        for relation in self.relationships.values() {
            let from = relation["from_id"].as_str().unwrap_or_default();
            let to = relation["to_id"].as_str().unwrap_or_default();
            let Some(firmware) = self.entities.get(from) else {
                continue;
            };
            if firmware["kind"] != "firmware" || !devices.contains(to) {
                continue;
            }
            if !["supports", "builds-for", "incompatible-with"]
                .contains(&relation["relation"].as_str().unwrap_or(""))
            {
                continue;
            }
            let relationship_id = relation["id"].as_str().unwrap_or_default();
            let relationship_hash = content_hash(relation)?;
            let artifacts = strings(&firmware["artifact_ids"]);
            // A firmware family with no payload still requires an explicit row.
            let candidates: Vec<Option<String>> = if artifacts.is_empty() {
                vec![None]
            } else {
                artifacts.into_iter().map(Some).collect()
            };
            for artifact_id in candidates {
                let artifact = artifact_id.as_ref().and_then(|id| self.artifacts.get(id));
                let mut row = entity_rows[from].clone();
                row.record_kind = "pair".into();
                row.id = format!(
                    "pair:{relationship_id}:{}",
                    artifact_id.as_deref().unwrap_or("no-payload")
                );
                row.device_id = Some(to.into());
                row.firmware_id = Some(from.into());
                row.artifact_id = artifact_id.clone();
                row.evidence_ids.extend(strings(&relation["source_ids"]));
                row.evidence_ids.push(relationship_id.into());
                row.constraints.push(relation.clone());
                row.reasons = vec![Reason::new("family-edge-is-candidate-only", "The relationship supplies a candidate. It does not establish support for each attached artifact.", "Apply the complete artifact constraints to an exact hardware profile.")];
                row.state = PlanState::Unknown;
                if relation["relation"] == "incompatible-with" {
                    // Preserve the conflict as evidence. Its free-text revision
                    // conditions are not safe to apply to every artifact.
                    row.reasons.push(Reason::new("conditional-conflict-needs-resolution", "An incompatibility claim exists. Its revision constraints must be resolved before exclusion.", "Match the exact target against this incompatibility claim."));
                }
                match artifact {
                    Some(artifact) => {
                        row.constraints.extend(artifact_constraints(artifact));
                        let (state, reason) = artifact_state(artifact);
                        if state == PlanState::Blocked {
                            row.state = state;
                            row.reasons.push(reason);
                        }
                        if artifact["kind"] != "firmware" {
                            row.reasons.push(Reason::new("support-input-not-boot-image", "This attached artifact has a supporting role. It is not a complete miner image.", "Resolve its role in a complete firmware bundle."));
                        }
                        row.record_hash = content_hash(&(
                            &entity_rows[from].record_hash,
                            &relationship_hash,
                            &artifact_hashes[artifact_id.as_ref().expect("artifact has an ID")],
                        ))?;
                    }
                    None => {
                        row.state = PlanState::Blocked;
                        row.reasons.push(Reason::new(
                            "firmware-payload-unassigned",
                            "The firmware candidate lacks a resolved artifact record.",
                            "Resolve and acquire its exact input payloads.",
                        ));
                    }
                }
                if let Some(id) = &artifact_id {
                    if firmware_artifacts.contains(id) {
                        explicit_pairs.insert((to.to_string(), id.clone()));
                    }
                }
                rows.push(row);
            }
        }
        rows.sort_by(|a, b| a.id.cmp(&b.id));
        let cartesian = (pairing == Pairing::Cartesian).then(|| CartesianDomain {
            total_pairs: devices.len() as u64 * firmware_artifacts.len() as u64,
            explicit_pairs: explicit_pairs.len() as u64,
            device_ids: devices.into_iter().collect(),
            firmware_artifact_ids: firmware_artifacts.into_iter().collect(),
            default_state: PlanState::Unknown,
            default_reason: Reason::new(
                "no-exact-pair-evidence",
                "No resolved exact target establishes this hardware and artifact pair.",
                "Resolve its artifact-level hardware constraints before execution.",
            ),
        });
        let mut plan = MatrixPlan {
            schema_version: "2.0".into(), plan_hash: String::new(), snapshot_hash: self.snapshot.manifest.snapshot_hash.clone(), pairing,
            selection_rule: "All entity and artifact records. Candidate rows use directed firmware-to-device supports, builds-for, and incompatible-with edges. All edge and artifact constraints remain unresolved until exact target resolution.".into(),
            inventory_entities: self.entities.len() as u64, inventory_artifacts: self.artifacts.len() as u64,
            rows, cartesian,
        };
        plan.plan_hash = plan.compute_hash()?;
        Ok(plan)
    }
}

impl MatrixPlan {
    fn compute_hash(&self) -> Result<String> {
        let mut value = serde_json::to_value(self)?;
        value
            .as_object_mut()
            .expect("serialized structure")
            .remove("plan_hash");
        content_hash(&value)
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == "2.0",
            "unsupported matrix plan version"
        );
        ensure!(
            self.plan_hash == self.compute_hash()?,
            "matrix plan hash mismatch"
        );
        let mut ids = BTreeSet::new();
        for row in &self.rows {
            ensure!(ids.insert(&row.id), "duplicate matrix row {}", row.id);
            ensure!(
                !row.reasons.is_empty(),
                "matrix row lacks reasons: {}",
                row.id
            );
        }
        ensure!(
            self.rows
                .iter()
                .filter(|r| r.record_kind == "entity")
                .count() as u64
                == self.inventory_entities,
            "entity accounting mismatch"
        );
        ensure!(
            self.rows
                .iter()
                .filter(|r| r.record_kind == "artifact")
                .count() as u64
                == self.inventory_artifacts,
            "artifact accounting mismatch"
        );
        if let Some(domain) = &self.cartesian {
            ensure!(
                domain.total_pairs
                    == domain.device_ids.len() as u64 * domain.firmware_artifact_ids.len() as u64,
                "Cartesian denominator mismatch"
            );
            ensure!(
                domain.explicit_pairs <= domain.total_pairs,
                "Cartesian explicit pair count exceeds total"
            );
        }
        Ok(())
    }

    pub fn summary(&self) -> PlanSummary {
        let mut states = BTreeMap::new();
        for row in &self.rows {
            *states.entry(row.state).or_default() += 1;
        }
        let implicit = self
            .cartesian
            .as_ref()
            .map_or(0, |c| c.total_pairs - c.explicit_pairs);
        if let Some(domain) = &self.cartesian {
            *states.entry(domain.default_state).or_default() += implicit;
        }
        PlanSummary {
            inventory_entities: self.inventory_entities,
            inventory_artifacts: self.inventory_artifacts,
            explicit_rows: self.rows.len() as u64,
            implicit_cartesian_pairs: implicit,
            states,
            // A classification never proves execution or a passing capability.
            executed_tests: 0,
            passing_firmware_targets: 0,
            complete: implicit == 0
                && self
                    .rows
                    .iter()
                    .all(|r| matches!(r.state, PlanState::Incompatible | PlanState::NotApplicable)),
        }
    }

    /// This checks unresolved plan coverage only. It cannot turn runnable rows
    /// into passing results. The runner must supply separate gate evidence.
    pub fn require_complete(&self) -> Result<()> {
        self.validate()?;
        ensure!(
            self.summary().complete,
            "coverage is incomplete; unresolved or unexecuted required cases remain"
        );
        Ok(())
    }
}

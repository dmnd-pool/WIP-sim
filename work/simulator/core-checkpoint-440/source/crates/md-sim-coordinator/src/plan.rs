use anyhow::{ensure, Result};
use md_sim_runtime::{isolation::ProcessSpec, store::ObjectId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub fn identity(value: &impl Serialize) -> Result<String> {
    Ok(object_identity(value)?.sha256)
}

fn object_identity(value: &impl Serialize) -> Result<ObjectId> {
    // Value serializes object keys in sorted order, independent of struct order.
    let bytes = serde_json::to_vec(&serde_json::to_value(value)?)?;
    Ok(ObjectId {
        sha256: hex::encode(Sha256::digest(&bytes)),
        size: bytes.len() as u64,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SuiteIdentity {
    pub id: String,
    pub version: String,
    pub suite: ObjectId,
    pub verifier: ObjectId,
    pub required_gates: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ExecutionScope {
    SourceInterfaceFixture {
        fixture: ObjectId,
        firmware_identity: String,
    },
    ProcessPrimitive {
        description: String,
    },
    UnresolvedTarget {
        catalogue_record: ObjectId,
        reason: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedRun {
    pub name: String,
    pub scope: ExecutionScope,
    pub process: Option<ProcessSpec>,
    pub suite: SuiteIdentity,
    /// Includes runtime files, source/preparation evidence, scenario and policies.
    pub inputs: BTreeMap<String, ObjectId>,
    pub backend_tools: BTreeMap<String, ObjectId>,
    pub policies: Value,
    pub capability_limits: Vec<String>,
}
impl LockedRun {
    pub fn hash(&self) -> Result<String> {
        Ok(self.object()?.sha256)
    }
    pub fn object(&self) -> Result<ObjectId> {
        self.validate()?;
        object_identity(&("md-sim-locked-run/2", self))
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.name.is_empty() && self.name.len() <= 128,
            "invalid run name"
        );
        ensure!(
            self.capability_limits.len() <= 64
                && self.capability_limits.iter().all(|s| s.len() <= 4096),
            "capability limits exceed metadata bound"
        );
        ensure!(
            !self.inputs.is_empty() && self.inputs.len() <= 65536,
            "invalid locked input count"
        );
        ensure!(
            !self.suite.required_gates.is_empty() && self.suite.required_gates.len() <= 64,
            "invalid required gates"
        );
        ensure!(
            !self.suite.id.is_empty() && !self.suite.version.is_empty(),
            "missing suite identity"
        );
        for object in self
            .inputs
            .values()
            .chain(self.backend_tools.values())
            .chain([&self.suite.suite, &self.suite.verifier])
        {
            object.validate()?;
        }
        let mut sizes = BTreeMap::new();
        for (name, input) in &self.inputs {
            ensure!(!name.is_empty() && name.len() <= 4096, "invalid input name");
            if let Some(old) = sizes.insert(&input.sha256, input.size) {
                ensure!(old == input.size, "one hash has conflicting input sizes");
            }
        }
        let required = |id: &ObjectId| -> Result<()> {
            ensure!(
                self.inputs.values().any(|input| input == id),
                "identity is absent from locked inputs"
            );
            Ok(())
        };
        required(&self.suite.suite)?;
        required(&self.suite.verifier)?;
        match &self.scope {
            ExecutionScope::SourceInterfaceFixture {
                fixture,
                firmware_identity,
            } => {
                required(fixture)?;
                ensure!(
                    firmware_identity == "rebuilt-source",
                    "source fixture must identify rebuilt firmware"
                );
                ensure!(
                    !self.capability_limits.is_empty(),
                    "fixture limits must be explicit"
                );
                ensure!(
                    self.process.is_some(),
                    "source fixture lacks its executable specification"
                );
            }
            ExecutionScope::ProcessPrimitive { description } => ensure!(
                !description.is_empty() && self.process.is_some(),
                "process primitive lacks contract"
            ),
            ExecutionScope::UnresolvedTarget {
                catalogue_record,
                reason,
            } => {
                required(catalogue_record)?;
                ensure!(
                    !reason.is_empty() && self.process.is_none(),
                    "unresolved target cannot execute"
                );
            }
        }
        if let Some(process) = &self.process {
            process.validate()?;
            for input in &process.inputs {
                required(&input.object)?;
            }
            if let Some(stdin) = &process.stdin_object {
                required(stdin)?;
            }
            ensure!(
                self.backend_tools.contains_key("bubblewrap")
                    && self.backend_tools.contains_key("unshare"),
                "isolation backend identities are missing"
            );
        }
        ensure!(self.policies.is_object(), "run policies must be explicit");
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueLimits {
    pub max_attempts: u32,
    pub max_evidence_bytes: u64,
}
impl Default for QueueLimits {
    fn default() -> Self {
        Self {
            max_attempts: 4096,
            max_evidence_bytes: 4 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanContent {
    pub schema_version: String,
    pub catalogue_snapshot: Option<ObjectId>,
    pub limits: QueueLimits,
    pub runs: Vec<LockedRun>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedPlan {
    pub content: PlanContent,
    pub sha256: String,
}
impl LockedPlan {
    pub fn new(content: PlanContent) -> Result<Self> {
        let sha256 = identity(&("md-sim-execution-plan/2", &content))?;
        let plan = Self { content, sha256 };
        plan.validate()?;
        Ok(plan)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.content.schema_version == "2.0",
            "unsupported execution plan version"
        );
        ensure!(
            !self.content.runs.is_empty() && self.content.runs.len() <= 4096,
            "invalid execution plan size"
        );
        ensure!(
            (1..=65536).contains(&self.content.limits.max_attempts)
                && self.content.limits.max_evidence_bytes > 0,
            "invalid queue limits"
        );
        let mut names = BTreeSet::new();
        let mut hashes = BTreeSet::new();
        for run in &self.content.runs {
            ensure!(names.insert(&run.name), "duplicate run name");
            ensure!(hashes.insert(run.hash()?), "duplicate resolved run");
        }
        ensure!(
            self.sha256 == identity(&("md-sim-execution-plan/2", &self.content))?,
            "execution plan identity changed"
        );
        if let Some(snapshot) = &self.content.catalogue_snapshot {
            snapshot.validate()?;
            ensure!(
                self.content
                    .runs
                    .iter()
                    .all(|r| r.inputs.values().any(|id| id == snapshot)),
                "snapshot absent from run inputs"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum GateStatus {
    Pass,
    Fail,
    Blocked,
    Inconclusive,
    NotRun,
    NotApplicable,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateResult {
    pub id: String,
    pub status: GateStatus,
    pub evidence: BTreeSet<String>,
    pub actual: Value,
    pub expected: Value,
    pub tolerance: Value,
    pub clock_domain: String,
    pub test_version: String,
}
pub trait SuiteVerifier {
    fn suite(&self) -> SuiteIdentity;
    /// Export bounded evidence and verify each declared gate independently.
    fn verify(
        &self,
        store: &md_sim_runtime::store::Store,
        run: &LockedRun,
        process: &md_sim_runtime::isolation::ProcessResult,
        out: &mut crate::queue::EvidenceWriter,
    ) -> Result<Vec<GateResult>>;
}

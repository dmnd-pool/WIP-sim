//! The pool uses a separate hash implementation and header construction path.
use crate::{Result, Target};
use bitcoin_hashes::{sha256d, Hash};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PoolJob {
    pub id: String,
    pub previous: [u8; 32],
    pub coinbase_prefix: Vec<u8>,
    pub extra_nonce1: Vec<u8>,
    pub extra_nonce2_size: usize,
    pub coinbase_suffix: Vec<u8>,
    /// Each branch is the raw SHA-256d digest, not a displayed hash string.
    pub merkle_branch: Vec<[u8; 32]>,
    pub version: u32,
    pub version_mask: u32,
    pub bits: u32,
    pub time_min: u32,
    pub time_max: u32,
    pub nonce_min: u32,
    pub nonce_max: u32,
    /// Difficulty is bound to this job. Later difficulty messages affect new jobs.
    pub share_target: Target,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Share {
    pub job_id: String,
    pub extra_nonce2: Vec<u8>,
    pub nonce: u32,
    pub time: u32,
    pub version: u32,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    Accepted {
        header_hex: String,
        hash_display: String,
        block_candidate: bool,
    },
    Duplicate,
    Stale,
    UnknownJob,
    InvalidExtraNonce,
    OutOfRange,
    InvalidVersion,
    LowDifficulty,
}

impl PoolJob {
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() || self.id.len() > 256 {
            return Err("invalid pool job ID".into());
        }
        if self.time_min > self.time_max || self.nonce_min > self.nonce_max {
            return Err("reversed pool job bounds".into());
        }
        if self.extra_nonce2_size > 32
            || self.merkle_branch.len() > 32
            || self.coinbase_prefix.len() + self.extra_nonce1.len() + self.coinbase_suffix.len()
                > 1_000_000
        {
            return Err("pool job exceeds input resource limits".into());
        }
        Target::from_compact(self.bits)?;
        Ok(())
    }

    /// Independently reconstruct the coinbase, Merkle root, and serialized header.
    /// Callers must validate the submission bounds before invoking this method.
    pub fn reconstruct(&self, share: &Share) -> Result<[u8; 80]> {
        let coinbase = self.coinbase(share)?;
        let mut root = sha256d::Hash::hash(&coinbase).to_byte_array();
        for branch in &self.merkle_branch {
            let mut pair = [0; 64];
            pair[..32].copy_from_slice(&root);
            pair[32..].copy_from_slice(branch);
            root = sha256d::Hash::hash(&pair).to_byte_array();
        }
        let mut bytes = Vec::with_capacity(80);
        bytes.extend_from_slice(&share.version.to_le_bytes());
        bytes.extend_from_slice(&self.previous);
        bytes.extend_from_slice(&root);
        bytes.extend_from_slice(&share.time.to_le_bytes());
        bytes.extend_from_slice(&self.bits.to_le_bytes());
        bytes.extend_from_slice(&share.nonce.to_le_bytes());
        Ok(bytes.try_into().expect("fixed header fields"))
    }

    /// The exact serialized coinbase supplied in the Stratum job and submission.
    pub fn coinbase(&self, share: &Share) -> Result<Vec<u8>> {
        if share.extra_nonce2.len() != self.extra_nonce2_size {
            return Err("wrong extra nonce length".into());
        }
        let mut coinbase = self.coinbase_prefix.clone();
        coinbase.extend_from_slice(&self.extra_nonce1);
        coinbase.extend_from_slice(&share.extra_nonce2);
        coinbase.extend_from_slice(&self.coinbase_suffix);
        Ok(coinbase)
    }
}

pub struct PoolVerifier {
    jobs: HashMap<String, PoolJob>,
    stale: HashSet<String>,
    seen_headers: HashSet<[u8; 80]>,
    max_jobs: usize,
    max_shares: usize,
}
impl Default for PoolVerifier {
    fn default() -> Self {
        Self::new(256, 100_000).unwrap()
    }
}
impl PoolVerifier {
    pub fn new(max_jobs: usize, max_shares: usize) -> Result<Self> {
        if max_jobs == 0 || max_shares == 0 {
            return Err("pool limits must be positive".into());
        }
        Ok(Self {
            jobs: HashMap::new(),
            stale: HashSet::new(),
            seen_headers: HashSet::new(),
            max_jobs,
            max_shares,
        })
    }
    pub fn install(&mut self, job: PoolJob, clean: bool) -> Result<()> {
        job.validate()?;
        if self.jobs.contains_key(&job.id) || self.stale.contains(&job.id) {
            return Err("pool job IDs must never be reused within a session".into());
        }
        if self.jobs.len() + self.stale.len() >= self.max_jobs {
            return Err("pool job history limit reached".into());
        }
        if clean {
            self.stale.extend(self.jobs.drain().map(|(id, _)| id));
        }
        self.jobs.insert(job.id.clone(), job);
        Ok(())
    }
    pub fn job(&self, id: &str) -> Option<&PoolJob> {
        self.jobs.get(id)
    }
    pub fn verify(&mut self, share: &Share) -> Result<Verdict> {
        if self.stale.contains(&share.job_id) {
            return Ok(Verdict::Stale);
        }
        let Some(job) = self.jobs.get(&share.job_id) else {
            return Ok(Verdict::UnknownJob);
        };
        if share.extra_nonce2.len() != job.extra_nonce2_size {
            return Ok(Verdict::InvalidExtraNonce);
        }
        if share.nonce < job.nonce_min
            || share.nonce > job.nonce_max
            || share.time < job.time_min
            || share.time > job.time_max
        {
            return Ok(Verdict::OutOfRange);
        }
        if (share.version ^ job.version) & !job.version_mask != 0 {
            return Ok(Verdict::InvalidVersion);
        }
        let header = job.reconstruct(share)?;
        if self.seen_headers.contains(&header) {
            return Ok(Verdict::Duplicate);
        }
        if self.seen_headers.len() >= self.max_shares {
            return Err("pool share history limit reached".into());
        }
        // Track invalid submissions as well as accepted shares.
        self.seen_headers.insert(header);
        let digest = sha256d::Hash::hash(&header).to_byte_array();
        // Compare independently. Do not invoke the ASIC engine's target test.
        let numeric: Vec<u8> = digest.iter().rev().copied().collect();
        if numeric.as_slice() > job.share_target.0.as_slice() {
            return Ok(Verdict::LowDifficulty);
        }
        let block_target = Target::from_compact(job.bits)?;
        Ok(Verdict::Accepted {
            header_hex: hex::encode(header),
            hash_display: hex::encode(&numeric),
            block_candidate: numeric.as_slice() <= block_target.0.as_slice(),
        })
    }
}

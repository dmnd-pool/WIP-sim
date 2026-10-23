//! Exact block export for an explicit regtest genesis parent.
//! Export proves serialization and proof of work only. A Bitcoin Core import
//! must independently establish consensus acceptance and the chain-state change.
use crate::{stratum::PoolReport, PoolVerifier, Result, Share, Verdict};
use bitcoin_hashes::{sha256d, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Constants from archived Bitcoin Core 619334569b44b4278d0155bb8daf4c8535a47367,
/// src/kernel/chainparams.cpp. Internal hashes are serialized digest bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegtestAnchor {
    pub network: String,
    pub previous_hash: [u8; 32],
    pub candidate_height: u32,
    pub bits: u32,
    pub genesis_time: u32,
    pub message_start: [u8; 4],
}
impl RegtestAnchor {
    pub fn genesis() -> Self {
        let mut previous_hash: [u8; 32] =
            hex::decode("0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206")
                .expect("fixed hash")
                .try_into()
                .expect("fixed hash size");
        previous_hash.reverse();
        Self {
            network: "bitcoin-regtest".into(),
            previous_hash,
            candidate_height: 1,
            bits: 0x207fffff,
            genesis_time: 1_296_688_602,
            message_start: [0xfa, 0xbf, 0xb5, 0xda],
        }
    }
    pub fn validate(&self) -> Result<()> {
        if *self != Self::genesis() {
            return Err("only the exact Bitcoin regtest genesis anchor is supported".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockEvidence {
    pub source_submission_index: usize,
    pub share: Share,
    pub anchor: RegtestAnchor,
    pub header_hex: String,
    pub coinbase_hex: String,
    pub block_hex: String,
    /// Core -loadblock bytes: network magic, little-endian length, full block.
    pub import_hex: String,
    pub block_hash_internal_hex: String,
    pub block_hash_display: String,
    pub claim: String,
}

/// Reconstruct only accepted submissions. Do not search, adjust the nonce,
/// or use a node-generated block. Each exported block is a height-one candidate;
/// multiple jobs represent competing blocks with the same genesis parent.
pub fn regtest_blocks(report: &PoolReport) -> Result<Vec<BlockEvidence>> {
    let anchor = report
        .regtest_anchor
        .as_ref()
        .ok_or("pool report has no explicit regtest anchor")?;
    anchor.validate()?;
    if report.jobs.len() > 64 || report.submissions.len() > 100_000 {
        return Err("pool report exceeds block-export limits".into());
    }
    let mut blocks = Vec::new();
    let mut seen = HashSet::new();
    for (index, submission) in report.submissions.iter().enumerate() {
        let Verdict::Accepted {
            header_hex,
            hash_display,
            block_candidate: true,
        } = &submission.verdict
        else {
            continue;
        };
        let mut jobs = report
            .jobs
            .iter()
            .filter(|job| job.id == submission.share.job_id);
        let job = jobs.next().ok_or("submission job is absent")?;
        if jobs.next().is_some()
            || job.previous != anchor.previous_hash
            || job.bits != anchor.bits
            || job.time_min <= anchor.genesis_time
            || !job.merkle_branch.is_empty()
        {
            return Err("job does not match the supported coinbase-only regtest anchor".into());
        }
        // Re-run independent validation. A saved Accepted label is not an oracle.
        let mut verifier = PoolVerifier::new(1, 1)?;
        verifier.install(job.clone(), true)?;
        if verifier.verify(&submission.share)? != submission.verdict {
            return Err("saved submission verdict differs from independent revalidation".into());
        }
        let header = job.reconstruct(&submission.share)?;
        if !seen.insert(header) {
            return Err("duplicate accepted header in block-export evidence".into());
        }
        let coinbase = job.coinbase(&submission.share)?;
        let mut block = header.to_vec();
        block.push(1); // CompactSize: exactly one transaction.
        block.extend_from_slice(&coinbase);
        let mut import = anchor.message_start.to_vec();
        import.extend_from_slice(&(block.len() as u32).to_le_bytes());
        import.extend_from_slice(&block);
        let digest = sha256d::Hash::hash(&header).to_byte_array();
        blocks.push(BlockEvidence {
            source_submission_index: index,
            share: submission.share.clone(),
            anchor: anchor.clone(),
            header_hex: header_hex.clone(),
            coinbase_hex: hex::encode(coinbase),
            block_hex: hex::encode(block),
            import_hex: hex::encode(import),
            block_hash_internal_hex: hex::encode(digest),
            block_hash_display: hash_display.clone(),
            claim: "candidate reconstructed from accepted submission; Core acceptance requires separate evidence".into(),
        });
    }
    if blocks.is_empty() {
        return Err("no accepted regtest block candidate is present".into());
    }
    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn archived_genesis_constants_match_independent_header_hash() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/bitcoin/genesis-header.json"
        ))
        .unwrap();
        let mut header = hex::decode(fixture["header_hex"].as_str().unwrap()).unwrap();
        header[68..72].copy_from_slice(&1_296_688_602u32.to_le_bytes());
        header[72..76].copy_from_slice(&0x207fffffu32.to_le_bytes());
        header[76..80].copy_from_slice(&2u32.to_le_bytes());
        let hash: [u8; 32] = Sha256::digest(Sha256::digest(header)).into();
        let mut anchor = RegtestAnchor::genesis();
        assert_eq!(hash, anchor.previous_hash);
        anchor.validate().unwrap();
        anchor.candidate_height = 2;
        assert!(anchor.validate().is_err());
    }
}

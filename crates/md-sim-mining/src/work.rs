use crate::{Filter, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Hash fields use the 32 raw bytes found in a serialized Bitcoin header.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub version: u32,
    pub previous: [u8; 32],
    pub merkle_root: [u8; 32],
    pub time: u32,
    pub bits: u32,
    pub nonce: u32,
}

impl Header {
    pub fn encode(&self) -> [u8; 80] {
        let mut bytes = [0; 80];
        bytes[..4].copy_from_slice(&self.version.to_le_bytes());
        bytes[4..36].copy_from_slice(&self.previous);
        bytes[36..68].copy_from_slice(&self.merkle_root);
        bytes[68..72].copy_from_slice(&self.time.to_le_bytes());
        bytes[72..76].copy_from_slice(&self.bits.to_le_bytes());
        bytes[76..].copy_from_slice(&self.nonce.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 80 {
            return Err("a Bitcoin header must contain 80 bytes".into());
        }
        let u32_at = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
        Ok(Self {
            version: u32_at(0),
            previous: bytes[4..36].try_into().unwrap(),
            merkle_root: bytes[36..68].try_into().unwrap(),
            time: u32_at(68),
            bits: u32_at(72),
            nonce: u32_at(76),
        })
    }

    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(Sha256::digest(self.encode())).into()
    }
}

/// A complete work range. The nonce endpoints are inclusive. Version rolling
/// starts with the supplied version and visits every assignment of mask bits.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Work {
    pub id: String,
    pub header: Header,
    pub nonce_start: u32,
    pub nonce_end: u32,
    pub version_mask: u32,
    pub filter: Filter,
}

impl Work {
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() || self.id.len() > 256 {
            return Err("invalid work ID".into());
        }
        if self.nonce_start > self.nonce_end {
            return Err("reversed nonce range".into());
        }
        Ok(())
    }

    pub fn version_at(&self, index: u64) -> Option<u32> {
        roll_version(self.header.version, self.version_mask, index)
    }
}

pub(crate) fn roll_version(version: u32, mask: u32, index: u64) -> Option<u32> {
    if index >= (1u64 << mask.count_ones()) {
        return None;
    }
    let mut assignment = 0;
    let mut dense_bit = 0;
    for bit in 0..32 {
        if mask & (1u32 << bit) != 0 {
            assignment |= (((index >> dense_bit) & 1) as u32) << bit;
            dense_bit += 1;
        }
    }
    Some(version ^ assignment)
}

/// Work received through a compression-state interface. The engine cannot
/// reconstruct the version from a midstate. It returns the selected index;
/// the firmware and independent pool verifier retain the complete header.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MidstateWork {
    pub id: String,
    pub states: Vec<[u32; 8]>,
    pub tail: [u8; 12],
    pub nonce_start: u32,
    pub nonce_end: u32,
    pub filter: Filter,
}

impl MidstateWork {
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() || self.id.len() > 256 {
            return Err("invalid work ID".into());
        }
        if self.nonce_start > self.nonce_end {
            return Err("reversed nonce range".into());
        }
        // A runtime resource limit. Device-specific limits stay in the model.
        if self.states.is_empty() || self.states.len() > 256 {
            return Err("midstate count must be between1 and256".into());
        }
        Ok(())
    }
}

/// Restore compression state for a header after its first 64 bytes. The tail
/// contains merkle-root bytes 28..32, time and bits in header serialization order.
/// This function does not hash serialized midstate bytes as a new message.
pub fn digest_from_midstate(state: [u32; 8], tail: [u8; 12], nonce: u32) -> [u8; 32] {
    let mut state = state;
    let mut last = [0u8; 64];
    last[..12].copy_from_slice(&tail);
    last[12..16].copy_from_slice(&nonce.to_le_bytes());
    last[16] = 0x80;
    last[56..64].copy_from_slice(&640u64.to_be_bytes());
    sha2::compress256(&mut state, &[last.into()]);
    let mut first_hash = [0; 32];
    for (output, word) in first_hash.chunks_exact_mut(4).zip(state) {
        output.copy_from_slice(&word.to_be_bytes());
    }
    Sha256::digest(first_hash).into()
}

pub fn header_midstate(header: &Header) -> [u32; 8] {
    let mut state = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let first: [u8; 64] = header.encode()[..64].try_into().unwrap();
    sha2::compress256(&mut state, &[first.into()]);
    state
}

/// cgminer's getwork representation stores each SHA word in reverse byte order.
/// Its midstate stores eight compression words in little-endian byte order.
pub fn cgminer_sha256d(midstate: [u8; 32], mut tail: [u8; 12], nonce: u32) -> [u8; 32] {
    let mut state = [0; 8];
    for (word, bytes) in state.iter_mut().zip(midstate.chunks_exact(4)) {
        *word = u32::from_le_bytes(bytes.try_into().unwrap());
    }
    for word in tail.chunks_exact_mut(4) {
        word.reverse();
    }
    digest_from_midstate(state, tail, nonce)
}

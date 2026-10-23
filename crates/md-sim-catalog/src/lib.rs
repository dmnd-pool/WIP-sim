//! Frozen catalogue inputs, explicit coverage accounting, and execution identities.
//!
//! Catalogue support claims are candidate evidence. They do not establish an
//! executable target. Only a complete exact target may become runnable.

mod identity;
mod plan;
mod snapshot;

pub use identity::*;
pub use plan::*;
pub use snapshot::*;

use anyhow::{Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Hash serialized data with sorted JSON object keys. Arrays retain their order.
/// A run seed, policy, budget, or input change therefore changes the identity.
pub fn content_hash(value: &impl Serialize) -> Result<String> {
    let value = serde_json::to_value(value)?;
    Ok(hash_bytes(&serde_json::to_vec(&value)?))
}

pub fn hash_file(path: &Path) -> Result<(String, u64)> {
    let mut file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut count = 0;
    let mut buffer = [0; 65536];
    loop {
        let length = file.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        hasher.update(&buffer[..length]);
        count += length as u64;
    }
    Ok((format!("{:x}", hasher.finalize()), count))
}

pub fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

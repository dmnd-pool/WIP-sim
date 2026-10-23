use crate::Result;
use serde::{Deserialize, Serialize};

/// Unsigned 256-bit integer in big-endian order, as in Bitcoin display strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Target(pub [u8; 32]);

impl Target {
    pub const MAX: Self = Self([255; 32]);
    pub const ZERO: Self = Self([0; 32]);

    /// Decode Bitcoin's compact target. Reject negative, zero and overflow.
    pub fn from_compact(bits: u32) -> Result<Self> {
        let size = bits >> 24;
        let mut word = bits & 0x007f_ffff;
        if word == 0 || bits & 0x0080_0000 != 0 {
            return Err("compact target is zero or negative".into());
        }
        if size > 34 || (word > 0xff && size > 33) || (word > 0xffff && size > 32) {
            return Err("compact target exceeds 256 bits".into());
        }
        let mut bytes = [0; 32];
        if size <= 3 {
            word >>= 8 * (3 - size);
            bytes[28..].copy_from_slice(&word.to_be_bytes());
        } else {
            let shift = size as usize - 3;
            for i in 0..3 {
                let byte = ((word >> (8 * i)) & 0xff) as u8;
                if shift + i < 32 {
                    bytes[31 - shift - i] = byte;
                } else if byte != 0 {
                    return Err("compact target exceeds 256 bits".into());
                }
            }
        }
        if bytes == [0; 32] {
            return Err("compact target rounds to zero".into());
        }
        Ok(Self(bytes))
    }

    /// Hash input is the raw SHA-256d digest. Bitcoin interprets it little-endian.
    pub fn accepts(&self, digest: &[u8; 32]) -> bool {
        digest.iter().rev().cmp(self.0.iter()).is_le()
    }

    pub fn probability(self) -> f64 {
        let mut p = 0.0;
        for b in self.0.iter().rev() {
            p = (p + f64::from(*b)) / 256.0;
        }
        p + 2f64.powi(-256)
    }
}

/// The programmed ASIC filter. Masks use the numerical big-endian hash order.
/// A mask selects bits that must be zero. No pool target overrides this filter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Filter {
    Target(Target),
    ZeroMask([u8; 32]),
}

impl Filter {
    pub fn accepts(&self, digest: &[u8; 32]) -> bool {
        match self {
            Self::Target(target) => target.accepts(digest),
            Self::ZeroMask(mask) => mask
                .iter()
                .zip(digest.iter().rev())
                .all(|(m, h)| m & h == 0),
        }
    }

    /// Exact bit recurrence for P(hash <= pool target AND ASIC filter).
    pub fn intersection_probability(&self, pool: Target) -> f64 {
        match self {
            Self::Target(target) => (*target).min(pool).probability(),
            Self::ZeroMask(mask) => {
                let (mut equal, mut less) = (1.0, 0.0);
                for (target, mask) in pool.0.iter().zip(mask) {
                    for bit in (0..8).rev() {
                        let limit = (target >> bit) & 1;
                        let forced_zero = (mask >> bit) & 1 != 0;
                        let choices = if forced_zero { 1.0 } else { 2.0 };
                        less = less * choices / 2.0 + if limit == 1 { equal / 2.0 } else { 0.0 };
                        equal = if limit == 1 && forced_zero {
                            0.0
                        } else {
                            equal / 2.0
                        };
                    }
                }
                equal + less
            }
        }
    }

    pub fn expected_interval_seconds(&self, pool: Target, hps: f64) -> Result<f64> {
        if !hps.is_finite() || hps <= 0.0 {
            return Err("hash rate must be finite and positive".into());
        }
        Ok(1.0 / (hps * self.intersection_probability(pool)))
    }
}

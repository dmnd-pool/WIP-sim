//! Compare upstream FASM semantics with the separately verified Rust predicates.
use anyhow::{ensure, Result};
use md_sim_runtime::store::ObjectId;
use serde::{de::SeqAccess, Deserialize, Deserializer};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

#[derive(Deserialize)]
pub struct Probe {
    pub schema: String,
    pub passed: bool,
    pub error: Option<String>,
    pub input_manifest: ObjectId,
    pub inspector: ObjectId,
    pub decoding: Decoding,
}

#[derive(Deserialize)]
pub struct Decoding {
    pub configuration_sha256: String,
    pub configuration_set_bits_excluding_ecc: u64,
    #[serde(deserialize_with = "project")]
    pub decoded: Projection,
    pub unresolved: Vec<Value>,
    pub unexplained: Vec<Value>,
}

pub struct Projection {
    pub names: BTreeMap<String, bool>,
    pub asserted_bits_sha256: String,
    pub asserted_bit_count: u64,
}

#[derive(Deserialize)]
struct Segment {
    matches: Vec<Match>,
}
#[derive(Deserialize)]
struct Match {
    name: String,
    all_zero_conditions: bool,
    asserted_bits: Vec<Position>,
}

#[derive(Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
struct Position {
    frame: u32,
    bit: u32,
}

fn bit_digest(bits: &BTreeSet<Position>) -> String {
    let mut hash = Sha256::new();
    hash.update(b"md-sim-fasm-frame-bits/1\0");
    hash.update((bits.len() as u32).to_le_bytes());
    for position in bits {
        hash.update(position.frame.to_le_bytes());
        hash.update(position.bit.to_le_bytes());
    }
    hex::encode(hash.finalize())
}

fn canonical(name: &str) -> Result<String> {
    ensure!(
        !name.is_empty()
            && name.len() <= 1024
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.[]".contains(&b)),
        "invalid feature name"
    );
    if let Some((base, tail)) = name.split_once('[') {
        ensure!(
            !base.contains(']') && tail.ends_with(']'),
            "invalid feature address"
        );
        let number = tail.strip_suffix(']').unwrap();
        ensure!(
            !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()),
            "invalid feature address"
        );
        let number: u32 = number.parse()?;
        return Ok(if number == 0 {
            base.into()
        } else {
            format!("{base}[{number}]")
        });
    }
    ensure!(!name.contains(']'), "unexpected feature address end");
    Ok(name.into())
}

fn project<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Projection, D::Error> {
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = Projection;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("bounded feature segments")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            let mut bits = BTreeSet::new();
            let mut count = 0usize;
            while let Some(segment) = seq.next_element::<Segment>()? {
                for feature in segment.matches {
                    count += 1;
                    if count > 1_000_000
                        || feature.asserted_bits.len() > 256
                        || feature.all_zero_conditions != feature.asserted_bits.is_empty()
                    {
                        return Err(serde::de::Error::custom(
                            "feature projection bound or zero flag",
                        ));
                    }
                    for position in feature.asserted_bits {
                        if position.bit >= 3232 || (1600..1613).contains(&position.bit) {
                            return Err(serde::de::Error::custom(
                                "invalid asserted frame bit or ECC bit",
                            ));
                        }
                        bits.insert(position);
                        if bits.len() > 1_000_000 {
                            return Err(serde::de::Error::custom("asserted frame bit count bound"));
                        }
                    }
                    let name = canonical(&feature.name).map_err(serde::de::Error::custom)?;
                    if result
                        .insert(name, feature.all_zero_conditions)
                        .is_some_and(|old| old != feature.all_zero_conditions)
                    {
                        return Err(serde::de::Error::custom("conflicting canonical feature"));
                    }
                }
            }
            Ok(Projection {
                names: result,
                asserted_bits_sha256: bit_digest(&bits),
                asserted_bit_count: bits.len() as u64,
            })
        }
    }
    deserializer.deserialize_seq(Visitor)
}

pub fn digest(names: &BTreeSet<String>) -> String {
    let mut hash = Sha256::new();
    hash.update(b"md-sim-fasm-names/1\0");
    hash.update((names.len() as u32).to_le_bytes());
    for name in names {
        hash.update((name.len() as u32).to_le_bytes());
        hash.update(name.as_bytes());
    }
    hex::encode(hash.finalize())
}

pub fn compare(
    expected: &BTreeMap<String, bool>,
    raw: &[String],
    removed: &[String],
    merged_digest: &str,
    merged_count: u64,
) -> Result<(usize, usize)> {
    ensure!(
        raw.len() <= 1_000_000 && removed.len() <= raw.len(),
        "export name count bound"
    );
    let mut names = BTreeSet::new();
    let mut zero_count = 0;
    for name in raw {
        ensure!(
            canonical(name)? == *name && names.insert(name.clone()),
            "noncanonical or duplicate export feature"
        );
        let zero = expected.get(name).ok_or_else(|| {
            anyhow::anyhow!("upstream feature absent from Rust predicates: {name}")
        })?;
        zero_count += usize::from(*zero);
    }
    for (name, zero) in expected {
        ensure!(
            *zero || names.contains(name),
            "positive Rust predicate absent from export: {name}"
        );
    }
    let mut unique_removed = BTreeSet::new();
    for name in removed {
        ensure!(
            expected.get(name) == Some(&true) && unique_removed.insert(name) && names.remove(name),
            "removed export feature is not a matched zero-only default"
        );
    }
    ensure!(
        names.len() as u64 == merged_count && digest(&names) == merged_digest,
        "formatted FASM semantic digest differs"
    );
    Ok((names.len(), zero_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_bit_digest_uses_canonical_unique_positions() {
        let bits = BTreeSet::from([
            Position {
                frame: 4,
                bit: 1613,
            },
            Position { frame: 0, bit: 7 },
            Position { frame: 0, bit: 7 },
        ]);
        assert_eq!(
            bit_digest(&bits),
            "820fd20c65a0b5edf8a9eb0b527a1886d450ec37d4a75156d70f13953f6869c8"
        );
    }
    #[test]
    fn projection_rejects_ecc_and_conflicting_canonical_defaults() {
        let decoding = |matches: Value| {
            serde_json::from_value::<Decoding>(serde_json::json!({
                "configuration_sha256":"0".repeat(64),
                "configuration_set_bits_excluding_ecc":1,
                "decoded":[{"matches":matches}],"unresolved":[],"unexplained":[]
            }))
        };
        assert!(decoding(serde_json::json!([{"name":"T.X[0]",
            "all_zero_conditions":false,"asserted_bits":[{"frame":0,"bit":1600}]}]))
        .is_err());
        assert!(decoding(serde_json::json!([
            {"name":"T.X[0]","all_zero_conditions":false,"asserted_bits":[{"frame":0,"bit":7}]},
            {"name":"T.X","all_zero_conditions":true,"asserted_bits":[]}
        ]))
        .is_err());
        let projected = decoding(serde_json::json!([
            {"name":"T.X[0]","all_zero_conditions":false,"asserted_bits":[{"frame":0,"bit":7}]},
            {"name":"T.OTHER","all_zero_conditions":false,"asserted_bits":[{"frame":0,"bit":7}]}
        ]))
        .unwrap();
        assert_eq!(projected.decoded.asserted_bit_count, 1);
        assert_eq!(projected.decoded.names.len(), 2);
    }
    #[test]
    fn zero_address_is_canonical_and_invalid_addresses_fail() {
        assert_eq!(canonical("T.X[0]").unwrap(), "T.X");
        assert_eq!(canonical("T.X[31]").unwrap(), "T.X[31]");
        for bad in ["T.X[-1]", "T.X[1][2]", "T.X]", "T.X[", "T.X\n"] {
            assert!(canonical(bad).is_err());
        }
    }
    #[test]
    fn positive_features_cannot_be_lost_or_replaced_by_defaults() {
        let expected = BTreeMap::from([("T.POS".into(), false), ("T.ZERO".into(), true)]);
        let raw = vec!["T.POS".into(), "T.ZERO".into()];
        let digest = digest(&BTreeSet::from(["T.POS".into()]));
        assert_eq!(
            compare(&expected, &raw, &["T.ZERO".into()], &digest, 1).unwrap(),
            (1, 1)
        );
        assert!(compare(&expected, &["T.ZERO".into()], &[], &digest, 1).is_err());
        assert!(compare(&expected, &raw, &["T.POS".into()], &digest, 1).is_err());
        assert!(compare(
            &expected,
            &["T.POS".into(), "T.POS".into()],
            &[],
            &digest,
            1
        )
        .is_err());
        assert!(compare(&expected, &["T.OTHER".into()], &[], &digest, 1).is_err());
    }
}

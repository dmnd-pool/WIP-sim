//! Stream one segment at a time and compare all predicate result fields.
use anyhow::{ensure, Result};
use md_sim_runtime::store::ObjectId;
use serde::{de::SeqAccess, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeReport {
    pub schema: String,
    pub passed: bool,
    pub error: Option<String>,
    pub decoding: Decoding,
    pub input_manifest: ObjectId,
    pub inspector: ObjectId,
    pub scope: String,
}

#[derive(Deserialize, Serialize)]
pub struct Decoding {
    #[serde(deserialize_with = "summaries")]
    pub decoded: Vec<SegmentDigest>,
    #[serde(flatten)]
    pub other: BTreeMap<String, Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Segment {
    tile: String,
    tile_type: String,
    bus: String,
    database_sha256: String,
    alias: Value,
    features_outside_alias_range: usize,
    matches: Vec<Matched>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Matched {
    pub name: String,
    pub database_feature: String,
    pub all_zero_conditions: bool,
    pub asserted_bits: Vec<Position>,
}

#[derive(Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub frame: u32,
    pub bit: u32,
}

#[derive(Serialize)]
pub struct SegmentDigest {
    tile: String,
    tile_type: String,
    bus: String,
    database_sha256: String,
    alias: Value,
    features_outside_alias_range: usize,
    matches_sha256: String,
    match_count: usize,
    zero_count: usize,
}

pub fn digest_matches(matches: &mut [Matched]) -> Result<String> {
    ensure!(matches.len() <= 100_000, "segment match count bound");
    matches.sort_by(|a, b| a.name.cmp(&b.name));
    ensure!(
        matches.windows(2).all(|pair| pair[0].name != pair[1].name),
        "duplicate matched feature name"
    );
    let mut hash = Sha256::new();
    hash.update(b"md-sim-prjxray-matches/1\0");
    hash.update((matches.len() as u32).to_le_bytes());
    for feature in matches {
        for text in [&feature.name, &feature.database_feature] {
            ensure!(text.len() <= 1024, "feature name bound");
            hash.update((text.len() as u32).to_le_bytes());
            hash.update(text.as_bytes());
        }
        ensure!(
            feature.asserted_bits.len() <= 256
                && feature.all_zero_conditions == feature.asserted_bits.is_empty(),
            "feature positions or zero-only flag differ"
        );
        feature.asserted_bits.sort();
        hash.update([u8::from(feature.all_zero_conditions)]);
        hash.update((feature.asserted_bits.len() as u32).to_le_bytes());
        for position in &feature.asserted_bits {
            ensure!(position.bit < 3232, "feature bit outside frame");
            hash.update(position.frame.to_le_bytes());
            hash.update(position.bit.to_le_bytes());
        }
    }
    Ok(hex::encode(hash.finalize()))
}

fn summaries<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<SegmentDigest>, D::Error> {
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = Vec<SegmentDigest>;
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("bounded feature segments")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut summaries = Vec::new();
            while let Some(mut segment) = seq.next_element::<Segment>()? {
                if summaries.len() >= 100_000 {
                    return Err(serde::de::Error::custom("segment count bound"));
                }
                let matches_sha256 =
                    digest_matches(&mut segment.matches).map_err(serde::de::Error::custom)?;
                summaries.push(SegmentDigest {
                    tile: segment.tile,
                    tile_type: segment.tile_type,
                    bus: segment.bus,
                    database_sha256: segment.database_sha256,
                    alias: segment.alias,
                    features_outside_alias_range: segment.features_outside_alias_range,
                    matches_sha256,
                    match_count: segment.matches.len(),
                    zero_count: segment
                        .matches
                        .iter()
                        .filter(|item| item.all_zero_conditions)
                        .count(),
                });
            }
            Ok(summaries)
        }
    }
    deserializer.deserialize_seq(Visitor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn features() -> Vec<Matched> {
        serde_json::from_value(json!([
            {"name":"T.S.X","database_feature":"TYPE.S.X","all_zero_conditions":false,
             "asserted_bits":[{"frame":128,"bit":64},{"frame":129,"bit":32}]},
            {"name":"T.S.Z","database_feature":"TYPE.S.Z","all_zero_conditions":true,
             "asserted_bits":[]}]))
        .unwrap()
    }

    #[test]
    fn canonical_record_digest_retains_every_semantic_field() {
        let baseline = digest_matches(&mut features()).unwrap();
        let mut reordered = features();
        reordered.reverse();
        reordered[1].asserted_bits.reverse();
        assert_eq!(baseline, digest_matches(&mut reordered).unwrap());
        for change in 0..5 {
            let mut changed = features();
            match change {
                0 => changed[0].name.push('x'),
                1 => changed[0].database_feature.push('x'),
                2 => changed[0].asserted_bits[0].frame += 1,
                3 => changed[0].asserted_bits[0].bit += 1,
                _ => {
                    changed[0].asserted_bits.pop().unwrap();
                }
            }
            assert_ne!(baseline, digest_matches(&mut changed).unwrap());
        }
        let mut wrong = features();
        wrong[0].all_zero_conditions = true;
        assert!(digest_matches(&mut wrong).is_err());
        let mut duplicate = features();
        duplicate[1].name = duplicate[0].name.clone();
        assert!(digest_matches(&mut duplicate).is_err());
        let mut out_of_range = features();
        out_of_range[0].asserted_bits[0].bit = 3232;
        assert!(digest_matches(&mut out_of_range).is_err());
    }
}

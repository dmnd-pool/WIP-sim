//! Pinned Project X-Ray feature predicates over checked configuration frames.
//! This decodes bit conditions, not connectivity or a simulated logic netlist.
use super::{
    frames::{map_configuration, Part},
    tilegrid::{Alias, RawSegment, TileGrid},
};
use anyhow::{ensure, Context, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_DATABASE_BYTES: usize = 4 << 20;
const MAX_FEATURES: usize = 100_000;
const MAX_OUTPUT_BITS: usize = 1_000_000;
const MAX_MATCHES: usize = 1_000_000;

pub struct FeatureDatabase {
    tile_type: String,
    bus: String,
    sha256: String,
    rules: Vec<Rule>,
}

struct Rule {
    name: String,
    bits: Vec<Condition>,
}

struct Condition {
    frame: u32,
    bit: usize,
    value: bool,
}

impl FeatureDatabase {
    /// Type and bus come from the pinned manifest, never from a filename.
    pub fn from_pinned_text(
        bytes: &[u8],
        expected_sha256: &str,
        tile_type: &str,
        bus: &str,
    ) -> Result<Self> {
        ensure!(
            !bytes.is_empty() && bytes.len() <= MAX_DATABASE_BYTES,
            "feature database size bound"
        );
        let sha256 = crate::prepare::hash_bytes(bytes);
        ensure!(sha256 == expected_sha256, "feature database hash mismatch");
        ensure!(
            !tile_type.is_empty()
                && tile_type.len() <= 128
                && tile_type
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_'),
            "feature tile type encoding"
        );
        ensure!(
            matches!(bus, "CLB_IO_CLK" | "BLOCK_RAM" | "CFG_CLB"),
            "feature bus unsupported"
        );
        let text = std::str::from_utf8(bytes).context("feature database encoding")?;
        let mut rules = Vec::new();
        let mut names = BTreeSet::new();
        for line in text.lines() {
            ensure!(line.len() <= 8192, "feature line bound");
            let mut words = line.split_ascii_whitespace();
            let Some(name) = words.next() else {
                continue;
            };
            ensure!(
                name.len() <= 256
                    && name.bytes().all(|b| b.is_ascii_graphic())
                    && name.starts_with(&format!("{tile_type}."))
                    && name.len() > tile_type.len() + 1,
                "feature name or tile type differs"
            );
            ensure!(
                rules.len() < MAX_FEATURES && names.insert(name.to_owned()),
                "duplicate feature or rule limit"
            );
            let mut bits = Vec::new();
            let mut positions = BTreeSet::new();
            for token in words {
                ensure!(bits.len() < 256, "feature condition count bound");
                let (value, position) = token
                    .strip_prefix('!')
                    .map_or((true, token), |s| (false, s));
                let (frame, bit) = position
                    .split_once('_')
                    .context("unsupported feature bit token")?;
                ensure!(
                    !frame.is_empty()
                        && frame.len() <= 3
                        && frame.bytes().all(|b| b.is_ascii_digit())
                        && !bit.is_empty()
                        && bit.len() <= 4
                        && bit.bytes().all(|b| b.is_ascii_digit()),
                    "feature bit encoding"
                );
                let frame: u32 = frame.parse()?;
                let bit: usize = bit.parse()?;
                ensure!(frame < 128 && bit < 101 * 32, "feature bit range");
                ensure!(
                    positions.insert((frame, bit)),
                    "duplicate or contradictory feature condition"
                );
                bits.push(Condition { frame, bit, value });
            }
            ensure!(!bits.is_empty(), "feature has no conditions");
            rules.push(Rule {
                name: name.into(),
                bits,
            });
        }
        ensure!(!rules.is_empty(), "feature database has no rules");
        Ok(Self {
            tile_type: tile_type.into(),
            bus: bus.into(),
            sha256,
            rules,
        })
    }
}

#[derive(Serialize)]
pub struct Decoding {
    pub configuration_sha256: String,
    pub part_sha256: String,
    pub tilegrid_sha256: String,
    pub tables: Vec<TableIdentity>,
    pub decoded: Vec<DecodedSegment>,
    pub unresolved: Vec<UnresolvedSegment>,
    pub matched_features: usize,
    pub all_zero_condition_matches: usize,
    pub configuration_set_bits_excluding_ecc: usize,
    pub set_bits_explained_by_matching_features: usize,
    pub set_bits_without_matching_features: usize,
    pub unexplained: Vec<FrameBits>,
    pub limit: &'static str,
}

#[derive(Serialize)]
pub struct TableIdentity {
    pub tile_type: String,
    pub bus: String,
    pub sha256: String,
    pub rules: usize,
}

#[derive(Serialize)]
pub struct DecodedSegment {
    pub tile: String,
    pub tile_type: String,
    pub bus: String,
    pub database_sha256: String,
    pub alias: Option<Alias>,
    pub features_outside_alias_range: usize,
    pub matches: Vec<MatchedFeature>,
}

#[derive(Serialize)]
pub struct MatchedFeature {
    /// Tile-local name, with any alias site name translated back.
    pub name: String,
    pub database_feature: String,
    pub all_zero_conditions: bool,
    pub asserted_bits: Vec<BitLocation>,
}

#[derive(Clone, Serialize)]
pub struct BitLocation {
    pub frame: u32,
    pub bit: usize,
}

#[derive(Serialize)]
pub struct UnresolvedSegment {
    pub tile: String,
    pub tile_type: String,
    pub bus: String,
    pub required_database_type: String,
    pub reason: &'static str,
}

#[derive(Serialize)]
pub struct FrameBits {
    pub frame: u32,
    /// Absolute bit indices within the 101-word frame, least-significant-bit first per word.
    pub bits: Vec<usize>,
}

fn alias_suffix(name: &str, alias: Option<&Alias>) -> Result<String> {
    let (_, suffix) = name.split_once('.').context("feature prefix")?;
    let Some(alias) = alias else {
        return Ok(suffix.into());
    };
    let mut reverse = BTreeMap::new();
    for (from, to) in &alias.sites {
        ensure!(
            reverse.insert(to.as_str(), from.as_str()).is_none(),
            "ambiguous alias site mapping"
        );
    }
    let (site, rest) = suffix.split_once('.').unwrap_or((suffix, ""));
    let site = reverse.get(site).copied().unwrap_or(site);
    Ok(if rest.is_empty() {
        site.into()
    } else {
        format!("{site}.{rest}")
    })
}

fn positions(rule: &Rule, range: &RawSegment) -> Result<Option<Vec<BitLocation>>> {
    let base = range.base_address()?;
    let first_word = range.alias.as_ref().map_or(0, |a| a.start_offset);
    if range.alias.is_some()
        && rule
            .bits
            .iter()
            .any(|b| b.bit / 32 < first_word || b.bit / 32 >= first_word + range.words)
    {
        return Ok(None);
    }
    let mut positions = Vec::with_capacity(rule.bits.len());
    for condition in &rule.bits {
        ensure!(
            condition.frame < range.frames && condition.bit / 32 < first_word + range.words,
            "feature exceeds selected tile segment"
        );
        let bit = range.offset * 32 + condition.bit - first_word * 32;
        ensure!(
            !(1600..1613).contains(&bit),
            "feature condition targets frame ECC"
        );
        positions.push(BitLocation {
            frame: base + condition.frame,
            bit,
        });
    }
    Ok(Some(positions))
}

/// Retain every unsupported segment and every set data bit not explained by
/// a matching rule. Default zero-only matches do not imply a used resource.
pub fn decode(
    configuration: &[u8],
    part: &Part,
    grid: &TileGrid,
    databases: &[FeatureDatabase],
) -> Result<Decoding> {
    ensure!(
        grid.part_sha256 == part.sha256(),
        "feature grid part identity differs"
    );
    ensure!(
        !databases.is_empty() && databases.len() <= 64,
        "feature database count bound"
    );
    let mapping = map_configuration(configuration, part)?;
    let lookup: BTreeMap<_, _> = mapping
        .frames
        .iter()
        .enumerate()
        .map(|(i, f)| (f.address, (i, f.byte_offset)))
        .collect();
    let mut table_map = BTreeMap::new();
    for db in databases {
        ensure!(
            table_map
                .insert((db.tile_type.as_str(), db.bus.as_str()), db)
                .is_none(),
            "duplicate feature table identity"
        );
    }
    let mut explained = vec![0u32; mapping.frames.len() * 101];
    let mut decoded = Vec::new();
    let mut unresolved = Vec::new();
    let mut matched_features = 0;
    let mut all_zero_condition_matches = 0;
    for (name, tile) in &grid.tiles {
        for (bus, range) in &tile.bits {
            let required_type = range
                .alias
                .as_ref()
                .map_or(tile.tile_type.as_str(), |a| a.tile_type.as_str());
            let Some(db) = table_map.get(&(required_type, bus.as_str())) else {
                unresolved.push(UnresolvedSegment {
                    tile: name.clone(),
                    tile_type: tile.tile_type.clone(),
                    bus: bus.clone(),
                    required_database_type: required_type.into(),
                    reason: "missing_pinned_feature_database",
                });
                continue;
            };
            let mut matches = Vec::new();
            let mut outside_alias = 0;
            for rule in &db.rules {
                let Some(positions) = positions(rule, range)? else {
                    outside_alias += 1;
                    continue;
                };
                let mut matched = true;
                for (location, condition) in positions.iter().zip(&rule.bits) {
                    let &(_, offset) = lookup
                        .get(&location.frame)
                        .context("feature frame missing from checked map")?;
                    let start = offset + (location.bit / 32) * 4;
                    let word = u32::from_be_bytes(configuration[start..start + 4].try_into()?);
                    if (word & (1 << (location.bit % 32)) != 0) != condition.value {
                        matched = false;
                        break;
                    }
                }
                if !matched {
                    continue;
                }
                ensure!(
                    matched_features < MAX_MATCHES,
                    "matched feature output bound"
                );
                matched_features += 1;
                let mut asserted_bits = Vec::new();
                for (location, condition) in positions.into_iter().zip(&rule.bits) {
                    if condition.value {
                        let &(index, _) = lookup.get(&location.frame).unwrap();
                        explained[index * 101 + location.bit / 32] |= 1 << (location.bit % 32);
                        asserted_bits.push(location);
                    }
                }
                let all_zero_conditions = asserted_bits.is_empty();
                all_zero_condition_matches += usize::from(all_zero_conditions);
                matches.push(MatchedFeature {
                    name: format!("{name}.{}", alias_suffix(&rule.name, range.alias.as_ref())?),
                    database_feature: rule.name.clone(),
                    all_zero_conditions,
                    asserted_bits,
                });
            }
            decoded.push(DecodedSegment {
                tile: name.clone(),
                tile_type: tile.tile_type.clone(),
                bus: bus.clone(),
                database_sha256: db.sha256.clone(),
                alias: range.alias.clone(),
                features_outside_alias_range: outside_alias,
                matches,
            });
        }
    }
    let mut total_bits = 0;
    let mut unexplained_bits = 0;
    let mut unexplained = Vec::new();
    for (index, frame) in mapping.frames.iter().enumerate() {
        let mut bits = Vec::new();
        for word_index in 0..101 {
            let at = frame.byte_offset + word_index * 4;
            let mut word = u32::from_be_bytes(configuration[at..at + 4].try_into()?);
            if word_index == 50 {
                word &= !0x1fff;
            }
            total_bits += word.count_ones() as usize;
            let mut remaining = word & !explained[index * 101 + word_index];
            while remaining != 0 {
                ensure!(
                    unexplained_bits < MAX_OUTPUT_BITS,
                    "unexplained feature bit output bound"
                );
                unexplained_bits += 1;
                bits.push(word_index * 32 + remaining.trailing_zeros() as usize);
                remaining &= remaining - 1;
            }
        }
        if !bits.is_empty() {
            unexplained.push(FrameBits {
                frame: frame.address,
                bits,
            });
        }
    }
    Ok(Decoding {
        configuration_sha256: crate::prepare::hash_bytes(configuration), part_sha256: mapping.part_sha256,
        tilegrid_sha256: grid.sha256.clone(), tables: databases.iter().map(|db| TableIdentity {
            tile_type: db.tile_type.clone(), bus: db.bus.clone(), sha256: db.sha256.clone(), rules: db.rules.len(),
        }).collect(), decoded, unresolved, matched_features, all_zero_condition_matches,
        configuration_set_bits_excluding_ecc: total_bits,
        set_bits_explained_by_matching_features: total_bits - unexplained_bits,
        set_bits_without_matching_features: unexplained_bits, unexplained,
        limit: "Pinned feature bit predicates only. Zero-only defaults need not denote used resources. Unmatched data bits and missing tables remain explicit. No complete routing, reconstructed netlist, timing, firmware execution or exact-board acceptance.",
    })
}

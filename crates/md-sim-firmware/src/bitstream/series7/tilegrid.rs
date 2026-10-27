//! Associate checked frame words with a pinned Project X-Ray tile grid.
//! This identifies physical configuration ranges, not logic or board wiring.
//! Whole-word ranges can overlap: feature bit masks are a separate database.
use super::frames::{map_configuration, unique_map, Part};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_DATABASE_BYTES: usize = 32 * 1024 * 1024;
const MAX_TILES: usize = 100_000;
const FRAME_WORDS: usize = 101;

pub struct TileGrid {
    pub(super) sha256: String,
    pub(super) part_sha256: String,
    pub(super) tiles: BTreeMap<String, RawTile>,
}

impl TileGrid {
    /// The caller must obtain the expected hash independently of these bytes.
    /// Every referenced frame must belong to the separately pinned part.
    pub fn from_pinned_json(bytes: &[u8], expected_sha256: &str, part: &Part) -> Result<Self> {
        ensure!(bytes.len() <= MAX_DATABASE_BYTES, "tile grid size limit");
        let sha256 = crate::prepare::hash_bytes(bytes);
        ensure!(sha256 == expected_sha256, "tile grid hash mismatch");
        #[derive(Deserialize)]
        struct Tiles(#[serde(deserialize_with = "unique_map")] BTreeMap<String, RawTile>);
        let Tiles(tiles) = serde_json::from_slice(bytes).context("tile grid schema")?;
        ensure!(
            !tiles.is_empty() && tiles.len() <= MAX_TILES,
            "tile count limit"
        );
        let mut locations = BTreeSet::new();
        let mut sites = BTreeSet::new();
        for (name, tile) in &tiles {
            text(name)?;
            text(&tile.tile_type)?;
            ensure!(
                tile.grid_x < 4096
                    && tile.grid_y < 4096
                    && locations.insert((tile.grid_x, tile.grid_y)),
                "duplicate or out-of-range tile location"
            );
            if let Some(region) = &tile.clock_region {
                text(region)?;
            }
            for (site, kind) in &tile.sites {
                text(site)?;
                text(kind)?;
                ensure!(sites.insert(site), "duplicate physical site");
            }
            for (pin, function) in &tile.pin_functions {
                text(pin)?;
                text(function)?;
            }
            for site in &tile.prohibited_sites {
                text(site)?;
            }
            for (bus, segment) in &tile.bits {
                let base = segment.base_address()?;
                let block = match bus.as_str() {
                    "CLB_IO_CLK" => 0,
                    "BLOCK_RAM" => 1,
                    "CFG_CLB" => 2,
                    _ => bail!("unsupported tile configuration bus {bus}"),
                };
                ensure!(
                    base >> 23 == block && base & 127 == 0,
                    "tile base address fields"
                );
                ensure!(
                    (1..=128).contains(&segment.frames),
                    "tile frame count limit"
                );
                ensure!(
                    segment.words > 0
                        && segment.offset < FRAME_WORDS
                        && segment.words <= FRAME_WORDS - segment.offset,
                    "tile word range outside frame"
                );
                for minor in 0..segment.frames {
                    ensure!(
                        part.addresses().binary_search(&(base + minor)).is_ok(),
                        "tile {name} references frame absent from part"
                    );
                }
                if let Some(alias) = &segment.alias {
                    text(&alias.tile_type)?;
                    ensure!(alias.start_offset < FRAME_WORDS, "tile alias offset limit");
                    let mut alias_sites = BTreeSet::new();
                    for (from, to) in &alias.sites {
                        text(from)?;
                        text(to)?;
                        ensure!(alias_sites.insert(to), "ambiguous alias site mapping");
                    }
                }
            }
        }
        Ok(Self {
            sha256,
            part_sha256: part.sha256().to_owned(),
            tiles,
        })
    }
}

fn text(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 256
            && value.bytes().all(|b| (0x20..=0x7e).contains(&b)),
        "tile metadata text bound"
    );
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawTile {
    #[serde(default, deserialize_with = "unique_map")]
    pub(super) bits: BTreeMap<String, RawSegment>,
    grid_x: u32,
    grid_y: u32,
    #[serde(rename = "type")]
    pub(super) tile_type: String,
    #[serde(default)]
    clock_region: Option<String>,
    #[serde(deserialize_with = "unique_map")]
    sites: BTreeMap<String, String>,
    #[serde(default, deserialize_with = "unique_map")]
    pin_functions: BTreeMap<String, String>,
    #[serde(default)]
    prohibited_sites: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawSegment {
    baseaddr: String,
    pub(super) frames: u32,
    pub(super) offset: usize,
    pub(super) words: usize,
    #[serde(default)]
    pub(super) alias: Option<Alias>,
}

impl RawSegment {
    pub(super) fn base_address(&self) -> Result<u32> {
        let value = self
            .baseaddr
            .strip_prefix("0x")
            .context("tile address prefix")?;
        ensure!(
            value.len() == 8 && value.bytes().all(|b| b.is_ascii_hexdigit()),
            "tile address encoding"
        );
        Ok(u32::from_str_radix(value, 16)?)
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Alias {
    #[serde(rename = "type")]
    pub tile_type: String,
    pub start_offset: usize,
    #[serde(deserialize_with = "unique_map")]
    pub sites: BTreeMap<String, String>,
}

#[derive(Serialize)]
pub struct TileMapping {
    pub configuration_sha256: String,
    pub part_sha256: String,
    pub tilegrid_sha256: String,
    pub database_tiles: usize,
    pub tiles_without_configuration_ranges: usize,
    pub unique_mapped_frame_words: usize,
    pub unmapped_frame_words: usize,
    /// Additional associations of whole words, not a conflict of feature bits.
    pub repeated_frame_word_associations: usize,
    pub alias_segments: usize,
    pub tiles: Vec<Tile>,
    pub limit: &'static str,
}

#[derive(Serialize)]
pub struct Tile {
    pub name: String,
    pub tile_type: String,
    pub grid_x: u32,
    pub grid_y: u32,
    pub clock_region: Option<String>,
    pub sites: BTreeMap<String, String>,
    pub segments: Vec<Segment>,
}

#[derive(Serialize)]
pub struct Segment {
    pub bus: String,
    pub base_address: u32,
    pub frames: u32,
    pub word_offset: usize,
    pub words_per_frame: usize,
    /// SHA-256 of exact big-endian word bytes in ascending frame order.
    pub raw_words_sha256: String,
    /// Includes ECC bits when a range includes them. This is not feature count.
    pub raw_set_bits: usize,
    pub alias: Option<Alias>,
}

/// Repeat packet, CRC, part and ECC checks before associating any frame bytes.
/// No feature database or placement/routing reconstruction is implied.
pub fn map_tiles(configuration: &[u8], part: &Part, grid: &TileGrid) -> Result<TileMapping> {
    ensure!(
        part.sha256() == grid.part_sha256,
        "tile grid was checked against a different part identity"
    );
    let mapping = map_configuration(configuration, part)?;
    let lookup: BTreeMap<_, _> = mapping
        .frames
        .iter()
        .enumerate()
        .map(|(i, frame)| (frame.address, (i, frame.byte_offset)))
        .collect();
    let mut covered = vec![false; mapping.frames.len() * FRAME_WORDS];
    let mut repeated = 0;
    let mut aliases = 0;
    let mut tiles = Vec::new();
    for (name, tile) in &grid.tiles {
        if tile.bits.is_empty() {
            continue;
        }
        let mut segments = Vec::new();
        for (bus, range) in &tile.bits {
            let base = range.base_address()?;
            let mut hash = Sha256::new();
            let mut set_bits = 0;
            for minor in 0..range.frames {
                let &(index, start) = lookup
                    .get(&(base + minor))
                    .context("tile grid references a different part")?;
                let begin = start + range.offset * 4;
                let end = begin + range.words * 4;
                let bytes = configuration.get(begin..end).context("tile frame slice")?;
                hash.update(bytes);
                set_bits += bytes.iter().map(|b| b.count_ones() as usize).sum::<usize>();
                for word in range.offset..range.offset + range.words {
                    repeated += usize::from(std::mem::replace(
                        &mut covered[index * FRAME_WORDS + word],
                        true,
                    ));
                }
            }
            aliases += usize::from(range.alias.is_some());
            segments.push(Segment {
                bus: bus.clone(),
                base_address: base,
                frames: range.frames,
                word_offset: range.offset,
                words_per_frame: range.words,
                raw_words_sha256: hex::encode(hash.finalize()),
                raw_set_bits: set_bits,
                alias: range.alias.clone(),
            });
        }
        tiles.push(Tile {
            name: name.clone(),
            tile_type: tile.tile_type.clone(),
            grid_x: tile.grid_x,
            grid_y: tile.grid_y,
            clock_region: tile.clock_region.clone(),
            sites: tile.sites.clone(),
            segments,
        });
    }
    let mapped = covered.iter().filter(|v| **v).count();
    Ok(TileMapping {
        configuration_sha256: crate::prepare::hash_bytes(configuration),
        part_sha256: mapping.part_sha256, tilegrid_sha256: grid.sha256.clone(),
        database_tiles: grid.tiles.len(), tiles_without_configuration_ranges: grid.tiles.len() - tiles.len(),
        unique_mapped_frame_words: mapped, unmapped_frame_words: covered.len() - mapped,
        repeated_frame_word_associations: repeated, alias_segments: aliases, tiles,
        limit: "Physical tile/frame-word association only. Aliases are retained, not resolved to features. No logic equivalence, connectivity, timing, firmware execution or board acceptance.",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn part() -> Part {
        let data = include_bytes!("../../../../../fixtures/fpga/xc7z010clg400-1/part.json");
        Part::from_pinned_json(
            data,
            "1754ec1c7a8e0447a116984505cc422819d3be06389527de96bf192c5175d095",
        )
        .unwrap()
    }

    fn fixture() -> Value {
        json!({"TEST_TILE": {"bits":{"CLB_IO_CLK":{"baseaddr":"0x00000000","frames":1,"offset":99,"words":2}},
            "grid_x":1,"grid_y":2,"sites":{},"type":"TEST","prohibited_sites":[],"pin_functions":{}}})
    }

    fn parse(value: &Value) -> Result<TileGrid> {
        let bytes = serde_json::to_vec(value)?;
        TileGrid::from_pinned_json(&bytes, &crate::prepare::hash_bytes(&bytes), &part())
    }

    #[test]
    fn last_two_words_fit_without_requiring_a_logic_database() {
        assert!(parse(&fixture()).is_ok());
        let mut value = fixture();
        value["TEST_TILE"]["bits"]["CLB_IO_CLK"]["words"] = json!(3);
        assert!(parse(&value).is_err());
    }

    #[test]
    fn identity_duplicates_and_wrong_part_ranges_fail() {
        let bytes = serde_json::to_vec(&fixture()).unwrap();
        assert!(TileGrid::from_pinned_json(&bytes, &"0".repeat(64), &part()).is_err());
        let duplicate = String::from_utf8(bytes)
            .unwrap()
            .replace("\"grid_x\":1", "\"grid_x\":1,\"grid_x\":2");
        assert!(TileGrid::from_pinned_json(
            duplicate.as_bytes(),
            &crate::prepare::hash_bytes(duplicate.as_bytes()),
            &part()
        )
        .is_err());
        for (field, value) in [
            ("baseaddr", json!("0x00000001")),
            ("baseaddr", json!("0x00200000")),
            ("baseaddr", json!("0x00800000")),
            ("frames", json!(129)),
            ("frames", json!(0)),
            ("offset", json!(101)),
            ("words", json!(0)),
        ] {
            let mut input = fixture();
            input["TEST_TILE"]["bits"]["CLB_IO_CLK"][field] = value;
            assert!(parse(&input).is_err(), "accepted invalid {field}");
        }
    }

    #[test]
    fn word_sharing_is_preserved_but_physical_sites_and_locations_are_unique() {
        let mut value = fixture();
        value["SECOND_TILE"] = value["TEST_TILE"].clone();
        assert!(parse(&value).is_err());
        value["SECOND_TILE"]["grid_x"] = json!(2);
        assert!(parse(&value).is_ok());
        value["TEST_TILE"]["sites"] = json!({"SITE":"SLICEL"});
        value["SECOND_TILE"]["sites"] = json!({"SITE":"SLICEL"});
        assert!(parse(&value).is_err());
    }

    #[test]
    fn aliases_require_unique_destinations_before_feature_matching() {
        let mut value = fixture();
        value["TEST_TILE"]["bits"]["CLB_IO_CLK"]["alias"] = json!({"type":"PARENT","start_offset":1,"sites":{"FIRST":"PARENT_A","SECOND":"PARENT_B"}});
        assert!(parse(&value).is_ok());
        value["TEST_TILE"]["bits"]["CLB_IO_CLK"]["alias"]["sites"]["SECOND"] = json!("PARENT_A");
        assert!(parse(&value)
            .err()
            .unwrap()
            .to_string()
            .contains("ambiguous alias site mapping"));
    }
}

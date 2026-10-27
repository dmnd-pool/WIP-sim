use bitcoin_hashes::{sha256, Hash};
use md_sim_firmware::{
    bitstream::series7::{
        frames::Part,
        tilegrid::{map_tiles, TileGrid},
    },
    prepare::hash_bytes,
};
use serde_json::json;

#[path = "support/series7.rs"]
mod vectors;

fn grid(part: &Part) -> TileGrid {
    let mut tile = json!({"bits":{"CLB_IO_CLK":{"baseaddr":"0x00000500","frames":2,"offset":99,"words":2}},
        "grid_x":1,"grid_y":2,"sites":{},"type":"TEST"});
    let mut entries = json!({"FIRST":tile.clone()});
    tile["grid_x"] = json!(2);
    tile["bits"]["CLB_IO_CLK"]["alias"] =
        json!({"type":"TEST_ALIAS","start_offset":2,"sites":{"FROM":"TO"}});
    entries["SECOND"] = tile;
    entries["EMPTY"] = json!({"bits":{},"grid_x":3,"grid_y":2,"sites":{},"type":"EMPTY"});
    let bytes = serde_json::to_vec(&entries).unwrap();
    TileGrid::from_pinned_json(&bytes, &hash_bytes(&bytes), part).unwrap()
}

#[test]
fn fixed_checked_frames_map_exact_words_and_keep_alias_and_overlap_explicit() {
    let db = vectors::small_database();
    let part = Part::from_pinned_json(&db, &hash_bytes(&db)).unwrap();
    let bytes = vectors::stream(&vectors::payload(), 0, 0x6c39381b);
    let mapped = map_tiles(&bytes, &part, &grid(&part)).unwrap();
    assert_eq!(mapped.database_tiles, 3);
    assert_eq!(mapped.tiles_without_configuration_ranges, 1);
    assert_eq!(mapped.unique_mapped_frame_words, 4);
    assert_eq!(mapped.unmapped_frame_words, 703);
    assert_eq!(mapped.repeated_frame_word_associations, 4);
    assert_eq!(mapped.alias_segments, 1);
    // Independent SHA implementation and literal bytes from the fixed vector.
    let expected = sha256::Hash::hash(&[
        0, 0x11, 0, 0x11, 0, 0x11, 0, 0x11, 0, 0x11, 0, 0x11, 0, 0x11, 0, 0x11,
    ])
    .to_string();
    for tile in &mapped.tiles {
        let segment = &tile.segments[0];
        assert_eq!(segment.raw_words_sha256, expected);
        assert_eq!(segment.raw_set_bits, 16);
        assert_eq!(segment.base_address, 0x500);
    }
    let alias = mapped.tiles[1].segments[0].alias.as_ref().unwrap();
    assert_eq!(alias.tile_type, "TEST_ALIAS");
    assert_eq!(alias.start_offset, 2);
    assert_eq!(alias.sites["FROM"], "TO");
}

#[test]
fn mapping_rechecks_configuration_and_exact_part_identity() {
    let mut db = vectors::small_database();
    let part = Part::from_pinned_json(&db, &hash_bytes(&db)).unwrap();
    let grid = grid(&part);
    let mut bytes = vectors::stream(&vectors::payload(), 0, 0x6c39381b);
    db.push(b'\n');
    let other_part = Part::from_pinned_json(&db, &hash_bytes(&db)).unwrap();
    assert!(map_tiles(&bytes, &other_part, &grid).is_err());
    bytes[60] ^= 1;
    assert!(map_tiles(&bytes, &part, &grid).is_err());
}

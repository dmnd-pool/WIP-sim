use md_sim_firmware::{
    bitstream::series7::{
        features::{decode, FeatureDatabase},
        frames::Part,
        tilegrid::TileGrid,
    },
    prepare::hash_bytes,
};
use serde_json::{json, Value};

#[path = "support/series7.rs"]
mod vectors;

fn part() -> Part {
    let bytes = vectors::small_database();
    Part::from_pinned_json(&bytes, &hash_bytes(&bytes)).unwrap()
}

fn raw_grid() -> Value {
    json!({"TILE": {"type":"TYPE","grid_x":0,"grid_y":0,"sites":{},
        "bits":{"CLB_IO_CLK":{"baseaddr":"0x00000000","frames":1,"offset":0,"words":2}}},
        "MISSING":{"type":"OTHER","grid_x":1,"grid_y":0,"sites":{},
        "bits":{"CLB_IO_CLK":{"baseaddr":"0x00000000","frames":1,"offset":2,"words":2}}}})
}

fn grid(value: Value, part: &Part) -> TileGrid {
    let bytes = serde_json::to_vec(&value).unwrap();
    TileGrid::from_pinned_json(&bytes, &hash_bytes(&bytes), part).unwrap()
}

fn database(text: &str, tile_type: &str) -> FeatureDatabase {
    FeatureDatabase::from_pinned_text(
        text.as_bytes(),
        &hash_bytes(text.as_bytes()),
        tile_type,
        "CLB_IO_CLK",
    )
    .unwrap()
}

#[test]
fn positive_negated_and_default_rules_preserve_unmatched_bits_and_missing_tables() {
    let part = part();
    let grid = grid(raw_grid(), &part);
    let config = vectors::stream(&vectors::payload(), 0, 0x6c39381b);
    let table = database(
        "TYPE.POSITIVE 00_00 !00_01\nTYPE.DEFAULT !00_01 !00_02\nTYPE.FALSE 00_01\n",
        "TYPE",
    );
    let report = decode(&config, &part, &grid, &[table]).unwrap();
    assert_eq!(report.matched_features, 2);
    assert_eq!(report.all_zero_condition_matches, 1);
    assert_eq!(report.configuration_set_bits_excluding_ecc, 2814);
    assert_eq!(report.set_bits_explained_by_matching_features, 1);
    assert_eq!(report.set_bits_without_matching_features, 2813);
    assert_eq!(report.unresolved.len(), 1);
    assert_eq!(report.unresolved[0].tile, "MISSING");
    let matches = &report.decoded[0].matches;
    assert_eq!(matches[0].name, "TILE.POSITIVE");
    assert_eq!(matches[0].asserted_bits[0].frame, 0);
    assert_eq!(matches[0].asserted_bits[0].bit, 0);
    assert!(matches[1].all_zero_conditions);
    assert!(matches[1].asserted_bits.is_empty());
    assert!(!report.unexplained[0].bits.contains(&0));
    assert!(report.unexplained[0].bits.contains(&4));
    assert!(report
        .unexplained
        .iter()
        .all(|frame| frame.bits.iter().all(|b| !(1600..1613).contains(b))));
}

#[test]
fn alias_subtracts_word_offset_translates_sites_and_filters_entire_rules() {
    let part = part();
    let mut raw = raw_grid();
    raw["TILE"]["bits"]["CLB_IO_CLK"]["alias"] =
        json!({"type":"PARENT","start_offset":2,"sites":{"REAL_SITE":"ALIAS_SITE"}});
    let grid = grid(raw, &part);
    let config = vectors::stream(&vectors::payload(), 0, 0x6c39381b);
    let table = database(
        "PARENT.ALIAS_SITE.POSITIVE 00_64 !00_65\nPARENT.ALIAS_SITE.OUTSIDE 00_64 !00_01\n",
        "PARENT",
    );
    let report = decode(&config, &part, &grid, &[table]).unwrap();
    assert_eq!(report.matched_features, 1);
    assert_eq!(report.decoded[0].features_outside_alias_range, 1);
    assert_eq!(report.decoded[0].matches[0].name, "TILE.REAL_SITE.POSITIVE");
    assert_eq!(report.decoded[0].matches[0].asserted_bits[0].bit, 0);
}

#[test]
fn malformed_or_ambiguous_tables_are_rejected_before_matching() {
    for text in [
        "",
        "TYPE.X",
        "OTHER.X 00_00",
        "TYPE.X 128_00",
        "TYPE.X 00_3232",
        "TYPE.X always",
        "TYPE.X 00_00 !00_00",
        "TYPE.X 00_00 00_00",
        "TYPE.X 00_00\nTYPE.X 00_01",
        "TYPE.X +0_00",
        "TYPE.X 0_-1",
    ] {
        assert!(
            FeatureDatabase::from_pinned_text(
                text.as_bytes(),
                &hash_bytes(text.as_bytes()),
                "TYPE",
                "CLB_IO_CLK"
            )
            .is_err(),
            "accepted {text}"
        );
    }
    let good = b"TYPE.X 00_00\n";
    assert!(
        FeatureDatabase::from_pinned_text(good, &"0".repeat(64), "TYPE", "CLB_IO_CLK").is_err()
    );
    assert!(FeatureDatabase::from_pinned_text(good, &hash_bytes(good), "TYPE", "UNKNOWN").is_err());
}

#[test]
fn geometry_ecc_part_identity_and_configuration_are_rechecked() {
    let part = part();
    let mut raw = raw_grid();
    raw["TILE"]["bits"]["CLB_IO_CLK"]["offset"] = json!(50);
    let grid = grid(raw, &part);
    let config = vectors::stream(&vectors::payload(), 0, 0x6c39381b);
    for rule in [
        "TYPE.ECC 00_00",
        "TYPE.TOO_FAR 01_13",
        "TYPE.TOO_WIDE 00_64",
    ] {
        assert!(decode(&config, &part, &grid, &[database(rule, "TYPE")]).is_err());
    }
    let mut data = vectors::small_database();
    data.push(b'\n');
    let other = Part::from_pinned_json(&data, &hash_bytes(&data)).unwrap();
    assert!(decode(&config, &other, &grid, &[database("TYPE.X 00_13", "TYPE")]).is_err());
    let mut corrupt = config;
    corrupt[60] ^= 1;
    assert!(decode(&corrupt, &part, &grid, &[database("TYPE.X 00_13", "TYPE")]).is_err());
}

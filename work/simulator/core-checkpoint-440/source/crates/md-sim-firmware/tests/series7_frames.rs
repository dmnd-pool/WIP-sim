use md_sim_firmware::{
    bitstream::series7::{
        frames::{frame_ecc, map_configuration, Part},
        inspect,
    },
    prepare::hash_bytes,
};

const PIN: &str = "1754ec1c7a8e0447a116984505cc422819d3be06389527de96bf192c5175d095";
const DATABASE: &[u8] = include_bytes!("../../../fixtures/fpga/xc7z010clg400-1/part.json");
const ID: u32 = 0x03722093;

#[path = "support/series7.rs"]
mod vectors;
use vectors::{payload, small_database, stream};
fn part(bytes: &[u8]) -> Part {
    Part::from_pinned_json(bytes, &hash_bytes(bytes)).unwrap()
}

#[test]
fn published_part_counts_and_address_boundaries_are_preserved() {
    let part = Part::from_pinned_json(DATABASE, PIN).unwrap();
    assert_eq!(part.idcode(), ID);
    let addresses = part.addresses();
    assert_eq!(addresses.len(), 5144);
    assert_eq!(addresses[0], 0);
    assert_eq!(addresses[1931], (55 << 7) | 41);
    assert_eq!(addresses[1932], 0x00400000);
    assert_eq!(addresses[3863], 0x00400000 | (55 << 7) | 41);
    assert_eq!(addresses[3864], 0x00800000);
    assert_eq!(addresses[4503], 0x00800000 | (4 << 7) | 127);
    assert_eq!(addresses[4504], 0x00c00000);
    assert_eq!(addresses[5143], 0x00c00000 | (4 << 7) | 127);
    assert!(addresses.windows(2).all(|v| v[0] < v[1]));
    assert!(Part::from_pinned_json(DATABASE, &"0".repeat(64)).is_err());
}

#[test]
fn numeric_column_order_and_every_payload_word_are_accounted_for() {
    let part = part(&small_database());
    let expected = [0, 0x100, 0x500, 0x501, 0x00400000, 0x00800000, 0x00c00000];
    assert_eq!(part.addresses(), expected);
    let bytes = stream(&payload(), 0, 0x6c39381b);
    let mapped = map_configuration(&bytes, &part).unwrap();
    assert_eq!(mapped.frames.len(), 7);
    assert_eq!(mapped.mapped_data_words, 707);
    assert_eq!(mapped.checked_padding_words, 808);
    assert_eq!(mapped.inspection.frame_data.words, 1515);
    let offsets = [60, 464, 868, 1272, 2484, 3696, 4908];
    for ((frame, address), offset) in mapped.frames.iter().zip(expected).zip(offsets) {
        assert_eq!(frame.address, address);
        assert_eq!(frame.byte_offset, offset);
        assert_eq!(frame.sha256, hash_bytes(&bytes[offset..offset + 404]));
        assert_eq!(frame.ecc, 0x1004);
    }
    assert_eq!(
        mapped
            .padding
            .iter()
            .map(|p| p.byte_offset)
            .collect::<Vec<_>>(),
        [1676, 2888, 4100, 5312]
    );
    assert_eq!(
        mapped.padding.last().unwrap().byte_offset + 808,
        mapped.inspection.frame_data.byte_offset + mapped.inspection.frame_data.words * 4
    );
}

#[test]
fn valid_crc_cannot_hide_nonzero_missing_or_excess_padding_or_partial_data() {
    let part = part(&small_database());
    let good = payload();
    let mut nonzero = good.clone();
    nonzero[404] = 1;
    let mut extra = good.clone();
    extra.extend([0; 101]);
    let mut bad_ecc = good.clone();
    bad_ecc[50] ^= 1;
    for (data, far, crc, error) in [
        (nonzero, 0, 0x3de1103f, "nonzero row padding"),
        (good[..1414].to_vec(), 0, 0x625eb3b0, "missing row padding"),
        (extra, 0, 0xd74f1723, "unmapped FDRI data"),
        (good, 128, 0x61ddc573, "partial configuration"),
        (bad_ecc, 0, 0x2c682133, "frame ECC mismatch"),
    ] {
        let bytes = stream(&data, far, crc);
        inspect(&bytes, ID).unwrap();
        let result = map_configuration(&bytes, &part).unwrap_err();
        assert!(result.to_string().contains(error), "{result:#}");
    }
}

#[test]
fn ecc_bit_labels_cross_reserved_ranges_and_exclude_the_stored_code() {
    for (word, bit, expected) in [
        (0, 0, 0x0320),
        (6, 31, 0x13ff),
        (7, 0, 0x1420),
        (37, 31, 0x07ff),
        (38, 0, 0x1820),
        (50, 13, 0x09ad),
        (100, 31, 0x1fff),
    ] {
        let mut words = [0u32; 101];
        words[word] = 1 << bit;
        let bytes: Vec<_> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
        assert_eq!(frame_ecc(&bytes).unwrap(), expected);
    }
    let mut words = [0u32; 101];
    for stored in 0..=0x1fff {
        words[50] = stored;
        let bytes: Vec<_> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
        assert_eq!(frame_ecc(&bytes).unwrap(), 0);
    }
    for size in [0, 400, 403, 405, 408] {
        assert!(frame_ecc(&vec![0; size]).is_err());
    }
    // Each non-ECC single bit has a distinct nonzero syndrome.
    let mut syndromes = std::collections::BTreeSet::new();
    for bit in 0..3232 {
        if (1600..1613).contains(&bit) {
            continue;
        }
        let mut words = [0u32; 101];
        words[bit / 32] = 1 << (bit % 32);
        let bytes: Vec<_> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
        let syndrome = frame_ecc(&bytes).unwrap();
        assert_ne!(syndrome, 0);
        assert!(syndromes.insert(syndrome));
    }
}

#[test]
fn invalid_geometry_unknown_fields_and_duplicate_keys_are_rejected() {
    let text = String::from_utf8(small_database()).unwrap();
    for changed in [
        text.replacen("\"idcode\":57811091", "\"idcode\":0", 1),
        text.replacen("\"frame_count\":1", "\"frame_count\":0", 1),
        text.replacen("\"frame_count\":1", "\"frame_count\":129", 1),
        text.replacen("\"frame_count\":1", "\"frame_count\":1,\"ignored\":2", 1),
        text.replacen("\"10\":", "\"1024\":", 1),
        text.replacen("\"10\":", "\"01\":", 1),
        text.replacen("\"10\":", "\"-1\":", 1),
        text.replacen("\"10\":", "\"2\":", 1),
        text.replacen(
            "\"0\":{\"configuration_buses\"",
            "\"32\":{\"configuration_buses\"",
            1,
        ),
        text.replacen(
            "\"0\":{\"configuration_buses\"",
            "\"1\":{\"configuration_buses\"",
            1,
        ),
        text.replacen("BLOCK_RAM", "reserved", 1),
        text.replacen("BLOCK_RAM", "CFG_CLB", 1),
    ] {
        assert!(
            Part::from_pinned_json(changed.as_bytes(), &hash_bytes(changed.as_bytes())).is_err(),
            "accepted {changed}"
        );
    }
    assert!(Part::from_pinned_json(&vec![b' '; 1024 * 1024 + 1], PIN).is_err());
}

#[test]
fn row_transitions_sort_before_the_half_and_block_transition() {
    let mut value: serde_json::Value = serde_json::from_slice(&small_database()).unwrap();
    let rows = &mut value["global_clock_regions"]["top"]["rows"];
    rows["1"] = rows["0"].clone();
    let part = part(&serde_json::to_vec(&value).unwrap());
    assert_eq!(
        part.addresses(),
        [
            0, 0x100, 0x500, 0x501, 0x20000, 0x20100, 0x20500, 0x20501, 0x400000, 0x800000,
            0x820000, 0xc00000
        ]
    );
}

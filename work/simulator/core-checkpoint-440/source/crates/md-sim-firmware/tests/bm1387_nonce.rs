use md_sim_firmware::bm1387::nonce::{Chip, Error, Layout};
use std::collections::HashSet;

fn chip(address: u8, cores: &[u8]) -> Chip {
    Chip {
        address,
        cores: cores.to_vec(),
    }
}

#[test]
fn complete_one_core_space_has_no_duplicates_and_respects_archived_owner_bits() {
    let layout = Layout::new(vec![chip(104, &[0])]).unwrap();
    assert_eq!(layout.candidate_count(), 524288);
    let mut seen = HashSet::new();
    for index in 0..layout.candidate_count() {
        let nonce = layout.at(index).unwrap();
        // Independent formulas copied from the pinned driver CoreAddress.
        assert_eq!((nonce >> 2) & 0x3f, 26);
        assert_eq!((nonce >> 24) & 0x7f, 0);
        assert!(seen.insert(nonce));
        assert_eq!(layout.owner(nonce), Some((0, 0)));
    }
    assert!(layout.at(layout.candidate_count()).is_none());
    // Both nonce bit31 values are searched; it is not part of the core ID.
    assert!(seen.contains(&0x80000068));
    assert!(seen.contains(&0x80ffff6b));
}

#[test]
fn explicit_uniform_fixture_keeps_each_chip_core_and_missing_ranges_distinct() {
    // A synthetic address map, not a resolved S9 silicon map.
    let cores: Vec<_> = (0..114).collect();
    let layout = Layout::new((0..63).map(|n| chip(n * 4, &cores)).collect()).unwrap();
    assert_eq!(layout.candidate_count(), 63 * 114 * 524288);
    for position in 0..(63 * 114) {
        let nonce = layout.at(position).unwrap();
        assert_eq!(
            layout.owner(nonce),
            Some(((position % 63) as usize, (position / 63) as u8))
        );
    }
    assert_eq!(layout.owner(0x00_00_00_fc), None); // Chip63 absent.
    assert_eq!(layout.owner(0x72_00_00_00), None); // Core114 absent.
    assert_eq!(layout.owner(0xff_00_00_00), None); // Core127 absent.
    assert_eq!(layout.at(layout.candidate_count() - 1), Some(0xf1fffffb));
    assert!(layout.at(u64::MAX).is_none());
    let cores: Vec<_> = (0..128).collect();
    let sparse = Layout::new(vec![chip(252, &cores), chip(0, &cores), chip(104, &cores)]).unwrap();
    assert_eq!(sparse.at(0), Some(252));
    assert_eq!(sparse.at(1), Some(0));
    assert_eq!(sparse.at(2), Some(104));
    assert_eq!(sparse.owner(u32::MAX), Some((0, 127)));
    assert_eq!(sparse.owner(4), None);
    let full = Layout::new((0..64).map(|n| chip(n * 4, &cores)).collect()).unwrap();
    assert_eq!(full.candidate_count(), 1u64 << 32);
    assert_eq!(full.at(u64::from(u32::MAX)), Some(u32::MAX));
    assert!(full.at(1u64 << 32).is_none());
}

#[test]
fn invalid_or_overlapping_topologies_cannot_create_a_search_space() {
    assert!(matches!(Layout::new(vec![]), Err(Error::Chips)));
    assert!(matches!(
        Layout::new((0..65).map(|_| chip(0, &[0])).collect()),
        Err(Error::Chips)
    ));
    assert!(matches!(Layout::new(vec![chip(0, &[])]), Err(Error::Cores)));
    assert!(matches!(
        Layout::new(vec![chip(0, &[0; 129])]),
        Err(Error::Cores)
    ));
    assert!(matches!(
        Layout::new(vec![chip(1, &[0])]),
        Err(Error::Address)
    ));
    assert!(matches!(
        Layout::new(vec![chip(0, &[0]), chip(4, &[1]), chip(0, &[2])]),
        Err(Error::DuplicateAddress)
    ));
    assert!(matches!(
        Layout::new(vec![chip(0, &[128])]),
        Err(Error::CoreAddress)
    ));
    assert!(matches!(
        Layout::new(vec![chip(0, &[3, 17, 3])]),
        Err(Error::DuplicateCore)
    ));
}

#[test]
fn non_consecutive_core_maps_keep_per_chip_holes() {
    let layout = Layout::new(vec![chip(104, &[127, 3]), chip(4, &[64])]).unwrap();
    assert_eq!(layout.candidate_count(), 3 * 524288);
    assert_eq!(layout.at(0), Some(0x7f000068));
    assert_eq!(layout.at(1), Some(0x40000004));
    assert_eq!(layout.at(2), Some(0x03000068));
    assert_eq!(layout.at(3), Some(0x7f000069));
    for (nonce, owner) in [
        (0xff12ab6b, Some((0, 127))),
        (0x8312ab6b, Some((0, 3))),
        (0xc0ffff07, Some((1, 64))),
        (0xc0000068, None),
        (0x03000004, None),
        (0x00000068, None),
        (0x7e000068, None),
    ] {
        assert_eq!(layout.owner(nonce), owner);
    }
}

#[test]
fn vendor_reference_map_matches_counted_core_indices() {
    let evidence: serde_json::Value = serde_json::from_str(include_str!(
        "../../../simulator/backends/qemu/bm1387-reference-core-map.json"
    ))
    .unwrap();
    let cores: Vec<u8> = serde_json::from_value(evidence["cores"].clone()).unwrap();
    assert_eq!(cores.len(), 114);
    // Independent vendor receive formula, applied only to its114 counted
    // positions. Codes57..63 have no mapping. Codes121..127 exceed that
    // count after subtraction. This does not certify a physical stepping.
    for (position, &code) in cores.iter().enumerate() {
        let dense = if code <= 56 {
            usize::from(code)
        } else {
            assert!((64..128).contains(&code));
            usize::from(code) - 7
        };
        assert_eq!(dense, position);
    }
    let layout = Layout::new(vec![chip(0, &cores)]).unwrap();
    assert_eq!(layout.candidate_count(), 114 * 524288);
    for code in 0..128u32 {
        let nonce = code << 24;
        let present = code <= 56 || (64..=120).contains(&code);
        assert_eq!(layout.owner(nonce).is_some(), present);
    }
}

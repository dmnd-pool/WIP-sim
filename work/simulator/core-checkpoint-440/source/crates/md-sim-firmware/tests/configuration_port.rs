use md_sim_firmware::{
    bitstream::{port::Port, series7::frames::Part},
    prepare::hash_bytes,
};
#[path = "support/series7.rs"]
mod vectors;

fn fixture() -> (Vec<u8>, Vec<u8>, Part) {
    let bytes = vectors::stream(&vectors::payload(), 0, 0x6c39381b);
    let mut container = vec![0, 9, 15, 240, 15, 240, 15, 240, 15, 240, 0, 0, 1];
    for key in b"abcd" {
        container.extend([*key, 0, 2, b'X', 0]);
    }
    container.push(b'e');
    container.extend((bytes.len() as u32).to_be_bytes());
    container.extend(&bytes);
    let db = vectors::small_database();
    let part = Part::from_pinned_json(&db, &hash_bytes(&db)).unwrap();
    (container, bytes, part)
}

fn dma(bytes: &[u8]) -> Vec<u8> {
    bytes
        .chunks_exact(4)
        .flat_map(|w| w.iter().rev().copied())
        .collect()
}

#[test]
fn complete_image_and_sync_suffix_have_exact_dma_identity_across_chunk_sizes() {
    let (container, bytes, part) = fixture();
    for start in [0, 16] {
        for chunk in [4, 12, 1024, 4096, bytes.len()] {
            let mut port = Port::new(&container, &part).unwrap();
            port.begin();
            let input = dma(&bytes[start..]);
            let mut received = 0;
            for block in input.chunks(chunk) {
                assert!(!port.complete());
                port.write_dma(block).unwrap();
                received += block.len();
                assert_eq!(port.received(), received);
                assert_eq!(port.complete(), received == input.len());
            }
            assert_eq!(port.omitted_preamble(), start);
            assert_eq!(port.received_sha256(), hash_bytes(&bytes[start..]));
        }
    }
}

#[test]
fn missing_changed_unaligned_and_extra_data_never_complete() {
    let (container, bytes, part) = fixture();
    let input = dma(&bytes);
    let mut port = Port::new(&container, &part).unwrap();
    // Every possible word-aligned truncation remains incomplete.
    for end in (4..input.len()).step_by(4) {
        port.reset();
        port.begin();
        port.write_dma(&input[..end]).unwrap();
        assert!(!port.complete());
    }
    for at in [0, 16, 60, 1660, input.len() - 1] {
        let mut changed = input.clone();
        changed[at] ^= 1;
        port.reset();
        port.begin();
        assert!(port.write_dma(&changed).is_err());
        assert_eq!(port.received(), 0); // No partial commit of a DMA chunk.
        assert!(!port.complete());
        assert!(port.write_dma(&input).is_err()); // Sticky until reset.
    }
    for len in [0, 1, 3, 5] {
        port.reset();
        port.begin();
        assert!(port.write_dma(&input[..len]).is_err());
    }
    port.reset();
    port.begin();
    port.write_dma(&input).unwrap();
    assert!(port.complete());
    assert!(port.write_dma(&[0; 4]).is_err());
    assert!(!port.complete());
    port.reset();
    port.begin();
    let mut extra = input.clone();
    extra.extend([0; 4]);
    assert!(port.write_dma(&extra).is_err());
    assert_eq!(port.received(), 0);
}

#[test]
fn programming_reset_discards_prior_words_and_requires_new_enable() {
    let (container, bytes, part) = fixture();
    let mut port = Port::new(&container, &part).unwrap();
    let input = dma(&bytes);
    assert!(port.write_dma(&input).is_err());
    port.begin();
    assert!(port.write_dma(&input).is_err());
    port.reset();
    port.begin();
    port.write_dma(&input[..128]).unwrap();
    port.reset();
    assert_eq!(port.received(), 0);
    assert!(!port.complete());
    port.begin();
    assert!(port.write_dma(&input[128..]).is_err());
    port.reset();
    port.begin();
    port.write_dma(&input).unwrap();
    assert!(port.complete());
    port.reset();
    assert!(!port.complete());
    assert_eq!(port.received_sha256(), hash_bytes(&[]));
}

#[test]
fn bus_rejection_preserves_partial_evidence_and_latches_until_reset() {
    let (container, bytes, part) = fixture();
    let mut port = Port::new(&container, &part).unwrap();
    let input = dma(&bytes);
    port.begin();
    port.write_dma(&input[..128]).unwrap();
    let prefix = port.received_sha256();
    port.reject();
    port.begin();
    assert!(port.write_dma(&input[128..]).is_err());
    assert!(!port.complete());
    assert_eq!(port.received(), 128);
    assert_eq!(port.received_sha256(), prefix);
    port.reset();
    assert!(port.write_dma(&input).is_err()); // Enable is still required.
    port.reset();
    port.begin();
    port.write_dma(&input).unwrap();
    assert!(port.complete());
}

#[test]
fn damaged_reference_image_cannot_be_authorized() {
    let (mut container, _, part) = fixture();
    container[120] ^= 1;
    assert!(Port::new(&container, &part).is_err());
}

use md_sim_firmware::bitstream::series7::inspect;

const ID: u32 = 0x03722093;

// The footer write values and CRC are from selected FPGA980532de... . NOPs
// between those writes are omitted. The zero-frame prefix CRC is a fixed
// Project X-Ray ICAP algorithm vector.
// The complete archived payload is checked separately by s9_bitstream_probe.
fn words() -> Vec<u32> {
    let mut data = vec![
        0xffffffff, 0x000000bb, 0x11220044, 0xffffffff, 0xaa995566, 0x30008001,
        7, // reset CRC
        0x30018001, ID, // silicon identity
        0x30002001, 0, // FAR
        0x30008001, 1, // WCFG
        0x30004000, 0x50000065, // extended FDRI,101words
    ];
    data.extend([0; 101]);
    data.extend([
        0x30000001, 0xe733e8df, 0x30008001, 10, 0x30008001, 3, 0x30008001, 5, 0x30002001,
        0x03be0000, 0x3000c001, 0x501, 0x3000a001, 0x501, 0x30000001, 0xe3ad7ea5, 0x30008001, 13,
        0x20000000,
    ]);
    data
}

fn bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|v| v.to_be_bytes()).collect()
}

#[test]
fn crc_domains_include_register_addresses_and_reset_after_a_check() {
    let input = bytes(&words());
    let result = inspect(&input, ID).unwrap();
    assert_eq!(result.idcode, ID);
    assert_eq!(result.sync_byte_offset, 16);
    assert_eq!(result.frame_data.byte_offset, 60);
    assert_eq!(result.frame_data.words, 101);
    assert_eq!(result.frame_data.initial_far, 0);
    assert_eq!(result.crc_checks.len(), 2);
    assert_eq!(result.crc_checks[0].computed, 0xe733e8df);
    assert_eq!(result.crc_checks[1].computed, 0xe3ad7ea5);
    assert!(result.crc_checks.iter().all(|c| c.expected == c.computed));
    assert_eq!(result.nop_count, 1);
    assert_eq!(result.zero_padding_words, 0);
}

#[test]
fn damaged_frame_register_and_crc_values_are_rejected() {
    let input = words();
    // Every data bit in the frame participates, as do register addresses
    // and values. Neither a valid container nor a valid gzip CRC suffices.
    for index in 15..116 {
        for bit in 0..32 {
            let mut changed = input.clone();
            changed[index] ^= 1 << bit;
            assert!(
                inspect(&bytes(&changed), ID).is_err(),
                "frame word{index} bit{bit}"
            );
        }
    }
    for index in [8, 10, 117, 119, 121, 123, 125, 127, 129, 131] {
        let mut changed = input.clone();
        changed[index] ^= 1;
        assert!(
            inspect(&bytes(&changed), ID).is_err(),
            "register word{index}"
        );
    }
    assert!(inspect(&bytes(&input), ID ^ 1).is_err());
}

#[test]
fn truncated_overlong_and_unsupported_packets_cannot_silently_end_inspection() {
    let input = bytes(&words());
    // The final NOP is optional; everything through DESYNC must be present.
    for size in 0..input.len() - 4 {
        assert!(inspect(&input[..size], ID).is_err(), "truncation{size}");
    }
    assert!(inspect(&input[..input.len() - 4], ID).is_ok());
    for (at, value) in [
        (0, 0x12345678),  // unexplained preamble
        (5, 0x28008001),  // read opcode
        (5, 0x30009801),  // reserved header bits
        (5, 0x30040001),  // address exceeds CRC's five bits
        (13, 0x20000000), // orphan Type2
        (14, 0x57ffffff), // extended count exceeds actual data
        (14, 0x50000064), // incomplete frame
        (5, 0x30014001),  // unsupported compressed MFWR
        (5, 0x30016001),  // unsupported encrypted CBC
    ] {
        let mut changed = words();
        changed[at] = value;
        assert!(
            inspect(&bytes(&changed), ID).is_err(),
            "word{at} value{value:08x}"
        );
    }
    let mut changed = words();
    changed.extend([0x30008001, 7]);
    assert!(inspect(&bytes(&changed), ID).is_err());
    let mut changed = words();
    changed.insert(5, 0);
    assert!(inspect(&bytes(&changed), ID).is_err());
    let mut changed = words();
    changed.extend([0; 8]);
    assert_eq!(inspect(&bytes(&changed), ID).unwrap().zero_padding_words, 8);
}

#[test]
fn unsupported_encryption_and_crc_discard_are_explicit_failures() {
    let mut changed = words();
    changed.splice(7..7, [0x3000c001, 0x40, 0x3000a001, 0x40]);
    assert!(format!("{:#}", inspect(&bytes(&changed), ID).unwrap_err()).contains("encrypted"));
    let mut changed = words();
    changed.splice(116..116, [0x30008001, 7]);
    assert!(
        format!("{:#}", inspect(&bytes(&changed), ID).unwrap_err()).contains("discard evidence")
    );
    let mut changed = words();
    changed.splice(7..7, [0x3000c001, 1 << 21, 0x30030001, 1 << 21]);
    assert!(format!("{:#}", inspect(&bytes(&changed), ID).unwrap_err()).contains("CTL1"));
}

#[test]
fn data_and_startup_crc_checks_cannot_be_omitted() {
    for remove in [116..118, 130..132] {
        let mut changed = words();
        changed.drain(remove);
        assert!(inspect(&bytes(&changed), ID).is_err());
    }
    let mut changed = words();
    changed.truncate(14); // zero-length Type1 with missing Type2
    assert!(inspect(&bytes(&changed), ID).is_err());
}

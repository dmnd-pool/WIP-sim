use md_sim_firmware::s9io::crc::{Kind, SerialCrc};

fn byte(engine: &mut SerialCrc, value: u8) {
    assert!(engine.ready());
    assert!(engine.tick(Some(value), false, 0));
    for bit in 0..8 {
        assert!(!engine.ready());
        // A competing write during ANY of the eight shifts must drop.
        assert!(!engine.tick(Some(0xee), false, 0));
        assert_eq!(engine.ready(), bit == 7);
    }
}

#[test]
fn clocked_crc_outputs_match_literal_archived_transmit_vectors() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/s9io-reference.json")).unwrap();
    for (name, kind, count) in [
        ("tc_send_cmd5", Kind::Command, 9),
        ("tc_send_cmd9", Kind::Command, 11),
        ("tc_send_work_midstate1", Kind::Work, 1),
        ("tc_send_work_midstate4", Kind::Work, 1),
    ] {
        let mut engine = SerialCrc::new(kind);
        for index in 1..=count {
            let wire: Vec<_> = reference["tasks"][name][format!("uart_data{index}")]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            engine.tick(None, true, 0);
            let length = wire.len() - if matches!(kind, Kind::Work) { 2 } else { 1 };
            for &value in &wire[..length] {
                byte(&mut engine, value);
            }
            let expected = if matches!(kind, Kind::Work) {
                u16::from_be_bytes(wire[length..].try_into().unwrap())
            } else {
                u16::from(wire[length])
            };
            assert_eq!(engine.value(), expected, "{name} vector{index}");
        }
    }
}

#[test]
fn response_engine_uses_type_initialization_and_modified_polynomial() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/s9io-reference.json")).unwrap();
    let mut engine = SerialCrc::new(Kind::Response);
    for name in ["tc_cmd_response", "tc_work_response"] {
        for (key, values) in reference["tasks"][name].as_object().unwrap() {
            if !key.starts_with("uart") {
                continue;
            }
            let values: Vec<_> = values
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            assert_eq!(values.len(), 7);
            engine.tick(None, true, if values[6] & 0x80 != 0 { 0x1b } else { 3 });
            for &value in &values[..6] {
                byte(&mut engine, value);
            }
            assert_eq!(engine.value(), u16::from(values[6] & 31), "{name}/{key}");
        }
    }
}

#[test]
fn synchronous_clear_does_not_reset_busy_state_but_ip_reset_does() {
    for (kind, reset) in [
        (Kind::Command, 31),
        (Kind::Work, 65535),
        (Kind::Response, 31),
    ] {
        let mut engine = SerialCrc::new(kind);
        assert_eq!(engine.value(), reset);
        assert!(engine.tick(Some(0xa5), true, 0xff));
        assert!(!engine.ready());
        for _ in 0..3 {
            assert!(!engine.tick(None, false, 0));
        }
        // Fourth bit is shifted even when clear overrides its CRC update.
        assert!(!engine.tick(Some(0xff), true, 0xff));
        assert_eq!(engine.value(), reset);
        for bit in 0..4 {
            assert!(!engine.ready());
            engine.tick(None, false, 0);
            assert_eq!(engine.ready(), bit == 3);
        }
        engine.tick(Some(0x55), false, 0);
        engine.reset();
        assert!(engine.ready());
        assert_eq!(engine.value(), reset);
    }
}

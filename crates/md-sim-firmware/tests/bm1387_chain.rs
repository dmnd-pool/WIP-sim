use md_sim_firmware::bm1387::{
    self,
    chain::{Chain, Error, Profile},
    Command, Decoder, Packet,
};

fn profile() -> Profile {
    Profile {
        chips: 63,
        cores_per_chip: 114,
        oscillator_hz: 25_000_000,
        initial_baud_divisor: 26,
        // Literal archived GetAddressReg test vector. This is not a
        // measured reset value for a resolved production silicon stepping.
        identity_reserved: 0x90,
    }
}
fn packet(broadcast: bool, command: Command) -> Packet {
    Packet::Command { broadcast, command }
}
fn read(address: u8, register: u8) -> Command {
    Command::Read { address, register }
}
fn write(address: u8, register: u8, value: u32) -> Command {
    Command::Write {
        address,
        register,
        value,
    }
}
fn enumerate(chain: &mut Chain) {
    for _ in 0..3 {
        chain
            .receive(&packet(
                true,
                Command::Inactive {
                    address: 0,
                    reserved: 0,
                },
            ))
            .unwrap();
    }
    for index in 0..63 {
        chain
            .receive(&packet(
                false,
                Command::SetAddress {
                    address: index * 4,
                    reserved: 0,
                },
            ))
            .unwrap();
    }
}
fn ready() -> Chain {
    let mut chain = Chain::new(profile()).unwrap();
    chain.power(true).unwrap();
    chain.reset_line(false);
    chain
}
fn value(reply: &[u8; 7]) -> u32 {
    // The shared packet regression separately checks all reply checksum bits.
    assert_eq!(&reply[4..6], &[0, 0]);
    u32::from_be_bytes(reply[..4].try_into().unwrap())
}

#[test]
fn explicit_sensor_wiring_routes_only_its_selected_chip_and_bus() {
    use md_sim_firmware::{
        bm1387::i2c,
        tmp421::{Diode, Inputs, Timing},
    };
    let mut chain = Chain::new(profile()).unwrap();
    chain
        .attach_temperature(
            61,
            i2c::Profile {
                address_7bit: 0x4c,
                clock_hz: 100_000,
                reset_word: 0,
                timing: Timing::REFERENCE,
                inputs: Inputs {
                    local_millicelsius: 25_000,
                    remote_millicelsius: 65_000,
                    diode: Diode::Connected,
                    supply_valid: true,
                },
            },
        )
        .unwrap();
    chain
        .install_temperature_plan(vec![i2c::InputEvent {
            at_ns: 300_000_000,
            inputs: Inputs {
                local_millicelsius: 25_000,
                remote_millicelsius: 105_000,
                diode: Diode::Connected,
                supply_valid: true,
            },
        }])
        .unwrap();
    chain.power(true).unwrap();
    chain.reset_line(false);
    enumerate(&mut chain);
    chain
        .receive(&packet(true, write(0, 0x1c, 0x0020_0180)))
        .unwrap();
    assert_eq!(
        chain.receive(&packet(false, write(240, 0x1c, 0x4020_41e0))),
        Err(Error::PinRoutingUnsupported)
    );
    assert_eq!(
        chain.receive(&packet(false, write(244, 0x1c, 0x4021_41e0))),
        Err(Error::PinRoutingUnsupported)
    );
    chain
        .receive(&packet(false, write(244, 0x1c, 0x4020_41e0)))
        .unwrap();
    assert_eq!(
        value(
            &chain
                .receive(&packet(false, read(244, 0x20)))
                .unwrap()
                .replies[0]
        ),
        0
    );
    chain
        .receive(&packet(false, write(244, 0x20, 0x0198_fe00)))
        .unwrap();
    assert_eq!(
        chain.receive(&packet(false, write(244, 0x1c, 0x0020_0180))),
        Err(Error::I2c(i2c::Error::RoutingChangeDuringTransfer))
    );
    chain.advance(389_999).unwrap();
    assert_eq!(
        value(
            &chain
                .receive(&packet(false, read(244, 0x20)))
                .unwrap()
                .replies[0]
        ),
        0x8198_fe00
    );
    chain.advance(390_000).unwrap();
    assert_eq!(
        value(
            &chain
                .receive(&packet(false, read(244, 0x20)))
                .unwrap()
                .replies[0]
        ),
        0x0198_fe55
    );
    assert!(chain.receive(&packet(false, read(240, 0x20))).is_err());
    for (at, expected) in [(250_000_000, 0x0198_0141), (500_000_000, 0x0198_0169)] {
        chain.advance(at).unwrap();
        chain
            .receive(&packet(false, write(244, 0x20, 0x0198_0100)))
            .unwrap();
        chain.advance(at + 390_000).unwrap();
        let replies = chain
            .receive(&packet(false, read(244, 0x20)))
            .unwrap()
            .replies;
        assert_eq!(replies.len(), 1);
        assert_eq!(value(&replies[0]), expected);
    }
    chain.power(false).unwrap();
    chain.reset_time_origin();
    chain.power(true).unwrap();
    chain.reset_line(false);
    enumerate(&mut chain);
    assert_eq!(
        chain.receive(&packet(false, read(244, 0x1c))),
        Err(Error::UnknownResetValue(0x1c))
    );
}

#[test]
fn powered_chain_retains_all_63_addresses_and_resets_real_state() {
    let query = bm1387::decode(&[0x54, 5, 0, 0, 0x19]).unwrap();
    let mut chain = Chain::new(profile()).unwrap();
    assert!(chain.receive(&query).unwrap().replies.is_empty());
    chain.power(true).unwrap();
    assert!(chain.receive(&query).unwrap().replies.is_empty());
    chain.reset_line(false);
    let replies = chain.receive(&query).unwrap().replies;
    assert_eq!(replies.len(), 63);
    assert!(replies.iter().all(|r| value(r) == 0x1387_9000));
    assert_eq!(
        chain.receive(&packet(false, read(0, 0))),
        Err(Error::AmbiguousAddress)
    );
    enumerate(&mut chain);
    for index in 0..63 {
        let replies = chain
            .receive(&packet(false, read(index * 4, 0)))
            .unwrap()
            .replies;
        assert_eq!(replies.len(), 1);
        assert_eq!(value(&replies[0]), 0x1387_9000 | u32::from(index * 4));
    }
    assert!(chain
        .receive(&packet(false, read(252, 0)))
        .unwrap()
        .replies
        .is_empty());
    chain.reset_line(true);
    assert!(chain.receive(&query).unwrap().replies.is_empty());
    chain.reset_line(false);
    assert!(chain
        .receive(&query)
        .unwrap()
        .replies
        .iter()
        .all(|r| value(r) == 0x1387_9000));
    enumerate(&mut chain);
    chain.power(false).unwrap();
    assert!(chain.receive(&query).unwrap().replies.is_empty());
    chain.power(true).unwrap();
    assert!(chain
        .receive(&query)
        .unwrap()
        .replies
        .iter()
        .all(|r| value(r) == 0x1387_9000));
}

#[test]
fn programmed_filters_and_serial_rates_have_consistent_readback() {
    let mut chain = ready();
    enumerate(&mut chain);
    assert_eq!(chain.initial_serial_period_ns(), 8640);
    assert_eq!(
        chain.receive(&packet(false, read(0, 0x18))),
        Err(Error::UnknownResetValue(0x18))
    );
    chain.receive(&packet(true, write(0, 0x18, 0x3f))).unwrap();
    chain
        .receive(&packet(false, write(124, 0x18, 0x0100_0002)))
        .unwrap();
    let replies = chain.receive(&packet(true, read(0, 0x18))).unwrap().replies;
    assert_eq!(replies.len(), 63);
    for (index, reply) in replies.iter().enumerate() {
        assert_eq!(value(reply), if index == 31 { 0x0100_0002 } else { 0x3f });
    }
    // These words follow the archived MiscCtrlReg bit fields. not_set_baud
    // does not suppress a divisor change; the driver warns about that.
    for (word, period) in [(0x4020_9a80, 8640), (0x0020_8180, 640)] {
        let effect = chain.receive(&packet(true, write(0, 0x1c, word))).unwrap();
        assert_eq!(effect.serial_period_ns, Some(period));
        assert!(chain
            .receive(&packet(true, read(0, 0x1c)))
            .unwrap()
            .replies
            .iter()
            .all(|r| value(r) == word));
    }
    assert_eq!(
        chain.receive(&packet(false, write(0, 0x1c, 0x4020_9a80))),
        Err(Error::DifferentSerialRates)
    );
    assert_eq!(
        value(
            &chain
                .receive(&packet(false, read(0, 0x1c)))
                .unwrap()
                .replies[0]
        ),
        0x0020_8180
    );
    chain.reset_line(true);
    chain.reset_line(false);
    assert_eq!(
        chain.receive(&packet(true, read(0, 0x18))),
        Err(Error::UnknownResetValue(0x18))
    );
}

#[test]
fn unsupported_states_are_errors_and_invalid_writes_are_atomic() {
    let mut chain = ready();
    enumerate(&mut chain);
    chain
        .receive(&packet(true, write(0, 0x1c, 0x0020_8180)))
        .unwrap();
    assert_eq!(
        chain.receive(&packet(true, write(0, 0x1c, 0x4020_5ae0))),
        Err(Error::PinRoutingUnsupported)
    );
    assert_eq!(
        chain.receive(&packet(true, write(0, 0x1c, 0x0020_ff80))),
        Err(Error::InvalidRegister(0x1c))
    );
    assert_eq!(
        value(
            &chain
                .receive(&packet(false, read(0, 0x1c)))
                .unwrap()
                .replies[0]
        ),
        0x0020_8180
    );
    chain
        .receive(&packet(false, write(0, 0x0c, 0x0068_0221)))
        .unwrap();
    assert_eq!(
        chain.receive(&packet(false, read(0, 0x0c))),
        Err(Error::PllStatusUnknown)
    );
    for value in [0, 0x0068_0201, 0x0068_0223, 0x8068_0221, 0x0068_1221] {
        assert_eq!(
            chain.receive(&packet(false, write(0, 0x0c, value))),
            Err(Error::InvalidRegister(0x0c))
        );
    }
    assert_eq!(
        chain.receive(&packet(true, write(0, 0x20, 1))),
        Err(Error::UnknownRegister(0x20))
    );
    assert_eq!(
        chain.receive(&packet(true, read(0, 0x08))),
        Err(Error::UnknownRegister(0x08))
    );
}

#[test]
fn stream_decoder_accepts_all_archived_commands_and_recovers_after_bad_packets() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/s9io-reference.json")).unwrap();
    let mut decoder = Decoder::default();
    let mut count = 0;
    for task in ["tc_send_cmd5", "tc_send_cmd9"] {
        for (key, values) in reference["tasks"][task].as_object().unwrap() {
            if !key.starts_with("uart") {
                continue;
            }
            let frame: Vec<_> = values
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            let expected = bm1387::decode(&frame).unwrap();
            for (index, byte) in frame.iter().copied().enumerate() {
                let result = decoder.push(byte);
                if index == frame.len() - 1 {
                    assert_eq!(result, Some(Ok(expected.clone())));
                } else {
                    assert_eq!(result, None);
                }
            }
            count += 1;
        }
    }
    assert_eq!(count, 20);
    assert_eq!(decoder.push(0xff), Some(Err(bm1387::Error::Header)));
    assert_eq!(decoder.push(0x54), None);
    assert_eq!(decoder.push(255), Some(Err(bm1387::Error::Length)));
    for byte in [0x54, 5, 0, 0] {
        assert_eq!(decoder.push(byte), None);
    }
    assert_eq!(decoder.push(0), Some(Err(bm1387::Error::Checksum)));
    assert_eq!(decoder.push(0x54), None);
    decoder.reset();
    let results: Vec<_> = [0x54, 5, 0, 0, 0x19]
        .into_iter()
        .filter_map(|byte| decoder.push(byte))
        .collect();
    assert_eq!(results, [Ok(packet(true, read(0, 0)))]);
}

#[test]
fn profile_and_control_bounds_do_not_silently_change_topology() {
    for bad in [
        Profile {
            chips: 0,
            ..profile()
        },
        Profile {
            chips: 65,
            ..profile()
        },
        Profile {
            cores_per_chip: 129,
            ..profile()
        },
        Profile {
            oscillator_hz: 0,
            ..profile()
        },
        Profile {
            oscillator_hz: 33_333_333,
            ..profile()
        },
        Profile {
            initial_baud_divisor: 27,
            ..profile()
        },
    ] {
        assert!(matches!(Chain::new(bad), Err(Error::Topology)));
    }
    let mut chain = ready();
    assert_eq!(
        chain.receive(&packet(
            false,
            Command::SetAddress {
                address: 0,
                reserved: 0
            }
        )),
        Err(Error::InvalidControl)
    );
    assert_eq!(
        chain.receive(&packet(
            false,
            Command::Inactive {
                address: 0,
                reserved: 0
            }
        )),
        Err(Error::InvalidControl)
    );
}

use md_sim_firmware::tmp421::{Diode, Error, Inputs, Sensor, Timing};

const MS: u64 = 1_000_000;
const START: u64 = 50_000;

fn inputs() -> Inputs {
    Inputs {
        local_millicelsius: 25_500,
        remote_millicelsius: 81_250,
        diode: Diode::Connected,
        supply_valid: true,
    }
}

fn sensor() -> Sensor {
    let mut sensor = Sensor::new(Timing::REFERENCE, inputs()).unwrap();
    sensor.power(0, true).unwrap();
    sensor
}

fn temperature(sensor: &mut Sensor, ns: u64, channel: u8) -> [u8; 2] {
    [
        sensor.read(ns, channel).unwrap(),
        sensor.read(ns, channel + 0x10).unwrap(),
    ]
}

#[test]
fn reset_values_sequential_conversions_and_range_change() {
    let mut s = sensor();
    for (register, expected) in [
        (0, 0),
        (1, 0),
        (8, 0),
        (9, 0),
        (10, 0x1c),
        (11, 7),
        (0xfe, 0x55),
        (0xff, 0x21),
    ] {
        assert_eq!(s.read(0, register), Ok(expected));
    }
    assert_eq!(s.read(START, 8), Ok(0x80));
    assert_eq!(temperature(&mut s, START + 115 * MS - 1, 0), [0, 0]);
    assert_eq!(temperature(&mut s, START + 115 * MS, 0), [25, 0x80]);
    assert_eq!(temperature(&mut s, START + 115 * MS, 1), [0, 0]);
    assert_eq!(s.read(START + 120 * MS, 8), Ok(0));
    assert_eq!(s.read(START + 125 * MS, 8), Ok(0x80));
    assert_eq!(temperature(&mut s, START + 240 * MS, 1), [81, 0x40]);
    // The actual archived driver requires the old value to persist until
    // another ADC conversion completes after this configuration write.
    s.write(START + 245 * MS, 9, 4).unwrap();
    assert_eq!(temperature(&mut s, START + 245 * MS, 0), [25, 0x80]);
    assert_eq!(temperature(&mut s, START + 365 * MS, 0), [89, 0x80]);
    assert_eq!(temperature(&mut s, START + 490 * MS, 1), [145, 0x40]);
}

#[test]
fn adjacent_high_low_reads_are_coherent_across_conversion() {
    let mut s = sensor();
    assert_eq!(s.read(START + 115 * MS, 0), Ok(25));
    s.set_inputs(
        START + 200 * MS,
        Inputs {
            local_millicelsius: 26_250,
            ..inputs()
        },
    )
    .unwrap();
    // The ADC now holds 26.25, but the next low byte belongs to 25.5.
    assert_eq!(s.read(START + 365 * MS, 0x10), Ok(0x80));
    assert_eq!(temperature(&mut s, START + 365 * MS, 0), [26, 0x40]);
    s.read(START + 365 * MS, 0).unwrap();
    s.set_inputs(
        START + 400 * MS,
        Inputs {
            local_millicelsius: 27_750,
            ..inputs()
        },
    )
    .unwrap();
    s.read(START + 615 * MS, 0xfe).unwrap();
    assert_eq!(s.read(START + 615 * MS, 0x10), Ok(0xc0));
}

#[test]
fn shutdown_finishes_sequence_and_one_shot_returns_to_shutdown() {
    let mut s = sensor();
    s.write(START + 1, 9, 0x44).unwrap();
    s.write(START + 2, 0x0f, 99).unwrap(); // Busy one-shot is ignored.
    assert_eq!(temperature(&mut s, START + 240 * MS, 1), [145, 0x40]);
    assert_eq!(s.read(START + 240 * MS, 8), Ok(0));
    s.set_inputs(
        500 * MS,
        Inputs {
            remote_millicelsius: 90_500,
            ..inputs()
        },
    )
    .unwrap();
    assert_eq!(temperature(&mut s, 1_000 * MS, 1), [145, 0x40]);
    s.write(1_000 * MS, 0x0f, 0).unwrap();
    assert_eq!(s.read(1_000 * MS, 8), Ok(0x80));
    assert_eq!(temperature(&mut s, 1_240 * MS, 1), [154, 0x80]);
    assert_eq!(s.read(1_250 * MS, 8), Ok(0));
    assert!(s.next_deadline().is_none());
    s.write(1_500 * MS, 9, 4).unwrap();
    assert_eq!(s.read(1_500 * MS, 8), Ok(0x80));
}

#[test]
fn channel_enable_rate_and_resistance_correction_change_real_deadlines() {
    let mut s = sensor();
    s.write(0, 10, 0x0c).unwrap(); // Local only.
    s.write(0, 11, 4).unwrap(); // One sequence per second.
    assert_eq!(temperature(&mut s, START + 115 * MS, 0), [25, 0x80]);
    assert_eq!(s.next_deadline(), Some(START + 1_000 * MS));
    assert_eq!(temperature(&mut s, START + 999 * MS, 1), [0, 0]);
    s.write(START + 999 * MS, 10, 0x08).unwrap(); // RC off, half-time policy.
    assert_eq!(s.read(START + 1_057 * MS, 8), Ok(0x80));
    assert_eq!(s.read(START + 1_058 * MS, 8), Ok(0));
    s.write(START + 1_058 * MS, 10, 0).unwrap();
    s.advance(START + 2_000 * MS).unwrap();
    assert!(s.next_deadline().is_none());
    s.write(START + 2_000 * MS, 10, 0x14).unwrap(); // Remote only.
    assert_eq!(s.read(START + 2_000 * MS, 8), Ok(0x80));
    assert_eq!(temperature(&mut s, START + 2_115 * MS, 1), [81, 0x40]);
}

#[test]
fn faults_are_sampled_by_conversions_and_reset_clears_state() {
    let mut s = sensor();
    s.write(0, 9, 4).unwrap();
    s.set_inputs(
        START + 150 * MS,
        Inputs {
            diode: Diode::Open,
            supply_valid: false,
            ..inputs()
        },
    )
    .unwrap();
    s.set_inputs(START + 160 * MS, inputs()).unwrap();
    assert_eq!(s.read(START + 240 * MS, 0x11), Ok(0x43));
    s.set_inputs(
        START + 250 * MS,
        Inputs {
            diode: Diode::Short,
            ..inputs()
        },
    )
    .unwrap();
    assert_eq!(temperature(&mut s, START + 490 * MS, 1), [0, 0]);
    s.write(START + 500 * MS, 0xfc, 0x5a).unwrap();
    assert_eq!(temperature(&mut s, START + 500 * MS, 1), [0, 0]);
    assert_eq!(s.read(START + 500 * MS, 9), Ok(0));
    s.power(START + 501 * MS, false).unwrap();
    assert_eq!(s.read(START + 501 * MS, 0xfe), Err(Error::Unpowered));
    s.power(START + 600 * MS, true).unwrap();
    assert_eq!(s.read(START + 600 * MS, 11), Ok(7));
}

#[test]
fn independent_temperature_vectors_cover_negative_fraction_and_n_factor() {
    // Literal values from the documented binary format, not an invocation
    // of the production encoder. The correction case uses T(K)*0.9-273.15.
    for (milli, range, correction, expected) in [
        (-14_250, 0, 0, [0xf1, 0xc0]),
        (-14_250, 4, 0, [0x31, 0xc0]),
        (-64_001, 0, 0, [0xc0, 0]),
        (150_000, 0, 0, [0x7f, 0]),
        (150_000, 4, 0, [0xd6, 0]),
        (191_001, 4, 0, [0xff, 0]),
        (26_850, 4, 30, [0x3c, 0xd0]), // -3.15 C -> -3.1875 C.
    ] {
        let mut s = sensor();
        s.write(0, 9, range).unwrap();
        s.write(0, 0x21, correction).unwrap();
        s.set_inputs(
            0,
            Inputs {
                remote_millicelsius: milli,
                ..inputs()
            },
        )
        .unwrap();
        assert_eq!(temperature(&mut s, START + 240 * MS, 1), expected);
        assert_eq!(s.read(START + 240 * MS, 0x21), Ok(correction));
    }
}

#[test]
fn invalid_operations_do_not_succeed_or_change_configuration() {
    let mut s = sensor();
    for (reg, value) in [(9, 0x80), (10, 0x20), (11, 8)] {
        assert_eq!(s.write(0, reg, value), Err(Error::ReservedBits(reg)));
    }
    assert_eq!(s.read(0, 9), Ok(0));
    assert_eq!(s.read(0, 10), Ok(0x1c));
    assert_eq!(s.read(0, 11), Ok(7));
    assert_eq!(s.write(0, 0xfe, 1), Err(Error::ReadOnly(0xfe)));
    assert_eq!(s.read(0, 0x0f), Err(Error::UndefinedRead(0x0f)));
    assert_eq!(s.read(0, 2), Err(Error::UnknownRegister(2)));
    s.advance(1).unwrap();
    assert_eq!(s.read(0, 0xfe), Err(Error::TimeReversed));
    let mut s = Sensor::new(Timing::REFERENCE, inputs()).unwrap();
    assert_eq!(s.power(u64::MAX, true), Err(Error::TimeOverflow));
}

#[test]
fn advance_chunking_preserves_registers_and_deadlines() {
    let mut large = sensor();
    let mut small = sensor();
    for index in 0..5_000 {
        small.advance(index * MS).unwrap();
    }
    large.advance(4_999 * MS).unwrap();
    for reg in [0, 1, 8, 9, 10, 11, 0x10, 0x11, 0x21, 0xfe, 0xff] {
        assert_eq!(large.read(4_999 * MS, reg), small.read(4_999 * MS, reg));
    }
    assert_eq!(large.next_deadline(), small.next_deadline());
}

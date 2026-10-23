use md_sim_firmware::serial8n1::{Error, Event, Serial};

fn wire(byte: u8) -> [bool; 10] {
    [
        false,
        byte & 1 != 0,
        byte & 2 != 0,
        byte & 4 != 0,
        byte & 8 != 0,
        byte & 16 != 0,
        byte & 32 != 0,
        byte & 64 != 0,
        byte & 128 != 0,
        true,
    ]
}

#[test]
fn receiver_decodes_all_bytes_at_both_bm1387_rates_and_arbitrary_phase() {
    for period in [8640, 640] {
        let mut serial = Serial::new(period, 2).unwrap();
        let mut time = 37;
        for byte in 0..=255 {
            let mut events = Vec::new();
            for (index, high) in wire(byte).into_iter().enumerate() {
                events.extend(serial.input(time + index as u64 * period, high).unwrap());
            }
            events.extend(serial.advance(time + 10 * period).unwrap());
            assert_eq!(
                events,
                [Event::Received {
                    ns: time + 9 * period + period / 2,
                    byte
                }]
            );
            time += 10 * period;
        }
    }
}

#[test]
fn transmitter_matches_independent_bit_waveform_without_polling_drift() {
    let bytes = [0x54, 5, 0, 0, 0x19];
    for period in [8640, 640] {
        let mut expected = Vec::new();
        let mut previous = true;
        for (offset, byte) in bytes.iter().copied().enumerate() {
            for (bit, high) in wire(byte).into_iter().enumerate() {
                if high != previous {
                    expected.push(Event::Level {
                        ns: (offset as u64 * 10 + bit as u64) * period,
                        high,
                    });
                    previous = high;
                }
            }
            expected.push(Event::Transmitted {
                ns: (offset as u64 + 1) * 10 * period,
                byte,
            });
        }
        for step in [1, period / 3, 100 * period] {
            let mut serial = Serial::new(period, bytes.len()).unwrap();
            let mut events = serial.enqueue(&bytes).unwrap();
            let end = 50 * period;
            let mut now = 0;
            while now < end {
                now = (now + step).min(end);
                events.extend(serial.advance(now).unwrap());
            }
            assert_eq!(events, expected);
            assert!(serial.tx_high());
            assert_eq!(serial.queued_bytes(), 0);
            assert_eq!(serial.next_deadline(), None);
        }
    }
}

#[test]
fn false_start_break_reset_rate_change_and_queue_bounds_are_real_state() {
    let mut serial = Serial::new(640, 2).unwrap();
    assert!(serial.input(0, false).unwrap().is_empty());
    assert_eq!(serial.configure(8640), Err(Error::Busy));
    assert!(serial.input(100, true).unwrap().is_empty());
    assert_eq!(
        serial.advance(320).unwrap(),
        [Event::FalseStart { ns: 320 }]
    );
    serial.input(1000, false).unwrap();
    assert_eq!(
        serial.advance(100_000).unwrap(),
        [Event::FramingError { ns: 7080, byte: 0 }]
    );
    assert_eq!(serial.next_deadline(), None);
    serial.advance(100_001).unwrap();
    serial.reset();
    assert!(serial.advance(200_000).unwrap().is_empty());
    assert_eq!(serial.configure(8640), Err(Error::Busy));
    serial.input(200_001, true).unwrap();
    serial.configure(8640).unwrap();
    assert_eq!(serial.bit_period_ns(), 8640);
    serial.enqueue(&[0, 1]).unwrap();
    assert_eq!(serial.enqueue(&[2]), Err(Error::Capacity));
    assert_eq!(serial.queued_bytes(), 2);
    assert_eq!(serial.configure(640), Err(Error::Busy));
    assert_eq!(serial.advance(0), Err(Error::TimeReversed));
    assert_eq!(serial.queued_bytes(), 2);
    serial.advance(200_002).unwrap();
    serial.reset();
    assert!(serial.tx_high());
    assert_eq!(serial.queued_bytes(), 0);
    assert!(serial.advance(300_000).unwrap().is_empty());
    serial.configure(640).unwrap();
    assert_eq!(serial.configure(3), Err(Error::BitPeriod));
    assert_eq!(serial.bit_period_ns(), 640);
}

#[test]
fn invalid_sizes_and_time_overflow_do_not_queue_bytes() {
    assert!(matches!(Serial::new(1, 2), Err(Error::BitPeriod)));
    assert!(matches!(Serial::new(640, 0), Err(Error::Capacity)));
    assert!(matches!(Serial::new(640, 4097), Err(Error::Capacity)));
    let mut serial = Serial::new(640, 2).unwrap();
    serial.advance(u64::MAX - 10).unwrap();
    assert_eq!(serial.enqueue(&[1]), Err(Error::TimeOverflow));
    assert_eq!(serial.queued_bytes(), 0);
    assert_eq!(serial.next_deadline(), None);

    let mut serial = Serial::new(640, 2).unwrap();
    serial.advance(u64::MAX - 6401).unwrap();
    let before = serde_json::to_vec(&serial.snapshot()).unwrap();
    assert_eq!(serial.enqueue(&[1, 2]), Err(Error::TimeOverflow));
    assert_eq!(before, serde_json::to_vec(&serial.snapshot()).unwrap());
    serial.enqueue(&[1]).unwrap();
    assert_eq!(serial.enqueue(&[2]), Err(Error::TimeOverflow));
    assert_eq!(serial.queued_bytes(), 1);
    let events = serial.advance(u64::MAX - 1).unwrap();
    assert_eq!(
        events.last(),
        Some(&Event::Transmitted {
            ns: u64::MAX - 1,
            byte: 1
        })
    );
    assert_eq!(serial.input(u64::MAX - 1, false), Err(Error::TimeOverflow));
    assert!(Serial::restore(serial.snapshot()).is_ok());
}

#[test]
fn saved_partial_frames_and_queues_replay_full_duplex_without_new_edges() {
    for period in [640, 8640] {
        for byte in [0, 0xff, 0xa5, 0x5a] {
            let mut original = Serial::new(period, 3).unwrap();
            let mut restored = Serial::new(period, 3).unwrap();
            let output = [byte, byte ^ 0xff, 0x54];
            assert_eq!(original.enqueue(&output), restored.enqueue(&output));
            let mut events = Vec::new();
            for quarter in 0..=120 {
                let ns = quarter * period / 4;
                let a = original.advance(ns).unwrap();
                let b = restored.advance(ns).unwrap();
                assert_eq!(a, b);
                events.extend(b);
                if quarter % 4 == 0 && quarter < 80 {
                    let index = (quarter / 4) as usize;
                    let high = wire(output[index / 10])[index % 10];
                    assert_eq!(original.input(ns, high), restored.input(ns, high));
                }
                // Serialize at every bit boundary and sampling phase. Pending
                // bytes, half-received bytes, line levels and deadlines survive.
                let bytes = serde_json::to_vec(&restored.snapshot()).unwrap();
                restored = Serial::restore(serde_json::from_slice(&bytes).unwrap()).unwrap();
                assert_eq!(original.next_deadline(), restored.next_deadline());
                assert_eq!(original.tx_high(), restored.tx_high());
                assert_eq!(original.queued_bytes(), restored.queued_bytes());
            }
            let received: Vec<_> = events
                .iter()
                .filter_map(|e| match e {
                    Event::Received { ns, byte } => Some((*ns, *byte)),
                    _ => None,
                })
                .collect();
            assert_eq!(
                received,
                [(19 * period / 2, byte), (39 * period / 2, byte ^ 0xff)]
            );
            let sent: Vec<_> = events
                .iter()
                .filter_map(|e| match e {
                    Event::Transmitted { ns, byte } => Some((*ns, *byte)),
                    _ => None,
                })
                .collect();
            assert_eq!(
                sent,
                [
                    (10 * period, byte),
                    (20 * period, byte ^ 0xff),
                    (30 * period, 0x54)
                ]
            );
        }
    }
    // A break remains a break after restore; reset drops queued output.
    let mut serial = Serial::new(640, 2).unwrap();
    serial.input(0, false).unwrap();
    serial.advance(6400).unwrap();
    serial.enqueue(&[1, 2]).unwrap();
    serial.reset();
    let mut serial = Serial::restore(serial.snapshot()).unwrap();
    assert!(serial.advance(12800).unwrap().is_empty());
    assert_eq!(serial.queued_bytes(), 0);
    assert_eq!(serial.configure(8640), Err(Error::Busy));
    serial.input(12801, true).unwrap();
    serial.configure(8640).unwrap();
}

#[test]
fn incompatible_or_corrupt_wire_state_cannot_schedule_events() {
    use serde_json::json;
    let mut serial = Serial::new(640, 2).unwrap();
    serial.enqueue(&[0x55, 0xaa]).unwrap();
    serial.input(0, false).unwrap();
    let snapshot = serde_json::to_value(serial.snapshot()).unwrap();
    for (key, value, error) in [
        ("version", json!(2), Error::SnapshotVersion),
        ("capacity", json!(0), Error::Capacity),
        ("capacity", json!(4097), Error::Capacity),
        ("pending", json!([1, 2]), Error::Capacity),
        ("bit_ns", json!(3), Error::BitPeriod),
        ("receive_at", json!(0), Error::SnapshotState),
        ("receive_at", json!(321), Error::SnapshotState),
        ("transmit_at", json!(641), Error::SnapshotState),
        ("transmit_at", json!(null), Error::SnapshotState),
        ("tx_high", json!(true), Error::SnapshotState),
        (
            "transmit",
            json!({"byte":0,"index":10}),
            Error::SnapshotState,
        ),
        (
            "receive",
            json!({"Data":{"byte":0,"index":8}}),
            Error::SnapshotState,
        ),
        (
            "receive",
            json!({"Data":{"byte":4,"index":2}}),
            Error::SnapshotState,
        ),
    ] {
        let mut bad = snapshot.clone();
        bad[key] = value;
        assert!(
            matches!(Serial::restore(serde_json::from_value(bad).unwrap()), Err(e) if e == error),
            "{key}"
        );
    }
    let mut unknown = snapshot;
    unknown["future_field"] = json!(0);
    assert!(serde_json::from_value::<md_sim_firmware::serial8n1::Snapshot>(unknown).is_err());

    // Deadlines may fit while the rest of the frame or queue cannot.
    let mut overflow = serde_json::to_value(serial.snapshot()).unwrap();
    overflow["now"] = json!(u64::MAX - 640);
    overflow["receive_at"] = json!(u64::MAX - 320);
    overflow["transmit_at"] = json!(u64::MAX);
    assert!(matches!(
        Serial::restore(serde_json::from_value(overflow).unwrap()),
        Err(Error::SnapshotState)
    ));
}

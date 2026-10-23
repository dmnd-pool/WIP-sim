use md_sim_firmware::s9io::uart::Uart;

fn bits(byte: u8, stop: bool) -> [bool; 10] {
    let mut bits = [false; 10];
    for n in 0..8 {
        bits[n + 1] = byte & (1 << n) != 0;
    }
    bits[9] = stop;
    bits
}

/// Independent8N1 input waveform. No simulator TX code constructs it.
fn send(uart: &mut Uart, divisor: u16, byte: u8, stop: bool) -> Vec<(usize, u8)> {
    let period = 16 * (usize::from(divisor) + 1);
    let mut received = Vec::new();
    for (bit, value) in bits(byte, stop).into_iter().enumerate() {
        for clock in 0..period {
            if let Some(value) = uart.tick(divisor, value, None, false, false).received {
                received.push((bit * period + clock, value));
            }
        }
    }
    received
}

fn settle(uart: &mut Uart, divisor: u16) {
    for _ in 0..32 * (usize::from(divisor) + 1) {
        uart.tick(divisor, true, None, false, false);
    }
}

#[test]
fn receiver_samples_external_8n1_waveforms_for_all_byte_values() {
    for divisor in [0, 1, 13] {
        let mut uart = Uart::default();
        settle(&mut uart, divisor);
        for byte in 0..=255 {
            let received = send(&mut uart, divisor, byte, true);
            assert_eq!(received.len(), 1, "divisor{divisor} byte{byte}");
            assert_eq!(received[0].1, byte);
            assert_eq!(uart.tick(divisor, true, None, true, false).read, Some(byte));
            assert!(uart.rx_empty());
        }
        assert_eq!(uart.errors(), (false, false));
    }
    let mut uart = Uart::default();
    settle(&mut uart, 4095);
    assert_eq!(send(&mut uart, 4095, 0xa5, true)[0].1, 0xa5);
}

#[test]
fn transmitter_emits_exact_bit_periods_without_hash_clock_scaling() {
    for divisor in [0, 2, 13, 127] {
        for byte in [0, 1, 0x80, 0x55, 0xa5, 0xff] {
            let mut uart = Uart::default();
            settle(&mut uart, divisor);
            let width = 16 * (usize::from(divisor) + 1);
            let mut wire = Vec::new();
            let mut started = Vec::new();
            for clock in 0..width * 12 {
                let out = uart.tick(divisor, true, (clock == 0).then_some(byte), false, false);
                wire.push(out.txd);
                if let Some(value) = out.started {
                    started.push((clock, value));
                }
            }
            assert_eq!(started.len(), 1);
            let (start, value) = started[0];
            assert_eq!(value, byte);
            assert!(wire[..=start].iter().all(|bit| *bit));
            for (index, expected) in bits(byte, true).into_iter().enumerate() {
                let begin = start + 1 + index * width;
                assert!(
                    wire[begin..begin + width]
                        .iter()
                        .all(|bit| *bit == expected),
                    "divisor{divisor} byte{byte} bit{index}"
                );
            }
            assert!(wire[start + 1 + 10 * width..].iter().all(|bit| *bit));
        }
    }
}

#[test]
fn glitches_bad_stop_fifo_clear_and_ip_reset_have_distinct_effects() {
    let mut uart = Uart::default();
    settle(&mut uart, 0);
    // A short low pulse fails the half-start-bit check.
    for _ in 0..3 {
        assert_eq!(uart.tick(0, false, None, false, false).received, None);
    }
    settle(&mut uart, 0);
    assert!(uart.rx_empty());
    assert_eq!(uart.errors(), (false, false));
    assert!(send(&mut uart, 0, 0xa5, false).is_empty());
    settle(&mut uart, 0);
    assert_eq!(uart.errors(), (true, false));
    assert!(uart.rx_empty());
    assert_eq!(send(&mut uart, 0, 0x72, true), [(155, 0x72)]);
    uart.tick(0, true, None, false, true);
    assert!(uart.rx_empty());
    assert_eq!(uart.errors(), (true, false));
    uart.reset();
    assert_eq!(uart.errors(), (false, false));
    assert!(uart.txd());
    assert!(uart.rx_empty());
}

#[test]
fn receive_fullness_precedes_same_clock_read_and_sets_sticky_overrun() {
    let mut uart = Uart::default();
    settle(&mut uart, 0);
    for byte in 0..16 {
        assert_eq!(send(&mut uart, 0, byte, true), [(155, byte)]);
    }
    let mut removed = None;
    for (bit, value) in bits(99, true).into_iter().enumerate() {
        for subclock in 0..16 {
            let clock = bit * 16 + subclock;
            let out = uart.tick(0, value, None, clock == 155, false);
            if clock == 155 {
                assert_eq!(out.received, Some(99));
                removed = out.read;
            }
        }
    }
    assert_eq!(removed, Some(0));
    assert_eq!(uart.errors(), (false, true));
    for byte in 1..16 {
        assert_eq!(uart.tick(0, true, None, true, false).read, Some(byte));
    }
    // The incoming99 was discarded even though this edge freed one slot.
    assert_eq!(uart.tick(0, true, None, true, false).read, None);
    uart.tick(0, true, None, false, true);
    assert_eq!(uart.errors(), (false, true));
    uart.reset();
    assert_eq!(uart.errors(), (false, false));
}

#[test]
fn fifo_clear_keeps_inflight_bits_and_baud_change_waits_for_drain() {
    let mut uart = Uart::default();
    settle(&mut uart, 1);
    let mut accepted = Vec::new();
    let mut started = Vec::new();
    for clock in 0..25 {
        let byte = clock as u8;
        let out = uart.tick(1, true, Some(byte), false, false);
        if out.write_accepted {
            accepted.push(byte);
        }
        if let Some(byte) = out.started {
            started.push(byte);
        }
    }
    assert_eq!(accepted, (0..17).collect::<Vec<_>>());
    assert_eq!(started, [0]);
    assert!(uart.tx_full());
    assert!(uart.sync_busy(3));
    let cleared = uart.tick(3, true, None, false, true);
    assert!(!cleared.txd); // Active start bit survives the FIFO-only clear.
    assert!(!uart.tx_full());
    assert!(uart.sync_busy(3));
    for _ in 0..400 {
        let out = uart.tick(3, true, None, false, false);
        assert_eq!(out.started, None);
    }
    assert!(!uart.sync_busy(3));
    assert!(uart.txd());

    // Two queued bytes must retain the old baud rate until BOTH drain.
    uart.reset();
    settle(&mut uart, 1);
    let mut wire = Vec::new();
    let mut starts = Vec::new();
    for clock in 0..800 {
        let out = uart.tick(
            if clock < 8 { 1 } else { 3 },
            true,
            (clock < 2).then_some(0xa5),
            false,
            false,
        );
        wire.push(out.txd);
        if out.started.is_some() {
            starts.push(clock);
        }
        if (8..600).contains(&clock) {
            assert!(uart.sync_busy(3));
        }
    }
    assert_eq!(starts.len(), 2);
    assert_eq!(starts[1] - starts[0], 322);
    for start in starts {
        for (index, bit) in bits(0xa5, true).into_iter().enumerate() {
            assert!(wire[start + 1 + 32 * index..start + 1 + 32 * (index + 1)]
                .iter()
                .all(|actual| *actual == bit));
        }
    }
    assert!(!uart.sync_busy(3));
}

//! Optimization equivalence against ordinary clock-by-clock execution.
//! Literal packet and independent waveform tests remain separate oracles.
use md_sim_firmware::s9io::{
    axi::{Axi, Input, Output},
    registers::*,
};

struct Pair {
    slow: Axi,
    fast: Axi,
    skipped: u64,
    tx_bytes: Vec<u8>,
}
impl Pair {
    fn new() -> Self {
        Self {
            slow: Axi::new(0x12345678),
            fast: Axi::new(0x12345678),
            skipped: 0,
            tx_bytes: Vec::new(),
        }
    }

    fn check(&self) {
        assert_eq!(self.slow.pins(), self.fast.pins());
        assert_eq!(self.slow.interrupts(), self.fast.interrupts());
        assert_eq!(self.slow.txd(), self.fast.txd());
    }

    fn tick(&mut self, input: Input) -> Output {
        let slow = self.slow.tick(input).unwrap();
        let fast = self.fast.tick(input).unwrap();
        assert_eq!(slow, fast);
        if let Some(byte) = fast.cycle.as_ref().and_then(|c| c.device.uart.started) {
            self.tx_bytes.push(byte);
        }
        self.check();
        fast
    }

    fn advance(&mut self, mut clocks: u64, rxd: bool) {
        while clocks != 0 {
            let limit = self.fast.idle_limit(rxd);
            let pins = self.fast.pins();
            assert_eq!(self.fast.idle_limit(rxd), limit);
            assert_eq!(self.fast.pins(), pins);
            let skipped = self.fast.skip_idle(clocks, rxd);
            assert_eq!(skipped, clocks.min(limit));
            if skipped == 0 {
                self.tick(Input {
                    rxd,
                    ..Input::default()
                });
                clocks -= 1;
            } else {
                self.skipped += skipped;
                let interrupts = self.slow.interrupts();
                let txd = self.slow.txd();
                for _ in 0..skipped {
                    let output = self
                        .slow
                        .tick(Input {
                            rxd,
                            ..Input::default()
                        })
                        .unwrap();
                    let cycle = output.cycle.unwrap();
                    // An internal work timer ACK is allowed only when it
                    // causes no reset/clear, queue, byte, register or pin
                    // change. All other outputs must remain inactive.
                    assert_eq!(
                        cycle.device.core.tx,
                        md_sim_firmware::s9io::transmit::Output {
                            work_time_ack: cycle.device.core.tx.work_time_ack,
                            ..Default::default()
                        }
                    );
                    assert_eq!(cycle.device.core.rx, Default::default());
                    assert_eq!(cycle.device.uart.txd, txd);
                    assert!(!cycle.device.uart.write_accepted);
                    assert!(cycle.device.uart.started.is_none());
                    assert!(cycle.device.uart.received.is_none());
                    assert!(cycle.device.uart.read.is_none());
                    assert!(cycle.read.is_none() && cycle.write.is_none());
                    assert_eq!(self.slow.interrupts(), interrupts);
                }
                clocks -= skipped;
                self.check();
            }
        }
    }

    fn write(&mut self, offset: u16, value: u32) {
        let input = Input {
            awvalid: true,
            awaddr: offset,
            wvalid: true,
            wdata: value,
            wstrb: 15,
            ..Input::default()
        };
        self.tick(input);
        self.tick(input);
        self.tick(Input {
            bready: true,
            ..Input::default()
        });
    }

    fn read(&mut self, offset: u16) -> Result<u32, BusError> {
        let input = Input {
            arvalid: true,
            araddr: offset,
            ..Input::default()
        };
        self.tick(input);
        let value = self.tick(input).pins.rdata;
        self.tick(Input {
            rready: true,
            ..Input::default()
        });
        value
    }
}

#[test]
fn enabled_empty_controller_can_advance_a_guest_sleep() {
    let mut registers = Registers::new(0x12345678);
    registers.write(CONTROL, 8, 15).unwrap();
    for _ in 0..10 {
        registers.tick(true).unwrap();
    }
    // Default WORK_TIME=1 must not require fifty million callbacks for a
    // one-second guest sleep. No clocks, register state or pin edges change.
    assert_eq!(registers.idle_limit(true), u64::MAX);
    assert_eq!(registers.skip_idle(50_000_000, true), 50_000_000);
    assert_eq!(registers.read(WORK_TIME), Ok(1));
    assert_eq!(registers.read(CONTROL), Ok(8));
    assert_eq!(registers.read(COMMAND_STATUS), Ok(5));
    assert_eq!(registers.interrupts(), 0);
    assert!(registers.txd());
}

#[test]
fn empty_check_phase_survives_timer_changes_and_pending_resets() {
    for divisor in [0, 1, 13] {
        for previous_reload in [0, 1, 2, 3, 7] {
            for reload in [0, 1, 2, 3, 8, 0x0100_0001] {
                for phase in 0..9 {
                    let mut pair = Pair::new();
                    pair.write(BAUD, divisor);
                    pair.write(WORK_TIME, previous_reload);
                    pair.write(CONTROL, 8);
                    pair.advance(phase, true);
                    pair.write(WORK_TIME, reload);
                    for clocks in [0, 1, 2, 3, 4, 31, 256] {
                        pair.advance(clocks, true);
                        // Compare the next complete edge, including timer
                        // ACK and RX/TX state outputs, at each boundary.
                        pair.tick(Input::default());
                    }
                    pair.write(COMMAND_CONTROL, 7);
                    pair.write(WORK_RX_CONTROL, 7);
                    pair.write(WORK_TX_CONTROL, 7);
                    pair.advance(33, true);
                    for offset in [COMMAND_CONTROL, WORK_RX_CONTROL, WORK_TX_CONTROL] {
                        pair.read(offset).unwrap();
                    }
                    // A reload write does not restart the live counter.
                    // Reset before requiring a short reset-ACK deadline.
                    pair.tick(Input {
                        reset: true,
                        ..Input::default()
                    });
                    pair.write(CONTROL, 8);
                    pair.write(COMMAND_CONTROL, 7);
                    pair.write(WORK_RX_CONTROL, 7);
                    pair.write(WORK_TX_CONTROL, 7);
                    pair.advance(33, true);
                    assert_eq!(pair.read(COMMAND_CONTROL), Ok(4));
                    assert_eq!(pair.read(WORK_RX_CONTROL), Ok(4));
                    assert_eq!(pair.read(WORK_TX_CONTROL), Ok(4));
                    pair.write(COMMAND_TX, 0x00000554);
                    pair.advance(1500, true);
                }
            }
        }
    }
}

#[test]
fn full_u64_empty_spans_preserve_ack_uart_and_command_phase() {
    for reload in [1u64, 2, 3, 7] {
        let mut pair = Pair::new();
        pair.write(BAUD, 1);
        pair.write(WORK_TIME, reload as u32);
        pair.write(CONTROL, 8);
        pair.advance(100, true);
        assert_eq!(pair.fast.skip_idle(u64::MAX, true), u64::MAX);
        // A multiple of both the two-clock UART and the empty TX cycle
        // returns to this state. Include one full period after the residue.
        let period = 2 * reload.max(2);
        for _ in 0..u64::MAX % period + period {
            pair.slow.tick(Input::default()).unwrap();
        }
        pair.check();
        pair.tick(Input::default());
        pair.write(COMMAND_TX, 0x00000554);
        pair.advance(4000, true);
    }
}

#[test]
fn long_empty_timer_periods_preserve_wrap_and_reload_phase() {
    for reload in [0u32, 0x00ff_ffff] {
        let mut pair = Pair::new();
        pair.write(CONTROL, 8);
        pair.advance(32, true);
        pair.write(WORK_TIME, reload);
        pair.advance(32, true);
        let period = if reload == 0 {
            1u64 << 24
        } else {
            u64::from(reload)
        };
        // Stay below u64::MAX while advancing many complete24-bit timer
        // cycles and seventeen extra clocks. The UART divisor is zero.
        let clocks = (u64::MAX / period - 1) * period + 17;
        assert_eq!(pair.fast.skip_idle(clocks, true), clocks);
        for _ in 0..17 {
            pair.slow.tick(Input::default()).unwrap();
        }
        pair.tick(Input::default());
        pair.write(WORK_TIME, 1);
        pair.advance(1000, true);
        pair.read(COMMAND_STATUS).unwrap();
    }
}

#[test]
fn incomplete_work_keeps_words_and_timer_phase_until_the_last_word_arrives() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/s9io-reference.json")).unwrap();
    for (task, control) in [
        ("tc_send_work_midstate1", 8),
        ("tc_send_work_midstate4", 12),
    ] {
        let task = &reference["tasks"][task];
        let words: Vec<u32> = serde_json::from_value(task["fifo_data1"].clone()).unwrap();
        let expected: Vec<u8> = serde_json::from_value(task["uart_data1"].clone()).unwrap();
        for reload in [1, 2, 7, 257] {
            let mut pair = Pair::new();
            pair.write(BAUD, 1);
            pair.write(WORK_TIME, reload);
            pair.write(CONTROL, control);
            pair.advance(1000, true);
            for word in &words[..words.len() - 1] {
                pair.write(WORK_TX, *word);
                let before = pair.skipped;
                pair.advance(4096, true);
                assert!(
                    pair.tx_bytes.is_empty(),
                    "incomplete work must not transmit"
                );
                assert!(
                    pair.skipped - before > 4000,
                    "incomplete work caused clock polling"
                );
                pair.tick(Input::default());
            }
            pair.write(WORK_TX, *words.last().unwrap());
            pair.advance((expected.len() as u64 + 3) * 400, true);
            assert_eq!(pair.tx_bytes, expected);
            pair.read(LAST_WORK_ID).unwrap();
            pair.read(WORK_TX_STATUS).unwrap();
        }
    }
}

#[test]
fn incomplete_command_waits_without_clock_polling_then_sends_the_archived_packet() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/s9io-reference.json")).unwrap();
    let task = &reference["tasks"]["tc_send_cmd9"];
    for index in 1..=11 {
        let words: Vec<u32> =
            serde_json::from_value(task[format!("fifo_data{index}")].clone()).unwrap();
        let expected: Vec<u8> =
            serde_json::from_value(task[format!("uart_data{index}")].clone()).unwrap();
        assert_eq!(words.len(), 2);
        for reload in [1, 2, 7, 257] {
            let mut pair = Pair::new();
            pair.write(BAUD, 1);
            pair.write(WORK_TIME, reload);
            pair.write(CONTROL, 8);
            pair.advance(1000, true);
            pair.write(COMMAND_TX, words[0]);
            pair.advance(1000, true);
            let before = pair.skipped;
            pair.advance(4096, true);
            assert!(pair.tx_bytes.is_empty());
            assert!(
                pair.skipped - before > 4000,
                "incomplete command caused clock polling"
            );
            pair.tick(Input::default());
            pair.write(COMMAND_TX, words[1]);
            pair.advance(5000, true);
            assert_eq!(pair.tx_bytes, expected);
            pair.read(COMMAND_STATUS).unwrap();
        }
    }
}

#[test]
fn incomplete_work_keeps_phase_across_u64_spans_and_obeys_reset_and_mode_changes() {
    let mut pair = Pair::new();
    pair.write(BAUD, 1);
    pair.write(WORK_TIME, 2);
    pair.write(CONTROL, 12);
    pair.advance(100, true);
    for _ in 0..12 {
        pair.write(WORK_TX, 0);
    }
    pair.advance(100, true);
    assert!(pair.tx_bytes.is_empty());
    assert_eq!(pair.fast.skip_idle(u64::MAX, true), u64::MAX);
    // UART and polling phases both have period2. MAX and31 are odd.
    for _ in 0..31 {
        pair.slow.tick(Input::default()).unwrap();
    }
    pair.check();
    // A mode write makes the retained12 words into a complete one-midstate
    // packet. It must stop the polling skip at the next actual TX event.
    pair.write(CONTROL, 8);
    pair.advance(24000, true);
    assert_eq!(pair.tx_bytes.len(), 54);
    assert_eq!(&pair.tx_bytes[..4], &[0x21, 54, 0, 1]);
    pair.write(WORK_TX, 0x1234);
    pair.advance(100, true);
    pair.write(WORK_TX_CONTROL, 3);
    pair.advance(100, true);
    assert_eq!(pair.read(WORK_TX_CONTROL), Ok(0));
    assert_eq!(pair.read(WORK_TX_STATUS).unwrap() & 4, 4);
    assert_eq!(pair.tx_bytes.len(), 54);
    pair.tick(Input::default());
}

#[test]
fn timer_baud_and_enabled_transitions_match_every_clock() {
    for divisor in [0, 1, 13, 4095] {
        for reload in [0, 1, 2, 11, 0x00ff_ffff] {
            let mut pair = Pair::new();
            pair.write(BAUD, divisor);
            pair.write(WORK_TIME, reload);
            for clocks in [0, 1, 2, 17, 4097, 16384] {
                pair.advance(clocks, true);
            }
            // Disabled free-running timer must retain phase and due state.
            pair.write(CONTROL, 8);
            pair.write(COMMAND_CONTROL, 7);
            pair.write(WORK_RX_CONTROL, 7);
            pair.write(WORK_TX_CONTROL, 7);
            pair.advance(25000, true);
            for offset in [
                CONTROL,
                COMMAND_CONTROL,
                WORK_RX_CONTROL,
                WORK_TX_CONTROL,
                COMMAND_STATUS,
                WORK_RX_STATUS,
                WORK_TX_STATUS,
                LAST_WORK_ID,
                ERRORS,
            ] {
                pair.read(offset).unwrap();
            }
            // Shrinking the divisor while its counter exceeds the new value
            // must take the source's wrap path, not reduce phase modulo it.
            pair.write(BAUD, 0);
            pair.advance(5000, true);
            pair.write(CONTROL, 9);
            pair.advance(32, true);
            assert_eq!(pair.read(CONTROL), Ok(8));
            pair.write(CONTROL, 0);
            pair.advance(1000, true);
            pair.tick(Input {
                reset: true,
                ..Input::default()
            });
            pair.advance(1000, true);
            assert!(pair.skipped > 20000);
        }
    }
}

#[test]
fn serial_packets_queued_commands_and_live_irqs_survive_idle_advancement() {
    for divisor in [0u32, 1, 7, 13, 107] {
        let mut pair = Pair::new();
        pair.write(BAUD, divisor);
        pair.write(WORK_TIME, 257);
        pair.write(CONTROL, 8);
        pair.write(COMMAND_CONTROL, 4);
        pair.advance(10000, true);
        // Literal archived register query. Reply is independently driven
        // through RXD while the transmitter is also active.
        pair.write(COMMAND_TX, 0x00000554);
        let before_serial = pair.skipped;
        for byte in [0x13u8, 0x87, 0x90, 0, 0, 0, 7] {
            for bit in 0..10 {
                let level = match bit {
                    0 => false,
                    9 => true,
                    _ => byte & (1 << (bit - 1)) != 0,
                };
                pair.advance(u64::from(16 * (divisor + 1)), level);
            }
        }
        if divisor >= 7 {
            // Verify the active transfer benefits, independently of the
            // long idle spans before and after it. Every skipped clock
            // above is compared with the ordinary hardware state machine.
            let serial_clocks = 7 * 10 * 16 * u64::from(divisor + 1);
            assert!(pair.skipped - before_serial > serial_clocks * 9 / 10);
        }
        pair.advance(20000, true);
        assert_eq!(pair.fast.interrupts(), 4);
        assert_eq!(pair.read(COMMAND_RX), Ok(0x00908713));
        pair.advance(20000, true);
        assert_eq!(pair.read(COMMAND_RX), Ok(0x07000000));
        assert!(pair.skipped > 1000);
    }
}

#[test]
fn active_serial_skips_preserve_bad_stops_breaks_and_recovery() {
    let mut pair = Pair::new();
    pair.write(BAUD, 107);
    pair.write(CONTROL, 8);
    pair.write(COMMAND_CONTROL, 4);
    pair.advance(3000, true);
    // Low stop bits and a long BREAK must not become valid packets.
    pair.advance(16 * 108 * 30, false);
    // Release long enough for a frame that started during BREAK to end.
    pair.advance(16 * 108 * 12, true);
    assert_eq!(pair.read(COMMAND_STATUS).unwrap() & 1, 1);
    assert_eq!(pair.fast.interrupts(), 0);
    for byte in [0x13u8, 0x87, 0x90, 0, 0, 0, 7] {
        for bit in 0..10 {
            let level = match bit {
                0 => false,
                9 => true,
                _ => byte & (1 << (bit - 1)) != 0,
            };
            pair.advance(16 * 108, level);
        }
    }
    pair.advance(10000, true);
    assert_eq!(pair.read(COMMAND_RX), Ok(0x00908713));
    assert_eq!(pair.read(COMMAND_RX), Ok(0x07000000));
}

#[test]
fn full_uart_fifo_stalls_keep_every_work_and_command_byte_and_timer_phase() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/s9io-reference.json")).unwrap();
    let task = &reference["tasks"]["tc_send_work_midstate4"];
    let mut expected: Vec<u8> = serde_json::from_value(task["uart_data1"].clone()).unwrap();
    expected.extend([0x54, 5, 0, 0, 0x19].repeat(2));
    for divisor in [0u32, 1, 7, 13, 107] {
        let mut pair = Pair::new();
        pair.write(BAUD, divisor);
        pair.write(WORK_TIME, 257);
        pair.write(CONTROL, 12);
        pair.advance(10000, true);
        for word in task["fifo_data1"].as_array().unwrap() {
            pair.write(WORK_TX, word.as_u64().unwrap() as u32);
        }
        pair.write(COMMAND_TX, 0x00000554);
        pair.write(COMMAND_TX, 0x00000554);
        let before = pair.skipped;
        let clocks = (expected.len() as u64 + 3) * 161 * u64::from(divisor + 1);
        pair.advance(clocks, true);
        assert_eq!(pair.tx_bytes, expected);
        if divisor >= 7 {
            assert!(pair.skipped - before > clocks * 9 / 10);
        }
        pair.tick(Input::default());
        for offset in [LAST_WORK_ID, WORK_TX_STATUS, COMMAND_STATUS, ERRORS] {
            pair.read(offset).unwrap();
        }
    }
}

#[test]
fn pending_bus_channels_and_low_serial_input_cannot_be_skipped() {
    let mut pair = Pair::new();
    assert_eq!(pair.fast.skip_idle(100, false), 0);
    let write = Input {
        awvalid: true,
        awaddr: BAUD,
        wvalid: true,
        wdata: 13,
        wstrb: 15,
        ..Input::default()
    };
    pair.tick(write);
    assert_eq!(pair.fast.skip_idle(100, true), 0);
    pair.tick(write);
    assert_eq!(pair.fast.skip_idle(100, true), 0);
    pair.tick(Input {
        bready: true,
        ..Input::default()
    });
    let read = Input {
        arvalid: true,
        araddr: BAUD,
        ..Input::default()
    };
    pair.tick(read);
    assert_eq!(pair.fast.skip_idle(100, true), 0);
    pair.tick(read);
    assert_eq!(pair.fast.skip_idle(100, true), 0);
    pair.tick(Input {
        rready: true,
        ..Input::default()
    });
    pair.advance(1000, true);
    assert!(pair.skipped > 900);
}

#[test]
fn full_u64_spans_preserve_disabled_timer_and_uart_phase() {
    // Both clocks have period two after these writes. An odd positive span
    // has the same final phase as31 ordinary clocks, even across u64 limits.
    let mut pair = Pair::new();
    pair.write(BAUD, 1);
    pair.write(WORK_TIME, 2);
    pair.advance(16, true);
    assert_eq!(pair.fast.skip_idle(u64::MAX, true), u64::MAX);
    for _ in 0..31 {
        pair.slow.tick(Input::default()).unwrap();
    }
    pair.check();
    pair.write(CONTROL, 8);
    pair.write(COMMAND_TX, 0x00000554);
    pair.advance(10000, true);
}

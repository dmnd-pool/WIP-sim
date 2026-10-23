use md_sim_firmware::s9io::{
    transmit::{Control, Ready, TransmitFifos, COMMAND_WORDS, WORK_WORDS},
    Error, Midstates,
};
use serde_json::Value;

const READY: Ready = Ready {
    uart: true,
    crc5: true,
    crc16: true,
    uart_sync_busy: false,
};

fn control(midstates: Midstates) -> Control {
    Control {
        enabled: true,
        midstates,
        work_time: 50,
        reset_command_tx: false,
        reset_command_rx: false,
        reset_work_tx: false,
        reset_work_rx: false,
    }
}

fn reference() -> Value {
    let r: Value = serde_json::from_str(include_str!("fixtures/s9io-reference.json")).unwrap();
    assert_eq!(
        r["member_sha256"],
        "8bd72eecd9e5706fdcbdd756f9b8084eaffabf0e72a1624ce09c11a11b973889"
    );
    r
}

fn words(value: &Value) -> Vec<u32> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| u32::try_from(v.as_u64().unwrap()).unwrap())
        .collect()
}

#[test]
fn transmitted_ids_feed_real_receive_fifo_extension_vectors() {
    use md_sim_firmware::s9io::receive::{Control as RxControl, ReceiveFifos};
    let r = reference();
    for index in 1..=5 {
        let task = &r["tasks"][format!("tc_work_id_{index}")];
        let ctl = control(if index == 5 {
            Midstates::Four
        } else {
            Midstates::One
        });
        let mut tx = TransmitFifos::default();
        let mut rx = ReceiveFifos::default();
        tx.tick(ctl, READY).unwrap();
        let rx_ctl = RxControl {
            enabled: true,
            ..RxControl::default()
        };
        rx.tick(rx_ctl, None, 0, ctl.midstates);
        for word in words(&task["fifo_data1"]) {
            assert!(tx.write_work(word));
        }
        // Each accepted byte occupies eight subsequent serial-CRC clocks.
        let wire: Vec<_> = (0..2000)
            .filter_map(|_| tx.tick(ctl, READY).unwrap().byte.map(u32::from))
            .collect();
        assert_eq!(wire, words(&task["uart_data1"]));
        // Captured HDL testbench responses are regression inputs only.
        // This test makes no ASIC calculation or fresh-mining claim.
        for byte in words(&task["uart_data2"]) {
            assert!((0..100).any(|_| rx
                .tick(rx_ctl, Some(byte as u8), tx.last_work_id(), ctl.midstates)
                .read_uart));
        }
        for _ in 0..57 {
            rx.tick(rx_ctl, None, tx.last_work_id(), ctl.midstates);
        }
        assert_eq!(rx.work_status(), 16);
        let response = [rx.read_work().unwrap(), rx.read_work().unwrap()];
        assert_eq!(response.as_slice(), words(&task["fifo_data2"]));
        assert_eq!(rx.work_status(), 1);
    }
}

#[test]
fn two_midstates_use_twenty_words_and_an_independent_checksum() {
    let mut tx = TransmitFifos::default();
    let ctl = control(Midstates::Two);
    tx.tick(ctl, READY).unwrap();
    for _ in 0..19 {
        tx.write_work(0);
    }
    for _ in 0..100 {
        assert_eq!(tx.tick(ctl, READY).unwrap().byte, None);
    }
    tx.write_work(0);
    let actual: Vec<_> = (0..1000)
        .filter_map(|_| tx.tick(ctl, READY).unwrap().byte)
        .collect();
    let mut expected = vec![0x21, 0x56, 0, 2];
    expected.extend([0; 80]);
    // Independent Python3 binascii.crc_hqx(header +80 zero bytes,0xffff).
    expected.extend([0x08, 0xcc]);
    assert_eq!(actual, expected);
    assert_eq!(tx.work_status(0), 20);
}

#[test]
fn hdl_transmit_vectors_survive_independent_uart_and_crc_stalls() {
    let r = reference();
    for (name, count, mode) in [
        ("tc_send_cmd5", 9, Midstates::One),
        ("tc_send_cmd9", 11, Midstates::One),
        ("tc_send_work_midstate1", 1, Midstates::One),
        ("tc_send_work_midstate4", 1, Midstates::Four),
    ] {
        for index in 1..=count {
            let task = &r["tasks"][name];
            let input = words(&task[format!("fifo_data{index}")]);
            let expected = words(&task[format!("uart_data{index}")])
                .into_iter()
                .map(|v| u8::try_from(v).unwrap())
                .collect::<Vec<_>>();
            let mut tx = TransmitFifos::default();
            let ctl = control(mode);
            assert!(tx.tick(ctl, READY).unwrap().clear_uart);
            let work = name.contains("work");
            for word in &input {
                assert!(if work {
                    tx.write_work(*word)
                } else {
                    tx.write_command(*word)
                });
            }
            let mut observed = Vec::new();
            for clock in 0..3000 {
                let ready = Ready {
                    uart: clock % 3 == 0,
                    crc5: clock % 7 != 0,
                    crc16: clock % 5 != 0,
                    uart_sync_busy: clock < 5,
                };
                let before = tx.last_work_id();
                let output = tx.tick(ctl, ready).unwrap();
                if let Some(byte) = output.byte {
                    assert!(ready.uart && if work { ready.crc16 } else { ready.crc5 });
                    observed.push(byte);
                } else {
                    assert_eq!(before, tx.last_work_id());
                }
                if work && observed.len() < 3 {
                    assert_eq!(tx.last_work_id(), 0);
                }
            }
            assert_eq!(observed, expected, "{name} vector{index}");
            assert_eq!(tx.command_status(), 4);
            assert_eq!(tx.work_status(6), 20);
            assert_eq!(tx.last_work_id(), if work { input[0] as u16 } else { 0 });
        }
    }
}

#[test]
fn complete_work_has_priority_then_one_command_without_another_sync() {
    let r = reference();
    let work = words(&r["tasks"]["tc_send_work_midstate1"]["fifo_data1"]);
    let expected_work = words(&r["tasks"]["tc_send_work_midstate1"]["uart_data1"]);
    let command = words(&r["tasks"]["tc_send_cmd5"]["fifo_data1"]);
    let expected_command = words(&r["tasks"]["tc_send_cmd5"]["uart_data1"]);
    let mut tx = TransmitFifos::default();
    let mut ctl = control(Midstates::One);
    ctl.work_time = 1000;
    tx.tick(ctl, READY).unwrap();
    for _ in 0..2 {
        for &word in &work {
            assert!(tx.write_work(word));
        }
    }
    for &word in &command {
        assert!(tx.write_command(word));
        assert!(tx.write_command(word));
    }
    let mut bytes = Vec::new();
    let mut acks = Vec::new();
    // The work and one command fit before the next1000-clock sync, including
    // their actual serial checksum calculation. The assertions stay exact.
    for clock in 1..900 {
        let output = tx.tick(ctl, READY).unwrap();
        if output.work_time_ack {
            acks.push(clock);
        }
        if let Some(byte) = output.byte {
            bytes.push(u32::from(byte));
        }
    }
    assert_eq!(acks, [1]);
    assert_eq!(
        bytes,
        expected_work
            .into_iter()
            .chain(expected_command)
            .collect::<Vec<_>>()
    );
    // Neither the next work nor the second command can bypass the timer.
    assert_eq!(tx.command_status(), 0);
    assert_eq!(tx.work_status(6), 0);
}

#[test]
fn partial_command_stalls_until_its_next_word_and_reset_waits_for_sync() {
    let r = reference();
    let input = words(&r["tasks"]["tc_send_cmd9"]["fifo_data1"]);
    let expected = words(&r["tasks"]["tc_send_cmd9"]["uart_data1"]);
    let mut tx = TransmitFifos::default();
    let mut ctl = control(Midstates::One);
    tx.tick(ctl, READY).unwrap();
    tx.write_command(input[0]);
    for _ in 0..80 {
        assert_eq!(tx.tick(ctl, READY).unwrap().byte, None);
    }
    // The first word is held in the synchronous read register; FIFO is empty.
    assert_eq!(tx.command_status(), 4);
    ctl.reset_command_tx = true;
    for _ in 0..80 {
        assert_eq!(tx.tick(ctl, READY).unwrap().byte, None);
    }
    assert!(tx.write_command(input[1]));
    let mut observed = Vec::new();
    for _ in 0..100 {
        if let Some(byte) = tx.tick(ctl, READY).unwrap().byte {
            observed.push(u32::from(byte));
        }
    }
    assert_eq!(observed, expected);
    assert_eq!(tx.command_status(), 4);

    // A partial work cannot block a complete control command.
    ctl.reset_command_tx = false;
    tx.write_work(0x1234);
    tx.write_command(0x00000554);
    let mut command = Vec::new();
    for _ in 0..100 {
        if let Some(byte) = tx.tick(ctl, READY).unwrap().byte {
            command.push(byte);
        }
    }
    // Literal tc_send_cmd_after_work vector, including independent CRC.
    assert_eq!(command, [0x54, 0x05, 0, 0, 0x19]);
    assert_eq!(tx.last_work_id(), 0);
}

#[test]
fn command_bytes_wait_for_eight_checksum_shift_clocks() {
    let mut tx = TransmitFifos::default();
    let ctl = control(Midstates::One);
    tx.tick(ctl, READY).unwrap();
    tx.write_command(0x554);
    let mut observed = Vec::new();
    for clock in 1..60 {
        if let Some(byte) = tx.tick(ctl, READY).unwrap().byte {
            observed.push((clock, byte));
        }
    }
    // Source st_cmd_read/check precede the first byte. Eight CRC shifts
    // follow every accepted data byte; the ninth edge can accept the next.
    assert_eq!(observed, [(5, 0x54), (14, 5), (23, 0), (32, 0), (41, 0x19)]);
}

#[test]
fn fifo_capacity_threshold_wrap_and_enable_reset_match_hdl() {
    let mut tx = TransmitFifos::default();
    let ctl = control(Midstates::One);
    tx.tick(ctl, READY).unwrap();
    for n in 0..COMMAND_WORDS {
        assert!(tx.write_command(n as u32));
    }
    assert!(!tx.write_command(0xabcdef01));
    assert_eq!(tx.command_status(), 8);
    for n in 0..WORK_WORDS {
        assert!(tx.write_work(n as u32));
        if n < 6 {
            assert_eq!(tx.work_irq(6, true), n < 5);
        }
    }
    assert!(!tx.write_work(0xabcdef01));
    assert_eq!(tx.work_status(6), 24); // Full pointer wrap also asserts IRQ.
    assert_eq!(tx.work_status(0), 8);
    assert!(!tx.work_irq(6, false));
    let mut disabled = ctl;
    disabled.enabled = false;
    tx.tick(disabled, READY).unwrap();
    assert_eq!(tx.command_status(), 8);
    assert_eq!(tx.work_status(6), 24);
    let output = tx.tick(ctl, READY).unwrap();
    assert!(output.clear_command_rx && output.clear_work_rx && output.clear_receive_errors);
    assert!(output.clear_uart);
    assert_eq!(tx.command_status(), 4);
    assert_eq!(tx.work_status(0), 20);
}

#[test]
fn work_timer_runs_while_disabled_wraps_at24_bits_and_waits_for_uart_sync() {
    let mut tx = TransmitFifos::default();
    let mut ctl = control(Midstates::One);
    ctl.work_time = 10;
    let mut acks = Vec::new();
    for clock in 0..35 {
        if tx.tick(ctl, READY).unwrap().work_time_ack {
            acks.push(clock);
        }
    }
    assert_eq!(acks, [1, 11, 21, 31]);
    tx.reset();
    ctl.enabled = false;
    ctl.work_time = 0;
    for _ in 0..100 {
        assert!(!tx.tick(ctl, READY).unwrap().work_time_ack);
    }
    ctl.enabled = true;
    let busy = Ready {
        uart_sync_busy: true,
        ..READY
    };
    tx.tick(ctl, busy).unwrap();
    for _ in 0..100 {
        assert!(!tx.tick(ctl, busy).unwrap().work_time_ack);
    }
    assert!(tx.tick(ctl, READY).unwrap().work_time_ack);
    for _ in 0..100 {
        assert!(!tx.tick(ctl, READY).unwrap().work_time_ack);
    }
    tx.reset();
    ctl.work_time = 0xff00_000a;
    let acks = (0..35)
        .filter(|_| tx.tick(ctl, READY).unwrap().work_time_ack)
        .count();
    assert_eq!(acks, 4);
}

#[test]
fn id_changes_at_its_wire_byte_and_unknown_command_length_is_visible() {
    let r = reference();
    let mut work = words(&r["tasks"]["tc_send_work_midstate1"]["fifo_data1"]);
    work[0] = 0x1234abcd;
    let mut tx = TransmitFifos::default();
    let ctl = control(Midstates::One);
    tx.tick(ctl, READY).unwrap();
    for word in work {
        tx.write_work(word);
    }
    let mut bytes = Vec::new();
    while bytes.len() < 3 {
        assert_eq!(tx.last_work_id(), 0);
        if let Some(byte) = tx.tick(ctl, READY).unwrap().byte {
            bytes.push(byte);
        }
    }
    assert_eq!(bytes, [0x21, 54, 0x4d]);
    assert_eq!(tx.last_work_id(), 0xabcd);
    let disabled = Control {
        enabled: false,
        ..ctl
    };
    tx.tick(disabled, READY).unwrap();
    tx.tick(ctl, READY).unwrap();
    assert_eq!(tx.last_work_id(), 0xabcd);
    tx.write_command(0x00000d51);
    assert!((0..200).any(|_| tx.tick(ctl, READY) == Err(Error::UnsupportedCommandLength)));
    tx.reset();
    assert_eq!(tx.last_work_id(), 0);
}

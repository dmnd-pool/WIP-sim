use md_sim_firmware::s9io::registers::*;

fn configured(divisor: u16, midstates: u32) -> Registers {
    let mut device = Registers::new(0x12345678);
    device.write(BAUD, u32::from(divisor), 15).unwrap();
    device.write(CONTROL, 8 | midstates, 15).unwrap();
    device.write(COMMAND_CONTROL, 4, 15).unwrap();
    device.write(WORK_RX_CONTROL, 4, 15).unwrap();
    for _ in 0..64 * (u32::from(divisor) + 1) {
        device.tick(true).unwrap();
    }
    device
}

fn input(device: &mut Registers, bytes: &[u8], divisor: u16) {
    // Independent8N1 wire generator. It does not call the model transmitter.
    for &byte in bytes {
        for bit in 0..10 {
            let level = match bit {
                0 => false,
                9 => true,
                _ => byte & (1 << (bit - 1)) != 0,
            };
            for _ in 0..16 * (u32::from(divisor) + 1) {
                device.tick(level).unwrap();
            }
        }
    }
    for _ in 0..80 {
        device.tick(true).unwrap();
    }
}

/// Read pin samples with a separate fixed-baud decoder. No internal TX
/// event, FIFO output, CRC function or UART receiver is used as the oracle.
fn decode(pins: &[bool], divisor: u16) -> Vec<u8> {
    let width = 16 * (usize::from(divisor) + 1);
    let mut index = 1;
    let mut bytes = Vec::new();
    while index + 10 * width <= pins.len() {
        if pins[index - 1] && !pins[index] {
            assert!(!pins[index + width / 2]);
            let mut byte = 0;
            for bit in 0..8 {
                byte |= u8::from(pins[index + width * (bit + 1) + width / 2]) << bit;
            }
            assert!(pins[index + width * 9 + width / 2]);
            bytes.push(byte);
            index += 10 * width;
        } else {
            index += 1;
        }
    }
    bytes
}

#[test]
fn all_archived_transmit_vectors_cross_registers_crc_fifo_and_serial_pins() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/s9io-reference.json")).unwrap();
    for (task, mode, work, count) in [
        ("tc_send_cmd5", 0, false, 9),
        ("tc_send_cmd9", 0, false, 11),
        ("tc_send_work_midstate1", 0, true, 1),
        ("tc_send_work_midstate4", 4, true, 1),
    ] {
        for index in 1..=count {
            for divisor in [0, 13] {
                let arrays = &reference["tasks"][task];
                let mut device = configured(divisor, mode);
                let expected: Vec<_> = arrays[format!("uart_data{index}")]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect();
                for word in arrays[format!("fifo_data{index}")].as_array().unwrap() {
                    device
                        .write(
                            if work { WORK_TX } else { COMMAND_TX },
                            word.as_u64().unwrap() as u32,
                            15,
                        )
                        .unwrap();
                }
                let clocks = (expected.len() + 2) * 161 * (usize::from(divisor) + 1);
                let pins: Vec<_> = (0..clocks)
                    .map(|_| device.tick(true).unwrap().uart.txd)
                    .collect();
                assert_eq!(decode(&pins, divisor), expected, "{task}/{index}/{divisor}");
                assert_eq!(device.uart_errors(), (false, false));
                assert_eq!(device.read(ERRORS), Ok(0));
            }
        }
    }
}

#[test]
fn archived_receive_vectors_cross_serial_pins_crc_irq_and_register_reads() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/s9io-reference.json")).unwrap();
    for (task, port, irq) in [
        ("tc_cmd_response", COMMAND_RX, 4),
        ("tc_work_response", WORK_RX, 2),
    ] {
        for (key, value) in reference["tasks"][task].as_object().unwrap() {
            if !key.starts_with("uart") {
                continue;
            }
            for divisor in [0, 2, 13] {
                let mut device = configured(divisor, 0);
                // Establish last ID0x7f through real work transmission.
                device.write(WORK_TX, 0x7f, 15).unwrap();
                for _ in 1..12 {
                    device.write(WORK_TX, 0, 15).unwrap();
                }
                for _ in 0..60 * 161 * (usize::from(divisor) + 1) {
                    device.tick(true).unwrap();
                }
                assert_eq!(device.read(LAST_WORK_ID), Ok(0x7f));
                let bytes: Vec<_> = value
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_u64().unwrap() as u8)
                    .collect();
                input(&mut device, &bytes, divisor);
                assert_eq!(device.interrupts(), irq);
                for word in reference["tasks"][task][key.replace("uart", "fifo")]
                    .as_array()
                    .unwrap()
                {
                    assert_eq!(device.read(port), Ok(word.as_u64().unwrap() as u32));
                    assert_eq!(device.interrupts(), 0);
                }
                assert_eq!(device.read(ERRORS), Ok(0));
                assert_eq!(device.uart_errors(), (false, false));
            }
        }
    }
}

#[test]
fn independent_uart_and_packet_errors_survive_fifo_clear_but_reset_clears_them() {
    let mut device = configured(0, 0);
    // All-zero character with a low stop bit is dropped by the UART.
    for _ in 0..160 {
        device.tick(false).unwrap();
    }
    for _ in 0..80 {
        device.tick(true).unwrap();
    }
    assert_eq!(device.uart_errors(), (true, false));
    assert_eq!(device.read(ERRORS), Ok(0));
    input(&mut device, &[0; 7], 0);
    assert_eq!(device.read(ERRORS), Ok(1));
    device.write(CONTROL, 0, 15).unwrap();
    device.tick(true).unwrap();
    device.write(CONTROL, 8, 15).unwrap();
    assert!(device.tick(true).unwrap().core.tx.clear_uart);
    assert_eq!(device.uart_errors(), (true, false));
    assert_eq!(device.read(ERRORS), Ok(0));
    input(&mut device, &[0x13, 0x87, 0x90, 0, 0, 0, 7], 0);
    assert_eq!(device.read(COMMAND_RX), Ok(0x00908713));
    device.reset();
    assert_eq!(device.uart_errors(), (false, false));
    assert_eq!(device.read(ERRORS), Ok(0));
    assert_eq!(device.read(COMMAND_RX), Ok(0x00908713));
    assert_eq!(device.read(COMMAND_STATUS), Ok(5));
}

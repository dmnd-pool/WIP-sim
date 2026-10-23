use md_sim_firmware::s9io::{
    registers::*,
    transmit::{Ready, COMMAND_WORDS},
    Midstates,
};

const READY: Ready = Ready {
    uart: true,
    crc5: true,
    crc16: true,
    uart_sync_busy: false,
};

fn enabled() -> Registers {
    let mut device = Registers::new(0x76543210);
    device.write(CONTROL, 8, 15).unwrap();
    assert!(device.tick_core(READY, None).unwrap().tx.clear_uart);
    device
}

fn feed(device: &mut Registers, bytes: &[u8]) {
    for &byte in bytes {
        assert!((0..100).any(|_| device.tick_core(READY, Some(byte)).unwrap().rx.read_uart));
    }
    // Six serial checksum bytes, one check, then two FIFO write clocks.
    for _ in 0..57 {
        device.tick_core(READY, None).unwrap();
    }
}

#[test]
fn register_map_retains_hdl_version_conflict_and_byte_strobes() {
    let mut device = Registers::new(0x76543210);
    // These are direct HDL offsets/defaults, not values copied from SVD.
    for (offset, value) in [
        (0x0000, 0x00900200),
        (0x0004, 0x76543210),
        (0x0008, 0),
        (0x000c, 0),
        (0x0010, 0),
        (0x0014, 1),
        (0x0018, 0),
        (0x100c, 5),
        (0x200c, 1),
        (0x300c, 20),
        (0x3010, 0),
        (0x3014, 0),
        (0x3ffc, 0),
    ] {
        assert_eq!(device.read(offset), Ok(value));
    }
    assert_eq!(device.write(VERSION, 0x00090002, 15), Ok(Write::Ignored));
    assert_eq!(device.read(VERSION), Ok(0x00900200));
    assert_eq!(device.read(0x4000), Err(BusError::OutsideWindow));
    assert_eq!(device.write(0x4000, 0, 15), Err(BusError::OutsideWindow));
    device.write(BAUD, 0x12345678, 15).unwrap();
    device.write(BAUD, 0xaabbccdd, 5).unwrap();
    assert_eq!(device.read(BAUD), Ok(0x12bb56dd));
    assert_eq!(device.uart_divisor(), 0x6dd);
    device.write(BAUD, u32::MAX, 0).unwrap();
    assert_eq!(device.read(BAUD), Ok(0x12bb56dd));
    assert_eq!(device.read(BAUD + 3), Ok(0x12bb56dd));
    device.write(BAUD + 1, 0x8700, 2).unwrap();
    assert_eq!(device.read(BAUD), Ok(0x12bb87dd));
    for (bits, expected) in [
        (0, Midstates::One),
        (2, Midstates::Two),
        (4, Midstates::Four),
        (6, Midstates::Four),
    ] {
        device.write(CONTROL, 0xffff0000 | bits, 15).unwrap();
        assert_eq!(device.midstates(), expected);
    }
    device.reset();
    assert_eq!(device.read(BUILD_ID), Ok(0x76543210));
    assert_eq!(device.read(BAUD), Ok(0));
    assert_eq!(device.read(WORK_TIME), Ok(1));
}

#[test]
fn fifo_strobes_zero_unselected_lanes_and_full_writes_are_dropped() {
    let mut device = enabled();
    // tc_send_cmd_after_work from the archived HDL. Upper unselected bytes
    // are zero, not old register contents or ones from the current bus word.
    device.write(COMMAND_TX, 0xffff0554, 3).unwrap();
    let bytes: Vec<_> = (0..50)
        .filter_map(|_| device.tick_core(READY, None).unwrap().tx.byte)
        .collect();
    assert_eq!(bytes, [0x54, 0x05, 0, 0, 0x19]);
    assert_eq!(device.read(COMMAND_STATUS), Ok(5));
    // Even zero strobes still cause a FIFO write. Keep the transmitter idle
    // while exercising the hardware capacity and bus acknowledgement.
    for _ in 0..COMMAND_WORDS {
        assert_eq!(
            device.write(COMMAND_TX, u32::MAX, 0),
            Ok(Write::Fifo { accepted: true })
        );
    }
    assert_eq!(device.read(COMMAND_STATUS), Ok(9));
    assert_eq!(
        device.write(COMMAND_TX, 0x554, 15),
        Ok(Write::Fifo { accepted: false })
    );
}

#[test]
fn receive_read_strobes_clear_irq_and_empty_reads_retain_last_output() {
    let mut device = enabled();
    assert_eq!(device.read(COMMAND_RX), Err(BusError::UninitializedRead));
    assert_eq!(device.read(WORK_RX), Err(BusError::UninitializedRead));
    device.write(COMMAND_CONTROL, 4, 15).unwrap();
    device.write(WORK_RX_CONTROL, 4, 15).unwrap();
    device.write(WORK_TX_CONTROL, 4, 15).unwrap();
    assert_eq!(device.interrupts(), 1);
    // Original tc_cmd_response first vector and its two literal FIFO words.
    feed(&mut device, &[0x13, 0x87, 0x90, 0, 0, 0, 7]);
    assert_eq!(device.interrupts(), 5);
    assert_eq!(device.read(COMMAND_STATUS), Ok(20));
    assert_eq!(device.read(COMMAND_RX), Ok(0x00908713));
    assert_eq!(device.interrupts(), 1);
    assert_eq!(device.read(COMMAND_RX), Ok(0x07000000));
    assert_eq!(device.read(COMMAND_STATUS), Ok(5));
    assert_eq!(device.read(COMMAND_RX), Ok(0x07000000));
    device.reset();
    assert_eq!(device.read(COMMAND_RX), Ok(0x07000000));
    assert_eq!(device.read(WORK_RX), Err(BusError::UninitializedRead));
    assert_eq!(device.interrupts(), 0);
}

#[test]
fn reset_requests_wait_for_sync_and_preserve_irq_enable() {
    let mut device = enabled();
    device.write(WORK_TIME, 50, 15).unwrap();
    feed(&mut device, &[0x13, 0x87, 0x90, 0, 0, 0, 7]);
    device.write(COMMAND_CONTROL, 0xabcd0007, 15).unwrap();
    device.write(WORK_RX_CONTROL, 7, 15).unwrap();
    device.write(WORK_TX_CONTROL, 7, 15).unwrap();
    device.write(COMMAND_TX, 0x554, 15).unwrap();
    device.write(WORK_TX, 0x1234, 15).unwrap();
    let busy = Ready {
        uart_sync_busy: true,
        ..READY
    };
    for _ in 0..80 {
        assert!(!device.tick_core(busy, None).unwrap().tx.work_time_ack);
    }
    assert_eq!(device.read(COMMAND_CONTROL), Ok(0xabcd0007));
    assert_eq!(device.read(COMMAND_STATUS), Ok(16));
    let output = device.tick_core(READY, None).unwrap().tx;
    assert!(output.work_time_ack && output.clear_command_rx && output.clear_work_rx);
    assert_eq!(device.read(COMMAND_CONTROL), Ok(0xabcd0004));
    assert_eq!(device.read(WORK_RX_CONTROL), Ok(4));
    assert_eq!(device.read(WORK_TX_CONTROL), Ok(4));
    assert_eq!(device.read(COMMAND_STATUS), Ok(5));
    assert_eq!(device.read(WORK_RX_STATUS), Ok(1));
    assert_eq!(device.read(WORK_TX_STATUS), Ok(20));
    assert_eq!(device.interrupts(), 1);
}

#[test]
fn errors_clear_once_and_last_id_is_from_transmitted_work() {
    let mut device = enabled();
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/s9io-reference.json")).unwrap();
    let task = &reference["tasks"]["tc_work_id_2"];
    for value in task["fifo_data1"].as_array().unwrap() {
        device
            .write(WORK_TX, value.as_u64().unwrap() as u32, 15)
            .unwrap();
    }
    // Include eight serial-CRC shift clocks after each work data byte.
    let bytes: Vec<_> = (0..600)
        .filter_map(|_| device.tick_core(READY, None).unwrap().tx.byte)
        .collect();
    let expected: Vec<_> = task["uart_data1"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect();
    assert_eq!(bytes, expected);
    assert_eq!(device.read(LAST_WORK_ID), Ok(0xcdef));
    let response: Vec<_> = task["uart_data2"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect();
    feed(&mut device, &response);
    for value in task["fifo_data2"].as_array().unwrap() {
        assert_eq!(device.read(WORK_RX), Ok(value.as_u64().unwrap() as u32));
    }
    feed(&mut device, &[0; 7]);
    assert_eq!(device.read(ERRORS), Ok(1));
    device.write(CONTROL, 9, 15).unwrap();
    assert_eq!(device.read(CONTROL), Ok(9));
    device.tick_core(READY, None).unwrap();
    assert_eq!(device.read(ERRORS), Ok(0));
    assert_eq!(device.read(CONTROL), Ok(8));
    // Clearing errors does not flush the sliding receive window.
    feed(&mut device, &[0]);
    assert_eq!(device.read(ERRORS), Ok(1));
    device.tick_core(READY, None).unwrap();
    assert_eq!(device.read(ERRORS), Ok(1));
}

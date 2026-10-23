use md_sim_firmware::s9io::{
    axi::{Axi, Input},
    registers::*,
    transmit::COMMAND_WORDS,
};

fn write(bus: &mut Axi, address: u16, value: u32, strobes: u8) {
    let input = Input {
        awvalid: true,
        awaddr: address,
        wvalid: true,
        wdata: value,
        wstrb: strobes,
        ..Input::default()
    };
    let first = bus.tick(input).unwrap();
    assert!(first.pins.awready && first.pins.wready);
    assert!(!first.pins.bvalid);
    assert!(first.cycle.unwrap().write.is_none());
    let second = bus.tick(input).unwrap();
    assert!(!second.pins.awready && !second.pins.wready);
    assert!(second.pins.bvalid);
    assert_eq!(second.pins.bresp, 0);
    assert!(second.cycle.unwrap().write.unwrap().is_ok());
    assert!(
        !bus.tick(Input {
            bready: true,
            ..Input::default()
        })
        .unwrap()
        .pins
        .bvalid
    );
}

fn read(bus: &mut Axi, address: u16) -> Result<u32, BusError> {
    let input = Input {
        arvalid: true,
        araddr: address,
        ..Input::default()
    };
    let first = bus.tick(input).unwrap();
    assert!(first.pins.arready && !first.pins.rvalid);
    assert!(first.cycle.unwrap().read.is_none());
    let second = bus.tick(input).unwrap();
    assert!(!second.pins.arready && second.pins.rvalid);
    assert_eq!(second.pins.rresp, 0);
    let value = second.pins.rdata;
    assert!(
        !bus.tick(Input {
            rready: true,
            ..Input::default()
        })
        .unwrap()
        .pins
        .rvalid
    );
    value
}

#[test]
fn registered_handshakes_byte_strobes_aliases_and_reset_match_wrapper() {
    let mut bus = Axi::new(0x12345678);
    assert_eq!(read(&mut bus, VERSION), Ok(0x00900200));
    assert_eq!(read(&mut bus, BUILD_ID), Ok(0x12345678));
    write(&mut bus, BAUD, 0x12345678, 15);
    write(&mut bus, BAUD, 0xaabbccdd, 5);
    assert_eq!(read(&mut bus, BAUD), Ok(0x12bb56dd));
    // AXI wrapper decodes only address bits13:2 within its64KiB region.
    assert_eq!(read(&mut bus, 0xc013), Ok(0x12bb56dd));
    write(&mut bus, 0x4012, 0x8700, 2);
    assert_eq!(read(&mut bus, BAUD), Ok(0x12bb87dd));
    assert_eq!(read(&mut bus, COMMAND_RX), Err(BusError::UninitializedRead));
    let out = bus
        .tick(Input {
            reset: true,
            awvalid: true,
            wvalid: true,
            arvalid: true,
            ..Input::default()
        })
        .unwrap();
    assert!(out.cycle.is_none());
    assert!(
        !out.pins.awready
            && !out.pins.wready
            && !out.pins.bvalid
            && !out.pins.arready
            && !out.pins.rvalid
    );
    assert_eq!(out.pins.rdata, Ok(0));
    assert_eq!(read(&mut bus, BAUD), Ok(0));
    assert_eq!(read(&mut bus, WORK_TIME), Ok(1));
    assert_eq!(read(&mut bus, BUILD_ID), Ok(0x12345678));
}

#[test]
fn write_address_and_data_must_be_valid_together() {
    let mut bus = Axi::new(0);
    for _ in 0..5 {
        let pins = bus
            .tick(Input {
                awvalid: true,
                awaddr: BAUD,
                ..Input::default()
            })
            .unwrap()
            .pins;
        assert!(!pins.awready && !pins.wready && !pins.bvalid);
    }
    for _ in 0..5 {
        let pins = bus
            .tick(Input {
                wvalid: true,
                wdata: 0x1234,
                wstrb: 15,
                ..Input::default()
            })
            .unwrap()
            .pins;
        assert!(!pins.awready && !pins.wready && !pins.bvalid);
    }
    assert_eq!(read(&mut bus, BAUD), Ok(0));
    write(&mut bus, BAUD, 0x4321, 15);
    assert_eq!(read(&mut bus, BAUD), Ok(0x4321));
}

#[test]
fn archived_pending_response_defects_remain_visible() {
    let mut bus = Axi::new(0);
    let input = Input {
        awvalid: true,
        awaddr: BAUD,
        wvalid: true,
        wdata: 13,
        wstrb: 15,
        ..Input::default()
    };
    bus.tick(input).unwrap();
    assert!(bus.tick(input).unwrap().pins.bvalid);
    // No BREADY. Source aw_en never becomes0, so another write is accepted.
    let next = Input { wdata: 26, ..input };
    let first = bus.tick(next).unwrap();
    assert!(first.pins.bvalid && first.pins.awready);
    let second = bus.tick(next).unwrap();
    assert!(second.pins.bvalid && second.cycle.unwrap().write.is_some());
    for _ in 0..5 {
        assert!(bus.tick(Input::default()).unwrap().pins.bvalid);
    }
    bus.tick(Input {
        bready: true,
        ..Input::default()
    })
    .unwrap();
    assert!(!bus.pins().bvalid);
    assert_eq!(read(&mut bus, BAUD), Ok(26));

    let request = Input {
        arvalid: true,
        araddr: BAUD,
        ..Input::default()
    };
    bus.tick(request).unwrap();
    assert_eq!(bus.tick(request).unwrap().pins.rdata, Ok(26));
    assert!(bus.pins().rvalid);
    // No RREADY. New ARADDR changes the live synchronous-FIFO mux even
    // though the old response is still pending and no FIFO read occurs.
    let next = Input {
        araddr: COMMAND_RX,
        ..request
    };
    let out = bus.tick(next).unwrap();
    assert!(out.pins.arready && out.pins.rvalid);
    assert_eq!(out.pins.rdata, Err(BusError::UninitializedRead));
    assert!(out.cycle.unwrap().read.is_none());
    assert!(bus.tick(next).unwrap().cycle.unwrap().read.is_none());
    bus.tick(Input {
        rready: true,
        ..Input::default()
    })
    .unwrap();
    assert!(!bus.pins().rvalid);
}

#[test]
fn concurrent_fifo_read_and_write_obey_preclock_fullness_and_clear_priority() {
    let mut device = Registers::new(0);
    device.write(CONTROL, 8, 15).unwrap();
    device.tick(true).unwrap();
    for _ in 0..COMMAND_WORDS {
        device.write(COMMAND_TX, 0x554, 15).unwrap();
    }
    device.tick(true).unwrap(); // Wait -> Check
    device.tick(true).unwrap(); // Check -> CommandRead
    assert_eq!(device.read(COMMAND_STATUS), Ok(9));
    // CommandRead frees one word, but the simultaneous CPU write saw FULL.
    let cycle = device
        .cycle(true, None, Some((COMMAND_TX, 0x554, 15)))
        .unwrap();
    assert_eq!(cycle.write, Some(Ok(Write::Fifo { accepted: false })));
    assert_eq!(
        device.write(COMMAND_TX, 0x554, 15),
        Ok(Write::Fifo { accepted: true })
    );
    assert_eq!(device.read(COMMAND_STATUS), Ok(9));
    assert_eq!(
        device.write(COMMAND_TX, 0x554, 15),
        Ok(Write::Fifo { accepted: false })
    );

    device.reset();
    device.write(CONTROL, 8, 15).unwrap();
    // Enable's FIFO clear wins over this simultaneous new word.
    let cycle = device
        .cycle(true, None, Some((COMMAND_TX, 0x554, 15)))
        .unwrap();
    assert!(cycle.device.core.tx.clear_command_tx);
    assert_eq!(cycle.write, Some(Ok(Write::Fifo { accepted: false })));
    assert_eq!(device.read(COMMAND_STATUS), Ok(5));
}

#[test]
fn simultaneous_control_write_wins_over_acknowledgement_and_error_clear() {
    let mut device = Registers::new(0);
    // The bus write is sampled after the disabled core's clock.
    let cycle = device.cycle(true, None, Some((CONTROL, 9, 15))).unwrap();
    assert!(!cycle.device.core.tx.clear_uart);
    assert_eq!(device.read(CONTROL), Ok(9));
    let cycle = device.cycle(true, None, Some((CONTROL, 9, 15))).unwrap();
    assert!(cycle.device.core.tx.clear_uart);
    assert_eq!(device.read(CONTROL), Ok(9)); // bus write wins automatic bit0clear
    device.write(COMMAND_CONTROL, 7, 15).unwrap();
    let cycle = device
        .cycle(true, None, Some((COMMAND_CONTROL, 7, 15)))
        .unwrap();
    assert!(cycle.device.core.tx.work_time_ack);
    assert_eq!(device.read(COMMAND_CONTROL), Ok(7)); // re-request survives ACK
    assert_eq!(device.read(CONTROL), Ok(8));
}

#[test]
fn serial_reply_reaches_synchronous_axi_read_output_and_irq() {
    let mut bus = Axi::new(0);
    write(&mut bus, CONTROL, 8, 15);
    write(&mut bus, COMMAND_CONTROL, 4, 15);
    for byte in [0x13u8, 0x87, 0x90, 0, 0, 0, 7] {
        for bit in 0..10 {
            let level = match bit {
                0 => false,
                9 => true,
                _ => byte & (1 << (bit - 1)) != 0,
            };
            for _ in 0..16 {
                bus.tick(Input {
                    rxd: level,
                    ..Input::default()
                })
                .unwrap();
            }
        }
    }
    for _ in 0..80 {
        bus.tick(Input::default()).unwrap();
    }
    assert_eq!(bus.interrupts(), 4);
    assert_eq!(read(&mut bus, COMMAND_STATUS), Ok(20));
    let input = Input {
        arvalid: true,
        araddr: COMMAND_RX,
        ..Input::default()
    };
    bus.tick(input).unwrap();
    let output = bus.tick(input).unwrap();
    assert_eq!(output.pins.rdata, Ok(0x00908713));
    assert_eq!(bus.interrupts(), 0);
    for _ in 0..20 {
        let pins = bus.tick(Input::default()).unwrap().pins;
        assert!(pins.rvalid);
        assert_eq!(pins.rdata, Ok(0x00908713));
    }
    bus.tick(Input {
        rready: true,
        ..Input::default()
    })
    .unwrap();
    assert_eq!(read(&mut bus, COMMAND_RX), Ok(0x07000000));
    assert_eq!(read(&mut bus, COMMAND_RX), Ok(0x07000000));
    bus.tick(Input {
        reset: true,
        ..Input::default()
    })
    .unwrap();
    assert_eq!(read(&mut bus, COMMAND_RX), Ok(0x07000000));
}

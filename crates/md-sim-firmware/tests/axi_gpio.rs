use md_sim_firmware::axi_gpio::{Direction, Gpio};

#[test]
fn settled_input_pins_ignore_output_writes_and_survive_peripheral_reset() {
    let mut gpio = Gpio::new(9, Direction::Input, 0, u32::MAX, 0x1e0).unwrap();
    assert_eq!(gpio.read(0), Ok(0x1e0));
    assert_eq!(gpio.read(4), Ok(0x1ff));
    for strobe in 0..=15 {
        gpio.write(0, u32::MAX, strobe).unwrap();
        gpio.write(4, 0, strobe).unwrap();
        assert_eq!(gpio.read(0), Ok(0x1e0));
        assert_eq!(gpio.outputs(), 0);
    }
    gpio.set_settled_inputs(0x1234);
    assert_eq!(gpio.read(0), Ok(0x34));
    gpio.reset();
    assert_eq!(gpio.read(0), Ok(0x34));
    assert_eq!(gpio.read(4), Ok(0x1ff));
}

#[test]
fn output_pins_use_wdata_even_with_zero_strobe_and_read_back_zero() {
    let mut gpio = Gpio::new(13, Direction::Output, 0x1234, u32::MAX, u32::MAX).unwrap();
    assert_eq!(gpio.outputs(), 0x1234);
    assert_eq!(gpio.read(0), Ok(0));
    for strobe in 0..=15 {
        gpio.write(0, 0xf5a5_0000 | (u32::from(strobe) * 0x111), strobe)
            .unwrap();
        assert_eq!(gpio.outputs(), u32::from(strobe) * 0x111);
        assert_eq!(gpio.read(0), Ok(0));
    }
    gpio.write(4, 0xaaaa_5555, 0).unwrap();
    assert_eq!(gpio.read(4), Ok(0x1555));
    gpio.reset();
    assert_eq!(gpio.outputs(), 0x1234);
    assert_eq!(gpio.read(4), Ok(0x1fff));
}

#[test]
fn absent_channel_interrupts_and_reserved_registers_cannot_change_pins() {
    let mut gpio = Gpio::new(32, Direction::Output, 0x1234_5678, 0, 0).unwrap();
    for offset in (8..0x200).step_by(4) {
        gpio.write(offset, u32::MAX, 15).unwrap();
        assert_eq!(gpio.read(offset), Ok(0));
        assert_eq!(gpio.outputs(), 0x1234_5678);
        assert_eq!(gpio.tristate_register(), 0);
    }
    for offset in [1, 2, 3, 0x1ff, 0x200, 0xfffc, 0xffff] {
        assert!(gpio.write(offset, 0, 15).is_err());
        assert!(gpio.read(offset).is_err());
        assert_eq!(gpio.outputs(), 0x1234_5678);
    }
    assert!(Gpio::new(0, Direction::Input, 0, 0, 0).is_none());
    assert!(Gpio::new(33, Direction::Input, 0, 0, 0).is_none());
    let mut one = Gpio::new(1, Direction::Output, 0, 0, 0).unwrap();
    one.write(0, u32::MAX, 15).unwrap();
    assert_eq!(one.outputs(), 1);
}

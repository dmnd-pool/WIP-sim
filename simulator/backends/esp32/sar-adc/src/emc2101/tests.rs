use super::*;

fn sensor() -> Sensor {
    Sensor::new(3_300_000, false, 27 * 8, Diode::Temperature(25 * 8), 0).unwrap()
}
fn write(s: &mut Sensor, reg: u8, value: u8, now: u64) -> bool {
    assert!(s.start(false, now));
    assert!(s.write(reg, now));
    let accepted = s.write(value, now);
    s.stop();
    accepted
}
fn read(s: &mut Sensor, reg: u8, now: u64) -> u8 {
    assert!(s.start(false, now));
    assert!(s.write(reg, now));
    assert!(s.start(true, now));
    let result = s.read(now).unwrap();
    s.nack();
    s.stop();
    result
}
#[test]
fn power_up_aliases_and_host_straps() {
    let mut s = sensor();
    assert!(!s.start(false, 9_999_999));
    assert_eq!(read(&mut s, 0xfd, 10_000_000), 0x28);
    assert_eq!(read(&mut s, 0xfe, 10_000_000), 0x5d);
    assert_eq!(read(&mut s, 0xff, 10_000_000), 1);
    assert_eq!(read(&mut s, 0xa, 10_000_000), 8);
    assert!(write(&mut s, 9, 4, 10_000_000));
    assert_eq!(read(&mut s, 3, 10_000_000), 4);
    assert!(write(&mut s, 0xb, 42, 10_000_000));
    assert_eq!(read(&mut s, 5, 10_000_000), 42);
    assert_eq!(s.fan_setting(10_000_000), 0);
    let mut full = Sensor::new(3_300_000, true, 216, Diode::Open, 0).unwrap();
    assert_eq!(full.fan_setting(10_000_000), 63);
    assert_eq!(read(&mut full, 0x4a, 10_000_000), 0x30);
    assert!(Sensor::new(2_900_000, false, 216, Diode::Open, 0).is_none());
}
#[test]
fn delayed_real_input_conversion_and_read_interlock() {
    let mut s = sensor();
    assert!(s.set_inputs(216, Diode::Temperature(-83), 0)); // -10.375 C
    assert_eq!(read(&mut s, 0, 12_999_999), 0);
    assert_eq!(read(&mut s, 0, 13_000_000), 27);
    assert_eq!(read(&mut s, 1, 33_999_999), 0);
    assert_eq!(read(&mut s, 1, 34_000_000), 0xf5);
    assert_eq!(read(&mut s, 0x10, 34_000_000), 0xa0);
    assert!(s.set_inputs(216, Diode::Temperature(203), 35_000_000)); // 25.375 C
    s.advance(96_500_000);
    // Read of low retains the previous high-byte snapshot across conversions.
    assert_eq!(read(&mut s, 0x10, 96_500_000), 0xa0);
    assert_eq!(read(&mut s, 1, 96_500_000), 25);
    assert_eq!(read(&mut s, 0x10, 96_500_000), 0x60);
    assert_eq!(s.conversions(), 2);
    assert_eq!(s.faults(), 0);
}
#[test]
fn datasheet_saturation_distinguishes_temperature_from_short_fault() {
    for (input, high, low) in [
        (Diode::Temperature(-800), 0xc0, 0),
        (Diode::Temperature(1023), 0x7f, 0xc0),
        (Diode::Short, 0x7f, 0xe0),
    ] {
        let mut s = Sensor::new(3_300_000, false, 216, input, 0).unwrap();
        assert_eq!(read(&mut s, 1, 34_000_000), high);
        assert_eq!(read(&mut s, 0x10, 34_000_000), low);
        assert_eq!(s.faults(), 0);
    }
}
#[test]
fn open_short_standby_one_shot_and_thermal_limit() {
    let mut s = Sensor::new(3_300_000, false, 216, Diode::Open, 0).unwrap();
    assert_eq!(read(&mut s, 1, 34_000_000), 127);
    assert_eq!(read(&mut s, 0x10, 34_000_000), 0);
    assert_eq!(read(&mut s, 2, 34_000_000) & 0x16, 0x16);
    assert_eq!(s.fan_setting(34_000_000), 63);
    assert!(write(&mut s, 3, 0x40, 35_000_000));
    assert!(s.set_inputs(216, Diode::Short, 36_000_000));
    s.advance(100_000_000);
    assert_eq!(s.conversions(), 1);
    assert!(write(&mut s, 0xf, 0xff, 100_000_000));
    assert_eq!(read(&mut s, 1, 123_999_999), 127);
    assert_eq!(read(&mut s, 0x10, 123_999_999), 0);
    assert_eq!(read(&mut s, 1, 124_000_000), 127);
    assert_eq!(read(&mut s, 0x10, 124_000_000), 0xe0);
    assert_eq!(read(&mut s, 2, 124_000_000) & 4, 0);
    assert_eq!(s.conversions(), 2);
    assert_eq!(s.next_deadline(), NEVER);
    assert_eq!(s.fan_setting(124_000_000), 63); // mixed mode retains critical drive
    assert!(write(&mut s, 3, 0x60, 125_000_000));
    assert_eq!(s.fan_setting(125_000_000), 0); // full standby uses strap default
    assert!(write(&mut s, 3, 0x40, 126_000_000));
    assert_eq!(s.fan_setting(126_000_000), 63);
}
#[test]
fn tachometer_requires_five_actual_edges_and_latches_high_byte() {
    let mut s = sensor();
    assert!(write(&mut s, 3, 4, 10_000_000));
    assert!(write(&mut s, 0x4a, 0x2b, 10_000_000));
    assert!(write(&mut s, 0x4c, 40, 10_000_000));
    assert_eq!(read(&mut s, 0x46, 10_000_000), 0xff);
    assert_eq!(read(&mut s, 0x47, 10_000_000), 0xff);
    // Five edges in 20ms span one 3000 RPM revolution: 90kHz * .020 = 1800.
    for (index, time) in [20, 25, 30, 35].into_iter().enumerate() {
        s.tach_level(index % 2 == 0, time * 1_000_000);
    }
    assert_eq!(read(&mut s, 0x46, 35_000_000), 0xff);
    s.tach_level(true, 40_000_000);
    assert_eq!(read(&mut s, 0x46, 40_000_000), 0x08);
    for (index, time) in [50, 60, 70, 80].into_iter().enumerate() {
        s.tach_level(index % 2 != 0, time * 1_000_000);
    }
    assert_eq!(read(&mut s, 0x47, 80_000_000), 0x07); // first low latched 0x0708
    assert_eq!(read(&mut s, 0x46, 80_000_000), 0x10);
    assert_eq!(read(&mut s, 0x47, 80_000_000), 0x0e); // 1500 RPM -> 0x0e10
    s.advance(808_166_667);
    assert_eq!(read(&mut s, 0x46, 808_166_667), 0xff);
    assert_eq!(read(&mut s, 0x47, 808_166_667), 0xff);
    assert_eq!(s.faults(), 0);
}
#[test]
fn volatile_registers_spinup_and_missing_modes_are_visible() {
    let mut s = sensor();
    assert!(write(&mut s, 3, 4, 10_000_000));
    assert!(write(&mut s, 0x4b, 0x19, 10_000_000)); // 100%, 50ms, no fast TACH
    assert!(write(&mut s, 0x4c, 20, 10_000_000));
    assert_eq!(s.fan_setting(59_999_999), 63);
    assert_eq!(s.fan_setting(60_000_000), 20);
    assert!(!write(&mut s, 0x4a, 0, 60_000_000));
    assert_eq!(read(&mut s, 0x4a, 60_000_000), 0x20);
    assert!(!write(&mut s, 0xbf, 6, 60_000_000));
    assert!(!write(&mut s, 0xfd, 0, 60_000_000));
    assert_eq!(read(&mut s, 0xfd, 60_000_000), 0x28);
    assert_eq!(s.faults(), 3);
    assert_eq!(read(&mut s, 0x4c, 60_000_000), 20);
    s.stop();
    assert_eq!(read(&mut s, 0x4c, 60_000_000), 20);
    s.power_reset(60_000_000);
    assert!(!s.start(false, 69_999_999));
    assert_eq!(read(&mut s, 0x4c, 70_000_000), 0);
}
#[test]
fn fault_queue_and_single_write_critical_limit_lock() {
    let mut s = sensor();
    assert!(write(&mut s, 0x19, 100, 10_000_000)); // locked: ignored
    assert_eq!(read(&mut s, 0x19, 10_000_000), 85);
    assert!(write(&mut s, 3, 3, 10_000_000));
    assert!(write(&mut s, 0x19, 90, 10_000_000));
    assert!(write(&mut s, 0x19, 110, 10_000_000));
    assert_eq!(read(&mut s, 0x19, 10_000_000), 90);
    assert!(s.set_inputs(216, Diode::Temperature(95 * 8), 11_000_000));
    assert_eq!(read(&mut s, 2, 96_500_000) & 0x12, 0);
    assert_eq!(read(&mut s, 2, 159_000_000) & 0x12, 0);
    assert_eq!(read(&mut s, 2, 221_500_000) & 0x12, 0x12);
    assert_eq!(read(&mut s, 3, 221_500_000) & 0x80, 0x80);
}
#[test]
fn status_clear_preserves_alarm_condition_and_comparator_reasserts() {
    let mut s = Sensor::new(3_300_000, false, 216, Diode::Temperature(75 * 8), 0).unwrap();
    s.advance(34_000_000);
    assert!(s.alert());
    assert_eq!(read(&mut s, 2, 34_000_000), 0x10);
    assert!(!s.alert());
    assert!(write(&mut s, 3, 0, 34_000_000));
    assert!(s.alert()); // clear MASK while the internal alarm condition remains
    assert_eq!(read(&mut s, 2, 34_000_000), 0);
    assert!(s.alert()); // no status bits: the datasheet requires ARA to clear
    assert!(s.set_inputs(216, Diode::Temperature(25 * 8), 35_000_000));
    s.advance(96_500_000);
    assert_eq!(read(&mut s, 2, 96_500_000), 0);
    assert!(s.alert()); // interrupt latch persists after condition removal
    assert!(write(&mut s, 0xbf, 1, 97_000_000));
    assert!(!s.alert()); // comparator output follows the present condition
    assert!(s.set_inputs(216, Diode::Temperature(75 * 8), 98_000_000));
    s.advance(159_000_000);
    assert!(s.alert());
    assert_eq!(read(&mut s, 2, 159_000_000), 0x10);
    assert!(!s.alert()); // read releases the pin without clearing active bits
    s.advance(221_500_000);
    assert!(s.alert());
}

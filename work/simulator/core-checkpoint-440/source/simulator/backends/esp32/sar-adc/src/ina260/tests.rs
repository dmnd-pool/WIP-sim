use super::*;

fn sensor() -> Sensor {
    Sensor::new(3_300_000, 11_980_000, -1_250_000, 0).unwrap()
}
fn select(s: &mut Sensor, reg: u8, now: u64) {
    assert!(s.start(false, now));
    assert!(s.write(reg, now));
    s.stop(now);
}
fn read(s: &mut Sensor, reg: u8, now: u64) -> u16 {
    select(s, reg, now);
    assert!(s.start(true, now));
    let high = s.read(now).unwrap();
    let low = s.read(now).unwrap();
    s.nack();
    s.stop(now);
    u16::from_be_bytes([high, low])
}
fn write(s: &mut Sensor, reg: u8, word: u16, now: u64) {
    assert!(s.start(false, now));
    assert!(s.write(reg, now));
    for byte in word.to_be_bytes() {
        assert!(s.write(byte, now));
    }
    s.stop(now);
}

#[test]
fn fixed_identity_signed_current_and_documented_voltage_example() {
    let mut s = sensor();
    assert_eq!(read(&mut s, 0, 0), 0x6127);
    assert_eq!(read(&mut s, 0xfe, 0), 0x5449);
    assert_eq!(read(&mut s, 0xff, 0), 0x2270);
    assert_eq!(read(&mut s, 1, 1_100_000), 0); // no partial set published
    assert_eq!(read(&mut s, 2, 2_199_999), 0);
    assert_eq!(read(&mut s, 1, 2_200_000), 0xfc18); // -1000 *1.25mA
    assert_eq!(read(&mut s, 2, 2_200_000), 0x2570); // TI section8.5.1:11.98V
    assert_eq!(read(&mut s, 3, 2_200_000), 1497); // |1.25A|*11.98V /10mW
    assert_eq!(read(&mut s, 6, 2_200_000), CVRF);
    assert_eq!(read(&mut s, 6, 2_200_000), 0);
    assert_eq!(s.conversions(), 1);
    assert_eq!(s.faults(), 0);
}

#[test]
fn physical_inputs_are_sampled_per_phase_and_words_do_not_tear() {
    let mut s = Sensor::new(3_300_000, 5_000_000, 1_000_000, 0).unwrap();
    write(&mut s, 0, 0x6123, 0);
    assert!(s.set_inputs(10_000_000, -2_000_000, 1));
    assert_eq!(read(&mut s, 1, 2_200_000), 800); // current sampled at start
    assert_eq!(read(&mut s, 2, 2_200_000), 8000); // voltage sampled at1.1ms
    assert_eq!(read(&mut s, 3, 2_200_000), 1000);
    assert_eq!(s.next_deadline(), NEVER);
    // Latch both bytes of one register even if a conversion completes between.
    write(&mut s, 0, 0x6123, 2_200_000);
    select(&mut s, 1, 2_200_000);
    s.start(true, 2_200_000);
    assert_eq!(s.read(2_200_000), Some(3));
    assert_eq!(s.read(4_400_000), Some(0x20));
    assert_eq!(read(&mut s, 1, 4_400_000), 0xf9c0);
}

#[test]
fn averaging_accumulates_each_power_sample_before_output_update() {
    let mut s = Sensor::new(3_300_000, 10_000_000, 1_000_000, 0).unwrap();
    write(&mut s, 0, 0x0203, 0); // four pairs,140us per conversion,triggered
    s.set_inputs(20_000_000, 2_000_000, 279_999);
    s.set_inputs(30_000_000, 3_000_000, 559_999);
    s.set_inputs(10_000_000, 4_000_000, 839_999);
    assert_eq!(read(&mut s, 3, 1_119_999), 0);
    assert_eq!(read(&mut s, 3, 1_120_000), 4500); // (10+40+90+40)/4 W
    assert_eq!(read(&mut s, 1, 1_120_000), 2000); //2.5A
    assert_eq!(read(&mut s, 2, 1_120_000), 14000); //17.5V
    assert_eq!(s.conversions(), 1);
    assert_eq!(s.faults(), 0);
}

#[test]
fn power_down_preserves_outputs_and_ready_then_recovers_for_forty_us() {
    let mut s = sensor();
    write(&mut s, 0, 0x6120, 2_200_000);
    assert_eq!(s.next_deadline(), NEVER);
    assert_eq!(read(&mut s, 6, 100_000_000), CVRF);
    assert_eq!(read(&mut s, 2, 100_000_000), 9584);
    s.set_inputs(5_000_000, 0, 100_000_000);
    write(&mut s, 0, 0x0002, 100_000_000); // triggered bus only,140us
    assert_eq!(read(&mut s, 2, 100_179_999), 9584);
    assert_eq!(read(&mut s, 2, 100_180_000), 4000);
    assert_eq!(read(&mut s, 1, 100_180_000), 0xfc18); // current not converted
    assert_eq!(s.next_deadline(), NEVER);
}

#[test]
fn alert_comparisons_use_unaveraged_samples_and_highest_function() {
    let mut s = Sensor::new(3_300_000, 5_000_000, 2_000_000, 0).unwrap();
    write(&mut s, 0, 0x0203, 0);
    write(&mut s, 7, 800, 0); //1A
    write(&mut s, 6, 0xc000, 0); // overcurrent has priority over undercurrent
    assert!(!s.alert());
    s.advance(140_000);
    assert!(s.alert());
    assert!(!s.alert_level());
    assert_eq!(read(&mut s, 1, 140_000), 0); // average not ready
    assert_eq!(read(&mut s, 6, 140_000), 0xc010);
    assert!(s.alert()); // transparent AFF not cleared by read
    s.set_inputs(5_000_000, 0, 140_001);
    s.advance(420_000);
    assert!(!s.alert());
    assert_eq!(read(&mut s, 6, 420_000), 0xc000);
}

#[test]
fn latched_alert_read_clear_conversion_ready_and_reset() {
    let mut s = sensor();
    write(&mut s, 7, (-500i16) as u16, 0);
    write(&mut s, 6, 0x4403, 0); // undercurrent,CNVR,latch,active-high
    s.advance(1_100_000);
    assert!(s.alert());
    assert!(s.alert_level());
    assert_eq!(read(&mut s, 6, 1_100_000), 0x4413);
    assert!(!s.alert());
    s.advance(2_200_000);
    assert!(s.alert());
    assert_eq!(read(&mut s, 6, 2_200_000), 0x440b);
    assert!(!s.alert());
    write(&mut s, 0, 0xffff, 2_200_000);
    assert_eq!(read(&mut s, 0, 2_200_000), CONFIG);
    assert_eq!(read(&mut s, 6, 2_200_000), 0);
    assert_eq!(read(&mut s, 7, 2_200_000), 0);
    assert_eq!(read(&mut s, 1, 2_200_000), 0);
    assert_eq!(s.next_deadline(), 3_300_000);
}

#[test]
fn partial_write_does_not_commit_and_repeated_start_keeps_pointer() {
    let mut s = sensor();
    s.start(false, 0);
    assert!(s.write(0, 0));
    assert!(s.write(0x80, 0));
    assert_eq!(s.next_deadline(), NEVER);
    s.stop(10);
    assert_eq!(read(&mut s, 0, 10), CONFIG);
    assert_eq!(s.faults(), 1); // partial config timing is an explicit approximation
    s.start(false, 10);
    assert!(s.write(0xfe, 10));
    s.start(true, 10);
    assert_eq!(s.read(10), Some(0x54));
    assert_eq!(s.read(10), Some(0x49));
    s.nack();
    s.stop(10);
    s.start(true, 10);
    assert_eq!(s.read(10), Some(0x54));
    assert_eq!(s.read(10), Some(0x49));
    assert_eq!(s.read(10), None); // unspecified third byte is not fabricated
    assert!(!s.write(0, 10));
    s.start(false, 10);
    assert!(!s.write(4, 10));
    s.start(false, 10);
    assert!(s.write(1, 10));
    assert!(!s.write(0, 10));
}

#[test]
fn physical_range_overflow_and_skipped_event_budgets_are_visible() {
    assert!(Sensor::new(2_699_999, 5_000_000, 0, 0).is_none());
    assert!(Sensor::new(3_300_000, 36_000_001, 0, 0).is_none());
    assert!(Sensor::new(3_300_000, 5_000_000, 15_000_001, 0).is_none());
    let mut s = Sensor::new(3_300_000, 36_000_000, 15_000_000, 0).unwrap();
    assert_eq!(read(&mut s, 6, 2_200_000), CVRF | OVF);
    assert_eq!(read(&mut s, 3, 2_200_000), 0xa3d7);
    assert_eq!(s.faults(), 1); // undefined overflow result has explicit model fault
    assert!(!s.set_inputs(36_000_001, 0, 2_200_000));
    let mut s = sensor();
    s.advance(60_000_000_000);
    assert_eq!(s.next_deadline(), NEVER);
    assert_eq!(s.faults(), 1);
}

#[test]
fn completed_config_write_cancels_old_conversion_and_readonly_bits_stay_fixed() {
    let mut s = sensor();
    // Halfway through the original current conversion, replace the sequence
    // with a140us bus-only trigger. The old completion must not update current.
    write(&mut s, 0, 0x0002, 500_000);
    assert_eq!(read(&mut s, 0, 500_000), 0x6002);
    assert_eq!(read(&mut s, 2, 639_999), 0);
    assert_eq!(read(&mut s, 2, 640_000), 9584);
    assert_eq!(read(&mut s, 1, 10_000_000), 0);
    assert_eq!(read(&mut s, 3, 10_000_000), 0); // no measured current yet
    assert_eq!(s.conversions(), 1);
    write(&mut s, 6, 0x7ff, 10_000_000);
    assert_eq!(read(&mut s, 6, 10_000_000), 0x40b); // only writable bits+CVRF
}

#[test]
fn voltage_only_power_uses_last_measured_current_and_mask_releases_pin() {
    let mut s = Sensor::new(3_300_000, 5_000_000, 2_000_000, 0).unwrap();
    write(&mut s, 0, 0x0001, 0); // current only,140us
    write(&mut s, 7, 800, 0);
    write(&mut s, 6, 0x8001, 0);
    assert_eq!(read(&mut s, 1, 140_000), 1600);
    assert!(s.alert());
    write(&mut s, 6, 0, 140_000);
    assert!(!s.alert());
    s.set_inputs(10_000_000, -10_000_000, 140_000);
    write(&mut s, 0, 0x0002, 140_000);
    assert_eq!(read(&mut s, 3, 280_000), 2000); // measured2A*new10V
    assert_eq!(read(&mut s, 1, 280_000), 1600);
    assert_eq!(s.conversions(), 2);
    assert_eq!(s.faults(), 0);
}

#[test]
fn ambiguous_latch_config_clear_is_explicit_and_transparent_flag_is_retained() {
    for latch in [0, 1] {
        let mut s = Sensor::new(3_300_000, 5_000_000, 2_000_000, 0).unwrap();
        write(&mut s, 7, 800, 0);
        write(&mut s, 6, 0x8000 | latch, 0);
        s.advance(1_100_000);
        assert!(s.alert());
        write(&mut s, 0, 0x6120, 1_100_000);
        if latch == 1 {
            assert!(!s.alert());
            assert_eq!(s.faults(), 1); // section8.3.3.2 versusTable6 conflict
        } else {
            assert!(s.alert());
            assert_eq!(s.faults(), 0);
        }
        write(&mut s, 0, 0x0001, 1_100_000);
        s.advance(1_280_000); //40us recovery +140us current conversion
        assert!(s.alert());
        assert_eq!(read(&mut s, 6, 1_280_000) & AFF, AFF);
        assert_eq!(s.alert(), latch == 0);
    }
}

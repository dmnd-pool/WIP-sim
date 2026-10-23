use md_sim_firmware::fan::{Controller, Error, MotorConfig};

fn motor() -> MotorConfig {
    MotorConfig {
        minimum_rpm: 0,
        maximum_rpm: 60,
        slew_rpm_per_second: 120,
        pulses_per_revolution: 2,
        pwm_channel: 0,
        tach_channel: Some(1),
    }
}

#[test]
fn tach_counts_integrated_revolutions_and_elapsed_measurement_windows() {
    let mut c = Controller::new(&[motor()]).unwrap();
    c.write(16, 4, 100).unwrap();
    c.advance(500_000_000).unwrap();
    assert_eq!(c.motor_rpm(0), Ok(60));
    assert_eq!(c.total_pulses(0), Ok(0)); // Quarter revolution while accelerating.
    assert_eq!(c.read(4, 4), Ok(0)); // No complete measurement window yet.
    c.advance(500_000_000).unwrap();
    assert_eq!(c.total_pulses(0), Ok(1)); // Three quarters of one revolution.
    assert_eq!(c.read(4, 4), Ok(0)); // One pulse does not make a revolution.
    c.advance(1_000_000_000).unwrap();
    assert_eq!(c.total_pulses(0), Ok(3));
    assert_eq!(c.read(4, 4), Ok(1));
    c.write(16, 1, 0).unwrap();
    c.advance(500_000_000).unwrap();
    assert_eq!(c.motor_rpm(0), Ok(0));
    assert_eq!(c.total_pulses(0), Ok(4)); // Coast adds exactly a quarter revolution.
}

#[test]
fn late_timer_keeps_elapsed_motion_and_measurement_phase() {
    let mut c = Controller::new(&[MotorConfig {
        maximum_rpm: 6000,
        slew_rpm_per_second: 12000,
        ..motor()
    }])
    .unwrap();
    c.write(16, 4, 100).unwrap();
    c.advance(2_000_000_000).unwrap();
    // A half-second linear ramp adds 50 pulses; steady motion adds 300.
    assert_eq!(c.total_pulses(0), Ok(350));
    assert_eq!(c.read(4, 4), Ok(100));
    assert_eq!(c.advance(65_000_000_000), Err(Error::TimeSpan));
    assert_eq!(c.total_pulses(0), Ok(350));
    c.advance_elapsed(65_250_000_000).unwrap();
    // 200 pulses per second, including the partial measurement window.
    assert_eq!(c.total_pulses(0), Ok(13400));
    assert_eq!(c.read(4, 4), Ok(100));
    assert_eq!(c.next_sample_ns(), 750_000_000);
    c.set_powered(0, false).unwrap();
    c.advance_elapsed(65_000_000_000).unwrap();
    // Coast-down adds exactly 50 pulses before the rotor stops.
    assert_eq!(c.total_pulses(0), Ok(13450));
    assert_eq!(c.motor_rpm(0), Ok(0));
    assert_eq!(c.read(4, 4), Ok(0));
    assert_eq!(c.next_sample_ns(), 750_000_000);
}

#[test]
fn sampling_is_invariant_under_irregular_backend_scheduling() {
    for slope in [7, 137, 12_013, 1_000_000] {
        let config = MotorConfig {
            minimum_rpm: 917,
            maximum_rpm: 5999,
            slew_rpm_per_second: slope,
            pulses_per_revolution: 7,
            ..motor()
        };
        let mut a = Controller::new(&[config]).unwrap();
        let mut b = Controller::new(&[config]).unwrap();
        let mut seed = 0x12345678_u64;
        for (duty, ns) in [(93, 3_141_592_653), (3, 7_123_456_789), (71, 2_000_000_013)] {
            a.write(16, 4, duty).unwrap();
            b.write(16, 4, duty).unwrap();
            a.advance(ns).unwrap();
            let mut left = ns;
            while left > 0 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let chunk = left.min(1 + seed % 100_000_000);
                b.advance(chunk).unwrap();
                left -= chunk;
            }
            assert_eq!(a.motor_rpm(0), b.motor_rpm(0));
            assert_eq!(a.total_pulses(0), b.total_pulses(0));
            assert_eq!(a.read(4, 4), b.read(4, 4));
            assert_eq!(a.next_sample_ns(), b.next_sample_ns());
        }
    }
}

#[test]
fn independent_duties_unwired_tach_stall_and_power_loss() {
    let mut c = Controller::new(&[
        MotorConfig {
            tach_channel: None,
            ..motor()
        },
        MotorConfig {
            maximum_rpm: 120,
            pwm_channel: 1,
            ..motor()
        },
    ])
    .unwrap();
    c.write(16, 1, 100).unwrap();
    c.write(20, 1, 100).unwrap();
    c.advance(3_000_000_000).unwrap();
    assert_eq!(c.motor_rpm(0), Ok(60));
    assert_eq!(c.motor_rpm(1), Ok(120));
    assert_eq!(c.read(0, 4), Ok(0)); // Real unwired channel stays zero.
    assert_eq!(c.read(4, 4), Ok(2));
    c.set_stalled(1, true).unwrap();
    assert_eq!(c.motor_rpm(1), Ok(0));
    c.advance(1_000_000_000).unwrap();
    assert_eq!(c.read(4, 4), Ok(0));
    c.set_stalled(1, false).unwrap();
    c.advance(2_000_000_000).unwrap();
    assert_eq!(c.read(4, 4), Ok(2));
    c.set_powered(1, false).unwrap();
    assert_eq!(c.motor_rpm(1), Ok(120)); // Power loss does not lock the rotor.
    c.advance(500_000_000).unwrap();
    assert_eq!(c.motor_rpm(1), Ok(60));
    c.advance(1_500_000_000).unwrap();
    assert_eq!(c.read(4, 4), Ok(0));
}

#[test]
fn reset_clears_registers_but_does_not_reset_physical_motion() {
    let mut c = Controller::new(&[motor()]).unwrap();
    c.write(16, 1, 100).unwrap();
    c.advance(3_250_000_000).unwrap();
    let pulses = c.total_pulses(0).unwrap();
    c.reset();
    assert_eq!(c.read(16, 4), Ok(0));
    assert_eq!(c.read(4, 4), Ok(0));
    assert_eq!(c.motor_rpm(0), Ok(60));
    assert_eq!(c.total_pulses(0), Ok(pulses));
    assert_eq!(c.next_sample_ns(), 1_000_000_000);
    c.advance(1_000_000_000).unwrap();
    assert_eq!(c.motor_rpm(0), Ok(0));
    assert_eq!(c.read(4, 4), Ok(0));
}

#[test]
fn register_widths_and_unsupported_values_do_not_fabricate_feedback() {
    let mut c = Controller::new(&[]).unwrap();
    c.write(16, 4, 0x100).unwrap();
    assert_eq!(c.read(16, 4), Ok(0)); // Eight-bit SVD register.
    c.write(16, 2, 100).unwrap();
    c.write(17, 1, 255).unwrap();
    assert_eq!(c.read(16, 4), Ok(100));
    assert_eq!(c.read(17, 1), Ok(0));
    assert_eq!(c.write(16, 1, 127), Err(Error::UnsupportedDuty));
    assert_eq!(c.read(16, 4), Ok(100));
    assert_eq!(c.write(17, 2, 0), Err(Error::Access));
    assert_eq!(c.read(0, 3), Err(Error::Access));
    assert_eq!(c.read(0x20, 4), Err(Error::Access));
    c.write(4, 4, 200).unwrap();
    c.advance(60_000_000_000).unwrap();
    assert_eq!(c.read(4, 4), Ok(0));
    assert_eq!(c.advance(60_000_000_001), Err(Error::TimeSpan));
    assert_eq!(c.next_sample_ns(), 1_000_000_000);
}

#[test]
fn invalid_motor_topology_and_parameters_are_rejected() {
    for config in [
        MotorConfig {
            minimum_rpm: 61,
            ..motor()
        },
        MotorConfig {
            maximum_rpm: 1_000_001,
            ..motor()
        },
        MotorConfig {
            slew_rpm_per_second: 0,
            ..motor()
        },
        MotorConfig {
            pulses_per_revolution: 0,
            ..motor()
        },
        MotorConfig {
            pwm_channel: 2,
            ..motor()
        },
        MotorConfig {
            tach_channel: Some(4),
            ..motor()
        },
    ] {
        assert!(Controller::new(&[config]).is_err());
    }
    assert!(Controller::new(&[motor(), motor()]).is_err());
}

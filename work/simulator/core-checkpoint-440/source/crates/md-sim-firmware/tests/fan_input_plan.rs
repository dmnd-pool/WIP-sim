use md_sim_firmware::fan::{Controller, Error, InputEvent, MotorConfig};

fn controller() -> Controller {
    Controller::new(&[MotorConfig {
        minimum_rpm: 0,
        maximum_rpm: 60,
        slew_rpm_per_second: 120,
        pulses_per_revolution: 2,
        pwm_channel: 0,
        tach_channel: Some(1),
    }])
    .unwrap()
}

fn plan() -> Vec<InputEvent> {
    vec![
        InputEvent {
            at_ns: 2_000_000_000,
            motor: 0,
            powered: Some(false),
            stalled: None,
        },
        InputEvent {
            at_ns: 3_000_000_000,
            motor: 0,
            powered: Some(true),
            stalled: None,
        },
        InputEvent {
            at_ns: 4_000_000_000,
            motor: 0,
            powered: None,
            stalled: Some(true),
        },
        InputEvent {
            at_ns: 5_000_000_000,
            motor: 0,
            powered: None,
            stalled: Some(false),
        },
    ]
}

#[test]
fn scheduled_power_coasts_and_stall_stops_after_the_previous_sample() {
    let mut c = controller();
    c.install_input_plan(plan()).unwrap();
    c.write(16, 4, 100).unwrap();
    c.advance_elapsed(2_000_000_000).unwrap();
    assert_eq!(c.motor_rpm(0), Ok(60)); // Power loss does not stop the rotor instantly.
    assert_eq!(c.read(4, 4), Ok(1)); // The preceding one-second window is complete.
    assert_eq!(c.take_applied_input(), Some(plan()[0]));
    c.advance_elapsed(1_000_000_000).unwrap();
    assert_eq!(c.motor_rpm(0), Ok(0));
    assert_eq!(c.total_pulses(0), Ok(4)); // Ramp-up, steady motion, and coast: two turns.
    c.advance_elapsed(1_000_000_000).unwrap();
    assert_eq!(c.motor_rpm(0), Ok(0)); // Locked rotor stops at exactly four seconds.
    c.advance_elapsed(2_000_000_000).unwrap();
    assert_eq!(c.motor_rpm(0), Ok(60));
    assert_eq!(c.total_pulses(0), Ok(7)); // Two further 0.75-turn acceleration windows.
    assert_eq!(c.read(4, 4), Ok(1));
    assert_eq!(
        (0..3)
            .map(|_| c.take_applied_input().unwrap())
            .collect::<Vec<_>>(),
        plan()[1..]
    );
    assert_eq!(c.take_applied_input(), None);
}

#[test]
fn input_timing_survives_late_callbacks_irregular_steps_and_controller_reset() {
    let mut a = controller();
    let mut b = controller();
    for c in [&mut a, &mut b] {
        c.install_input_plan(plan()).unwrap();
        c.write(16, 4, 100).unwrap();
    }
    a.advance_elapsed(65_123_456_789).unwrap();
    let mut remaining = 65_123_456_789u64;
    let mut seed = 7u64;
    while remaining > 0 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let step = remaining.min(1 + seed % 123_456_789);
        b.advance(step).unwrap();
        remaining -= step;
    }
    assert_eq!(a.total_pulses(0), b.total_pulses(0));
    assert_eq!(a.motor_rpm(0), b.motor_rpm(0));
    assert_eq!(a.read(4, 4), b.read(4, 4));
    assert_eq!(a.next_event_ns(), b.next_event_ns());
    for expected in plan() {
        assert_eq!(a.take_applied_input(), Some(expected));
        assert_eq!(b.take_applied_input(), Some(expected));
    }
    let mut c = controller();
    c.install_input_plan(plan()).unwrap();
    c.write(16, 4, 100).unwrap();
    c.advance_elapsed(4_500_000_000).unwrap();
    for _ in 0..3 {
        c.take_applied_input().unwrap();
    }
    c.reset();
    c.write(16, 4, 100).unwrap();
    assert_eq!(c.next_event_ns(), 500_000_000);
    c.advance(500_000_000).unwrap();
    assert_eq!(c.take_applied_input(), Some(plan()[3]));
    assert_eq!(c.motor_rpm(0), Ok(0));
    c.advance(500_000_000).unwrap();
    assert_eq!(c.motor_rpm(0), Ok(60));
    assert_eq!(c.take_applied_input(), None);
}

#[test]
fn invalid_or_late_plans_cannot_replace_valid_pending_inputs() {
    let mut c = controller();
    for events in [Vec::new(), vec![plan()[0]; 33], vec![plan()[1], plan()[0]]] {
        assert_eq!(
            c.install_input_plan(events.clone()),
            Err(Error::Configuration)
        );
    }
    for field in 0..4 {
        let mut e = plan()[0];
        match field {
            0 => e.at_ns = 0,
            1 => e.motor = 1,
            2 => {
                e.powered = None;
                e.stalled = None;
            }
            _ => e.at_ns = 3_600_000_000_001,
        }
        assert_eq!(c.install_input_plan(vec![e]), Err(Error::Configuration));
    }
    c.install_input_plan(plan()).unwrap();
    assert_eq!(c.install_input_plan(plan()), Err(Error::Configuration));
    let mut late = controller();
    late.advance(1).unwrap();
    assert_eq!(late.install_input_plan(plan()), Err(Error::Configuration));
    assert!(serde_json::from_str::<InputEvent>(
        r#"{"at_ns":1,"motor":0,"powered":false,"stalled":null,"invent_rpm":6000}"#
    )
    .is_err());
}

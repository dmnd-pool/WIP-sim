//! Operation bytes follow archived Braiins i2c.rs hash
//! 5cb55313d402cca4d61d1c03d6097a9e807298ea6e8a50335aa33b3cf8135ff1.
//! Even means read in the ASIC register; odd means write.
use md_sim_firmware::{
    bm1387::i2c::{Controller, Error, Profile},
    tmp421::{Diode, Inputs, Timing},
};

fn controller() -> Controller {
    let mut controller = Controller::new(Profile {
        address_7bit: 0x4c,
        clock_hz: 100_000,
        reset_word: 0,
        timing: Timing::REFERENCE,
        inputs: Inputs {
            local_millicelsius: 25_500,
            remote_millicelsius: 81_250,
            diode: Diode::Connected,
            supply_valid: true,
        },
    })
    .unwrap();
    controller.power(0, true).unwrap();
    controller
}

#[test]
fn actual_register_protocol_preserves_address_data_and_busy_deadlines() {
    let mut c = controller();
    assert_eq!(c.read(0), Ok(0));
    c.write(1_000, 0x0198_fe00).unwrap();
    assert_eq!(c.read(1_000), Ok(0x8198_fe00));
    assert_eq!(c.read(390_999), Ok(0x8198_fe00));
    assert_eq!(c.read(391_000), Ok(0x0198_fe55));
    c.write(400_000, 0x0199_0904).unwrap();
    assert_eq!(c.read(689_999), Ok(0x8199_0904));
    assert_eq!(c.read(690_000), Ok(0x0199_0904));
    c.write(700_000, 0x0198_0900).unwrap();
    assert_eq!(c.read(1_090_000), Ok(0x0198_0904));
    c.write(500_000_000, 0x0198_0100).unwrap();
    assert_eq!(c.read(500_390_000), Ok(0x0198_0191)); // 81 + 64.
}

#[test]
fn reset_aborts_pending_write_but_preserves_external_sensor_until_power_loss() {
    let mut c = controller();
    c.write(0, 0x0199_0904).unwrap();
    c.advance(290_000).unwrap();
    c.write(300_000, 0x0199_0944).unwrap();
    c.reset_controller();
    assert_eq!(c.read(1_000_000), Ok(0));
    c.write(1_000_000, 0x0198_0900).unwrap();
    assert_eq!(c.read(1_390_000), Ok(0x0198_0904));
    c.power(2_000_000, false).unwrap();
    assert_eq!(c.read(2_000_000), Err(Error::Unpowered));
    c.power(3_000_000, true).unwrap();
    c.write(3_000_000, 0x0198_0900).unwrap();
    assert_eq!(c.read(3_390_000), Ok(0x0198_0900));
    c.reset_time_origin();
    c.power(0, true).unwrap();
    assert_eq!(c.read(0), Ok(0));
}

#[test]
fn unknown_nack_and_busy_overwrite_fail_without_fabricated_data() {
    let mut c = controller();
    assert_eq!(
        c.write(0, 0x019b_fe00),
        Err(Error::NackBehaviorUnknown(0x9b))
    );
    assert_eq!(c.read(0), Ok(0));
    assert_eq!(c.write(0, 0x8099_fe00), Err(Error::InvalidCommand));
    c.write(0, 0x0198_ff00).unwrap();
    assert_eq!(c.write(1, 0x0198_fe00), Err(Error::BusyWrite));
    assert_eq!(c.read(390_000), Ok(0x0198_ff21));
    assert_eq!(c.read(389_999), Err(Error::TimeReversed));
}

fn planned(events: &[(u64, i32, Diode, bool)]) -> Controller {
    use md_sim_firmware::bm1387::i2c::InputEvent;
    let inputs = Inputs {
        local_millicelsius: 25_000,
        remote_millicelsius: 65_000,
        diode: Diode::Connected,
        supply_valid: true,
    };
    let mut c = Controller::new(Profile {
        address_7bit: 0x4c,
        clock_hz: 100_000,
        reset_word: 0,
        timing: Timing::REFERENCE,
        inputs,
    })
    .unwrap();
    c.install_input_plan(
        events
            .iter()
            .map(
                |&(at_ns, remote_millicelsius, diode, supply_valid)| InputEvent {
                    at_ns,
                    inputs: Inputs {
                        remote_millicelsius,
                        diode,
                        supply_valid,
                        ..inputs
                    },
                },
            )
            .collect(),
    )
    .unwrap();
    c
}

fn sensor_read(c: &mut Controller, at: u64, register: u8) -> u8 {
    c.write(at, 0x0198_0000 | (u32::from(register) << 8))
        .unwrap();
    let reply = c.read(at + 390_000).unwrap();
    assert_eq!(
        reply & 0xffff_ff00,
        0x0198_0000 | (u32::from(register) << 8)
    );
    reply as u8
}

#[test]
fn timed_temperature_edges_reach_real_registers_and_replay_after_timeline_reset() {
    let mut c = planned(&[
        (300_000_000, 105_000, Diode::Connected, true),
        (500_000_000, 65_000, Diode::Connected, true),
    ]);
    for _ in 0..2 {
        c.power(0, true).unwrap();
        assert_eq!(sensor_read(&mut c, 250_000_000, 1), 65);
        assert_eq!(sensor_read(&mut c, 490_500_000, 1), 105);
        assert_eq!(sensor_read(&mut c, 750_000_000, 1), 65);
        c.reset_time_origin();
    }
}

#[test]
fn late_poll_does_not_lose_transient_faults_or_reorder_bus_completion() {
    let events = [
        (150_000_000, 65_000, Diode::Open, false),
        (160_000_000, 65_000, Diode::Connected, true),
    ];
    let mut late = planned(&events);
    let mut stepped = planned(&events);
    late.power(0, true).unwrap();
    stepped.power(0, true).unwrap();
    for ns in (1_000_000..=249_000_000).step_by(1_000_000) {
        stepped.advance(ns).unwrap();
    }
    // The recovered diode still reports both faults from the conversion.
    assert_eq!(sensor_read(&mut late, 250_000_000, 0x11), 3);
    assert_eq!(sensor_read(&mut stepped, 250_000_000, 0x11), 3);
    assert_eq!(sensor_read(&mut late, 500_000_000, 0x11), 0);
    assert_eq!(sensor_read(&mut stepped, 500_000_000, 0x11), 0);

    let mut c = planned(&[(240_050_000, 105_000, Diode::Connected, true)]);
    c.power(0, true).unwrap();
    // Bus read and remote conversion both finish at the external edge.
    // The external change cannot retroactively change their65C result.
    assert_eq!(sensor_read(&mut c, 239_660_000, 1), 65);
    assert_eq!(sensor_read(&mut c, 500_000_000, 1), 105);
}

#[test]
fn external_input_plan_continues_during_power_loss_and_rejects_late_replacement() {
    use md_sim_firmware::bm1387::i2c::InputEvent;
    let event = InputEvent {
        at_ns: 1,
        inputs: Inputs {
            local_millicelsius: 25_000,
            remote_millicelsius: 105_000,
            diode: Diode::Connected,
            supply_valid: true,
        },
    };
    let mut c = planned(&[(300_000_000, 105_000, Diode::Connected, true)]);
    c.power(0, true).unwrap();
    c.power(100_000_000, false).unwrap();
    c.advance(400_000_000).unwrap();
    c.power(400_000_000, true).unwrap();
    assert_eq!(sensor_read(&mut c, 650_000_000, 1), 105);
    assert_eq!(c.install_input_plan(vec![event]), Err(Error::Profile));
    let mut fresh = controller();
    fresh.power(0, false).unwrap();
    assert_eq!(fresh.install_input_plan(vec![event]), Err(Error::Profile));
}

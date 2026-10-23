use md_sim_firmware::{
    bm1387::{
        chain::Profile,
        wire::{Error, Trace, Wire},
    },
    bm1397::crc5,
    serial8n1::{Error as SerialError, Event},
};

fn profile() -> Profile {
    Profile {
        chips: 63,
        cores_per_chip: 114,
        oscillator_hz: 25_000_000,
        initial_baud_divisor: 26,
        identity_reserved: 0x90,
    }
}

#[test]
fn model_failure_retains_the_actual_last_receive_byte_and_stop_time() {
    let mut wire = Wire::new(profile(), 1024).unwrap();
    wire.power(true).unwrap();
    wire.reset_line(false);
    let mut frame = vec![0x58, 9, 0, 0xff, 0, 0, 0, 0];
    let checksum = crc5(&frame);
    frame.push(checksum);
    assert_eq!(
        send(&mut wire, 0, 8640, &frame),
        Err(Error::Chain(
            md_sim_firmware::bm1387::chain::Error::UnknownRegister(0xff)
        ))
    );
    let ns = 89 * 8640 + 4320;
    assert_eq!(
        wire.take_failed_trace(),
        vec![
            Trace::Serial(Event::Received { ns, byte: checksum }),
            Trace::Stopped { ns }
        ]
    );
    assert!(wire.take_failed_trace().is_empty());
}

fn send(wire: &mut Wire, start: u64, period: u64, bytes: &[u8]) -> Result<Vec<Trace>, Error> {
    let mut trace = Vec::new();
    for (index, &byte) in bytes.iter().enumerate() {
        let at = start + index as u64 * 10 * period;
        trace.extend(wire.input(at, false)?);
        for bit in 0..8 {
            trace.extend(wire.input(at + (bit + 1) * period, byte & (1 << bit) != 0)?);
        }
        trace.extend(wire.input(at + 9 * period, true)?);
    }
    trace.extend(wire.advance(start + bytes.len() as u64 * 10 * period)?);
    Ok(trace)
}

#[test]
fn broadcast_identifies_63_chips_through_actual_timed_serial_bits() {
    let mut wire = Wire::new(profile(), 1024).unwrap();
    let query = [0x54, 5, 0, 0, 0x19];
    assert!(send(&mut wire, 0, 8640, &query).unwrap().is_empty());
    wire.power(true).unwrap();
    wire.reset_line(false);
    let start = 1_000_000;
    let period = 8640;
    let command_at = start + 49 * period + period / 2;
    let end = command_at + 63 * 7 * 10 * period;
    let mut trace = send(&mut wire, start, period, &query).unwrap();
    trace.extend(wire.advance(end).unwrap());
    let commands: Vec<_> = trace
        .iter()
        .filter(|e| matches!(e, Trace::Command { .. }))
        .collect();
    assert_eq!(
        commands,
        [&Trace::Command {
            ns: command_at,
            opcode: 0x54,
            replies: 63,
        }]
    );
    let levels: Vec<_> = trace
        .iter()
        .filter_map(|e| match e {
            Trace::Serial(Event::Level { ns, high }) => Some((*ns, *high)),
            _ => None,
        })
        .collect();
    let sent: Vec<_> = trace
        .iter()
        .filter_map(|e| match e {
            Trace::Serial(Event::Transmitted { ns, byte }) => Some((*ns, *byte)),
            _ => None,
        })
        .collect();
    // Identity bytes from the archived GetAddressReg vector. CRC07 was
    // calculated with the separate51-bit reference shift-register oracle.
    let reply = [0x13, 0x87, 0x90, 0, 0, 0, 7];
    assert_eq!(sent.len(), 63 * 7);
    for (index, &(ns, byte)) in sent.iter().enumerate() {
        assert_eq!(byte, reply[index % 7]);
        assert_eq!(ns, command_at + (index as u64 + 1) * 10 * period);
        // Sample the output waveform independently at each bit midpoint.
        for bit in 0..10 {
            let sample = command_at + (index as u64 * 10 + bit) * period + period / 2;
            let high = levels[levels.partition_point(|(at, _)| *at <= sample) - 1].1;
            let expected = match bit {
                0 => false,
                9 => true,
                _ => byte & (1 << (bit - 1)) != 0,
            };
            assert_eq!(high, expected, "byte{index} bit{bit}");
        }
    }
    assert!(wire.tx_high());
    assert_eq!(wire.next_deadline(), None);
}

#[test]
fn power_loss_cancels_replies_and_partial_commands_and_restores_baud() {
    let mut wire = Wire::new(profile(), 1024).unwrap();
    wire.power(true).unwrap();
    wire.reset_line(false);
    let mut baud = vec![0x58, 9, 0, 0x1c, 0, 0x20, 0x81, 0x80];
    baud.push(crc5(&baud));
    send(&mut wire, 0, 8640, &baud).unwrap();
    assert_eq!(wire.bit_period_ns(), 640);
    wire.advance(1_000_000).unwrap();
    // A partial command is cancelled by a real power loss.
    send(&mut wire, 1_000_000, 640, &[0x54, 5]).unwrap();
    wire.power(false).unwrap();
    assert_eq!(wire.bit_period_ns(), 8640);
    wire.input(2_000_000, false).unwrap();
    wire.power(true).unwrap();
    assert!(wire.advance(2_100_000).unwrap().is_empty());
    wire.input(2_100_000, true).unwrap();
    let trace = send(&mut wire, 3_000_000, 8640, &[0x54, 5, 0, 0, 0x19]).unwrap();
    assert!(trace.iter().any(|event| matches!(
        event,
        Trace::Command {
            opcode: 0x54,
            replies: 63,
            ..
        }
    )));
    assert!(!wire.tx_high()); // First response start bit is on the wire.
    wire.power(false).unwrap();
    assert!(wire.tx_high());
    assert_eq!(wire.next_deadline(), None);
    assert!(wire.advance(100_000_000).unwrap().is_empty());
}

#[test]
fn checksum_rejection_and_reply_resource_limits_cannot_emit_success() {
    let mut wire = Wire::new(profile(), 1024).unwrap();
    wire.power(true).unwrap();
    wire.reset_line(false);
    let trace = send(&mut wire, 0, 8640, &[0x54, 5, 0, 0, 0]).unwrap();
    assert!(trace.iter().any(|event| matches!(
        event,
        Trace::Rejected {
            error: md_sim_firmware::bm1387::Error::Checksum,
            ..
        }
    )));
    assert!(!trace
        .iter()
        .any(|event| matches!(event, Trace::Command { .. })));
    assert!(wire.tx_high());
    assert_eq!(wire.next_deadline(), None);
    let mut small = Wire::new(profile(), 7).unwrap();
    small.power(true).unwrap();
    small.reset_line(false);
    assert_eq!(
        send(&mut small, 0, 8640, &[0x54, 5, 0, 0, 0x19]),
        Err(Error::Serial(SerialError::Capacity))
    );
    assert!(small.tx_high());
    assert_eq!(small.next_deadline(), None);
}

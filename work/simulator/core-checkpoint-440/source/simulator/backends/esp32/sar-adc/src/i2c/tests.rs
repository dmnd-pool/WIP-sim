use super::*;
use std::vec::Vec;

#[derive(Default)]
struct Device {
    events: Vec<(u8, u16)>,
    nack_at: Option<usize>,
    input: Vec<u8>,
}
impl Bus for Device {
    fn start(&mut self, a: u8, r: bool) -> bool {
        self.events.push((0, u16::from(a) | (u16::from(r) << 8)));
        self.nack_at != Some(self.events.len())
    }
    fn write(&mut self, b: u8) -> bool {
        self.events.push((1, u16::from(b)));
        self.nack_at != Some(self.events.len())
    }
    fn read(&mut self) -> Option<u8> {
        self.events.push((2, 0));
        if self.input.is_empty() {
            None
        } else {
            Some(self.input.remove(0))
        }
    }
    fn stop(&mut self) {
        self.events.push((3, 0));
    }
    fn nack(&mut self) {
        self.events.push((4, 0));
    }
}
fn configured() -> Controller {
    let mut c = Controller::default();
    c.write(4, 4, 0x113, 0);
    c.write(0, 4, 49, 0);
    c.write(0x38, 4, 24 | (24 << 9), 0);
    c.write(0x40, 4, 49, 0);
    c.write(0x44, 4, 49, 0);
    c.write(0x48, 4, 49, 0);
    c.write(0x4c, 4, 49, 0);
    c.write(0x54, 4, (1 << 21) | 3, 0);
    c.write(4, 4, 0x113 | UPDATE, 0);
    c
}
fn command(c: &mut Controller, commands: &[u32], bytes: &[u8], now: u64) {
    for (i, &cmd) in commands.iter().enumerate() {
        c.write(0x58 + 4 * i as u64, 4, cmd, now);
    }
    for &b in bytes {
        c.write(0x1c, 4, u32::from(b), now);
    }
    c.write(4, 4, 0x113 | START, now);
}
fn finish(c: &mut Controller, b: &mut Device) {
    for _ in 0..2060 {
        let Some(now) = c.deadline() else { return };
        c.advance(now, b);
    }
    panic!("bounded command sequence did not end");
}
#[test]
fn trm_master_write_has_actual_bytes_timing_done_and_irq() {
    let mut c = configured();
    let mut b = Device::default();
    c.write(0x28, 4, COMPLETE, 0);
    command(
        &mut c,
        &[6 << 11, (1 << 11) | (1 << 8) | 3, 2 << 11],
        &[0x50, 0xab, 0xcd],
        100,
    );
    assert_eq!(c.deadline(), Some(10_100)); // (49+49+2) / 10MHz
    c.advance(10_099, &mut b);
    assert!(b.events.is_empty());
    c.advance(10_100, &mut b);
    assert_eq!(c.deadline(), Some(100_100)); //9*100 cycles
    finish(&mut c, &mut b);
    assert_eq!(b.events, [(0, 0x28), (1, 0xab), (1, 0xcd), (3, 0)]);
    assert_eq!(c.bytes(), 3);
    assert_eq!(c.faults(), 0);
    assert!(c.irq());
    assert_eq!(c.read(0x08, 4) & (1 << 4), 0);
    for a in [0x58, 0x5c, 0x60] {
        assert_ne!(c.read(a, 4) & DONE, 0);
    }
    c.write(0x24, 4, COMPLETE, 0);
    assert!(!c.irq());
}
#[test]
fn address_and_data_nack_do_not_report_completion_or_later_bytes() {
    for fail_at in [1, 2] {
        let mut c = configured();
        let mut b = Device {
            nack_at: Some(fail_at),
            ..Device::default()
        };
        command(
            &mut c,
            &[6 << 11, (1 << 11) | (1 << 8) | 3, 2 << 11],
            &[0x78, 0, 0xaf],
            0,
        );
        finish(&mut c, &mut b);
        assert_eq!(c.read(0x20, 4) & (NACK | COMPLETE), NACK);
        assert_eq!(c.read(0x60, 4) & DONE, 0);
        assert_eq!(c.faults(), 0);
        assert_eq!(b.events.len(), fail_at + 1);
        assert_eq!(b.events.last(), Some(&(3, 0)));
    }
}
#[test]
fn repeated_start_read_and_last_nack_keep_transaction_order() {
    let mut c = configured();
    let mut b = Device {
        input: vec![0x12, 0x34],
        ..Device::default()
    };
    command(
        &mut c,
        &[
            6 << 11,
            (1 << 11) | (1 << 8) | 2,
            6 << 11,
            (1 << 11) | (1 << 8) | 1,
            (3 << 11) | 1,
            (3 << 11) | (1 << 10) | 1,
            2 << 11,
        ],
        &[0x90, 0x20, 0x91],
        0,
    );
    finish(&mut c, &mut b);
    assert_eq!(
        b.events,
        [
            (0, 0x48),
            (1, 0x20),
            (0, 0x148),
            (2, 0),
            (2, 0),
            (4, 0),
            (3, 0)
        ]
    );
    assert_eq!(c.read(0x1c, 4), 0x12);
    assert_eq!(c.read(0x1c, 4), 0x34);
    assert_eq!(c.faults(), 0);
}
#[test]
fn end_retains_bus_and_following_sequence_continues_without_address() {
    let mut c = configured();
    let mut b = Device::default();
    command(
        &mut c,
        &[6 << 11, (1 << 11) | (1 << 8) | 2, 4 << 11],
        &[0x50, 0xab],
        0,
    );
    finish(&mut c, &mut b);
    assert_eq!(c.read(0x20, 4) & (END | COMPLETE), END);
    assert_ne!(c.read(8, 4) & 16, 0);
    command(
        &mut c,
        &[(1 << 11) | (1 << 8) | 1, 2 << 11],
        &[0xcd],
        1_000_000,
    );
    finish(&mut c, &mut b);
    assert_eq!(b.events, [(0, 0x28), (1, 0xab), (1, 0xcd), (3, 0)]);
    assert_eq!(c.faults(), 0);
}
#[test]
fn fifo_bounds_bad_commands_and_mode_gaps_are_visible() {
    let mut c = configured();
    for b in 0..33 {
        c.write(0x1c, 4, b, 0);
    }
    assert_ne!(c.read(0x20, 4) & (1 << 11), 0);
    assert_eq!(c.read(8, 4) >> 18 & 63, 32);
    c.write(0x18, 4, 0x608b, 0);
    assert_eq!(c.read(8, 4) >> 18 & 63, 0);
    let mut b = Device::default();
    command(&mut c, &[6 << 11, (1 << 11) | 2, 2 << 11], &[0x50], 0);
    finish(&mut c, &mut b);
    assert_ne!(c.read(0x20, 4) & (1 << 6), 0);
    assert_eq!(b.events, [(0, 0x28), (3, 0)]);
    for (address, value) in [(4, 0x103), (0x10, 1 << 31), (0x18, 1 << 10)] {
        let mut c = configured();
        c.write(address, 4, value, 0);
        let control = c.read(4, 4);
        c.write(4, 4, control | UPDATE, 0);
        command(&mut c, &[6 << 11, 2 << 11], &[], 0);
        assert!(c.deadline().is_none());
        assert!(c.faults() > 0);
    }
    let mut c = configured();
    command(&mut c, &[0], &[], 0);
    finish(&mut c, &mut b);
    assert!(c.faults() > 0);
    let before = c.faults();
    c.write(0x78, 1, 0, 0);
    c.read(0x100, 4);
    assert_eq!(c.faults(), before + 2);
}
#[test]
fn configuration_sync_reset_and_deadline_overflow_do_not_emit_bytes() {
    let mut c = Controller::default();
    assert_eq!(c.read(4, 4), 0x10b); // TRM27.11 includes RX_FULL_ACK_LEVEL.
    let mut b = Device::default();
    c.write(4, 4, 0x113, 0);
    command(&mut c, &[6 << 11, 2 << 11], &[], 0);
    assert!(c.deadline().is_none());
    let mut c = configured();
    command(&mut c, &[6 << 11, (1 << 11) | 1, 2 << 11], &[0x50], 0);
    c.advance(c.deadline().unwrap(), &mut b);
    c.write(4, 4, 0x113 | RESET, 0);
    assert!(c.take_stop());
    assert!(c.deadline().is_none());
    c.advance(u64::MAX, &mut b);
    assert!(b.events.is_empty());
    let mut c = configured();
    command(&mut c, &[6 << 11, 2 << 11], &[], u64::MAX);
    assert!(c.deadline().is_none());
    assert!(c.faults() > 0);
}

#[test]
fn rx_overflow_retains_exact_bytes_and_level_irq_survives_clear() {
    let mut c = configured();
    let mut b = Device {
        input: (0..33).collect(),
        ..Device::default()
    };
    c.write(0x28, 4, 1, 0);
    command(
        &mut c,
        &[6 << 11, (1 << 11) | (1 << 8) | 1, (3 << 11) | 33, 2 << 11],
        &[0x91],
        0,
    );
    finish(&mut c, &mut b);
    assert_eq!(b.events.iter().filter(|v| v.0 == 2).count(), 32);
    assert_eq!(c.read(8, 4) >> 8 & 63, 32);
    assert_ne!(c.read(0x20, 4) & (1 << 2), 0);
    assert_eq!(c.read(0x20, 4) & COMPLETE, 0);
    assert!(c.irq());
    c.write(0x24, 4, 1, 0);
    assert!(c.irq()); // The FIFO still satisfies the level condition.
    for byte in 0..32 {
        assert_eq!(c.read(0x1c, 4), byte);
    }
    assert!(!c.irq());
    c.write(0x20, 4, 1 << 2, 0); // TRM table: RAW is R/SS/WTC.
    assert_eq!(c.read(0x20, 4) & (1 << 2), 0);
    assert_eq!(c.read(0x1c, 4), 0);
    assert_ne!(c.read(0x20, 4) & (1 << 12), 0);
}

#[test]
fn active_command_mutation_is_rejected_and_original_sequence_is_retained() {
    let mut c = configured();
    let mut b = Device::default();
    command(
        &mut c,
        &[6 << 11, (1 << 11) | (1 << 8) | 1, 2 << 11],
        &[0x50],
        0,
    );
    c.write(0x5c, 4, (1 << 11) | (1 << 8) | 255, 0);
    finish(&mut c, &mut b);
    assert_eq!(b.events, [(0, 0x28), (3, 0)]);
    assert_eq!(c.read(0x5c, 4) & 255, 1);
    assert_eq!(c.faults(), 1);
}

#[test]
fn actual_guest_stop_restart_refreshes_command_without_false_timeout() {
    let mut c = configured();
    let mut b = Device {
        nack_at: Some(1),
        ..Device::default()
    };
    command(
        &mut c,
        &[6 << 11, (1 << 11) | (1 << 8) | 1, 4 << 11],
        &[0x78],
        0,
    );
    finish(&mut c, &mut b);
    assert_eq!(c.read(0x20, 4) & (NACK | TIMEOUT), NACK);
    c.write(0x24, 4, NACK, 100_000);
    // The real firmware retries its STOP trigger before the first STOP timer.
    command(&mut c, &[2 << 11], &[], 101_000);
    assert_eq!(c.deadline(), Some(111_000));
    c.write(4, 4, 0x113 | UPDATE, 101_500);
    c.write(4, 4, 0x113 | START, 102_000);
    assert_eq!(c.deadline(), Some(112_000));
    c.advance(111_999, &mut b);
    assert_eq!(c.read(0x20, 4) & (COMPLETE | TIMEOUT), 0);
    c.advance(112_000, &mut b);
    assert_eq!(c.read(0x20, 4) & (COMPLETE | TIMEOUT), COMPLETE);
    assert_eq!(c.faults(), 0);
    assert_eq!(b.events, [(0, 0x3c), (3, 0)]);
}

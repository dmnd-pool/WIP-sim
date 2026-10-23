use super::*;
fn ready() -> Controller {
    let mut c = Controller::default();
    c.write(0, 0x104, 4, 1 << 30, 0);
    c.write(0, 0x3c, 4, 3 << 29, 0);
    c.write(0, 0x14, 4, 0, 0);
    c
}
fn start(c: &mut Controller, now: u64) {
    c.write(0, 0xc, 4, PAD_FORCE | FORCE | (1 << 19), now);
    c.write(0, 0xc, 4, PAD_FORCE | FORCE | (1 << 19) | START, now);
}
#[test]
fn reference_equation_and_saturation() {
    assert_eq!(convert(550_000, 1_100_000, 0), 2047);
    assert_eq!(convert(1_100_000, 1_100_000, 1), 3071);
    assert_eq!(convert(2_200_000, 1_100_000, 2), 4095);
    assert_eq!(convert(3_300_000, 1_100_000, 3), 3071);
    assert_eq!(convert(u32::MAX, 1, 0), 4095);
}
#[test]
fn external_conversion_has_latency_and_readonly_result() {
    let mut c = ready();
    assert!(c.set_input(0, 0, 550_000));
    start(&mut c, 100);
    let due = c.next_deadline();
    assert_eq!(due, 6350);
    assert_eq!(c.read(0, 0xc, 4) & DONE, 0);
    c.advance(due - 1);
    assert_eq!(c.read(0, 0xc, 4) & DONE, 0);
    c.advance(due);
    assert_eq!(c.read(0, 0xc, 4) & 0x1ffff, DONE | 2047);
    c.write(
        0,
        0xc,
        4,
        PAD_FORCE | FORCE | (1 << 19) | START | 0xffff,
        due,
    );
    assert_eq!(c.read(0, 0xc, 4) & 0x1ffff, DONE | 2047);
    assert_eq!(c.completed(), 1);
}
#[test]
fn actual_guest_ground_command_is_stateful_and_reset_cancels() {
    let mut c = ready();
    c.write(1, 0, 4, 0x0520_0769, 0);
    c.write(1, 0, 4, 0x0400_0769, 0);
    assert_eq!(c.read(1, 0, 4), 0x0020_0000);
    c.write(0, 0xc, 4, PAD_FORCE | FORCE, 0);
    c.write(0, 0xc, 4, PAD_FORCE | FORCE | START, 0);
    c.advance(c.next_deadline());
    assert_eq!(c.read(0, 0xc, 4) & 0x1ffff, DONE);
    c.write(1, 0, 4, 0x0500_0769, 0);
    c.write(0, 0xc, 4, FORCE, 0);
    c.write(0, 0xc, 4, FORCE | START, 0);
    assert_eq!(c.next_deadline(), u64::MAX);
    assert_eq!(c.read(0, 0xc, 4) & DONE, 0);
    assert!(c.faults() > 0);
    c.write(1, 0, 4, 0x0520_0769, 0);
    c.write(0, 0xc, 4, FORCE, 0);
    c.write(0, 0xc, 4, FORCE | START, 0);
    c.reset();
    c.advance(u64::MAX);
    assert_eq!(c.completed(), 0);
    assert_eq!(c.read(0, 0xc, 4), 0);
}
#[test]
fn clocks_power_missing_inputs_and_invalid_access_do_not_succeed() {
    let mut c = ready();
    start(&mut c, 0);
    assert_eq!(c.next_deadline(), u64::MAX);
    c.set_input(0, 0, 1_100_000);
    c.write(0, 0x104, 4, 0, 0);
    start(&mut c, 0);
    assert_eq!(c.next_deadline(), u64::MAX);
    c.write(0, 0x104, 4, CLOCK, 0);
    c.write(0, 0x3c, 4, 2 << 29, 0);
    start(&mut c, 0);
    assert_eq!(c.next_deadline(), u64::MAX);
    assert_eq!(c.read(0, 1, 4), 0);
    c.write(0, u32::MAX, 4, 1, 0);
    c.write(1, 0, 4, 0xff, 0);
    assert!(c.faults() >= 6);
}
#[test]
fn stopping_clock_cancels_and_next_start_uses_new_sample() {
    let mut c = ready();
    c.set_input(0, 0, 550_000);
    start(&mut c, 0);
    c.write(0, 0x104, 4, 0, 10);
    c.advance(99999);
    assert_eq!(c.completed(), 0);
    c.write(0, 0x104, 4, CLOCK, 100_000);
    c.set_input(0, 0, 1_100_000);
    start(&mut c, 100_000);
    c.advance(c.next_deadline());
    assert_eq!(c.read(0, 0xc, 4) & 0xfff, 4095);
}

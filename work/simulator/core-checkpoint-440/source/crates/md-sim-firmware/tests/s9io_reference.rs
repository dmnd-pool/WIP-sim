use md_sim_firmware::s9io::{
    registers::{BusError, Registers, Revision, BAUD, CONTROL, VERSION, WORK_TIME},
    Error,
};

#[test]
fn reference_clock_break_and_reset_are_distinct_from_legacy() {
    let mut s = Registers::with_revision(0x12345678, Revision::Reference1_0_2);
    assert_eq!(s.read(VERSION).unwrap(), 0x00901002);
    assert_eq!(s.core_period_ns(), 5);
    assert!(!s.txd());
    assert!(!s.tick(true).unwrap().uart.txd);
    s.write(CONTROL, 0x0c, 15).unwrap();
    assert!(s.txd());
    s.write(BAUD, 108, 15).unwrap();
    // S9 reference formula: 200MHz / (16 * (108 + 1)).
    assert_eq!(u64::from(s.core_period_ns()) * 16 * 109, 8720);
    s.write(WORK_TIME, 0x40507, 15).unwrap();
    s.write(CONTROL, 4, 15).unwrap();
    assert!(!s.txd());
    assert!(!s.tick(true).unwrap().uart.txd);
    s.reset();
    assert_eq!(s.read(VERSION).unwrap(), 0x00901002);
    assert_eq!(s.read(BAUD).unwrap(), 0);
    assert_eq!(s.read(WORK_TIME).unwrap(), 1);
    assert!(!s.txd());
    let legacy = Registers::new(0x12345678);
    assert_eq!(legacy.core_period_ns(), 20);
    assert!(legacy.txd());
}

#[test]
fn unimplemented_bm139x_mode_cannot_run_as_bm1387() {
    let mut s = Registers::with_revision(0, Revision::Reference1_0_2);
    s.write(CONTROL, 0x10, 15).unwrap();
    assert!(!s.txd());
    s.tick(true).unwrap(); // Disabled configuration has no active dataplane.
    s.write(CONTROL, 0x18, 15).unwrap();
    assert_eq!(s.read(CONTROL).unwrap(), 0x18);
    assert_eq!(s.idle_limit(true), 0);
    assert_eq!(s.skip_idle(1_000_000, true), 0);
    assert_eq!(s.tick(true), Err(Error::UnsupportedAsicMode));
    s.reset();
    s.tick(true).unwrap();
}

#[test]
fn actual_command_bytes_and_unsupported_disable_are_observable() {
    let mut s = Registers::with_revision(0, Revision::Reference1_0_2);
    s.write(CONTROL, 12, 15).unwrap();
    s.write(WORK_TIME, 257, 15).unwrap();
    // Let the inherited clocked enable/reset sequence clear its queues
    // before the host submits a command.
    for _ in 0..100 {
        s.tick(true).unwrap();
    }
    // Reference read-all command: bytes54 05 00 00, FPGA appends CRC5=19.
    s.write(0x1004, 0x00000554, 15).unwrap();
    let mut bytes = Vec::new();
    for _ in 0..2000 {
        if let Some(byte) = s.tick(true).unwrap().uart.started {
            bytes.push(byte);
        }
    }
    assert_eq!(bytes, [0x54, 5, 0, 0, 0x19]);
    assert_eq!(
        s.write(CONTROL, 0, 15),
        Err(BusError::UnsupportedTransition)
    );
    assert_eq!(s.read(CONTROL).unwrap(), 12);
    // The documented reset preserves the midstate field.
    s.write(CONTROL, 4, 15).unwrap();
    assert!(!s.txd());
    s.write(CONTROL, 12, 15).unwrap();
    assert!(s.txd());
    s.reset();
    s.write(CONTROL, 0, 15).unwrap();
}

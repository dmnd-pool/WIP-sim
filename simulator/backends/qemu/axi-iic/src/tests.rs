use super::*;
#[derive(Default)]
struct Peripheral {
    events: Vec<String>,
    data: Vec<u8>,
    pos: usize,
    reject_address: bool,
    reject_data: bool,
}
impl Bus for Peripheral {
    fn start(&mut self, address: u8, read: bool) -> bool {
        self.events.push(format!("start:{address:02x}:{read}"));
        address == 0x34 && !self.reject_address
    }
    fn send(&mut self, byte: u8) -> bool {
        self.events.push(format!("send:{byte:02x}"));
        !self.reject_data
    }
    fn receive(&mut self) -> u8 {
        let v = self.data[self.pos];
        self.pos += 1;
        self.events.push(format!("receive:{v:02x}"));
        v
    }
    fn nack(&mut self) {
        self.events.push("nack".into());
    }
    fn end(&mut self) {
        self.events.push("end".into());
    }
}
fn initialized() -> Controller {
    let mut c = Controller::default();
    // Literal writes from the archived Linux xiic_reinit sequence.
    c.write(0x40, 4, 0xa);
    c.write(0x120, 1, 15);
    c.write(0x100, 1, 2);
    c.write(0x100, 1, 1);
    assert_eq!(c.read(0x104, 1), 0xc0);
    c.write(0x1c, 4, 0x8000_0000);
    c.write(0x28, 4, 0x21);
    assert!(!c.irq());
    c
}
fn drain(c: &mut Controller, p: &mut Peripheral) {
    for _ in 0..300 {
        if !c.pending() {
            return;
        }
        c.step(p);
    }
    panic!("controller did not finish bounded transfer");
}
#[test]
fn vendor_four_byte_write_sequence_obeys_clock_and_stop() {
    // PG090 dynamic-controller example: target0x34, four data bytes.
    let mut c = initialized();
    let mut p = Peripheral::default();
    for word in [0x168, 0x89, 0xab, 0xcd, 0x2ef] {
        c.write(0x108, 2, word);
    }
    assert!(p.events.is_empty());
    assert_eq!(c.read(0x114, 1), 4);
    for expected in 1..=5 {
        c.step(&mut p);
        assert_eq!(p.events.len(), expected + usize::from(expected == 5));
    }
    assert_eq!(
        p.events,
        [
            "start:34:false",
            "send:89",
            "send:ab",
            "send:cd",
            "send:ef",
            "end"
        ]
    );
    assert_eq!(c.read(0x104, 1), 0xc0);
    assert_eq!(c.fault_count(), 0);
}
#[test]
fn repeated_start_read_has_real_data_and_nack() {
    let mut c = initialized();
    let mut p = Peripheral {
        data: vec![0x12, 0x34, 0x56, 0x78],
        ..Default::default()
    };
    c.write(0x120, 1, 3);
    for word in [0x168, 0x20, 0x169, 0x204] {
        c.write(0x108, 2, word);
    }
    drain(&mut c, &mut p);
    assert_eq!(
        p.events,
        [
            "start:34:false",
            "send:20",
            "start:34:true",
            "receive:12",
            "receive:34",
            "receive:56",
            "receive:78",
            "nack",
            "end"
        ]
    );
    c.write(0x28, 4, 8);
    assert!(c.irq());
    assert_eq!(c.read(0x118, 1), 3);
    let received: Vec<_> = (0..4).map(|_| c.read(0x10c, 1)).collect();
    assert_eq!(received, [0x12, 0x34, 0x56, 0x78]);
    c.write(0x20, 4, 8);
    assert!(!c.irq());
    assert_eq!(c.read(0x104, 1), 0xc0);
}
#[test]
fn receive_count_is_not_an_extra_wire_byte() {
    let mut c = initialized();
    let mut p = Peripheral {
        data: vec![0xa5, 0x5a],
        ..Default::default()
    };
    c.write(0x108, 2, 0x169);
    c.write(0x108, 2, 0x202);
    c.step(&mut p); // One address interval.
    assert_eq!(p.events, ["start:34:true"]);
    c.step(&mut p); // First data interval also consumes the control count.
    assert_eq!(p.events, ["start:34:true", "receive:a5"]);
    c.step(&mut p); // Last data interval includes NACK/STOP.
    assert_eq!(
        p.events,
        ["start:34:true", "receive:a5", "receive:5a", "nack", "end"]
    );
    assert!(!c.pending());
    assert_eq!(c.read(0x10c, 1), 0xa5);
    assert_eq!(c.read(0x10c, 1), 0x5a);
    assert_eq!(c.fault_count(), 0);
}
#[test]
fn missing_device_and_data_nack_raise_error_and_end_bus() {
    for data in [false, true] {
        let mut c = initialized();
        let mut p = Peripheral {
            reject_address: !data,
            reject_data: data,
            ..Default::default()
        };
        c.write(0x28, 4, 2);
        for word in [0x168, 0x201] {
            c.write(0x108, 2, word);
        }
        drain(&mut c, &mut p);
        assert!(c.irq());
        assert_eq!(p.events.last().unwrap(), "end");
        assert_eq!(p.events.len(), if data { 3 } else { 2 });
        assert_eq!(c.fault_count(), 1);
        assert_eq!(c.read(0x104, 1), 0xc0);
    }
}
#[test]
fn fifo_bound_and_receive_backpressure_preserve_bytes() {
    let mut c = initialized();
    let mut p = Peripheral {
        data: (0..20).collect(),
        ..Default::default()
    };
    for i in 0..16 {
        c.write(0x108, 2, if i == 0 { 0x168 } else { i });
    }
    assert_eq!(c.read(0x114, 1), 15);
    assert_eq!(c.read(0x104, 1) & 0x10, 0x10);
    c.write(0x108, 2, 0x55);
    assert_eq!(c.fault_count(), 1);
    assert_eq!(c.read(0x114, 1), 15);
    c.reset();
    c.write(0x100, 1, 1);
    c.write(0x120, 1, 15);
    c.write(0x108, 2, 0x169);
    c.write(0x108, 2, 0x214);
    drain(&mut c, &mut p);
    assert_eq!(p.pos, 16);
    assert_eq!(c.read(0x104, 1) & 0x20, 0x20);
    let mut received = vec![];
    for _ in 0..4 {
        received.push(c.read(0x10c, 1));
        c.step(&mut p);
    }
    for _ in 0..16 {
        received.push(c.read(0x10c, 1));
    }
    assert_eq!(received, (0..20).collect::<Vec<_>>());
    assert_eq!(p.events.last().unwrap(), "end");
}
#[test]
fn soft_reset_ends_inflight_bus_and_discards_queued_data() {
    let mut c = initialized();
    let mut p = Peripheral::default();
    c.write(0x108, 2, 0x168);
    c.write(0x108, 2, 0x201);
    c.step(&mut p);
    c.write(0x40, 4, 0xa);
    assert!(!c.irq());
    assert_eq!(c.read(0x104, 1), 0xc0);
    c.step(&mut p);
    assert_eq!(p.events, ["start:34:false", "end"]);
    assert!(!c.pending());
    assert_eq!(c.read(0x100, 1), 0);
    assert_eq!(c.read(0x28, 4), 0);
}
#[test]
fn lane_access_and_toggle_interrupt_semantics() {
    let mut c = initialized();
    c.write(0x1f, 1, 0);
    assert!(!c.irq());
    assert_eq!(c.read(0x1c, 4), 0);
    c.write(0x1f, 1, 0x80);
    assert_eq!(c.read(0x1c, 4), 0x8000_0000);
    c.write(0x20, 4, 1);
    assert!(c.irq());
    c.write(0x20, 4, 1);
    assert!(!c.irq());
    c.write(0x103, 2, 1);
    assert_eq!(c.fault_count(), 1);
    assert_eq!(c.read(0x100, 1), 1);
}

#[test]
fn bus_idle_interrupt_survives_completion_between_read_and_toggle() {
    // Regression from original SD2022 run017: its driver reads ISR,
    // a one-byte write finishes, then the driver clears/enables BNB.
    // Losing BNB caused a timeout and retransmission of an ACKed PIC byte.
    for steps_before_read in 0..=2 {
        for steps_before_clear in steps_before_read..=2 {
            let mut c = initialized();
            let mut p = Peripheral::default();
            c.write(0x28, 4, 0);
            c.write(0x108, 2, 0x168);
            c.write(0x108, 2, 0x255);
            assert_eq!(c.read(0x104, 1) & 4, 4);
            for _ in 0..steps_before_read {
                c.step(&mut p);
            }
            let previous = c.read(0x20, 4) & 0x14;
            for _ in steps_before_read..steps_before_clear {
                c.step(&mut p);
            }
            c.write(0x20, 4, previous);
            c.write(0x28, 4, 0x10);
            assert_eq!(c.irq(), steps_before_clear == 2);
            for _ in steps_before_clear..2 {
                c.step(&mut p);
            }
            assert!(c.irq());
            assert_eq!(p.events, ["start:34:false", "send:55", "end"]);
            assert_eq!(c.fault_count(), 0);
            // Idle is a level condition, even when its status is toggled.
            c.write(0x20, 4, 0x10);
            assert!(c.irq());
            // Starting a new transfer removes the condition; now clear it.
            c.write(0x108, 2, 0x168);
            c.write(0x20, 4, 0x10);
            assert!(!c.irq());
            c.step(&mut p);
            assert!(!c.irq());
        }
    }
}

#[test]
fn disabled_queued_start_does_not_claim_the_bus() {
    let mut c = initialized();
    let mut p = Peripheral::default();
    c.write(0x100, 1, 0);
    c.write(0x108, 2, 0x168);
    c.write(0x20, 4, 0x10);
    c.write(0x28, 4, 0x10);
    assert!(c.irq());
    assert_eq!(c.read(0x104, 1) & 4, 0);
    c.step(&mut p);
    assert!(p.events.is_empty());
    c.write(0x100, 1, 1);
    assert_eq!(c.read(0x104, 1) & 4, 4);
    c.write(0x20, 4, 0x10);
    assert!(!c.irq());
    c.step(&mut p);
    assert_eq!(p.events, ["start:34:false"]);
}

#[test]
fn programmed_receive_watermark_stretches_until_drain() {
    let mut c = initialized();
    let mut p = Peripheral {
        data: vec![9, 8, 7, 6],
        ..Default::default()
    };
    c.write(0x120, 1, 1);
    c.write(0x28, 4, 8);
    c.write(0x108, 2, 0x169);
    c.write(0x108, 2, 0x204);
    drain(&mut c, &mut p);
    assert_eq!(p.pos, 2);
    assert!(c.irq());
    c.write(0x20, 4, 8);
    assert!(c.irq()); // threshold condition still exists
    assert_eq!(c.read(0x10c, 1), 9);
    c.write(0x20, 4, 8);
    assert!(!c.irq());
    c.step(&mut p);
    assert_eq!(p.pos, 3);
    assert!(c.irq());
    assert_eq!(c.read(0x10c, 1), 8);
    c.step(&mut p);
    assert_eq!(c.read(0x10c, 1), 7);
    assert_eq!(c.read(0x10c, 1), 6);
    assert_eq!(p.events.last().unwrap(), "end");
}

#[test]
fn fifo_reset_held_stops_tx_and_reset_register_has_documented_default() {
    let mut c = Controller::default();
    let mut p = Peripheral::default();
    c.reset();
    assert_eq!(c.read(0x20, 4), 0xd0);
    c.write(0x100, 1, 3);
    c.write(0x108, 2, 0x168);
    c.step(&mut p);
    assert!(p.events.is_empty());
    assert!(!c.pending());
    c.write(0x100, 1, 1);
    c.step(&mut p);
    assert_eq!(p.events, ["start:34:false"]);
    c.write(0x28, 4, 4);
    c.write(0x1c, 4, 0x8000_0000);
    assert!(c.irq());
    c.write(0x20, 4, 4);
    assert!(c.irq()); // TX empty throttle cannot be cleared
    c.write(0x108, 2, 0x200);
    c.write(0x20, 4, 4);
    assert!(!c.irq());
    c.step(&mut p);
    assert_eq!(p.events.last().unwrap(), "end");
}

#[test]
fn unsupported_modes_raise_error_and_need_software_reset() {
    for (register, value) in [
        (0x100, 5),
        (0x100, 0x11),
        (0x100, 0x41),
        (0x110, 0x42),
        (0x11c, 1),
    ] {
        let mut c = initialized();
        let mut p = Peripheral::default();
        c.write(0x108, 2, 0x168);
        c.step(&mut p);
        c.write(0x28, 4, 2);
        c.write(register, 1, value);
        assert!(c.irq());
        c.write(0x20, 4, 2);
        assert!(c.irq());
        c.step(&mut p);
        assert_eq!(p.events, ["start:34:false", "end"]);
        c.write(0x100, 1, 1);
        c.write(0x108, 2, 0x168);
        c.step(&mut p);
        assert_eq!(p.events.len(), 2);
        assert!(!c.pending());
        c.write(0x40, 4, 10);
        c.write(0x100, 1, 1);
        c.write(0x108, 2, 0x168);
        c.step(&mut p);
        assert_eq!(p.events.last().unwrap(), "start:34:false");
        assert_eq!(p.events.len(), 3);
    }
}

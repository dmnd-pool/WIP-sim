use md_sim_firmware::s9io::{
    receive::{Control, Destination, Output, ReceiveFifos, COMMAND_WORDS, WORK_WORDS},
    Midstates,
};

const ENABLED: Control = Control {
    enabled: true,
    clear_command: false,
    clear_work: false,
    clear_errors: false,
    read_command: false,
    read_work: false,
};
const COMMAND: [u8; 7] = [0x13, 0x87, 0x90, 0, 0, 0, 7];
const WORK: [u8; 7] = [0x72, 3, 0xea, 0x83, 0, 3, 0x98];

fn tick(f: &mut ReceiveFifos, byte: Option<u8>) -> Output {
    f.tick(ENABLED, byte, 0x7f, Midstates::One)
}

fn input(f: &mut ReceiveFifos, bytes: &[u8]) {
    for &byte in bytes {
        assert!((0..100).any(|_| tick(f, Some(byte)).read_uart));
    }
}

/// Stop after checksum verification, before the first FIFO write.
fn checked(f: &mut ReceiveFifos, bytes: &[u8]) {
    input(f, bytes);
    for _ in 0..55 {
        let output = tick(f, None);
        assert!(!output.read_uart);
        assert!(output.write.is_none());
        assert!(!output.checksum_error);
    }
}

fn transfer(f: &mut ReceiveFifos, destination: Destination) {
    for _ in 0..2 {
        assert_eq!(tick(f, None).write, Some(destination));
    }
    assert!(tick(f, None).write.is_none());
}

#[test]
fn receive_fifo_interrupts_follow_clocked_checksum_and_two_word_completion() {
    let mut f = ReceiveFifos::default();
    assert!(
        !f.tick(Control::default(), Some(COMMAND[0]), 0, Midstates::One)
            .read_uart
    );
    assert_eq!(f.command_status(), 1);
    checked(&mut f, &COMMAND);
    assert_eq!(f.command_status(), 1);
    // Another UART byte is not removed during either word-write phase.
    let first = tick(&mut f, Some(0));
    assert!(!first.read_uart);
    assert_eq!(first.write, Some(Destination::Command));
    assert_eq!(f.command_status(), 0);
    let second = tick(&mut f, Some(0));
    assert!(!second.read_uart);
    assert_eq!(second.write, Some(Destination::Command));
    assert_eq!(f.command_status(), 16);
    assert_eq!(f.read_command(), Some(0x00908713));
    assert_eq!(f.command_status(), 0);
    assert_eq!(f.read_command(), Some(0x07000000));
    assert_eq!(f.command_status(), 1);
    assert_eq!(f.read_command(), None);
    checked(&mut f, &COMMAND);
    // Clear wins over concurrent FIFO write one. The pending word two is
    // still written on the next clock.
    let clearing = Control {
        clear_command: true,
        ..ENABLED
    };
    assert_eq!(
        f.tick(clearing, None, 0, Midstates::One).write,
        Some(Destination::Command)
    );
    assert_eq!(f.command_status(), 1);
    assert_eq!(tick(&mut f, None).write, Some(Destination::Command));
    assert_eq!(f.read_command(), Some(0x07000000));
    assert_eq!(f.read_command(), None);
    f.reset();
    assert_eq!(f.command_status(), 1);
    assert_eq!(f.work_status(), 1);
    assert!(
        !f.tick(Control::default(), Some(0), 0, Midstates::One)
            .read_uart
    );
}

#[test]
fn receive_fifo_capacity_and_hdl_cross_fifo_backpressure_are_preserved() {
    let mut f = ReceiveFifos::default();
    for _ in 0..WORK_WORDS / 2 {
        checked(&mut f, &WORK);
        transfer(&mut f, Destination::Work);
    }
    assert_eq!(f.work_status(), 18);
    checked(&mut f, &COMMAND);
    for _ in 0..100 {
        let output = tick(&mut f, Some(0xee));
        assert!(output.write.is_none() && !output.read_uart);
    }
    assert_eq!(f.command_status(), 1);
    let output = f.tick(
        Control {
            read_work: true,
            ..ENABLED
        },
        None,
        0x7f,
        Midstates::One,
    );
    assert_eq!(output.work_read, Some(0x83ea0372));
    // Freeing a full work FIFO cannot release the command path until the
    // next clock. Fullness was sampled before this read.
    assert!(output.write.is_none());
    transfer(&mut f, Destination::Command);
    assert_eq!(f.command_status(), 16);
    f.tick(
        Control {
            clear_work: true,
            ..ENABLED
        },
        None,
        0x7f,
        Midstates::One,
    );
    for _ in 1..COMMAND_WORDS / 2 {
        checked(&mut f, &COMMAND);
        transfer(&mut f, Destination::Command);
    }
    assert_eq!(f.command_status(), 18);
    // Command-full discards both writes, completes IRQ and adds no error.
    checked(&mut f, &[0x13, 0x87, 0x90, 0xf4, 0, 0, 0x1c]);
    transfer(&mut f, Destination::Command);
    assert_eq!(f.command_status(), 18);
    for _ in 0..COMMAND_WORDS / 2 {
        assert_eq!(f.read_command(), Some(0x00908713));
        assert_eq!(f.read_command(), Some(0x07000000));
    }
    assert_eq!(f.read_command(), None);
    assert_eq!(f.errors(), 0);
}

#[test]
fn extended_id_uses_transmit_state_at_second_write_after_checksum() {
    let reference: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/s9io-reference.json")).unwrap();
    let task = &reference["tasks"]["tc_work_id_5"];
    let bytes: Vec<_> = task["uart_data2"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect();
    let mut f = ReceiveFifos::default();
    checked(&mut f, &bytes);
    assert_eq!(
        f.tick(ENABLED, None, 0, Midstates::One).write,
        Some(Destination::Work)
    );
    assert_eq!(
        f.tick(
            ENABLED,
            None,
            task["fifo_data1"][0].as_u64().unwrap() as u16,
            Midstates::Four
        )
        .write,
        Some(Destination::Work)
    );
    assert_eq!(
        f.read_work(),
        Some(task["fifo_data2"][0].as_u64().unwrap() as u32)
    );
    assert_eq!(
        f.read_work(),
        Some(task["fifo_data2"][1].as_u64().unwrap() as u32)
    );
}

#[test]
fn crc_error_clock_clear_priority_and_sliding_recovery_follow_source() {
    let mut f = ReceiveFifos::default();
    checked(&mut f, &[0; 7]);
    assert_eq!(f.errors(), 0);
    let output = f.tick(
        Control {
            clear_errors: true,
            ..ENABLED
        },
        Some(0),
        0,
        Midstates::One,
    );
    assert!(output.checksum_error && !output.read_uart);
    assert_eq!(f.errors(), 0); // clear has priority on the error clock
    input(&mut f, &[0]);
    for _ in 0..55 {
        assert!(!tick(&mut f, None).checksum_error);
    }
    assert!(tick(&mut f, None).checksum_error);
    assert_eq!(f.errors(), 1);
    input(&mut f, &COMMAND);
    for _ in 0..57 {
        tick(&mut f, None);
    }
    assert_eq!(f.errors(), 7); // six shifted invalid windows, then valid frame
    assert_eq!(f.read_command(), Some(0x00908713));
    assert_eq!(f.read_command(), Some(0x07000000));
}

#[test]
fn disable_and_fifo_clear_preserve_current_clock_outputs() {
    let mut f = ReceiveFifos::default();
    checked(&mut f, &COMMAND);
    // HDL computes this write before forcing the next state to Idle.
    assert_eq!(
        f.tick(Control::default(), None, 0, Midstates::One).write,
        Some(Destination::Command)
    );
    assert!(f
        .tick(Control::default(), None, 0, Midstates::One)
        .write
        .is_none());
    assert_eq!(f.read_command(), Some(0x00908713));
    assert_eq!(f.read_command(), None);
    checked(&mut f, &COMMAND);
    tick(&mut f, None);
    let clear = Control {
        clear_command: true,
        ..ENABLED
    };
    f.tick(clear, None, 0, Midstates::One);
    // FIFO clear wins over data write; source IRQ assertion wins over clear.
    assert_eq!(f.command_status(), 17);
    assert_eq!(f.read_command(), None);
    assert_eq!(f.command_status(), 1);
}

#[test]
fn concurrent_cpu_read_uses_old_fifo_data_and_irq_write_priority() {
    let mut f = ReceiveFifos::default();
    checked(&mut f, &COMMAND);
    let read = Control {
        read_command: true,
        ..ENABLED
    };
    let first = f.tick(read, None, 0, Midstates::One);
    assert_eq!(first.command_read, None); // empty before the first write
    assert_eq!(first.write, Some(Destination::Command));
    assert_eq!(f.command_status(), 0);
    let second = f.tick(read, None, 0, Midstates::One);
    assert_eq!(second.command_read, Some(0x00908713));
    assert_eq!(second.write, Some(Destination::Command));
    assert_eq!(f.command_status(), 16); // write2 IRQ wins concurrent read-clear
    assert_eq!(f.read_command(), Some(0x07000000));

    for _ in 0..COMMAND_WORDS / 2 {
        checked(&mut f, &COMMAND);
        transfer(&mut f, Destination::Command);
    }
    checked(&mut f, &[0x13, 0x87, 0x90, 0xf4, 0, 0, 0x1c]);
    let first = f.tick(read, None, 0, Midstates::One);
    assert_eq!(first.command_read, Some(0x00908713));
    assert_eq!(first.write, Some(Destination::Command));
    // Full-at-edge discards new word1, despite the concurrent removal.
    assert_eq!(f.command_status(), 0);
    assert_eq!(tick(&mut f, None).write, Some(Destination::Command));
    assert_eq!(f.command_status(), 18);
    for index in 1..COMMAND_WORDS {
        assert_eq!(
            f.read_command(),
            Some(if index % 2 == 0 {
                0x00908713
            } else {
                0x07000000
            })
        );
    }
    assert_eq!(f.read_command(), Some(0x1c000000));
    assert_eq!(f.read_command(), None);
}

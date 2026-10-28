use super::*;
use crate::bm1397::{decode, Job};

fn packet(broadcast: bool, command: Command) -> Packet {
    Packet::Command { broadcast, command }
}

fn write(address: u8, register: u8, value: u32, broadcast: bool) -> Packet {
    packet(
        broadcast,
        Command::Write {
            address,
            register,
            value,
        },
    )
}

fn configured(addresses: &[u8]) -> IdentificationChain {
    let mut chain = IdentificationChain::with_control_support(
        addresses.len() as u16,
        ControlWriteSupport::RegisterStorage,
    )
    .unwrap();
    chain.power(true);
    chain.reset_line(false);
    chain
        .receive(&packet(
            true,
            Command::Inactive {
                address: 0,
                reserved: 0,
            },
        ))
        .unwrap();
    for &address in addresses {
        chain
            .receive(&packet(
                false,
                Command::SetAddress {
                    address,
                    reserved: 0,
                },
            ))
            .unwrap();
    }
    chain
}

#[test]
fn capability_is_explicit_and_real_frames_change_only_programmed_state() {
    let core = decode(&[0x55, 0xaa, 0x51, 9, 0, 0x3c, 0x80, 0, 0x80, 0x74, 0x10]).unwrap();
    let pll = decode(&[0x55, 0xaa, 0x51, 9, 0, 0x68, 0xc0, 0x70, 1, 0x11, 0]).unwrap();
    let mut strict = IdentificationChain::new(1).unwrap();
    strict.power(true);
    strict.reset_line(false);
    assert_eq!(strict.programmed_controls(0), None);
    for command in [&core, &pll] {
        assert_eq!(
            strict.receive(command),
            Err(IdentifyError::UnsupportedCommand)
        );
    }
    let mut chain = configured(&[0]);
    assert_eq!(
        chain.programmed_controls(0),
        Some(ProgrammedControls::default())
    );
    for command in [&core, &pll, &core, &pll] {
        assert!(chain.receive(command).unwrap().is_empty());
    }
    let state = chain.programmed_controls(0).unwrap();
    assert_eq!(state.core0_write, Some(0x74));
    assert_eq!(state.pll0_write, None);
    let parameter = state.pll3_write.unwrap();
    assert_eq!(parameter.written_word, 0xc0700111);
    assert_eq!(parameter.driver_integer_mhz(1), Ok(2800));
    // No assumed status/readback, even after repeated identical writes.
    for register in [0x3c, 0x40, 0x08, 0x68, 0x28, 0x18, 0x70] {
        assert_eq!(
            chain.receive(&packet(
                true,
                Command::Read {
                    address: 0,
                    register
                }
            )),
            Err(IdentifyError::UnsupportedCommand)
        );
    }
    let work = Job {
        id: 4,
        midstate_count: 1,
        starting_nonce: [0; 4],
        nbits: [0; 4],
        ntime: [0; 4],
        merkle_tail: [0; 4],
        midstates: [[0; 32]; 4],
    };
    assert_eq!(
        chain.receive(&Packet::Job {
            broadcast: false,
            work
        }),
        Err(IdentifyError::UnsupportedCommand)
    );
    assert_eq!(
        chain.receive(&write(0, 0x2c, 0x0600000f, true)),
        Err(IdentifyError::UnsupportedCommand)
    );
    assert_eq!(chain.programmed_controls(0), Some(state));
}

#[test]
fn addressed_and_broadcast_writes_keep_each_chip_and_unknown_field_distinct() {
    let mut chain = configured(&[0, 128]);
    for value in 0..=255u32 {
        assert!(chain
            .receive(&write(0, 0x3c, 0x80008000 | value, true))
            .unwrap()
            .is_empty());
        assert_eq!(
            chain.programmed_controls(0).unwrap().core0_write,
            Some(value as u8)
        );
        assert_eq!(
            chain.programmed_controls(1).unwrap().core0_write,
            Some(value as u8)
        );
    }
    chain.receive(&write(128, 0x3c, 0x80008074, false)).unwrap();
    chain.receive(&write(0, 0x08, 0x40480221, false)).unwrap();
    chain.receive(&write(128, 0x68, 0xc0700111, false)).unwrap();
    let first = chain.programmed_controls(0).unwrap();
    let second = chain.programmed_controls(1).unwrap();
    assert_eq!(
        (first.core0_write, second.core0_write),
        (Some(255), Some(0x74))
    );
    assert_eq!(first.pll0_write.unwrap().driver_integer_mhz(1), Ok(450));
    assert_eq!(first.pll3_write, None);
    assert_eq!(second.pll0_write, None);
    assert_eq!(second.pll3_write.unwrap().written_word, 0xc0700111);
    for register in [0x3c, 0x08, 0x68, 0x28, 0x18, 0x70] {
        let value = if register == 0x3c { 0x80008000 } else { 0 };
        assert!(chain
            .receive(&write(7, register, value, false))
            .unwrap()
            .is_empty());
    }
    assert_eq!(chain.programmed_controls(0), Some(first));
    assert_eq!(chain.programmed_controls(1), Some(second));
    assert_eq!(chain.programmed_controls(2), None);
    // A supplied bit31 remains only a written bit, including for invalid divisors.
    chain.receive(&write(128, 0x68, 0x80000000, false)).unwrap();
    assert_eq!(
        chain
            .programmed_controls(1)
            .unwrap()
            .pll3_write
            .unwrap()
            .driver_integer_mhz(1),
        Err(super::super::programmed::ArithmeticError::ZeroDivisor)
    );
}

#[test]
fn invalid_controls_and_ambiguous_addresses_do_not_partially_update() {
    let mut chain = configured(&[0, 0]);
    for register in [0x3c, 0x08, 0x68, 0x28, 0x18, 0x70] {
        let value = if register == 0x3c {
            0x80008074
        } else {
            0xc0700111
        };
        assert_eq!(
            chain.receive(&write(0, register, value, false)),
            Err(IdentifyError::AmbiguousAddress)
        );
    }
    for value in [0x80018074, 0x80000074, 0x00008074, 0x80009074] {
        assert_eq!(
            chain.receive(&write(0, 0x3c, value, true)),
            Err(IdentifyError::InvalidControl)
        );
    }
    for value in [0x80008174, 0xff, 0x1ff00ff] {
        assert_eq!(
            chain.receive(&write(0, 0x3c, value, true)),
            Err(IdentifyError::UnsupportedCommand)
        );
    }
    for chip in 0..2 {
        assert_eq!(
            chain.programmed_controls(chip),
            Some(ProgrammedControls::default())
        );
    }
    assert!(chain
        .receive(&write(0, 0x3c, 0x80008074, true))
        .unwrap()
        .is_empty());
    for chip in 0..2 {
        assert_eq!(
            chain.programmed_controls(chip).unwrap().core0_write,
            Some(0x74)
        );
    }
}

#[test]
fn reset_and_power_loss_remove_knowledge_without_creating_reset_values() {
    for power_loss in [false, true] {
        let mut chain = configured(&[0, 128]);
        for (register, value) in [
            (0x3c, 0x80008074),
            (0x08, 0x40480221),
            (0x68, 0xc0700111),
            (0x28, 0x0600000f),
            (0x18, 0x00007a31),
            (0x70, 0x0f0f0f00),
        ] {
            chain.receive(&write(0, register, value, true)).unwrap();
        }
        if power_loss {
            chain.power(false);
        } else {
            chain.reset_line(true);
        }
        for chip in 0..2 {
            assert_eq!(chain.programmed_controls(chip), None);
        }
        for (register, value) in [
            (0x3c, 0x800080b4),
            (0x08, 0x40480221),
            (0x68, 0xc0700111),
            (0x28, 0xffffffff),
            (0x18, 0xffffffff),
            (0x70, 0xffffffff),
        ] {
            assert!(chain
                .receive(&write(0, register, value, true))
                .unwrap()
                .is_empty());
        }
        chain.power(true);
        chain.reset_line(false);
        for chip in 0..2 {
            assert_eq!(
                chain.programmed_controls(chip),
                Some(ProgrammedControls::default())
            );
        }
    }
}

#[test]
fn uart_words_use_selected_banks_without_acknowledgement_or_readback() {
    // Original factory037 TX, including its independently checked CRC.
    let fast = decode(&[0x55, 0xaa, 0x51, 9, 0, 0x28, 6, 0, 0, 15, 0x18]).unwrap();
    let mut strict = IdentificationChain::new(1).unwrap();
    strict.power(true);
    strict.reset_line(false);
    for packet in [&fast, &write(0, 0x18, 0x00007a31, true)] {
        assert_eq!(
            strict.receive(packet),
            Err(IdentifyError::UnsupportedCommand)
        );
    }
    let mut chain = configured(&[0, 128]);
    assert!(chain.receive(&fast).unwrap().is_empty());
    assert!(chain
        .receive(&write(128, 0x18, 0x00007a31, false))
        .unwrap()
        .is_empty());
    let first = chain.programmed_controls(0).unwrap();
    let second = chain.programmed_controls(1).unwrap();
    assert_eq!(first.fast_uart_write, second.fast_uart_write);
    assert_eq!(first.fast_uart_write.unwrap().written_word, 0x0600000f);
    assert_eq!(first.misc_write, None);
    assert_eq!(second.misc_write.unwrap().baud_divider_field(), 26);
    assert_eq!(second.misc_write.unwrap().written_word, 0x00007a31);
    for register in [0x18, 0x28] {
        assert!(chain
            .receive(&write(1, register, 0xffffffff, false))
            .unwrap()
            .is_empty());
        assert_eq!(chain.programmed_controls(0), Some(first));
        assert_eq!(chain.programmed_controls(1), Some(second));
        for broadcast in [false, true] {
            assert_eq!(
                chain.receive(&packet(
                    broadcast,
                    Command::Read {
                        address: 128,
                        register
                    }
                )),
                Err(IdentifyError::UnsupportedCommand)
            );
        }
    }
    // Repeated writes replace the same raw bank, retaining all reserved bits.
    for value in [0xffffffff, 0, 0x0600000f, 0x0600000f] {
        assert!(chain
            .receive(&write(0, 0x28, value, true))
            .unwrap()
            .is_empty());
        assert_eq!(
            chain
                .programmed_controls(0)
                .unwrap()
                .fast_uart_write
                .unwrap()
                .written_word,
            value
        );
        assert_eq!(
            chain
                .programmed_controls(1)
                .unwrap()
                .fast_uart_write
                .unwrap()
                .written_word,
            value
        );
    }
    // The core-reset control bit and PLL source selection remain only posted
    // requests. They must not manufacture completed reset, lock, or line rate.
    chain.receive(&write(0, 0x3c, 0x80008074, true)).unwrap();
    chain.receive(&write(0, 0x68, 0xc0700111, true)).unwrap();
    for word in [0xffffffff, 0x00416731, 0x00416731] {
        assert!(chain
            .receive(&write(0, 0x18, word, true))
            .unwrap()
            .is_empty());
        for chip in 0..2 {
            let state = chain.programmed_controls(chip).unwrap();
            assert_eq!(state.core0_write, Some(0x74));
            assert_eq!(state.pll3_write.unwrap().written_word, 0xc0700111);
            assert_eq!(state.misc_write.unwrap().written_word, word);
            assert_eq!(
                state.misc_write.unwrap().requested_clki_baud(25_000_000),
                Err(super::super::uart_control::BaudRequestError::Pll3Unsupported)
            );
        }
    }
}

#[test]
fn original_divider_frame_and_all_low_bytes_preserve_silent_programmed_state() {
    use super::super::programmed::DividerError;
    // Original factory038 TX, not a predicted packet or generated reply.
    let frame = decode(&[0x55, 0xaa, 0x51, 9, 0, 0x70, 15, 15, 15, 0, 0x19]).unwrap();
    let mut strict = IdentificationChain::new(1).unwrap();
    strict.power(true);
    strict.reset_line(false);
    assert_eq!(
        strict.receive(&frame),
        Err(IdentifyError::UnsupportedCommand)
    );
    let mut chain = configured(&[0, 128]);
    for _ in 0..2 {
        assert!(chain.receive(&frame).unwrap().is_empty());
        for chip in 0..2 {
            let state = chain.programmed_controls(chip).unwrap();
            let raw = state.pll0_divider_write.unwrap();
            assert_eq!(raw.written_word, 0x0f0f0f00);
            assert_eq!(raw.driver_software_divider(), Ok(1));
            assert_eq!(state.pll0_write, None);
        }
    }
    let unchanged = chain.programmed_controls(0);
    for byte in 0..=255u32 {
        // Unknown upper-field semantics do not discard any received bit.
        let word = 0xa1b2c300 | byte;
        assert!(chain
            .receive(&write(128, 0x70, word, false))
            .unwrap()
            .is_empty());
        assert_eq!(chain.programmed_controls(0), unchanged);
        let raw = chain
            .programmed_controls(1)
            .unwrap()
            .pll0_divider_write
            .unwrap();
        assert_eq!(raw.written_word, word);
        let decoded = raw.driver_software_divider();
        if byte == 255 {
            assert_eq!(decoded, Err(DividerError::UnrepresentableSoftwareDivider));
        } else {
            assert_eq!(u32::from(decoded.unwrap()), byte + 1);
        }
    }
    // No readback or effective-clock side effect is created by a valid word.
    for broadcast in [false, true] {
        assert_eq!(
            chain.receive(&packet(
                broadcast,
                Command::Read {
                    address: 128,
                    register: 0x70
                }
            )),
            Err(IdentifyError::UnsupportedCommand)
        );
    }
}

#[test]
fn decoded_software_dividers_join_original_arithmetic_results() {
    use super::super::programmed::{Pll0Divider, PllParameter};
    // Original496 executed this complete divider sweep. It does not prove
    // silicon-valid output fields or timing; it checks the software view.
    let bytes =
        include_bytes!("../../../../simulator/firmware/fixtures/t17-pll-frequency-results.bin");
    let word = |b: &[u8]| u32::from_le_bytes(b.try_into().unwrap());
    let mut observed = [false; 256];
    for row in bytes[16..].chunks_exact(16) {
        let parameter = word(&row[4..8]);
        let software = word(&row[8..12]);
        if parameter != 0x00700221 || !(1..=255).contains(&software) {
            continue;
        }
        let posted = Pll0Divider {
            written_word: 0x0f0f0f00 | (software - 1),
        };
        let decoded = posted.driver_software_divider().unwrap();
        assert_eq!(
            PllParameter {
                written_word: parameter
            }
            .driver_integer_mhz(u32::from(decoded)),
            Ok(word(&row[12..16]))
        );
        observed[software as usize] = true;
    }
    assert!(observed[1..].iter().all(|x| *x));
}

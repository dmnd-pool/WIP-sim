//! Identification, addressing, ticket filters and clock-control storage.
//! Reset identity0x13971800 is reported in
//! skot/BM1397/registers.md; the retained provider view has SHA-256
//! 501c855b43ecf49a37575766454d1e4e439d0911f1b0c2f5bd2bef287642fe67.
//! It is community evidence, not a manufacturer specification or resolved
//! silicon stepping. The archived ESP-Miner and Gekko drivers both send
//! broadcast INACTIVE followed by one unicast SETADDRESS per chain position.
//! Assigned chips pass later address commands to the next inactive chip,
//! including when their assigned address is zero. This is a reference
//! chain policy; its electrical forwarding delay is not calibrated.
//! Unsupported commands fail visibly. This model cannot
//! initialize a mining chip, return a nonce, or establish a board gate.
//! Clock-order and enable writes retain per-chip state. Clock routing and
//! lock/status remain unsupported; register storage is not a clock model.
use super::programmed::{ProgrammedControls, Write};
use super::{Command, Packet, Reply};

#[cfg(test)]
#[path = "programmed_tests.rs"]
mod programmed_tests;

/// An explicit profile capability. RegisterStorage records posted settings;
/// it does not establish effective core/clock state or enable mining.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ControlWriteSupport {
    Unsupported,
    RegisterStorage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum IdentifyError {
    ChipCount,
    UnsupportedCommand,
    AmbiguousAddress,
    InvalidControl,
}

pub struct IdentificationChain {
    addresses: Vec<u8>,
    clock_registers: Vec<[u32; 3]>,
    ticket_registers: Vec<u32>,
    programmed_controls: Option<Vec<ProgrammedControls>>,
    /// None means normal operation. Some(index) is the next inactive chip.
    next_address: Option<usize>,
    powered: bool,
    reset_asserted: bool,
}

impl IdentificationChain {
    pub fn new(chips: u16) -> Result<Self, IdentifyError> {
        Self::with_control_support(chips, ControlWriteSupport::Unsupported)
    }

    pub fn with_control_support(
        chips: u16,
        support: ControlWriteSupport,
    ) -> Result<Self, IdentifyError> {
        if !(1..=256).contains(&chips) {
            return Err(IdentifyError::ChipCount);
        }
        Ok(Self {
            addresses: vec![0; usize::from(chips)],
            clock_registers: vec![[0xd95c8410, 0xfb73ea62, 0x0000ffff]; usize::from(chips)],
            ticket_registers: vec![0; usize::from(chips)],
            programmed_controls: (support == ControlWriteSupport::RegisterStorage)
                .then(|| vec![ProgrammedControls::default(); usize::from(chips)]),
            next_address: None,
            powered: false,
            reset_asserted: true,
        })
    }

    pub fn power(&mut self, powered: bool) {
        if !powered {
            self.reset_registers();
        }
        self.powered = powered;
    }

    pub fn reset_line(&mut self, asserted: bool) {
        if asserted {
            self.reset_registers();
        }
        self.reset_asserted = asserted;
    }

    fn reset_registers(&mut self) {
        self.addresses.fill(0);
        self.clock_registers
            .fill([0xd95c8410, 0xfb73ea62, 0x0000ffff]);
        self.ticket_registers.fill(0);
        if let Some(banks) = &mut self.programmed_controls {
            // Loss of programmed knowledge, not a claim of silicon defaults.
            banks.fill(ProgrammedControls::default());
        }
        self.next_address = None;
    }

    /// Physical chain position, not the assigned address. The raw word is
    /// shared by register readback and Job::engine_work. Power or reset
    /// prevents work dispatch. Register zero still requires 32 zero bits.
    pub fn ticket_register(&self, chip: usize) -> Option<u32> {
        if !self.powered || self.reset_asserted {
            return None;
        }
        self.ticket_registers.get(chip).copied()
    }

    /// Address by physical chain position. Power and reset gate access.
    /// This exposes stored addressing only, not chip readiness or routing.
    pub fn assigned_address(&self, chip: usize) -> Option<u8> {
        if !self.powered || self.reset_asserted {
            return None;
        }
        self.addresses.get(chip).copied()
    }

    /// Diagnostic state by physical chain position. It is not guest readback.
    /// None means no selected capability, no such chip, power off or reset.
    /// A bank with absent fields has no known posted configuration.
    pub fn programmed_controls(&self, chip: usize) -> Option<ProgrammedControls> {
        if !self.powered || self.reset_asserted {
            return None;
        }
        self.programmed_controls.as_ref()?.get(chip).copied()
    }

    pub fn receive(&mut self, packet: &Packet) -> Result<Vec<Reply>, IdentifyError> {
        if !self.powered || self.reset_asserted {
            return Ok(Vec::new());
        }
        match packet {
            Packet::Command {
                broadcast,
                command:
                    Command::Write {
                        address,
                        register,
                        value,
                    },
            } if self.programmed_controls.is_some()
                && matches!(register, 0x3c | 0x08 | 0x68 | 0x28 | 0x18 | 0x70) =>
            {
                let write = Write::decode(*register, *value)?;
                let selected: Vec<_> = self
                    .addresses
                    .iter()
                    .enumerate()
                    .filter_map(|(index, candidate)| {
                        (*broadcast || candidate == address).then_some(index)
                    })
                    .collect();
                if !broadcast && selected.len() > 1 {
                    return Err(IdentifyError::AmbiguousAddress);
                }
                let banks = self.programmed_controls.as_mut().unwrap();
                for index in selected {
                    write.apply(&mut banks[index]);
                }
                // No ACK, ready event, register reply or effective clock follows.
                Ok(Vec::new())
            }
            Packet::Command {
                broadcast,
                command: Command::Read { address, register },
            } if matches!(register, 0 | 0x14 | 0x20 | 0x80 | 0x84) => {
                let selected: Vec<_> = self
                    .addresses
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| *broadcast || *candidate == address)
                    .collect();
                if !broadcast && selected.len() > 1 {
                    return Err(IdentifyError::AmbiguousAddress);
                }
                Ok(selected
                    .into_iter()
                    .map(|(index, &address)| Reply::Register {
                        value: match register {
                            0 => 0x13971800 | u32::from(address),
                            0x14 => self.ticket_registers[index],
                            0x80 => self.clock_registers[index][0],
                            0x84 => self.clock_registers[index][1],
                            _ => self.clock_registers[index][2],
                        },
                        asic_address: address,
                        register: *register,
                    })
                    .collect())
            }
            Packet::Command {
                broadcast,
                command:
                    Command::Write {
                        address,
                        register,
                        value,
                    },
            } if matches!(register, 0x14 | 0x20 | 0x80 | 0x84) => {
                // Retain the addressed chip state. Clock writes do not
                // imply a clock output or lock indication.
                for (index, candidate) in self.addresses.iter().enumerate() {
                    if *broadcast || candidate == address {
                        if *register == 0x14 {
                            self.ticket_registers[index] = *value;
                            continue;
                        }
                        let slot = match register {
                            0x80 => 0,
                            0x84 => 1,
                            _ => 2,
                        };
                        self.clock_registers[index][slot] = *value;
                    }
                }
                Ok(Vec::new())
            }
            Packet::Command {
                broadcast: true,
                command:
                    Command::Inactive {
                        address: 0,
                        reserved: 0,
                    },
            } => {
                self.next_address = Some(0);
                Ok(Vec::new())
            }
            Packet::Command {
                broadcast: false,
                command:
                    Command::SetAddress {
                        address,
                        reserved: 0,
                    },
            } => {
                // The current source-backed contract starts assignment with
                // INACTIVE. Do not infer another reset-time assignment mode.
                let index = self.next_address.ok_or(IdentifyError::UnsupportedCommand)?;
                if let Some(slot) = self.addresses.get_mut(index) {
                    *slot = *address;
                    self.next_address = Some(index + 1);
                }
                Ok(Vec::new())
            }
            Packet::Command {
                command: Command::Inactive { .. } | Command::SetAddress { .. },
                ..
            } => Err(IdentifyError::InvalidControl),
            _ => Err(IdentifyError::UnsupportedCommand),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bm1397::decode;

    fn command(broadcast: bool, command: Command) -> Packet {
        Packet::Command { broadcast, command }
    }

    fn enumerate(chain: &mut IdentificationChain) -> Vec<Reply> {
        chain
            .receive(&command(
                true,
                Command::Read {
                    address: 0,
                    register: 0,
                },
            ))
            .unwrap()
    }

    #[test]
    fn ticket_writes_drive_readback_and_filter_and_reset() {
        let mut chain = IdentificationChain::new(2).unwrap();
        assert_eq!(chain.ticket_register(0), None);
        chain.power(true);
        assert_eq!(chain.ticket_register(0), None);
        chain.reset_line(false);
        chain
            .receive(&command(
                true,
                Command::Inactive {
                    address: 0,
                    reserved: 0,
                },
            ))
            .unwrap();
        for address in [0, 128] {
            chain
                .receive(&command(
                    false,
                    Command::SetAddress {
                        address,
                        reserved: 0,
                    },
                ))
                .unwrap();
        }
        // Archived get_difficulty_mask(256) programs this raw register.
        // Preserve noncontiguous bits too; no threshold rounding is allowed.
        for (broadcast, address, value) in [(true, 0, 0xff), (false, 128, 0x01020304)] {
            chain
                .receive(&command(
                    broadcast,
                    Command::Write {
                        address,
                        register: 0x14,
                        value,
                    },
                ))
                .unwrap();
        }
        for (chip, address, value) in [(0, 0, 0xff), (1, 128, 0x01020304)] {
            assert_eq!(chain.ticket_register(chip), Some(value));
            assert_eq!(
                chain
                    .receive(&command(
                        false,
                        Command::Read {
                            address,
                            register: 0x14
                        }
                    ))
                    .unwrap(),
                vec![Reply::Register {
                    value,
                    asic_address: address,
                    register: 0x14
                }]
            );
            let filter = super::super::ticket_filter(chain.ticket_register(chip).unwrap());
            for bit in 0..256 {
                let mut digest = [0; 32];
                digest[bit / 8] = 1 << (bit % 8);
                // Independent raw SHA-byte view of the published mask rule.
                let fixed = digest[28..].iter().any(|&b| b != 0);
                let ticket = u32::from_be_bytes(digest[24..28].try_into().unwrap()) & value;
                assert_eq!(filter.accepts(&digest), !fixed && ticket == 0);
            }
        }
        assert_eq!(chain.ticket_register(2), None);
        // No addressed chip: the write must not alter either filter.
        chain
            .receive(&command(
                false,
                Command::Write {
                    address: 7,
                    register: 0x14,
                    value: 0,
                },
            ))
            .unwrap();
        assert_eq!(chain.ticket_register(0), Some(0xff));
        assert_eq!(chain.ticket_register(1), Some(0x01020304));
        for power_loss in [false, true] {
            chain
                .receive(&command(
                    true,
                    Command::Write {
                        address: 0,
                        register: 0x14,
                        value: u32::MAX,
                    },
                ))
                .unwrap();
            if power_loss {
                chain.power(false);
            } else {
                chain.reset_line(true);
            }
            assert_eq!(chain.ticket_register(0), None);
            assert!(chain
                .receive(&command(
                    true,
                    Command::Write {
                        address: 0,
                        register: 0x14,
                        value: 123
                    }
                ))
                .unwrap()
                .is_empty());
            chain.power(true);
            chain.reset_line(false);
            assert_eq!(chain.ticket_register(0), Some(0));
            assert_eq!(chain.ticket_register(1), Some(0));
        }
        // This addition does not bypass the next unknown core command.
        assert_eq!(
            chain.receive(&command(
                true,
                Command::Write {
                    address: 0,
                    register: 0x3c,
                    value: 0x80008074
                }
            )),
            Err(IdentifyError::UnsupportedCommand)
        );
    }

    #[test]
    fn ordered_clock_enable_readback_is_per_chip_and_clears_on_power_loss_or_reset() {
        let mut chain = IdentificationChain::new(2).unwrap();
        chain.power(true);
        chain.reset_line(false);
        chain
            .receive(&command(
                true,
                Command::Inactive {
                    address: 0,
                    reserved: 0,
                },
            ))
            .unwrap();
        for address in [0, 128] {
            chain
                .receive(&command(
                    false,
                    Command::SetAddress {
                        address,
                        reserved: 0,
                    },
                ))
                .unwrap();
        }
        let read = |chain: &mut IdentificationChain, address| {
            chain
                .receive(&command(
                    false,
                    Command::Read {
                        address,
                        register: 0x20,
                    },
                ))
                .unwrap()
        };
        for address in [0, 128] {
            assert_eq!(
                read(&mut chain, address),
                vec![Reply::Register {
                    value: 0xffff,
                    asic_address: address,
                    register: 0x20
                }]
            );
        }
        // Original-probe033 UART records9 and10, including the guest CRC.
        let enable = decode(&[
            0x55, 0xaa, 0x51, 0x09, 0x00, 0x20, 0x00, 0x00, 0x00, 0x01, 0x02,
        ])
        .unwrap();
        assert!(chain.receive(&enable).unwrap().is_empty());
        chain
            .receive(&command(
                false,
                Command::Write {
                    address: 128,
                    register: 0x20,
                    value: 0x0100,
                },
            ))
            .unwrap();
        assert_eq!(
            read(&mut chain, 0),
            vec![Reply::Register {
                value: 1,
                asic_address: 0,
                register: 0x20
            }]
        );
        assert_eq!(
            read(&mut chain, 128),
            vec![Reply::Register {
                value: 0x0100,
                asic_address: 128,
                register: 0x20
            }]
        );
        assert_eq!(
            chain.receive(&command(
                true,
                Command::Read {
                    address: 0,
                    register: 0x8c
                }
            )),
            Err(IdentifyError::UnsupportedCommand)
        );
        for power_loss in [false, true] {
            if power_loss {
                chain.power(false);
                chain.power(true);
            } else {
                chain.reset_line(true);
                chain.reset_line(false);
            }
            assert_eq!(
                chain
                    .receive(&command(
                        true,
                        Command::Read {
                            address: 0,
                            register: 0x20
                        }
                    ))
                    .unwrap(),
                vec![
                    Reply::Register {
                        value: 0xffff,
                        asic_address: 0,
                        register: 0x20
                    };
                    2
                ]
            );
            chain
                .receive(&command(
                    true,
                    Command::Write {
                        address: 0,
                        register: 0x20,
                        value: 0,
                    },
                ))
                .unwrap();
        }
    }

    #[test]
    fn clock_order_storage_is_per_chip_and_reset_does_not_invent_clock_lock() {
        let mut chain = IdentificationChain::new(2).unwrap();
        chain.power(true);
        chain.reset_line(false);
        chain
            .receive(&command(
                true,
                Command::Inactive {
                    address: 0,
                    reserved: 0,
                },
            ))
            .unwrap();
        for address in [0, 128] {
            chain
                .receive(&command(
                    false,
                    Command::SetAddress {
                        address,
                        reserved: 0,
                    },
                ))
                .unwrap();
        }
        let read = |chain: &mut IdentificationChain, address, register| {
            chain
                .receive(&command(false, Command::Read { address, register }))
                .unwrap()
        };
        for address in [0, 128] {
            for (register, value) in [(0x80, 0xd95c8410), (0x84, 0xfb73ea62)] {
                assert_eq!(
                    read(&mut chain, address, register),
                    vec![Reply::Register {
                        value,
                        asic_address: address,
                        register
                    }]
                );
            }
        }
        // Actual original-probe032 frame, including its observed checksum.
        let clear = decode(&[
            0x55, 0xaa, 0x51, 0x09, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x1c,
        ])
        .unwrap();
        assert!(chain.receive(&clear).unwrap().is_empty());
        chain
            .receive(&command(
                false,
                Command::Write {
                    address: 128,
                    register: 0x84,
                    value: 0,
                },
            ))
            .unwrap();
        assert_eq!(
            read(&mut chain, 0, 0x84),
            vec![Reply::Register {
                value: 0xfb73ea62,
                asic_address: 0,
                register: 0x84
            }]
        );
        assert_eq!(
            read(&mut chain, 128, 0x84),
            vec![Reply::Register {
                value: 0,
                asic_address: 128,
                register: 0x84
            }]
        );
        assert_eq!(
            read(&mut chain, 128, 0x80),
            vec![Reply::Register {
                value: 0,
                asic_address: 128,
                register: 0x80
            }]
        );
        assert_eq!(
            chain.receive(&command(
                true,
                Command::Write {
                    address: 0,
                    register: 0x08,
                    value: 1
                }
            )),
            Err(IdentifyError::UnsupportedCommand)
        );
        assert_eq!(
            chain.receive(&command(
                true,
                Command::Read {
                    address: 0,
                    register: 0x8c
                }
            )),
            Err(IdentifyError::UnsupportedCommand)
        );
        for power in [false, true] {
            if power {
                chain.power(false);
                chain.power(true);
            } else {
                chain.reset_line(true);
                chain.reset_line(false);
            }
            assert_eq!(
                chain
                    .receive(&command(
                        true,
                        Command::Read {
                            address: 0,
                            register: 0x80
                        }
                    ))
                    .unwrap(),
                vec![
                    Reply::Register {
                        value: 0xd95c8410,
                        asic_address: 0,
                        register: 0x80
                    };
                    2
                ]
            );
        }
    }

    #[test]
    fn chain_positions_receive_distinct_addresses_including_zero() {
        // Both archived drivers split all256 logical addresses across the
        // detected chain. This checks full enumeration and each unicast reply.
        for count in [1u16, 2, 3, 48, 256] {
            let mut chain = IdentificationChain::new(count).unwrap();
            chain.power(true);
            chain.reset_line(false);
            let inactive = decode(&[0x55, 0xaa, 0x53, 5, 0, 0, 3]).unwrap();
            assert!(chain.receive(&inactive).unwrap().is_empty());
            for position in 0..count {
                let address = (position * (256 / count)) as u8;
                assert!(chain
                    .receive(&command(
                        false,
                        Command::SetAddress {
                            address,
                            reserved: 0
                        }
                    ))
                    .unwrap()
                    .is_empty());
            }
            let replies = enumerate(&mut chain);
            assert_eq!(replies.len(), usize::from(count));
            for (position, reply) in replies.into_iter().enumerate() {
                let address = (position * (256 / usize::from(count))) as u8;
                let expected = Reply::Register {
                    value: 0x13971800 | u32::from(address),
                    asic_address: address,
                    register: 0,
                };
                assert_eq!(reply, expected);
                assert_eq!(
                    chain
                        .receive(&command(
                            false,
                            Command::Read {
                                address,
                                register: 0
                            }
                        ))
                        .unwrap(),
                    vec![expected]
                );
            }
            let before = enumerate(&mut chain);
            // An extra assignment passes the final chip without changing it.
            assert!(chain
                .receive(&command(
                    false,
                    Command::SetAddress {
                        address: 255,
                        reserved: 0
                    }
                ))
                .unwrap()
                .is_empty());
            assert_eq!(enumerate(&mut chain), before);
            chain.reset_line(true);
            assert!(enumerate(&mut chain).is_empty());
            chain.reset_line(false);
            assert_eq!(
                enumerate(&mut chain),
                vec![
                    Reply::Register {
                        value: 0x13971800,
                        asic_address: 0,
                        register: 0
                    };
                    usize::from(count)
                ]
            );
        }
    }

    #[test]
    fn reassignment_power_loss_and_invalid_controls_do_not_hide_aliases() {
        let mut chain = IdentificationChain::new(2).unwrap();
        chain.power(true);
        chain.reset_line(false);
        let set = |address| {
            command(
                false,
                Command::SetAddress {
                    address,
                    reserved: 0,
                },
            )
        };
        assert_eq!(
            chain.receive(&set(7)),
            Err(IdentifyError::UnsupportedCommand)
        );
        let inactive = command(
            true,
            Command::Inactive {
                address: 0,
                reserved: 0,
            },
        );
        chain.receive(&inactive).unwrap();
        chain.receive(&set(7)).unwrap();
        chain.receive(&set(7)).unwrap();
        assert_eq!(
            chain.receive(&command(
                false,
                Command::Read {
                    address: 7,
                    register: 0
                }
            )),
            Err(IdentifyError::AmbiguousAddress)
        );
        let before = enumerate(&mut chain);
        for packet in [
            command(
                false,
                Command::Inactive {
                    address: 0,
                    reserved: 0,
                },
            ),
            command(
                true,
                Command::Inactive {
                    address: 0,
                    reserved: 1,
                },
            ),
            command(
                true,
                Command::SetAddress {
                    address: 0,
                    reserved: 0,
                },
            ),
            command(
                false,
                Command::SetAddress {
                    address: 0,
                    reserved: 1,
                },
            ),
        ] {
            assert_eq!(chain.receive(&packet), Err(IdentifyError::InvalidControl));
            assert_eq!(enumerate(&mut chain), before);
        }
        chain.receive(&inactive).unwrap();
        chain.receive(&set(0)).unwrap();
        chain.receive(&set(128)).unwrap();
        assert_eq!(
            chain
                .receive(&command(
                    false,
                    Command::Read {
                        address: 128,
                        register: 0
                    }
                ))
                .unwrap(),
            vec![Reply::Register {
                value: 0x13971880,
                asic_address: 128,
                register: 0
            }]
        );
        chain.power(false);
        assert!(enumerate(&mut chain).is_empty());
        chain.power(true);
        assert_eq!(
            enumerate(&mut chain),
            vec![
                Reply::Register {
                    value: 0x13971800,
                    asic_address: 0,
                    register: 0
                };
                2
            ]
        );
        assert_eq!(
            chain.receive(&set(1)),
            Err(IdentifyError::UnsupportedCommand)
        );
    }

    #[test]
    fn identification_obeys_power_reset_and_actual_query_without_unknown_registers() {
        let query = decode(&[0x55, 0xaa, 0x52, 0x05, 0, 0, 0x0a]).unwrap();
        for count in [1, 2, 48, 256] {
            let mut chain = IdentificationChain::new(count).unwrap();
            assert!(chain.receive(&query).unwrap().is_empty());
            chain.power(true);
            assert!(chain.receive(&query).unwrap().is_empty());
            chain.reset_line(false);
            let replies = chain.receive(&query).unwrap();
            assert_eq!(replies.len(), usize::from(count));
            for reply in replies {
                let bytes = reply.encode().unwrap();
                assert_eq!(&bytes[..8], &[0xaa, 0x55, 0x13, 0x97, 0x18, 0, 0, 0]);
                assert_eq!(super::super::crc5(&bytes[2..]), 0);
            }
            let absent = Packet::Command {
                broadcast: false,
                command: Command::Read {
                    address: 1,
                    register: 0,
                },
            };
            assert!(chain.receive(&absent).unwrap().is_empty());
            if count > 1 {
                let ambiguous = Packet::Command {
                    broadcast: false,
                    command: Command::Read {
                        address: 0,
                        register: 0,
                    },
                };
                assert_eq!(
                    chain.receive(&ambiguous),
                    Err(IdentifyError::AmbiguousAddress)
                );
            }
            for command in [
                Command::Read {
                    address: 0,
                    register: 4,
                },
                Command::Write {
                    address: 0,
                    register: 0x3c,
                    value: 0x80008074,
                },
            ] {
                assert_eq!(
                    chain.receive(&Packet::Command {
                        broadcast: true,
                        command
                    }),
                    Err(IdentifyError::UnsupportedCommand)
                );
            }
            chain.reset_line(true);
            assert!(chain.receive(&query).unwrap().is_empty());
            chain.reset_line(false);
            chain.power(false);
            assert!(chain.receive(&query).unwrap().is_empty());
        }
        assert!(IdentificationChain::new(0).is_err());
        assert!(IdentificationChain::new(257).is_err());
    }
}

//! One-shot reference adaptation for discovering the next firmware command.
//! This is probe control, not an ASIC core-register implementation. The strict
//! chain remains unchanged. No reply, register readback, timing or clock state
//! follows from observing the one exact factory035 write.
use md_sim_firmware::bm1397::{Command, IdentificationChain, IdentifyError, Packet, Reply};
use serde::Serialize;

pub const ADAPTATION: &str = "bm1397-factory035-core0-write-intent-v1";

#[derive(Default)]
pub struct Observer {
    observed: bool,
}

#[derive(Debug, Serialize)]
pub struct WriteIntent {
    pub adaptation: &'static str,
    pub word: u32,
    pub core_register: u8,
    pub data: u8,
    pub response_bytes: usize,
    pub silicon_effects_modeled: bool,
}

pub struct Reception {
    pub replies: Vec<Reply>,
    pub intent: Option<WriteIntent>,
}

pub fn receive(
    chain: &mut IdentificationChain,
    observer: Option<&mut Observer>,
    packet: &Packet,
) -> Result<Reception, IdentifyError> {
    match chain.receive(packet) {
        Ok(replies) => Ok(Reception {
            replies,
            intent: None,
        }),
        Err(IdentifyError::UnsupportedCommand)
            if matches!(
                packet,
                Packet::Command {
                    broadcast: true,
                    command: Command::Write {
                        address: 0,
                        register: 0x3c,
                        value: 0x8000_8074
                    }
                }
            ) =>
        {
            let observer = observer.ok_or(IdentifyError::UnsupportedCommand)?;
            if observer.observed {
                return Err(IdentifyError::UnsupportedCommand);
            }
            observer.observed = true;
            Ok(Reception {
                replies: Vec::new(),
                intent: Some(WriteIntent {
                    adaptation: ADAPTATION,
                    word: 0x8000_8074,
                    core_register: 0,
                    data: 0x74,
                    response_bytes: 0,
                    silicon_effects_modeled: false,
                }),
            })
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use md_sim_firmware::bm1397::decode;

    fn exact() -> Packet {
        decode(&[0x55, 0xaa, 0x51, 9, 0, 0x3c, 0x80, 0, 0x80, 0x74, 0x10]).unwrap()
    }
    fn ready() -> IdentificationChain {
        let mut chain = IdentificationChain::new(1).unwrap();
        chain.power(true);
        chain.reset_line(false);
        chain
    }
    fn write(register: u8, value: u32) -> Packet {
        Packet::Command {
            broadcast: true,
            command: Command::Write {
                address: 0,
                register,
                value,
            },
        }
    }
    #[test]
    fn default_stays_strict_and_observer_sends_no_ack_or_register_state() {
        let mut chain = ready();
        assert!(matches!(
            receive(&mut chain, None, &exact()),
            Err(IdentifyError::UnsupportedCommand)
        ));
        let mut observer = Observer::default();
        let observed = receive(&mut chain, Some(&mut observer), &exact()).unwrap();
        assert!(observed.replies.is_empty());
        let intent = observed.intent.unwrap();
        assert_eq!(
            (intent.word, intent.core_register, intent.data),
            (0x80008074, 0, 0x74)
        );
        assert_eq!(intent.response_bytes, 0);
        assert!(!intent.silicon_effects_modeled);
        assert_eq!(chain.ticket_register(0), Some(0));
        for register in [0x3c, 0x40] {
            let read = Packet::Command {
                broadcast: true,
                command: Command::Read {
                    address: 0,
                    register,
                },
            };
            assert!(matches!(
                receive(&mut chain, Some(&mut observer), &read),
                Err(IdentifyError::UnsupportedCommand)
            ));
        }
        assert!(matches!(
            chain.receive(&exact()),
            Err(IdentifyError::UnsupportedCommand)
        ));
    }
    #[test]
    fn one_shot_does_not_expand_to_other_words_addresses_or_repeats() {
        let mut chain = ready();
        let mut observer = Observer::default();
        let mut candidates = vec![
            write(0x3c, 0x800080b4),
            write(0x3c, 0x800000ff),
            write(0x3c, 0x80018074),
            write(0x3c, 0x80008174),
        ];
        candidates.push(Packet::Command {
            broadcast: false,
            command: Command::Write {
                address: 0,
                register: 0x3c,
                value: 0x80008074,
            },
        });
        candidates.push(Packet::Command {
            broadcast: true,
            command: Command::Write {
                address: 1,
                register: 0x3c,
                value: 0x80008074,
            },
        });
        for packet in candidates {
            assert!(matches!(
                receive(&mut chain, Some(&mut observer), &packet),
                Err(IdentifyError::UnsupportedCommand)
            ));
        }
        assert!(receive(&mut chain, Some(&mut observer), &exact())
            .unwrap()
            .intent
            .is_some());
        assert!(matches!(
            receive(&mut chain, Some(&mut observer), &exact()),
            Err(IdentifyError::UnsupportedCommand)
        ));
    }
    #[test]
    fn power_reset_and_next_unsupported_boundary_remain_strict() {
        let mut chain = IdentificationChain::new(1).unwrap();
        let mut observer = Observer::default();
        assert!(receive(&mut chain, Some(&mut observer), &exact())
            .unwrap()
            .intent
            .is_none());
        chain.power(true);
        assert!(receive(&mut chain, Some(&mut observer), &exact())
            .unwrap()
            .intent
            .is_none());
        chain.reset_line(false);
        assert!(receive(&mut chain, Some(&mut observer), &exact())
            .unwrap()
            .intent
            .is_some());
        // The next archived ESP commands can be observed without bypassing PLL3.
        let ticket = receive(&mut chain, Some(&mut observer), &write(0x14, 0xff)).unwrap();
        assert!(ticket.intent.is_none() && ticket.replies.is_empty());
        assert_eq!(chain.ticket_register(0), Some(0xff));
        assert!(matches!(
            receive(&mut chain, Some(&mut observer), &write(0x68, 0xc0700111)),
            Err(IdentifyError::UnsupportedCommand)
        ));
        for power_cycle in [false, true] {
            if power_cycle {
                chain.power(false);
                chain.power(true);
            } else {
                chain.reset_line(true);
                chain.reset_line(false);
            }
            assert_eq!(chain.ticket_register(0), Some(0));
            // A reset invalidates chip state; it does not grant another bypass.
            assert!(matches!(
                receive(&mut chain, Some(&mut observer), &exact()),
                Err(IdentifyError::UnsupportedCommand)
            ));
        }
    }
}

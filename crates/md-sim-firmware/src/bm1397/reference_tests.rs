use super::*;
use bitcoin_hashes::{sha256d, Hash};
use md_sim_mining::{work::header_midstate, Header};

fn ready(address: u8, slots: usize) -> ReferenceChain {
    let mut chain = ReferenceChain::new(1, 10_000.0, slots).unwrap();
    chain.power(true);
    chain.reset_line(false);
    chain
        .receive(&Packet::Command {
            broadcast: true,
            command: Command::Inactive {
                address: 0,
                reserved: 0,
            },
        })
        .unwrap();
    chain
        .receive(&Packet::Command {
            broadcast: false,
            command: Command::SetAddress {
                address,
                reserved: 0,
            },
        })
        .unwrap();
    chain
}
fn packet(id: u8, header: &Header, count: u8) -> Packet {
    let mut job = Job {
        id,
        midstate_count: count,
        starting_nonce: header.nonce.to_le_bytes(),
        nbits: header.bits.to_le_bytes(),
        ntime: header.time.to_le_bytes(),
        merkle_tail: header.merkle_root[28..].try_into().unwrap(),
        midstates: [[0; 32]; 4],
    };
    for i in 0..usize::from(count.min(4)) {
        let full = Header {
            version: header.version ^ i as u32,
            ..header.clone()
        };
        for (bytes, word) in job.midstates[i]
            .chunks_exact_mut(4)
            .zip(header_midstate(&full).into_iter().rev())
        {
            bytes.copy_from_slice(&word.to_le_bytes());
        }
    }
    Packet::Job {
        broadcast: false,
        work: job,
    }
}
fn test_header() -> Header {
    Header {
        version: 0x2000_0000,
        previous: [0x71; 32],
        merkle_root: [0x31; 32],
        time: 1_730_001_234,
        bits: 0x207f_ffff,
        nonce: 0,
    }
}
fn next(chain: &mut ReferenceChain) -> Step {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(step) = chain.poll() {
            return step;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(chain.wait_duration());
    }
}
fn write_ticket(value: u32) -> Packet {
    Packet::Command {
        broadcast: true,
        command: Command::Write {
            address: 0,
            register: 0x14,
            value,
        },
    }
}

#[test]
fn one_chip_consumer_accepts_every_nonce_address_field() {
    let address_interval = 256u32; // Exact C int division256/chip_counter, one chip.
    for field in 0u32..=255 {
        let nonce = (field << 17).swap_bytes();
        assert_eq!(((nonce.swap_bytes() >> 17) & 255) / address_interval, 0);
    }
    let mut nonzero = ready(64, 1);
    assert_eq!(
        nonzero.receive(&packet(4, &test_header(), 1)),
        Err(Error::UnsupportedAddressAssignment)
    );
}

#[test]
fn programmed_filter_is_read_back_and_all_actual_hashes_feed_the_ledger() {
    let mut chain = ready(0, 2);
    let header = test_header();
    chain.receive(&write_ticket(0xffff_ffff)).unwrap();
    chain.receive(&packet(4, &header, 1)).unwrap();
    let mut nonces = Vec::new();
    for ordinal in 1..=12 {
        let step = next(&mut chain);
        assert_eq!(step.job_computation_ordinal, ordinal);
        let full = Header {
            nonce: step.computation.nonce,
            ..header.clone()
        };
        let independent = sha256d::Hash::hash(&full.encode()).to_byte_array();
        assert_eq!(step.computation.digest, independent);
        assert_eq!(step.computation.filter, ticket_filter(u32::MAX));
        assert_eq!(((step.computation.nonce.swap_bytes() >> 17) & 255) / 256, 0);
        assert!(!step.computation.accepted);
        assert!(step.reply.is_none());
        nonces.push(step.computation.nonce);
    }
    assert_eq!(nonces, (0..12).collect::<Vec<_>>());
    assert_eq!(chain.hashes_computed(), 12);
    assert_eq!(chain.measurement().epoch_computations, 12);
    assert_eq!(
        chain
            .receive(&Packet::Command {
                broadcast: true,
                command: Command::Read {
                    address: 0,
                    register: 0x14
                }
            })
            .unwrap(),
        vec![Reply::Register {
            value: u32::MAX,
            asic_address: 0,
            register: 0x14
        }]
    );
    chain.receive(&write_ticket(0)).unwrap();
    let next = next(&mut chain);
    assert_eq!(next.computation.nonce, 12);
    assert_eq!(next.computation.filter, ticket_filter(0));
    let future = chain.ledger.origin + Duration::from_secs(1);
    let sample = chain.ledger.snapshot(future);
    assert_eq!(sample.last_window_computations, Some(13));
    assert_eq!(sample.register_0x04, Some(0)); // 13 real hashes / 2^20.
    let idle = chain.ledger.snapshot(future + Duration::from_secs(2));
    assert_eq!(idle.last_window_computations, Some(0));
    assert_eq!(idle.epoch_computations, 13);
}

#[test]
fn fixed_genesis_solution_uses_actual_hash_and_real_reply_without_filter_override() {
    // Required fixed-work regression. This is not fresh firmware mining.
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../fixtures/bitcoin/genesis-header.json"
    ))
    .unwrap();
    let header =
        Header::decode(&hex::decode(value["header_hex"].as_str().unwrap()).unwrap()).unwrap();
    let address = 0;
    let mut chain = ready(address, 1);
    chain.receive(&packet(4, &header, 1)).unwrap();
    let step = next(&mut chain);
    assert_eq!(step.computation.nonce, header.nonce);
    assert_eq!(
        step.computation.digest,
        sha256d::Hash::hash(&header.encode()).to_byte_array()
    );
    let bytes = step.reply.unwrap().encode().unwrap();
    assert_eq!(&bytes[2..6], &header.nonce.to_le_bytes());
    assert_eq!(bytes[6], 0);
    assert_eq!(bytes[7], 4);
    assert_eq!(chain.hashes_computed(), 1);
}

#[test]
fn bounded_replacement_reset_and_address_changes_do_not_credit_work() {
    let header = test_header();
    let mut chain = ready(0, 1);
    chain.receive(&packet(4, &header, 1)).unwrap();
    let first = next(&mut chain);
    for id in (128..=252).step_by(4) {
        assert_eq!(
            chain.receive(&packet(id, &header, 1)),
            Err(Error::UnsupportedJobId)
        );
        assert_eq!(chain.queue_len(), 1);
        assert_eq!(chain.hashes_computed(), 1);
        assert_eq!(
            chain.jobs.get(&4).unwrap().engine_id,
            first.computation.work_id
        );
    }

    assert_eq!(chain.receive(&packet(8, &header, 1)), Err(Error::QueueFull));
    assert_eq!(
        chain.receive(&packet(5, &header, 1)),
        Err(Error::InvalidJob)
    );
    assert_eq!(chain.queue_len(), 1);
    chain
        .receive(&packet(
            4,
            &Header {
                nonce: 100,
                ..header.clone()
            },
            1,
        ))
        .unwrap();
    let replaced = next(&mut chain);
    assert_ne!(first.computation.work_id, replaced.computation.work_id);
    assert_eq!(first.job_computation_ordinal, 1);
    assert_eq!(replaced.job_computation_ordinal, 1);
    assert_eq!(replaced.computation.nonce, 100);
    chain
        .receive(&Packet::Command {
            broadcast: true,
            command: Command::Inactive {
                address: 0,
                reserved: 0,
            },
        })
        .unwrap();
    assert_eq!(chain.queue_len(), 0);
    assert!(chain.poll().is_none());
    assert_eq!(chain.measurement().epoch_computations, 2);
    chain
        .receive(&Packet::Command {
            broadcast: false,
            command: Command::SetAddress {
                address: 64,
                reserved: 0,
            },
        })
        .unwrap();
    assert_eq!(
        chain.receive(&packet(4, &header, 1)),
        Err(Error::UnsupportedAddressAssignment)
    );
    chain.reset_line(true);
    assert_eq!(chain.queue_len(), 0);
    assert_eq!(chain.measurement().epoch_computations, 0);
    assert!(chain.receive(&packet(4, &header, 1)).unwrap().is_empty());
    assert!(chain.poll().is_none());
    assert_eq!(chain.hashes_computed(), 2);
    chain.reset_line(false);
    chain.power(false);
    assert!(chain.receive(&packet(4, &header, 1)).unwrap().is_empty());
    assert!(ReferenceChain::new(2, 100.0, 1).is_err());
    assert!(ReferenceChain::new(1, 10_001.0, 1).is_err());
    assert!(ReferenceChain::new(1, 100.0, 33).is_err());
}

#[test]
fn default_chain_retains_counter_and_job_failures() {
    let mut strict = IdentificationChain::new(1).unwrap();
    strict.power(true);
    strict.reset_line(false);
    for register in [4, 0x4c] {
        assert_eq!(
            strict.receive(&Packet::Command {
                broadcast: true,
                command: Command::Read {
                    address: 0,
                    register
                }
            }),
            Err(super::super::IdentifyError::UnsupportedCommand)
        );
    }
    assert_eq!(
        strict.receive(&packet(4, &test_header(), 1)),
        Err(super::super::IdentifyError::UnsupportedCommand)
    );
}

#[test]
fn actual_datapath_faults_drop_computed_solutions_and_reset_the_event_epoch() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../fixtures/bitcoin/genesis-header.json"
    ))
    .unwrap();
    let header =
        Header::decode(&hex::decode(value["header_hex"].as_str().unwrap()).unwrap()).unwrap();
    let address = 0;
    let mut chain = ReferenceChain::with_fault_policy(
        1,
        10_000.0,
        1,
        FaultPolicy::DropEveryNthComputation { interval: 2 },
    )
    .unwrap();
    chain.power(true);
    chain.reset_line(false);
    chain
        .receive(&Packet::Command {
            broadcast: true,
            command: Command::Inactive {
                address: 0,
                reserved: 0,
            },
        })
        .unwrap();
    chain
        .receive(&Packet::Command {
            broadcast: false,
            command: Command::SetAddress {
                address,
                reserved: 0,
            },
        })
        .unwrap();
    let read = Packet::Command {
        broadcast: true,
        command: Command::Read {
            address: 0,
            register: 0x4c,
        },
    };
    for ordinal in 1..=3 {
        chain.receive(&packet(4, &header, 1)).unwrap();
        let step = next(&mut chain);
        assert_eq!(step.computation.nonce, header.nonce);
        assert_eq!(
            step.computation.digest,
            sha256d::Hash::hash(&header.encode()).to_byte_array()
        );
        assert!(step.computation.accepted);
        assert_eq!(step.datapath_dropped, ordinal == 2);
        assert_eq!(step.reply.is_some(), ordinal != 2);
    }
    for _ in 0..2 {
        assert_eq!(
            chain.receive(&read).unwrap(),
            vec![Reply::Register {
                value: 1,
                asic_address: address,
                register: 0x4c
            }]
        );
    }
    assert_eq!(chain.hashes_computed(), 3);
    assert_eq!(chain.measurement().datapath_drop_events, 1);
    chain.reset_line(true);
    chain.reset_line(false);
    assert_eq!(
        chain.receive(&read).unwrap(),
        vec![Reply::Register {
            value: 0,
            asic_address: 0,
            register: 0x4c
        }]
    );
    assert_eq!(chain.measurement().epoch_computations, 0);
    assert_eq!(chain.hashes_computed(), 3);
    assert!(ReferenceChain::with_fault_policy(
        1,
        1.0,
        1,
        FaultPolicy::DropEveryNthComputation { interval: 0 }
    )
    .is_err());
}

#[test]
fn filter_misses_invalid_commands_and_no_work_do_not_become_fault_events() {
    let mut chain = ready(0, 1);
    assert!(chain.poll().is_none());
    chain.receive(&write_ticket(u32::MAX)).unwrap();
    chain.receive(&packet(4, &test_header(), 1)).unwrap();
    for _ in 0..4 {
        let step = next(&mut chain);
        assert!(!step.computation.accepted);
        assert!(!step.datapath_dropped);
    }
    let unknown = Packet::Command {
        broadcast: true,
        command: Command::Read {
            address: 0,
            register: 0xff,
        },
    };
    assert!(chain.receive(&unknown).is_err());
    let mut invalid_crc = [0x55, 0xaa, 0x52, 5, 0, 0, 0x0a];
    invalid_crc[6] ^= 1;
    assert!(super::super::decode(&invalid_crc).is_err());
    assert_eq!(chain.measurement().datapath_drop_events, 0);
    assert_eq!(chain.measurement().epoch_computations, 4);
    assert!(matches!(chain.policy().fault_policy, FaultPolicy::NoFaults));
}

#[test]
fn one_chip_four_midstate_range_really_exhausts_at_word_boundary() {
    let mut chain = ready(0, 1);
    let header = Header {
        nonce: u32::MAX,
        ..test_header()
    };
    chain.receive(&packet(4, &header, 4)).unwrap();
    for index in 0..4 {
        let step = next(&mut chain);
        let full = Header {
            version: header.version ^ index as u32,
            ..header.clone()
        };
        assert_eq!(
            step.computation.variant,
            ComputedVariant::Midstate { index }
        );
        assert_eq!(step.computation.nonce, u32::MAX);
        assert_eq!(
            step.computation.digest,
            sha256d::Hash::hash(&full.encode()).to_byte_array()
        );
        assert_eq!(step.computation.exhausted, index == 3);
    }
    assert_eq!(chain.queue_len(), 0);
    assert!(chain.poll().is_none());
    assert_eq!(chain.hashes_computed(), 4);
}

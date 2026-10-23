use bitcoin_hashes::{sha256, sha256d, Hash, HashEngine};
use md_sim_firmware::{
    bm1387::{
        budget::Budget,
        chain::{Chain, Error, Profile},
        wire::{Trace, Wire},
        Command, Job, Packet,
    },
    bm1397::{crc16, crc5},
};
use md_sim_mining::Header;
use std::{
    io::Read,
    time::{Duration, Instant},
};

fn profile(chips: u16, cores: u16) -> Profile {
    Profile {
        chips,
        cores_per_chip: cores,
        oscillator_hz: 25_000_000,
        initial_baud_divisor: 26,
        identity_reserved: 0x90,
    }
}

fn command(broadcast: bool, command: Command) -> Packet {
    Packet::Command { broadcast, command }
}

fn write(register: u8, value: u32) -> Packet {
    command(
        true,
        Command::Write {
            address: 0,
            register,
            value,
        },
    )
}

fn setup(chain: &mut Chain, count: u8, gated: bool) {
    chain.power(true).unwrap();
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
    for index in 0..count {
        chain
            .receive(&command(
                false,
                Command::SetAddress {
                    address: index * 4,
                    reserved: 0,
                },
            ))
            .unwrap();
    }
    for packet in [
        write(0x0c, 0x006d_0511),
        write(0x18, 0),
        write(0x1c, if gated { 0x0020_8180 } else { 0x0020_0180 }),
    ] {
        chain.receive(&packet).unwrap();
    }
}

fn fresh(id: u8) -> (Vec<Header>, Job) {
    let mut bytes = [0; 80];
    std::fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut bytes)
        .unwrap();
    let mut first = Header::decode(&bytes).unwrap();
    first.bits = 0x207fffff;
    let headers: Vec<_> = [0, 0x2000, 0x4000, 0x6000]
        .into_iter()
        .map(|bits| Header {
            version: first.version ^ bits,
            ..first.clone()
        })
        .collect();
    let states = headers
        .iter()
        .map(|header| {
            let mut hash = sha256::Hash::engine();
            hash.input(&header.encode()[..64]);
            let midstate = hash.midstate().to_byte_array();
            std::array::from_fn(|i| {
                u32::from_be_bytes(midstate[i * 4..i * 4 + 4].try_into().unwrap())
            })
        })
        .collect();
    let job = Job {
        id,
        states,
        nonce_origin: [0; 4],
        tail: headers[0].encode()[64..76].try_into().unwrap(),
    };
    (headers, job)
}

fn check(attempt: &md_sim_firmware::bm1387::search::Attempt, headers: &[Header]) {
    let mut header = headers[attempt.midstate_index].clone();
    header.nonce = attempt.nonce;
    let digest = sha256d::Hash::hash(&header.encode()).to_byte_array();
    assert_eq!(attempt.digest, digest);
    assert_eq!(attempt.response.is_some(), digest[28..] == [0; 4]);
    if let Some(response) = attempt.response {
        assert_eq!(
            u32::from_le_bytes(response[..4].try_into().unwrap()),
            header.nonce
        );
        assert_eq!(response[5], attempt.job_id | attempt.midstate_index as u8);
    }
}

#[test]
fn chains_compute_fresh_work_with_one_weighted_budget_and_cancel_on_reset() {
    let budget = Budget::new(10_000.0).unwrap();
    let mut chains = [
        Chain::with_mining(profile(2, 2), vec![vec![0, 64], vec![1, 120]]).unwrap(),
        Chain::with_mining(profile(1, 1), vec![vec![56]]).unwrap(),
    ];
    let slots = [
        Budget::attach(&budget).unwrap(),
        Budget::attach(&budget).unwrap(),
    ];
    let jobs = [fresh(0), fresh(4)];
    for index in 0..2 {
        setup(&mut chains[index], if index == 0 { 2 } else { 1 }, false);
        chains[index]
            .receive(&Packet::Job(jobs[index].1.clone()))
            .unwrap();
        slots[index].set_weight(chains[index].active_cores());
    }
    let start = Instant::now();
    let mut counts = [0; 2];
    let mut last_grant = 0;
    let mut last_ns = 0;
    while counts.iter().sum::<u64>() < 50 {
        assert!(start.elapsed() < Duration::from_secs(3));
        for index in 0..2 {
            if let Some((grant, attempt)) = chains[index].poll(&slots[index]).unwrap() {
                assert_eq!(grant.ordinal, last_grant + 1);
                assert!(grant.elapsed_ns - last_ns >= 100_000);
                last_grant = grant.ordinal;
                last_ns = grant.elapsed_ns;
                check(&attempt, &jobs[index].0);
                assert_eq!(attempt.chain_position, ((attempt.nonce >> 2) & 63) as usize);
                counts[index] += 1;
            }
            if counts.iter().sum::<u64>() == 50 {
                break;
            }
        }
        std::thread::sleep(Duration::from_micros(100));
    }
    assert_eq!(counts, [40, 10]);
    // First grant may already be due after setup; the following49 cannot.
    assert!(start.elapsed() >= Duration::from_micros(4900));
    assert_eq!(chains[0].hashes_computed(), 40);
    assert_eq!(chains[1].hashes_computed(), 10);
    chains[0].reset_line(true);
    assert!(chains[0].poll(&slots[0]).unwrap().is_none());
    assert_eq!(chains[0].active_cores(), 0);
    let (headers, new_job) = fresh(8);
    chains[1].receive(&Packet::Job(new_job)).unwrap();
    loop {
        if let Some((grant, attempt)) = chains[1].poll(&slots[1]).unwrap() {
            assert_eq!(grant.ordinal, 51);
            assert!(grant.elapsed_ns - last_ns >= 100_000);
            assert_eq!(attempt.job_id, 8);
            check(&attempt, &headers);
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(3));
        std::thread::sleep(Duration::from_micros(100));
    }
    chains[1].power(false).unwrap();
    assert!(chains[1].poll(&slots[1]).unwrap().is_none());
    assert_eq!(chains[1].hashes_computed(), 11);
}

#[test]
fn reference_core_startup_opens_every_core_without_a_success_shortcut() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../../simulator/backends/qemu/bm1387-reference-core-map.json"
    ))
    .unwrap();
    let cores: Vec<u8> = serde_json::from_value(reference["cores"].clone()).unwrap();
    let mut chain = Chain::with_mining(profile(63, 114), vec![cores; 63]).unwrap();
    setup(&mut chain, 63, true);
    let mut null = Job {
        id: 0,
        nonce_origin: [0; 4],
        tail: [0; 12],
        states: vec![[0; 8]; 4],
    };
    // Literal first guest job009, retained with parent hashes in asic-packets.json.
    null.tail[..4].fill(0xff);
    null.tail[8..].fill(0xff);
    for count in 1..=114 {
        null.id = ((count - 1) * 4 % 128) as u8;
        let effect = chain.receive(&Packet::Job(null.clone())).unwrap();
        assert!(effect.replies.is_empty());
        assert_eq!(effect.work, Some((null.id, count * 63)));
        assert_eq!(chain.active_cores(), count * 63);
        assert_eq!(chain.hashes_computed(), 0);
    }
    // Same-rate unicast control write is legal after full core startup.
    chain
        .receive(&command(
            false,
            Command::Write {
                address: 244,
                register: 0x1c,
                value: 0x0020_0180,
            },
        ))
        .unwrap();
    let budget = Budget::new(10_000.0).unwrap();
    let slot = Budget::attach(&budget).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut seen = std::collections::BTreeSet::new();
    while seen.len() < 63 {
        assert!(Instant::now() < deadline);
        if let Some((_, attempt)) = chain.poll(&slot).unwrap() {
            assert_eq!(attempt.core_address, 0);
            seen.insert(attempt.chain_position);
        }
        std::thread::sleep(Duration::from_micros(100));
    }
    assert_eq!(seen, (0..63).collect());
    assert_eq!(chain.hashes_computed(), 249); // four midstates per earlier chip
}

fn send(wire: &mut Wire, now: &mut u64, bytes: &[u8]) -> Vec<Trace> {
    let mut trace = Vec::new();
    let period = wire.bit_period_ns();
    for &byte in bytes {
        trace.extend(wire.input(*now, false).unwrap());
        for bit in 0..8 {
            trace.extend(
                wire.input(*now + (bit + 1) * period, byte & (1 << bit) != 0)
                    .unwrap(),
            );
        }
        trace.extend(wire.input(*now + 9 * period, true).unwrap());
        *now += 10 * period;
    }
    trace.extend(wire.advance(*now).unwrap());
    trace
}

fn reg_frame(register: u8, value: u32) -> Vec<u8> {
    let mut frame = vec![0x58, 9, 0, register];
    frame.extend(value.to_be_bytes());
    frame.push(crc5(&frame));
    frame
}

fn job_frame(job: &Job) -> Vec<u8> {
    let mut frame = vec![0x21, 150, job.id, 4, 0, 0, 0, 0];
    for word in job.tail.chunks_exact(4).rev() {
        frame.extend(word);
    }
    for state in &job.states {
        for word in state.iter().rev() {
            frame.extend(word.to_be_bytes());
        }
    }
    frame.extend(crc16(&frame).to_be_bytes());
    frame
}

#[test]
fn serial_work_bytes_drive_hashes_and_replacement_preserves_clock_and_reset_rules() {
    let budget = Budget::new(10_000.0).unwrap();
    // Explicit synthetic geometry, not a claim about a physical S9.
    let mut wire = Wire::with_mining(
        profile(1, 2),
        1024,
        vec![vec![0, 64]],
        Budget::attach(&budget).unwrap(),
    )
    .unwrap();
    wire.power(true).unwrap();
    wire.reset_line(false);
    let mut now = 0;
    for (register, value) in [(0x0c, 0x006d_0511), (0x18, 0), (0x1c, 0x0020_0180)] {
        send(&mut wire, &mut now, &reg_frame(register, value));
    }
    for id in [0, 4] {
        let (headers, job) = fresh(id);
        let trace = send(&mut wire, &mut now, &job_frame(&job));
        assert!(trace.iter().any(
            |event| matches!(event,Trace::Work {id:work_id,enabled_cores:2,..} if *work_id==id)
        ));
        let start = Instant::now();
        let mut count = 0;
        while count < 16 {
            std::thread::sleep(Duration::from_micros(100));
            now += 100_000;
            for event in wire.advance(now).unwrap() {
                if let Trace::Hash { attempt, .. } = event {
                    assert_eq!(attempt.job_id, id);
                    check(&attempt, &headers);
                    assert!(matches!(attempt.core_address, 0 | 64));
                    count += 1;
                }
            }
            assert!(start.elapsed() < Duration::from_secs(3));
        }
    }
    wire.reset_line(true);
    now += 100_000_000;
    assert!(wire.advance(now).unwrap().is_empty());
    assert_eq!(wire.next_deadline(), None);
    assert_eq!(wire.bit_period_ns(), 8640);
    wire.reset();
    assert!(wire.advance(1_000_000).unwrap().is_empty());
}

#[test]
fn missing_maps_and_unknown_register_state_do_not_enable_mining() {
    let (_, job) = fresh(0);
    let mut unconfigured = Chain::new(profile(1, 1)).unwrap();
    setup(&mut unconfigured, 1, false);
    assert_eq!(
        unconfigured.receive(&Packet::Job(job.clone())),
        Err(Error::WorkNotConfigured)
    );
    let mut chain = Chain::with_mining(profile(1, 1), vec![vec![0]]).unwrap();
    chain.power(true).unwrap();
    chain.reset_line(false);
    assert_eq!(
        chain.receive(&Packet::Job(job.clone())),
        Err(Error::UnknownResetValue(0x1c))
    );
    assert_eq!(chain.hashes_computed(), 0);
    setup(&mut chain, 1, true);
    let mut closed = job;
    closed.tail[8] &= !1;
    assert_eq!(
        chain.receive(&Packet::Job(closed)).unwrap().work,
        Some((0, 0))
    );
    assert_eq!(chain.active_cores(), 0);
}

#[test]
fn explicit_filter_adaptation_returns_computed_fresh_results_through_serial() {
    use md_sim_firmware::{bm1387::filter::Policy, serial8n1::Event};
    let budget = Budget::new(10_000.0).unwrap();
    let mut wire = Wire::with_mining_filter(
        profile(1, 2),
        1024,
        vec![vec![0, 64]],
        Budget::attach(&budget).unwrap(),
        Policy::from_fixed_bits(1).unwrap(),
    )
    .unwrap();
    let mut now = 0;
    for id in [0, 4] {
        // Power/reset must retain the declared filter policy.
        wire.power(true).unwrap();
        wire.reset_line(false);
        for (register, value) in [(0x0c, 0x006d_0511), (0x18, 0), (0x1c, 0x0020_0180)] {
            send(&mut wire, &mut now, &reg_frame(register, value));
        }
        let (headers, job) = fresh(id);
        let mut trace = send(&mut wire, &mut now, &job_frame(&job));
        let mut responses = Vec::new();
        let mut transmitted = Vec::new();
        let mut count = 0;
        let start = Instant::now();
        while count < 256 {
            for event in trace.drain(..) {
                match event {
                    Trace::Hash { attempt, .. } => {
                        let mut header = headers[attempt.midstate_index].clone();
                        header.nonce = attempt.nonce;
                        let digest = sha256d::Hash::hash(&header.encode()).to_byte_array();
                        assert_eq!(attempt.digest, digest);
                        assert_eq!(attempt.response.is_some(), digest[31] & 0x80 == 0);
                        if let Some(response) = attempt.response {
                            assert_eq!(
                                u32::from_le_bytes(response[..4].try_into().unwrap()),
                                header.nonce
                            );
                            assert_eq!(response[5], id | attempt.midstate_index as u8);
                            responses.extend(response);
                        }
                        count += 1;
                    }
                    Trace::Serial(Event::Transmitted { byte, .. }) => transmitted.push(byte),
                    _ => {}
                }
            }
            if count < 256 {
                assert!(start.elapsed() < Duration::from_secs(3));
                std::thread::sleep(Duration::from_micros(100));
                now += 100_000;
                trace.extend(wire.advance(now).unwrap());
            }
        }
        // The serial output may end partway through a real queued response.
        // Check every transmitted byte, without discarding partial responses.
        assert!(transmitted.len() >= 7);
        assert_eq!(transmitted, responses[..transmitted.len()]);
        wire.power(false).unwrap();
        now += 1_000_000;
        assert!(wire.advance(now).unwrap().is_empty());
    }
}

#[test]
fn adapted_filter_keeps_ticket_write_and_readback_after_reset() {
    use md_sim_firmware::bm1387::filter::Policy;
    let mut chain = Chain::with_mining_filter(
        profile(1, 1),
        vec![vec![0]],
        Policy::from_fixed_bits(12).unwrap(),
    )
    .unwrap();
    for value in [0x01020304, 0, u32::MAX] {
        setup(&mut chain, 1, false);
        chain.receive(&write(0x18, value)).unwrap();
        let reply = chain
            .receive(&command(
                false,
                Command::Read {
                    address: 0,
                    register: 0x18,
                },
            ))
            .unwrap();
        assert_eq!(reply.replies.len(), 1);
        assert_eq!(
            u32::from_be_bytes(reply.replies[0][..4].try_into().unwrap()),
            value
        );
        chain.reset_line(true);
        chain.reset_line(false);
        assert_eq!(
            chain.receive(&command(
                false,
                Command::Read {
                    address: 0,
                    register: 0x18
                }
            )),
            Err(Error::UnknownResetValue(0x18)),
        );
    }
}

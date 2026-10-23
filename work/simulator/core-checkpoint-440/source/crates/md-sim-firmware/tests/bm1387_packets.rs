use bitcoin_hashes::{sha256, sha256d, Hash, HashEngine};
use md_sim_firmware::{
    bm1387::{self, Command, Error, Job, Packet},
    bm1397::crc16,
    s9io::{self, Midstates, Response},
};
use md_sim_mining::{Engine, EngineEvent, Header, Target};
use std::time::{Duration, Instant};

fn reference() -> serde_json::Value {
    serde_json::from_str(include_str!("fixtures/s9io-reference.json")).unwrap()
}

fn bytes(values: &serde_json::Value) -> Vec<u8> {
    values
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}

fn frame(header: &Header, versions: &[u32], id: u16) -> Vec<u8> {
    let mut fifo = vec![
        u32::from(id),
        header.bits,
        header.time,
        u32::from_le_bytes(header.merkle_root[28..].try_into().unwrap()),
    ];
    for &version in versions {
        let mut h = header.clone();
        h.version = version;
        let mut oracle = sha256::Hash::engine();
        oracle.input(&h.encode()[..64]);
        let state = oracle.midstate().to_byte_array();
        // Exact ARM source chain: words::<u32>() reads LE; rev() reverses
        // word order; to_be() swaps the numeric word on the LE controller.
        for raw in state.chunks_exact(4).rev() {
            fifo.push(u32::from_le_bytes(raw.try_into().unwrap()).swap_bytes());
        }
    }
    let mode = match versions.len() {
        1 => Midstates::One,
        2 => Midstates::Two,
        4 => Midstates::Four,
        _ => panic!(),
    };
    s9io::work_frame(&fifo, mode).unwrap()
}

fn job(frame: &[u8]) -> Job {
    let Packet::Job(job) = bm1387::decode(frame).unwrap() else {
        panic!("not a job")
    };
    job
}

#[test]
fn command_opcodes_and_values_match_all_archived_transmit_vectors() {
    let r = reference();
    for task in ["tc_send_cmd5", "tc_send_cmd9"] {
        for (key, values) in r["tasks"][task].as_object().unwrap() {
            if !key.starts_with("uart") {
                continue;
            }
            let frame = bytes(values);
            let Packet::Command { broadcast, command } = bm1387::decode(&frame).unwrap() else {
                panic!()
            };
            match command {
                Command::SetAddress { address, reserved } => {
                    assert_eq!(frame[0], 0x41);
                    assert!(!broadcast);
                    assert_eq!(address, frame[2]);
                    assert_eq!(reserved, 0);
                }
                Command::Inactive { address, reserved } => {
                    assert_eq!(frame[0], 0x55);
                    assert!(broadcast);
                    assert_eq!((address, reserved), (0, 0));
                }
                Command::Read { address, register } => {
                    assert_eq!(frame[0], 0x54);
                    assert!(broadcast);
                    assert_eq!((address, register), (0, 0));
                }
                Command::Write {
                    address,
                    register,
                    value,
                } => {
                    if frame[0] == 0x48 {
                        assert!(!broadcast);
                        assert_eq!(address, frame[2]);
                        assert_eq!((register, value), (0x0c, 0x00680221));
                    } else {
                        assert!(broadcast);
                        assert_eq!(address, 0);
                        assert!([(0x1c, 0x40209a80), (0x18, 0x3f), (0x14, 0)]
                            .contains(&(register, value)));
                    }
                }
            }
            for bit in 0..frame.len() * 8 {
                let mut damaged = frame.clone();
                damaged[bit / 8] ^= 1 << (bit % 8);
                assert!(bm1387::decode(&damaged).is_err(), "{task}/{key}/bit{bit}");
            }
        }
    }
    assert!(bm1387::decode(&[0x52, 5, 0, 0, 10]).is_err()); // BM1397 opcode differs
    assert!(bm1387::decode(&[0x55, 0xaa, 0x54, 5, 0, 0, 25]).is_err());
}

#[test]
fn independent_full_header_hashes_match_one_two_and_four_wire_midstates() {
    let mut seed = 0x1387c0def00d74b8u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed as u32
    };
    for _ in 0..64 {
        let mut header = Header {
            version: next(),
            previous: [0; 32],
            merkle_root: [0; 32],
            time: next(),
            bits: 0x207fffff,
            nonce: 0,
        };
        for word in header
            .previous
            .chunks_exact_mut(4)
            .chain(header.merkle_root.chunks_exact_mut(4))
        {
            word.copy_from_slice(&next().to_le_bytes());
        }
        let versions = [header.version, next(), next(), next()];
        for count in [1, 2, 4] {
            let wire = frame(&header, &versions[..count], 0x7c);
            let decoded = job(&wire);
            assert_eq!(decoded.id, 0x7c);
            for nonce in [0, 1, 0x80000000, u32::MAX, next()] {
                header.nonce = nonce;
                for (index, version) in versions[..count].iter().enumerate() {
                    header.version = *version;
                    assert_eq!(
                        decoded.digest(index, nonce).unwrap(),
                        sha256d::Hash::hash(&header.encode()).to_byte_array()
                    );
                }
            }
            assert_eq!(decoded.digest(count, 0), Err(Error::MidstateCount));
            for byte in 0..wire.len() {
                let mut damaged = wire.clone();
                damaged[byte] ^= 1;
                assert!(
                    bm1387::decode(&damaged).is_err(),
                    "accepted damaged byte{byte}"
                );
            }
        }
    }
}

#[test]
fn register_replies_match_literal_hdl_responses_and_independent_crc_oracle() {
    let r = reference();
    for (key, value) in r["tasks"]["tc_cmd_response"].as_object().unwrap() {
        if !key.starts_with("uart") {
            continue;
        }
        let wire = bytes(value);
        assert_eq!(&wire[4..6], &[0, 0]);
        let register = u32::from_be_bytes(wire[..4].try_into().unwrap());
        assert_eq!(bm1387::register_response(register).as_slice(), wire);
    }
    for value in [0, 1, 0x13879000, 0x12345678, u32::MAX] {
        let reply = bm1387::register_response(value);
        // Independent bit-array51-bit oracle, separate from shared helpers.
        let mut c = [1u8; 5];
        for bit in 0..51 {
            let old = c;
            let input = (reply[bit / 8] >> (7 - bit % 8)) & 1;
            c = [
                old[4] ^ input,
                old[0],
                old[1] ^ old[4] ^ input,
                old[2],
                old[3],
            ];
        }
        assert_eq!(
            reply[6],
            c.iter().enumerate().map(|(i, b)| b << i).sum::<u8>()
        );
    }
}

#[test]
fn fixed_work_engine_results_are_computed_filtered_and_decoded_as_fpga_words() {
    // Fixed regression fixture only. No fresh-work, firmware or Core gate.
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/bitcoin/genesis-header.json"
    ))
    .unwrap();
    let mut header =
        Header::decode(&hex::decode(fixture["header_hex"].as_str().unwrap()).unwrap()).unwrap();
    let nonce = header.nonce;
    let work = job(&frame(&header, &[header.version], 4));
    assert_eq!(work.nonce_origin, [0; 4]);
    // Select a small fixed regression range through the engine contract.
    // No firmware nonce-origin field or ASIC filter is changed.
    let mut engine = Engine::new(10_000.0, 1).unwrap();
    engine
        .submit_midstate(
            work.engine_work("fixed-regression".into(), nonce - 2..=nonce + 2, 0)
                .unwrap(),
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut solutions = Vec::new();
    while engine.queue_len() != 0 {
        assert!(Instant::now() < deadline);
        for event in engine.poll() {
            if let EngineEvent::MidstateSolution {
                work_id,
                nonce,
                midstate_index,
                digest,
            } = event
            {
                assert_eq!(work_id, "fixed-regression");
                header.nonce = nonce;
                assert_eq!(
                    digest,
                    sha256d::Hash::hash(&header.encode()).to_byte_array()
                );
                let reply = work
                    .response(midstate_index, nonce, nonce - 2..=nonce + 2, 0, 0)
                    .unwrap();
                let Response::Work(words) =
                    s9io::response_words(&reply, 0x104, Midstates::One).unwrap()
                else {
                    panic!()
                };
                assert_eq!(words[0], nonce);
                assert_eq!((words[1] >> 8) & 0xffff, 0x104);
                solutions.push(nonce);
            }
        }
        std::thread::sleep(engine.wait_duration());
    }
    assert_eq!(engine.hashes_computed, 5);
    assert_eq!(solutions, [nonce]);
    assert_eq!(
        work.response(0, nonce - 1, nonce - 2..=nonce + 2, 0, 0),
        Err(Error::Filter)
    );
    assert_eq!(
        work.response(0, nonce, 0..=nonce - 1, 0, 0),
        Err(Error::NonceRange)
    );
    assert_eq!(
        work.response(1, nonce, nonce - 2..=nonce + 2, 0, 0),
        Err(Error::MidstateCount)
    );
    assert_eq!(
        work.response(0, nonce, nonce - 2..=nonce + 2, 0, u32::MAX),
        Err(Error::Filter)
    );
    assert!(work
        .engine_work("bad".into(), nonce - 2..=nonce - 3, 0)
        .is_err());
}

#[test]
fn ticket_bits_nonce_routing_and_malformed_work_keep_exact_constraints() {
    for (ticket, bits) in [(0, 32), (0xfc, 38), (0xff, 40), (u32::MAX, 64)] {
        let filter = bm1387::ticket_filter(ticket);
        assert_eq!(
            filter.intersection_probability(Target::MAX),
            2f64.powi(-bits)
        );
        assert_eq!(
            filter
                .expected_interval_seconds(Target::MAX, 10_000.0)
                .unwrap(),
            2f64.powi(bits) / 10_000.0
        );
    }
    for chip in 0..64 {
        for core in 0..128 {
            let nonce = 0x8000ff03 | (chip << 2) | (core << 24);
            assert_eq!(bm1387::nonce_address(nonce), (chip as u8, core as u8));
        }
    }
    let header = Header {
        version: 1,
        previous: [0; 32],
        merkle_root: [0; 32],
        time: 1,
        bits: 0x207fffff,
        nonce: 0,
    };
    let mut wire = frame(&header, &[1, 2], 3);
    assert_eq!(job(&wire).digest(0, 0), Err(Error::WorkId));
    wire[3] = 3;
    assert_eq!(bm1387::decode(&wire), Err(Error::MidstateCount));
    assert_eq!(bm1387::decode(&[]), Err(Error::Length));
    assert_eq!(bm1387::decode(&[0x21, 2]), Err(Error::Length));
    for offset in 4..8 {
        let mut wire = frame(&header, &[1], 4);
        wire[offset] = 0x55;
        let end = wire.len() - 2;
        let crc = crc16(&wire[..end]);
        wire[end..].copy_from_slice(&crc.to_be_bytes());
        let decoded = job(&wire);
        assert_eq!(decoded.nonce_origin[offset - 4], 0x55);
        assert_eq!(decoded.digest(0, 0), Err(Error::UnsupportedNonceOrigin));
        assert!(decoded
            .engine_work("unknown-origin".into(), 0..=10, 0)
            .is_err());
    }
}

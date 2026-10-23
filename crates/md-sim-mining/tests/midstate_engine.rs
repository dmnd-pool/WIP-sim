//! Shared scheduling and independent complete-header oracle. These tests do
//! not execute firmware or establish an exact hardware mining gate.
use bitcoin_hashes::{sha256d, Hash};
use md_sim_mining::{
    work::header_midstate, Engine, EngineEvent, Filter, Header, MidstateWork, Target, Work,
};
use std::{
    collections::BTreeSet,
    io::Read,
    time::{Duration, Instant},
};

fn headers() -> Vec<Header> {
    let mut entropy = [0; 64];
    std::fs::File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut entropy)
        .unwrap();
    (0..4)
        .map(|index| Header {
            version: 0x20000000 | index << 13,
            previous: entropy[..32].try_into().unwrap(),
            merkle_root: entropy[32..].try_into().unwrap(),
            time: 1_750_000_000,
            bits: 0x207fffff,
            nonce: 0,
        })
        .collect()
}

fn work(id: &str, headers: &[Header], start: u32, end: u32) -> MidstateWork {
    MidstateWork {
        id: id.into(),
        states: headers.iter().map(header_midstate).collect(),
        tail: headers[0].encode()[64..76].try_into().unwrap(),
        nonce_start: start,
        nonce_end: end,
        filter: Filter::Target(Target::MAX),
    }
}

#[test]
fn mixed_inputs_share_one_cap_queue_and_independent_digest_oracle() {
    let headers = headers();
    let mut engine = Engine::new(1000.0, 2).unwrap();
    engine
        .submit_midstate(work("midstates", &headers, u32::MAX - 1, u32::MAX))
        .unwrap();
    engine
        .submit(Work {
            id: "header".into(),
            header: headers[0].clone(),
            nonce_start: 41,
            nonce_end: 44,
            version_mask: 0,
            filter: Filter::Target(Target::MAX),
        })
        .unwrap();
    assert!(engine
        .submit_midstate(work("overflow", &headers, 0, 0))
        .is_err());
    let start = Instant::now();
    let mut midstate_results = BTreeSet::new();
    let mut header_results = BTreeSet::new();
    let mut exhausted = BTreeSet::new();
    let mut ordering = Vec::new();
    while engine.queue_len() != 0 {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "hash engine did not drain"
        );
        for event in engine.poll() {
            match event {
                EngineEvent::MidstateSolution {
                    work_id,
                    nonce,
                    midstate_index,
                    digest,
                } => {
                    assert_eq!(work_id, "midstates");
                    assert!(midstate_results.insert((midstate_index, nonce)));
                    let mut full = headers[midstate_index].clone();
                    full.nonce = nonce;
                    assert_eq!(digest, sha256d::Hash::hash(&full.encode()).to_byte_array());
                    ordering.push(work_id);
                }
                EngineEvent::Solution {
                    work_id,
                    nonce,
                    version,
                    digest,
                } => {
                    assert_eq!(work_id, "header");
                    assert_eq!(version, headers[0].version);
                    assert!(header_results.insert(nonce));
                    let mut full = headers[0].clone();
                    full.nonce = nonce;
                    assert_eq!(digest, sha256d::Hash::hash(&full.encode()).to_byte_array());
                    ordering.push(work_id);
                }
                EngineEvent::Exhausted { work_id } => {
                    assert!(exhausted.insert(work_id));
                }
            }
        }
        std::thread::sleep(engine.wait_duration().min(Duration::from_millis(1)));
    }
    assert_eq!(midstate_results.len(), 8);
    assert_eq!(header_results, (41..=44).collect());
    assert_eq!(
        exhausted,
        ["header".to_string(), "midstates".to_string()].into()
    );
    assert_eq!(
        &ordering[..8],
        [
            "midstates",
            "header",
            "midstates",
            "header",
            "midstates",
            "header",
            "midstates",
            "header"
        ]
    );
    assert_eq!(engine.hashes_computed, 12);
    assert!(start.elapsed() >= Duration::from_millis(11));
}

#[test]
fn invalid_ranges_duplicates_cancellation_reset_and_empty_polls_do_not_hash() {
    let headers = headers();
    let valid = work("same", &headers, 0, 10);
    let mut engine = Engine::new(10_000.0, 3).unwrap();
    for mutate in 0..4 {
        let mut invalid = valid.clone();
        match mutate {
            0 => invalid.states.clear(),
            1 => invalid.states.resize(257, [0; 8]),
            2 => invalid.nonce_start = 11,
            _ => invalid.id.clear(),
        }
        assert!(engine.submit_midstate(invalid).is_err());
    }
    engine.submit_midstate(valid.clone()).unwrap();
    assert!(engine
        .submit(Work {
            id: "same".into(),
            header: headers[0].clone(),
            nonce_start: 0,
            nonce_end: 0,
            version_mask: 0,
            filter: Filter::Target(Target::MAX),
        })
        .is_err());
    assert!(engine.submit_midstate(valid.clone()).is_err());
    engine.cancel("absent");
    assert_eq!(engine.queue_len(), 1);
    engine.cancel("same");
    engine.submit_midstate(valid).unwrap();
    engine.reset();
    std::thread::sleep(Duration::from_millis(2));
    assert!(engine.poll().is_empty());
    assert_eq!(engine.hashes_computed, 0);
}

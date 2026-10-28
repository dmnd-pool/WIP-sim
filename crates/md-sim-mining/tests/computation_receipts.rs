use bitcoin_hashes::{sha256d, Hash};
use md_sim_mining::{
    work::header_midstate, ComputedVariant, Engine, EngineEvent, Filter, Header, MidstateWork,
    Target, Work,
};
use std::time::{Duration, Instant};

fn header() -> Header {
    Header {
        version: 0x2000_0000,
        previous: [0x51; 32],
        merkle_root: [0x93; 32],
        time: 1_730_001_234,
        bits: 0x207f_ffff,
        nonce: 0,
    }
}
fn next(engine: &mut Engine) -> md_sim_mining::Computation {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(receipt) = engine.poll_computation() {
            return receipt;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(engine.wait_duration());
    }
}
fn work(id: &str, filter: Filter) -> Work {
    Work {
        id: id.into(),
        header: header(),
        nonce_start: 0,
        nonce_end: 2,
        version_mask: 0,
        filter,
    }
}

#[test]
fn midstates_match_independent_full_header_hashes_and_exhaustion() {
    let headers: Vec<_> = (0..4)
        .map(|i| Header {
            version: 0x2000_0000 | i,
            ..header()
        })
        .collect();
    let bytes = headers[0].encode();
    let mut engine = Engine::new(1_000_000.0, 1).unwrap();
    engine
        .submit_midstate(MidstateWork {
            id: "constrained".into(),
            states: headers.iter().map(header_midstate).collect(),
            tail: bytes[64..76].try_into().unwrap(),
            nonce_start: 11,
            nonce_end: 70,
            filter: Filter::Target(Target::ZERO),
        })
        .unwrap();
    let expected: Vec<_> = (0..4)
        .flat_map(|index| (11..=70).map(move |n| (index, n)))
        .collect();
    for (position, &(index, nonce)) in expected.iter().enumerate() {
        let receipt = next(&mut engine);
        assert_eq!(receipt.variant, ComputedVariant::Midstate { index });
        assert_eq!(receipt.nonce, nonce);
        let full = Header {
            nonce,
            ..headers[index].clone()
        };
        assert_eq!(
            receipt.digest,
            sha256d::Hash::hash(&full.encode()).to_byte_array()
        );
        assert!(!receipt.accepted);
        assert!(receipt.completed_at >= receipt.admitted_at);
        assert_eq!(receipt.exhausted, position + 1 == expected.len());
    }
    assert_eq!(engine.hashes_computed, expected.len() as u64);
    assert_eq!(engine.queue_len(), 0);
    assert!(engine.poll_computation().is_none());
}

#[test]
fn live_filter_change_round_robin_and_cancel_preserve_actual_cursors() {
    let mut engine = Engine::new(1_000_000.0, 2).unwrap();
    engine
        .submit(work("a", Filter::Target(Target::ZERO)))
        .unwrap();
    engine
        .submit(work("b", Filter::Target(Target::MAX)))
        .unwrap();
    let first = next(&mut engine);
    assert_eq!(
        (first.work_id.as_str(), first.nonce, first.accepted),
        ("a", 0, false)
    );
    assert!(engine.update_filter("a", Filter::Target(Target::MAX)));
    assert!(!engine.update_filter("absent", Filter::Target(Target::MAX)));
    let second = next(&mut engine);
    assert_eq!((second.work_id.as_str(), second.nonce), ("b", 0));
    let third = next(&mut engine);
    assert_eq!(
        (third.work_id.as_str(), third.nonce, third.accepted),
        ("a", 1, true)
    );
    engine.cancel("b");
    let fourth = next(&mut engine);
    assert_eq!(
        (fourth.work_id.as_str(), fourth.nonce, fourth.exhausted),
        ("a", 2, true)
    );
    assert_eq!(engine.hashes_computed, 4);
    assert!(engine.poll_computation().is_none());
}

#[test]
fn receipts_preserve_cap_no_idle_credit_and_original_event_api() {
    let mut engine = Engine::new(20.0, 1).unwrap();
    engine
        .submit(work("cap", Filter::Target(Target::MAX)))
        .unwrap();
    assert!(engine.poll_computation().is_none());
    assert_eq!(engine.hashes_computed, 0);
    let a = next(&mut engine);
    assert!(engine.poll_computation().is_none());
    let b = next(&mut engine);
    assert!(b.admitted_at.duration_since(a.admitted_at) >= Duration::from_millis(50));
    // Existing callers still get the solution followed by exhaustion.
    std::thread::sleep(engine.wait_duration());
    let events = engine.poll();
    assert!(matches!(
        &events[..],
        [
            EngineEvent::Solution { nonce: 2, .. },
            EngineEvent::Exhausted { .. }
        ]
    ));
    assert_eq!(engine.hashes_computed, 3);
}

#[test]
fn invalid_midstate_bounds_fail_without_mutating_the_live_queue() {
    let mut engine = Engine::default();
    engine
        .submit(work("existing", Filter::Target(Target::MAX)))
        .unwrap();
    let result = engine.submit_midstate(MidstateWork {
        id: "reversed".into(),
        states: vec![header_midstate(&header())],
        tail: [0; 12],
        nonce_start: 7,
        nonce_end: 4,
        filter: Filter::Target(Target::MAX),
    });
    assert!(result.is_err());
    assert_eq!(engine.queue_len(), 1);
    assert_eq!(engine.hashes_computed, 0);
}

use bitcoin_hashes::{sha256d, Hash};
use md_sim_mining::{
    engine::RateLimiter,
    work::{cgminer_sha256d, digest_from_midstate, header_midstate},
    Engine, EngineEvent, Filter, Header, PoolJob, PoolVerifier, Share, Target, Verdict, Work,
};
use std::time::{Duration, Instant};

fn genesis() -> Header {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/bitcoin/genesis-header.json"
    ))
    .unwrap();
    Header::decode(&hex::decode(fixture["header_hex"].as_str().unwrap()).unwrap()).unwrap()
}
fn job(id: &str, seed: u8, target: Target) -> PoolJob {
    PoolJob {
        id: id.into(),
        previous: [seed; 32],
        coinbase_prefix: vec![1, seed, 3],
        extra_nonce1: vec![seed; 4],
        extra_nonce2_size: 4,
        coinbase_suffix: vec![4, 5, 6],
        merkle_branch: vec![[seed.wrapping_add(1); 32]],
        version: 0x20000000,
        version_mask: 0x2000,
        bits: 0x207fffff,
        time_min: 1_750_000_000,
        time_max: 1_750_000_060,
        nonce_min: 0,
        nonce_max: 1000,
        share_target: target,
    }
}
fn share(job: &PoolJob) -> Share {
    Share {
        job_id: job.id.clone(),
        extra_nonce2: vec![0; 4],
        nonce: 0,
        time: job.time_min,
        version: job.version,
    }
}

#[test]
fn core_genesis_and_compact_target() {
    let h = genesis();
    let mut digest = h.digest();
    assert_eq!(digest, sha256d::Hash::hash(&h.encode()).to_byte_array());
    assert!(Target::from_compact(h.bits).unwrap().accepts(&digest));
    digest.reverse();
    assert_eq!(
        hex::encode(digest),
        "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"
    );
    assert_eq!(Target::from_compact(0x207fffff).unwrap().0[0], 0x7f);
    for bad in [
        0, 0x01800001, 0x01000001, 0x23000001, 0x22000100, 0x21010000,
    ] {
        assert!(Target::from_compact(bad).is_err(), "{bad:x}");
    }
    assert_eq!(Target::from_compact(0x01010000).unwrap().0[31], 1);
    assert_eq!(Target::from_compact(0x220000ff).unwrap().0[0], 255);
}

#[test]
fn midstate_and_cgminer_bytes_match_independent_full_header_hashes() {
    // Deterministic varied headers test compression-state restoration, nonce
    // endian and padding. Oracle uses the independent rust-bitcoin hash code.
    let mut h = genesis();
    for n in 0u32..256 {
        h.version = 0x20000000 ^ n.wrapping_mul(0x10201);
        h.nonce = n.wrapping_mul(0x6c078965);
        h.time = h.time.wrapping_add(n);
        h.merkle_root[(n % 32) as usize] ^= n as u8;
        let state = header_midstate(&h);
        let tail: [u8; 12] = h.encode()[64..76].try_into().unwrap();
        let oracle = sha256d::Hash::hash(&h.encode()).to_byte_array();
        assert_eq!(digest_from_midstate(state, tail, h.nonce), oracle);
        let mut cg_mid = [0; 32];
        for (out, w) in cg_mid.chunks_exact_mut(4).zip(state) {
            out.copy_from_slice(&w.to_le_bytes());
        }
        let mut cg_tail = tail;
        for word in cg_tail.chunks_exact_mut(4) {
            word.reverse();
        }
        assert_eq!(cgminer_sha256d(cg_mid, cg_tail, h.nonce), oracle);
    }
}

#[test]
fn target_boundaries_and_ticket_intersection_counting() {
    let mut target = [0; 32];
    target[30] = 0x12;
    target[31] = 0x34;
    let t = Target(target);
    let mut digest = target;
    digest.reverse();
    assert!(t.accepts(&digest));
    digest[0] += 1;
    assert!(!t.accepts(&digest));
    assert_eq!(Target::MAX.probability(), 1.0);
    // Exhaust all small integer values with a high zero mask, then compare the
    // explicit count with the probability recurrence for arbitrary low masks.
    for mask in [0, 1, 0x55, 0xaa, 0xff] {
        let mut m = [255; 32];
        m[31] = mask;
        for bound in [0, 1, 31, 127, 255] {
            let mut t = [0; 32];
            t[31] = bound;
            let count = (0..=bound).filter(|n| n & mask == 0).count();
            let p = Filter::ZeroMask(m).intersection_probability(Target(t));
            assert!((p * 2f64.powi(256) - count as f64).abs() < 1e-8);
        }
    }
    let hard = Target::from_compact(0x1d00ffff).unwrap();
    assert_eq!(
        Filter::Target(hard).intersection_probability(Target::MAX),
        hard.probability()
    );
}

#[test]
fn engine_preserves_nonce_and_sparse_version_ranges_and_caps() {
    let mut engine = Engine::new(1000.0, 1).unwrap();
    let w = Work {
        id: "range".into(),
        header: genesis(),
        nonce_start: u32::MAX - 1,
        nonce_end: u32::MAX,
        version_mask: 0x80000001,
        filter: Filter::Target(Target::MAX),
    };
    engine.submit(w.clone()).unwrap();
    assert!(engine.submit(w).is_err());
    let start = Instant::now();
    let mut solutions = Vec::new();
    let mut exhausted = 0;
    while engine.queue_len() != 0 {
        for e in engine.poll() {
            match e {
                EngineEvent::Solution { nonce, version, .. } => solutions.push((nonce, version)),
                EngineEvent::Exhausted { .. } => exhausted += 1,
                EngineEvent::MidstateSolution { .. } => panic!("header work changed result type"),
            }
        }
        std::thread::sleep(engine.wait_duration().min(Duration::from_millis(1)));
    }
    assert_eq!(solutions.len(), 8);
    assert_eq!(engine.hashes_computed, 8);
    assert_eq!(exhausted, 1);
    assert!(start.elapsed() >= Duration::from_millis(8));
    assert_eq!(
        solutions
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        8
    );
    assert!(RateLimiter::new(0.1).is_ok());
    for bad in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        assert!(RateLimiter::new(bad).is_err());
    }
}

#[test]
fn fresh_jobs_clean_transition_difficulty_and_rejections() {
    // Fresh at test time. This is engine/pool integration, not firmware mining.
    let mut entropy = [0; 2];
    std::io::Read::read_exact(
        &mut std::fs::File::open("/dev/urandom").unwrap(),
        &mut entropy,
    )
    .unwrap();
    let first = job("fresh-1", entropy[0], Target::MAX);
    let mut pool = PoolVerifier::default();
    pool.install(first.clone(), true).unwrap();
    let submitted = share(&first);
    let header = Header::decode(&first.reconstruct(&submitted).unwrap()).unwrap();
    let mut engine = Engine::new(10_000.0, 1).unwrap();
    engine
        .submit(Work {
            id: first.id.clone(),
            header,
            nonce_start: 0,
            nonce_end: 0,
            version_mask: 0,
            filter: Filter::Target(Target::MAX),
        })
        .unwrap();
    let mut returned = None;
    while returned.is_none() {
        for event in engine.poll() {
            if let EngineEvent::Solution { nonce, version, .. } = event {
                returned = Some((nonce, version));
            }
        }
        std::thread::sleep(engine.wait_duration());
    }
    let (nonce, version) = returned.unwrap();
    let actual = Share {
        nonce,
        version,
        ..submitted.clone()
    };
    assert!(matches!(
        pool.verify(&actual).unwrap(),
        Verdict::Accepted { .. }
    ));
    assert_eq!(pool.verify(&actual).unwrap(), Verdict::Duplicate);
    let mut invalid = actual.clone();
    invalid.nonce = 1001;
    assert_eq!(pool.verify(&invalid).unwrap(), Verdict::OutOfRange);
    invalid = actual.clone();
    invalid.time -= 1;
    assert_eq!(pool.verify(&invalid).unwrap(), Verdict::OutOfRange);
    invalid = actual.clone();
    invalid.version ^= 1;
    assert_eq!(pool.verify(&invalid).unwrap(), Verdict::InvalidVersion);
    invalid = actual.clone();
    invalid.extra_nonce2.push(0);
    assert_eq!(pool.verify(&invalid).unwrap(), Verdict::InvalidExtraNonce);
    let second = job("fresh-2", entropy[1], Target::ZERO);
    pool.install(second.clone(), true).unwrap();
    assert_eq!(pool.verify(&actual).unwrap(), Verdict::Stale);
    assert_eq!(
        pool.verify(&share(&second)).unwrap(),
        Verdict::LowDifficulty
    );
    // Earlier jobs keep their original difficulty when a non-clean job arrives.
    let third = job("fresh-3", entropy[1] ^ 1, Target::MAX);
    pool.install(third.clone(), false).unwrap();
    assert!(matches!(
        pool.verify(&share(&third)).unwrap(),
        Verdict::Accepted { .. }
    ));
    let mut s = share(&second);
    s.nonce = 1;
    assert_eq!(pool.verify(&s).unwrap(), Verdict::LowDifficulty);
    assert!(pool.install(first, true).is_err());
}

#[test]
fn pool_deduplicates_identical_headers_across_job_aliases() {
    let first = job("alias-a", 1, Target::MAX);
    let mut second = first.clone();
    second.id = "alias-b".into();
    let mut pool = PoolVerifier::default();
    pool.install(first.clone(), false).unwrap();
    pool.install(second.clone(), false).unwrap();
    assert!(matches!(
        pool.verify(&share(&first)).unwrap(),
        Verdict::Accepted { .. }
    ));
    assert_eq!(pool.verify(&share(&second)).unwrap(), Verdict::Duplicate);
}

#[test]
fn archived_esp_miner_bm1397_compression_state_vector() {
    // ESP-Miner commit cd470d4df1944fb71b6c07d101a5fddade9ac02d,
    // archive SHA256 b93d5fd28110e6dd7082346caccf8799340194391016ead9a179d18e91bfcd88,
    // components/asic/test/test_bm_job_packet_building.c,
    // member SHA256 ed25932cb89359d4e9cdbf4067399ff7e803bb7872d3fb7eec9c6d5863e04800.
    let mut previous: [u8; 32] =
        hex::decode("bf44fd3513dc7b837d60e5c628b572b448d204a8000007490000000000000000")
            .unwrap()
            .try_into()
            .unwrap();
    for word in previous.chunks_exact_mut(4) {
        word.reverse();
    }
    let h = Header {
        version: 0x20000004,
        previous,
        merkle_root: hex::decode(
            "cd1be82132ef0d12053dcece1fa0247fcfdb61d4dbd3eb32ea9ef9b4c604a846",
        )
        .unwrap()
        .try_into()
        .unwrap(),
        time: 0x64658bd8,
        bits: 0x1705dd01,
        nonce: 0,
    };
    // The published vector writes each state word little-endian.
    let bytes: Vec<u8> = header_midstate(&h)
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    assert_eq!(
        hex::encode(bytes),
        "91dfea528a9f73683d0d495dd6dd7415e1ca21cb411759e3e05d7d5ff285314d"
    );
}

#[test]
fn rational_pool_target_is_stricter_than_programmed_asic_filter() {
    use md_sim_mining::stratum::{difficulty_target, difficulty_target_scaled, DifficultyScale};
    let asic = difficulty_target(-20).unwrap();
    let pool = difficulty_target_scaled(-20, DifficultyScale::ThreeHalves).unwrap();
    assert_eq!(
        hex::encode(pool.0),
        "000aaaa000000000000000000000000000000000000000000000000000000000"
    );
    assert!(pool < asic);
    let mut just_above_pool = pool.0;
    just_above_pool[31] = 1;
    just_above_pool.reverse();
    assert!(asic.accepts(&just_above_pool));
    assert!(!pool.accepts(&just_above_pool));
    assert_eq!(
        Filter::Target(asic).intersection_probability(pool),
        pool.probability()
    );
}

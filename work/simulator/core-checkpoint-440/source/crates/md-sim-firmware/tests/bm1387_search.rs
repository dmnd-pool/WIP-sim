use bitcoin_hashes::{sha256, sha256d, Hash, HashEngine};
use md_sim_firmware::{
    bm1387::{
        nonce::{Chip, Layout},
        search::{Attempt, Search},
        Error, Job,
    },
    s9io::{self, Midstates, Response},
};
use md_sim_mining::{engine::RateLimiter, Header};
use std::{
    io::Read,
    time::{Duration, Instant},
};

fn job(headers: &[Header], id: u8) -> Job {
    let states = headers
        .iter()
        .map(|header| {
            let mut oracle = sha256::Hash::engine();
            oracle.input(&header.encode()[..64]);
            let bytes = oracle.midstate().to_byte_array();
            std::array::from_fn(|index| {
                u32::from_be_bytes(bytes[index * 4..index * 4 + 4].try_into().unwrap())
            })
        })
        .collect();
    Job {
        id,
        nonce_origin: [0; 4],
        tail: headers[0].encode()[64..76].try_into().unwrap(),
        states,
    }
}

fn collect(search: &mut Search, budget: &mut RateLimiter) -> Vec<Attempt> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut attempts = Vec::new();
    while !search.exhausted() {
        assert!(Instant::now() < deadline);
        if let Some(attempt) = search.poll(budget).unwrap() {
            attempts.push(attempt);
        }
        std::thread::sleep(budget.wait_duration());
    }
    attempts
}

#[test]
fn fresh_headers_use_owned_nonces_and_share_one_aggregate_rate_budget() {
    let mut jobs = Vec::new();
    for id in [0, 4] {
        let mut bytes = [0; 80];
        std::fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut bytes)
            .unwrap();
        let mut first = Header::decode(&bytes).unwrap();
        first.bits = 0x207fffff;
        let headers: Vec<_> = [0, 0x2000, 0x4000, 0x6000]
            .map(|mask| Header {
                version: first.version ^ mask,
                ..first.clone()
            })
            .into();
        let layout = Layout::new(vec![
            Chip {
                address: 4,
                cores: vec![0, 64],
            },
            Chip {
                address: 104,
                cores: vec![120],
            },
        ])
        .unwrap();
        let search = Search::new(job(&headers, id), layout, 0, 0..=15).unwrap();
        jobs.push((search, headers));
    }
    let start = Instant::now();
    let mut budget = RateLimiter::new(10_000.0).unwrap();
    let mut count = 0;
    while jobs.iter().any(|(search, _)| !search.exhausted()) {
        assert!(start.elapsed() < Duration::from_secs(3));
        for (search, headers) in &mut jobs {
            if let Some(attempt) = search.poll(&mut budget).unwrap() {
                let mut header = headers[attempt.midstate_index].clone();
                header.nonce = attempt.nonce;
                let expected = sha256d::Hash::hash(&header.encode()).to_byte_array();
                assert_eq!(attempt.digest, expected);
                let chip = (header.nonce >> 2) & 63;
                let core = (header.nonce >> 24) & 127;
                assert!((chip == 1 && matches!(core, 0 | 64)) || (chip == 26 && core == 120));
                assert_eq!(attempt.chain_position, usize::from(chip == 26));
                assert_eq!(u32::from(attempt.core_address), core);
                assert_eq!(attempt.response.is_some(), expected[28..] == [0; 4]);
                count += 1;
            }
        }
        std::thread::sleep(budget.wait_duration());
    }
    assert_eq!(count, 128);
    assert!(start.elapsed() >= Duration::from_micros(count * 100));
    for (mut search, _) in jobs {
        assert_eq!(search.hashes_computed(), 64);
        assert!(search.poll(&mut budget).unwrap().is_none());
        assert_eq!(search.hashes_computed(), 64);
    }
}

#[test]
fn fixed_regression_computes_a_real_filtered_wire_response() {
    // Synthetic one-core geometry for a Bitcoin regression, not an S9
    // physical map: genesis core124 is absent from the vendor reference.
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/bitcoin/genesis-header.json"
    ))
    .unwrap();
    let mut header =
        Header::decode(&hex::decode(fixture["header_hex"].as_str().unwrap()).unwrap()).unwrap();
    let known = header.nonce;
    let ordinal = u64::from((known & 3) | (((known >> 8) & 0xffff) << 2) | ((known >> 31) << 18));
    let layout = || {
        Layout::new(vec![Chip {
            address: (known & 0xfc) as u8,
            cores: vec![((known >> 24) & 127) as u8],
        }])
        .unwrap()
    };
    let mut search = Search::new(
        job(&[header.clone()], 4),
        layout(),
        0,
        ordinal - 2..=ordinal + 2,
    )
    .unwrap();
    let mut budget = RateLimiter::new(10_000.0).unwrap();
    let attempts = collect(&mut search, &mut budget);
    assert_eq!(attempts.len(), 5);
    assert_eq!(search.hashes_computed(), 5);
    let mut responses = Vec::new();
    for attempt in attempts {
        header.nonce = attempt.nonce;
        assert_eq!(
            attempt.digest,
            sha256d::Hash::hash(&header.encode()).to_byte_array()
        );
        if let Some(response) = attempt.response {
            let Response::Work(words) =
                s9io::response_words(&response, 0x104, Midstates::One).unwrap()
            else {
                panic!("not a work response")
            };
            responses.push(words[0]);
            assert_eq!((words[1] >> 8) & 0xffff, 0x104);
        }
    }
    assert_eq!(responses, [known]);
    let mut filtered = Search::new(job(&[header], 4), layout(), 0, ordinal..=ordinal).unwrap();
    filtered.set_ticket_mask(u32::MAX);
    assert!(collect(&mut filtered, &mut budget)[0].response.is_none());
    assert_eq!(filtered.hashes_computed(), 1);
}

#[test]
fn invalid_ranges_and_work_cannot_start_and_idle_poll_does_not_hash() {
    let header = Header {
        version: 1,
        previous: [0; 32],
        merkle_root: [0; 32],
        time: 1,
        bits: 0x207fffff,
        nonce: 0,
    };
    let layout = || {
        Layout::new(vec![Chip {
            address: 0,
            cores: vec![0],
        }])
        .unwrap()
    };
    for range in [std::ops::RangeInclusive::new(1, 0), 0..=(1 << 19)] {
        assert!(matches!(
            Search::new(job(std::slice::from_ref(&header), 0), layout(), 0, range),
            Err(Error::NonceRange)
        ));
    }
    let mut invalid = job(std::slice::from_ref(&header), 0);
    invalid.nonce_origin[0] = 1;
    assert!(matches!(
        Search::new(invalid, layout(), 0, 0..=0),
        Err(Error::UnsupportedNonceOrigin)
    ));
    let mut search = Search::new(job(&[header], 0), layout(), 0, 0..=0).unwrap();
    let mut slow = RateLimiter::new(0.1).unwrap();
    assert!(search.poll(&mut slow).unwrap().is_none());
    assert_eq!(search.hashes_computed(), 0);
    assert!(!search.exhausted());
}

//! Real private IPC server/client test. This is not firmware execution evidence.
use md_sim_mining::{
    stratum::{serve_inherited, ResponsePolicy, StratumConfig},
    Header, Verdict,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::net::UnixStream,
    },
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

fn send(file: &mut File, value: Value) {
    writeln!(file, "{value}").unwrap();
}
fn receive(file: &mut File) -> Value {
    let mut line = Vec::new();
    loop {
        let mut p = libc::pollfd {
            fd: file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: the descriptor remains open during this one-entry poll.
        assert!(
            unsafe { libc::poll(&mut p, 1, 2000) } > 0,
            "pool response timed out"
        );
        let mut b = [0];
        file.read_exact(&mut b).unwrap();
        line.push(b[0]);
        assert!(line.len() <= 65536);
        if b[0] == b'\n' {
            return serde_json::from_slice(&line).unwrap();
        }
    }
}
fn decode_header(notify: &Value, extra1: &str, extra2: &str) -> Header {
    let p = &notify["params"];
    let mut coinbase = hex::decode(p[2].as_str().unwrap()).unwrap();
    coinbase.extend(hex::decode(extra1).unwrap());
    coinbase.extend(hex::decode(extra2).unwrap());
    coinbase.extend(hex::decode(p[3].as_str().unwrap()).unwrap());
    let mut root: [u8; 32] = Sha256::digest(Sha256::digest(&coinbase)).into();
    for branch in p[4].as_array().unwrap() {
        let mut pair = root.to_vec();
        pair.extend(hex::decode(branch.as_str().unwrap()).unwrap());
        root = Sha256::digest(Sha256::digest(&pair)).into();
    }
    let mut previous: [u8; 32] = hex::decode(p[1].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    for word in previous.chunks_exact_mut(4) {
        word.reverse();
    }
    Header {
        version: u32::from_str_radix(p[5].as_str().unwrap(), 16).unwrap(),
        previous,
        merkle_root: root,
        bits: u32::from_str_radix(p[6].as_str().unwrap(), 16).unwrap(),
        time: u32::from_str_radix(p[7].as_str().unwrap(), 16).unwrap(),
        nonce: 0,
    }
}
fn submission(id: u32, notify: &Value, nonce: u32, time: u32) -> Value {
    json!({"id":id,"method":"mining.submit","params":["simulator",notify["params"][0],"00000000",format!("{time:08x}"),format!("{nonce:08x}")]})
}

#[test]
fn separate_pipe_descriptors_deliver_fresh_work_and_cancel() {
    fn pipe() -> (File, File) {
        let mut fds = [-1; 2];
        // SAFETY: pipe2 initializes exactly these two descriptors on success.
        assert_eq!(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) }, 0);
        unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) }
    }
    let dir = tempfile::tempdir().unwrap();
    let (reader, mut client_write) = pipe();
    let (mut client_read, writer) = pipe();
    let mut seed = [0; 32];
    File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut seed)
        .unwrap();
    let config = StratumConfig {
        response_policy: ResponsePolicy::JsonRpc1StratumV1,
        regtest_anchor: None,
        difficulty_scale: md_sim_mining::stratum::DifficultyScale::One,
        limits: Default::default(),
        seed,
        time: 1_750_000_000,
        difficulty_exponents: vec![-20, -19],
        timeout: Duration::from_secs(4),
        trace_path: dir.path().join("stratum.jsonl"),
    };
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let server = std::thread::spawn(move || {
        md_sim_mining::stratum::serve_pipes(reader, writer, config, signal).unwrap()
    });
    send(
        &mut client_write,
        json!({"id":1,"method":"mining.subscribe","params":[]}),
    );
    assert_eq!(receive(&mut client_read)["id"], 1);
    send(
        &mut client_write,
        json!({"id":2,"method":"mining.authorize","params":["simulator","x"]}),
    );
    assert_eq!(receive(&mut client_read)["result"], true);
    assert_eq!(receive(&mut client_read)["method"], "mining.set_difficulty");
    let job = receive(&mut client_read);
    assert_eq!(job["method"], "mining.notify");
    stop.store(true, Ordering::Release);
    let report = server.join().unwrap();
    assert!(report.stopped && !report.completed && !report.timed_out);
    assert!(report.failure.is_none());
    assert_eq!(report.transport, "private-duplex-pipes-posix-io");
    assert_eq!(report.jobs[0].id, job["params"][0].as_str().unwrap());
    assert_ne!(report.jobs[0].id, report.jobs[1].id);
    assert!(report.submissions.is_empty());
}

#[test]
fn private_stratum_two_fresh_jobs_and_negative_submissions() {
    exercise_fresh_jobs(false, false);
}

#[test]
fn malformed_firmware_frame_is_retained_byte_for_byte_without_acceptance() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("malformed.jsonl");
    let (a, b) = UnixStream::pair().unwrap();
    let mut client = File::from(OwnedFd::from(b));
    let config = StratumConfig {
        response_policy: ResponsePolicy::JsonRpc1StratumV1,
        regtest_anchor: None,
        seed: [8; 32],
        time: 1_750_000_000,
        difficulty_exponents: vec![-28, -27],
        difficulty_scale: md_sim_mining::stratum::DifficultyScale::One,
        timeout: Duration::from_secs(2),
        trace_path: path.clone(),
        limits: Default::default(),
    };
    let server = std::thread::spawn(move || {
        serve_inherited(
            File::from(OwnedFd::from(a)),
            config,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
    });
    let raw = b"{\xffbroken\r\n";
    client.write_all(&raw[..3]).unwrap();
    client.write_all(&raw[3..]).unwrap();
    let report = server.join().unwrap();
    assert!(!report.completed && report.failure.is_some());
    assert_eq!(report.messages_received, 1);
    assert!(report.submissions.is_empty());
    let entry: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(entry["direction"], "firmware-to-pool-invalid-json");
    assert_eq!(entry["message"]["raw_hex"], hex::encode(raw));
}

#[test]
fn bip310_rolls_versions_across_fresh_jobs_and_rejects_unpermitted_bits() {
    exercise_fresh_jobs(true, false);
}

#[test]
fn command_connections_keep_binary_probe_separate_from_fresh_mining() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("probe.jsonl");
    let (server, client) = UnixStream::pair().unwrap();
    let mut client = File::from(OwnedFd::from(client));
    let config = StratumConfig {
        response_policy: ResponsePolicy::JsonRpc1StratumV1,
        regtest_anchor: None,
        seed: [4; 32],
        time: 1_750_000_000,
        difficulty_exponents: vec![-28, -27],
        difficulty_scale: md_sim_mining::stratum::DifficultyScale::One,
        timeout: Duration::from_secs(4),
        trace_path: path.clone(),
        limits: Default::default(),
    };
    let server = File::from(OwnedFd::from(server));
    let writer = server.try_clone().unwrap();
    let probe = std::thread::spawn(move || {
        md_sim_mining::stratum::serve_command_stream(
            server,
            writer,
            config,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
    });
    client.write_all(b"\x09\x00STR2\x01AESG").unwrap();
    exercise_fresh_jobs(true, true);
    drop(client);
    let report = probe.join().unwrap();
    assert!(!report.completed && report.submissions.is_empty());
    assert_eq!(report.messages_received, 0);
    assert!(report.failure.unwrap().contains("stream closed"));
    let retained: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(retained["direction"], "firmware-to-pool-incomplete-frame");
    assert_eq!(retained["message"]["raw_hex"], "0900535452320141455347");
}

fn exercise_fresh_jobs(rolling: bool, command_transport: bool) {
    exercise_session(
        rolling,
        command_transport,
        "close-after-work",
        ResponsePolicy::JsonRpc1StratumV1,
    );
}

#[test]
fn old_frozen_response_contract_remains_explicit() {
    exercise_session(
        true,
        false,
        "close-after-work",
        ResponsePolicy::LegacyFalseCode23,
    );
}

#[test]
fn persistent_connection_verifies_later_work_and_preserves_failure_bounds() {
    for ending in ["peer", "window", "partial", "cancel", "budget"] {
        exercise_session(true, false, ending, ResponsePolicy::JsonRpc1StratumV1);
    }
}

#[test]
fn persistent_emulator_command_checks_fresh_work_and_every_shutdown_path() {
    for ending in ["peer", "window", "partial", "cancel", "budget"] {
        exercise_session(true, true, ending, ResponsePolicy::JsonRpc1StratumV1);
    }
}

fn exercise_session(rolling: bool, command_transport: bool, ending: &str, policy: ResponsePolicy) {
    let result = if policy == ResponsePolicy::LegacyFalseCode23 {
        json!(false)
    } else {
        Value::Null
    };
    let rejected = |client: &mut File, id: u64, code: i32, message: &str| {
        let code = if policy == ResponsePolicy::LegacyFalseCode23 {
            23
        } else {
            code
        };
        assert_eq!(
            receive(client),
            json!({"id":id,"result":result,"error":[code,message,null]})
        );
    };
    let persistent = ending != "close-after-work";
    let first_bits = if rolling { 0x1000_0000 } else { 0 };
    let second_bits = if rolling { 0x0000_2000 } else { 0 };
    let submit = |id, job: &Value, nonce, time, bits| {
        let mut value = submission(id, job, nonce, time);
        if rolling {
            value["params"]
                .as_array_mut()
                .unwrap()
                .push(json!(format!("{bits:08x}")));
        }
        value
    };
    let dir = tempfile::tempdir().unwrap();
    let mut seed = [0; 32];
    File::open("/dev/urandom")
        .unwrap()
        .read_exact(&mut seed)
        .unwrap();
    let (a, b) = UnixStream::pair().unwrap();
    let a = File::from(OwnedFd::from(a));
    let mut client = File::from(OwnedFd::from(b));
    let config = StratumConfig {
        response_policy: policy,
        regtest_anchor: None,
        difficulty_scale: md_sim_mining::stratum::DifficultyScale::ThreeHalves,
        limits: md_sim_mining::stratum::StratumLimits {
            max_submissions: if ending == "budget" { 11 } else { 5000 },
            ..Default::default()
        },
        seed,
        time: 1_750_000_000,
        difficulty_exponents: vec![-31, -30],
        timeout: Duration::from_secs(if ending == "window" { 1 } else { 8 }),
        trace_path: dir.path().join("stratum.jsonl"),
    };
    let stop = Arc::new(AtomicBool::new(false));
    let signal = stop.clone();
    let server = std::thread::spawn(move || {
        if persistent && command_transport {
            let writer = a.try_clone().unwrap();
            md_sim_mining::stratum::serve_command_window(a, writer, config, stop).unwrap()
        } else if persistent {
            md_sim_mining::stratum::serve_inherited_window(a, config, stop).unwrap()
        } else if command_transport {
            let writer = a.try_clone().unwrap();
            md_sim_mining::stratum::serve_command_stream(a, writer, config, stop).unwrap()
        } else {
            serve_inherited(a, config, stop).unwrap()
        }
    });
    send(
        &mut client,
        json!({"id":0,"method":"mining.submit","params":[]}),
    );
    assert_eq!(
        receive(&mut client),
        json!({"id":0,"result":result,"error":[24,"not authorized",null]})
    );
    send(
        &mut client,
        json!({"id":30,"method":"mining.authorize","params":["wrong","x"]}),
    );
    assert_eq!(
        receive(&mut client),
        json!({"id":30,"result":false,"error":null})
    );
    send(
        &mut client,
        json!({"id":31,"method":"mining.authorize","params":["simulator","x"]}),
    );
    assert_eq!(
        receive(&mut client),
        json!({"id":31,"result":true,"error":null})
    );
    send(
        &mut client,
        json!({"id":32,"method":"mining.submit","params":[]}),
    );
    let unsubscribed_error = if policy == ResponsePolicy::LegacyFalseCode23 {
        json!([24, "not authorized", null])
    } else {
        json!([25, "not subscribed", null])
    };
    assert_eq!(
        receive(&mut client),
        json!({"id":32,"result":result,"error":unsubscribed_error})
    );
    send(
        &mut client,
        json!({"id":33,"method":"mining.authorize","params":["wrong","x"]}),
    );
    assert_eq!(
        receive(&mut client),
        json!({"id":33,"result":false,"error":null})
    );
    if rolling {
        send(
            &mut client,
            json!({
                "id":10,"method":"mining.configure","params":[["version-rolling"],{
                    "version-rolling.mask":"1fffe000","version-rolling.min-bit-count":16
                }]
            }),
        );
        let reply = receive(&mut client);
        assert_eq!(reply["result"]["version-rolling"], true);
        assert_eq!(reply["result"]["version-rolling.mask"], "1fffe000");
    }
    // Split one real wire frame over two writes.
    client
        .write_all(b"{\"id\":1,\"method\":\"mining.sub")
        .unwrap();
    client.write_all(b"scribe\",\"params\":[]}\n").unwrap();
    let subscription = receive(&mut client);
    let extra1 = subscription["result"][1].as_str().unwrap();
    send(
        &mut client,
        json!({"id":2,"method":"mining.authorize","params":["simulator","x"]}),
    );
    assert_eq!(receive(&mut client)["result"], true);
    assert_eq!(receive(&mut client)["params"][0], 1.5 * 2f64.powi(-31));
    let first = receive(&mut client);
    assert_eq!(first["method"], "mining.notify");
    let mut header = decode_header(&first, extra1, "00000000");
    header.version |= first_bits;
    send(
        &mut client,
        json!({"id":34,"method":"mining.submit","params":[]}),
    );
    let malformed = receive(&mut client);
    assert_eq!(malformed["id"], 34);
    assert_eq!(malformed["result"], result);
    assert_eq!(malformed["error"][0], 20);
    assert!(malformed["error"][1].is_string());
    assert_eq!(malformed["error"][2], Value::Null);
    assert_eq!(malformed["error"].as_array().unwrap().len(), 3);
    let mut other_worker = submit(35, &first, 0, header.time, first_bits);
    other_worker["params"][0] = json!("other-worker");
    send(&mut client, other_worker);
    let worker_code = if policy == ResponsePolicy::LegacyFalseCode23 {
        20
    } else {
        24
    };
    assert_eq!(
        receive(&mut client),
        json!({"id":35,"result":result,"error":[worker_code,"submission worker differs from authorized worker",null]})
    );
    let mut invalid_worker = submit(36, &first, 0, header.time, first_bits);
    invalid_worker["params"][0] = json!(42);
    send(&mut client, invalid_worker);
    assert_eq!(
        receive(&mut client),
        json!({"id":36,"result":result,"error":[20,"submission worker differs from authorized worker",null]})
    );
    if rolling {
        send(&mut client, submission(11, &first, 0, header.time));
        assert_eq!(receive(&mut client)["error"][0], 20);
        send(&mut client, submit(12, &first, 0, header.time, 0x2000_0000));
        assert_eq!(receive(&mut client)["error"][0], 20);
    }
    let target = md_sim_mining::stratum::difficulty_target_scaled(
        -31,
        md_sim_mining::stratum::DifficultyScale::ThreeHalves,
    )
    .unwrap();
    let bad = (0..1000)
        .find(|nonce| {
            header.nonce = *nonce;
            !target.accepts(&header.digest())
        })
        .unwrap();
    send(&mut client, submit(3, &first, bad, header.time, first_bits));
    rejected(&mut client, 3, 23, "LowDifficulty");
    send(&mut client, submit(4, &first, bad, header.time, first_bits));
    rejected(&mut client, 4, 22, "Duplicate");
    send(
        &mut client,
        submit(5, &first, bad, header.time - 1, first_bits),
    );
    rejected(&mut client, 5, 20, "OutOfRange");
    let good = (0..1000)
        .find(|nonce| {
            header.nonce = *nonce;
            target.accepts(&header.digest())
        })
        .unwrap();
    send(
        &mut client,
        submit(6, &first, good, header.time, first_bits),
    );
    assert_eq!(receive(&mut client)["result"], true);
    assert_eq!(receive(&mut client)["params"][0], 1.5 * 2f64.powi(-30));
    let second = receive(&mut client);
    assert_ne!(first["params"][0], second["params"][0]);
    assert_eq!(second["params"][8], true);
    send(
        &mut client,
        submit(7, &first, good, header.time, first_bits),
    );
    rejected(&mut client, 7, 21, "Stale");
    let mut h2 = decode_header(&second, extra1, "00000000");
    h2.version |= second_bits;
    let target2 = md_sim_mining::stratum::difficulty_target_scaled(
        -30,
        md_sim_mining::stratum::DifficultyScale::ThreeHalves,
    )
    .unwrap();
    let good2 = (0..1000)
        .find(|nonce| {
            h2.nonce = *nonce;
            target2.accepts(&h2.digest())
        })
        .unwrap();
    send(&mut client, submit(8, &second, good2, h2.time, second_bits));
    assert_eq!(receive(&mut client)["result"], true);
    let mut extra_header = None;
    if persistent {
        send(
            &mut client,
            submit(20, &second, good2, h2.time, second_bits),
        );
        rejected(&mut client, 20, 22, "Duplicate");
        let extra = (good2 + 1..2000)
            .find(|nonce| {
                h2.nonce = *nonce;
                target2.accepts(&h2.digest())
            })
            .unwrap();
        extra_header = Some(h2.clone());
        send(
            &mut client,
            submit(21, &second, extra, h2.time, second_bits),
        );
        assert_eq!(receive(&mut client)["result"], true);
        send(
            &mut client,
            submit(22, &first, good, header.time, first_bits),
        );
        rejected(&mut client, 22, 21, "Stale");
        send(
            &mut client,
            submit(23, &second, extra, h2.time - 1, second_bits),
        );
        rejected(&mut client, 23, 20, "OutOfRange");
        let bad2 = (0..2000)
            .find(|nonce| {
                h2.nonce = *nonce;
                !target2.accepts(&h2.digest())
            })
            .unwrap();
        send(&mut client, submit(24, &second, bad2, h2.time, second_bits));
        rejected(&mut client, 24, 23, "LowDifficulty");
        h2.nonce = good2;
        match ending {
            "partial" => client.write_all(b"{\"id\":99").unwrap(),
            "cancel" => signal.store(true, Ordering::Release),
            "budget" => send(
                &mut client,
                submit(25, &second, extra, h2.time, second_bits),
            ),
            _ => {}
        }
    }
    let _held = if matches!(ending, "window" | "cancel" | "budget") {
        Some(client)
    } else {
        drop(client);
        None
    };
    let report = server.join().unwrap();
    assert_eq!(report.response_policy, policy);
    assert!(report.completed);
    assert_eq!(report.stopped, ending == "cancel");
    assert!(!report.timed_out);
    assert_eq!(report.budget_exhausted, ending == "budget");
    assert_eq!(
        report.failure.is_some(),
        matches!(ending, "partial" | "budget")
    );
    if ending == "partial" {
        assert!(report.failure.as_deref().unwrap().contains("partial frame"));
    }
    assert_eq!(
        report.connection_policy == md_sim_mining::stratum::CompletionPolicy::KeepOpen,
        persistent
    );
    if ending == "window" {
        assert_eq!(report.connection_end.as_deref(), Some("planned-window-end"));
    }
    if ending == "peer" {
        assert_eq!(report.connection_end.as_deref(), Some("peer-closed"));
    }
    let verdicts: Vec<_> = report.submissions.iter().map(|s| &s.verdict).collect();
    assert_eq!(verdicts[0], &Verdict::LowDifficulty);
    assert_eq!(verdicts[1], &Verdict::Duplicate);
    assert_eq!(verdicts[2], &Verdict::OutOfRange);
    assert_eq!(verdicts[4], &Verdict::Stale);
    if let Some(expected) = extra_header {
        assert_eq!(verdicts[6], &Verdict::Duplicate);
        let Verdict::Accepted {
            header_hex,
            hash_display,
            ..
        } = verdicts[7]
        else {
            panic!("later work was not verified");
        };
        assert_eq!(*header_hex, hex::encode(expected.encode()));
        assert_eq!(
            *hash_display,
            hex::encode(expected.digest().iter().rev().copied().collect::<Vec<_>>())
        );
        assert_eq!(verdicts[8], &Verdict::Stale);
        assert_eq!(verdicts[9], &Verdict::OutOfRange);
        assert_eq!(verdicts[10], &Verdict::LowDifficulty);
    }
    for (index, expected) in [(3, header), (5, h2)] {
        let Verdict::Accepted {
            header_hex,
            hash_display,
            ..
        } = verdicts[index]
        else {
            panic!("expected accepted computed header");
        };
        assert_eq!(*header_hex, hex::encode(expected.encode()));
        let digest: Vec<_> = expected.digest().iter().rev().copied().collect();
        assert_eq!(*hash_display, hex::encode(digest));
    }
    assert_eq!(
        verdicts
            .iter()
            .filter(|v| matches!(v, Verdict::Accepted { .. }))
            .count(),
        if persistent { 3 } else { 2 }
    );
}

#[test]
fn repeated_unknown_submissions_stop_with_bounded_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = UnixStream::pair().unwrap();
    let mut client = File::from(OwnedFd::from(b));
    let config = StratumConfig {
        response_policy: ResponsePolicy::JsonRpc1StratumV1,
        regtest_anchor: None,
        difficulty_scale: md_sim_mining::stratum::DifficultyScale::One,
        seed: [7; 32],
        time: 1_750_000_000,
        difficulty_exponents: vec![-31, -30],
        timeout: Duration::from_secs(2),
        trace_path: dir.path().join("bounded.jsonl"),
        limits: md_sim_mining::stratum::StratumLimits {
            max_submissions: 1,
            ..Default::default()
        },
    };
    let server = std::thread::spawn(move || {
        serve_inherited(
            File::from(OwnedFd::from(a)),
            config,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
    });
    send(
        &mut client,
        json!({"id":1,"method":"mining.subscribe","params":[]}),
    );
    receive(&mut client);
    send(
        &mut client,
        json!({"id":2,"method":"mining.authorize","params":["simulator","x"]}),
    );
    receive(&mut client);
    receive(&mut client);
    receive(&mut client);
    let request = json!({"id":3,"method":"mining.submit","params":["simulator","unknown","00000000","684ee180","00000000"]});
    send(&mut client, request.clone());
    assert_eq!(
        receive(&mut client),
        json!({"id":3,"result":null,"error":[21,"UnknownJob",null]})
    );
    send(&mut client, request);
    let report = server.join().unwrap();
    assert!(!report.completed);
    assert!(report.budget_exhausted);
    assert_eq!(report.submissions.len(), 1);
    assert_eq!(report.submissions[0].verdict, Verdict::UnknownJob);
    assert!(report.failure.unwrap().contains("submission budget"));
}

#[test]
fn trace_budget_stops_without_discarding_report() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tiny.jsonl");
    let (a, b) = UnixStream::pair().unwrap();
    let mut client = File::from(OwnedFd::from(b));
    let config = StratumConfig {
        response_policy: ResponsePolicy::JsonRpc1StratumV1,
        regtest_anchor: None,
        difficulty_scale: md_sim_mining::stratum::DifficultyScale::One,
        seed: [8; 32],
        time: 1_750_000_000,
        difficulty_exponents: vec![-31, -30],
        timeout: Duration::from_secs(2),
        trace_path: path.clone(),
        limits: md_sim_mining::stratum::StratumLimits {
            trace_bytes: 1,
            ..Default::default()
        },
    };
    let server = std::thread::spawn(move || {
        serve_inherited(
            File::from(OwnedFd::from(a)),
            config,
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap()
    });
    send(
        &mut client,
        json!({"id":1,"method":"mining.subscribe","params":[]}),
    );
    let report = server.join().unwrap();
    assert!(!report.completed);
    assert!(report.budget_exhausted);
    assert!(report.failure.unwrap().contains("trace budget"));
    assert!(std::fs::metadata(path).unwrap().len() <= 1);
}

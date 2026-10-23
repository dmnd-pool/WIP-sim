use md_sim_qemu_core::*;
use std::ffi::CStr;

struct Core {
    state: *mut S9State,
    traces: Vec<(u64, u32, u32, u32)>,
    grants: Vec<(u64, u64)>,
    tx: Vec<u8>,
    elapsed_ns: u64,
}
impl Drop for Core {
    fn drop(&mut self) {
        unsafe { md_s9_free(self.state) }
    }
}
impl Core {
    fn new() -> Self {
        let state = md_s9_new_bm1387(0x74657374, 63, 114, 25_000_000, 26, 0x90, 1024);
        assert!(!state.is_null());
        Self {
            state,
            traces: Vec::new(),
            grants: Vec::new(),
            tx: Vec::new(),
            elapsed_ns: 0,
        }
    }
    fn error(&self) -> String {
        unsafe { CStr::from_ptr(md_s9_error(self.state)) }
            .to_str()
            .unwrap()
            .to_string()
    }
    fn trace(&mut self) {
        let mut event = AsicTrace::default();
        while unsafe { md_s9_take_asic_trace(self.state, &mut event) } {
            if event.kind == 10 {
                self.grants.push((event.grant, event.wall_ns));
            } else {
                assert_eq!((event.grant, event.wall_ns), (0, 0));
            }
            self.traces
                .push((event.ns, event.kind, event.value, event.extra));
        }
    }
    fn pin(&mut self, pin: u32, level: bool) {
        assert!(
            unsafe { md_s9_asic_pin(self.state, pin, level, 3) },
            "{}",
            self.error()
        );
        self.trace();
    }
    fn write(&self, address: u32, value: u32) {
        assert!(unsafe { md_s9_write(self.state, address, 4, value) });
    }
    fn read(&self, address: u32) -> u32 {
        let mut value = 0xdeadbeef;
        assert_eq!(unsafe { md_s9_read(self.state, address, 4, &mut value) }, 0);
        value
    }
    fn advance(&mut self, clocks: u64) {
        self.advance_with_input(clocks, false);
    }
    fn advance_with_input(&mut self, mut clocks: u64, rxd: bool) {
        while clocks > 0 {
            let mut event = S9Event::default();
            assert!(
                unsafe { md_s9_advance(self.state, clocks, rxd, &mut event) },
                "{}",
                self.error()
            );
            assert!(event.clocks > 0 && event.clocks <= clocks);
            clocks -= event.clocks;
            self.elapsed_ns += event.clocks * 5;
            if event.flags & 1 != 0 {
                self.tx.push(event.tx_byte);
            }
            self.trace();
        }
    }
}

#[test]
fn fpga_and_63_asics_preserve_every_wire_edge_across_late_and_split_advancement() {
    let mut baseline = None;
    for chunk in [9_000_000, 314_159, 257] {
        let mut core = Core::new();
        core.pin(0, true);
        core.pin(1, true);
        core.write(0x10, 0x6b);
        core.write(8, 8);
        core.write(0x1008, 4);
        core.advance(2000);
        core.write(0x1004, 0x00000554);
        let mut remaining = 9_000_000;
        while remaining != 0 {
            let count = remaining.min(chunk);
            core.advance(count);
            remaining -= count;
        }
        assert_eq!(core.tx, [0x54, 5, 0, 0, 0x19]);
        assert_eq!(core.read(0x18), 0); // No checksum errors.
        assert_eq!(unsafe { md_s9_status(core.state) } & 0x70, 0);
        let commands: Vec<_> = core.traces.iter().filter(|e| e.1 == 4).collect();
        assert_eq!(commands.len(), 1);
        assert_eq!((commands[0].2, commands[0].3), (0x54, 63));
        let sent: Vec<_> = core
            .traces
            .iter()
            .filter(|e| e.1 == 2)
            .map(|e| e.2 as u8)
            .collect();
        assert_eq!(sent, [0x13, 0x87, 0x90, 0, 0, 0, 7].repeat(63));
        // Read the real FPGA FIFO only after the complete serial exchange.
        // 126words fit its archived256-word command receive queue.
        for _ in 0..63 {
            assert_eq!(core.read(0x100c) & 1, 0);
            assert_eq!(core.read(0x1000), 0x00908713);
            assert_eq!(core.read(0x1000), 0x07000000);
        }
        assert_eq!(core.read(0x100c) & 1, 1);
        if let Some(expected) = &baseline {
            assert_eq!(&core.traces, expected);
        } else {
            baseline = Some(core.traces.clone());
        }
    }
}

#[test]
fn invalid_peer_configuration_and_pin_phases_fail_closed() {
    assert!(md_s9_new_bm1387(0, 65, 114, 25_000_000, 26, 0x90, 1024).is_null());
    let core = Core::new();
    assert!(!unsafe { md_s9_asic_pin(core.state, 0, true, 5) });
    assert!(core.error().contains("phase"));
    assert_ne!(unsafe { md_s9_status(core.state) } & 64, 0);
    unsafe { md_s9_reset(core.state) };
    assert_eq!(unsafe { md_s9_status(core.state) } & 64, 0);
    assert!(!unsafe { md_s9_asic_pin(core.state, 2, true, 0) });
    assert!(core.error().contains("unknown ASIC pin"));
}

#[test]
fn a_preempted_multiword_command_advances_one_second_without_per_clock_polling() {
    let mut core = Core {
        state: md_s9_new_reference_v1(0x74657374),
        traces: Vec::new(),
        grants: Vec::new(),
        tx: Vec::new(),
        elapsed_ns: 0,
    };
    assert!(!core.state.is_null());
    core.write(0x10, 1);
    core.write(0x14, 7);
    core.write(8, 8);
    core.advance_with_input(1000, true);
    // First word of the archived nine-byte command. The hardware waits
    // for its second word without dropping the first or sending a prefix.
    core.write(0x1004, 201_328_968);
    core.advance_with_input(1000, true);
    let mut idle = S9Idle::default();
    unsafe { md_s9_idle_details(core.state, true, &mut idle) };
    assert_eq!(idle.transmit, u64::MAX);
    let status = unsafe { md_s9_status(core.state) };
    let mut event = S9Event::default();
    assert!(unsafe { md_s9_advance(core.state, 200_000_000, true, &mut event) });
    assert_eq!(event.clocks, 200_000_000);
    assert_eq!(event.flags, 0);
    assert_eq!(unsafe { md_s9_status(core.state) }, status);
    assert!(core.tx.is_empty());
    core.write(0x1004, 553_805_824);
    core.advance_with_input(5000, true);
    assert_eq!(core.tx, [72, 9, 0, 12, 0, 104, 2, 33, 2]);
}

#[test]
fn reference_peers_retain_shared_budget_after_the_creating_handle_is_released() {
    for cap in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e-30] {
        assert!(md_hash_budget_new(cap).is_null());
    }
    unsafe {
        assert!(md_s9_new_bm1387_reference_mining(0, std::ptr::null(), 1024, 32).is_null());
        for cap in [10_000.0, 1.0, 0.01] {
            let budget = md_hash_budget_new(cap);
            assert!(!budget.is_null());
            for bits in [0, 33, u32::MAX] {
                assert!(md_s9_new_bm1387_reference_mining(0, budget, 1024, bits).is_null());
            }
            let peers: Vec<_> = [32, 12, 1]
                .into_iter()
                .map(|bits| md_s9_new_bm1387_reference_mining(0, budget, 1024, bits))
                .collect();
            md_hash_budget_free(budget);
            for peer in peers {
                assert!(!peer.is_null());
                assert!(md_s9_asic_pin(peer, 0, true, 0));
                assert!(md_s9_asic_pin(peer, 1, true, 0));
                md_s9_reset(peer);
                assert_eq!(md_s9_status(peer) & 64, 0);
                md_s9_free(peer);
            }
        }
    }
}

#[test]
fn fresh_fpga_work_reaches_real_nonce_search_with_independent_full_header_checks() {
    check_fresh_fpga_work(32, false);
}

#[test]
fn adapted_filter_preserves_computed_hashes_and_transmits_real_results() {
    check_fresh_fpga_work(1, false);
}

#[test]
fn actual_hash_search_continues_while_the_controller_clock_is_stopped() {
    check_fresh_fpga_work(1, true);
}

fn peer_until(core: &mut Core, until: u64) {
    loop {
        let mut reached = u64::MAX;
        assert!(
            unsafe { md_s9_peer_advance(core.state, until, &mut reached) },
            "{}",
            core.error()
        );
        core.trace();
        if reached == until {
            break;
        }
        assert!(reached < until);
    }
}

fn check_fresh_fpga_work(fixed_filter_bits: u32, gate_last_job: bool) {
    use bitcoin_hashes::{sha256, sha256d, Hash, HashEngine};
    use std::{
        collections::BTreeMap,
        io::Read,
        time::{Duration, Instant},
    };
    let state = unsafe {
        let budget = md_hash_budget_new(10000.0);
        let state = md_s9_new_bm1387_reference_mining(0x74657374, budget, 1024, fixed_filter_bits);
        md_hash_budget_free(budget);
        state
    };
    assert!(!state.is_null());
    let mut core = Core {
        state,
        traces: Vec::new(),
        grants: Vec::new(),
        tx: Vec::new(),
        elapsed_ns: 0,
    };
    core.pin(0, true);
    core.pin(1, true);
    core.write(0x10, 107);
    core.write(8, 12); // Four midstates, enabled, BM1387 reference mode.
    core.advance(2000);
    core.write(0x1004, 0x00000555);
    for chip in 0..63 {
        core.write(0x1004, 0x00000541 | (chip * 4) << 16);
    }
    for (register, value) in [(0x0c, 0x006d0511u32), (0x18, 0), (0x1c, 0x40201a80)] {
        core.write(0x1004, 0x00000958 | register << 24);
        core.write(0x1004, value.swap_bytes());
    }
    core.advance(7_000_000);
    assert!(core.traces.iter().filter(|event| event.1 == 4).count() >= 67);
    let mut headers = BTreeMap::new();
    let start = Instant::now();
    for id in [0, 4] {
        let mut header = [0; 80];
        std::fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut header)
            .unwrap();
        header[72..76].copy_from_slice(&0x207fffffu32.to_le_bytes());
        let version = u32::from_le_bytes(header[..4].try_into().unwrap());
        let mut words = vec![id];
        for offset in [72, 68, 64] {
            words.push(u32::from_le_bytes(
                header[offset..offset + 4].try_into().unwrap(),
            ));
        }
        for (index, mask) in [0, 0x2000, 0x4000, 0x6000].into_iter().enumerate() {
            header[..4].copy_from_slice(&(version ^ mask).to_le_bytes());
            let mut hash = sha256::Hash::engine();
            hash.input(&header[..64]);
            let midstate = hash.midstate().to_byte_array();
            for word in midstate.chunks_exact(4).rev() {
                words.push(u32::from_be_bytes(word.try_into().unwrap()));
            }
            headers.insert((id, index as u32), header);
        }
        assert_eq!(words.len(), 36);
        for word in words {
            core.write(0x3004, word);
        }
        core.advance(3_000_000);
        assert!(core
            .traces
            .iter()
            .any(|event| event.1 == 11 && event.2 == id && event.3 == 63 * 114));
        let initial_hashes = core
            .traces
            .iter()
            .filter(|event| event.1 == 10 && event.3 & 255 == id)
            .count();
        let required_hashes = if gate_last_job && id == 4 {
            initial_hashes + 16
        } else {
            16
        };
        while core
            .traces
            .iter()
            .filter(|event| event.1 == 10 && event.3 & 255 == id)
            .count()
            < required_hashes
        {
            assert!(start.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_micros(100));
            if gate_last_job && id == 4 {
                let before = unsafe { md_s9_status(core.state) };
                core.elapsed_ns += 100_000;
                let until = core.elapsed_ns;
                peer_until(&mut core, until);
                assert_eq!(unsafe { md_s9_status(core.state) }, before);
            } else {
                core.advance(20_000);
            }
        }
    }
    let mut count = 0;
    for &(_, kind, nonce, extra) in &core.traces {
        if kind != 10 {
            continue;
        }
        let mut header = headers[&(extra & 255, (extra >> 8) & 255)];
        header[76..].copy_from_slice(&nonce.to_le_bytes());
        let digest = sha256d::Hash::hash(&header).to_byte_array();
        assert_eq!(
            extra >> 16,
            u32::from(
                u32::from_le_bytes(digest[28..].try_into().unwrap()) >> (32 - fixed_filter_bits)
                    == 0
            )
        );
        assert!((nonce >> 2) & 63 < 63);
        assert!(matches!((nonce>>24)&127,0..=56|64..=120));
        count += 1;
    }
    assert!(count >= 32);
    assert_eq!(core.grants.len() as u64, count);
    let mut last_ns = 0;
    for (index, &(ordinal, wall_ns)) in core.grants.iter().enumerate() {
        assert_eq!(ordinal, index as u64 + 1);
        assert!(wall_ns - last_ns >= 100_000);
        last_ns = wall_ns;
    }
    if fixed_filter_bits < 32 {
        assert!(core
            .traces
            .iter()
            .any(|event| event.1 == 10 && event.3 & 0x10000 != 0));
        assert!(core.traces.iter().any(|event| event.1 == 2));
    }
    assert_eq!(core.read(0x18), 0);
    assert_eq!(unsafe { md_s9_status(core.state) } & 0x70, 0);
    // First grant may be due at start; there is no accumulated idle credit.
    assert!(start.elapsed() >= Duration::from_micros((count - 1) * 100));
    if gate_last_job {
        assert!(unsafe { md_s9_asic_pin_at(core.state, 0, false, core.elapsed_ns) });
        core.trace();
    } else {
        core.pin(0, false);
    }
    let before = core.traces.iter().filter(|event| event.1 == 10).count();
    if gate_last_job {
        let until = core.elapsed_ns + 10_000_000;
        peer_until(&mut core, until);
    } else {
        core.advance(2_000_000);
    }
    assert_eq!(
        core.traces.iter().filter(|event| event.1 == 10).count(),
        before
    );
}

fn timed_advance(core: &mut Core, edge: &mut u128, period: u64, mut clocks: u64) {
    while clocks > 0 {
        let clock = S9Clock {
            edge_ns: (*edge >> 32) as u64,
            fraction: *edge as u32,
            period_q32: period,
        };
        let mut event = S9Event::default();
        assert!(
            unsafe { md_s9_advance_timed(core.state, &clock, clocks, true, &mut event) },
            "{}",
            core.error()
        );
        assert!(event.clocks > 0 && event.clocks <= clocks);
        clocks -= event.clocks;
        *edge += u128::from(event.clocks) * u128::from(period);
        if event.flags & 1 != 0 {
            core.tx.push(event.tx_byte);
        }
        core.trace();
    }
}

#[test]
fn independent_asic_uart_works_with_three_fabric_periods_including_a_fraction() {
    for (period, divisor) in [(5 << 32, 107), (10 << 32, 53), ((7 << 32) + (1 << 31), 71)] {
        let mut core = Core::new();
        core.pin(0, true);
        core.pin(1, true);
        core.write(0x10, divisor);
        core.write(8, 8);
        core.write(0x1008, 4);
        let mut edge = 0;
        timed_advance(&mut core, &mut edge, period, 2000);
        core.write(0x1004, 0x00000554);
        // At least45ms for the five-byte command and441-byte reply.
        let count = ((45_000_000u128 << 32) / u128::from(period)) as u64;
        timed_advance(&mut core, &mut edge, period, count);
        assert_eq!(core.tx, [0x54, 5, 0, 0, 0x19]);
        let sent: Vec<_> = core
            .traces
            .iter()
            .filter(|e| e.1 == 2)
            .map(|e| e.2 as u8)
            .collect();
        assert_eq!(sent, [0x13, 0x87, 0x90, 0, 0, 0, 7].repeat(63));
        for _ in 0..63 {
            assert_eq!(core.read(0x1000), 0x00908713);
            assert_eq!(core.read(0x1000), 0x07000000);
        }
        assert_eq!(core.read(0x100c) & 1, 1);
        assert_eq!(core.read(0x18), 0);
        assert_eq!(unsafe { md_s9_status(core.state) } & 0x70, 0);
    }
}

#[test]
fn gating_the_fabric_does_not_delay_or_queue_the_physical_reply() {
    let mut core = Core::new();
    core.pin(0, true);
    core.pin(1, true);
    core.write(0x10, 107);
    core.write(8, 8);
    core.write(0x1008, 4);
    let mut edge = 0;
    timed_advance(&mut core, &mut edge, 5 << 32, 2000);
    core.write(0x1004, 0x00000554);
    while !core.traces.iter().any(|e| e.1 == 4) {
        timed_advance(&mut core, &mut edge, 5 << 32, 1);
        assert!(edge < 1_000_000u128 << 32);
    }
    assert_eq!(core.read(0x100c) & 1, 1);
    let status = unsafe { md_s9_status(core.state) };
    peer_until(&mut core, 50_000_000);
    assert_eq!(unsafe { md_s9_status(core.state) }, status);
    assert_eq!(core.traces.iter().filter(|e| e.1 == 2).count(), 441);
    assert_eq!(core.read(0x100c) & 1, 1); // No samples were taken during the gate.
    edge = 50_000_000u128 << 32;
    timed_advance(&mut core, &mut edge, 10 << 32, 1000);
    assert_eq!(core.read(0x100c) & 1, 1); // The reply cannot appear on resume.
}

#[test]
fn timed_calls_reject_backwards_phase_subnanosecond_edges_and_time_overflow() {
    for clock in [
        S9Clock {
            edge_ns: 0,
            fraction: 0,
            period_q32: 0,
        },
        S9Clock {
            edge_ns: 0,
            fraction: 0,
            period_q32: (1 << 32) - 1,
        },
        S9Clock {
            edge_ns: u64::MAX,
            fraction: 0,
            period_q32: 5 << 32,
        },
    ] {
        let core = Core::new();
        let mut event = S9Event::default();
        assert!(!unsafe { md_s9_advance_timed(core.state, &clock, 1, true, &mut event) });
        assert_ne!(unsafe { md_s9_status(core.state) } & 64, 0);
    }
    let mut core = Core::new();
    let mut edge = 0;
    timed_advance(&mut core, &mut edge, 5 << 32, 2000);
    let clock = S9Clock {
        edge_ns: 0,
        fraction: 0,
        period_q32: 5 << 32,
    };
    let mut event = S9Event::default();
    assert!(!unsafe { md_s9_advance_timed(core.state, &clock, 1, true, &mut event) });
}

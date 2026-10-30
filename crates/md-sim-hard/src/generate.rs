//! Sequence generators. A planner runs its own copy of the model while it
//! writes steps, so it knows when every chain is quiet. Reads planned at
//! quiet points are compared exactly. Planning assumes slower bus steps than
//! the circuit has, so a planned quiet point is never early.
use crate::{
    model::Mapping,
    rng::Rng,
    sequence::{PadEvent, Sequence, Step, CHAINS},
};
use md_sim_firmware::s9io::{
    registers::{
        Registers, Revision, BAUD, BUILD_ID, COMMAND_CONTROL, COMMAND_RX, COMMAND_STATUS,
        COMMAND_TX, CONTROL, ERRORS, LAST_WORK_ID, VERSION, WORK_RX, WORK_RX_CONTROL,
        WORK_RX_STATUS, WORK_TIME, WORK_TX, WORK_TX_CONTROL, WORK_TX_STATUS, WORK_TX_THRESHOLD,
    },
    response_words, Midstates,
};

/// Planned cycles per bus step. The circuit needs about 9 to 12.
pub const STEP_COST: u64 = 16;

/// Registers read in quiet phases. All are free of read side effects.
pub const STATUS_REGISTERS: [u16; 14] = [
    VERSION,
    BUILD_ID,
    CONTROL,
    BAUD,
    WORK_TIME,
    ERRORS,
    COMMAND_CONTROL,
    COMMAND_STATUS,
    WORK_RX_CONTROL,
    WORK_RX_STATUS,
    WORK_TX_CONTROL,
    WORK_TX_STATUS,
    WORK_TX_THRESHOLD,
    LAST_WORK_ID,
];

/// Model clock cycles per serial bit: 16 samples of `divisor + 1` cycles.
pub fn bit_period(divisor: u16) -> u64 {
    16 * (u64::from(divisor) + 1)
}

pub struct Planner {
    regs: Vec<Registers>,
    /// Planned circuit cycle.
    t: u64,
    /// Model clock cycles per circuit cycle.
    ratio: u64,
    rxd: [bool; CHAINS],
    pending: Vec<PadEvent>,
    next: usize,
    mapping: Mapping,
    broken: [bool; CHAINS],
    rx_free: [u64; CHAINS],
    stable_since: [u64; CHAINS],
    last_snapshot: [[u32; 7]; CHAINS],
    pub seq: Sequence,
}

/// Unchanged visible state for this long counts as quiet. It is longer
/// than ten bit periods at the largest generated divisor.
pub const STABLE_CYCLES: u64 = 4096;

/// Serial wire time per chain and direction in one random phase.
pub const PHASE_WIRE_CYCLES: u64 = 12_000;

impl Planner {
    pub fn new(seed: u64, kind: &str, mapping: Mapping, build_id: u32, ratio: u64) -> Self {
        Self {
            regs: (0..CHAINS)
                .map(|_| Registers::with_revision(build_id, Revision::Reference1_0_2))
                .collect(),
            t: 0,
            ratio: ratio.max(1),
            rxd: [true; CHAINS],
            pending: Vec::new(),
            next: 0,
            mapping,
            broken: [false; CHAINS],
            rx_free: [0; CHAINS],
            stable_since: [0; CHAINS],
            last_snapshot: [[u32::MAX; 7]; CHAINS],
            seq: Sequence::new(seed, kind),
        }
    }

    pub fn now(&self) -> u64 {
        self.t
    }

    fn tick(&mut self, read: Option<(usize, u16)>, write: Option<(usize, u16, u32, u8)>) {
        while self.next < self.pending.len() && self.pending[self.next].cycle <= self.t {
            let event = self.pending[self.next];
            for chain in 0..CHAINS {
                if self.mapping.rx[chain] == Some(event.pad) {
                    self.rxd[chain] = event.level;
                }
            }
            self.next += 1;
        }
        for sub in 0..self.ratio {
            for chain in 0..CHAINS {
                if self.broken[chain] {
                    continue;
                }
                // The bus strobe falls on the first model cycle only.
                let r = read.filter(|x| x.0 == chain && sub == 0).map(|x| x.1);
                let w = write
                    .filter(|x| x.0 == chain && sub == 0)
                    .map(|x| (x.1, x.2, x.3));
                match self.regs[chain].cycle(self.rxd[chain], r, w) {
                    Ok(cycle) => {
                        if matches!(cycle.write, Some(Err(_))) {
                            self.broken[chain] = true;
                        }
                    }
                    Err(_) => self.broken[chain] = true,
                }
            }
        }
        self.t += 1;
        self.track_stability();
    }

    fn advance(&mut self, cycles: u64) {
        for _ in 0..cycles {
            self.tick(None, None);
        }
    }

    /// No pin events are pending and every chain is idle. A chain also
    /// counts as idle when its visible state has not changed for
    /// `STABLE_CYCLES`, for example a disabled chain holding received bytes.
    pub fn quiet(&self) -> bool {
        self.next >= self.pending.len()
            && (0..CHAINS).all(|c| {
                self.broken[c]
                    || self.regs[c].idle_limit(self.rxd[c]) == u64::MAX
                    || self.t - self.stable_since[c] >= STABLE_CYCLES
            })
    }

    fn snapshot(&mut self, chain: usize) -> [u32; 7] {
        let mut values = [0u32; 7];
        for (value, offset) in values.iter_mut().zip([
            CONTROL,
            ERRORS,
            COMMAND_STATUS,
            WORK_RX_STATUS,
            WORK_TX_STATUS,
            LAST_WORK_ID,
        ]) {
            *value = self.regs[chain].read(offset).unwrap_or(u32::MAX);
        }
        values[6] = u32::from(self.regs[chain].txd());
        values
    }

    fn track_stability(&mut self) {
        if self.t % 64 != 0 {
            return;
        }
        for chain in 0..CHAINS {
            let now = self.snapshot(chain);
            if now != self.last_snapshot[chain] {
                self.last_snapshot[chain] = now;
                self.stable_since[chain] = self.t;
            }
        }
    }

    pub fn read(&mut self, chain: usize, offset: u16, settled: bool) {
        self.seq.steps.push(Step::Read {
            chain: chain as u8,
            offset,
            settled,
        });
        self.tick(Some((chain, offset)), None);
        self.advance(STEP_COST - 1);
    }

    pub fn write(&mut self, chain: usize, offset: u16, data: u32, strobe: u8) {
        self.seq.steps.push(Step::Write {
            chain: chain as u8,
            offset,
            data,
            strobe,
        });
        self.tick(None, Some((chain, offset, data, strobe)));
        self.advance(STEP_COST - 1);
    }

    pub fn idle(&mut self, cycles: u32) {
        self.seq.steps.push(Step::Idle { cycles });
        self.advance(u64::from(cycles));
    }

    pub fn until(&mut self, cycle: u64) {
        let cycle = cycle.max(self.t);
        self.advance(cycle - self.t);
        self.seq.steps.push(Step::Until { cycle });
    }

    /// Run until every chain is quiet, then `guard` more cycles. Returns
    /// false if `cap` cycles pass first.
    pub fn until_quiet(&mut self, guard: u64, cap: u64) -> bool {
        let limit = self.t + cap;
        while !self.quiet() && self.t < limit {
            self.tick(None, None);
        }
        let quiet = self.quiet();
        self.until(self.t + guard);
        quiet
    }

    pub fn mark(&mut self, tag: u32) {
        self.seq.steps.push(Step::Mark { tag });
        self.advance(1);
    }

    pub fn control(&mut self, chain: usize) -> u32 {
        self.regs[chain].read(CONTROL).unwrap_or(0)
    }

    pub fn midstates(&self, chain: usize) -> Midstates {
        self.regs[chain].midstates()
    }

    pub fn broken(&self, chain: usize) -> bool {
        self.broken[chain]
    }

    /// Schedule 8N1 bytes on one input pin from `start`. Returns the cycle
    /// after the last stop bit.
    pub fn frame_on_pad(&mut self, pad: u16, period: u64, bytes: &[u8], start: u64) -> u64 {
        let mut level = true;
        let mut t = start.max(self.t + 1);
        for &byte in bytes {
            let bits = std::iter::once(false)
                .chain((0..8).map(|i| byte >> i & 1 != 0))
                .chain(std::iter::once(true));
            for bit in bits {
                if bit != level {
                    let event = PadEvent {
                        cycle: t,
                        pad,
                        level: bit,
                    };
                    self.seq.events.push(event);
                    let at = self.next
                        + self.pending[self.next..].partition_point(|e| e.cycle <= event.cycle);
                    self.pending.insert(at, event);
                    level = bit;
                }
                t += period;
            }
        }
        t
    }

    /// Serial bit length of a chain in circuit cycles.
    pub fn circuit_bit(&self, chain: usize) -> u64 {
        (bit_period(self.seq.divisors[chain]) / self.ratio).max(1)
    }

    /// Schedule bytes on a chain's mapped RX pin after any earlier frame.
    pub fn frame(&mut self, chain: usize, bytes: &[u8], delay: u64) -> Option<u64> {
        let pad = self.mapping.rx[chain]?;
        let period = self.circuit_bit(chain);
        let start = (self.t + delay).max(self.rx_free[chain]);
        let end = self.frame_on_pad(pad, period, bytes, start);
        self.rx_free[chain] = end + period;
        Some(end)
    }

    pub fn finish(mut self, tail: u32) -> Sequence {
        self.seq.tail = tail;
        self.seq.events.sort_by_key(|e| e.cycle);
        self.seq
    }
}

/// A 7-byte chip response. Valid frames carry the CRC the model accepts.
pub fn response(rng: &mut Rng, work: bool, valid: bool) -> [u8; 7] {
    let mut bytes = [0u8; 7];
    for b in &mut bytes[..6] {
        *b = rng.u32() as u8;
    }
    let top = if work { 0x80 } else { 0 } | (rng.u32() as u8 & 0x60);
    for crc in 0..32u8 {
        bytes[6] = top | crc;
        if response_words(&bytes, 0, Midstates::One).is_ok() {
            break;
        }
    }
    if !valid {
        bytes[6] ^= 1 + rng.below(31) as u8;
    }
    bytes
}

/// One command for the command FIFO. Length byte 5 or 9.
pub fn command(rng: &mut Rng, length: u32) -> Vec<u32> {
    let first = (rng.u32() & 0xffff_00ff) | (length << 8);
    if length == 9 {
        vec![first, rng.u32()]
    } else {
        vec![first]
    }
}

/// One job for the work FIFO, with the midstate index bits of the ID clear.
pub fn work(rng: &mut Rng, midstates: Midstates) -> Vec<u32> {
    let mut words: Vec<u32> = (0..midstates.work_words()).map(|_| rng.u32()).collect();
    words[0] = rng.u32() & 0xffff & !(midstates.count() as u32 - 1);
    words
}

fn configure_all(p: &mut Planner, divisor: u16) {
    p.seq.divisors = [divisor; CHAINS];
    for chain in 0..CHAINS {
        p.write(chain, BAUD, u32::from(divisor), 0xf);
        p.write(chain, WORK_TIME, 1, 0xf);
    }
    for chain in 0..CHAINS {
        p.write(chain, CONTROL, 8, 0xf);
    }
}

/// Each chain sends two commands in its own window, between marks c+1 and
/// c+2 (the last window ends at mark 9). The pin that toggles in a window
/// is that chain's TX pin.
pub fn tx_map(build_id: u32) -> Sequence {
    let mut rng = Rng::new(0x7478);
    let mut p = Planner::new(0, "tx-map", Mapping::default(), build_id, 1);
    configure_all(&mut p, 1);
    p.until_quiet(64, 20_000);
    for chain in 0..CHAINS {
        p.mark(chain as u32 + 1);
        for length in [5, 9] {
            for word in command(&mut rng, length) {
                p.write(chain, COMMAND_TX, word, 0xf);
            }
        }
        p.until_quiet(256, 40_000);
    }
    p.mark(9);
    p.finish(100)
}

/// Frames on one input pin. Marks 1 and 2 bracket the frames; the chain
/// whose status changes between them listens on that pin.
pub fn rx_map(pad: u16, build_id: u32) -> Sequence {
    let mut rng = Rng::new(0x7278 + u64::from(pad));
    let mut p = Planner::new(
        u64::from(pad),
        &format!("rx-map:{pad}"),
        Mapping::default(),
        build_id,
        1,
    );
    configure_all(&mut p, 1);
    p.until_quiet(64, 20_000);
    for chain in 0..CHAINS {
        for offset in [COMMAND_STATUS, WORK_RX_STATUS, ERRORS] {
            p.read(chain, offset, true);
        }
    }
    p.mark(1);
    let mut bytes = Vec::new();
    bytes.extend(response(&mut rng, false, true));
    bytes.extend(response(&mut rng, true, true));
    bytes.extend(response(&mut rng, false, false));
    let start = p.now() + 8;
    let end = p.frame_on_pad(pad, bit_period(1), &bytes, start);
    p.until(end + 400);
    p.mark(2);
    for chain in 0..CHAINS {
        for offset in [COMMAND_STATUS, WORK_RX_STATUS, ERRORS] {
            p.read(chain, offset, true);
        }
        for offset in [COMMAND_RX, COMMAND_RX, WORK_RX, WORK_RX] {
            p.read(chain, offset, true);
        }
    }
    p.finish(100)
}

/// Busy reads right after FIFO writes and around frame arrival. Used to
/// find the bus alignment; every read here depends on exact timing.
pub fn timing(mapping: &Mapping, build_id: u32, ratio: u64) -> Sequence {
    let mut rng = Rng::new(0x7469);
    let mut p = Planner::new(0, "timing", mapping.clone(), build_id, ratio);
    configure_all(&mut p, 1);
    p.until_quiet(64, 20_000);
    p.mark(1);
    for chain in 0..CHAINS {
        for word in command(&mut rng, 9) {
            p.write(chain, COMMAND_TX, word, 0xf);
        }
        for i in 0..12 {
            let offset = if i % 2 == 0 {
                COMMAND_STATUS
            } else {
                WORK_TX_STATUS
            };
            p.read(chain, offset, false);
        }
        if let Some(end) = p.frame(chain, &response(&mut rng, false, true), 4) {
            while p.now() < end + 400 {
                p.read(chain, COMMAND_STATUS, false);
                p.read(chain, ERRORS, false);
            }
        }
        p.until_quiet(64, 40_000);
        for offset in [COMMAND_RX, COMMAND_RX] {
            p.read(chain, offset, true);
        }
    }
    p.mark(2);
    p.finish(100)
}

fn configure_chain(p: &mut Planner, rng: &mut Rng, chain: usize, probe: bool) {
    let current = p.control(chain);
    let mut control = (rng.below(4) as u32) << 1;
    if rng.chance(85) {
        control |= 8;
    }
    if rng.chance(10) {
        control |= 1;
    }
    // Disabling with a midstate change after transmission is a case the
    // model does not implement. Keep the midstate bits unless probing.
    if control & 8 == 0 && current & 8 != 0 && !probe {
        control = (control & !6) | (current & 6);
    }
    if probe && rng.chance(50) {
        control |= 0x18;
    }
    let strobe = if rng.chance(90) {
        0xf
    } else {
        rng.below(16) as u8
    };
    p.write(chain, CONTROL, control, strobe);
    if rng.chance(40) {
        p.write(
            chain,
            WORK_TIME,
            rng.pick(&[1, 2, 3, 10, 50, 120, 300]),
            0xf,
        );
    }
    if rng.chance(30) {
        p.write(chain, WORK_TX_THRESHOLD, rng.below(64) as u32, 0xf);
    }
    for offset in [COMMAND_CONTROL, WORK_RX_CONTROL, WORK_TX_CONTROL] {
        if rng.chance(30) {
            let resets = if rng.chance(15) {
                rng.below(4) as u32
            } else {
                0
            };
            p.write(chain, offset, resets | (rng.below(2) as u32) << 2, 0xf);
        }
    }
}

/// A random sequence of about `budget` planned cycles.
pub fn random(seed: u64, mapping: &Mapping, build_id: u32, budget: u64, ratio: u64) -> Sequence {
    let mut rng = Rng::new(seed);
    let probe = rng.chance(2);
    let kind = if probe { "random-probe" } else { "random" };
    let mut p = Planner::new(seed, kind, mapping.clone(), build_id, ratio);
    for chain in 0..CHAINS {
        p.seq.divisors[chain] = rng.pick(&[0, 1, 1, 1, 2, 3, 7]);
    }
    for chain in 0..CHAINS {
        let divisor = p.seq.divisors[chain];
        p.write(chain, BAUD, u32::from(divisor), 0xf);
    }
    let mut responses = [[0u32; 2]; CHAINS];
    let mut phase = 0;
    while p.now() < budget && p.seq.steps.len() < 6000 {
        phase += 1;
        for chain in 0..CHAINS {
            if phase == 1 || rng.chance(40) {
                let probing = probe && rng.chance(30);
                configure_chain(&mut p, &mut rng, chain, probing);
            }
        }
        // Wire time left per chain in this phase. Work frames are long, so
        // slow serial settings get fewer and shorter transfers.
        let mut tx_left = [PHASE_WIRE_CYCLES; CHAINS];
        let mut rx_left = [PHASE_WIRE_CYCLES; CHAINS];
        for _ in 0..rng.range(1, 14) {
            let chain = rng.below(CHAINS as u64) as usize;
            let byte_cycles = 10 * p.circuit_bit(chain);
            match rng.below(100) {
                0..=34 => {
                    let length = if probe && rng.chance(5) {
                        7
                    } else if rng.chance(20) {
                        9
                    } else {
                        5
                    };
                    let cost = u64::from(length) * byte_cycles;
                    if cost > tx_left[chain] {
                        continue;
                    }
                    tx_left[chain] -= cost;
                    for word in command(&mut rng, length) {
                        p.write(chain, COMMAND_TX, word, 0xf);
                    }
                }
                35..=59 => {
                    let midstates = p.midstates(chain);
                    let cost = (22 + 32 * midstates.count() as u64) * byte_cycles;
                    if cost > tx_left[chain] {
                        continue;
                    }
                    tx_left[chain] -= cost;
                    let mut words = work(&mut rng, midstates);
                    if rng.chance(10) {
                        words.truncate(rng.range(1, words.len() as u64) as usize);
                    }
                    for word in words {
                        p.write(chain, WORK_TX, word, 0xf);
                    }
                }
                60..=84 => {
                    for _ in 0..rng.range(1, 3) {
                        let is_work = rng.chance(50);
                        let valid = rng.chance(85);
                        let mut bytes = response(&mut rng, is_work, valid).to_vec();
                        if rng.chance(5) {
                            bytes.truncate(rng.range(1, 6) as usize);
                        }
                        let cost = (bytes.len() as u64 + 1) * byte_cycles;
                        if cost > rx_left[chain] {
                            break;
                        }
                        rx_left[chain] -= cost;
                        let delay = rng.below(64);
                        if p.frame(chain, &bytes, delay).is_some() && valid && bytes.len() == 7 {
                            responses[chain][usize::from(is_work)] += 1;
                        }
                    }
                }
                85..=94 => {
                    let offset = rng.pick(&STATUS_REGISTERS);
                    p.read(chain, offset, false);
                }
                _ => p.idle(rng.below(400) as u32),
            }
        }
        let quiet = p.until_quiet(rng.range(16, 96), 120_000);
        p.mark(phase);
        for (chain, counts) in responses.iter_mut().enumerate() {
            for offset in STATUS_REGISTERS {
                if offset == VERSION && !rng.chance(10) {
                    continue;
                }
                p.read(chain, offset, quiet);
            }
            // Each response is two FIFO words. One extra read checks the
            // empty FIFO's held output.
            for (count, offset) in counts.iter_mut().zip([COMMAND_RX, WORK_RX]) {
                for _ in 0..2 * *count + rng.below(2) as u32 {
                    p.read(chain, offset, quiet);
                }
                *count = 0;
            }
        }
        if (0..CHAINS).all(|c| p.broken(c)) {
            break;
        }
    }
    p.finish(200)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapped() -> Mapping {
        Mapping {
            tx: [Some(0), Some(1), Some(2)],
            rx: [Some(0), Some(1), Some(2)],
        }
    }

    #[test]
    fn response_frames_pass_or_fail_the_model_crc_as_requested() {
        let mut rng = Rng::new(3);
        for i in 0..50 {
            let good = response(&mut rng, i % 2 == 0, true);
            assert!(response_words(&good, 0, Midstates::One).is_ok());
            let bad = response(&mut rng, i % 3 == 0, false);
            assert!(response_words(&bad, 0, Midstates::One).is_err());
        }
    }

    #[test]
    fn random_sequences_replay_from_their_seed() {
        let a = random(11, &mapped(), 0x5fca_47e9, 20_000, 2);
        let b = random(11, &mapped(), 0x5fca_47e9, 20_000, 2);
        assert_eq!(a, b);
        assert_ne!(a, random(12, &mapped(), 0x5fca_47e9, 20_000, 2));
    }

    #[test]
    fn random_sequences_have_sorted_events_and_settled_reads() {
        for seed in 0..20 {
            let s = random(seed, &mapped(), 0x5fca_47e9, 20_000, 1 + seed % 2);
            assert!(s.events.windows(2).all(|w| w[0].cycle <= w[1].cycle));
            assert!(s
                .steps
                .iter()
                .any(|step| matches!(step, Step::Read { settled: true, .. })));
            assert!(s.divisors.iter().all(|&d| d <= 7));
        }
    }

    #[test]
    fn calibration_sequences_have_the_expected_marks() {
        let tx = tx_map(0);
        let marks: Vec<u32> = tx
            .steps
            .iter()
            .filter_map(|s| match s {
                Step::Mark { tag } => Some(*tag),
                _ => None,
            })
            .collect();
        assert_eq!(marks, [1, 2, 3, 9]);
        let rx = rx_map(4, 0);
        assert!(rx.events.iter().all(|e| e.pad == 4));
        assert!(!rx.events.is_empty());
    }
}

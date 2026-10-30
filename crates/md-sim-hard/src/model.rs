//! Run the Rust S9IO model (reference contract 1.0.2, the version the S9
//! simulator uses) on the inputs the circuit received. Each bus strobe uses
//! the circuit's own start cycle plus a fixed alignment, so bus-delay
//! differences cannot accumulate over a sequence.
use crate::{
    circuit::CircuitTrace,
    sequence::{Sequence, Step, CHAINS},
};
use md_sim_firmware::s9io::registers::{BusError, Registers, Revision, COMMAND_RX, WORK_RX};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Half-width, in cycles, of the tolerance window for reads taken while
/// the model is busy.
pub const WINDOW: u64 = 32;

/// Cycle offsets from the circuit's bus start (or pin event) to the model
/// strobe, in circuit cycles. They absorb the AXI interconnect delay, which
/// the model lacks. `ratio` is model clock cycles per circuit cycle: the S9
/// simulator clocks the reference model at 200MHz, and calibration measures
/// which ratio matches the circuit's serial bit length.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alignment {
    pub read: i64,
    pub write: i64,
    pub rx: i64,
    pub ratio: u64,
}

impl Default for Alignment {
    fn default() -> Self {
        Self {
            read: 0,
            write: 0,
            rx: 0,
            ratio: 1,
        }
    }
}

/// Circuit pins that carry each chain's serial lines. Found by calibration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mapping {
    pub tx: [Option<u16>; CHAINS],
    pub rx: [Option<u16>; CHAINS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Known(u32),
    /// The model has no defined value, for example an unread FIFO output.
    Unknown,
}

impl Value {
    fn of(result: Result<u32, BusError>) -> Self {
        result.map_or(Value::Unknown, Value::Known)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelRead {
    pub cycle: u64,
    pub value: Value,
    /// No serial activity, pending state change or pin edge near the strobe.
    pub quiet: bool,
    /// Values the register had within `WINDOW` cycles, for busy reads of
    /// non-FIFO registers only.
    pub window: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelWrite {
    Done,
    Rejected(String),
}

/// Model results. All cycle numbers are circuit cycles.
#[derive(Clone, Debug, Default)]
pub struct ModelTrace {
    pub reads: BTreeMap<usize, ModelRead>,
    pub writes: BTreeMap<usize, ModelWrite>,
    /// TX level changes per chain.
    pub txd: [Vec<(u64, bool)>; CHAINS],
    /// First case the model does not implement, per chain. Later steps on
    /// that chain are not compared.
    pub unsupported: [Option<(u64, String)>; CHAINS],
    pub end: u64,
    /// Model clock cycles per circuit cycle used for this run.
    pub ratio: u64,
}

impl ModelTrace {
    pub fn txd_at(&self, chain: usize, cycle: u64) -> Option<bool> {
        self.txd[chain]
            .iter()
            .rev()
            .find(|(c, _)| *c <= cycle)
            .map(|&(_, level)| level)
    }
}

pub fn is_fifo(offset: u16) -> bool {
    matches!(offset & 0x3ffc, COMMAND_RX | WORK_RX)
}

#[derive(Clone, Copy)]
enum Kind {
    Read { settled: bool },
    Write { data: u32, strobe: u8 },
}

#[derive(Clone, Copy)]
struct Op {
    cycle: u64,
    step: usize,
    chain: usize,
    offset: u16,
    kind: Kind,
}

struct Window {
    start: u64,
    end: u64,
    chain: usize,
    offset: u16,
    step: usize,
}

/// Circuit cycle plus offset, converted to a model clock cycle.
fn shift(cycle: u64, by: i64, ratio: u64) -> u64 {
    (cycle as i64 + by).max(0) as u64 * ratio
}

fn bus_ops(seq: &Sequence, circuit: &CircuitTrace, align: Alignment) -> Vec<Op> {
    let ratio = align.ratio.max(1);
    let mut ops = Vec::new();
    for (step, s) in seq.steps.iter().enumerate() {
        let (obs, kind, chain, offset) = match *s {
            Step::Read {
                chain,
                offset,
                settled,
            } => (
                circuit
                    .reads
                    .get(&step)
                    .map(|o| shift(o.start, align.read, ratio)),
                Kind::Read { settled },
                chain,
                offset,
            ),
            Step::Write {
                chain,
                offset,
                data,
                strobe,
            } => (
                circuit
                    .writes
                    .get(&step)
                    .map(|o| shift(o.start, align.write, ratio)),
                Kind::Write { data, strobe },
                chain,
                offset,
            ),
            _ => continue,
        };
        // A step the circuit never completed is not run on the model.
        let Some(cycle) = obs else { continue };
        ops.push(Op {
            cycle,
            step,
            chain: usize::from(chain),
            offset,
            kind,
        });
    }
    ops.sort_by_key(|o| (o.cycle, o.step));
    // The circuit runs one bus step at a time, so two strobes on one chain
    // cannot share a cycle. Keep the order if an alignment would merge them.
    for i in 1..ops.len() {
        let previous = ops[..i]
            .iter()
            .rev()
            .find(|o| o.chain == ops[i].chain)
            .map(|o| o.cycle);
        if let Some(p) = previous {
            if ops[i].cycle <= p {
                ops[i].cycle = p + 1;
            }
        }
    }
    ops.sort_by_key(|o| (o.cycle, o.step));
    ops
}

fn rx_changes(seq: &Sequence, mapping: &Mapping, align: Alignment) -> [Vec<(u64, bool)>; CHAINS] {
    std::array::from_fn(|chain| {
        let Some(pad) = mapping.rx[chain] else {
            return Vec::new();
        };
        let mut changes: Vec<(u64, bool)> = seq
            .events
            .iter()
            .filter(|e| e.pad == pad)
            .map(|e| (shift(e.cycle, align.rx, align.ratio.max(1)), e.level))
            .collect();
        changes.sort_by_key(|c| c.0);
        changes
    })
}

fn edge_near(changes: &[(u64, bool)], cycle: u64, window: u64) -> bool {
    let low = cycle.saturating_sub(window);
    let first = changes.partition_point(|c| c.0 < low);
    changes.get(first).is_some_and(|c| c.0 <= cycle + window)
}

pub fn run(
    seq: &Sequence,
    circuit: &CircuitTrace,
    mapping: &Mapping,
    align: Alignment,
    build_id: u32,
) -> ModelTrace {
    // Inside this function, cycle numbers are model clock cycles. Results
    // are converted back to circuit cycles.
    let ratio = align.ratio.max(1);
    let window = WINDOW * ratio;
    let ops = bus_ops(seq, circuit, align);
    let rx = rx_changes(seq, mapping, align);
    let mut windows: Vec<Window> = ops
        .iter()
        .filter_map(|op| match op.kind {
            Kind::Read { settled: false } if !is_fifo(op.offset) => Some(Window {
                start: op.cycle.saturating_sub(window),
                end: op.cycle + window,
                chain: op.chain,
                offset: op.offset,
                step: op.step,
            }),
            _ => None,
        })
        .collect();
    windows.sort_by_key(|w| w.start);

    let end = (circuit.last_cycle() + 64) * ratio;
    let end = end.max(ops.last().map_or(0, |o| o.cycle + ratio));
    let mut regs: Vec<Registers> = (0..CHAINS)
        .map(|_| Registers::with_revision(build_id, Revision::Reference1_0_2))
        .collect();
    let mut trace = ModelTrace {
        end: end / ratio,
        ratio,
        ..ModelTrace::default()
    };
    for (chain, r) in regs.iter().enumerate() {
        trace.txd[chain].push((0, r.txd()));
    }
    let mut rx_next = [0usize; CHAINS];
    let mut rxd = [true; CHAINS];
    let mut next_op = 0;
    let mut next_window = 0;
    let mut active: Vec<usize> = Vec::new();
    let mut samples: BTreeMap<usize, Vec<Value>> = BTreeMap::new();

    for t in 0..=end {
        for chain in 0..CHAINS {
            while rx_next[chain] < rx[chain].len() && rx[chain][rx_next[chain]].0 <= t {
                rxd[chain] = rx[chain][rx_next[chain]].1;
                rx_next[chain] += 1;
            }
        }
        while next_window < windows.len() && windows[next_window].start <= t {
            active.push(next_window);
            next_window += 1;
        }
        active.retain(|&w| windows[w].end >= t);
        for &w in &active {
            let window = &windows[w];
            if trace.unsupported[window.chain].is_some() {
                continue;
            }
            // Non-FIFO reads do not change state. Sample before the edge,
            // as a real read strobe at this cycle would.
            let value = Value::of(regs[window.chain].read(window.offset));
            let list = samples.entry(window.step).or_default();
            if !list.contains(&value) {
                list.push(value);
            }
        }
        let mut strobe: [Option<Op>; CHAINS] = [None; CHAINS];
        while next_op < ops.len() && ops[next_op].cycle <= t {
            strobe[ops[next_op].chain] = Some(ops[next_op]);
            next_op += 1;
        }
        for chain in 0..CHAINS {
            if trace.unsupported[chain].is_some() {
                continue;
            }
            let op = strobe[chain];
            let (read, write) = match op.map(|o| o.kind) {
                Some(Kind::Read { .. }) => (op.map(|o| o.offset), None),
                Some(Kind::Write { data, strobe }) => (None, op.map(|o| (o.offset, data, strobe))),
                None => (None, None),
            };
            let quiet = read.is_some()
                && regs[chain].idle_limit(rxd[chain]) >= 4 * window
                && !edge_near(&rx[chain], t, window);
            match regs[chain].cycle(rxd[chain], read, write) {
                Ok(cycle) => {
                    if let Some(op) = op {
                        match op.kind {
                            Kind::Read { .. } => {
                                let value = match cycle.read {
                                    Some(result) => Value::of(result),
                                    None => Value::Unknown,
                                };
                                trace.reads.insert(
                                    op.step,
                                    ModelRead {
                                        cycle: t / ratio,
                                        value,
                                        quiet,
                                        window: Vec::new(),
                                    },
                                );
                            }
                            Kind::Write { .. } => match cycle.write {
                                Some(Err(error)) => {
                                    trace.writes.insert(
                                        op.step,
                                        ModelWrite::Rejected(format!("{error:?}")),
                                    );
                                    trace.unsupported[chain] = Some((
                                        t / ratio,
                                        format!("write {:#06x}: {error:?}", op.offset),
                                    ));
                                }
                                _ => {
                                    trace.writes.insert(op.step, ModelWrite::Done);
                                }
                            },
                        }
                    }
                }
                Err(error) => {
                    trace.unsupported[chain] = Some((t / ratio, format!("{error:?}")));
                }
            }
            let level = regs[chain].txd();
            if trace.txd[chain].last().map(|x| x.1) != Some(level) {
                trace.txd[chain].push((t / ratio, level));
            }
        }
    }
    for (step, values) in samples {
        if let Some(read) = trace.reads.get_mut(&step) {
            read.window = values;
        }
    }
    trace
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::BusObs;
    use md_sim_firmware::s9io::registers::{BAUD, CONTROL, VERSION};

    fn obs(start: u64) -> BusObs {
        BusObs {
            start,
            done: start + 10,
            data: None,
            resp: Some(0),
        }
    }

    #[test]
    fn model_reads_back_written_registers_at_circuit_cycles() {
        let mut seq = Sequence::new(1, "unit");
        seq.steps = vec![
            Step::Write {
                chain: 1,
                offset: BAUD,
                data: 0x123,
                strobe: 0xf,
            },
            Step::Read {
                chain: 1,
                offset: BAUD,
                settled: true,
            },
            Step::Read {
                chain: 0,
                offset: VERSION,
                settled: true,
            },
        ];
        let mut circuit = CircuitTrace::default();
        circuit.writes.insert(0, obs(10));
        circuit.reads.insert(1, obs(30));
        circuit.reads.insert(2, obs(50));
        circuit.end = Some(80);
        let trace = run(
            &seq,
            &circuit,
            &Mapping::default(),
            Alignment::default(),
            0x5fca_47e9,
        );
        assert_eq!(trace.writes[&0], ModelWrite::Done);
        assert_eq!(trace.reads[&1].value, Value::Known(0x123));
        assert_eq!(trace.reads[&1].cycle, 30);
        assert!(trace.reads[&1].quiet);
        assert_eq!(trace.reads[&2].value, Value::Known(0x0090_1002));
    }

    #[test]
    fn disabled_reference_chain_holds_tx_low_until_enabled() {
        let mut seq = Sequence::new(1, "unit");
        seq.steps = vec![Step::Write {
            chain: 2,
            offset: CONTROL,
            data: 8,
            strobe: 0xf,
        }];
        let mut circuit = CircuitTrace::default();
        circuit.writes.insert(0, obs(100));
        circuit.end = Some(200);
        let trace = run(&seq, &circuit, &Mapping::default(), Alignment::default(), 0);
        assert_eq!(trace.txd_at(2, 50), Some(false));
        assert_eq!(trace.txd_at(2, 199), Some(true));
        assert_eq!(trace.txd_at(0, 199), Some(false));
    }

    #[test]
    fn steps_missing_from_the_circuit_trace_are_not_run() {
        let mut seq = Sequence::new(1, "unit");
        seq.steps = vec![Step::Read {
            chain: 0,
            offset: VERSION,
            settled: true,
        }];
        let trace = run(
            &seq,
            &CircuitTrace::default(),
            &Mapping::default(),
            Alignment::default(),
            0,
        );
        assert!(trace.reads.is_empty());
    }
}

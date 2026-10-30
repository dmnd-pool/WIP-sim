//! Calibration from the circuit itself: build ID, serial pin map and the
//! bus alignment between the circuit's AXI port and the model's register
//! strobe. Pin names in the netlist are physical sites, so the map comes
//! from behavior, not from names.
use crate::{
    circuit::CircuitTrace,
    compare::{self, Finding},
    generate,
    model::{self, Alignment, Mapping, Value},
    run::{self, Prepared},
    sequence::{Sequence, Step, CHAINS},
};
use anyhow::{Context, Result};
use md_sim_firmware::s9io::registers::{BUILD_ID, COMMAND_STATUS, ERRORS, VERSION, WORK_RX_STATUS};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
    thread,
    time::Duration,
};

pub const CALIBRATION_SCHEMA: &str = "md-sim-hard-s9-fabric-calibration/1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Calibration {
    pub schema: String,
    pub build_id: u32,
    pub version: u32,
    pub mapping: Mapping,
    pub alignment: Alignment,
    /// Level changes per output pin inside each chain's TX window.
    pub tx_window_transitions: Vec<Vec<u32>>,
    /// Chains whose receive status changed, per input pin.
    pub rx_reactions: BTreeMap<u16, Vec<u8>>,
    pub alignment_scores: Vec<(i64, u64)>,
    pub rx_scores: Vec<(i64, u64)>,
    pub read_scores: Vec<(i64, u64)>,
    /// Findings on the calibration sequences at the chosen alignment.
    pub findings: Vec<Finding>,
    pub notes: Vec<String>,
}

fn identity() -> Sequence {
    let mut s = Sequence::new(0, "identity");
    for chain in 0..CHAINS as u8 {
        for offset in [VERSION, BUILD_ID] {
            s.steps.push(Step::Read {
                chain,
                offset,
                settled: true,
            });
        }
    }
    s.tail = 50;
    s
}

fn run_all(
    prepared: &Prepared,
    dir: &Path,
    sequences: Vec<Sequence>,
    jobs: usize,
) -> Result<Vec<(Sequence, CircuitTrace)>> {
    let next = AtomicUsize::new(0);
    let results = Mutex::new(Vec::new());
    thread::scope(|scope| {
        for _ in 0..jobs.max(1).min(sequences.len()) {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(seq) = sequences.get(i) else { break };
                let case = dir.join(seq.kind.replace(':', "-"));
                let outcome = run::simulate(prepared, &case, seq, Duration::from_secs(3600));
                results.lock().unwrap().push((i, outcome));
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|r| r.0);
    results
        .into_iter()
        .map(|(i, outcome)| {
            let kind = &sequences[i].kind;
            outcome
                .map(|(trace, _)| (sequences[i].clone(), trace))
                .with_context(|| format!("calibration sequence {kind}"))
        })
        .collect()
}

fn read_value(seq: &Sequence, trace: &CircuitTrace, chain: u8, offset: u16) -> Option<u32> {
    seq.steps.iter().enumerate().find_map(|(i, s)| match *s {
        Step::Read {
            chain: c,
            offset: o,
            ..
        } if c == chain && o == offset => trace.reads.get(&i).and_then(|r| r.data),
        _ => None,
    })
}

fn mark_cycle(trace: &CircuitTrace, tag: u32) -> Option<u64> {
    trace.marks.iter().find(|m| m.0 == tag).map(|m| m.1)
}

/// The pin with the most level changes in each chain's window, if it has
/// at least 8 and no other chain claims it.
pub fn tx_mapping(trace: &CircuitTrace, outputs: usize) -> (Vec<Vec<u32>>, [Option<u16>; CHAINS]) {
    let mut table = Vec::new();
    let mut map = [None; CHAINS];
    for chain in 0..CHAINS {
        let start = mark_cycle(trace, chain as u32 + 1);
        let end = mark_cycle(trace, chain as u32 + 2).or_else(|| mark_cycle(trace, 9));
        let mut counts = vec![0u32; outputs];
        if let (Some(start), Some(end)) = (start, end) {
            for (pad, changes) in &trace.outputs {
                if let Some(count) = counts.get_mut(usize::from(*pad)) {
                    *count = changes
                        .iter()
                        .filter(|(c, _)| *c >= start && *c < end)
                        .count() as u32;
                }
            }
        }
        let best = (0..outputs).max_by_key(|&p| (counts[p], std::cmp::Reverse(p)));
        if let Some(pad) = best.filter(|&p| counts[p] >= 8) {
            if !map.contains(&Some(pad as u16)) {
                map[chain] = Some(pad as u16);
            }
        }
        table.push(counts);
    }
    (table, map)
}

/// Shortest interval between TX level changes after mark 1, over the
/// mapped pins. In the TX-map sequence this is one serial bit.
pub fn measured_bit(trace: &CircuitTrace, tx: &[Option<u16>; CHAINS]) -> Option<u64> {
    let start = mark_cycle(trace, 1)?;
    tx.iter()
        .flatten()
        .filter_map(|pad| {
            let cycles: Vec<u64> = trace
                .outputs
                .get(pad)?
                .iter()
                .map(|c| c.0)
                .filter(|&c| c >= start)
                .collect();
            cycles.windows(2).map(|w| w[1] - w[0]).min()
        })
        .min()
}

/// Chains whose receive status or error count changed across the frames.
pub fn rx_reaction(seq: &Sequence, trace: &CircuitTrace) -> Vec<u8> {
    let position = |tag| seq.steps.iter().position(|s| *s == Step::Mark { tag });
    let (Some(first), Some(second)) = (position(1), position(2)) else {
        return Vec::new();
    };
    let mut before = BTreeMap::new();
    let mut after = BTreeMap::new();
    for (i, s) in seq.steps.iter().enumerate() {
        let Step::Read { chain, offset, .. } = *s else {
            continue;
        };
        if ![COMMAND_STATUS, WORK_RX_STATUS, ERRORS].contains(&offset) {
            continue;
        }
        let value = trace.reads.get(&i).map(|r| r.data);
        if i < first {
            before.insert((chain, offset), value);
        } else if i > second {
            after.entry((chain, offset)).or_insert(value);
        }
    }
    let mut chains: Vec<u8> = after
        .iter()
        .filter(|(key, value)| before.get(key) != Some(value))
        .map(|((chain, _), _)| *chain)
        .collect();
    chains.dedup();
    chains
}

/// Exact read mismatches over all reads, busy or not.
fn strict_mismatches(
    seqs: &[(Sequence, CircuitTrace)],
    mapping: &Mapping,
    align: Alignment,
    build_id: u32,
) -> u64 {
    let mut total = 0;
    for (seq, trace) in seqs {
        let m = model::run(seq, trace, mapping, align, build_id);
        for (step, read) in &m.reads {
            let circuit = trace.reads.get(step).and_then(|r| r.data);
            total += u64::from(match (circuit, read.value) {
                (Some(c), Value::Known(v)) => c != v,
                (None, Value::Known(_)) => true,
                _ => false,
            });
        }
    }
    total
}

fn by_magnitude(center: i64, radius: i64) -> Vec<i64> {
    let mut values: Vec<i64> = (center - radius..=center + radius).collect();
    values.sort_by_key(|v| ((v - center).abs(), *v));
    values
}

fn search(candidates: Vec<i64>, mut score: impl FnMut(i64) -> u64) -> (i64, Vec<(i64, u64)>) {
    let mut table: Vec<(i64, u64)> = candidates.iter().map(|&v| (v, score(v))).collect();
    let best = table.iter().min_by_key(|e| e.1).map_or(0, |e| e.0);
    table.sort_by_key(|e| e.0);
    (best, table)
}

pub fn calibrate(run_dir: &Path, jobs: usize) -> Result<Calibration> {
    let prepared: Prepared = run::read_json(&run_dir.join("prepare.json"))?;
    let dir = run_dir.join("calibration");
    let mut notes = Vec::new();

    let mut first = vec![identity(), generate::tx_map(0)];
    for pad in 0..prepared.pads.inputs.len() as u16 {
        first.push(generate::rx_map(pad, 0));
    }
    let results = run_all(&prepared, &dir, first, jobs)?;
    let (id_seq, id_trace) = &results[0];
    let build_id = read_value(id_seq, id_trace, 0, BUILD_ID).context("circuit BUILD_ID read")?;
    let version = read_value(id_seq, id_trace, 0, VERSION).context("circuit VERSION read")?;
    for chain in 1..CHAINS as u8 {
        if read_value(id_seq, id_trace, chain, BUILD_ID) != Some(build_id) {
            notes.push(format!("chain {chain} BUILD_ID differs from chain 0"));
        }
    }

    let (tx_window_transitions, tx) = tx_mapping(&results[1].1, prepared.pads.outputs.len());
    let mut rx_reactions = BTreeMap::new();
    for (seq, trace) in &results[2..] {
        let pad: u16 = seq.kind.trim_start_matches("rx-map:").parse()?;
        rx_reactions.insert(pad, rx_reaction(seq, trace));
    }
    let mut rx = [None; CHAINS];
    for (chain, slot) in rx.iter_mut().enumerate() {
        let pads: Vec<u16> = rx_reactions
            .iter()
            .filter(|(_, chains)| chains.as_slice() == [chain as u8])
            .map(|(pad, _)| *pad)
            .collect();
        match pads.as_slice() {
            [pad] => *slot = Some(*pad),
            [] => notes.push(format!("no input pin changes chain {chain} receive status")),
            _ => notes.push(format!("several input pins change chain {chain}: {pads:?}")),
        }
    }
    for (chain, pad) in tx.iter().enumerate() {
        if pad.is_none() {
            notes.push(format!("no TX pin found for chain {chain}"));
        }
    }
    let mapping = Mapping { tx, rx };

    // Shortest TX level interval at divisor 1 is one circuit bit.
    let circuit_bit = measured_bit(&results[1].1, &mapping.tx);
    let model_bit = generate::bit_period(1);
    let ratio = match circuit_bit {
        Some(bit) if bit > 0 && model_bit % bit == 0 && model_bit / bit <= 4 => model_bit / bit,
        other => {
            notes.push(format!(
                "circuit TX bit {other:?} cycles does not divide the model bit {model_bit}; ratio 1"
            ));
            1
        }
    };
    notes.push(format!(
        "serial bit at divisor 1: circuit {circuit_bit:?} fabric cycles, model {model_bit} model cycles; comparing with {ratio} model cycles per circuit cycle"
    ));

    let timing = run_all(
        &prepared,
        &dir,
        vec![generate::timing(&mapping, build_id, ratio)],
        1,
    )?;
    // The pin-map sequences use a guessed bit length, so they only map
    // pins. Alignment uses sequences generated at the measured ratio.
    let mut tuning: Vec<(Sequence, CircuitTrace)> = vec![results[1].clone()];
    tuning.extend(timing);
    let align = |read, write, rx| Alignment {
        read,
        write,
        rx,
        ratio,
    };
    let (d, alignment_scores) = search(by_magnitude(0, 24), |d| {
        strict_mismatches(&tuning, &mapping, align(d, d, 0), build_id)
    });
    let (r, rx_scores) = search(by_magnitude(0, 12), |r| {
        strict_mismatches(&tuning, &mapping, align(d, d, r), build_id)
    });
    let (read, read_scores) = search(by_magnitude(d, 8), |read| {
        strict_mismatches(&tuning, &mapping, align(read, d, r), build_id)
    });
    let alignment = align(read, d, r);

    let mut findings = Vec::new();
    let mut checked: Vec<&(Sequence, CircuitTrace)> = vec![&results[0]];
    checked.extend(tuning.iter());
    for (seq, trace) in checked {
        let m = model::run(seq, trace, &mapping, alignment, build_id);
        for mut f in compare::compare(seq, trace, &m, &mapping).findings {
            f.detail = format!("[{}] {}", seq.kind, f.detail);
            findings.push(f);
        }
    }
    let calibration = Calibration {
        schema: CALIBRATION_SCHEMA.into(),
        build_id,
        version,
        mapping,
        alignment,
        tx_window_transitions,
        rx_reactions,
        alignment_scores,
        rx_scores,
        read_scores,
        findings,
        notes,
    };
    run::write_json(&run_dir.join("calibration.json"), &calibration)?;
    Ok(calibration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::{BusObs, Level};

    #[test]
    fn tx_mapping_picks_the_busy_pin_in_each_window() {
        let mut trace = CircuitTrace {
            marks: vec![(1, 100), (2, 1000), (3, 2000), (9, 3000)],
            ..CircuitTrace::default()
        };
        let busy = |from: u64| {
            (0..20)
                .map(|i| {
                    (
                        from + i * 10,
                        if i % 2 == 0 { Level::Low } else { Level::High },
                    )
                })
                .collect::<Vec<_>>()
        };
        trace.outputs.insert(4, busy(200));
        trace.outputs.insert(7, busy(1200));
        trace.outputs.insert(1, busy(2200));
        trace.outputs.insert(0, vec![(0, Level::High)]);
        let (_, map) = tx_mapping(&trace, 8);
        assert_eq!(map, [Some(4), Some(7), Some(1)]);
    }

    #[test]
    fn rx_reaction_reports_the_changed_chain() {
        let seq = generate::rx_map(3, 0);
        let mut trace = CircuitTrace::default();
        let first = seq
            .steps
            .iter()
            .position(|s| *s == Step::Mark { tag: 1 })
            .unwrap();
        for (i, s) in seq.steps.iter().enumerate() {
            if let Step::Read { chain, offset, .. } = *s {
                let changed = i > first && chain == 2 && offset == COMMAND_STATUS;
                trace.reads.insert(
                    i,
                    BusObs {
                        start: 0,
                        done: 0,
                        data: Some(if changed { 0x10 } else { 0 }),
                        resp: Some(0),
                    },
                );
            }
        }
        assert_eq!(rx_reaction(&seq, &trace), [2]);
    }

    #[test]
    fn search_prefers_the_smallest_offset_on_ties() {
        let (best, table) = search(by_magnitude(0, 3), |v| if v.abs() <= 1 { 0 } else { 5 });
        assert_eq!(best, 0);
        assert_eq!(table.len(), 7);
    }
}

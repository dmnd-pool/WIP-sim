//! Compare one circuit trace with one model trace.
//!
//! Classes, strongest first:
//! - `read-mismatch`: a quiet read or FIFO read returned different data.
//! - `read-mismatch-busy`: a busy read matched no model value nearby.
//! - `circuit-unknown`: the circuit returned x where the model has a value.
//! - `bus-response`: the circuit answered SLVERR or DECERR.
//! - `tx-bytes`, `tx-level`, `tx-unknown`: serial output differs.
//! - `circuit-timeout`: a bus handshake never completed.
use crate::{
    circuit::{CircuitTrace, Level},
    model::{is_fifo, Mapping, ModelTrace, ModelWrite, Value},
    sequence::{Sequence, Step, CHAINS},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub class: String,
    pub chain: Option<u8>,
    pub step: Option<usize>,
    /// Register offset for bus findings.
    pub offset: Option<u16>,
    pub cycle: u64,
    pub detail: String,
}

impl Finding {
    /// Grouping key for summaries, for example `read-mismatch 0x300c`.
    pub fn key(&self) -> String {
        match self.offset {
            Some(offset) => format!("{} {offset:#06x}", self.class),
            None => self.class.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub exact_reads: u64,
    pub window_reads: u64,
    /// Busy reads that matched only a nearby model value.
    pub timing_only: u64,
    pub writes: u64,
    pub tx_frames: u64,
    /// Model had no value (unread FIFO); the circuit showed a defined one.
    pub unknown_resolved: u64,
    /// Steps skipped after the model reported an unsupported case.
    pub skipped_unsupported: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub counts: Counts,
    /// Circuit TX frame start minus model TX frame start, per matched frame.
    pub tx_offsets: Vec<i64>,
    pub unsupported: Vec<String>,
}

impl Report {
    fn add(
        &mut self,
        class: &str,
        chain: Option<u8>,
        step: Option<usize>,
        cycle: u64,
        detail: String,
    ) {
        let offset = detail
            .split_whitespace()
            .nth(1)
            .and_then(|t| t.trim_end_matches(':').strip_prefix("0x"))
            .and_then(|t| u16::from_str_radix(t, 16).ok())
            .filter(|_| detail.starts_with("read ") || detail.starts_with("write "));
        self.findings.push(Finding {
            class: class.into(),
            chain,
            step,
            offset,
            cycle,
            detail,
        });
    }
}

/// One decoded 8N1 frame. `byte` is `None` for a framing error or break.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub start: u64,
    pub byte: Option<u8>,
}

/// Decode 8N1 frames from level changes. `period` is cycles per bit.
pub fn decode(changes: &[(u64, Level)], period: u64, end: u64) -> Vec<Frame> {
    let level_at = |cycle: u64| -> Level {
        let i = changes.partition_point(|c| c.0 <= cycle);
        if i == 0 {
            Level::Unknown
        } else {
            changes[i - 1].1
        }
    };
    let mut frames = Vec::new();
    // Candidate start: a change to Low whose previous level was High.
    let mut cursor = 0;
    let mut i = 1;
    while i < changes.len() {
        let (cycle, level) = changes[i];
        if level != Level::Low || changes[i - 1].1 != Level::High || cycle < cursor {
            i += 1;
            continue;
        }
        let sample = |bit: u64| level_at(cycle + period / 2 + bit * period);
        if cycle + period / 2 + 9 * period > end {
            break;
        }
        if sample(0) != Level::Low {
            i += 1;
            continue;
        }
        let mut byte = 0u8;
        let mut known = true;
        for bit in 0..8 {
            match sample(1 + bit) {
                Level::High => byte |= 1 << bit,
                Level::Low => {}
                Level::Unknown => known = false,
            }
        }
        let stop = sample(9) == Level::High;
        frames.push(Frame {
            start: cycle,
            byte: (known && stop).then_some(byte),
        });
        cursor = cycle + period / 2 + 9 * period;
        if !stop {
            // A break or framing error: resume at the next rise.
            while i < changes.len() && (changes[i].0 < cursor || changes[i].1 != Level::High) {
                i += 1;
            }
            if i < changes.len() {
                cursor = changes[i].0;
            }
        }
        i += 1;
    }
    frames
}

fn model_levels(changes: &[(u64, bool)]) -> Vec<(u64, Level)> {
    changes
        .iter()
        .map(|&(c, l)| (c, if l { Level::High } else { Level::Low }))
        .collect()
}

fn show(frames: &[Frame]) -> String {
    frames
        .iter()
        .take(12)
        .map(|f| f.byte.map_or("--".into(), |b| format!("{b:02x}")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn compare_reads(seq: &Sequence, circuit: &CircuitTrace, model: &ModelTrace, report: &mut Report) {
    for (step, s) in seq.steps.iter().enumerate() {
        let Step::Read {
            chain,
            offset,
            settled,
        } = *s
        else {
            continue;
        };
        let Some(obs) = circuit.reads.get(&step) else {
            continue;
        };
        if obs.resp != Some(0) {
            report.add(
                "bus-response",
                Some(chain),
                Some(step),
                obs.start,
                format!("read {offset:#06x}: response {:?}", obs.resp),
            );
        }
        let Some(read) = model.reads.get(&step) else {
            report.counts.skipped_unsupported += 1;
            continue;
        };
        let exact = (settled && read.quiet) || is_fifo(offset);
        match (obs.data, read.value) {
            (Some(c), Value::Known(m)) if c == m => {
                if exact {
                    report.counts.exact_reads += 1;
                } else {
                    report.counts.window_reads += 1;
                }
            }
            (None, Value::Unknown) => report.counts.exact_reads += 1,
            (Some(_), Value::Unknown) => report.counts.unknown_resolved += 1,
            (None, Value::Known(m)) => report.add(
                "circuit-unknown",
                Some(chain),
                Some(step),
                obs.start,
                format!("read {offset:#06x}: circuit x, model {m:#010x}"),
            ),
            (Some(c), Value::Known(m)) => {
                if !exact && read.window.contains(&Value::Known(c)) {
                    report.counts.timing_only += 1;
                } else {
                    report.add(
                        if exact {
                            "read-mismatch"
                        } else {
                            "read-mismatch-busy"
                        },
                        Some(chain),
                        Some(step),
                        obs.start,
                        format!(
                            "read {offset:#06x}: circuit {c:#010x}, model {m:#010x}, quiet={}",
                            read.quiet
                        ),
                    );
                }
            }
        }
    }
}

fn compare_writes(seq: &Sequence, circuit: &CircuitTrace, model: &ModelTrace, report: &mut Report) {
    for (step, s) in seq.steps.iter().enumerate() {
        let Step::Write { chain, offset, .. } = *s else {
            continue;
        };
        let Some(obs) = circuit.writes.get(&step) else {
            continue;
        };
        report.counts.writes += 1;
        if obs.resp != Some(0) {
            report.add(
                "bus-response",
                Some(chain),
                Some(step),
                obs.start,
                format!("write {offset:#06x}: response {:?}", obs.resp),
            );
        }
        if let Some(ModelWrite::Rejected(why)) = model.writes.get(&step) {
            report
                .unsupported
                .push(format!("chain {chain} step {step}: {why}"));
        }
    }
}

fn compare_tx(
    seq: &Sequence,
    circuit: &CircuitTrace,
    model: &ModelTrace,
    mapping: &Mapping,
    report: &mut Report,
) {
    for chain in 0..CHAINS {
        let Some(pad) = mapping.tx[chain] else {
            continue;
        };
        let changes = circuit.outputs.get(&pad).cloned().unwrap_or_default();
        // The circuit drives defined levels after reset. Unknown later on
        // is a finding by itself.
        if let Some(&(cycle, _)) = changes
            .iter()
            .find(|(c, l)| *c > 200 && *l == Level::Unknown)
        {
            report.add(
                "tx-unknown",
                Some(chain as u8),
                None,
                cycle,
                format!("TX pin {pad} became unknown"),
            );
            continue;
        }
        // Stop at the first unsupported model case on this chain.
        let limit = model.unsupported[chain].as_ref().map_or(u64::MAX, |u| u.0);
        let end = circuit.last_cycle().min(model.end).min(limit);
        // Model bit length in circuit cycles, at the calibrated clock ratio.
        let period = 16 * (u64::from(seq.divisors[chain]) + 1) / model.ratio.max(1);
        let ours = decode(&changes, period, end);
        let theirs = decode(&model_levels(&model.txd[chain]), period, end);
        let pairs = ours.len().min(theirs.len());
        report.counts.tx_frames += pairs as u64;
        for i in 0..pairs {
            report
                .tx_offsets
                .push(ours[i].start as i64 - theirs[i].start as i64);
        }
        let first_diff = (0..pairs).find(|&i| ours[i].byte != theirs[i].byte);
        if first_diff.is_some() || ours.len() != theirs.len() {
            let i = first_diff.unwrap_or(pairs);
            let from = i.saturating_sub(2);
            report.add(
                "tx-bytes",
                Some(chain as u8),
                None,
                ours.get(i).or(theirs.get(i)).map_or(0, |f| f.start),
                format!(
                    "frame {i}: circuit [{}] ({} frames), model [{}] ({} frames)",
                    show(&ours[from.min(ours.len())..]),
                    ours.len(),
                    show(&theirs[from.min(theirs.len())..]),
                    theirs.len()
                ),
            );
        }
        // Idle level at marks. Marks sit at quiet points of the sequence.
        for &(tag, cycle) in &circuit.marks {
            if cycle >= limit {
                break;
            }
            let ours = circuit.level_at(pad, cycle);
            let theirs = model
                .txd_at(chain, cycle)
                .map(|l| if l { Level::High } else { Level::Low });
            if Some(ours) != theirs {
                report.add(
                    "tx-level",
                    Some(chain as u8),
                    None,
                    cycle,
                    format!("mark {tag}: circuit {ours:?}, model {theirs:?}"),
                );
                break;
            }
        }
    }
}

pub fn compare(
    seq: &Sequence,
    circuit: &CircuitTrace,
    model: &ModelTrace,
    mapping: &Mapping,
) -> Report {
    let mut report = Report::default();
    if let Some((step, cycle)) = circuit.timeout {
        report.add(
            "circuit-timeout",
            None,
            Some(step),
            cycle,
            "bus handshake did not complete".into(),
        );
    }
    compare_reads(seq, circuit, model, &mut report);
    compare_writes(seq, circuit, model, &mut report);
    compare_tx(seq, circuit, model, mapping, &mut report);
    for (chain, u) in model.unsupported.iter().enumerate() {
        if let Some((cycle, why)) = u {
            report
                .unsupported
                .push(format!("chain {chain} from cycle {cycle}: {why}"));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 8N1 waveform for bytes, starting idle high at cycle 0.
    fn wave(bytes: &[u8], period: u64, start: u64) -> Vec<(u64, Level)> {
        let mut out = vec![(0, Level::High)];
        let mut t = start;
        for &b in bytes {
            let bits = std::iter::once(false)
                .chain((0..8).map(|i| b >> i & 1 != 0))
                .chain(std::iter::once(true));
            for bit in bits {
                let level = if bit { Level::High } else { Level::Low };
                if out.last().unwrap().1 != level {
                    out.push((t, level));
                }
                t += period;
            }
        }
        out
    }

    #[test]
    fn decode_reads_back_bytes() {
        let w = wave(&[0x55, 0x00, 0xff, 0x41], 32, 100);
        let frames = decode(&w, 32, 10_000);
        let bytes: Vec<_> = frames.iter().map(|f| f.byte).collect();
        assert_eq!(bytes, [Some(0x55), Some(0x00), Some(0xff), Some(0x41)]);
        assert_eq!(frames[0].start, 100);
    }

    #[test]
    fn decode_marks_a_break_and_recovers() {
        let mut w = vec![(0, Level::High), (50, Level::Low), (1000, Level::High)];
        w.extend(wave(&[0x41], 16, 2000).into_iter().skip(1));
        let frames = decode(&w, 16, 10_000);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].byte, None);
        assert_eq!(frames[1].byte, Some(0x41));
    }

    #[test]
    fn findings_carry_the_register_offset() {
        let mut r = Report::default();
        r.add(
            "read-mismatch",
            Some(0),
            Some(3),
            10,
            "read 0x300c: circuit 0x00000000, model 0x00000014, quiet=true".into(),
        );
        r.add(
            "tx-level",
            Some(1),
            None,
            20,
            "mark 1: circuit High, model Some(Low)".into(),
        );
        assert_eq!(r.findings[0].offset, Some(0x300c));
        assert_eq!(r.findings[0].key(), "read-mismatch 0x300c");
        assert_eq!(r.findings[1].key(), "tx-level");
    }

    #[test]
    fn decode_ignores_frames_cut_by_the_end() {
        let w = wave(&[0x41], 16, 100);
        assert!(decode(&w, 16, 150).is_empty());
    }
}

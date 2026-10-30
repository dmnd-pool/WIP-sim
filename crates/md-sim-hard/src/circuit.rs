//! Parse the trace written by the Verilog player.
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;

/// One completed bus operation. `data` and `resp` are `None` when the
/// circuit returned unknown (x or z) bits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BusObs {
    pub start: u64,
    pub done: u64,
    pub data: Option<u32>,
    pub resp: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Low,
    High,
    Unknown,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CircuitTrace {
    pub reads: BTreeMap<usize, BusObs>,
    pub writes: BTreeMap<usize, BusObs>,
    /// Level changes per output pin, sampled once per cycle.
    pub outputs: BTreeMap<u16, Vec<(u64, Level)>>,
    pub irq: Vec<(u64, Option<u32>)>,
    pub marks: Vec<(u32, u64)>,
    /// Step index and cycle of a bus handshake that never completed.
    pub timeout: Option<(usize, u64)>,
    pub end: Option<u64>,
}

impl CircuitTrace {
    /// Last cycle covered by the trace.
    pub fn last_cycle(&self) -> u64 {
        self.end
            .or(self.timeout.map(|(_, c)| c))
            .or_else(|| {
                self.reads
                    .values()
                    .chain(self.writes.values())
                    .map(|o| o.done)
                    .max()
            })
            .unwrap_or(0)
    }

    /// Output level of one pin at a cycle, from the change list.
    pub fn level_at(&self, pad: u16, cycle: u64) -> Level {
        self.outputs
            .get(&pad)
            .and_then(|changes| changes.iter().rev().find(|(c, _)| *c <= cycle))
            .map_or(Level::Unknown, |&(_, level)| level)
    }
}

fn hex(token: &str) -> Option<u32> {
    u32::from_str_radix(token, 16).ok()
}

fn bits(token: &str) -> Option<u8> {
    u8::from_str_radix(token, 2).ok()
}

fn level(token: &str) -> Level {
    match token {
        "0" => Level::Low,
        "1" => Level::High,
        _ => Level::Unknown,
    }
}

pub fn parse(text: &str) -> Result<CircuitTrace> {
    let mut trace = CircuitTrace::default();
    for (number, line) in text.lines().enumerate() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let Some(&kind) = fields.first() else {
            continue;
        };
        let get = |i: usize| -> Result<&str> {
            fields
                .get(i)
                .copied()
                .with_context(|| format!("trace line {} is short: {line}", number + 1))
        };
        let num = |i: usize| -> Result<u64> {
            get(i)?
                .parse()
                .with_context(|| format!("trace line {}: {line}", number + 1))
        };
        match kind {
            "R" => {
                trace.reads.insert(
                    num(1)? as usize,
                    BusObs {
                        start: num(2)?,
                        done: num(3)?,
                        data: hex(get(4)?),
                        resp: bits(get(5)?),
                    },
                );
            }
            "W" => {
                trace.writes.insert(
                    num(1)? as usize,
                    BusObs {
                        start: num(2)?,
                        done: num(3)?,
                        data: None,
                        resp: bits(get(4)?),
                    },
                );
            }
            "T" => trace
                .outputs
                .entry(num(2)? as u16)
                .or_default()
                .push((num(1)?, level(get(3)?))),
            "Q" => trace.irq.push((num(1)?, hex(get(2)?))),
            "K" => trace.marks.push((num(1)? as u32, num(2)?)),
            "X" => trace.timeout = Some((num(1)? as usize, num(2)?)),
            "E" => trace.end = Some(num(1)?),
            // Simulator messages such as the PS7 model notice go to stdout,
            // not this file. Anything else here is a format error.
            _ => bail!("unknown trace line {}: {line}", number + 1),
        }
    }
    Ok(trace)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_keeps_unknown_bits_unknown() {
        let t = parse(
            "T 0 3 x\nT 5 3 1\nR 0 1 12 00901002 00\nR 1 13 24 xxxxxxxx 00\nW 2 25 33 00\nQ 7 00000\nK 1 40\nE 50\n",
        )
        .unwrap();
        assert_eq!(t.reads[&0].data, Some(0x0090_1002));
        assert_eq!(t.reads[&1].data, None);
        assert_eq!(t.writes[&2].resp, Some(0));
        assert_eq!(t.level_at(3, 4), Level::Unknown);
        assert_eq!(t.level_at(3, 5), Level::High);
        assert_eq!(t.marks, [(1, 40)]);
        assert_eq!(t.last_cycle(), 50);
    }

    #[test]
    fn parse_records_timeout() {
        let t = parse("X 4 2000\n").unwrap();
        assert_eq!(t.timeout, Some((4, 2000)));
        assert_eq!(t.last_cycle(), 2000);
    }

    #[test]
    fn parse_rejects_unknown_lines() {
        assert!(parse("Z 1\n").is_err());
    }
}

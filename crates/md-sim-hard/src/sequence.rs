//! One test sequence: bus steps for the CPU side and timed level changes for
//! the FPGA input pins. The same sequence drives the circuit and the model.
use serde::{Deserialize, Serialize};
use std::fmt::Write;

pub const SCHEMA: &str = "md-sim-hard-s9-sequence/1";
pub const CHAINS: usize = 3;
/// S9IO base addresses read from the archived bosminer binary
/// (`simulator/firmware/s9-bosminer-mmio-511.json`).
pub const BASES: [u32; CHAINS] = [0x43c0_0000, 0x43c1_0000, 0x43c2_0000];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Step {
    /// `settled` asks for an exact comparison. The comparison still checks
    /// that the model is quiet at that moment.
    Read {
        chain: u8,
        offset: u16,
        settled: bool,
    },
    Write {
        chain: u8,
        offset: u16,
        data: u32,
        strobe: u8,
    },
    /// Wait this many clock cycles after the previous step.
    Idle { cycles: u32 },
    /// Wait until the absolute cycle count reaches this value.
    Until { cycle: u64 },
    /// Record the cycle and pin levels at this point.
    Mark { tag: u32 },
}

/// A level change on one FPGA input pin, at an absolute cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PadEvent {
    pub cycle: u64,
    pub pad: u16,
    pub level: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sequence {
    pub schema: String,
    pub seed: u64,
    pub kind: String,
    /// Serial divisor per chain. It stays constant inside one sequence.
    pub divisors: [u16; CHAINS],
    pub steps: Vec<Step>,
    pub events: Vec<PadEvent>,
    /// Idle cycles after the last step.
    pub tail: u32,
}

impl Sequence {
    pub fn new(seed: u64, kind: impl Into<String>) -> Self {
        Self {
            schema: SCHEMA.into(),
            seed,
            kind: kind.into(),
            divisors: [0; CHAINS],
            steps: Vec::new(),
            events: Vec::new(),
            tail: 200,
        }
    }

    /// Plain text read by the Verilog player. Opcodes: 1 read, 2 write,
    /// 3 idle, 4 wait until, 5 mark, 9 end.
    pub fn bench_text(&self) -> String {
        let mut events = self.events.clone();
        events.sort_by_key(|e| e.cycle);
        let mut text = String::new();
        let _ = writeln!(text, "{}", events.len());
        for e in &events {
            let _ = writeln!(text, "{} {} {}", e.cycle, e.pad, u8::from(e.level));
        }
        for step in &self.steps {
            let _ = match *step {
                Step::Read { chain, offset, .. } => {
                    writeln!(text, "1 {:08x}", address(chain, offset))
                }
                Step::Write {
                    chain,
                    offset,
                    data,
                    strobe,
                } => writeln!(
                    text,
                    "2 {:08x} {:08x} {:x}",
                    address(chain, offset),
                    data,
                    strobe & 0xf
                ),
                Step::Idle { cycles } => writeln!(text, "3 {cycles}"),
                Step::Until { cycle } => writeln!(text, "4 {cycle}"),
                Step::Mark { tag } => writeln!(text, "5 {tag}"),
            };
        }
        let _ = writeln!(text, "9 {}", self.tail);
        text
    }
}

pub fn address(chain: u8, offset: u16) -> u32 {
    BASES[usize::from(chain)] + u32::from(offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_text_uses_absolute_addresses_and_sorted_events() {
        let mut s = Sequence::new(1, "unit");
        s.events = vec![
            PadEvent {
                cycle: 9,
                pad: 2,
                level: false,
            },
            PadEvent {
                cycle: 3,
                pad: 1,
                level: true,
            },
        ];
        s.steps = vec![
            Step::Write {
                chain: 1,
                offset: 0x10,
                data: 0xabcd,
                strobe: 0xf,
            },
            Step::Read {
                chain: 2,
                offset: 0x100c,
                settled: true,
            },
            Step::Idle { cycles: 5 },
            Step::Until { cycle: 77 },
            Step::Mark { tag: 4 },
        ];
        s.tail = 10;
        assert_eq!(
            s.bench_text(),
            "2\n3 1 1\n9 2 0\n2 43c10010 0000abcd f\n1 43c2100c\n3 5\n4 77\n5 4\n9 10\n"
        );
    }

    #[test]
    fn sequence_round_trips_through_json() {
        let mut s = Sequence::new(5, "unit");
        s.steps.push(Step::Mark { tag: 1 });
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<Sequence>(&text).unwrap(), s);
    }
}

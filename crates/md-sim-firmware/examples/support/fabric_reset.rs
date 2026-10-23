//! Actual system resets must clear fabric divider and gate state.
use super::{qmp, Transactions};
use anyhow::{ensure, Result};
use serde_json::Value;
use std::io::{BufRead, Write};

fn defaults(io: &mut Transactions) -> Result<()> {
    for channel in 0..4 {
        let base = 0xf8000170 + channel * 16;
        for (offset, value) in [(0, 0x00101800), (4, 0), (8, 0), (12, 0x10000)] {
            io.read(base + offset, value)?;
        }
    }
    io.read(0xf8000240, 0x01f33f0f)
}

pub fn run(
    io: &mut Transactions,
    output: &mut impl Write,
    input: &mut impl BufRead,
    events: &mut Vec<Value>,
) -> Result<()> {
    defaults(io)?;
    for cycle in 0..2 {
        io.write(0xf8000008, 0xdf0d)?;
        for channel in 0..4 {
            let base = 0xf8000170 + channel * 16;
            io.write(base, 0x00400500)?;
            io.read(base, 0x00400500)?;
            io.write(base + 8, 1)?;
            io.read(base + 8, 1)?;
        }
        io.write(0xf8000240, 0)?;
        io.read(0xf8000240, 0)?;
        io.advance(1_000_000)?;
        qmp(output, input, 2 + cycle * 2, "system_reset", events)?;
        qmp(output, input, 3 + cycle * 2, "query-status", events)?;
        ensure!(
            events.iter().filter(|e| e["event"] == "RESET").count() == cycle as usize + 1,
            "actual fabric reset event absent"
        );
        defaults(io)?;
        io.advance(1_000_000)?;
        defaults(io)?;
    }
    ensure!(
        io.irq_events.is_empty(),
        "unexpected GIC interrupt during fabric reset"
    );
    Ok(())
}

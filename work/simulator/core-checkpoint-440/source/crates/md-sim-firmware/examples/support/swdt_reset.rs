//! SWDT lifecycle tests on the shared private QMP/qtest transport.
use super::{qmp, Transactions};
use anyhow::{ensure, Result};
use serde_json::Value;
use std::io::{BufRead, Write};

fn defaults(io: &mut Transactions) -> Result<()> {
    for (offset, value) in [(0, 0x1c0), (4, 0x3ffc), (8, 0), (12, 0)] {
        io.read(0xf8005000 + offset, value)?;
    }
    Ok(())
}

fn program(io: &mut Transactions, mode: u32) -> Result<()> {
    for (address, value) in [
        (0xf8000008, 0xdf0d),
        (0xf8000100, 0x28000),
        (0xf8000120, 0x1f000200),
        (0xf80001c4, 1),
        (0xf8005000, 0xabc040),
        (0xf8005004, 0x920000),
        (0xf8005000, mode),
        (0xf8005008, 0x1999),
    ] {
        io.write(address, value)?;
    }
    Ok(())
}

fn resets(events: &[Value], count: usize) -> Result<()> {
    ensure!(
        events.iter().filter(|e| e["event"] == "RESET").count() == count,
        "actual SWDT lifecycle reset count differs"
    );
    Ok(())
}

pub fn run(
    io: &mut Transactions,
    output: &mut impl Write,
    input: &mut impl BufRead,
    events: &mut Vec<Value>,
) -> Result<()> {
    defaults(io)?;
    // Reset while an actual32-cycle pulse is high must lower its GIC wire.
    program(io, 0xabc1c5)?;
    io.advance(294842)?;
    io.read(0xf800500c, 1)?;
    ensure!(
        io.irq_events == [(9, true)],
        "watchdog interrupt was not raised"
    );
    qmp(output, input, 2, "system_reset", events)?;
    qmp(output, input, 3, "query-status", events)?;
    resets(events, 1)?;
    defaults(io)?;
    ensure!(
        io.irq_events == [(9, true), (9, false)],
        "system reset did not lower SWDT IRQ"
    );
    io.advance(1000)?;
    defaults(io)?;

    // The next reset must come from the modeled timeout. No QMP reset
    // request is sent in this phase. QMP only observes the resulting state.
    program(io, 0xabc043)?;
    io.advance(294842)?;
    qmp(output, input, 4, "query-status", events)?;
    qmp(output, input, 5, "query-status", events)?;
    resets(events, 2)?;
    ensure!(
        events
            .iter()
            .filter(|e| e["event"] == "RESET")
            .nth(1)
            .is_some_and(|e| e["data"]["guest"] == true),
        "watchdog reset is not guest-caused"
    );
    defaults(io)?;
    io.advance(1000)?;
    defaults(io)?;

    // Reset an armed counter before its deadline. Its old callback must
    // not fire later and must not produce a second reset or an interrupt.
    program(io, 0xabc047)?;
    io.advance(100000)?;
    qmp(output, input, 7, "system_reset", events)?;
    qmp(output, input, 8, "query-status", events)?;
    resets(events, 3)?;
    defaults(io)?;
    io.advance(500000)?;
    defaults(io)?;
    qmp(output, input, 9, "query-status", events)?;
    resets(events, 3)?;
    ensure!(
        io.irq_events == [(9, true), (9, false)],
        "unexpected SWDT interrupt after reset"
    );
    Ok(())
}

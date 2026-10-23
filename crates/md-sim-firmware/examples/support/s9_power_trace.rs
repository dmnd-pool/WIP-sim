//! Join the declared PIC enable output to the connected ASIC power input.
//! Timestamps can differ in real-time QEMU. Producer order must still hold.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

#[derive(Default)]
pub struct PowerTrace {
    producer_on: bool,
    consumer_on: bool,
    last_producer_ns: u64,
    pending: Option<(u64, usize)>,
    transfers: Vec<Value>,
}

impl PowerTrace {
    pub fn producer(&mut self, ns: u64, line: usize, on: bool) -> Result<()> {
        ensure!(ns >= self.last_producer_ns, "PIC power clock reversed");
        self.last_producer_ns = ns;
        if on != self.producer_on {
            ensure!(self.pending.is_none(), "PIC power edge was not delivered");
            self.producer_on = on;
            self.pending = Some((ns, line));
        }
        Ok(())
    }

    pub fn consumer(&mut self, ns: u64, line: usize, on: bool) -> Result<()> {
        ensure!(
            on == self.producer_on,
            "ASIC power differs from the PIC output"
        );
        if on != self.consumer_on {
            let (producer_ns, producer_line) =
                self.pending.context("ASIC power edge lacks a PIC source")?;
            ensure!(
                producer_line < line && producer_ns <= ns,
                "PIC power trace follows its ASIC consumer"
            );
            self.pending = None;
            self.consumer_on = on;
            self.transfers
                .push(json!({"on":on,"producer_ns":producer_ns,"consumer_ns":ns,
                "producer_line":producer_line,"consumer_line":line}));
        }
        Ok(())
    }

    pub fn finish(&self) -> Result<&[Value]> {
        ensure!(
            self.pending.is_none(),
            "undelivered PIC power edge at trace end"
        );
        ensure!(
            !self.transfers.is_empty(),
            "no PIC-to-ASIC power transition observed"
        );
        Ok(&self.transfers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn producer_must_precede_and_match_every_consumer_edge() -> Result<()> {
        let mut trace = PowerTrace::default();
        trace.consumer(0, 1, false)?;
        // Captured dangerous-174: PIC0x57 then chain8 at power-on and shutdown.
        trace.producer(74_811_707_939, 38259, true)?;
        trace.consumer(74_811_711_295, 38260, true)?;
        trace.producer(151_721_800_993, 204419, false)?;
        assert!(trace.finish().is_err());
        assert!(trace.consumer(151_721_800_992, 204420, false).is_err());
        trace.consumer(151_721_803_538, 204420, false)?;
        assert_eq!(trace.finish()?.len(), 2);
        let mut late = PowerTrace::default();
        assert!(late.consumer(1, 1, true).is_err());
        let mut reversed = PowerTrace::default();
        reversed.producer(2, 2, true)?;
        assert!(reversed.consumer(3, 1, true).is_err());
        let mut missing = PowerTrace::default();
        missing.producer(1, 1, true)?;
        assert!(missing.producer(2, 2, false).is_err());
        let mut valid = PowerTrace::default();
        valid.producer(1, 1, true)?;
        valid.consumer(2, 2, true)?;
        valid.producer(3, 3, true)?; // Voltage code changes need no enable edge.
        valid.consumer(4, 4, true)?;
        valid.producer(5, 5, false)?;
        valid.consumer(6, 6, false)?;
        assert_eq!(valid.finish()?.len(), 2);
        assert!(valid.producer(4, 7, false).is_err());
        Ok(())
    }
}

//! Event-timed serial wire contract: one start bit, eight low-bit-first
//! data bits and one stop bit. This is not the S9IO HDL UART model.
//! ASIC adapters must declare this sampling contract and their bit period.
//! It has no host-clock or mining-rate dependency.
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    BitPeriod,
    Capacity,
    Busy,
    TimeReversed,
    TimeOverflow,
    SnapshotVersion,
    SnapshotState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Level { ns: u64, high: bool },
    Received { ns: u64, byte: u8 },
    Transmitted { ns: u64, byte: u8 },
    FalseStart { ns: u64 },
    FramingError { ns: u64, byte: u8 },
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Receive {
    #[default]
    Idle,
    Start,
    Data {
        byte: u8,
        index: u8,
    },
    Stop {
        byte: u8,
    },
    WaitHigh,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Transmit {
    byte: u8,
    index: u8,
}

/// Device-local state at the last processed timestamp. The board must also
/// save the peer's pins and event queue at that same barrier. This does not
/// by itself provide a complete controller or network snapshot.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    version: u32,
    now: u64,
    bit_ns: u64,
    capacity: usize,
    rx_high: bool,
    tx_high: bool,
    receive: Receive,
    receive_at: Option<u64>,
    transmit: Option<Transmit>,
    transmit_at: Option<u64>,
    pending: VecDeque<u8>,
}

pub struct Serial {
    now: u64,
    bit_ns: u64,
    capacity: usize,
    rx_high: bool,
    tx_high: bool,
    receive: Receive,
    receive_at: Option<u64>,
    transmit: Option<Transmit>,
    transmit_at: Option<u64>,
    pending: VecDeque<u8>,
}

impl Serial {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            version: 1,
            now: self.now,
            bit_ns: self.bit_ns,
            capacity: self.capacity,
            rx_high: self.rx_high,
            tx_high: self.tx_high,
            receive: self.receive.clone(),
            receive_at: self.receive_at,
            transmit: self.transmit.clone(),
            transmit_at: self.transmit_at,
            pending: self.pending.clone(),
        }
    }

    /// Reject a different schema or an impossible state before installing it.
    /// In particular, restoring cannot inject an overdue byte or a line edge.
    pub fn restore(saved: Snapshot) -> Result<Self, Error> {
        if saved.version != 1 {
            return Err(Error::SnapshotVersion);
        }
        Self::validate_period(saved.bit_ns)?;
        if !(1..=4096).contains(&saved.capacity)
            || saved.pending.len() + usize::from(saved.transmit.is_some()) > saved.capacity
        {
            return Err(Error::Capacity);
        }
        let future = |at: Option<u64>, span: u64| {
            at.is_some_and(|at| at > saved.now && at - saved.now <= span)
        };
        let receive_valid = match saved.receive {
            Receive::Idle => saved.receive_at.is_none() && saved.rx_high,
            Receive::WaitHigh => saved.receive_at.is_none() && !saved.rx_high,
            Receive::Start => future(saved.receive_at, saved.bit_ns / 2),
            Receive::Data { byte, index } => {
                index < 8
                    && u16::from(byte) < (1 << index)
                    && future(saved.receive_at, saved.bit_ns)
            }
            Receive::Stop { .. } => future(saved.receive_at, saved.bit_ns),
        };
        let transmit_valid = match &saved.transmit {
            None => saved.transmit_at.is_none() && saved.pending.is_empty() && saved.tx_high,
            Some(tx) => {
                tx.index <= 9
                    && future(saved.transmit_at, saved.bit_ns)
                    && saved.tx_high
                        == match tx.index {
                            0 => false,
                            1..=8 => tx.byte & (1 << (tx.index - 1)) != 0,
                            _ => true,
                        }
            }
        };
        if !receive_valid || !transmit_valid {
            return Err(Error::SnapshotState);
        }
        let receive_ticks = match saved.receive {
            Receive::Start => 9,
            Receive::Data { index, .. } => 8 - u64::from(index),
            _ => 0,
        };
        if saved.receive_at.is_some_and(|at| {
            saved
                .bit_ns
                .checked_mul(receive_ticks)
                .and_then(|span| at.checked_add(span))
                .is_none()
        }) {
            return Err(Error::SnapshotState);
        }
        Self::transmit_end(
            saved.now,
            saved.bit_ns,
            saved.transmit.as_ref(),
            saved.transmit_at,
            saved.pending.len(),
        )
        .ok_or(Error::SnapshotState)?;
        Ok(Self {
            now: saved.now,
            bit_ns: saved.bit_ns,
            capacity: saved.capacity,
            rx_high: saved.rx_high,
            tx_high: saved.tx_high,
            receive: saved.receive,
            receive_at: saved.receive_at,
            transmit: saved.transmit,
            transmit_at: saved.transmit_at,
            pending: saved.pending,
        })
    }

    fn transmit_end(
        now: u64,
        bit_ns: u64,
        tx: Option<&Transmit>,
        at: Option<u64>,
        queued: usize,
    ) -> Option<u64> {
        let end = match tx {
            Some(tx) => at?.checked_add(bit_ns.checked_mul(u64::from(9 - tx.index))?)?,
            None => now,
        };
        end.checked_add(bit_ns.checked_mul(10 * queued as u64)?)
    }

    /// Periods must have a whole-nanosecond midpoint. Capacity includes
    /// the byte currently on the wire, not only the pending FIFO.
    pub fn new(bit_ns: u64, capacity: usize) -> Result<Self, Error> {
        Self::validate_period(bit_ns)?;
        if !(1..=4096).contains(&capacity) {
            return Err(Error::Capacity);
        }
        Ok(Self {
            now: 0,
            bit_ns,
            capacity,
            rx_high: true,
            tx_high: true,
            receive: Receive::Idle,
            receive_at: None,
            transmit: None,
            transmit_at: None,
            pending: VecDeque::new(),
        })
    }

    fn validate_period(bit_ns: u64) -> Result<(), Error> {
        if bit_ns < 2 || bit_ns % 2 != 0 {
            return Err(Error::BitPeriod);
        }
        Ok(())
    }

    pub fn bit_period_ns(&self) -> u64 {
        self.bit_ns
    }

    pub fn tx_high(&self) -> bool {
        self.tx_high
    }

    pub fn queued_bytes(&self) -> usize {
        self.pending.len() + usize::from(self.transmit.is_some())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.receive_at.into_iter().chain(self.transmit_at).min()
    }

    /// The caller first advances to the configuration write's timestamp.
    /// A changed rate during a frame is unsupported, not silently delayed.
    pub fn configure(&mut self, bit_ns: u64) -> Result<(), Error> {
        Self::validate_period(bit_ns)?;
        if self.bit_ns == bit_ns {
            return Ok(());
        }
        if self.transmit.is_some() || !matches!(self.receive, Receive::Idle) {
            return Err(Error::Busy);
        }
        self.bit_ns = bit_ns;
        Ok(())
    }

    /// Reset at the current timestamp, after advance(), drops partial
    /// frames and queued bytes. The external input
    /// level is retained. A low input must return high before a new start.
    /// The caller propagates tx_high() after reset.
    pub fn reset(&mut self) {
        self.receive = if self.rx_high {
            Receive::Idle
        } else {
            Receive::WaitHigh
        };
        self.receive_at = None;
        self.transmit = None;
        self.transmit_at = None;
        self.tx_high = true;
        self.pending.clear();
    }

    /// A device reset can restore its declared baud divisor even if the
    /// input is held low. The line still waits for high before a new frame.
    pub fn reset_period(&mut self, bit_ns: u64) -> Result<(), Error> {
        Self::validate_period(bit_ns)?;
        self.reset();
        self.bit_ns = bit_ns;
        Ok(())
    }

    /// Enqueue at the current timestamp, after advance(). Rejection leaves
    /// the queue unchanged. The returned events are actual output edges.
    pub fn enqueue(&mut self, bytes: &[u8]) -> Result<Vec<Event>, Error> {
        if bytes.len() > self.capacity - self.queued_bytes() {
            return Err(Error::Capacity);
        }
        // Reserve representable time for every queued byte, not just its
        // first edge. Overflow cannot strand a half-transmitted frame.
        Self::transmit_end(
            self.now,
            self.bit_ns,
            self.transmit.as_ref(),
            self.transmit_at,
            self.pending.len() + bytes.len(),
        )
        .ok_or(Error::TimeOverflow)?;
        self.pending.extend(bytes.iter().copied());
        let mut events = Vec::new();
        self.start_transmit(self.now, &mut events)?;
        Ok(events)
    }

    /// Elapsed sampling events precede a pin transition at the same ns.
    pub fn input(&mut self, ns: u64, high: bool) -> Result<Vec<Event>, Error> {
        let events = self.advance(ns)?;
        if matches!(self.receive, Receive::WaitHigh) && high {
            self.receive = Receive::Idle;
        } else if matches!(self.receive, Receive::Idle) && self.rx_high && !high {
            self.bit_ns
                .checked_mul(9)
                .and_then(|span| span.checked_add(self.bit_ns / 2))
                .and_then(|span| ns.checked_add(span))
                .ok_or(Error::TimeOverflow)?;
            self.receive_at = Some(ns.checked_add(self.bit_ns / 2).ok_or(Error::TimeOverflow)?);
            self.receive = Receive::Start;
        }
        self.rx_high = high;
        Ok(events)
    }

    pub fn advance(&mut self, ns: u64) -> Result<Vec<Event>, Error> {
        if ns < self.now {
            return Err(Error::TimeReversed);
        }
        let mut events = Vec::new();
        while let Some(at) = self.next_deadline().filter(|at| *at <= ns) {
            self.now = at;
            if self.receive_at == Some(at) {
                self.sample(at, &mut events)?;
            }
            if self.transmit_at == Some(at) {
                self.shift(at, &mut events)?;
            }
        }
        self.now = ns;
        Ok(events)
    }

    fn sample(&mut self, ns: u64, events: &mut Vec<Event>) -> Result<(), Error> {
        self.receive_at = None;
        self.receive = match self.receive {
            Receive::Start if self.rx_high => {
                events.push(Event::FalseStart { ns });
                Receive::Idle
            }
            Receive::Start => Receive::Data { byte: 0, index: 0 },
            Receive::Data { byte, index } => {
                let byte = byte | (u8::from(self.rx_high) << index);
                if index == 7 {
                    Receive::Stop { byte }
                } else {
                    Receive::Data {
                        byte,
                        index: index + 1,
                    }
                }
            }
            Receive::Stop { byte } if self.rx_high => {
                events.push(Event::Received { ns, byte });
                Receive::Idle
            }
            Receive::Stop { byte } => {
                events.push(Event::FramingError { ns, byte });
                Receive::WaitHigh
            }
            Receive::Idle | Receive::WaitHigh => unreachable!("unscheduled receiver state"),
        };
        if matches!(self.receive, Receive::Data { .. } | Receive::Stop { .. }) {
            self.receive_at = Some(ns.checked_add(self.bit_ns).ok_or(Error::TimeOverflow)?);
        }
        Ok(())
    }

    fn level(&mut self, ns: u64, high: bool, events: &mut Vec<Event>) {
        if self.tx_high != high {
            self.tx_high = high;
            events.push(Event::Level { ns, high });
        }
    }

    fn start_transmit(&mut self, ns: u64, events: &mut Vec<Event>) -> Result<(), Error> {
        if self.transmit.is_none() && !self.pending.is_empty() {
            let at = ns.checked_add(self.bit_ns).ok_or(Error::TimeOverflow)?;
            let byte = self.pending.pop_front().unwrap();
            self.transmit = Some(Transmit { byte, index: 0 });
            self.transmit_at = Some(at);
            self.level(ns, false, events);
        }
        Ok(())
    }

    fn shift(&mut self, ns: u64, events: &mut Vec<Event>) -> Result<(), Error> {
        let mut tx = self.transmit.take().expect("scheduled transmitter");
        self.transmit_at = None;
        if tx.index == 9 {
            events.push(Event::Transmitted { ns, byte: tx.byte });
            return self.start_transmit(ns, events);
        }
        let high = tx.index == 8 || tx.byte & (1 << tx.index) != 0;
        tx.index += 1;
        let at = ns.checked_add(self.bit_ns).ok_or(Error::TimeOverflow)?;
        self.transmit = Some(tx);
        self.transmit_at = Some(at);
        self.level(ns, high, events);
        Ok(())
    }
}

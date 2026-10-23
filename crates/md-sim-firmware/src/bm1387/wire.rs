//! Powered BM1387 serial adapter. Queued replies use actual 8N1 wire time.
//! Register response ordering follows chain position. Electrical per-hop
//! delay and buffer capacity are explicit development policies.
use super::{
    budget::Slot,
    chain::{self, Chain, Profile},
    filter::Policy,
    search::Attempt,
    Decoder,
};
use crate::serial8n1::{self, Event, Serial};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Chain(chain::Error),
    Serial(serial8n1::Error),
}

impl From<chain::Error> for Error {
    fn from(value: chain::Error) -> Self {
        Self::Chain(value)
    }
}
impl From<serial8n1::Error> for Error {
    fn from(value: serial8n1::Error) -> Self {
        Self::Serial(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trace {
    Serial(Event),
    Command {
        ns: u64,
        opcode: u8,
        replies: usize,
    },
    Rejected {
        ns: u64,
        error: super::Error,
    },
    Work {
        ns: u64,
        id: u8,
        enabled_cores: usize,
    },
    Hash {
        ns: u64,
        grant: super::budget::Grant,
        attempt: Attempt,
    },
    /// The model stopped with an explicit error. This is not a chip reply.
    Stopped {
        ns: u64,
    },
}

pub struct Wire {
    chain: Chain,
    serial: Serial,
    decoder: Decoder,
    opcode: Option<u8>,
    now: u64,
    slot: Option<Slot>,
    hash_at: Option<u64>,
    failed_trace: Vec<Trace>,
}

impl Wire {
    pub fn new(profile: Profile, reply_byte_capacity: usize) -> Result<Self, Error> {
        let chain = Chain::new(profile)?;
        Self::from_chain(chain, reply_byte_capacity, None)
    }

    pub fn with_mining(
        profile: Profile,
        reply_byte_capacity: usize,
        core_maps: Vec<Vec<u8>>,
        slot: Slot,
    ) -> Result<Self, Error> {
        Self::with_mining_filter(
            profile,
            reply_byte_capacity,
            core_maps,
            slot,
            Policy::default(),
        )
    }

    pub fn with_mining_filter(
        profile: Profile,
        reply_byte_capacity: usize,
        core_maps: Vec<Vec<u8>>,
        slot: Slot,
        filter_policy: Policy,
    ) -> Result<Self, Error> {
        Self::from_chain(
            Chain::with_mining_filter(profile, core_maps, filter_policy)?,
            reply_byte_capacity,
            Some(slot),
        )
    }

    fn from_chain(
        chain: Chain,
        reply_byte_capacity: usize,
        slot: Option<Slot>,
    ) -> Result<Self, Error> {
        let serial = Serial::new(chain.initial_serial_period_ns(), reply_byte_capacity)?;
        Ok(Self {
            chain,
            serial,
            decoder: Decoder::default(),
            opcode: None,
            now: 0,
            slot,
            hash_at: None,
            failed_trace: Vec::new(),
        })
    }

    pub fn tx_high(&self) -> bool {
        self.serial.tx_high()
    }

    pub fn now_ns(&self) -> u64 {
        self.now
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.serial
            .next_deadline()
            .into_iter()
            .chain(self.hash_at)
            .min()
    }

    pub fn bit_period_ns(&self) -> u64 {
        self.serial.bit_period_ns()
    }

    pub fn attach_temperature(
        &mut self,
        position: usize,
        profile: super::i2c::Profile,
    ) -> Result<(), Error> {
        self.chain.attach_temperature(position, profile)?;
        Ok(())
    }

    pub fn install_temperature_plan(
        &mut self,
        events: Vec<super::i2c::InputEvent>,
    ) -> Result<(), Error> {
        self.chain.install_temperature_plan(events)?;
        Ok(())
    }

    /// Advance wire events before applying a power/reset pin change.
    /// Pin methods do not hide elapsed bytes or clear their trace.
    pub fn power(&mut self, enabled: bool) -> Result<(), Error> {
        self.chain.power(enabled)?;
        if !self.chain.running() {
            self.reset_serial();
        }
        Ok(())
    }

    pub fn reset_line(&mut self, asserted: bool) {
        self.chain.reset_line(asserted);
        if !self.chain.running() {
            self.reset_serial();
        }
    }

    fn reset_serial(&mut self) {
        self.serial
            .reset_period(self.chain.initial_serial_period_ns())
            .expect("validated serial period");
        self.decoder.reset();
        self.opcode = None;
        self.hash_at = None;
        if let Some(slot) = &self.slot {
            slot.set_weight(0);
        }
    }

    /// System reset starts a new device time origin. Keep the run-owned
    /// wall-clock budget, so reset cannot create hashing credit.
    pub fn reset(&mut self) {
        self.chain.reset_time_origin();
        self.failed_trace.clear();
        self.reset_serial();
        self.serial = Serial::new(
            self.chain.initial_serial_period_ns(),
            self.serial.capacity(),
        )
        .expect("validated serial profile");
        self.now = 0;
    }

    fn schedule_hash(&mut self, ns: u64) -> Result<(), Error> {
        if let Some(slot) = &self.slot {
            let cores = self.chain.active_cores();
            slot.set_weight(cores);
            if cores == 0 {
                self.hash_at = None;
            } else if self.hash_at.is_none() {
                self.hash_at = Some(
                    ns.checked_add(slot.interval_ns())
                        .ok_or(serial8n1::Error::TimeOverflow)?,
                );
            }
        }
        Ok(())
    }

    fn events(&mut self, events: Vec<Event>, output: &mut Vec<Trace>) -> Result<(), Error> {
        for event in events {
            output.push(Trace::Serial(event));
            match event {
                Event::Received { ns, byte } => {
                    if self.opcode.is_none() {
                        self.opcode = Some(byte);
                    }
                    if let Some(result) = self.decoder.push(byte) {
                        let opcode = self.opcode.take().unwrap();
                        match result {
                            Ok(packet) => {
                                let effect = self.chain.receive(&packet)?;
                                if let Some(period) = effect.serial_period_ns {
                                    self.serial.configure(period)?;
                                }
                                output.push(Trace::Command {
                                    ns,
                                    opcode,
                                    replies: effect.replies.len(),
                                });
                                if let Some((id, enabled_cores)) = effect.work {
                                    output.push(Trace::Work {
                                        ns,
                                        id,
                                        enabled_cores,
                                    });
                                }
                                let bytes: Vec<_> = effect.replies.into_iter().flatten().collect();
                                output.extend(
                                    self.serial.enqueue(&bytes)?.into_iter().map(Trace::Serial),
                                );
                                self.schedule_hash(ns)?;
                            }
                            Err(error) => output.push(Trace::Rejected { ns, error }),
                        }
                    }
                }
                Event::FramingError { .. } | Event::FalseStart { .. } => {
                    self.decoder.reset();
                    self.opcode = None;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn advance(&mut self, ns: u64) -> Result<Vec<Trace>, Error> {
        let mut output = Vec::new();
        if let Err(error) = self.advance_into(ns, &mut output) {
            output.push(Trace::Stopped { ns: self.now });
            self.failed_trace = output;
            return Err(error);
        }
        Ok(output)
    }

    /// Events that occurred before an error must survive that error.
    pub fn take_failed_trace(&mut self) -> Vec<Trace> {
        std::mem::take(&mut self.failed_trace)
    }

    fn advance_into(&mut self, ns: u64, output: &mut Vec<Trace>) -> Result<(), Error> {
        if ns < self.now {
            return Err(serial8n1::Error::TimeReversed.into());
        }
        // Deliver each byte at its actual stop-bit sampling time. Processing
        // only after the final time would delay replies when polling is sparse.
        while let Some(at) = self.next_deadline().filter(|at| *at <= ns) {
            self.now = at;
            self.chain.advance(at)?;
            let events = self.serial.advance(at)?;
            self.events(events, output)?;
            // A complete received job at the same timestamp is applied
            // before hashing. External power/reset pins follow advance().
            // Queued replies retain their real wire time; a new job does
            // not rewrite earlier results.
            if self.hash_at == Some(at) {
                self.hash_at = None;
                if let Some(slot) = &self.slot {
                    if let Some((grant, attempt)) = self.chain.poll(slot)? {
                        let response = attempt.response;
                        output.push(Trace::Hash {
                            ns: at,
                            grant,
                            attempt,
                        });
                        if let Some(response) = response {
                            output.extend(
                                self.serial
                                    .enqueue(&response)?
                                    .into_iter()
                                    .map(Trace::Serial),
                            );
                        }
                    }
                }
                self.schedule_hash(at)?;
            }
        }
        let events = self.serial.advance(ns)?;
        self.chain.advance(ns)?;
        self.events(events, output)?;
        self.now = ns;
        Ok(())
    }

    pub fn input(&mut self, ns: u64, high: bool) -> Result<Vec<Trace>, Error> {
        let mut output = self.advance(ns)?;
        let result = (|| {
            let events = self.serial.input(ns, high)?;
            if self.chain.running() {
                self.events(events, &mut output)?;
            } else {
                self.serial.reset();
            }
            Ok::<(), Error>(())
        })();
        if let Err(error) = result {
            output.push(Trace::Stopped { ns: self.now });
            self.failed_trace = output;
            return Err(error);
        }
        Ok(output)
    }
}

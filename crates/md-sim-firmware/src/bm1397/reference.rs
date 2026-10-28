//! Explicit software reference ASIC. This is not a physical BM1397 model.
//! It preserves programmed filters and computes only permitted nonces. Queue,
//! routing and measurement policies below are adaptations, not silicon claims.
use super::{ticket_filter, Command, ControlWriteSupport, IdentificationChain, Job, Packet, Reply};
use md_sim_mining::{Computation, ComputedVariant, Engine};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

pub const POLICY: &str = "bm1397-one-chip-computed-work-reference-v1";
pub const RATE_WINDOW: Duration = Duration::from_secs(1);
pub const RATE_UNIT: u64 = 1 << 20;

/// Immutable diagnostic injection. It acts only after a real hash completes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub enum FaultPolicy {
    NoFaults,
    DropEveryNthComputation { interval: u64 },
}
impl FaultPolicy {
    fn drops(self, ordinal: u64) -> bool {
        match self {
            Self::NoFaults => false,
            Self::DropEveryNthComputation { interval } => ordinal.checked_rem(interval) == Some(0),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Policy {
    pub id: &'static str,
    pub chips: u16,
    pub hash_cap_hps: f64,
    pub queue_slots: usize,
    pub rate_window_ns: u64,
    pub rate_unit_hps: u64,
    pub timer: &'static str,
    pub queue: &'static str,
    pub nonce_routing: &'static str,
    pub counter_0x04: &'static str,
    pub counter_0x4c: &'static str,
    pub fault_policy: FaultPolicy,
    pub reset: &'static str,
    pub physical_model: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Error {
    UnsupportedTopology,
    UnsupportedAddressAssignment,
    InvalidPolicy,
    InvalidJob,
    UnsupportedJobId,
    QueueFull,
    GenerationExhausted,
    MeasurementWindowIncomplete,
    Identification(super::IdentifyError),
    Engine(String),
}

#[derive(Clone, Debug, Serialize)]
pub struct Measurement {
    pub completed_windows: u64,
    pub current_window_computations: u64,
    pub last_window_computations: Option<u64>,
    pub epoch_computations: u64,
    pub datapath_drop_events: u64,
    pub register_0x4c: u32,
    pub register_0x04: Option<u32>,
}

/// The ledger has no public increment or arbitrary-count interface. Only a
/// receipt obtained directly from this model's private engine can credit it.
struct Ledger {
    origin: Instant,
    window: u64,
    current: u64,
    last: Option<u64>,
    total: u64,
    drops: u64,
}
impl Ledger {
    fn new(now: Instant) -> Self {
        Self {
            origin: now,
            window: 0,
            current: 0,
            last: None,
            total: 0,
            drops: 0,
        }
    }
    fn advance(&mut self, now: Instant) {
        let index = now.saturating_duration_since(self.origin).as_secs();
        if index > self.window {
            self.last = Some(if index == self.window + 1 {
                self.current
            } else {
                0
            });
            self.current = 0;
            self.window = index;
        }
    }
    fn record(&mut self, receipt: &Computation, fault: FaultPolicy) -> bool {
        self.advance(receipt.completed_at);
        self.current += 1;
        self.total += 1;
        let dropped = fault.drops(self.total);
        if dropped {
            self.drops += 1;
        }
        dropped
    }
    fn snapshot(&mut self, now: Instant) -> Measurement {
        self.advance(now);
        Measurement {
            completed_windows: self.window,
            current_window_computations: self.current,
            last_window_computations: self.last,
            epoch_computations: self.total,
            datapath_drop_events: self.drops,
            register_0x4c: self.drops as u32,
            // One complete second per window; integer division rounds down.
            // The explicit <=10,000 H/s policy prevents register overflow.
            register_0x04: self.last.map(|count| (count / RATE_UNIT) as u32),
        }
    }
}

struct Pending {
    engine_id: String,
    job: Job,
    address: u8,
    computations: u64,
}

pub struct Step {
    pub computation: Computation,
    pub job_id: u8,
    pub chip_address: u8,
    pub reply: Option<Reply>,
    pub datapath_dropped: bool,
    pub job_computation_ordinal: u64,
}

pub struct ReferenceChain {
    chain: IdentificationChain,
    engine: Engine,
    jobs: BTreeMap<u8, Pending>,
    generation: u64,
    ledger: Ledger,
    policy: Policy,
}
impl ReferenceChain {
    pub fn new(chips: u16, cap: f64, queue_slots: usize) -> Result<Self, Error> {
        Self::with_fault_policy(chips, cap, queue_slots, FaultPolicy::NoFaults)
    }
    pub fn with_fault_policy(
        chips: u16,
        cap: f64,
        queue_slots: usize,
        fault_policy: FaultPolicy,
    ) -> Result<Self, Error> {
        if matches!(
            fault_policy,
            FaultPolicy::DropEveryNthComputation { interval: 0 }
        ) {
            return Err(Error::InvalidPolicy);
        }
        if chips != 1 {
            return Err(Error::UnsupportedTopology);
        }
        if !cap.is_finite() || cap <= 0.0 || cap > 10_000.0 || !(1..=32).contains(&queue_slots) {
            return Err(Error::InvalidPolicy);
        }
        Ok(Self {
            chain: IdentificationChain::with_control_support(chips, ControlWriteSupport::RegisterStorage)
                .map_err(Error::Identification)?,
            engine: Engine::new(cap, queue_slots).map_err(Error::Engine)?,
            jobs: BTreeMap::new(), generation: 0, ledger: Ledger::new(Instant::now()),
            policy: Policy {
                id: POLICY, chips, hash_cap_hps: cap, queue_slots,
                rate_window_ns: RATE_WINDOW.as_nanos() as u64, rate_unit_hps: RATE_UNIT,
                timer: "Host monotonic completion time; fixed consecutive one-second windows; no guest or physical clock calibration.",
                queue: "At most32 round-robin live slots for the exact producer IDs0,4,..124; same wire job ID replaces its old cursor; jobs have one or four midstates. Other silicon job-ID semantics unresolved.",
                nonce_routing: "One chip assigned address0; exact ESP address_interval=256/1 maps every eight-bit nonce address field to chip0, so the full supplied nonce range is searched. Physical core routing unresolved; reply midstate tag is reference zero.",
                counter_0x04: "Previous complete one-second count of actual SHA-256d computations / 2^20, rounded down and latched without read-clear; no reply before a complete window. The literal ESP consumer scale conflicts with the public 2^24 claim; physical scale unresolved.",
                counter_0x4c: "Low32 bits of actual internal datapath-drop event count; no read-clear; reset clears epoch. NoFaults uses the same empty ledger. The ESP consumer multiplies count deltas by2^32; this reference event unit is not a physical hash equivalent. Filter misses, CRC errors and pool rejects never increment it.",
                fault_policy,
                reset: "Power loss or asserted reset clears jobs, address/control state and measurement epoch; inactive/address changes cancel jobs but keep measured completed work.",
                physical_model: false,
            },
        })
    }
    pub fn policy(&self) -> &Policy {
        &self.policy
    }
    pub fn power(&mut self, powered: bool) {
        self.chain.power(powered);
        if !powered {
            self.clear_epoch();
        }
    }
    pub fn reset_line(&mut self, asserted: bool) {
        self.chain.reset_line(asserted);
        if asserted {
            self.clear_epoch();
        }
    }
    fn clear_jobs(&mut self) {
        self.engine.reset();
        self.jobs.clear();
    }
    fn clear_epoch(&mut self) {
        self.clear_jobs();
        self.ledger = Ledger::new(Instant::now());
    }
    pub fn queue_len(&self) -> usize {
        self.jobs.len()
    }
    pub fn hashes_computed(&self) -> u64 {
        self.engine.hashes_computed
    }
    pub fn wait_duration(&self) -> Duration {
        self.engine.wait_duration()
    }
    pub fn programmed_controls(
        &self,
        chip: usize,
    ) -> Option<super::programmed::ProgrammedControls> {
        self.chain.programmed_controls(chip)
    }
    pub fn measurement(&mut self) -> Measurement {
        self.ledger.snapshot(Instant::now())
    }

    pub fn receive(&mut self, packet: &Packet) -> Result<Vec<Reply>, Error> {
        let Some(address) = self.chain.assigned_address(0) else {
            return Ok(Vec::new());
        };
        match packet {
            Packet::Job {
                broadcast: false,
                work,
            } => {
                self.submit(work.clone(), address)?;
                Ok(Vec::new())
            }
            Packet::Job {
                broadcast: true, ..
            } => Err(Error::InvalidJob),
            Packet::Command {
                broadcast,
                command:
                    Command::Read {
                        address: selected,
                        register,
                    },
            } if matches!(register, 0x04 | 0x4c) => {
                if !broadcast && *selected != address {
                    return Ok(Vec::new());
                }
                let measurement = self.measurement();
                let value = if *register == 0x04 {
                    measurement
                        .register_0x04
                        .ok_or(Error::MeasurementWindowIncomplete)?
                } else {
                    measurement.register_0x4c
                };
                Ok(vec![Reply::Register {
                    value,
                    asic_address: address,
                    register: *register,
                }])
            }
            _ => {
                let replies = self.chain.receive(packet).map_err(Error::Identification)?;
                match packet {
                    Packet::Command {
                        command: Command::Inactive { .. },
                        ..
                    } => self.clear_jobs(),
                    Packet::Command {
                        command: Command::SetAddress { .. },
                        ..
                    } if self.chain.assigned_address(0) != Some(address) => self.clear_jobs(),
                    Packet::Command {
                        command: Command::Write { register: 0x14, .. },
                        ..
                    } => {
                        let filter = ticket_filter(
                            self.chain.ticket_register(0).expect("active single chip"),
                        );
                        for pending in self.jobs.values() {
                            assert!(self
                                .engine
                                .update_filter(&pending.engine_id, filter.clone()));
                        }
                    }
                    _ => {}
                }
                Ok(replies)
            }
        }
    }
    fn submit(&mut self, job: Job, address: u8) -> Result<(), Error> {
        if address != 0 {
            return Err(Error::UnsupportedAddressAssignment);
        }
        if job.id >= 128 {
            return Err(Error::UnsupportedJobId);
        }
        if job.id & 3 != 0 || !matches!(job.midstate_count, 1 | 4) {
            return Err(Error::InvalidJob);
        }
        if !self.jobs.contains_key(&job.id) && self.jobs.len() >= self.policy.queue_slots {
            return Err(Error::QueueFull);
        }
        let next = self
            .generation
            .checked_add(1)
            .ok_or(Error::GenerationExhausted)?;
        let engine_id = format!("bm1397-reference-{next}-{}", job.id);
        let work = job
            .engine_work(
                engine_id.clone(),
                u32::MAX,
                self.chain.ticket_register(0).expect("active single chip"),
            )
            .map_err(Error::Engine)?;
        if let Some(old) = self.jobs.remove(&job.id) {
            self.engine.cancel(&old.engine_id);
        }
        self.engine.submit_midstate(work).map_err(Error::Engine)?;
        self.generation = next;
        self.jobs.insert(
            job.id,
            Pending {
                engine_id,
                job,
                address,
                computations: 0,
            },
        );
        Ok(())
    }
    pub fn poll(&mut self) -> Option<Step> {
        let computation = self.engine.poll_computation()?;
        // Lookup cannot be supplied by a caller; both maps change together.
        let (&job_id, pending) = self
            .jobs
            .iter_mut()
            .find(|(_, job)| job.engine_id == computation.work_id)
            .expect("private engine work must have a retained job");
        let ComputedVariant::Midstate { index } = computation.variant else {
            unreachable!("BM1397 submits only midstates")
        };
        assert!(index < usize::from(pending.job.midstate_count));
        let datapath_dropped = self.ledger.record(&computation, self.policy.fault_policy);
        pending.computations += 1;
        let job_computation_ordinal = pending.computations;
        let chip_address = pending.address;
        let reply = (computation.accepted && !datapath_dropped).then_some(Reply::Job {
            nonce: computation.nonce,
            midstate_tag: 0,
            job_id,
            midstate_index: index as u8,
        });
        if computation.exhausted {
            self.jobs.remove(&job_id);
        }
        Some(Step {
            computation,
            job_id,
            chip_address,
            reply,
            datapath_dropped,
            job_computation_ordinal,
        })
    }
}

#[cfg(test)]
#[path = "reference_tests.rs"]
mod tests;

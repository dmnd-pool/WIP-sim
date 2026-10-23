//! Shared virtual timeline for the S9IO and its serial ASIC peer.
//! QEMU GPIO callbacks do not carry historic event timestamps. Coupling
//! the peers here preserves each serial edge during a late timer callback.
use md_sim_firmware::{
    bm1387::{
        budget::Slot,
        chain::Profile,
        filter::Policy,
        wire::{Error, Trace, Wire},
    },
    serial8n1::Event,
};
use std::collections::VecDeque;

#[derive(Clone, Copy, Default, Debug)]
#[repr(C)]
pub struct AsicTrace {
    pub ns: u64,
    pub kind: u32,
    pub value: u32,
    pub extra: u32,
    pub grant: u64,
    pub wall_ns: u64,
}

pub struct Peer {
    wire: Wire,
    traces: VecDeque<AsicTrace>,
}

impl Peer {
    pub fn new(profile: Profile, capacity: usize) -> Result<Self, Error> {
        Ok(Self {
            wire: Wire::new(profile, capacity)?,
            traces: VecDeque::new(),
        })
    }

    pub fn new_reference_mining(
        capacity: usize,
        slot: Slot,
        filter: Policy,
    ) -> Result<Self, String> {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../simulator/backends/qemu/bm1387-reference-core-map.json"
        ))
        .map_err(|error| error.to_string())?;
        let cores: Vec<u8> = serde_json::from_value(reference["cores"].clone())
            .map_err(|error| error.to_string())?;
        if cores.len() != 114 {
            return Err("reference map must preserve114 cores".into());
        }
        let profile = Profile {
            chips: 63,
            cores_per_chip: 114,
            oscillator_hz: 25_000_000,
            initial_baud_divisor: 26,
            identity_reserved: 0x90,
        };
        Ok(Self {
            wire: Wire::with_mining_filter(profile, capacity, vec![cores; 63], slot, filter)
                .map_err(|error| format!("{error:?}"))?,
            traces: VecDeque::new(),
        })
    }

    pub fn reset(&mut self) {
        self.wire.reset();
        self.traces.clear();
    }

    pub fn attach_reference_temperature(&mut self) -> Result<(), String> {
        let reference: serde_json::Value = serde_json::from_str(include_str!(
            "../../../simulator/backends/qemu/s9-tmp421-reference.json"
        ))
        .map_err(|error| error.to_string())?;
        let position = reference["chip_position"]
            .as_u64()
            .ok_or("sensor position absent")?;
        let position = usize::try_from(position).map_err(|error| error.to_string())?;
        let profile =
            serde_json::from_value(reference["bus"].clone()).map_err(|error| error.to_string())?;
        self.wire
            .attach_temperature(position, profile)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn install_temperature_plan(&mut self, bytes: &[u8]) -> Result<(), String> {
        let events = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        self.wire
            .install_temperature_plan(events)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn rxd(&self) -> bool {
        self.wire.tx_high()
    }

    pub fn now_ns(&self) -> u64 {
        self.wire.now_ns()
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.wire.next_deadline()
    }

    pub fn idle_limit(&self, ns: u64, core_ns: u64) -> u64 {
        self.wire
            .next_deadline()
            .map(|at| at.saturating_sub(ns).saturating_sub(1) / core_ns)
            .unwrap_or(u64::MAX)
    }

    pub fn pending(&self) -> bool {
        !self.traces.is_empty()
    }

    pub fn take_trace(&mut self) -> Option<AsicTrace> {
        self.traces.pop_front()
    }

    fn record(&mut self, events: Vec<Trace>) -> Result<(), String> {
        for event in events {
            if self.traces.len() >= 4096 {
                return Err("ASIC trace queue limit; caller must drain events".into());
            }
            let (grant, wall_ns) = match &event {
                Trace::Hash { grant, .. } => (grant.ordinal, grant.elapsed_ns),
                _ => (0, 0),
            };
            let (ns, kind, value, extra) = match event {
                Trace::Serial(Event::Received { ns, byte }) => (ns, 1, u32::from(byte), 0),
                Trace::Serial(Event::Transmitted { ns, byte }) => (ns, 2, u32::from(byte), 0),
                Trace::Serial(Event::Level { ns, high }) => (ns, 3, u32::from(high), 0),
                Trace::Command {
                    ns,
                    opcode,
                    replies,
                } => (ns, 4, u32::from(opcode), replies as u32),
                Trace::Rejected { ns, error } => (ns, 5, error as u32, 0),
                Trace::Serial(Event::FalseStart { ns }) => (ns, 6, 0, 0),
                Trace::Serial(Event::FramingError { ns, byte }) => (ns, 7, u32::from(byte), 0),
                Trace::Hash { ns, attempt, .. } => (
                    ns,
                    10,
                    attempt.nonce,
                    u32::from(attempt.job_id)
                        | (attempt.midstate_index as u32) << 8
                        | (u32::from(attempt.response.is_some()) << 16),
                ),
                Trace::Work {
                    ns,
                    id,
                    enabled_cores,
                } => (ns, 11, u32::from(id), enabled_cores as u32),
                Trace::Stopped { ns } => (ns, 12, 0, 0),
            };
            self.traces.push_back(AsicTrace {
                ns,
                kind,
                value,
                extra,
                grant,
                wall_ns,
            });
        }
        Ok(())
    }

    pub fn advance(&mut self, ns: u64) -> Result<(), String> {
        let result = self.wire.advance(ns);
        self.record_result(result)
    }

    pub fn input(&mut self, ns: u64, high: bool) -> Result<(), String> {
        let result = self.wire.input(ns, high);
        self.record_result(result)
    }

    fn record_result(&mut self, result: Result<Vec<Trace>, Error>) -> Result<(), String> {
        match result {
            Ok(events) => self.record(events),
            Err(error) => {
                let events = self.wire.take_failed_trace();
                self.record(events)?;
                Err(format!("{error:?}"))
            }
        }
    }

    pub fn pin(&mut self, ns: u64, pin: u32, level: bool) -> Result<(), String> {
        if pin > 1 {
            return Err("unknown ASIC pin".into());
        }
        self.advance(ns)?;
        if pin == 0 {
            self.wire
                .power(level)
                .map_err(|error| format!("{error:?}"))?;
        } else {
            self.wire.reset_line(!level);
        }
        if self.traces.len() >= 4096 {
            return Err("ASIC trace queue limit".into());
        }
        self.traces.push_back(AsicTrace {
            ns,
            kind: 8 + pin,
            value: u32::from(level),
            extra: u32::from(self.wire.tx_high()),
            grant: 0,
            wall_ns: 0,
        });
        Ok(())
    }
}

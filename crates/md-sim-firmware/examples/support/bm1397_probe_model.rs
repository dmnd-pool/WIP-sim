//! Probe adapter. Selection is explicit and part of the process environment.
//! The original identification/observer path is retained as a separate mode.
use super::write_intent;
use md_sim_firmware::bm1397::{
    self,
    reference::{FaultPolicy, ReferenceChain, Step},
    ControlWriteSupport, IdentificationChain, Packet,
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub chips: u16,
    pub hash_cap_hps: f64,
    pub queue_slots: usize,
    pub fault_policy: FaultPolicy,
}
impl Config {
    pub fn build(&self) -> Result<ReferenceChain, bm1397::reference::Error> {
        ReferenceChain::with_fault_policy(
            self.chips,
            self.hash_cap_hps,
            self.queue_slots,
            self.fault_policy,
        )
    }
}
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Error {
    Identify(bm1397::IdentifyError),
    Reference(bm1397::reference::Error),
}

pub enum Model {
    Identify {
        chain: IdentificationChain,
        observer: Option<write_intent::Observer>,
    },
    Reference(Box<ReferenceChain>),
}
impl Model {
    pub fn new(
        support: ControlWriteSupport,
        observe: bool,
        reference: Option<&Config>,
    ) -> Result<Self, Error> {
        if let Some(config) = reference {
            if observe || support != ControlWriteSupport::RegisterStorage {
                return Err(Error::Reference(bm1397::reference::Error::InvalidPolicy));
            }
            Ok(Self::Reference(Box::new(
                config.build().map_err(Error::Reference)?,
            )))
        } else {
            Ok(Self::Identify {
                chain: IdentificationChain::with_control_support(1, support)
                    .map_err(Error::Identify)?,
                observer: observe.then(write_intent::Observer::default),
            })
        }
    }
    pub fn power_and_release_reset(&mut self) {
        match self {
            Self::Identify { chain, .. } => {
                chain.power(true);
                chain.reset_line(false);
            }
            Self::Reference(chain) => {
                chain.power(true);
                chain.reset_line(false);
            }
        }
    }
    pub fn receive(&mut self, packet: &Packet) -> Result<write_intent::Reception, Error> {
        match self {
            Self::Identify { chain, observer } => {
                write_intent::receive(chain, observer.as_mut(), packet).map_err(Error::Identify)
            }
            Self::Reference(chain) => Ok(write_intent::Reception {
                replies: chain.receive(packet).map_err(Error::Reference)?,
                intent: None,
            }),
        }
    }
    pub fn programmed_controls(
        &self,
        chip: usize,
    ) -> Option<bm1397::programmed::ProgrammedControls> {
        match self {
            Self::Identify { chain, .. } => chain.programmed_controls(chip),
            Self::Reference(chain) => chain.programmed_controls(chip),
        }
    }
    pub fn poll(&mut self) -> Option<Step> {
        match self {
            Self::Reference(chain) => chain.poll(),
            _ => None,
        }
    }
    pub fn wait_duration(&self) -> Duration {
        match self {
            Self::Reference(chain) if chain.queue_len() > 0 => {
                chain.wait_duration().min(Duration::from_millis(100))
            }
            _ => Duration::from_millis(100),
        }
    }
    pub fn measurement(&mut self) -> Option<bm1397::reference::Measurement> {
        match self {
            Self::Reference(chain) => Some(chain.measurement()),
            _ => None,
        }
    }
}

/// Ordinary receipts are counted from actual poll results. This bounds trace
/// volume without inventing hash counts from elapsed time or configured rate.
/// Real solutions and injected drops are separately retained in full.
#[derive(Default)]
pub struct Batch {
    count: u64,
    first: Option<Instant>,
    previous: Option<Instant>,
    last: Option<Instant>,
    minimum_spacing: Option<Duration>,
    total: u64,
}
impl Batch {
    pub fn record(&mut self, step: &Step) {
        let at = step.computation.admitted_at;
        if let Some(previous) = self.previous {
            let spacing = at.duration_since(previous);
            self.minimum_spacing =
                Some(self.minimum_spacing.map_or(spacing, |old| old.min(spacing)));
        }
        self.previous = Some(at);
        self.last = Some(at);
        self.first.get_or_insert(at);
        self.count += 1;
        self.total += 1;
    }
    pub fn due(&self) -> bool {
        self.count >= 1024
    }
    pub fn take(&mut self, origin: Instant) -> Option<serde_json::Value> {
        if self.count == 0 {
            return None;
        }
        let value = serde_json::json!({"actual_computations":self.count,"cumulative_computations":self.total,
            "first_admission_ns":self.first.unwrap().duration_since(origin).as_nanos(),
            "last_admission_ns":self.last.unwrap().duration_since(origin).as_nanos(),
            "minimum_admission_spacing_ns":self.minimum_spacing.map(|x|x.as_nanos()),
            "scope":"Direct private-engine receipts, including filtered results; no elapsed-time-derived work."});
        self.count = 0;
        self.first = None;
        self.last = None;
        self.minimum_spacing = None;
        // Keep the prior admission so the next batch checks the boundary too.
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configuration_is_strict_bounded_and_excludes_the_observer() {
        let json =
            r#"{"chips":1,"hash_cap_hps":10000.0,"queue_slots":32,"fault_policy":"NoFaults"}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert!(config.build().is_ok());
        assert!(serde_json::from_str::<Config>(
            &json.replace("\"chips\":1", "\"chips\":1,\"hidden\":true")
        )
        .is_err());
        assert!(Model::new(ControlWriteSupport::RegisterStorage, true, Some(&config)).is_err());
        assert!(Model::new(ControlWriteSupport::Unsupported, false, Some(&config)).is_err());
        let mut invalid = config.clone();
        invalid.hash_cap_hps = 10001.0;
        assert!(invalid.build().is_err());
        invalid = config;
        invalid.chips = 2;
        assert!(invalid.build().is_err());
    }
}

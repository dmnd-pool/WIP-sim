//! BM1387 I2C register controller and an explicitly attached TMP421.
//! Packet fields follow the pinned Braiins driver. Clock, reset word,
//! and busy readback are explicit reference policies, not silicon evidence.
use crate::tmp421::{self, Inputs, Sensor, Timing};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Profile,
    TimeReversed,
    TimeOverflow,
    Unpowered,
    BusyWrite,
    RoutingChangeDuringTransfer,
    InvalidCommand,
    NackBehaviorUnknown(u8),
    Sensor(tmp421::Error),
}

impl From<tmp421::Error> for Error {
    fn from(error: tmp421::Error) -> Self {
        Self::Sensor(error)
    }
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub address_7bit: u8,
    pub clock_hz: u32,
    pub reset_word: u32,
    pub timing: Timing,
    pub inputs: Inputs,
}

/// External sensor inputs at absolute virtual time. This is a test scenario,
/// not a calibrated thermal plant. At equal times, conversions and bus
/// completions take effect before this external input edge.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputEvent {
    pub at_ns: u64,
    pub inputs: Inputs,
}

#[derive(Clone, Copy)]
struct Transfer {
    end: u64,
    command: u32,
}

pub struct Controller {
    profile: Profile,
    sensor: Sensor,
    now: u64,
    powered: bool,
    ever_powered: bool,
    register: u32,
    transfer: Option<Transfer>,
    input_plan: Vec<InputEvent>,
    next_input: usize,
}

impl Controller {
    pub fn new(profile: Profile) -> Result<Self, Error> {
        if !(0x4c..=0x4f).contains(&profile.address_7bit)
            || !(1..=3_400_000).contains(&profile.clock_hz)
            || profile.reset_word & 0x8000_0000 != 0
        {
            return Err(Error::Profile);
        }
        Ok(Self {
            sensor: Sensor::new(profile.timing, profile.inputs)?,
            profile,
            now: 0,
            powered: false,
            ever_powered: false,
            register: profile.reset_word,
            transfer: None,
            input_plan: Vec::new(),
            next_input: 0,
        })
    }

    /// Install once before first power. Absolute input events continue while
    /// the sensor is off. A whole timeline reset replays the frozen plan.
    pub fn install_input_plan(&mut self, events: Vec<InputEvent>) -> Result<(), Error> {
        if self.ever_powered
            || self.now != 0
            || !self.input_plan.is_empty()
            || events.is_empty()
            || events.len() > 256
            || events[0].at_ns == 0
            || events.windows(2).any(|pair| pair[0].at_ns >= pair[1].at_ns)
        {
            return Err(Error::Profile);
        }
        self.input_plan = events;
        Ok(())
    }

    pub fn power(&mut self, ns: u64, enabled: bool) -> Result<(), Error> {
        self.advance(ns)?;
        self.sensor.power(ns, enabled)?;
        self.ever_powered |= enabled;
        if self.powered != enabled {
            self.reset_controller();
            self.powered = enabled;
        }
        Ok(())
    }

    /// ASIC reset aborts a pending transaction. It does not reset the
    /// separately powered external sensor.
    pub fn reset_controller(&mut self) {
        self.register = self.profile.reset_word;
        self.transfer = None;
    }

    pub fn pending(&self) -> bool {
        self.transfer.is_some()
    }

    pub fn reset_time_origin(&mut self) {
        let plan = std::mem::take(&mut self.input_plan);
        *self = Self::new(self.profile).expect("validated controller profile");
        self.input_plan = plan;
    }

    pub fn set_inputs(&mut self, ns: u64, inputs: Inputs) -> Result<(), Error> {
        self.advance(ns)?;
        self.sensor.set_inputs(ns, inputs)?;
        Ok(())
    }

    pub fn advance(&mut self, ns: u64) -> Result<(), Error> {
        if ns < self.now {
            return Err(Error::TimeReversed);
        }
        loop {
            let input = self
                .input_plan
                .get(self.next_input)
                .copied()
                .filter(|event| event.at_ns <= ns);
            let transfer = self.transfer.filter(|transfer| transfer.end <= ns);
            if let Some(event) =
                input.filter(|event| transfer.is_none_or(|transfer| event.at_ns < transfer.end))
            {
                self.sensor.set_inputs(event.at_ns, event.inputs)?;
                self.next_input += 1;
                continue;
            }
            let Some(transfer) = transfer else {
                break;
            };
            let [_, address, register, value] = transfer.command.to_be_bytes();
            // This ASIC register reverses the conventional I2C R/W bit.
            // The archived Address helpers explicitly use even=read.
            let result = if address & 1 == 0 {
                self.sensor.read(transfer.end, register)?
            } else {
                self.sensor.write(transfer.end, register, value)?;
                value
            };
            self.register = (transfer.command & 0x7fff_ff00) | u32::from(result);
            self.transfer = None;
        }
        self.sensor.advance(ns)?;
        self.now = ns;
        Ok(())
    }

    pub fn read(&mut self, ns: u64) -> Result<u32, Error> {
        self.advance(ns)?;
        if !self.powered {
            return Err(Error::Unpowered);
        }
        Ok(self.register)
    }

    pub fn write(&mut self, ns: u64, command: u32) -> Result<(), Error> {
        self.advance(ns)?;
        if !self.powered {
            return Err(Error::Unpowered);
        }
        if self.transfer.is_some() {
            return Err(Error::BusyWrite);
        }
        let [flags, address, _, _] = command.to_be_bytes();
        if flags != 1 {
            return Err(Error::InvalidCommand);
        }
        if address >> 1 != self.profile.address_7bit {
            // The archive does not establish BM1387 NACK status behavior.
            // Do not turn an absent slave into a successful 0xff read.
            return Err(Error::NackBehaviorUnknown(address));
        }
        // Write-byte: address, pointer, data, each with ACK, START/STOP.
        // Read-byte: write address, pointer, read address, data, repeated
        // START and final NACK/STOP. One clock slot per condition is an
        // explicit timing policy of this register-level reference model.
        let clocks = if address & 1 != 0 { 29u64 } else { 39u64 };
        let duration = (clocks * 1_000_000_000).div_ceil(u64::from(self.profile.clock_hz));
        let end = ns.checked_add(duration).ok_or(Error::TimeOverflow)?;
        self.register = command | 0x8000_0000;
        self.transfer = Some(Transfer { end, command });
        Ok(())
    }
}

//! Braiins fan register contract and a separately configured motor model.
//! No RPM value is supplied as a successful-read fixture. Tachometer counts
//! come from integrated motor motion. Exact FPGA sampling and analogue
//! calibration are not established; see simulator/backends/qemu/FAN.md.
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const SECOND: u64 = 1_000_000_000;
// Integral of RPM * nanoseconds, with the factor two from trapezoid area.
const REVOLUTION_AREA: u128 = 120 * SECOND as u128 * SECOND as u128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Configuration,
    Access,
    UnsupportedDuty,
    TimeSpan,
}

/// Physical parameters are run inputs, not a claim about a miner family.
#[derive(Clone, Copy, Debug)]
pub struct MotorConfig {
    pub minimum_rpm: u32,
    pub maximum_rpm: u32,
    pub slew_rpm_per_second: u32,
    pub pulses_per_revolution: u32,
    pub pwm_channel: usize,
    pub tach_channel: Option<usize>,
}

/// External motor inputs. Time is absolute model nanoseconds, not a delay
/// measured from the previous input or a guest register access.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputEvent {
    pub at_ns: u64,
    pub motor: usize,
    pub powered: Option<bool>,
    pub stalled: Option<bool>,
}

pub fn validate_input_plan(events: &[InputEvent], motor_count: usize) -> Result<(), Error> {
    if events.is_empty() || events.len() > 32 {
        return Err(Error::Configuration);
    }
    let mut previous = None;
    for e in events {
        let key = (e.at_ns, e.motor);
        if !(1..=3600 * SECOND).contains(&e.at_ns)
            || e.motor >= motor_count
            || (e.powered.is_none() && e.stalled.is_none())
            || previous.is_some_and(|old| old >= key)
        {
            return Err(Error::Configuration);
        }
        previous = Some(key);
    }
    Ok(())
}

struct Motor {
    config: MotorConfig,
    powered: bool,
    stalled: bool,
    // RPM * SECOND. This retains sub-RPM changes without floating point.
    speed: u64,
    pulse_area: u128,
    window_pulses: u64,
    total_pulses: u64,
}

impl Motor {
    fn advance(&mut self, ns: u64, duty: u8) {
        let c = self.config;
        let target = if self.powered && !self.stalled {
            u64::from(c.minimum_rpm) * SECOND
                + u64::from(c.maximum_rpm - c.minimum_rpm) * SECOND * u64::from(duty) / 100
        } else {
            0
        };
        let slope = u64::from(c.slew_rpm_per_second);
        let distance = self.speed.abs_diff(target);
        let next = if self.speed < target {
            self.speed.saturating_add(slope * ns).min(target)
        } else {
            self.speed.saturating_sub(slope * ns).max(target)
        };
        // Keep slope in the denominator to integrate the fractional instant
        // at which a ramp reaches its target. Splitting time into calls must
        // not change pulse counts.
        let area = if slope * ns < distance {
            (u128::from(self.speed) + u128::from(next)) * u128::from(ns) * u128::from(slope)
        } else {
            let steady = 2 * u128::from(target) * u128::from(ns) * u128::from(slope);
            let triangle = u128::from(distance).pow(2);
            if self.speed < target {
                steady - triangle
            } else {
                steady + triangle
            }
        };
        self.speed = next;
        self.pulse_area += area * u128::from(c.pulses_per_revolution);
        let denominator = REVOLUTION_AREA * u128::from(slope);
        let pulses = (self.pulse_area / denominator) as u64;
        self.pulse_area %= denominator;
        self.window_pulses += pulses;
        self.total_pulses = self.total_pulses.saturating_add(pulses);
    }
}

pub struct Controller {
    pwm: [u8; 2],
    tach_rps: [u32; 4],
    motors: Vec<Motor>,
    sample_ns: u64,
    now_ns: u64,
    input_plan: Vec<InputEvent>,
    next_input: usize,
    applied_inputs: VecDeque<InputEvent>,
}

impl Controller {
    pub fn new(configs: &[MotorConfig]) -> Result<Self, Error> {
        if configs.len() > 4 {
            return Err(Error::Configuration);
        }
        let mut occupied = [false; 4];
        for c in configs {
            if c.minimum_rpm > c.maximum_rpm
                || c.maximum_rpm > 1_000_000
                || !(1..=1_000_000).contains(&c.slew_rpm_per_second)
                || !(1..=64).contains(&c.pulses_per_revolution)
                || c.pwm_channel >= 2
            {
                return Err(Error::Configuration);
            }
            if let Some(channel) = c.tach_channel {
                if channel >= 4 || occupied[channel] {
                    return Err(Error::Configuration);
                }
                occupied[channel] = true;
            }
        }
        Ok(Self {
            pwm: [0; 2],
            tach_rps: [0; 4],
            motors: configs
                .iter()
                .map(|&config| Motor {
                    config,
                    powered: true,
                    stalled: false,
                    speed: 0,
                    pulse_area: 0,
                    window_pulses: 0,
                    total_pulses: 0,
                })
                .collect(),
            sample_ns: 0,
            now_ns: 0,
            input_plan: Vec::new(),
            next_input: 0,
            applied_inputs: VecDeque::new(),
        })
    }

    /// Install once, before time advances. Reset does not replay external faults.
    pub fn install_input_plan(&mut self, events: Vec<InputEvent>) -> Result<(), Error> {
        if self.now_ns != 0 || !self.input_plan.is_empty() {
            return Err(Error::Configuration);
        }
        validate_input_plan(&events, self.motors.len())?;
        self.input_plan = events;
        Ok(())
    }

    pub fn take_applied_input(&mut self) -> Option<InputEvent> {
        self.applied_inputs.pop_front()
    }

    /// Reset the controller, retaining the external motors and their motion.
    /// The one-second measurement window is a declared model parameter.
    pub fn reset(&mut self) {
        self.pwm = [0; 2];
        self.tach_rps = [0; 4];
        self.sample_ns = 0;
        for motor in &mut self.motors {
            motor.window_pulses = 0;
        }
    }

    pub fn set_powered(&mut self, motor: usize, powered: bool) -> Result<(), Error> {
        self.motors
            .get_mut(motor)
            .ok_or(Error::Configuration)?
            .powered = powered;
        Ok(())
    }

    /// A locked rotor stops immediately. Losing power instead coasts down.
    pub fn set_stalled(&mut self, motor: usize, stalled: bool) -> Result<(), Error> {
        let motor = self.motors.get_mut(motor).ok_or(Error::Configuration)?;
        motor.stalled = stalled;
        if stalled {
            motor.speed = 0;
        }
        Ok(())
    }

    pub fn motor_rpm(&self, motor: usize) -> Result<u32, Error> {
        Ok((self.motors.get(motor).ok_or(Error::Configuration)?.speed / SECOND) as u32)
    }

    pub fn total_pulses(&self, motor: usize) -> Result<u64, Error> {
        Ok(self
            .motors
            .get(motor)
            .ok_or(Error::Configuration)?
            .total_pulses)
    }

    /// Bound work per call. Use `advance_elapsed` for a late backend callback.
    /// Neither entry point rescales guest time.
    pub fn advance(&mut self, mut ns: u64) -> Result<(), Error> {
        if ns > 60 * SECOND || self.now_ns.checked_add(ns).is_none() {
            return Err(Error::TimeSpan);
        }
        while ns > 0 {
            let step = ns.min(self.next_event_ns());
            for motor in &mut self.motors {
                motor.advance(step, self.pwm[motor.config.pwm_channel]);
            }
            self.sample_ns += step;
            self.now_ns += step;
            ns -= step;
            if self.sample_ns == SECOND {
                self.tach_rps = [0; 4];
                for motor in &mut self.motors {
                    if let Some(channel) = motor.config.tach_channel {
                        self.tach_rps[channel] = (motor.window_pulses
                            / u64::from(motor.config.pulses_per_revolution))
                            as u32;
                    }
                    motor.window_pulses = 0;
                }
                self.sample_ns = 0;
            }
            // The preceding interval and a sample at the same instant finish
            // before external inputs change. Equal-time motors use index order.
            while let Some(&event) = self.input_plan.get(self.next_input) {
                if event.at_ns != self.now_ns {
                    break;
                }
                if let Some(powered) = event.powered {
                    self.set_powered(event.motor, powered)?;
                }
                if let Some(stalled) = event.stalled {
                    self.set_stalled(event.motor, stalled)?;
                }
                self.applied_inputs.push_back(event);
                self.next_input += 1;
            }
        }
        Ok(())
    }

    /// Catch up after delayed timer service. Keep every elapsed nanosecond,
    /// motor transition and sample boundary. Cost grows with elapsed time.
    pub fn advance_elapsed(&mut self, mut ns: u64) -> Result<(), Error> {
        if self.now_ns.checked_add(ns).is_none() {
            return Err(Error::TimeSpan);
        }
        while ns > 0 {
            let step = ns.min(60 * SECOND);
            self.advance(step)?;
            ns -= step;
        }
        Ok(())
    }

    pub fn next_sample_ns(&self) -> u64 {
        SECOND - self.sample_ns
    }

    pub fn next_event_ns(&self) -> u64 {
        self.input_plan
            .get(self.next_input)
            .map_or(self.next_sample_ns(), |e| {
                self.next_sample_ns().min(e.at_ns - self.now_ns)
            })
    }

    pub fn read(&self, address: u32, size: u32) -> Result<u32, Error> {
        let (word, shift, mask) = access(address, size)?;
        let value = match word {
            0 | 4 | 8 | 12 => self.tach_rps[(word / 4) as usize],
            16 | 20 => u32::from(self.pwm[((word - 16) / 4) as usize]),
            _ => return Err(Error::Access),
        };
        Ok((value >> shift) & mask)
    }

    pub fn write(&mut self, address: u32, size: u32, value: u32) -> Result<(), Error> {
        let (word, shift, _) = access(address, size)?;
        match word {
            0 | 4 | 8 | 12 => Ok(()), // Read-only tachometer registers.
            16 | 20 => {
                if shift == 0 {
                    // The archived SVD describes an eight-bit PWM register.
                    let value = value as u8;
                    if value > 100 {
                        return Err(Error::UnsupportedDuty);
                    }
                    self.pwm[((word - 16) / 4) as usize] = value;
                }
                Ok(())
            }
            _ => Err(Error::Access),
        }
    }
}

fn access(address: u32, size: u32) -> Result<(u32, u32, u32), Error> {
    if !matches!(size, 1 | 2 | 4) || address % size != 0 {
        return Err(Error::Access);
    }
    Ok((address & !3, (address & 3) * 8, u32::MAX >> (32 - size * 8)))
}

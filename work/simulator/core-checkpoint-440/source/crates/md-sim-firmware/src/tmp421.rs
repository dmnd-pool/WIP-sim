//! TMP421 register and conversion state. See simulator/backends/qemu/TMP421.md.
//! The caller supplies physical inputs and virtual time. This is not a
//! calibrated thermal plant or a pin-level SMBus implementation.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Timing,
    TimeReversed,
    TimeOverflow,
    EventLimit,
    Unpowered,
    UnknownRegister(u8),
    ReadOnly(u8),
    UndefinedRead(u8),
    ReservedBits(u8),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Timing {
    pub power_start_ns: u64,
    pub conversion_ns: u64,
    pub channel_slot_ns: u64,
}

impl Timing {
    /// Explicit development timing, not measured timing of a physical board.
    pub const REFERENCE: Self = Self {
        power_start_ns: 50_000,
        conversion_ns: 115_000_000,
        channel_slot_ns: 125_000_000,
    };

    fn validate(self) -> Result<Self, Error> {
        if self.power_start_ns == 0
            || self.power_start_ns >= 100_000
            || !(100_000_000..=130_000_000).contains(&self.conversion_ns)
            || self.channel_slot_ns < self.conversion_ns
        {
            return Err(Error::Timing);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Diode {
    Connected,
    Open,
    Short,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inputs {
    pub local_millicelsius: i32,
    pub remote_millicelsius: i32,
    pub diode: Diode,
    /// False injects the ADC supply-valid fault, not a complete power loss.
    pub supply_valid: bool,
}

#[derive(Clone, Copy)]
enum Event {
    Sequence,
    Start(u8),
    Finish {
        channel: u8,
        slot_end: u64,
        extended: bool,
        correction: i8,
        invalid: bool,
        open: bool,
    },
}

#[derive(Clone)]
pub struct Sensor {
    timing: Timing,
    inputs: Inputs,
    now: u64,
    powered: bool,
    temperatures: [[u8; 2]; 2],
    config1: u8,
    config2: u8,
    rate: u8,
    correction: u8,
    latch: Option<(u8, u8)>,
    event: Option<(u64, Event)>,
    sequence_start: u64,
    sequence_channels: u8,
}

fn add(a: u64, b: u64) -> Result<u64, Error> {
    a.checked_add(b).ok_or(Error::TimeOverflow)
}

impl Sensor {
    pub fn new(timing: Timing, inputs: Inputs) -> Result<Self, Error> {
        Ok(Self {
            timing: timing.validate()?,
            inputs,
            now: 0,
            powered: false,
            temperatures: [[0; 2]; 2],
            config1: 0,
            config2: 0x1c,
            rate: 7,
            correction: 0,
            latch: None,
            event: None,
            sequence_start: 0,
            sequence_channels: 0,
        })
    }

    pub fn power(&mut self, ns: u64, enabled: bool) -> Result<(), Error> {
        self.advance(ns)?;
        if self.powered != enabled {
            self.powered = enabled;
            self.reset_registers()?;
        }
        Ok(())
    }

    fn reset_registers(&mut self) -> Result<(), Error> {
        self.temperatures = [[0; 2]; 2];
        self.config1 = 0;
        self.config2 = 0x1c;
        self.rate = 7;
        self.correction = 0;
        self.latch = None;
        self.sequence_channels = 0;
        self.event = if self.powered {
            Some((add(self.now, self.timing.power_start_ns)?, Event::Sequence))
        } else {
            None
        };
        Ok(())
    }

    pub fn set_inputs(&mut self, ns: u64, inputs: Inputs) -> Result<(), Error> {
        self.advance(ns)?;
        self.inputs = inputs;
        // A transient fault during an ADC conversion must not disappear
        // merely because the input recovers before the next register read.
        if let Some((
            _,
            Event::Finish {
                channel,
                invalid,
                open,
                ..
            },
        )) = &mut self.event
        {
            *invalid |= !inputs.supply_valid;
            *open |= *channel == 1 && inputs.diode == Diode::Open;
        }
        Ok(())
    }

    pub fn next_deadline(&self) -> Option<u64> {
        self.event.map(|(at, _)| at)
    }

    pub fn advance(&mut self, ns: u64) -> Result<(), Error> {
        if ns < self.now {
            return Err(Error::TimeReversed);
        }
        let mut events = 0;
        while let Some((at, event)) = self.event.filter(|(at, _)| *at <= ns) {
            if events == 100_000 {
                return Err(Error::EventLimit);
            }
            events += 1;
            self.now = at;
            self.event = None;
            match event {
                Event::Sequence => self.start_sequence()?,
                Event::Start(channel) => self.start_channel(channel)?,
                Event::Finish {
                    channel,
                    slot_end,
                    extended,
                    correction,
                    invalid,
                    open,
                } => {
                    self.temperatures[usize::from(channel)] =
                        self.convert(channel, extended, correction, invalid, open);
                    if channel == 0 && self.sequence_channels & 2 != 0 {
                        self.event = Some((slot_end, Event::Start(1)));
                    } else {
                        self.sequence_channels = 0;
                        if self.config1 & 0x40 == 0 {
                            let period = 16_000_000_000u64 >> self.rate;
                            self.event = Some((
                                slot_end.max(add(self.sequence_start, period)?),
                                Event::Sequence,
                            ));
                        }
                    }
                }
            }
        }
        self.now = ns;
        Ok(())
    }

    fn start_sequence(&mut self) -> Result<(), Error> {
        self.sequence_start = self.now;
        self.sequence_channels = self.config2 >> 3 & 3;
        match self.sequence_channels {
            0 => Ok(()),
            2 => self.start_channel(1),
            _ => self.start_channel(0),
        }
    }

    fn start_channel(&mut self, channel: u8) -> Result<(), Error> {
        // The data sheet gives an approximate 50% reduction with RC off.
        // This exact half-time is a declared development policy.
        let divisor = if self.config2 & 4 == 0 { 2 } else { 1 };
        self.event = Some((
            add(self.now, self.timing.conversion_ns / divisor)?,
            Event::Finish {
                channel,
                slot_end: add(self.now, self.timing.channel_slot_ns / divisor)?,
                extended: self.config1 & 4 != 0,
                correction: self.correction as i8,
                invalid: !self.inputs.supply_valid,
                open: channel == 1 && self.inputs.diode == Diode::Open,
            },
        ));
        Ok(())
    }

    fn convert(
        &self,
        channel: u8,
        extended: bool,
        correction: i8,
        invalid: bool,
        open: bool,
    ) -> [u8; 2] {
        let mut milli = i64::from(if channel == 0 {
            self.inputs.local_millicelsius
        } else {
            self.inputs.remote_millicelsius
        });
        if channel == 1 {
            // Inputs describe an ideal 1.008 diode. The programmed n-factor
            // changes its conversion scale in kelvin, not in degrees C.
            milli = (milli + 273_150) * (300 - i64::from(correction)) / 300 - 273_150;
            if self.inputs.diode == Diode::Short {
                milli = -64_000;
            }
        }
        let milli = milli.clamp(-64_000, if extended { 191_000 } else { 127_000 });
        // Exact 1/16 C truncation toward negative infinity. This preserves
        // negative fractions in the two's-complement register format.
        let units = (milli * 16).div_euclid(1000) + if extended { 1024 } else { 0 };
        [
            units.div_euclid(16) as u8,
            (units.rem_euclid(16) as u8) << 4 | u8::from(invalid) << 1 | u8::from(open),
        ]
    }

    pub fn read(&mut self, ns: u64, register: u8) -> Result<u8, Error> {
        self.advance(ns)?;
        if !self.powered {
            return Err(Error::Unpowered);
        }
        let latch = self.latch.take();
        match register {
            0..=1 => {
                let [high, low] = self.temperatures[usize::from(register)];
                self.latch = Some((register, low));
                Ok(high)
            }
            0x10..=0x11 => {
                let channel = register - 0x10;
                Ok(latch
                    .filter(|(previous, _)| *previous == channel)
                    .map(|(_, low)| low)
                    .unwrap_or(self.temperatures[usize::from(channel)][1]))
            }
            8 => Ok(if matches!(self.event, Some((_, Event::Finish { .. }))) {
                0x80
            } else {
                0
            }),
            9 => Ok(self.config1),
            10 => Ok(self.config2),
            11 => Ok(self.rate),
            0x21 => Ok(self.correction),
            0xfe => Ok(0x55),
            0xff => Ok(0x21),
            0x0f | 0xfc => Err(Error::UndefinedRead(register)),
            _ => Err(Error::UnknownRegister(register)),
        }
    }

    pub fn write(&mut self, ns: u64, register: u8, value: u8) -> Result<(), Error> {
        self.advance(ns)?;
        if !self.powered {
            return Err(Error::Unpowered);
        }
        match register {
            9 if value & !0x44 == 0 => {
                let was_shutdown = self.config1 & 0x40 != 0;
                self.config1 = value;
                if value & 0x40 != 0 && self.sequence_channels == 0 {
                    self.event = None;
                } else if was_shutdown && value & 0x40 == 0 && self.event.is_none() {
                    self.start_sequence()?;
                }
            }
            10 if value & !0x1c == 0 => {
                self.config2 = value;
                if self.event.is_none() && self.config1 & 0x40 == 0 {
                    self.start_sequence()?;
                }
            }
            11 if value <= 7 => self.rate = value,
            9..=11 => return Err(Error::ReservedBits(register)),
            0x0f => {
                if self.sequence_channels == 0 {
                    self.event = None;
                    self.start_sequence()?;
                }
            }
            0x21 => self.correction = value,
            0xfc => self.reset_registers()?,
            0..=1 | 8 | 0x10..=0x11 | 0xfe..=0xff => return Err(Error::ReadOnly(register)),
            _ => return Err(Error::UnknownRegister(register)),
        }
        Ok(())
    }
}

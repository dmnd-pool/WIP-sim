//! BM1387 powered register chain. Contract from archived Braiins driver
//! 74b80e7a6d862e49, not a calibrated silicon model. Unknown reset values
//! and operational states fail explicitly.
use super::{
    budget::Slot,
    filter::Policy,
    i2c, nonce, register_response,
    search::{Attempt, Search},
    Command, Job, Packet,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Topology,
    InvalidControl,
    AmbiguousAddress,
    UnknownRegister(u8),
    UnknownResetValue(u8),
    InvalidRegister(u8),
    PllStatusUnknown,
    PinRoutingUnsupported,
    DifferentSerialRates,
    WorkNotConfigured,
    Work(super::Error),
    CoreMap(nonce::Error),
    CoreStateUnknown,
    GateChangeDuringWork,
    I2c(i2c::Error),
    TimeReversed,
}

/// Explicit development inputs. Reserved identity bits are a fixture
/// choice; they are not inferred from the name of a firmware image.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Profile {
    pub chips: u16,
    pub cores_per_chip: u16,
    pub oscillator_hz: u32,
    pub initial_baud_divisor: u8,
    pub identity_reserved: u8,
}

#[derive(Clone, Default)]
struct Chip {
    address: u8,
    pll: Option<u32>,
    ticket: Option<u32>,
    misc: Option<u32>,
    open_cores: Option<usize>,
}

#[derive(Default, Debug, PartialEq, Eq)]
pub struct Effect {
    pub replies: Vec<[u8; 7]>,
    pub serial_period_ns: Option<u64>,
    pub work: Option<(u8, usize)>,
}

pub struct Chain {
    profile: Profile,
    chips: Vec<Chip>,
    next_address: Option<usize>,
    powered: bool,
    reset_asserted: bool,
    core_maps: Option<Vec<Vec<u8>>>,
    search: Option<Search>,
    search_chips: Vec<usize>,
    hashes: u64,
    filter_policy: Policy,
    now: u64,
    temperature: Option<(usize, i2c::Controller)>,
}

impl Chain {
    pub fn new(profile: Profile) -> Result<Self, Error> {
        if !(1..=64).contains(&profile.chips)
            || !(1..=128).contains(&profile.cores_per_chip)
            || profile.oscillator_hz == 0
            || profile.initial_baud_divisor > 26
        {
            return Err(Error::Topology);
        }
        Self::period(profile, profile.initial_baud_divisor)?;
        Ok(Self {
            profile,
            chips: vec![Chip::default(); usize::from(profile.chips)],
            next_address: None,
            powered: false,
            reset_asserted: true,
            core_maps: None,
            search: None,
            search_chips: Vec::new(),
            hashes: 0,
            filter_policy: Policy::default(),
            now: 0,
            temperature: None,
        })
    }

    /// Core codes are supplied explicitly by a resolved profile or a
    /// labelled reference fixture. Counts alone never infer a core map.
    pub fn with_mining(profile: Profile, core_maps: Vec<Vec<u8>>) -> Result<Self, Error> {
        Self::with_mining_filter(profile, core_maps, Policy::default())
    }

    pub fn with_mining_filter(
        profile: Profile,
        core_maps: Vec<Vec<u8>>,
        filter_policy: Policy,
    ) -> Result<Self, Error> {
        let mut chain = Self::new(profile)?;
        chain.filter_policy = filter_policy;
        if core_maps.len() != usize::from(profile.chips)
            || core_maps
                .iter()
                .any(|map| map.len() != usize::from(profile.cores_per_chip))
        {
            return Err(Error::Topology);
        }
        nonce::Layout::new(
            core_maps
                .iter()
                .enumerate()
                .map(|(index, cores)| nonce::Chip {
                    address: (index * 4) as u8,
                    cores: cores.clone(),
                })
                .collect(),
        )
        .map_err(Error::CoreMap)?;
        chain.core_maps = Some(core_maps);
        Ok(chain)
    }

    fn period(profile: Profile, divisor: u8) -> Result<u64, Error> {
        let numerator = 8_000_000_000 * (u64::from(divisor) + 1);
        let oscillator = u64::from(profile.oscillator_hz);
        if numerator % oscillator != 0 {
            return Err(Error::Topology);
        }
        let period = numerator / oscillator;
        if period < 2 || period % 2 != 0 {
            return Err(Error::Topology);
        }
        Ok(period)
    }

    pub fn initial_serial_period_ns(&self) -> u64 {
        Self::period(self.profile, self.profile.initial_baud_divisor)
            .expect("validated clock profile")
    }

    pub fn running(&self) -> bool {
        self.powered && !self.reset_asserted
    }

    /// Attach only an explicit board/profile input, never infer a sensor
    /// from a chip count or a firmware filename.
    pub fn attach_temperature(
        &mut self,
        position: usize,
        profile: i2c::Profile,
    ) -> Result<(), Error> {
        if self.powered || self.temperature.is_some() || position >= self.chips.len() {
            return Err(Error::Topology);
        }
        self.temperature = Some((position, i2c::Controller::new(profile).map_err(Error::I2c)?));
        Ok(())
    }

    pub fn install_temperature_plan(&mut self, events: Vec<i2c::InputEvent>) -> Result<(), Error> {
        if self.powered || self.now != 0 {
            return Err(Error::Topology);
        }
        self.temperature
            .as_mut()
            .ok_or(Error::Topology)?
            .1
            .install_input_plan(events)
            .map_err(Error::I2c)
    }

    pub fn advance(&mut self, ns: u64) -> Result<(), Error> {
        if ns < self.now {
            return Err(Error::TimeReversed);
        }
        if let Some((_, controller)) = &mut self.temperature {
            controller.advance(ns).map_err(Error::I2c)?;
        }
        self.now = ns;
        Ok(())
    }

    pub fn power(&mut self, enabled: bool) -> Result<(), Error> {
        if let Some((_, controller)) = &mut self.temperature {
            controller.power(self.now, enabled).map_err(Error::I2c)?;
        }
        if !enabled {
            self.reset_registers();
        }
        self.powered = enabled;
        Ok(())
    }

    pub fn reset_line(&mut self, asserted: bool) {
        if asserted {
            self.reset_registers();
        }
        self.reset_asserted = asserted;
    }

    fn reset_registers(&mut self) {
        self.chips.fill(Chip::default());
        self.next_address = None;
        self.search = None;
        self.search_chips.clear();
        if let Some((_, controller)) = &mut self.temperature {
            controller.reset_controller();
        }
    }

    pub fn reset_time_origin(&mut self) {
        self.reset_registers();
        self.now = 0;
        self.powered = false;
        self.reset_asserted = true;
        if let Some((_, controller)) = &mut self.temperature {
            controller.reset_time_origin();
        }
    }

    pub fn active_cores(&self) -> usize {
        if !self.running() || self.search.as_ref().is_none_or(Search::exhausted) {
            0
        } else {
            self.search_chips
                .iter()
                .map(|&index| self.chips[index].open_cores.unwrap_or(0))
                .sum()
        }
    }

    pub fn hashes_computed(&self) -> u64 {
        self.hashes
    }

    /// The caller owns virtual polling time. The shared slot enforces
    /// the run's wall-clock cap and enabled-core scheduling weights.
    pub fn poll(&mut self, slot: &Slot) -> Result<Option<(super::budget::Grant, Attempt)>, Error> {
        let active = self.active_cores();
        slot.set_weight(active);
        if active == 0 {
            return Ok(None);
        }
        let Some(grant) = slot.try_acquire() else {
            return Ok(None);
        };
        let mut attempt = self
            .search
            .as_mut()
            .unwrap()
            .compute()
            .map_err(Error::Work)?;
        attempt.chain_position = self.search_chips[attempt.chain_position];
        self.hashes += 1;
        slot.set_weight(self.active_cores());
        Ok(Some((grant, attempt)))
    }

    fn work(&mut self, job: &Job) -> Result<Effect, Error> {
        job.validate().map_err(Error::Work)?;
        let maps = self.core_maps.as_ref().ok_or(Error::WorkNotConfigured)?;
        let mut counts = Vec::new();
        let mut chips = Vec::new();
        let mut indices = Vec::new();
        for (index, chip) in self.chips.iter().enumerate() {
            let misc = chip.misc.ok_or(Error::UnknownResetValue(0x1c))?;
            let mut open = chip.open_cores.ok_or(Error::CoreStateUnknown)?;
            // Archived driver: gate_block permits one block to open for
            // each job whose nbits bit0 is set. Preserve the actual payload;
            // null work is not treated as a fabricated successful result.
            if misc & (1 << 15) != 0 && job.tail[8] & 1 != 0 {
                open = (open + 1).min(maps[index].len());
            }
            counts.push(open);
            if open == 0 {
                continue;
            }
            chip.pll.ok_or(Error::UnknownResetValue(0x0c))?;
            chip.ticket.ok_or(Error::UnknownResetValue(0x18))?;
            if misc & (1 << 7) == 0 && job.states.len() > 1 {
                return Err(Error::InvalidRegister(0x1c));
            }
            indices.push(index);
            chips.push(nonce::Chip {
                address: chip.address,
                cores: maps[index][..open].to_vec(),
            });
        }
        let search = if chips.is_empty() {
            None
        } else {
            let layout = nonce::Layout::new(chips).map_err(Error::CoreMap)?;
            let end = layout.candidate_count() - 1;
            let mut search = Search::new(job.clone(), layout, 0, 0..=end).map_err(Error::Work)?;
            search.set_filter_policy(self.filter_policy);
            for (position, &index) in indices.iter().enumerate() {
                search
                    .set_chip_ticket(position, self.chips[index].ticket.unwrap())
                    .map_err(Error::Work)?;
            }
            Some(search)
        };
        for (chip, count) in self.chips.iter_mut().zip(counts) {
            chip.open_cores = Some(count);
        }
        self.search = search;
        self.search_chips = indices;
        Ok(Effect {
            work: Some((job.id, self.active_cores())),
            ..Effect::default()
        })
    }

    fn selected(&self, broadcast: bool, address: u8) -> Result<Vec<usize>, Error> {
        let indices: Vec<_> = self
            .chips
            .iter()
            .enumerate()
            .filter_map(|(index, chip)| (broadcast || chip.address == address).then_some(index))
            .collect();
        if !broadcast && indices.len() > 1 {
            return Err(Error::AmbiguousAddress);
        }
        Ok(indices)
    }

    fn read(&mut self, index: usize, register: u8) -> Result<u32, Error> {
        if register == 0x20 {
            let now = self.now;
            return self.i2c_controller(index)?.read(now).map_err(Error::I2c);
        }
        let chip = &self.chips[index];
        match register {
            0 => Ok(0x1387_0000
                | u32::from(self.profile.identity_reserved) << 8
                | u32::from(chip.address)),
            0x18 => chip.ticket.ok_or(Error::UnknownResetValue(register)),
            0x1c => chip.misc.ok_or(Error::UnknownResetValue(register)),
            // Archived driver notes bit31 on PLL readback, but does not
            // establish its meaning or lock timing. Do not manufacture it.
            0x0c => Err(Error::PllStatusUnknown),
            _ => Err(Error::UnknownRegister(register)),
        }
    }

    fn validate_write(&self, register: u8, value: u32) -> Result<Option<u64>, Error> {
        match register {
            0x18 => Ok(None),
            0x0c => {
                let fb = value >> 16 & 255;
                let reference = value >> 8 & 15;
                let post1 = value >> 4 & 15;
                let post2 = value & 15;
                if value & !0x00ff_0f77 != 0
                    || fb == 0
                    || reference == 0
                    || post1 == 0
                    || post2 == 0
                    || post2 > post1
                {
                    return Err(Error::InvalidRegister(register));
                }
                Ok(None)
            }
            0x1c => {
                if value & !0x4021_dfe0 != 0 || (value >> 8 & 31) > 26 {
                    return Err(Error::InvalidRegister(register));
                }
                Self::period(self.profile, (value >> 8 & 31) as u8).map(Some)
            }
            _ => Err(Error::UnknownRegister(register)),
        }
    }

    fn i2c_controller(&mut self, index: usize) -> Result<&mut i2c::Controller, Error> {
        if self.chips[index]
            .misc
            .is_none_or(|value| value & 0x1_4060 != 0x4060)
        {
            return Err(Error::PinRoutingUnsupported);
        }
        self.temperature
            .as_mut()
            .filter(|(position, _)| *position == index)
            .map(|(_, controller)| controller)
            .ok_or(Error::UnknownRegister(0x20))
    }

    pub fn receive(&mut self, packet: &Packet) -> Result<Effect, Error> {
        if !self.running() {
            return Ok(Effect::default());
        }
        let Packet::Command { broadcast, command } = packet else {
            let Packet::Job(job) = packet else {
                unreachable!()
            };
            return self.work(job);
        };
        match command {
            Command::Inactive {
                address: 0,
                reserved: 0,
            } if *broadcast => {
                self.search = None;
                self.search_chips.clear();
                self.next_address = Some(0);
                Ok(Effect::default())
            }
            Command::SetAddress {
                address,
                reserved: 0,
            } if !broadcast && address % 4 == 0 => {
                let index = self.next_address.ok_or(Error::InvalidControl)?;
                if let Some(chip) = self.chips.get_mut(index) {
                    chip.address = *address;
                    self.next_address = Some(index + 1);
                }
                Ok(Effect::default())
            }
            Command::Inactive { .. } | Command::SetAddress { .. } => Err(Error::InvalidControl),
            Command::Read { address, register } => {
                let replies = self
                    .selected(*broadcast, *address)?
                    .into_iter()
                    .map(|index| self.read(index, *register).map(register_response))
                    .collect::<Result<_, _>>()?;
                Ok(Effect {
                    replies,
                    ..Effect::default()
                })
            }
            Command::Write {
                address,
                register,
                value,
            } => {
                let indices = self.selected(*broadcast, *address)?;
                if indices.is_empty() {
                    return Ok(Effect::default());
                }
                if *register == 0x20 {
                    if self.temperature.is_none() || indices.len() != 1 {
                        return Err(Error::UnknownRegister(0x20));
                    }
                    let now = self.now;
                    self.i2c_controller(indices[0])?
                        .write(now, *value)
                        .map_err(Error::I2c)?;
                    return Ok(Effect::default());
                }
                let rate = self.validate_write(*register, *value)?;
                if *register == 0x1c
                    && value & 0x1_4060 != 0
                    && (value & 0x1_4060 != 0x4060
                        || indices.len() != 1
                        || self
                            .temperature
                            .as_ref()
                            .is_none_or(|(position, _)| *position != indices[0]))
                {
                    return Err(Error::PinRoutingUnsupported);
                }
                if *register == 0x1c {
                    if let Some((position, controller)) = &self.temperature {
                        if indices.contains(position)
                            && controller.pending()
                            && self.chips[*position]
                                .misc
                                .is_some_and(|old| (old ^ value) & 0x1_4060 != 0)
                        {
                            return Err(Error::I2c(i2c::Error::RoutingChangeDuringTransfer));
                        }
                    }
                }
                // Equal-rate unicast writes can change other control bits.
                // Divergent rates need per-hop UARTs and remain an error.
                if let Some(period) = rate {
                    for (index, chip) in self.chips.iter().enumerate() {
                        if !indices.contains(&index) {
                            let divisor = chip
                                .misc
                                .map(|word| (word >> 8 & 31) as u8)
                                .unwrap_or(self.profile.initial_baud_divisor);
                            if Self::period(self.profile, divisor)? != period {
                                return Err(Error::DifferentSerialRates);
                            }
                        }
                    }
                }
                if *register == 0x1c && self.search.is_some() {
                    for &index in &indices {
                        let chip = &self.chips[index];
                        let gate_was = chip.misc.is_some_and(|word| word & (1 << 15) != 0);
                        let gate = value & (1 << 15) != 0;
                        if (!gate_was && gate)
                            || (gate_was
                                && !gate
                                && chip.open_cores
                                    != Some(usize::from(self.profile.cores_per_chip)))
                        {
                            return Err(Error::GateChangeDuringWork);
                        }
                    }
                }
                for index in indices {
                    let chip = &mut self.chips[index];
                    match register {
                        0x0c => chip.pll = Some(*value),
                        0x18 => {
                            chip.ticket = Some(*value);
                            if let Some(position) =
                                self.search_chips.iter().position(|&i| i == index)
                            {
                                self.search
                                    .as_mut()
                                    .unwrap()
                                    .set_chip_ticket(position, *value)
                                    .map_err(Error::Work)?;
                            }
                        }
                        0x1c => {
                            let was_gated = chip.misc.is_some_and(|word| word & (1 << 15) != 0);
                            if value & (1 << 15) == 0 {
                                chip.open_cores = Some(usize::from(self.profile.cores_per_chip));
                            } else if !was_gated {
                                chip.open_cores = Some(0);
                            }
                            chip.misc = Some(*value);
                        }
                        _ => unreachable!("validated register"),
                    }
                }
                Ok(Effect {
                    replies: Vec::new(),
                    serial_period_ns: rate,
                    work: None,
                })
            }
        }
    }
}

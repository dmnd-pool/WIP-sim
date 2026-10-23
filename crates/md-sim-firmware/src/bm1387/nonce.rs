//! BM1387 nonce ownership from the archived Braiins CoreAddress decoder.
//! Bits7:2 select the chip address/4; bits30:24 select its core.
//! The other19bits form the search space of that core. Enumeration order
//! is a simulator scheduling policy, not a claim about silicon scan order.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Chips,
    Cores,
    Address,
    DuplicateAddress,
    CoreAddress,
    DuplicateCore,
}

/// Core codes are explicit: the archived driver says the114 physical
/// cores have non-consecutive addresses within a128-address space.
/// A core count alone cannot establish which nonce ranges exist.
pub struct Chip {
    pub address: u8,
    pub cores: Vec<u8>,
}

/// A complete, non-overlapping nonce space for explicit chip/core maps.
/// Missing chips and cores remain holes. Maps may differ between chips.
pub struct Layout {
    chips: Vec<Chip>,
    slots: Vec<(usize, u8)>,
}

impl Layout {
    pub fn chip_count(&self) -> usize {
        self.chips.len()
    }

    pub fn new(chips: Vec<Chip>) -> Result<Self, Error> {
        if chips.is_empty() || chips.len() > 64 {
            return Err(Error::Chips);
        }
        let mut occupied = [false; 64];
        for chip in &chips {
            if chip.address % 4 != 0 {
                return Err(Error::Address);
            }
            let slot = &mut occupied[usize::from(chip.address / 4)];
            if *slot {
                return Err(Error::DuplicateAddress);
            }
            *slot = true;
            if chip.cores.is_empty() || chip.cores.len() > 128 {
                return Err(Error::Cores);
            }
            let mut cores = [false; 128];
            for &core in &chip.cores {
                let slot = cores.get_mut(usize::from(core)).ok_or(Error::CoreAddress)?;
                if *slot {
                    return Err(Error::DuplicateCore);
                }
                *slot = true;
            }
        }
        let mut slots = Vec::new();
        for position in 0..128 {
            for (index, chip) in chips.iter().enumerate() {
                if let Some(&core) = chip.cores.get(position) {
                    slots.push((index, core));
                }
            }
        }
        Ok(Self { chips, slots })
    }

    pub fn candidate_count(&self) -> u64 {
        self.slots.len() as u64 * (1 << 19)
    }

    /// Visit each chip first, then each core, then the remaining nonce bits.
    /// No nonce or hash is returned without an explicit ordinal in range.
    pub fn at(&self, ordinal: u64) -> Option<u32> {
        if ordinal >= self.candidate_count() {
            return None;
        }
        let (chip, core) = self.slots[(ordinal % self.slots.len() as u64) as usize];
        let free = (ordinal / self.slots.len() as u64) as u32;
        Some(
            u32::from(self.chips[chip].address)
                | u32::from(core) << 24
                | (free & 3)
                | ((free >> 2) & 0xffff) << 8
                | (free >> 18) << 31,
        )
    }

    /// Return the chain position and core address for an owned nonce.
    pub fn owner(&self, nonce: u32) -> Option<(usize, u8)> {
        let address = (nonce & 0xfc) as u8;
        let core = ((nonce >> 24) & 0x7f) as u8;
        let chip = self.chips.iter().position(|c| c.address == address)?;
        self.chips[chip]
            .cores
            .contains(&core)
            .then_some((chip, core))
    }
}

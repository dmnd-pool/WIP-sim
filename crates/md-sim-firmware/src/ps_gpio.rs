//! Zynq-7000 PS GPIO controller state from UG585v1.12.2, chapter14/appendixB.
//! This core accepts settled input events. It does not supply pad levels,
//! MIO mux/tri-state decisions, input synchronization, or physical clocks.
//! Bus-interface reset preserves controller state. Cold reset is separate.
/// Settled sampled bits. A known bit is1 only when its value is established.
/// This is not a VPI wire type: floating/high-impedance and unknown both lack
/// a determined sampled value. Bits outside a register are known0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Word {
    pub value: u32,
    pub known: u32,
}
impl Word {
    pub fn determined(self) -> Option<u32> {
        (self.known == u32::MAX).then_some(self.value)
    }
}

const MASKS: [u32; 4] = [u32::MAX, 0x003f_ffff, u32::MAX, u32::MAX];
const MIO_OUTPUT_ONLY: u32 = (1 << 7) | (1 << 8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessError {
    InvalidOffset,
    WriteOnlyRead,
    InvalidBank,
    InvalidInput,
    OutputOnlyInput,
}

/// Controller intent. Actual MIO driver enable also depends on SLCR mux/tri-state.
/// EMIO data and enable are separate wires. No output is fed back as an input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct Outputs {
    pub data: Word,
    pub direction: u32,
    pub enable: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bank {
    data: Word,
    inputs: Word,
    direction: u32,
    enable: u32,
    interrupt_mask: u32,
    edge_status: Word,
    edge_type: u32,
    polarity: u32,
    any_edge: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PsGpio {
    banks: [Bank; 4],
}

impl Default for PsGpio {
    fn default() -> Self {
        Self::new()
    }
}

impl PsGpio {
    pub fn new() -> Self {
        Self {
            banks: std::array::from_fn(|bank| {
                let mask = MASKS[bank];
                Bank {
                    // MIO output reset is unspecified; EMIO output reset is0.
                    data: if bank < 2 { unknown(mask) } else { known(0) },
                    // External pad/wire state has not been supplied. The EMIO
                    // DATA_RO reset-table value0 does not establish a later
                    // sampled external signal in this unclocked component.
                    inputs: unknown(mask),
                    direction: 0,
                    enable: 0,
                    interrupt_mask: 0,
                    edge_status: known(0),
                    edge_type: mask,
                    polarity: 0,
                    any_edge: 0,
                }
            }),
        }
    }

    /// Explicit controller cold reset. The caller must not map a warm bus reset
    /// to this function. No new external pad levels are created.
    pub fn cold_reset(&mut self) {
        let prior_inputs = self.banks.map(|bank| bank.inputs);
        *self = Self::new();
        for (bank, inputs) in self.banks.iter_mut().zip(prior_inputs) {
            bank.inputs = inputs;
        }
    }

    /// GPIO_CPU1X_RST resets the bus interface only (UG585 section14.4.2).
    /// This state core contains no in-flight bus transaction to reset.
    pub fn bus_interface_reset(&mut self) {}

    pub fn outputs(&self, bank: u8) -> Result<Outputs, AccessError> {
        let bank = self
            .banks
            .get(usize::from(bank))
            .ok_or(AccessError::InvalidBank)?;
        Ok(Outputs {
            data: bank.data,
            direction: bank.direction,
            enable: bank.direction & bank.enable,
        })
    }

    /// One settled observation event for selected pins. Unselected bits must
    /// be known0; they retain their prior state. No floating/unknown sample becomes a known bit.
    /// The first known sample after an unknown sample cannot prove an edge.
    /// It can leave edge status unknown until the guest explicitly clears it.
    pub fn set_settled_inputs(
        &mut self,
        bank: u8,
        value: Word,
        changed: u32,
    ) -> Result<(), AccessError> {
        let index = usize::from(bank);
        let mask = *MASKS.get(index).ok_or(AccessError::InvalidBank)?;
        if changed & !mask != 0 || (value.value | !value.known) & !changed != 0 {
            return Err(AccessError::InvalidInput);
        }
        if bank == 0 && changed & MIO_OUTPUT_ONLY != 0 {
            return Err(AccessError::OutputOnlyInput);
        }
        let b = &mut self.banks[index];
        for bit in 0..32 {
            let flag = 1 << bit;
            if changed & flag == 0 {
                continue;
            }
            if b.edge_type & flag != 0 {
                let event = edge_event(
                    possibilities(b.inputs, bit),
                    possibilities(value, bit),
                    b.polarity & flag != 0,
                    b.any_edge & flag != 0,
                );
                let prior = possibilities(b.edge_status, bit);
                assign(&mut b.edge_status, bit, bool_or(prior, event));
            }
        }
        b.inputs.value = (b.inputs.value & !changed) | value.value;
        b.inputs.known = (b.inputs.known & !changed) | (value.known & changed);
        Ok(())
    }

    /// Shared IRQ52. Known asserted input dominates unknown inputs. An unknown
    /// enabled source cannot be converted into a deasserted physical IRQ.
    pub fn irq(&self) -> Word {
        let mut possible = 1; // possibility set containing false
        for (index, bank) in self.banks.iter().enumerate() {
            let status = bank.status(MASKS[index]);
            for bit in 0..32 {
                let flag = 1 << bit;
                if MASKS[index] & flag != 0 && bank.interrupt_mask & flag == 0 {
                    possible = bool_or(possible, possibilities(status, bit));
                }
            }
        }
        let mut out = known(0);
        assign(&mut out, 0, possible);
        out
    }

    pub fn read32(&self, offset: u32) -> Result<Word, AccessError> {
        let (index, register) = decode(offset)?;
        let b = &self.banks[index];
        let mask = MASKS[index];
        Ok(match register {
            Register::MaskLow => shifted(b.data, 0, 0xffff),
            Register::MaskHigh => shifted(b.data, 16, mask >> 16),
            Register::Data => b.data,
            Register::Inputs if index == 0 => Word {
                // DATA_0_RO bits7/8 always read0 (UG585 AppendixB p1355).
                // This does not establish their interrupt detector input.
                value: b.inputs.value & !MIO_OUTPUT_ONLY,
                known: b.inputs.known | MIO_OUTPUT_ONLY,
            },
            Register::Inputs => b.inputs,
            Register::Direction => known(b.direction),
            Register::Enable => known(b.enable),
            Register::InterruptMask => known(b.interrupt_mask),
            Register::InterruptEnable | Register::InterruptDisable => {
                return Err(AccessError::WriteOnlyRead);
            }
            Register::Status => b.status(mask),
            Register::Type => known(b.edge_type),
            Register::Polarity => known(b.polarity),
            Register::AnyEdge => known(b.any_edge),
        })
    }

    /// The parent bus must reject sizes other than4 before calling this API.
    /// Invalid offsets are rejected before state changes. Reserved bits of a
    /// valid register are ignored on write and read as known0.
    pub fn write32(&mut self, offset: u32, value: u32) -> Result<(), AccessError> {
        let (index, register) = decode(offset)?;
        let mask = MASKS[index];
        let b = &mut self.banks[index];
        match register {
            Register::MaskLow | Register::MaskHigh => {
                let shift = if register == Register::MaskHigh {
                    16
                } else {
                    0
                };
                let selected = (!(value >> 16) & 0xffff) << shift & mask;
                let data = (value & 0xffff) << shift;
                b.data.value = (b.data.value & !selected) | (data & selected);
                b.data.known |= selected;
            }
            Register::Data => b.data = known(value & mask),
            Register::Inputs | Register::InterruptMask => {} // read-only
            Register::Direction => b.direction = value & mask,
            Register::Enable => b.enable = value & mask,
            Register::InterruptEnable => b.interrupt_mask &= !(value & mask),
            Register::InterruptDisable => b.interrupt_mask |= value & mask,
            Register::Status => {
                b.edge_status.value &= !(value & mask);
                b.edge_status.known |= value & mask;
                // A live level condition remains visible; clearing its source
                // or masking it is required (section14.2.4).
            }
            Register::Type => {
                let new_type = value & mask;
                // The source does not define status transfer when switching
                // level to edge. Preserve uncertainty until an explicit W1C.
                let changed_to_edge = !b.edge_type & new_type;
                b.edge_status.value &= !changed_to_edge;
                b.edge_status.known &= !changed_to_edge;
                b.edge_type = new_type;
            }
            Register::Polarity => b.polarity = value & mask,
            Register::AnyEdge => b.any_edge = value & mask,
        }
        Ok(())
    }
}

impl Bank {
    fn status(&self, mask: u32) -> Word {
        let mut out = known(0);
        for bit in 0..32 {
            let flag = 1 << bit;
            if mask & flag == 0 {
                continue;
            }
            if self.edge_type & flag != 0 {
                out.value |= self.edge_status.value & flag;
                out.known = (out.known & !flag) | (self.edge_status.known & flag);
            } else {
                let values = possibilities(self.inputs, bit);
                let active = if self.polarity & flag != 0 {
                    values
                } else {
                    ((values & 1) << 1) | ((values & 2) >> 1)
                };
                assign(&mut out, bit, active);
            }
        }
        out
    }
}

fn known(value: u32) -> Word {
    Word {
        value,
        known: u32::MAX,
    }
}
fn unknown(mask: u32) -> Word {
    Word {
        value: 0,
        known: !mask,
    }
}
fn shifted(word: Word, shift: u32, mask: u32) -> Word {
    Word {
        value: (word.value >> shift) & mask,
        known: ((word.known >> shift) & mask) | !mask,
    }
}
fn possibilities(word: Word, bit: u32) -> u8 {
    if word.known & (1 << bit) == 0 {
        3
    } else {
        1 << ((word.value >> bit) & 1)
    }
}
fn assign(word: &mut Word, bit: u32, possibilities: u8) {
    let flag = 1 << bit;
    word.value = (word.value & !flag) | if possibilities == 2 { flag } else { 0 };
    word.known = (word.known & !flag) | if possibilities != 3 { flag } else { 0 };
}
fn bool_or(a: u8, b: u8) -> u8 {
    let mut result = 0;
    for left in 0..=1 {
        for right in 0..=1 {
            if a & (1 << left) != 0 && b & (1 << right) != 0 {
                result |= 1 << (left | right);
            }
        }
    }
    result
}
fn edge_event(old: u8, new: u8, rising: bool, either: bool) -> u8 {
    let mut result = 0;
    for before in 0..=1 {
        for after in 0..=1 {
            if old & (1 << before) != 0 && new & (1 << after) != 0 {
                let trigger = before != after && (either || (after == u8::from(rising)));
                result |= 1 << u8::from(trigger);
            }
        }
    }
    result
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Register {
    MaskLow,
    MaskHigh,
    Data,
    Inputs,
    Direction,
    Enable,
    InterruptMask,
    InterruptEnable,
    InterruptDisable,
    Status,
    Type,
    Polarity,
    AnyEdge,
}
fn decode(offset: u32) -> Result<(usize, Register), AccessError> {
    if offset & 3 != 0 {
        return Err(AccessError::InvalidOffset);
    }
    if offset <= 0x1c {
        return Ok((
            (offset / 8) as usize,
            if offset & 4 == 0 {
                Register::MaskLow
            } else {
                Register::MaskHigh
            },
        ));
    }
    for (start, reg) in [(0x40, Register::Data), (0x60, Register::Inputs)] {
        if (start..=start + 12).contains(&offset) {
            return Ok((((offset - start) / 4) as usize, reg));
        }
    }
    if (0x200..0x300).contains(&offset) {
        let bank = ((offset - 0x200) / 0x40) as usize;
        let reg = match offset & 0x3f {
            0x04 => Register::Direction,
            0x08 => Register::Enable,
            0x0c => Register::InterruptMask,
            0x10 => Register::InterruptEnable,
            0x14 => Register::InterruptDisable,
            0x18 => Register::Status,
            0x1c => Register::Type,
            0x20 => Register::Polarity,
            0x24 => Register::AnyEdge,
            _ => return Err(AccessError::InvalidOffset),
        };
        return Ok((bank, reg));
    }
    Err(AccessError::InvalidOffset)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn read(g: &PsGpio, offset: u32) -> u32 {
        g.read32(offset).unwrap().determined().unwrap()
    }
    fn sample(g: &mut PsGpio, bank: u8, bit: u32, value: bool) {
        g.set_settled_inputs(bank, known(if value { 1 << bit } else { 0 }), 1 << bit)
            .unwrap();
    }
    #[test]
    fn documented_masked_data_and_bank_widths_preserve_unknowns() {
        let mut g = PsGpio::new();
        assert_eq!(g.read32(0x40).unwrap(), unknown(u32::MAX));
        assert_eq!(g.read32(0x44).unwrap(), unknown(0x3fffff));
        assert_eq!(read(&g, 0x48), 0);
        assert_eq!(read(&g, 0x4c), 0);
        g.write32(0x04, 0xbdef4210).unwrap();
        let o = g.read32(0x40).unwrap();
        assert_eq!(!o.known, 0xbdefffff);
        assert_eq!(o.value & o.known, 0x42100000);
        g.write32(0x40, 0x12345678).unwrap();
        assert_eq!(read(&g, 0), 0x5678);
        assert_eq!(read(&g, 4), 0x1234);
        g.write32(0, 0xfffe0001).unwrap();
        assert_eq!(read(&g, 0x40), 0x12345679);
        g.write32(0x0c, 0x0000ffff).unwrap();
        assert_eq!(read(&g, 0x0c), 0x3f);
        assert_eq!(!g.read32(0x44).unwrap().known, 0xffff);
        g.write32(0x44, u32::MAX).unwrap();
        assert_eq!(read(&g, 0x44), 0x3fffff);
    }
    #[test]
    fn data_latch_input_and_output_enable_are_independent() {
        let mut g = PsGpio::new();
        g.write32(0x40, 1).unwrap();
        sample(&mut g, 0, 0, false);
        assert_eq!(read(&g, 0x40), 1);
        assert_eq!(g.read32(0x60).unwrap().value & 1, 0);
        assert_eq!(g.read32(0x60).unwrap().known & 1, 1);
        g.write32(0x208, 1).unwrap();
        assert_eq!(g.outputs(0).unwrap().enable, 0);
        g.write32(0x204, 1).unwrap();
        assert_eq!(g.outputs(0).unwrap().enable, 1);
        sample(&mut g, 0, 0, true);
        g.write32(0x40, 0).unwrap();
        assert_eq!(g.read32(0x60).unwrap().value & 1, 1);
        g.write32(0x204, 0).unwrap();
        assert_eq!(g.outputs(0).unwrap().enable, 0);
    }
    #[test]
    fn five_interrupt_modes_follow_literal_transition_table() {
        // type,polarity,any,previous,next,asserted. These literals are the
        // five rows of UG585table14-1 and transitions that must not trigger.
        let rows = [
            (1, 1, 0, false, true, true),
            (1, 1, 0, true, false, false),
            (1, 0, 0, true, false, true),
            (1, 0, 0, false, true, false),
            (1, 0, 1, false, true, true),
            (1, 1, 1, true, false, true),
            (1, 1, 1, true, true, false),
            (1, 0, 1, false, false, false),
            (0, 1, 0, false, true, true),
            (0, 1, 1, true, false, false),
            (0, 0, 0, true, false, true),
            (0, 0, 1, false, true, false),
        ];
        for (edge, pol, any, old, new, want) in rows {
            let mut g = PsGpio::new();
            g.write32(0x214, u32::MAX).unwrap();
            g.write32(0x21c, (!1u32) | edge).unwrap();
            g.write32(0x220, pol).unwrap();
            g.write32(0x224, any).unwrap();
            sample(&mut g, 0, 0, old);
            g.write32(0x218, u32::MAX).unwrap();
            sample(&mut g, 0, 0, new);
            assert_eq!(read(&g, 0x218) & 1, u32::from(want));
            assert_eq!(g.irq().determined().unwrap(), 0);
            g.write32(0x210, 1).unwrap();
            assert_eq!(g.irq().determined().unwrap(), u32::from(want));
        }
    }
    #[test]
    fn pending_masks_multi_bank_or_w1c_and_live_levels() {
        let mut g = PsGpio::new();
        for (bank, base) in [(0, 0x200), (1, 0x240), (2, 0x280), (3, 0x2c0)] {
            g.write32(base + 0x14, u32::MAX).unwrap();
            sample(&mut g, bank, 0, true);
            g.write32(base + 0x18, u32::MAX).unwrap();
        }
        sample(&mut g, 0, 0, false);
        sample(&mut g, 2, 0, false);
        assert_eq!(read(&g, 0x218), 1);
        assert_eq!(read(&g, 0x298), 1);
        g.write32(0x210, 1).unwrap();
        assert_eq!(g.irq().determined().unwrap(), 1);
        g.write32(0x290, 1).unwrap();
        g.write32(0x218, 1).unwrap();
        assert_eq!(g.irq().determined().unwrap(), 1); // bank2 still asserts
        g.write32(0x298, 0).unwrap();
        assert_eq!(read(&g, 0x298), 1);
        g.write32(0x298, 1).unwrap();
        assert_eq!(g.irq().determined().unwrap(), 0);
        g.write32(0x21c, !1u32).unwrap(); // active-low level
        assert_eq!(read(&g, 0x218), 1);
        g.write32(0x218, 1).unwrap();
        assert_eq!(read(&g, 0x218), 1);
        sample(&mut g, 0, 0, true);
        assert_eq!(read(&g, 0x218), 0);
        g.write32(0x220, 1).unwrap();
        assert_eq!(read(&g, 0x218), 1);
    }
    #[test]
    fn unknowns_never_become_an_invented_input_or_irq_success() {
        let mut g = PsGpio::new();
        assert!(g.read32(0x68).unwrap().determined().is_none());
        let mio = g.read32(0x60).unwrap();
        assert_eq!(mio.known, 0x180);
        assert_eq!(mio.value & 0x180, 0);
        sample(&mut g, 0, 0, false); // unknown prior may have fallen
        assert_eq!(!g.read32(0x218).unwrap().known, 1);
        assert!(g.irq().determined().is_none());
        g.write32(0x214, 1).unwrap();
        assert_eq!(g.irq().determined().unwrap(), 0);
        g.write32(0x218, 1).unwrap();
        g.write32(0x210, 1).unwrap();
        assert_eq!(g.irq().determined().unwrap(), 0);
        g.write32(0x21c, !2u32).unwrap(); // unsampled low-level pin1
        assert!(g.irq().determined().is_none());
        sample(&mut g, 0, 0, true);
        sample(&mut g, 0, 0, false); // known asserted pin0 dominates
        assert_eq!(g.irq().determined().unwrap(), 1);
        let before = g.clone();
        assert_eq!(
            g.set_settled_inputs(0, known(0), 1 << 7),
            Err(AccessError::OutputOnlyInput)
        );
        assert_eq!(
            g.set_settled_inputs(1, known(1 << 22), 1 << 22),
            Err(AccessError::InvalidInput)
        );
        assert_eq!(
            g.set_settled_inputs(0, known(2), 1),
            Err(AccessError::InvalidInput)
        );
        assert_eq!(g, before);
    }
    #[test]
    fn cold_reset_preserves_external_samples_and_mode_change_is_not_a_fake_edge() {
        let mut g = PsGpio::new();
        sample(&mut g, 2, 3, true);
        g.write32(0x29c, !8u32).unwrap();
        g.write32(0x2a0, 8).unwrap();
        assert_eq!(read(&g, 0x298), 8);
        g.write32(0x29c, u32::MAX).unwrap();
        assert_eq!(!g.read32(0x298).unwrap().known, 8);
        g.write32(0x298, 8).unwrap();
        assert_eq!(read(&g, 0x298), 0);
        let input = g.read32(0x68).unwrap();
        g.cold_reset();
        assert_eq!(g.read32(0x68).unwrap(), input);
        assert_eq!(read(&g, 0x298), 0);
        assert_eq!(g.irq().determined(), Some(0));
        for (offset, want) in [
            (0x20c, 0),
            (0x24c, 0),
            (0x28c, 0),
            (0x2cc, 0),
            (0x21c, 0xffffffff),
            (0x25c, 0x3fffff),
            (0x29c, 0xffffffff),
            (0x2dc, 0xffffffff),
        ] {
            assert_eq!(read(&g, offset), want);
        }
    }
    #[test]
    fn reset_kinds_readonly_registers_and_invalid_offsets_are_explicit() {
        let mut g = PsGpio::new();
        g.write32(0x44, 0x123456).unwrap();
        g.write32(0x244, u32::MAX).unwrap();
        g.write32(0x248, u32::MAX).unwrap();
        g.write32(0x254, u32::MAX).unwrap();
        assert_eq!(read(&g, 0x24c), 0x3fffff);
        let before = g.clone();
        g.bus_interface_reset();
        assert_eq!(g, before);
        g.write32(0x24c, 0).unwrap();
        g.write32(0x64, 0).unwrap();
        assert_eq!(g, before);
        for offset in [
            1,
            2,
            3,
            0x20,
            0x50,
            0x70,
            0x200,
            0x228,
            0x2e8,
            0x304,
            0x1000,
            u32::MAX,
        ] {
            assert_eq!(g.read32(offset), Err(AccessError::InvalidOffset));
            assert_eq!(g.write32(offset, 0), Err(AccessError::InvalidOffset));
        }
        assert_eq!(g, before);
        for offset in [0x210, 0x214, 0x250, 0x254, 0x290, 0x294, 0x2d0, 0x2d4] {
            assert_eq!(g.read32(offset), Err(AccessError::WriteOnlyRead));
        }
        g.cold_reset();
        assert_eq!(g, PsGpio::new());
        assert_eq!(read(&g, 0x24c), 0);
        assert_eq!(read(&g, 0x25c), 0x3fffff);
    }
}

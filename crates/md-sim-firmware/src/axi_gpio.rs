//! Single-channel AXI GPIO register contract for fixed input/output ports.
//! PG144 documents DATA, TRI, whole-word writes and absent-register behavior.
//! The caller supplies settled pin levels. Synchronizer and AXI handshake
//! timing require a separate clocked binding; this is not a timing calibration.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidAccess;

/// One fixed-direction channel, with no second channel or interrupt block.
/// Width and reset values come from the selected hardware profile.
pub struct Gpio {
    mask: u32,
    direction: Direction,
    reset_data: u32,
    reset_tri: u32,
    data: u32,
    tri: u32,
    inputs: u32,
}

impl Gpio {
    pub fn new(
        width: u8,
        direction: Direction,
        reset_data: u32,
        reset_tri: u32,
        settled_inputs: u32,
    ) -> Option<Self> {
        if !(1..=32).contains(&width) {
            return None;
        }
        let mask = u32::MAX >> (32 - width);
        Some(Self {
            mask,
            direction,
            reset_data: reset_data & mask,
            reset_tri: reset_tri & mask,
            data: reset_data & mask,
            tri: reset_tri & mask,
            inputs: settled_inputs & mask,
        })
    }

    /// Peripheral reset does not reset external wires or remove a board.
    pub fn reset(&mut self) {
        self.data = self.reset_data;
        self.tri = self.reset_tri;
    }

    /// This interface accepts already synchronized pin values.
    pub fn set_settled_inputs(&mut self, inputs: u32) {
        self.inputs = inputs & self.mask;
    }

    /// Fixed output ports expose DATA independently of the TRI port.
    /// Fixed input ports have no driven outputs.
    pub fn outputs(&self) -> u32 {
        match self.direction {
            Direction::Input => 0,
            Direction::Output => self.data,
        }
    }

    /// The TRI register remains separate from fixed-direction external wiring.
    pub fn tristate_register(&self) -> u32 {
        self.tri
    }

    /// Offsets are within the IP's nine-bit address interface. A parent bus
    /// must perform any larger-window address decoding explicitly.
    pub fn read(&self, offset: u16) -> Result<u32, InvalidAccess> {
        check_offset(offset)?;
        Ok(match offset {
            0 if self.direction == Direction::Input => self.inputs,
            4 => self.tri,
            _ => 0,
        })
    }

    /// WSTRB is deliberately ignored, as specified by PG144. The caller must
    /// supply all 32 WDATA bits even when some byte lanes are not selected.
    pub fn write(&mut self, offset: u16, data: u32, _strobe: u8) -> Result<(), InvalidAccess> {
        check_offset(offset)?;
        match offset {
            0 if self.direction == Direction::Output => self.data = data & self.mask,
            4 => self.tri = data & self.mask,
            _ => {}
        }
        Ok(())
    }
}

fn check_offset(offset: u16) -> Result<(), InvalidAccess> {
    if offset < 0x200 && offset % 4 == 0 {
        Ok(())
    } else {
        Err(InvalidAccess)
    }
}

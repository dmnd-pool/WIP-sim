//! DS4432 dual current DAC. Reference: Maxim DS4432 Rev 0, December 2008.
//! This is a current output, not a voltage regulator or a voltage sensor.
//! Single-byte register operations are supported. Undocumented burst and
//! reserved-address behavior is rejected with a visible model fault.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Pointer,
    Write,
    Read,
    Finished,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Output {
    HighImpedance,
    /// Nominal current in nA, positive for sourcing and negative for sinking.
    /// Rounded toward zero. Not a measured or calibrated output.
    CurrentNa(i32),
    /// Invalid supply, unconnected FS resistor, or output outside compliance.
    Unspecified,
}

pub struct Dac {
    rfs_ohms: [u32; 2],
    supply_uv: u32,
    registers: [u8; 2],
    pointer: Option<usize>,
    phase: Phase,
    faults: u64,
    writes: u64,
}
impl Dac {
    /// A zero FS resistance denotes an unconnected pin, not an ideal source.
    /// Supported resistances follow the datasheet's 40..160 kOhm range.
    pub fn new(rfs_ohms: [u32; 2], supply_uv: u32) -> Option<Self> {
        if rfs_ohms
            .iter()
            .any(|r| *r != 0 && !(40_000..=160_000).contains(r))
        {
            return None;
        }
        Some(Self {
            rfs_ohms,
            supply_uv,
            registers: [0; 2],
            pointer: None,
            phase: Phase::Idle,
            faults: 0,
            writes: 0,
        })
    }
    fn powered(&self) -> bool {
        (2_700_000..=5_500_000).contains(&self.supply_uv)
    }
    /// Power cycling clears volatile settings. Controller/bus reset does not.
    pub fn power_reset(&mut self) {
        self.registers = [0; 2];
        self.pointer = None;
        self.phase = Phase::Idle;
    }
    pub fn set_supply(&mut self, uv: u32) {
        let was_powered = self.powered();
        self.supply_uv = uv;
        if !self.powered() || !was_powered {
            self.power_reset();
        }
    }
    pub fn start(&mut self, read: bool) -> bool {
        if !self.powered() {
            self.phase = Phase::Idle;
            return false;
        }
        if read && self.pointer.is_none() {
            return self.fail();
        }
        self.phase = if read { Phase::Read } else { Phase::Pointer };
        true
    }
    fn fail(&mut self) -> bool {
        self.faults = self.faults.saturating_add(1);
        self.phase = Phase::Finished;
        false
    }
    pub fn write(&mut self, value: u8) -> bool {
        if !self.powered() {
            return false;
        }
        match self.phase {
            Phase::Pointer => {
                self.pointer = match value {
                    0xf8 => Some(0),
                    0xf9 => Some(1),
                    _ => None,
                };
                if self.pointer.is_none() {
                    return self.fail();
                }
                self.phase = Phase::Write;
                true
            }
            Phase::Write => {
                let Some(channel) = self.pointer else {
                    return self.fail();
                };
                self.registers[channel] = value;
                self.writes = self.writes.saturating_add(1);
                self.phase = Phase::Finished;
                true
            }
            _ => self.fail(),
        }
    }
    pub fn read(&mut self) -> Option<u8> {
        if !self.powered() || self.phase != Phase::Read {
            self.fail();
            return None;
        }
        let channel = self.pointer?;
        self.phase = Phase::Finished;
        Some(self.registers[channel])
    }
    /// STOP or a controller-only bus reset preserves the DAC and pointer.
    pub fn stop(&mut self) {
        self.phase = Phase::Idle;
    }
    pub fn nack(&mut self) {
        self.phase = Phase::Finished;
    }
    pub fn faults(&self) -> u64 {
        self.faults
    }
    pub fn writes(&self) -> u64 {
        self.writes
    }
    pub fn output(&self, channel: usize, node_uv: u32) -> Output {
        if channel >= 2 || !self.powered() || node_uv > self.supply_uv + 500_000 {
            return Output::Unspecified;
        }
        let code = self.registers[channel];
        let magnitude = code & 0x7f;
        if magnitude == 0 {
            return Output::HighImpedance;
        }
        let rfs = self.rfs_ohms[channel];
        let source = code & 0x80 != 0;
        let compliant = if source {
            node_uv <= self.supply_uv - 750_000
        } else {
            (500_000..=3_500_000).contains(&node_uv)
        };
        if rfs == 0 || !compliant {
            return Output::Unspecified;
        }
        // Iout = VRFS / RFS * code / 16. VRFS is nominal 0.997 V.
        let na = (997_000_000u64 * u64::from(magnitude) / (16 * u64::from(rfs))) as i32;
        Output::CurrentNa(if source { na } else { -na })
    }
}

#[no_mangle]
pub extern "C" fn md_ds4432_size() -> usize {
    core::mem::size_of::<Dac>()
}
/// # Safety
/// State is aligned exclusive writable storage of md_ds4432_size bytes.
/// A false return leaves the storage uninitialized.
#[no_mangle]
pub unsafe extern "C" fn md_ds4432_init(
    state: *mut Dac,
    rfs0: u32,
    rfs1: u32,
    supply_uv: u32,
) -> bool {
    let Some(dac) = Dac::new([rfs0, rfs1], supply_uv) else {
        return false;
    };
    state.write(dac);
    true
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ds4432_reset(state: *mut Dac) {
    (*state).power_reset();
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ds4432_start(state: *mut Dac, read: bool) -> bool {
    (*state).start(read)
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ds4432_write(state: *mut Dac, value: u8) -> bool {
    (*state).write(value)
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ds4432_read(state: *mut Dac) -> i32 {
    (*state).read().map(i32::from).unwrap_or(-1)
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ds4432_stop(state: *mut Dac) {
    (*state).stop();
}
/// # Safety
/// State is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_ds4432_nack(state: *mut Dac) {
    (*state).nack();
}
/// # Safety
/// State is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_ds4432_faults(state: *const Dac) -> u64 {
    (*state).faults()
}
/// # Safety
/// State is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_ds4432_writes(state: *const Dac) -> u64 {
    (*state).writes()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(d: &mut Dac, reg: u8, code: u8) {
        assert!(d.start(false));
        assert!(d.write(reg));
        assert!(d.write(code));
        d.stop();
    }
    fn read(d: &mut Dac, reg: u8) -> u8 {
        assert!(d.start(false));
        assert!(d.write(reg));
        assert!(d.start(true));
        let code = d.read().unwrap();
        d.nack();
        d.stop();
        code
    }
    #[test]
    fn published_example_and_sign_are_independent_of_supply_feedback() {
        let mut d = Dac::new([80_000, 40_000], 3_300_000).unwrap();
        // DS4432 p6: RFS=80k, register AA -> source 32.714 uA.
        write(&mut d, 0xf8, 0xaa);
        assert_eq!(d.output(0, 1_200_000), Output::CurrentNa(32_714));
        assert_eq!(read(&mut d, 0xf8), 0xaa);
        write(&mut d, 0xf8, 0x2a);
        assert_eq!(d.output(0, 1_200_000), Output::CurrentNa(-32_714));
        write(&mut d, 0xf9, 0xff);
        assert_eq!(d.output(1, 1_200_000), Output::CurrentNa(197_842));
        assert_eq!(read(&mut d, 0xf8), 0x2a);
        assert_eq!(d.faults(), 0);
    }
    #[test]
    fn power_bus_reset_and_read_pointer_have_distinct_behavior() {
        let mut d = Dac::new([80_000, 0], 3_300_000).unwrap();
        assert_eq!(d.output(0, 0), Output::HighImpedance);
        assert!(!d.start(true)); // power-on pointer is unspecified, never invented.
        write(&mut d, 0xf8, 0x91);
        d.stop();
        assert_eq!(read(&mut d, 0xf8), 0x91);
        d.set_supply(0);
        assert!(!d.start(false));
        assert_eq!(d.output(0, 0), Output::Unspecified);
        d.set_supply(3_300_000);
        assert_eq!(read(&mut d, 0xf8), 0);
        write(&mut d, 0xf8, 0x80);
        assert_eq!(d.output(0, 0), Output::HighImpedance);
        write(&mut d, 0xf9, 0xff);
        assert_eq!(d.output(1, 1_200_000), Output::Unspecified);
    }
    #[test]
    fn unsupported_transactions_do_not_change_other_registers() {
        let mut d = Dac::new([80_000, 80_000], 3_300_000).unwrap();
        assert!(d.start(false));
        assert!(!d.write(0xf7));
        assert!(!d.write(0x77));
        assert_eq!(read(&mut d, 0xf8), 0);
        assert!(d.start(false));
        assert!(d.write(0xf8));
        assert!(d.write(3));
        assert!(!d.write(4));
        d.stop();
        assert_eq!(read(&mut d, 0xf8), 3);
        assert_eq!(read(&mut d, 0xf9), 0);
        assert!(d.start(true));
        assert_eq!(d.read(), Some(0));
        assert_eq!(d.read(), None);
        assert_eq!(d.writes(), 1);
        assert_eq!(d.faults(), 4);
    }
    #[test]
    fn output_compliance_and_resistor_limits_are_explicit() {
        assert!(Dac::new([39_999, 80_000], 3_300_000).is_none());
        assert!(Dac::new([80_000, 160_001], 3_300_000).is_none());
        let mut d = Dac::new([80_000, 80_000], 3_300_000).unwrap();
        write(&mut d, 0xf8, 0xff);
        assert_eq!(d.output(0, 2_550_001), Output::Unspecified);
        assert_eq!(d.output(0, 2_550_000), Output::CurrentNa(98_921));
        write(&mut d, 0xf8, 0x7f);
        assert_eq!(d.output(0, 499_999), Output::Unspecified);
        assert_eq!(d.output(0, 3_500_001), Output::Unspecified);
        assert_eq!(d.output(0, 500_000), Output::CurrentNa(-98_921));
        assert_eq!(d.output(2, 500_000), Output::Unspecified);
        d.set_supply(2_700_000);
        assert_eq!(d.output(0, 3_200_000), Output::CurrentNa(-98_921));
        assert_eq!(d.output(0, 3_200_001), Output::Unspecified);
        write(&mut d, 0xf8, 0);
        assert_eq!(d.output(0, 3_200_001), Output::Unspecified);
    }
}

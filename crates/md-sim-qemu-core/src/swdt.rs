//! Zynq SWDT registers. QEMU's existing PTIMER owns the clock and counter.
//! The contract and unresolved board details are in SWDT.md.

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Registers {
    mode: u32,
    control: u32,
    zero: u32,
}

impl Default for Registers {
    fn default() -> Self {
        Self {
            mode: 0x1c0,
            control: 0x3ffc,
            zero: 0,
        }
    }
}

/// Effects that the timer adapter must apply after a register transaction.
#[repr(i32)]
#[derive(Debug, PartialEq, Eq)]
pub enum WriteEffect {
    Invalid = -1,
    None = 0,
    Clock = 1,
    Restart = 2,
}

#[repr(C)]
#[derive(Default, Debug, PartialEq, Eq)]
pub struct Expiry {
    pub irq_cycles: u32,
    pub reset: u32,
}

impl Registers {
    pub fn read(&self, offset: u32) -> Option<u32> {
        match offset {
            0 => Some(self.mode),
            4 => Some(self.control),
            8 => Some(0),
            12 => Some(self.zero),
            _ => None,
        }
    }

    pub fn write(&mut self, offset: u32, value: u32) -> WriteEffect {
        match offset {
            0 if value & 0x00ff_f000 == 0x00ab_c000 => {
                self.mode = value & 0x1f7;
                WriteEffect::None
            }
            4 if value & 0x03ff_c000 == 0x0092_0000 => {
                let changed = (value ^ self.control) & 3 != 0;
                self.control = value & 0x3fff;
                if changed {
                    WriteEffect::Clock
                } else {
                    WriteEffect::None
                }
            }
            8 if value & 0xffff == 0x1999 => {
                self.zero = 0;
                WriteEffect::Restart
            }
            0 | 4 | 8 | 12 => WriteEffect::None,
            _ => WriteEffect::Invalid,
        }
    }

    pub fn reload(&self) -> u32 {
        ((self.control & 0x3ffc) << 10) | 0xfff
    }
    pub fn divisor(&self) -> u32 {
        [8, 64, 512, 4096][(self.control & 3) as usize]
    }

    /// Called once when the external counter reaches zero. Repeated calls
    /// cannot create another pulse until a restart clears the zero latch.
    pub fn expire(&mut self) -> Expiry {
        let previous = self.zero;
        self.zero = 1;
        if previous != 0 || self.mode & 1 == 0 {
            return Expiry::default();
        }
        Expiry {
            irq_cycles: if self.mode & 4 != 0 {
                4 << ((self.mode >> 7) & 3)
            } else {
                0
            },
            reset: u32::from(self.mode & 2 != 0),
        }
    }
}

/// # Safety
/// state points to writable, aligned storage for one Registers value.
#[no_mangle]
pub unsafe extern "C" fn md_swdt_reset(state: *mut Registers) {
    state.write(Registers::default());
}

/// # Safety
/// state is live; value points to writable u32 storage. Pointers do not alias.
#[no_mangle]
pub unsafe extern "C" fn md_swdt_read(
    state: *const Registers,
    offset: u32,
    value: *mut u32,
) -> bool {
    if let Some(result) = (*state).read(offset) {
        value.write(result);
        true
    } else {
        false
    }
}

/// # Safety
/// state points to a live, exclusively held Registers value.
#[no_mangle]
pub unsafe extern "C" fn md_swdt_write(state: *mut Registers, offset: u32, value: u32) -> i32 {
    (*state).write(offset, value) as i32
}

/// # Safety
/// state points to a live Registers value.
#[no_mangle]
pub unsafe extern "C" fn md_swdt_reload(state: *const Registers) -> u32 {
    (*state).reload()
}

/// # Safety
/// state points to a live Registers value.
#[no_mangle]
pub unsafe extern "C" fn md_swdt_divisor(state: *const Registers) -> u32 {
    (*state).divisor()
}

/// # Safety
/// state is live and exclusive; event is writable and does not alias state.
#[no_mangle]
pub unsafe extern "C" fn md_swdt_expire(state: *mut Registers, event: *mut Expiry) {
    event.write((*state).expire());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_reserved_bits_and_read_only_status() {
        let mut s = Registers::default();
        assert_eq!(
            [s.read(0), s.read(4), s.read(8), s.read(12)],
            [Some(0x1c0), Some(0x3ffc), Some(0), Some(0)]
        );
        for key in 0..4096 {
            let mut t = s;
            t.write(0, (key << 12) | 0xfff);
            assert_eq!(t.read(0), Some(if key == 0xabc { 0x1f7 } else { 0x1c0 }));
            t.write(4, (key << 14) | 0x23ab);
            assert_eq!(t.read(4), Some(if key == 0x248 { 0x23ab } else { 0x3ffc }));
        }
        s.expire();
        s.write(12, 0);
        assert_eq!(s.read(12), Some(1));
        for key in 0..65536 {
            let mut t = s;
            assert_eq!(
                t.write(8, key),
                if key == 0x1999 {
                    WriteEffect::Restart
                } else {
                    WriteEffect::None
                }
            );
            assert_eq!(t.read(12), Some(u32::from(key != 0x1999)));
        }
        assert_eq!(s.read(16), None);
        assert_eq!(s.write(16, 0), WriteEffect::Invalid);
        assert_eq!(s.read(1), None);
    }

    #[test]
    fn literal_reload_and_prescaler_vectors() {
        let mut s = Registers::default();
        for (word, count, divisor) in [
            (0x00920000, 4095, 8),
            (0x00920005, 8191, 64),
            (0x0092048e, 0x123fff, 512),
            (0x00923fff, 16777215, 4096),
        ] {
            s.write(4, word);
            assert_eq!((s.reload(), s.divisor()), (count, divisor));
        }
    }

    #[test]
    fn output_enables_are_independent_of_zero_and_restart() {
        for mode in 0..8 {
            for (length, cycles) in [(0, 4), (0x80, 8), (0x100, 16), (0x180, 32)] {
                let mut s = Registers::default();
                s.write(0, 0xabc040 | mode | length);
                let event = s.expire();
                assert_eq!(event.irq_cycles, if mode & 5 == 5 { cycles } else { 0 });
                assert_eq!(event.reset, u32::from(mode & 3 == 3));
                assert_eq!(s.read(12), Some(1));
                assert_eq!(s.expire(), Expiry::default());
                s.write(8, 0x1999);
                assert_eq!(s.read(12), Some(0));
                assert_eq!(s.expire(), event);
                s = Registers::default();
                assert_eq!(s.read(12), Some(0));
            }
        }
    }
}

//! ESP32-S3 RTC SAR conversion state. No guest code or boot conditions enter
//! this model. See ../README.md for supported behavior and evidence limits.
#![cfg_attr(not(test), no_std)]

pub mod ds4432;
pub mod emc2101;
pub mod i2c;
pub mod ina260;

const DONE: u32 = 1 << 16;
const START: u32 = 1 << 17;
const FORCE: u32 = 1 << 18;
const PAD_FORCE: u32 = 1 << 31;
const CLOCK: u32 = 1 << 30;
const CTRL: [usize; 2] = [3, 12];
const READER: [usize; 2] = [0, 9];
const ATTEN: [usize; 2] = [5, 14];

#[derive(Clone, Copy)]
struct Conversion {
    due: u64,
    data: u16,
}
/// All external voltages are explicit microvolts. -1 is an unconnected input.
/// The ideal transfer equation is independent of the controller state.
pub fn convert(uv: u32, reference_uv: u32, attenuation: u32) -> u16 {
    if reference_uv == 0 || attenuation > 3 {
        return 0;
    }
    let (n, d) = [(1u64, 1u64), (3, 4), (1, 2), (1, 4)][attenuation as usize];
    ((u64::from(uv) * n * 4095 / (u64::from(reference_uv) * d)).min(4095)) as u16
}

pub struct Controller {
    sens: [u32; 68],
    analog_config: [u32; 20],
    analog: [[u8; 256]; 128],
    analog_ctrl: [u32; 2],
    pending: [Option<Conversion>; 2],
    inputs_uv: [[i32; 10]; 2],
    reference_uv: u32,
    rtc_clock_hz: u32,
    faults: u64,
    completed: u64,
}
impl Default for Controller {
    fn default() -> Self {
        let mut out = Self {
            sens: [0; 68],
            analog_config: [0; 20],
            analog: [[0; 256]; 128],
            analog_ctrl: [0; 2],
            pending: [None; 2],
            inputs_uv: [[-1; 10]; 2],
            reference_uv: 1_100_000,
            rtc_clock_hz: 8_000_000,
            faults: 0,
            completed: 0,
        };
        out.reset();
        out
    }
}
impl Controller {
    /// Power reset clears register and in-flight state. Physical inputs persist.
    pub fn reset(&mut self) {
        self.sens = [0; 68];
        self.sens[0] = 0x2000_0002;
        self.sens[9] = 0x4001_0002;
        self.sens[5] = u32::MAX;
        self.sens[14] = u32::MAX;
        self.analog_config = [0; 20];
        self.analog = [[0; 256]; 128];
        self.analog_ctrl = [0; 2];
        self.pending = [None; 2];
        self.faults = 0;
        self.completed = 0;
    }
    pub fn set_input(&mut self, unit: usize, channel: usize, uv: i32) -> bool {
        if unit >= 2 || channel >= 10 || !(-1..=3_300_000).contains(&uv) {
            return false;
        }
        self.inputs_uv[unit][channel] = uv;
        true
    }
    pub fn faults(&self) -> u64 {
        self.faults
    }
    pub fn completed(&self) -> u64 {
        self.completed
    }
    pub fn next_deadline(&self) -> u64 {
        self.pending
            .iter()
            .flatten()
            .map(|v| v.due)
            .min()
            .unwrap_or(u64::MAX)
    }
    pub fn advance(&mut self, now: u64) {
        for (unit, index) in CTRL.into_iter().enumerate() {
            if let Some(pending) = self.pending[unit] {
                if now >= pending.due {
                    self.sens[index] =
                        (self.sens[index] & !0x1ffff) | DONE | u32::from(pending.data);
                    self.pending[unit] = None;
                    self.completed = self.completed.saturating_add(1);
                }
            }
        }
    }
    fn fault(&mut self) {
        self.faults = self.faults.saturating_add(1);
    }
    pub fn read(&mut self, bank: u32, offset: u32, size: u32) -> u32 {
        if size != 4 || offset & 3 != 0 {
            self.fault();
            return 0;
        }
        let index = (offset / 4) as usize;
        match bank {
            0 if index < self.sens.len() => self.sens[index],
            1 if index < 2 => {
                let command = self.analog_ctrl[index];
                let block = (command & 0xff) as usize;
                if block >= 128 {
                    self.fault();
                    return 0;
                }
                u32::from(self.analog[block][((command >> 8) & 0xff) as usize]) << 16
            }
            1 if index < self.analog_config.len() => self.analog_config[index],
            _ => {
                self.fault();
                0
            }
        }
    }
    pub fn write(&mut self, bank: u32, offset: u32, size: u32, value: u32, now: u64) {
        if size != 4 || offset & 3 != 0 {
            self.fault();
            return;
        }
        let index = (offset / 4) as usize;
        if bank == 1 {
            if index < 2 {
                let block = (value & 0xff) as usize;
                if block >= 128 {
                    self.fault();
                    return;
                }
                self.analog_ctrl[index] = value;
                if value & (1 << 24) != 0 {
                    self.analog[block][((value >> 8) & 0xff) as usize] = (value >> 16) as u8;
                }
            } else if index < self.analog_config.len() {
                self.analog_config[index] = value;
            } else {
                self.fault();
            }
            return;
        }
        if bank != 0 || index >= self.sens.len() {
            self.fault();
            return;
        }
        if let Some(unit) = CTRL.iter().position(|&v| v == index) {
            let old = self.sens[index];
            self.sens[index] = (old & 0x1ffff) | (value & !0x1ffff);
            if value & START == 0 {
                self.pending[unit] = None;
                self.sens[index] &= !DONE;
            } else if old & START == 0 {
                self.start(unit, now);
            }
            return;
        }
        match index {
            0 | 9 | 4 | 13 | 5 | 14 | 15 | 65 | 66 => self.sens[index] = value,
            // RTC I2C slave address registers share this window. Their ADC
            // status bits are read-only and stay idle outside a conversion.
            16..=19 => self.sens[index] = value & 0x3f_ffff,
            _ => {
                self.fault();
                return;
            }
        }
        if index == 66 && value & CLOCK != 0 {
            self.pending = [None; 2];
            for i in CTRL {
                self.sens[i] &= !0x3ffff;
            }
        }
        if (index == 65 && value & CLOCK == 0) || (index == 15 && ((value >> 29) & 3) == 2) {
            self.pending = [None; 2];
            for i in CTRL {
                self.sens[i] &= !DONE;
            }
        }
    }
    fn start(&mut self, unit: usize, now: u64) {
        let control = self.sens[CTRL[unit]];
        self.sens[CTRL[unit]] &= !DONE;
        // Driver-derived RTC power encoding: 3=force up, 2=force down.
        // This conflicts with the TRM wording; see the evidence note.
        if control & FORCE == 0
            || self.sens[65] & CLOCK == 0
            || self.sens[66] & CLOCK != 0
            || ((self.sens[15] >> 29) & 3) == 2
            || (unit == 0 && self.sens[4] & PAD_FORCE != 0)
        {
            self.fault();
            return;
        }
        let ground = self.analog[0x69][7] & (1 << (5 + unit * 2)) != 0;
        let data = if ground {
            0
        } else {
            let pins = (control >> 19) & 0xfff;
            if control & PAD_FORCE == 0 || pins.count_ones() != 1 || pins.trailing_zeros() >= 10 {
                self.fault();
                return;
            }
            let channel = pins.trailing_zeros() as usize;
            let uv = self.inputs_uv[unit][channel];
            if uv < 0 {
                self.fault();
                return;
            }
            convert(
                uv as u32,
                self.reference_uv,
                (self.sens[ATTEN[unit]] >> (channel * 2)) & 3,
            )
        };
        let invert = self.sens[READER[unit]] & (1 << (28 + unit)) != 0;
        let data = if invert { data ^ 0xfff } else { data };
        let divider = (self.sens[READER[unit]] & 0xff).max(1);
        // 25 conversion clocks plus configured input sample cycles. Timing is
        // an explicit functional approximation; no analogue calibration claim.
        let cycles = 25 + u64::from(self.analog[0x69][2] & 7);
        let delay =
            (cycles * u64::from(divider) * 1_000_000_000).div_ceil(u64::from(self.rtc_clock_hz));
        let Some(due) = now.checked_add(delay) else {
            self.fault();
            return;
        };
        self.pending[unit] = Some(Conversion { due, data });
    }
}

#[no_mangle]
pub extern "C" fn md_sar_size() -> usize {
    core::mem::size_of::<Controller>()
}
/// # Safety
/// Pointer has aligned writable storage of md_sar_size bytes, owned exclusively.
#[no_mangle]
pub unsafe extern "C" fn md_sar_init(s: *mut Controller) {
    s.write(Controller::default());
}
/// # Safety
/// Pointer is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_sar_reset(s: *mut Controller) {
    (*s).reset();
}
/// # Safety
/// Pointer is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_sar_set_input(
    s: *mut Controller,
    unit: u32,
    channel: u32,
    uv: i32,
) -> bool {
    (*s).set_input(unit as usize, channel as usize, uv)
}
/// # Safety
/// Pointer is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_sar_read(s: *mut Controller, b: u32, o: u32, n: u32) -> u32 {
    (*s).read(b, o, n)
}
/// # Safety
/// Pointer is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_sar_write(s: *mut Controller, b: u32, o: u32, n: u32, v: u32, t: u64) {
    (*s).write(b, o, n, v, t);
}
/// # Safety
/// Pointer is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_sar_advance(s: *mut Controller, t: u64) {
    (*s).advance(t);
}
/// # Safety
/// Pointer is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_sar_deadline(s: *const Controller) -> u64 {
    (*s).next_deadline()
}
/// # Safety
/// Pointer is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_sar_faults(s: *const Controller) -> u64 {
    (*s).faults()
}
/// # Safety
/// Pointer is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_sar_completed(s: *const Controller) -> u64 {
    (*s).completed()
}
#[cfg(not(test))]
extern "C" {
    fn abort() -> !;
}
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn rust_eh_personality(
    _: i32,
    _: u32,
    _: u64,
    _: *mut core::ffi::c_void,
    _: *mut core::ffi::c_void,
) -> i32 {
    unsafe { abort() }
}
#[cfg(not(test))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    unsafe { abort() }
}

#[cfg(test)]
mod tests;

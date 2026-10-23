//! AXI IIC dynamic-master register engine, shared by QEMU and protocol tests.
//! The backend calls `step` once per byte time. No time or bus I/O is hidden in
//! register accesses. The bus supplies actual ACK/NACK and received bytes.
#![cfg_attr(not(any(test, feature = "host-std")), no_std)]

use core::ffi::c_void;

pub mod onfi;
pub mod pl353;

const ENABLE: u8 = 1;
const TX_RESET: u8 = 2;
const START: u16 = 0x100;
const STOP: u16 = 0x200;
const TX_ERROR: u8 = 2;
const TX_EMPTY: u8 = 4;
const RX_FULL: u8 = 8;
const NOT_BUSY: u8 = 16;
const TX_HALF: u8 = 128;

#[derive(Clone, Copy, Default)]
#[repr(C)]
struct Fifo {
    values: [u16; 16],
    head: u8,
    len: u8,
}
impl Fifo {
    fn push(&mut self, value: u16) -> bool {
        if self.len == 16 {
            return false;
        }
        self.values[((self.head + self.len) & 15) as usize] = value;
        self.len += 1;
        true
    }
    fn pop(&mut self) -> Option<u16> {
        if self.len == 0 {
            return None;
        }
        let value = self.values[self.head as usize];
        self.head = (self.head + 1) & 15;
        self.len -= 1;
        Some(value)
    }
}

/// Small bus contract. `true` from start/send is ACK; `false` is NACK.
pub trait Bus {
    fn start(&mut self, address: u8, read: bool) -> bool;
    fn send(&mut self, byte: u8) -> bool;
    fn receive(&mut self) -> u8;
    fn nack(&mut self);
    fn end(&mut self);
}

#[derive(Default)]
#[repr(C)]
pub struct Controller {
    tx: Fifo,
    rx: Fifo,
    control: u8,
    address: u8,
    ten_bit_address: u8,
    receive_depth: u8,
    output: u8,
    isr: u8,
    ier: u8,
    global: bool,
    active: bool,
    reading: bool,
    read_count: u8,
    read_stop: bool,
    bus_end_pending: bool,
    unsupported: bool,
    faults: u64,
}
impl Controller {
    pub fn reset(&mut self) {
        let was_active = self.active || self.bus_end_pending;
        *self = Self {
            isr: TX_HALF | NOT_BUSY | 0x40,
            bus_end_pending: was_active,
            ..Self::default()
        };
    }
    pub fn irq(&self) -> bool {
        self.global && self.ier & self.isr != 0
    }
    fn bus_busy(&self) -> bool {
        // START occupies the bus before its address ACK. This byte-clock
        // engine delivers the address to Bus::start only at that ACK.
        // An enabled START at the FIFO head therefore reserves the bus.
        self.active
            || (!self.unsupported
                && self.control & (ENABLE | TX_RESET) == ENABLE
                && self.tx.len != 0
                && self.tx.values[self.tx.head as usize] & START != 0)
    }
    pub fn status(&self) -> u8 {
        (if self.bus_busy() { 4 } else { 0 })
            | (if self.tx.len == 16 { 0x10 } else { 0 })
            | (if self.rx.len == 16 { 0x20 } else { 0 })
            | (if self.rx.len == 0 { 0x40 } else { 0 })
            | (if self.tx.len == 0 { 0x80 } else { 0 })
    }
    pub fn pending(&self) -> bool {
        self.bus_end_pending
            || (!self.unsupported
                && self.control & (ENABLE | TX_RESET) == ENABLE
                && ((self.read_count > 0 && self.rx.len <= self.receive_depth)
                    || (self.read_count == 0 && self.tx.len > 0)))
    }
    pub fn fault_count(&self) -> u64 {
        self.faults
    }
    fn fault(&mut self) {
        self.faults = self.faults.saturating_add(1);
        self.isr |= TX_ERROR;
    }
    fn unsupported_request(&mut self) {
        self.fault();
        self.unsupported = true;
        self.bus_end_pending |= self.active;
        self.active = false;
        self.read_count = 0;
        self.tx = Fifo::default();
        self.isr |= NOT_BUSY | TX_EMPTY | TX_HALF;
    }
    fn stop(&mut self, bus: &mut impl Bus) {
        if self.active {
            bus.end();
        }
        self.active = false;
        self.reading = false;
        self.read_count = 0;
        self.isr |= NOT_BUSY;
    }
    fn failed(&mut self, bus: &mut impl Bus) {
        self.fault();
        self.tx = Fifo::default();
        self.stop(bus);
        self.isr |= TX_EMPTY | TX_HALF;
    }
    fn receive_step(&mut self, bus: &mut impl Bus) {
        if self.rx.len > self.receive_depth {
            return;
        } // RX clock stretching at the programmed watermark
        let byte = bus.receive();
        self.rx.push(byte.into());
        self.read_count -= 1;
        if self.rx.len > self.receive_depth {
            self.isr |= RX_FULL;
        }
        if self.read_count == 0 {
            bus.nack();
            // Core uses TX_ERROR for final receive completion too.
            self.isr |= TX_ERROR;
            if self.read_stop {
                self.stop(bus);
            }
        }
    }
    /// Consume at most one address/data byte from the peripheral bus.
    pub fn step(&mut self, bus: &mut impl Bus) {
        if self.bus_end_pending {
            bus.end();
            self.bus_end_pending = false;
            return;
        }
        if self.unsupported || self.control & (ENABLE | TX_RESET) != ENABLE {
            return;
        }
        if self.read_count != 0 {
            self.receive_step(bus);
            return;
        }
        let old_len = self.tx.len;
        let Some(word) = self.tx.pop() else {
            return;
        };
        if self.tx.len == 0 {
            self.isr |= TX_EMPTY;
        }
        if old_len > 8 && self.tx.len <= 8 {
            self.isr |= TX_HALF;
        }
        if word & START != 0 {
            let address = ((word as u8) >> 1) & 0x7f;
            let reading = word & 1 != 0;
            if !bus.start(address, reading) {
                // START exists even when address is NACKed; terminate it.
                self.active = true;
                self.failed(bus);
                return;
            }
            self.active = true;
            self.reading = reading;
            if word & STOP != 0 {
                self.stop(bus);
            }
        } else if !self.active {
            self.failed(bus);
        } else if self.reading {
            self.read_count = word as u8;
            self.read_stop = word & STOP != 0;
            if self.read_count == 0 {
                self.failed(bus);
            } else {
                // PG090: this FIFO word is a receive count, not an I2C byte.
                // It must not add another nine-clock transfer interval.
                self.receive_step(bus);
            }
        } else {
            if !bus.send(word as u8) {
                self.failed(bus);
                return;
            }
            if word & STOP != 0 {
                self.stop(bus);
            }
        }
    }
    /// Little-endian register lanes. Invalid and reserved accesses are visible
    /// in the fault counter and do not alias valid registers.
    pub fn read(&mut self, offset: u32, size: u32) -> u32 {
        if !valid_access(offset, size) {
            self.fault();
            return 0;
        }
        let shift = (offset & 3) * 8;
        let value = match offset & !3 {
            0x1c => {
                if self.global {
                    0x8000_0000
                } else {
                    0
                }
            }
            0x20 => u32::from(self.isr),
            0x28 => u32::from(self.ier),
            0x40 => 0,
            0x100 => u32::from(self.control),
            0x104 => u32::from(self.status()),
            0x10c => {
                if shift != 0 {
                    0
                } else if let Some(v) = self.rx.pop() {
                    u32::from(v)
                } else {
                    self.fault();
                    0
                }
            }
            0x110 => u32::from(self.address),
            0x114 => u32::from(self.tx.len.saturating_sub(1)),
            0x118 => u32::from(self.rx.len.saturating_sub(1)),
            0x11c => u32::from(self.ten_bit_address),
            0x120 => u32::from(self.receive_depth),
            0x124 => u32::from(self.output),
            _ => {
                self.fault();
                0
            }
        };
        (value >> shift) & mask(size)
    }
    pub fn write(&mut self, offset: u32, size: u32, value: u32) {
        if !valid_access(offset, size) {
            self.fault();
            return;
        }
        let lane = (offset & 3) * 8;
        let value = (value & mask(size)) << lane;
        match offset & !3 {
            0x1c => {
                if lane + size * 8 == 32 {
                    self.global = value & 0x8000_0000 != 0;
                }
            }
            0x20 => {
                self.isr ^= value as u8;
                if self.unsupported {
                    self.isr |= TX_ERROR;
                }
                // These requests stay asserted until firmware removes their
                // documented FIFO throttle condition (PG090 interrupts 2/3/7).
                if self.tx.len <= 8 {
                    self.isr |= TX_HALF;
                }
                if self.active && !self.reading && self.tx.len == 0 {
                    self.isr |= TX_EMPTY;
                }
                if self.rx.len > self.receive_depth {
                    self.isr |= RX_FULL;
                }
                // PG090 Interrupt(4): the idle condition cannot be cleared
                // while true. In particular, STOP can occur between the
                // driver's ISR read and its toggle write. Do not lose that
                // completed transfer when the write uses the earlier value.
                if !self.bus_busy() {
                    self.isr |= NOT_BUSY;
                }
            }
            0x28 => {
                if lane == 0 {
                    self.ier = value as u8;
                }
            }
            0x40 => {
                if value == 0xa {
                    self.reset();
                }
            }
            0x100 => {
                if lane == 0 {
                    if value & u32::from(TX_RESET) != 0 {
                        self.tx = Fifo::default();
                        self.isr |= TX_EMPTY | TX_HALF;
                    }
                    if value & u32::from(ENABLE) == 0 && self.active {
                        self.bus_end_pending = true;
                        self.active = false;
                        self.read_count = 0;
                        self.isr |= NOT_BUSY;
                    }
                    self.control = value as u8 & 0x7f;
                    // Only ENABLE and FIFO_RESET belong to dynamic mode.
                    // Standard-master, manual direction/ACK/repeated START,
                    // and slave general-call mode require another engine.
                    // Fail closed until software reset, with TX_ERROR visible.
                    if value & !u32::from(ENABLE | TX_RESET) != 0 {
                        self.unsupported_request();
                    }
                }
            }
            0x108 => {
                if lane != 0 || !self.tx.push(value as u16 & 0x3ff) {
                    self.fault();
                }
            }
            0x110 => {
                if lane == 0 {
                    self.address = value as u8;
                    if value != 0 {
                        self.unsupported_request();
                    }
                }
            }
            0x11c => {
                if lane == 0 {
                    self.ten_bit_address = value as u8 & 7;
                    if value != 0 {
                        self.unsupported_request();
                    }
                }
            }
            0x120 => {
                if lane == 0 {
                    self.receive_depth = value as u8 & 15;
                    if self.rx.len > self.receive_depth {
                        self.isr |= RX_FULL;
                    }
                }
            }
            0x124 => {
                if lane == 0 {
                    self.output = value as u8;
                }
            }
            _ => self.fault(),
        }
    }
}
fn valid_access(offset: u32, size: u32) -> bool {
    matches!(size, 1 | 2 | 4) && offset < 0x10000 && (offset & 3) + size <= 4
}
fn mask(size: u32) -> u32 {
    match size {
        1 => 0xff,
        2 => 0xffff,
        _ => u32::MAX,
    }
}

#[repr(C)]
pub struct BusCallbacks {
    pub opaque: *mut c_void,
    pub start: extern "C" fn(*mut c_void, u8, bool) -> bool,
    pub send: extern "C" fn(*mut c_void, u8) -> bool,
    pub receive: extern "C" fn(*mut c_void) -> u8,
    pub nack: extern "C" fn(*mut c_void),
    pub end: extern "C" fn(*mut c_void),
}
impl Bus for BusCallbacks {
    fn start(&mut self, a: u8, r: bool) -> bool {
        (self.start)(self.opaque, a, r)
    }
    fn send(&mut self, b: u8) -> bool {
        (self.send)(self.opaque, b)
    }
    fn receive(&mut self) -> u8 {
        (self.receive)(self.opaque)
    }
    fn nack(&mut self) {
        (self.nack)(self.opaque);
    }
    fn end(&mut self) {
        (self.end)(self.opaque);
    }
}
#[no_mangle]
pub extern "C" fn md_iic_size() -> usize {
    core::mem::size_of::<Controller>()
}
/// # Safety
/// `state` is aligned, exclusively owned storage of at least `md_iic_size()` bytes.
#[no_mangle]
pub unsafe extern "C" fn md_iic_init(state: *mut Controller) {
    state.write(Controller::default());
    (*state).reset();
}
/// # Safety
/// `state` is an initialized, exclusively borrowed controller.
#[no_mangle]
pub unsafe extern "C" fn md_iic_reset(state: *mut Controller) {
    (*state).reset();
}
/// # Safety
/// `state` is an initialized, exclusively borrowed controller.
#[no_mangle]
pub unsafe extern "C" fn md_iic_read(state: *mut Controller, offset: u32, size: u32) -> u32 {
    (*state).read(offset, size)
}
/// # Safety
/// `state` is an initialized, exclusively borrowed controller.
#[no_mangle]
pub unsafe extern "C" fn md_iic_write(state: *mut Controller, offset: u32, size: u32, value: u32) {
    (*state).write(offset, size, value);
}
/// # Safety
/// `state` and `bus` are initialized and exclusively borrowed. All callbacks are valid.
#[no_mangle]
pub unsafe extern "C" fn md_iic_step(state: *mut Controller, bus: *mut BusCallbacks) {
    (*state).step(&mut *bus);
}
/// # Safety
/// `state` is an initialized controller without a concurrent writer.
#[no_mangle]
pub unsafe extern "C" fn md_iic_pending(state: *const Controller) -> bool {
    (*state).pending()
}
/// # Safety
/// `state` is an initialized controller without a concurrent writer.
#[no_mangle]
pub unsafe extern "C" fn md_iic_irq(state: *const Controller) -> bool {
    (*state).irq()
}
/// # Safety
/// `state` is an initialized controller without a concurrent writer.
#[no_mangle]
pub unsafe extern "C" fn md_iic_faults(state: *const Controller) -> u64 {
    (*state).fault_count()
}

#[cfg(not(any(test, feature = "host-std")))]
extern "C" {
    fn abort() -> !;
}

// QEMU links this panic-abort static library into its Linux host executable.
// The prebuilt Rust core archive retains an EH personality reference when
// QEMU exports host symbols. No unwind may cross this C interface. If a foreign
// unwinder ever reaches it, terminate instead of inventing unwind success.
#[cfg(not(any(test, feature = "host-std")))]
#[no_mangle]
pub extern "C" fn rust_eh_personality(
    _version: i32,
    _actions: u32,
    _exception_class: u64,
    _exception: *mut core::ffi::c_void,
    _context: *mut core::ffi::c_void,
) -> i32 {
    unsafe { abort() }
}

#[cfg(not(any(test, feature = "host-std")))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    unsafe { abort() }
}

#[cfg(test)]
mod tests;

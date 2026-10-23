//! ESP32-S3 FIFO master controller. The existing QEMU I2C bus supplies slave
//! responses. Unsupported modes stop with a fault; they do not return success.
use core::ffi::c_void;

const START: u32 = 1 << 5;
const RESET: u32 = 1 << 10;
const UPDATE: u32 = 1 << 11;
const DONE: u32 = 1 << 31;
const END: u32 = 1 << 3;
const BYTE: u32 = 1 << 4;
const COMPLETE: u32 = 1 << 7;
const TIMEOUT: u32 = 1 << 8;
const NACK: u32 = 1 << 10;
const ALL_IRQ: u32 = (1 << 18) - 1;

pub trait Bus {
    fn start(&mut self, address: u8, read: bool) -> bool;
    fn write(&mut self, byte: u8) -> bool;
    fn read(&mut self) -> Option<u8>;
    fn nack(&mut self);
    fn stop(&mut self);
}

#[derive(Default)]
struct Fifo {
    bytes: [u8; 32],
    head: u8,
    tail: u8,
    count: u8,
}
impl Fifo {
    fn push(&mut self, value: u8) -> bool {
        if self.count == 32 {
            return false;
        }
        self.bytes[self.tail as usize] = value;
        self.tail = (self.tail + 1) & 31;
        self.count += 1;
        true
    }
    fn pop(&mut self) -> Option<u8> {
        if self.count == 0 {
            return None;
        }
        let value = self.bytes[self.head as usize];
        self.head = (self.head + 1) & 31;
        self.count -= 1;
        Some(value)
    }
}

pub struct Controller {
    regs: [u32; 64],
    synchronized: [u32; 64],
    commands: [u32; 8],
    tx: Fifo,
    rx: Fifo,
    raw: u32,
    index: usize,
    remaining: Option<u8>,
    due: Option<u64>,
    active: bool,
    busy: bool,
    address_pending: bool,
    selected: bool,
    reading: bool,
    last_nack: bool,
    stop_pending: bool,
    faults: u64,
    bytes: u64,
}
impl Default for Controller {
    fn default() -> Self {
        let mut regs = [0; 64];
        regs[0] = 0;
        regs[1] = 0x10b;
        regs[3] = 0x10;
        regs[6] = 0x408b;
        regs[20] = 0x300;
        regs[21] = 1 << 21;
        regs[30] = 0x10;
        regs[31] = 0x10;
        Self {
            regs,
            synchronized: regs,
            commands: [0; 8],
            tx: Fifo::default(),
            rx: Fifo::default(),
            raw: 0,
            index: 0,
            remaining: None,
            due: None,
            active: false,
            busy: false,
            address_pending: false,
            selected: false,
            reading: false,
            last_nack: false,
            stop_pending: false,
            faults: 0,
            bytes: 0,
        }
    }
}
impl Controller {
    pub fn faults(&self) -> u64 {
        self.faults
    }
    pub fn bytes(&self) -> u64 {
        self.bytes
    }
    pub fn deadline(&self) -> Option<u64> {
        self.due
    }
    pub fn take_stop(&mut self) -> bool {
        core::mem::take(&mut self.stop_pending)
    }
    fn fault(&mut self) {
        self.faults = self.faults.saturating_add(1);
    }
    fn raw(&self) -> u32 {
        let mut value = self.raw;
        if self.regs[6] & (1 << 14) != 0 {
            if u32::from(self.rx.count) > (self.regs[6] & 31) {
                value |= 1;
            }
            if u32::from(self.tx.count) < ((self.regs[6] >> 5) & 31) {
                value |= 2;
            }
        }
        value
    }
    pub fn irq(&self) -> bool {
        self.raw() & self.regs[10] != 0
    }
    pub fn read(&mut self, address: u64, size: u32) -> u32 {
        if size != 4 || address & 3 != 0 || address >= 0x100 {
            self.fault();
            return 0;
        }
        match address / 4 {
            2 => {
                u32::from(self.last_nack)
                    | (u32::from(self.busy) << 4)
                    | (3 << 14)
                    | (u32::from(self.rx.count) << 8)
                    | (u32::from(self.tx.count) << 18)
            }
            5 => {
                u32::from(self.rx.head)
                    | (u32::from(self.rx.tail) << 5)
                    | (u32::from(self.tx.head) << 10)
                    | (u32::from(self.tx.tail) << 15)
            }
            7 => match self.rx.pop() {
                Some(v) => u32::from(v),
                None => {
                    self.raw |= 1 << 12;
                    self.fault();
                    0
                }
            },
            8 => self.raw(),
            9 => 0,
            11 => self.raw() & self.regs[10],
            n => self.regs[n as usize],
        }
    }
    pub fn write(&mut self, address: u64, size: u32, value: u32, now: u64) {
        if size != 4 || address & 3 != 0 || address >= 0x100 {
            self.fault();
            return;
        }
        let index = address as usize / 4;
        match index {
            1 => {
                self.regs[1] = value & 0x7fff & !(START | RESET | UPDATE);
                if value & RESET != 0 {
                    self.end();
                    self.raw = 0;
                }
                if value & UPDATE != 0 {
                    self.synchronized = self.regs;
                }
                if value & START != 0 {
                    self.start(now);
                }
            }
            6 => {
                self.regs[6] = value & 0x7fff & !((1 << 12) | (1 << 13));
                if value & (1 << 12) != 0 {
                    self.rx = Fifo::default();
                }
                if value & (1 << 13) != 0 {
                    self.tx = Fifo::default();
                }
            }
            7 => {
                if !self.tx.push(value as u8) {
                    self.raw |= 1 << 11;
                    self.fault();
                }
            }
            8 | 9 => self.raw &= !(value & ALL_IRQ),
            10 => self.regs[10] = value & ALL_IRQ,
            2 | 5 | 11 => self.fault(),
            22..=29 => {
                if self.active {
                    self.fault();
                } else {
                    self.regs[index] = value & (DONE | 0x3fff);
                }
            }
            21 => {
                if self.active {
                    self.fail(TIMEOUT);
                }
                self.regs[index] = value & 0x3f_ffff;
                self.synchronized[index] = self.regs[index];
            }
            0 | 3 | 4 | 12 | 13 | 14 | 16..=20 | 30..=33 | 62 => self.regs[index] = value,
            _ => self.fault(),
        }
    }
    fn end(&mut self) {
        self.stop_pending |= self.busy || self.selected;
        self.active = false;
        self.busy = false;
        self.selected = false;
        self.address_pending = false;
        self.due = None;
        self.remaining = None;
    }
    fn fail(&mut self, irq: u32) {
        self.raw |= irq;
        self.fault();
        self.end();
    }
    fn start(&mut self, now: u64) {
        // TRM27.4.14: another TRANS_START write refreshes commands from slot0.
        // Keep FIFO and physical bus state. Replace the pending command timer;
        // do not invent completion or timeout on an active write-trigger.
        let control = self.synchronized[1];
        // Slave, LSB-first, non-FIFO and 10-bit modes are explicit gaps.
        if control & (1 << 4) == 0
            || control & ((1 << 6) | (1 << 7)) != 0
            || self.synchronized[4] & (1 << 31) != 0
            || self.regs[6] & (1 << 10) != 0
            || self.synchronized[21] & (1 << 21) == 0
        {
            self.fail(TIMEOUT);
            return;
        }
        self.commands.copy_from_slice(&self.regs[22..30]);
        self.index = 0;
        self.remaining = None;
        self.active = true;
        self.raw |= 1 << 9;
        self.schedule_next(now);
    }
    fn ns(&self, cycles: u64) -> Option<u64> {
        let clock = self.synchronized[21];
        let a = u64::from((clock >> 8) & 63);
        let b = u64::from((clock >> 14) & 63);
        if a != 0 && b == 0 {
            return None;
        }
        let denominator = b.max(1);
        let divider = (u64::from(clock & 255) + 1) * denominator + a;
        let hz = if clock & (1 << 20) == 0 {
            40_000_000
        } else {
            17_500_000
        };
        Some(
            (cycles * divider * 1_000_000_000)
                .div_ceil(hz * denominator)
                .max(1),
        )
    }
    fn schedule(&mut self, now: u64, cycles: u64) {
        self.due = self.ns(cycles).and_then(|delta| now.checked_add(delta));
        if self.due.is_none() {
            self.fail(TIMEOUT);
        }
    }
    fn byte_cycles(&self) -> u64 {
        let low = u64::from(self.synchronized[0] & 0x1ff);
        let high = u64::from(self.synchronized[14] & 0x1ff);
        let wait = u64::from((self.synchronized[14] >> 9) & 0x7f);
        9 * (low + high + wait + 3)
    }
    fn schedule_next(&mut self, now: u64) {
        let cycles = match self.commands.get(self.index).map(|c| (c >> 11) & 7) {
            Some(6) => {
                u64::from((self.synchronized[16] & 0x1ff) + (self.synchronized[17] & 0x1ff) + 2)
            }
            Some(2) => {
                u64::from((self.synchronized[18] & 0x1ff) + (self.synchronized[19] & 0x1ff) + 2)
            }
            Some(1 | 3) => self.byte_cycles(),
            _ => 1,
        };
        self.schedule(now, cycles);
    }
    fn done(&mut self) {
        self.regs[22 + self.index] |= DONE;
        self.index += 1;
        self.remaining = None;
    }
    /// Execute at most one bus action. No firmware instruction or expected
    /// response enters this controller. Delay is based on programmed clocks.
    pub fn advance(&mut self, now: u64, bus: &mut impl Bus) {
        if !self.active || self.due.is_none_or(|due| now < due) {
            return;
        }
        self.due = None;
        if self.index >= 8 {
            self.fail(TIMEOUT);
        } else {
            let cmd = self.commands[self.index];
            let opcode = (cmd >> 11) & 7;
            match opcode {
                6 => {
                    self.busy = true;
                    self.address_pending = true;
                    self.done();
                }
                2 => {
                    self.done();
                    self.raw |= COMPLETE;
                    self.end();
                }
                4 => {
                    self.done();
                    self.raw |= END;
                    self.active = false;
                }
                1 | 3 => {
                    let remaining = self.remaining.unwrap_or(cmd as u8);
                    if remaining == 0 || !self.busy {
                        self.fail(TIMEOUT);
                    } else if opcode == 1 {
                        self.send(cmd, remaining, bus);
                    } else {
                        self.receive(cmd, remaining, bus);
                    }
                }
                _ => self.fail(TIMEOUT),
            }
            if self.active {
                self.schedule_next(now);
            }
        }
        if self.take_stop() {
            bus.stop();
        }
    }
    fn send(&mut self, cmd: u32, remaining: u8, bus: &mut impl Bus) {
        let Some(value) = self.tx.pop() else {
            self.fail(1 << 6);
            return;
        };
        let ack = if self.address_pending {
            self.address_pending = false;
            self.reading = value & 1 != 0;
            // 10-bit address prefixes cannot be a 7-bit device address.
            if value >> 1 >= 0x78 {
                self.fail(TIMEOUT);
                return;
            }
            self.selected = bus.start(value >> 1, self.reading);
            self.selected
        } else if self.selected && !self.reading {
            bus.write(value)
        } else {
            false
        };
        self.last_nack = !ack;
        self.bytes = self.bytes.saturating_add(1);
        self.raw |= BYTE;
        if cmd & (1 << 8) != 0 && self.last_nack != (cmd & (1 << 9) != 0) {
            // An actual slave NACK is normal bus behavior, not a model fault.
            self.raw |= NACK;
            self.end();
            return;
        }
        if remaining == 1 {
            self.done();
        } else {
            self.remaining = Some(remaining - 1);
        }
    }
    fn receive(&mut self, cmd: u32, remaining: u8, bus: &mut impl Bus) {
        if !self.selected || !self.reading {
            self.fail(TIMEOUT);
            return;
        }
        if self.rx.count == 32 {
            self.fail(1 << 2);
            return;
        }
        let Some(value) = bus.read() else {
            self.fail(TIMEOUT);
            return;
        };
        self.rx.push(value);
        if cmd & (1 << 10) != 0 {
            bus.nack();
        }
        self.bytes = self.bytes.saturating_add(1);
        self.raw |= BYTE;
        if remaining == 1 {
            self.done();
        } else {
            self.remaining = Some(remaining - 1);
        }
    }
}

type Callback = unsafe extern "C" fn(*mut c_void, u32, u32) -> i32;
struct ForeignBus {
    opaque: *mut c_void,
    callback: Callback,
}
impl Bus for ForeignBus {
    fn start(&mut self, a: u8, read: bool) -> bool {
        unsafe { (self.callback)(self.opaque, 0, u32::from(a) | (u32::from(read) << 8)) == 0 }
    }
    fn write(&mut self, b: u8) -> bool {
        unsafe { (self.callback)(self.opaque, 1, u32::from(b)) == 0 }
    }
    fn read(&mut self) -> Option<u8> {
        u8::try_from(unsafe { (self.callback)(self.opaque, 2, 0) }).ok()
    }
    fn stop(&mut self) {
        unsafe {
            (self.callback)(self.opaque, 3, 0);
        }
    }
    fn nack(&mut self) {
        unsafe {
            (self.callback)(self.opaque, 4, 0);
        }
    }
}
#[no_mangle]
pub extern "C" fn md_i2c_size() -> usize {
    core::mem::size_of::<Controller>()
}
/// # Safety
/// `state` is aligned exclusive writable storage of md_i2c_size bytes.
#[no_mangle]
pub unsafe extern "C" fn md_i2c_init(state: *mut Controller) {
    state.write(Controller::default());
}
/// # Safety
/// `state` is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_i2c_read(state: *mut Controller, addr: u64, size: u32) -> u32 {
    (*state).read(addr, size)
}
/// # Safety
/// `state` is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_i2c_write(
    state: *mut Controller,
    addr: u64,
    size: u32,
    value: u32,
    now: u64,
) {
    (*state).write(addr, size, value, now);
}
/// # Safety
/// State and callback context are exclusively owned. Callback lives for this call.
#[no_mangle]
pub unsafe extern "C" fn md_i2c_advance(
    state: *mut Controller,
    now: u64,
    opaque: *mut c_void,
    callback: Callback,
) {
    (*state).advance(now, &mut ForeignBus { opaque, callback });
}
/// # Safety
/// `state` is initialized and exclusively owned for this call.
#[no_mangle]
pub unsafe extern "C" fn md_i2c_take_stop(state: *mut Controller) -> bool {
    (*state).take_stop()
}
/// # Safety
/// `state` is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_i2c_deadline(state: *const Controller) -> u64 {
    (*state).deadline().unwrap_or(u64::MAX)
}
/// # Safety
/// `state` is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_i2c_irq(state: *const Controller) -> bool {
    (*state).irq()
}
/// # Safety
/// `state` is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_i2c_faults(state: *const Controller) -> u64 {
    (*state).faults()
}
/// # Safety
/// `state` is initialized and not being modified.
#[no_mangle]
pub unsafe extern "C" fn md_i2c_bytes(state: *const Controller) -> u64 {
    (*state).bytes()
}

#[cfg(test)]
mod tests;

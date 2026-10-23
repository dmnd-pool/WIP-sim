//! PL353 NAND command aperture and 512-byte Hamming ECC engine.
//! The NAND chip remains a separate backend. Time is supplied explicitly.
//! This implements the x8 NAND command path used by the archived Zynq driver.
//! SRAM, x16, multiple interfaces, DMA and migration are not supported.

pub trait Bus {
    fn command(&mut self, byte: u8);
    fn address(&mut self, byte: u8);
    fn read(&mut self) -> u8;
    fn write(&mut self, byte: u8);
    fn deselect(&mut self);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Unsupported,
    Busy,
    AddressPhase,
}

#[derive(Default)]
#[repr(C)]
pub struct Controller {
    cycles: u32,
    opmode: u32,
    ecc_config: u32,
    ecc_commands: [u32; 2],
    ecc_values: [u32; 4],
    ecc_count: u32,
    ecc_active: bool,
    address_left: u8,
    command_address: u32,
    command: u8,
    ready_event: bool,
    interrupt_enabled: bool,
    busy_until: u64,
    faults: u64,
    onfi: bool,
    feature_bytes: u8,
    resume_command: u8,
}

impl Controller {
    fn command_tuple(&self, address: u32) -> bool {
        let command = (address >> 3) as u8;
        let end = (address >> 11) as u8;
        let valid = address & (1 << 20) != 0;
        let cycles = (address >> 21) & 7;
        address & 7 == 0
            && match command {
                0x00 => end == 0x30 && valid && cycles == 5,
                0x05 => end == 0xe0 && valid && cycles == 2,
                0x80 => end == 0x10 && !valid && cycles == 5,
                0x60 => end == 0xd0 && valid && cycles == 3,
                0x90 => end == 0 && !valid && cycles == 1,
                0xec | 0xee | 0xef if self.onfi => end == 0 && !valid && cycles == 1,
                0x70 | 0xff => end == 0 && !valid && cycles == 0,
                _ => false,
            }
    }
    fn data_tuple(&self, address: u32, write: bool) -> bool {
        if address & 7 != 0 {
            return false;
        }
        if write && self.command != 0x80 && !(self.onfi && self.command == 0xef) {
            return false;
        }
        if !(write
            || matches!(self.command, 0x00 | 0x05 | 0x90 | 0x70)
            || self.onfi && matches!(self.command, 0xec | 0xee))
        {
            return false;
        }
        address & (1 << 20) == 0 || (write && self.command == 0x80 && (address >> 11) as u8 == 0x10)
    }
    pub fn reset(&mut self, bus: &mut impl Bus) {
        let chip_deadline = self.busy_until;
        let onfi = self.onfi;
        bus.deselect();
        *self = Self::default();
        // Resetting the controller does not reset the external NAND chip.
        // Its read/reset/program/erase interval remains in progress.
        self.busy_until = chip_deadline;
        self.onfi = onfi;
    }
    pub fn enable_onfi(&mut self) {
        self.onfi = true;
    }
    pub fn tick(&mut self, now_ns: u64) {
        if self.busy_until != 0 && now_ns >= self.busy_until {
            self.busy_until = 0;
            self.ready_event = true;
        }
    }
    pub fn deadline(&self) -> u64 {
        self.busy_until
    }
    pub fn chip_busy_until(&mut self, deadline: u64) {
        if deadline != 0 {
            self.busy_until = deadline;
            self.ready_event = false;
        }
    }
    pub fn irq(&self) -> bool {
        self.interrupt_enabled && self.ready_event
    }
    pub fn faults(&self) -> u64 {
        self.faults
    }
    fn fail<T>(&mut self, fault: Fault) -> Result<T, Fault> {
        self.faults = self.faults.saturating_add(1);
        Err(fault)
    }
    pub fn read_register(&mut self, offset: u32) -> Result<u32, Fault> {
        Ok(match offset {
            0x00 => {
                if self.ready_event {
                    0x40
                } else {
                    0
                }
            }
            0x14 => self.cycles,
            0x18 => self.opmode,
            0x400 => {
                if self.ecc_active {
                    0x40
                } else {
                    0
                }
            }
            0x404 => self.ecc_config,
            0x408 => self.ecc_commands[0],
            0x40c => self.ecc_commands[1],
            0x418 | 0x41c | 0x420 | 0x424 => self.ecc_values[((offset - 0x418) / 4) as usize],
            _ => return self.fail(Fault::Unsupported),
        })
    }
    pub fn write_register(&mut self, offset: u32, value: u32) -> Result<(), Fault> {
        match offset {
            0x08 => {
                if value != 2 {
                    return self.fail(Fault::Unsupported);
                }
                self.interrupt_enabled = true;
            }
            0x0c => {
                if value & !0x52 != 0 {
                    return self.fail(Fault::Unsupported);
                }
                if value & 2 != 0 {
                    self.interrupt_enabled = false;
                }
                if value & 0x10 != 0 {
                    self.ready_event = false;
                }
                // ECC interrupt disable is accepted. Enabling it is unsupported.
            }
            0x10 => {
                // UpdateRegs, chip0, interface1; no fabricated device operation.
                if value != 0x0240_0000 {
                    return self.fail(Fault::Unsupported);
                }
            }
            0x14 => self.cycles = value,
            0x18 => {
                if value != 0 {
                    return self.fail(Fault::Unsupported);
                }
                self.opmode = value;
            }
            0x404 => {
                // page-size codes0..3, ECC bypass/APB modes0..1, read-at-end.
                if value & !0x1f != 0 || (value >> 2) & 3 > 1 {
                    return self.fail(Fault::Unsupported);
                }
                self.ecc_config = value;
            }
            0x408 => {
                if value != 0x0130_0080 {
                    return self.fail(Fault::Unsupported);
                }
                self.ecc_commands[0] = value;
            }
            0x40c => {
                if value != 0x01e0_0585 {
                    return self.fail(Fault::Unsupported);
                }
                self.ecc_commands[1] = value;
            }
            _ => return self.fail(Fault::Unsupported),
        }
        Ok(())
    }
    fn begin_ecc(&mut self) {
        self.ecc_values = [0; 4];
        self.ecc_count = 0;
        self.ecc_active = self.ecc_config & 0xc == 4;
    }
    fn ecc_byte(&mut self, byte: u8) {
        if !self.ecc_active {
            return;
        }
        let block = self.ecc_count / 512;
        if block >= 4 {
            self.ecc_active = false;
            return;
        }
        let byte_index = self.ecc_count % 512;
        for bit in 0..8 {
            if byte & (1 << bit) != 0 {
                let index = byte_index * 8 + bit;
                self.ecc_values[block as usize] ^= index | ((!index & 0xfff) << 12);
            }
        }
        self.ecc_count += 1;
        if self.ecc_count.is_multiple_of(512) {
            self.ecc_values[block as usize] |= 0x4000_0000;
        }
    }
    fn schedule(&mut self, command: u8, now_ns: u64) {
        // Typical device times from Micron AACWP RevD12/06. They are a
        // configurable-model baseline, not measured board calibration.
        let delay = match command {
            0xff => 5000, // reset timing remains an explicit model baseline
            0x30 | 0xec => 25_000,
            0xee | 0xef => 1_000, // M69A RevK Table30 tFEAT maximum
            0x10 => 300_000,
            0xd0 => 2_000_000,
            _ => 0,
        };
        if delay != 0 {
            self.ready_event = false;
            self.busy_until = now_ns.saturating_add(delay);
        }
    }
    fn address_data(&mut self, value: u32, size: u32, bus: &mut impl Bus, now_ns: u64) {
        for i in 0..size.min(u32::from(self.address_left)) {
            bus.address((value >> (i * 8)) as u8);
            self.address_left -= 1;
        }
        if self.address_left == 0 {
            if self.command_address & (1 << 20) != 0 {
                let end = (self.command_address >> 11) as u8;
                bus.command(end);
                self.schedule(end, now_ns);
            } else if self.command != 0xef {
                self.schedule(self.command, now_ns);
            }
        }
    }
    pub fn write_aperture(
        &mut self,
        address: u32,
        value: u32,
        size: u32,
        now_ns: u64,
        bus: &mut impl Bus,
    ) -> Result<(), Fault> {
        self.tick(now_ns);
        if !matches!(size, 1 | 2 | 4) {
            return self.fail(Fault::Unsupported);
        }
        if address & (1 << 19) == 0 {
            let command = (address >> 3) as u8;
            if self.onfi
                && address == 0
                && self.command == 0x70
                && matches!(self.resume_command, 0xec | 0xee)
            {
                if self.busy_until != 0 {
                    return self.fail(Fault::Busy);
                }
                bus.command(0);
                self.command = self.resume_command;
                self.resume_command = 0;
                return Ok(());
            }
            if !self.command_tuple(address) {
                return self.fail(Fault::Unsupported);
            }
            if self.busy_until != 0 && command != 0xff && command != 0x70 {
                return self.fail(Fault::Busy);
            }
            if command == 0xff {
                self.address_left = 0;
            }
            if self.address_left != 0 {
                if address != self.command_address {
                    return self.fail(Fault::AddressPhase);
                }
            } else {
                if command == 0x70 && matches!(self.command, 0xec | 0xee) {
                    self.resume_command = self.command;
                } else if command != 0x70 {
                    self.resume_command = 0;
                }
                self.command_address = address;
                self.command = command;
                self.feature_bytes = 0;
                self.address_left = ((address >> 21) & 7) as u8;
                if self.address_left > 5 {
                    return self.fail(Fault::AddressPhase);
                }
                if command == 0xff {
                    self.busy_until = 0;
                }
                if command == 0 || command == 0x80 {
                    self.begin_ecc();
                }
                bus.command(command);
            }
            self.address_data(value, size, bus, now_ns);
        } else {
            if !self.data_tuple(address, true) {
                return self.fail(Fault::Unsupported);
            }
            if self.busy_until != 0 {
                return self.fail(Fault::Busy);
            }
            if self.address_left != 0 {
                return self.fail(Fault::AddressPhase);
            }
            if self.command == 0xef && u32::from(self.feature_bytes) + size > 4 {
                return self.fail(Fault::Unsupported);
            }
            for i in 0..size {
                let byte = (value >> (i * 8)) as u8;
                bus.write(byte);
                self.ecc_byte(byte);
            }
            if self.command == 0xef {
                self.feature_bytes += size as u8;
                if self.feature_bytes == 4 {
                    self.schedule(0xef, now_ns);
                }
            }
            self.finish_data(address, now_ns, bus);
        }
        Ok(())
    }
    fn finish_data(&mut self, address: u32, now_ns: u64, bus: &mut impl Bus) {
        if address & (1 << 10) != 0 {
            self.ecc_active = false;
        }
        if address & (1 << 20) != 0 {
            let end = (address >> 11) as u8;
            bus.command(end);
            self.schedule(end, now_ns);
            // A completed program needs a new SEQIN transaction. Do not
            // accept more data against the chip's previous page buffer.
            self.command = end;
        }
        if address & (1 << 21) != 0 {
            bus.deselect();
        }
    }
    pub fn read_aperture(
        &mut self,
        address: u32,
        size: u32,
        now_ns: u64,
        bus: &mut impl Bus,
    ) -> Result<u32, Fault> {
        self.tick(now_ns);
        if !matches!(size, 1 | 2 | 4) || address & (1 << 19) == 0 {
            return self.fail(Fault::Unsupported);
        }
        if !self.data_tuple(address, false) {
            return self.fail(Fault::Unsupported);
        }
        if self.address_left != 0 {
            return self.fail(Fault::AddressPhase);
        }
        if self.busy_until != 0 && self.command != 0x70 {
            return self.fail(Fault::Busy);
        }
        let mut value = 0;
        for i in 0..size {
            let mut byte = bus.read();
            // The reused NAND storage engine has no timing. This controller
            // owns the declared virtual busy interval and masks READY then.
            if self.command == 0x70 && self.busy_until != 0 {
                byte &= !0x40;
            }
            value |= u32::from(byte) << (i * 8);
            self.ecc_byte(byte);
        }
        self.finish_data(address, now_ns, bus);
        Ok(value)
    }
}

#[repr(C)]
pub struct FfiBus {
    opaque: *mut core::ffi::c_void,
    output: extern "C" fn(*mut core::ffi::c_void, u32, u8),
    input: extern "C" fn(*mut core::ffi::c_void) -> u8,
    end: extern "C" fn(*mut core::ffi::c_void),
}
impl Bus for FfiBus {
    fn command(&mut self, b: u8) {
        (self.output)(self.opaque, 0, b);
    }
    fn address(&mut self, b: u8) {
        (self.output)(self.opaque, 1, b);
    }
    fn write(&mut self, b: u8) {
        (self.output)(self.opaque, 2, b);
    }
    fn read(&mut self) -> u8 {
        (self.input)(self.opaque)
    }
    fn deselect(&mut self) {
        (self.end)(self.opaque);
    }
}
#[no_mangle]
pub extern "C" fn md_pl353_size() -> usize {
    core::mem::size_of::<Controller>()
}
/// # Safety
/// `state` must point to aligned storage of at least `md_pl353_size()` bytes.
#[no_mangle]
pub unsafe extern "C" fn md_pl353_init(state: *mut Controller) {
    unsafe {
        state.write(Controller::default());
    }
}
/// # Safety
/// state is an initialized uniquely owned controller, before guest execution.
#[no_mangle]
pub unsafe extern "C" fn md_pl353_enable_onfi(state: *mut Controller) {
    unsafe {
        (&mut *state).enable_onfi();
    }
}
/// # Safety
/// Both arguments must refer to live unique objects during this call.
#[no_mangle]
pub unsafe extern "C" fn md_pl353_reset(state: *mut Controller, bus: *mut FfiBus) {
    unsafe {
        (&mut *state).reset(&mut *bus);
    }
}
/// # Safety
/// `state` must refer to a live unique controller.
#[no_mangle]
pub unsafe extern "C" fn md_pl353_tick(state: *mut Controller, now_ns: u64) {
    unsafe {
        (&mut *state).tick(now_ns);
    }
}
/// # Safety
/// `state` must refer to a live unique controller.
#[no_mangle]
pub unsafe extern "C" fn md_pl353_chip_busy_until(state: *mut Controller, deadline: u64) {
    unsafe {
        (&mut *state).chip_busy_until(deadline);
    }
}
/// # Safety
/// `state` must refer to a live controller.
#[no_mangle]
pub unsafe extern "C" fn md_pl353_deadline(state: *const Controller) -> u64 {
    unsafe { (&*state).deadline() }
}
/// # Safety
/// `state` must refer to a live controller.
#[no_mangle]
pub unsafe extern "C" fn md_pl353_irq(state: *const Controller) -> bool {
    unsafe { (&*state).irq() }
}
/// # Safety
/// `state` must refer to a live controller.
#[no_mangle]
pub unsafe extern "C" fn md_pl353_faults(state: *const Controller) -> u64 {
    unsafe { (&*state).faults() }
}
/// # Safety
/// `state`, `bus` and `value` must be valid unique pointers.
#[no_mangle]
pub unsafe extern "C" fn md_pl353_access(
    state: *mut Controller,
    bus: *mut FfiBus,
    registers: bool,
    write: bool,
    address: u32,
    size: u32,
    value: *mut u32,
    now_ns: u64,
) -> bool {
    let (s, b, v) = unsafe { (&mut *state, &mut *bus, &mut *value) };
    s.tick(now_ns);
    let result = if registers {
        if size != 4 || address & 3 != 0 {
            s.fail(Fault::Unsupported)
        } else if write {
            s.write_register(address, *v).map(|()| 0)
        } else {
            s.read_register(address)
        }
    } else if write {
        s.write_aperture(address, *v, size, now_ns, b).map(|()| 0)
    } else {
        s.read_aperture(address, size, now_ns, b)
    };
    match result {
        Ok(value) => {
            if !write {
                *v = value;
            }
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Wire {
        output: std::vec::Vec<(u8, u8)>,
        input: std::collections::VecDeque<u8>,
        ends: u32,
    }
    impl Bus for Wire {
        fn command(&mut self, b: u8) {
            self.output.push((0, b));
        }
        fn address(&mut self, b: u8) {
            self.output.push((1, b));
        }
        fn write(&mut self, b: u8) {
            self.output.push((2, b));
        }
        fn read(&mut self) -> u8 {
            self.input.pop_front().unwrap_or(0xff)
        }
        fn deselect(&mut self) {
            self.ends += 1;
        }
    }
    #[test]
    fn onfi_is_explicit_and_features_wait_for_all_four_real_bytes() {
        let mut s = Controller::default();
        let mut bus = Wire::default();
        let set = (1 << 21) | (0xef << 3);
        assert_eq!(
            s.write_aperture(set, 0x90, 4, 0, &mut bus),
            Err(Fault::Unsupported)
        );
        assert!(bus.output.is_empty());
        s.enable_onfi();
        s.write_aperture(set, 0x90, 4, 0, &mut bus).unwrap();
        assert_eq!(s.deadline(), 0);
        s.write_aperture(1 << 19, 0, 2, 100, &mut bus).unwrap();
        assert_eq!(s.deadline(), 0);
        s.write_aperture(1 << 19, 0, 2, 200, &mut bus).unwrap();
        assert_eq!(s.deadline(), 1200);
        let before = bus.output.clone();
        assert_eq!(
            s.write_aperture(1 << 19, 0, 1, 1200, &mut bus),
            Err(Fault::Unsupported)
        );
        assert_eq!(bus.output, before);
        s.write_aperture((1 << 21) | (0xec << 3), 0, 4, 1200, &mut bus)
            .unwrap();
        assert_eq!(s.deadline(), 26200);
        s.write_aperture(0x70 << 3, 0, 4, 1300, &mut bus).unwrap();
        assert_eq!(s.write_aperture(0, 0, 4, 26199, &mut bus), Err(Fault::Busy));
        s.write_aperture(0, 0, 4, 26200, &mut bus).unwrap();
        bus.input.push_back(b'O');
        assert_eq!(
            s.read_aperture(1 << 19, 1, 26200, &mut bus),
            Ok(u32::from(b'O'))
        );
        s.reset(&mut bus);
        s.write_aperture((1 << 21) | (0xee << 3), 0x90, 4, 27000, &mut bus)
            .unwrap();
        assert_eq!(s.deadline(), 28000);
    }
    #[test]
    fn split_five_address_bytes_and_real_read_completion() {
        let mut s = Controller::default();
        let mut bus = Wire::default();
        let address = (5 << 21) | (1 << 20) | (0x30 << 11);
        s.write_aperture(address, 0x1234_0567, 4, 100, &mut bus)
            .unwrap();
        assert_eq!(s.deadline(), 0);
        assert_eq!(
            bus.output,
            [(0, 0), (1, 0x67), (1, 5), (1, 0x34), (1, 0x12)]
        );
        s.write_aperture(address, 2, 4, 200, &mut bus).unwrap();
        assert_eq!(&bus.output[5..], [(1, 2), (0, 0x30)]);
        assert_eq!(s.deadline(), 25200);
        assert_eq!(
            s.read_aperture(1 << 19, 4, 25199, &mut bus),
            Err(Fault::Busy)
        );
        bus.input.extend([1, 2, 3, 4]);
        assert_eq!(
            s.read_aperture((1 << 19) | (1 << 21), 4, 25200, &mut bus),
            Ok(0x04030201)
        );
        assert_eq!(bus.ends, 1);
        assert_eq!(s.read_register(0).unwrap(), 0x40);
        s.write_register(0xc, 0x10).unwrap();
        assert_eq!(s.read_register(0).unwrap(), 0);
    }
    #[test]
    fn reset_status_timing_and_interrupt_clear() {
        let mut s = Controller::default();
        let mut bus = Wire::default();
        s.write_register(0x8, 2).unwrap();
        s.write_aperture(0xff << 3, 0, 4, 100, &mut bus).unwrap();
        s.write_aperture(0x70 << 3, 0, 4, 100, &mut bus).unwrap();
        bus.input.extend([0xc0, 0xc0]);
        assert_eq!(s.read_aperture(1 << 19, 1, 5099, &mut bus), Ok(0x80));
        assert!(!s.irq());
        assert_eq!(s.read_aperture(1 << 19, 1, 5100, &mut bus), Ok(0xc0));
        assert!(s.irq());
        s.write_register(0xc, 0x10).unwrap();
        assert!(!s.irq());
        s.reset(&mut bus);
        assert_eq!(s.deadline(), 0);
        assert_eq!(bus.ends, 1);
    }
    #[test]
    fn controller_reset_does_not_complete_a_pending_chip_operation() {
        let mut s = Controller::default();
        let mut bus = Wire::default();
        s.reset(&mut bus);
        // The binding restores the independently owned NAND chip deadline.
        s.chip_busy_until(300_000);
        assert_eq!(
            s.write_aperture((1 << 21) | (0x90 << 3), 0, 4, 299_999, &mut bus),
            Err(Fault::Busy)
        );
        assert!(bus.output.is_empty());
        s.tick(300_000);
        s.write_aperture((1 << 21) | (0x90 << 3), 0, 4, 300_000, &mut bus)
            .unwrap();
        assert_eq!(bus.output, [(0, 0x90), (1, 0)]);
        for (command, expected) in [
            (0xff << 3, 5000),
            ((5 << 21) | (1 << 20) | (0x30 << 11), 25000),
        ] {
            let mut s = Controller::default();
            let mut bus = Wire::default();
            s.write_aperture(command, 0, 4, 0, &mut bus).unwrap();
            if expected == 25000 {
                s.write_aperture(command, 0, 4, 0, &mut bus).unwrap();
            }
            s.reset(&mut bus);
            assert_eq!(s.deadline(), expected);
            assert_eq!(
                s.write_aperture((1 << 21) | (0x90 << 3), 0, 4, expected - 1, &mut bus),
                Err(Fault::Busy)
            );
            s.tick(expected);
            assert_eq!(s.read_register(0), Ok(0x40));
        }
    }
    #[test]
    fn page_program_data_order_end_command_and_reset_abort() {
        let mut s = Controller::default();
        let mut bus = Wire::default();
        let address = (5 << 21) | (0x10 << 11) | (0x80 << 3);
        s.write_aperture(address, 0, 4, 0, &mut bus).unwrap();
        assert_eq!(
            // A legal READID transaction cannot interrupt an address phase.
            s.write_aperture((1 << 21) | (0x90 << 3), 0, 4, 0, &mut bus),
            Err(Fault::AddressPhase)
        );
        s.write_aperture(address, 0, 4, 0, &mut bus).unwrap();
        let last = (1 << 19) | (1 << 20) | (1 << 21) | (0x10 << 11);
        s.write_aperture(last, 0x76543210, 4, 1, &mut bus).unwrap();
        assert_eq!(
            &bus.output[6..],
            [(2, 0x10), (2, 0x32), (2, 0x54), (2, 0x76), (0, 0x10)]
        );
        assert_eq!(s.deadline(), 300001);
        assert_eq!(bus.ends, 1);
        let before = bus.output.clone();
        assert_eq!(
            s.write_aperture(last, 0, 4, 300001, &mut bus),
            Err(Fault::Unsupported)
        );
        assert_eq!(bus.output, before);
        s.write_aperture(0xff << 3, 0, 4, 300002, &mut bus).unwrap();
        assert_eq!(s.deadline(), 305002);
    }
    #[test]
    fn hamming_code_locates_each_single_bit_independently() {
        // The archived driver's correction contract decodes odd/even parity
        // halves as a 12-bit (byte*8+bit) position. Exhaust all 4096 positions.
        for position in 0..4096u32 {
            let mut s = Controller::default();
            s.write_register(0x404, 0x17).unwrap();
            s.begin_ecc();
            for byte in 0..512 {
                s.ecc_byte(if byte == position / 8 {
                    1 << (position % 8)
                } else {
                    0
                });
            }
            s.ecc_active = false;
            let code = s.read_register(0x418).unwrap();
            assert_eq!(code >> 24, 0x40);
            let odd = code & 0xfff;
            let even = (code >> 12) & 0xfff;
            assert_eq!(odd, !even & 0xfff);
            assert_eq!(odd / 8, position / 8);
            assert_eq!(odd % 8, position % 8);
        }
        for byte in [0, 0xff] {
            let mut s = Controller::default();
            s.write_register(0x404, 0x17).unwrap();
            s.begin_ecc();
            for _ in 0..512 {
                s.ecc_byte(byte);
            }
            assert_eq!(s.ecc_values[0], 0x40000000);
        }
    }
    #[test]
    fn unsupported_modes_are_errors_without_bus_transactions() {
        let mut s = Controller::default();
        let mut bus = Wire::default();
        assert_eq!(s.write_register(0x18, 1), Err(Fault::Unsupported));
        assert_eq!(s.write_register(0x404, 8), Err(Fault::Unsupported));
        assert_eq!(
            s.write_aperture((2 << 21) | (0x85 << 3), 0, 4, 0, &mut bus),
            Err(Fault::Unsupported)
        );
        assert_eq!(
            s.write_aperture(0xec << 3, 0, 4, 0, &mut bus),
            Err(Fault::Unsupported)
        );
        assert!(bus.output.is_empty());
        assert_eq!(s.faults(), 4);
    }
    #[test]
    fn invalid_command_tuples_and_data_directions_have_no_bus_effect() {
        let mut s = Controller::default();
        let mut bus = Wire::default();
        // READ0 must never issue an erase confirm, even with valid addresses.
        let invalid_read = (5 << 21) | (1 << 20) | (0xd0 << 11);
        assert_eq!(
            s.write_aperture(invalid_read, 0, 4, 0, &mut bus),
            Err(Fault::Unsupported)
        );
        assert!(bus.output.is_empty());
        assert_eq!(s.deadline(), 0);
        assert_eq!(s.address_left, 0);
        s.write_aperture(0x70 << 3, 0, 4, 0, &mut bus).unwrap();
        let before = bus.output.clone();
        bus.input.push_back(0xc0);
        for end in [0x10, 0xd0] {
            let data = (1 << 19) | (1 << 20) | (end << 11);
            assert_eq!(
                s.write_aperture(data, 0, 4, 0, &mut bus),
                Err(Fault::Unsupported)
            );
            assert_eq!(
                s.read_aperture(data, 1, 0, &mut bus),
                Err(Fault::Unsupported)
            );
        }
        assert_eq!(bus.output, before);
        assert_eq!(bus.input.len(), 1);
        assert_eq!(bus.ends, 0);
        let program = (5 << 21) | (0x10 << 11) | (0x80 << 3);
        s.write_aperture(program, 0, 4, 0, &mut bus).unwrap();
        s.write_aperture(program, 0, 4, 0, &mut bus).unwrap();
        let before = bus.output.clone();
        assert_eq!(
            s.read_aperture(1 << 19, 1, 0, &mut bus),
            Err(Fault::Unsupported)
        );
        let invalid_confirm = (1 << 19) | (1 << 20) | (0xd0 << 11);
        assert_eq!(
            s.write_aperture(invalid_confirm, 0, 4, 0, &mut bus),
            Err(Fault::Unsupported)
        );
        assert_eq!(bus.output, before);
        assert_eq!(bus.input.len(), 1);
        assert_eq!(s.deadline(), 0);
    }
}

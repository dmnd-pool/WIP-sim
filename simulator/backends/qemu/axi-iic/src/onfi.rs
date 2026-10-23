//! M69A development profile: identification, parameter page and disabled-ECC
//! feature transactions. Array storage remains in the existing NAND backend.
//! See ONFI.md for source uncertainty and unsupported physical capabilities.

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Array,
    Id,
    Parameter,
    ParameterSeek,
    Get,
    Set,
    Status,
    Failed,
}

#[derive(Default)]
pub struct Identity {
    mode: Mode,
    saved: Mode,
    address: Option<u8>,
    cursor: usize,
    buffer: [u8; 4],
    failed: bool,
    faults: u64,
    seek_bytes: u8,
}

/// ONFI CRC16, initialized to 4f4e and polynomial x16+x15+x2+1.
pub fn crc16(bytes: &[u8]) -> u16 {
    let mut crc = 0x4f4eu16;
    for byte in bytes {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            crc = (crc << 1) ^ if crc & 0x8000 != 0 { 0x8005 } else { 0 };
        }
    }
    crc
}

/// Derived parameter page. Public table fields are transcribed; the vendor
/// suffix is explicitly zero-filled, not claimed as a physical chip dump.
pub fn parameter_page() -> [u8; 256] {
    let mut p = [0; 256];
    p[..4].copy_from_slice(b"ONFI");
    for (at, value) in [
        (4, 2),
        (6, 0x18),
        (8, 0x3f),
        (64, 0x2c),
        (81, 8),
        (84, 64),
        (87, 2),
        (90, 16),
        (92, 64),
        (97, 8),
        (100, 1),
        (101, 0x23),
        (102, 1),
        (103, 40),
        (105, 1),
        (106, 5),
        (107, 1),
        (110, 4),
        (112, 4),
        (113, 1),
        (114, 14),
        (128, 10),
        (129, 0x3f),
        (131, 0x3f),
        (133, 0x58),
        (134, 2),
        (135, 0xb8),
        (136, 11),
        (137, 25),
        (139, 100),
        (164, 1),
    ] {
        p[at] = value;
    }
    p[32..44].copy_from_slice(b"MICRON      ");
    p[44..64].copy_from_slice(b"MT29F2G08ABAEAWP    ");
    p[166..179].copy_from_slice(&[1, 0, 0, 2, 4, 128, 1, 129, 4, 1, 2, 1, 10]);
    let crc = crc16(&p[..254]);
    p[254..].copy_from_slice(&crc.to_le_bytes());
    p
}

impl Identity {
    fn fault(&mut self) {
        self.failed = true;
        self.faults = self.faults.saturating_add(1);
        self.mode = Mode::Failed;
    }
    /// Return true when this layer consumes the byte. False delegates it to
    /// the existing NAND array engine. kind0/1/2 are command/address/data.
    pub fn output(&mut self, kind: u32, value: u8) -> bool {
        if kind == 0 {
            if value == 0x05
                && (self.mode == Mode::Parameter
                    || self.mode == Mode::Status && self.saved == Mode::Parameter)
            {
                self.mode = Mode::ParameterSeek;
                self.cursor = 0;
                self.seek_bytes = 0;
                return true;
            }
            if value == 0xe0 && self.mode == Mode::ParameterSeek {
                if self.seek_bytes == 2 && self.cursor < 768 {
                    self.mode = Mode::Parameter;
                    self.address = Some(0);
                } else {
                    self.fault();
                }
                return true;
            }
            if value == 0x70 {
                if self.mode != Mode::Status {
                    self.saved = self.mode;
                }
                self.mode = Mode::Status;
                return false; // Preserve the array engine's real status.
            }
            if value == 0
                && self.mode == Mode::Status
                && matches!(self.saved, Mode::Parameter | Mode::Get)
            {
                self.mode = self.saved;
                return true;
            }
            self.address = None;
            self.cursor = 0;
            self.buffer = [0; 4];
            self.seek_bytes = 0;
            self.mode = match value {
                0x90 => Mode::Id,
                0xec => Mode::Parameter,
                0xee => Mode::Get,
                0xef => {
                    self.failed = false;
                    Mode::Set
                }
                0xff => {
                    self.failed = false;
                    Mode::Array
                }
                _ => Mode::Array,
            };
            self.saved = Mode::Array;
            return self.mode != Mode::Array;
        }
        if self.mode == Mode::Array || self.mode == Mode::Status {
            return false;
        }
        if kind == 1 && self.mode == Mode::ParameterSeek && self.seek_bytes < 2 {
            self.cursor |= usize::from(value) << (self.seek_bytes * 8);
            self.seek_bytes += 1;
        } else if kind == 1 && self.address.is_none() {
            let valid = match self.mode {
                Mode::Id => value == 0 || value == 0x20,
                Mode::Parameter => value == 0,
                Mode::Get | Mode::Set => value == 0x90 || value == 1,
                _ => false,
            };
            if valid {
                self.address = Some(value);
            } else {
                self.fault();
            }
        } else if kind == 2 && self.mode == Mode::Set && self.address.is_some() && self.cursor < 4 {
            self.buffer[self.cursor] = value;
            self.cursor += 1;
            if self.cursor == 4 && self.buffer != [0; 4] {
                // ECC/OTP and alternate bus timing are not implemented.
                // Do not accept them or falsify their feature readback.
                self.fault();
            }
        } else {
            self.fault();
        }
        true
    }
    /// The caller supplies real array status for status mode only. Ordinary
    /// array reads must not call the legacy engine twice.
    pub fn input(&mut self, array_status: u8) -> Option<u8> {
        if self.mode == Mode::Array {
            return None;
        }
        if self.mode == Mode::Status {
            return Some(array_status | u8::from(self.failed));
        }
        if self.address.is_none() {
            self.fault();
        }
        let byte = match self.mode {
            Mode::Id => {
                let bytes: &[u8] = if self.address == Some(0x20) {
                    b"ONFI"
                } else {
                    &[0x2c, 0xda, 0x90, 0x95, 0x06]
                };
                // The table defines only these ID bytes. Unassigned ID
                // output is pulled high in this development profile; it is
                // not a failed array program or a supported extra ID field.
                Some(bytes.get(self.cursor).copied().unwrap_or(0xff))
            }
            Mode::Parameter if self.cursor < 768 => Some(parameter_page()[self.cursor % 256]),
            Mode::Get if self.cursor < 4 => Some(0),
            _ => None,
        };
        self.cursor = self.cursor.saturating_add(1);
        if let Some(byte) = byte {
            Some(byte)
        } else {
            self.fault();
            Some(0xff)
        }
    }
    pub fn status_mode(&self) -> bool {
        self.mode == Mode::Status
    }
    pub fn faults(&self) -> u64 {
        self.faults
    }
}

#[no_mangle]
pub extern "C" fn md_onfi_size() -> usize {
    core::mem::size_of::<Identity>()
}
/// # Safety
/// state is aligned writable storage of md_onfi_size bytes.
#[no_mangle]
pub unsafe extern "C" fn md_onfi_init(state: *mut Identity) {
    unsafe {
        state.write(Identity::default());
    }
}
/// # Safety
/// state is initialized and exclusively owned. This is a NAND chip reset,
/// not a controller-only reset. It clears the transfer and error state.
#[no_mangle]
pub unsafe extern "C" fn md_onfi_reset(state: *mut Identity) {
    unsafe {
        (&mut *state).output(0, 0xff);
    }
}
/// # Safety
/// state is initialized and exclusively owned.
#[no_mangle]
pub unsafe extern "C" fn md_onfi_output(state: *mut Identity, kind: u32, byte: u8) -> bool {
    unsafe { (&mut *state).output(kind, byte) }
}
/// # Safety
/// state is initialized; value is writable. Both are exclusively owned.
#[no_mangle]
pub unsafe extern "C" fn md_onfi_input(state: *mut Identity, status: u8, value: *mut u8) -> bool {
    unsafe {
        if let Some(v) = (&mut *state).input(status) {
            *value = v;
            true
        } else {
            false
        }
    }
}
/// # Safety
/// state is initialized and live.
#[no_mangle]
pub unsafe extern "C" fn md_onfi_status_mode(state: *const Identity) -> bool {
    unsafe { (&*state).status_mode() }
}
/// # Safety
/// state is initialized and live.
#[no_mangle]
pub unsafe extern "C" fn md_onfi_faults(state: *const Identity) -> u64 {
    unsafe { (&*state).faults() }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn command(s: &mut Identity, cmd: u8, addr: u8) {
        assert!(s.output(0, cmd));
        assert!(s.output(1, addr));
    }
    #[test]
    fn parameter_geometry_redundancy_and_independent_bit_crc() {
        let p = parameter_page();
        assert_eq!(&p[..4], b"ONFI");
        assert_eq!(u32::from_le_bytes(p[80..84].try_into().unwrap()), 2048);
        assert_eq!(u16::from_le_bytes(p[84..86].try_into().unwrap()), 64);
        assert_eq!(p[101], 0x23);
        // Shift-register bit oracle, independent of byte-at-a-time XOR.
        let mut rem = 0x4f4eu32;
        for b in &p[..254] {
            for bit in (0..8).rev() {
                let feedback = ((rem >> 15) ^ u32::from((b >> bit) & 1)) & 1;
                rem = ((rem << 1) & 0xffff) ^ (feedback * 0x8005);
            }
        }
        assert_eq!(rem as u16, u16::from_le_bytes([p[254], p[255]]));
        let mut s = Identity::default();
        command(&mut s, 0xec, 0);
        for _ in 0..3 {
            for expected in p {
                assert_eq!(s.input(0), Some(expected));
            }
        }
        assert_eq!(s.faults(), 0);
        assert_eq!(s.input(0), Some(0xff));
        assert_eq!(s.faults(), 1);
    }
    #[test]
    fn status_resume_preserves_cursor_but_reset_exits_parameter_mode() {
        let mut s = Identity::default();
        command(&mut s, 0xec, 0);
        assert_eq!(s.input(0), Some(b'O'));
        assert!(!s.output(0, 0x70));
        assert_eq!(s.input(0x80), Some(0x80));
        assert!(s.output(0, 0));
        assert_eq!(s.input(0), Some(b'N'));
        assert!(!s.output(0, 0xff));
        assert_eq!(s.input(0), None);
    }
    #[test]
    fn random_parameter_read_never_selects_array_storage() {
        let mut s = Identity::default();
        command(&mut s, 0xec, 0);
        assert_eq!(s.input(0), Some(b'O'));
        assert!(s.output(0, 0x05));
        assert!(s.output(1, 101));
        assert!(s.output(1, 1)); // byte357: byte101 of copy2
        assert!(s.output(0, 0xe0));
        assert_eq!(s.input(0), Some(0x23));
        assert_eq!(s.faults(), 0);
        assert!(s.output(0, 0x05));
        assert!(s.output(1, 0));
        assert!(s.output(1, 3)); // beyond the three supported copies
        assert!(s.output(0, 0xe0));
        assert_eq!(s.input(0), Some(0xff));
        assert!(s.faults() > 0);
        assert!(!s.output(0, 0xff));
        assert_eq!(s.input(0), None);
    }
    #[test]
    fn only_real_disabled_feature_contract_is_accepted() {
        let mut s = Identity::default();
        command(&mut s, 0x90, 0);
        for v in [0x2c, 0xda, 0x90, 0x95, 6] {
            assert_eq!(s.input(0), Some(v));
        }
        for _ in 0..3 {
            assert_eq!(s.input(0), Some(0xff));
        }
        assert!(!s.output(0, 0x70));
        assert_eq!(s.input(0xc0), Some(0xc0));
        assert_eq!(s.faults(), 0);
        command(&mut s, 0xef, 0x90);
        for v in [8, 0, 0, 0] {
            assert!(s.output(2, v));
        }
        assert!(!s.output(0, 0x70));
        assert_eq!(s.input(0xc0), Some(0xc1));
        command(&mut s, 0xee, 0x90);
        for _ in 0..4 {
            assert_eq!(s.input(0), Some(0));
        }
        command(&mut s, 0xef, 0x90);
        for _ in 0..4 {
            assert!(s.output(2, 0));
        }
        assert!(!s.output(0, 0x70));
        assert_eq!(s.input(0xc0), Some(0xc0));
        assert_eq!(s.faults(), 1);
    }
}

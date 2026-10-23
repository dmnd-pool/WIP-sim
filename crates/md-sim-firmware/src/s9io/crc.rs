//! Serial checksum engines from crc5_serial.vhd, crc16_serial.vhd and
//! crc5_resp_serial.vhd. Accept one byte, then process one bit on each of
//! eight core clocks. Clear resets the checksum but not a busy shift state.
use crate::bm1397::{crc16_bit, crc5_bit};

#[derive(Clone, Copy)]
pub enum Kind {
    Command,
    Work,
    Response,
}

pub struct SerialCrc {
    kind: Kind,
    value: u16,
    data: u8,
    remaining: u8,
}

impl SerialCrc {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            value: if matches!(kind, Kind::Work) {
                0xffff
            } else {
                0x1f
            },
            data: 0,
            remaining: 0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.kind);
    }

    pub fn ready(&self) -> bool {
        self.remaining == 0
    }

    pub fn value(&self) -> u16 {
        self.value
    }

    /// Return whether this clock accepted the input byte. Busy writes drop.
    /// `response_init` connects the response engine's5-bit init input; the
    /// two transmit engines use their fixed all-ones reset value.
    pub fn tick(&mut self, input: Option<u8>, clear: bool, response_init: u8) -> bool {
        let accepted = self.ready() && input.is_some();
        if self.remaining != 0 {
            let bit = self.data >> 7;
            self.value = match self.kind {
                Kind::Command => u16::from(crc5_bit(self.value as u8, bit)),
                Kind::Work => crc16_bit(self.value, bit),
                Kind::Response => u16::from(crc5_bit(self.value as u8, bit) ^ (bit << 3)),
            };
            self.data <<= 1;
            self.remaining -= 1;
        } else if let Some(byte) = input {
            self.data = byte;
            self.remaining = 8;
        }
        if clear {
            self.value = match self.kind {
                Kind::Command => 0x1f,
                Kind::Work => 0xffff,
                Kind::Response => u16::from(response_init & 0x1f),
            };
        }
        accepted
    }
}

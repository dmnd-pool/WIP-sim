//! Braiins S9IO0.2 FIFO packet conversion.
//!
//! Source: archived braiins-legacy-bosminer-fork, SHA-256
//! dc5f58721f596d2fc36b50d5049461264986ac339c4b4ff4d383749b98f3a363.
//! Packet conversion and the clocked core/UART modules follow s9io_core.vhd.
//! The `axi` module retains the source wrapper's handshake defects.
//! Exact firmware/bitstream correspondence remains unresolved.
//! The packet converters do not compute or invent ASIC responses.
use crate::bm1397::{crc16, crc5};
pub mod axi;
pub mod crc;
pub mod receive;
pub mod registers;
pub mod transmit;
pub mod uart;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Midstates {
    One,
    Two,
    Four,
}

impl Midstates {
    pub fn count(self) -> usize {
        match self {
            Self::One => 1,
            Self::Two => 2,
            Self::Four => 4,
        }
    }

    pub fn work_words(self) -> usize {
        4 + 8 * self.count()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Length,
    UnsupportedCommandLength,
    Checksum,
    UnsupportedAsicMode,
}

/// Convert one complete command from the command FIFO. The FPGA adds the
/// command checksum. It does not add a55aa preamble or validate command IDs.
/// Commands longer than9 bytes have no timing guarantee in the source contract
/// and remain an explicit unsupported case in this implementation.
pub fn command_frame(words: &[u32]) -> Result<Vec<u8>, Error> {
    let first = *words.first().ok_or(Error::Length)?;
    let length = ((first >> 8) & 0xff) as usize;
    if !matches!(length, 5 | 9) {
        return Err(Error::UnsupportedCommandLength);
    }
    if words.len() != (length - 1) / 4 {
        return Err(Error::Length);
    }
    let mut bytes = Vec::with_capacity(length);
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.push(crc5(&bytes));
    Ok(bytes)
}

/// Convert one complete work from the work FIFO. The first word is an
/// extended work ID. The wire carries its low7 bits. The model must retain
/// the low16 bits as LAST_WORK_ID when that ID is actually transmitted.
///
/// Hardware does not reject misaligned IDs. Preserve that behavior: firmware
/// is responsible for leaving the midstate-index bits clear.
pub fn work_frame(words: &[u32], midstates: Midstates) -> Result<Vec<u8>, Error> {
    if words.len() != midstates.work_words() {
        return Err(Error::Length);
    }
    let length = 22 + 32 * midstates.count();
    let mut bytes = Vec::with_capacity(length);
    bytes.extend_from_slice(&[
        0x21,
        length as u8,
        (words[0] & 0x7f) as u8,
        midstates.count() as u8,
        0,
        0,
        0,
        0,
    ]);
    // nBits, nTime, merkle tail are little endian. Midstate words are
    // supplied in reverse word order and sent most significant byte first.
    for word in &words[1..4] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    for word in &words[4..] {
        bytes.extend_from_slice(&word.to_be_bytes());
    }
    bytes.extend_from_slice(&crc16(&bytes).to_be_bytes());
    Ok(bytes)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Command([u32; 2]),
    Work([u32; 2]),
}

/// The source uses a modified receive CRC with a type-dependent initial
/// state. Bits5/6 of the final byte are ignored by that hardware. Preserve
/// them in the FIFO rather than imposing the BM1397 codec's stricter policy.
fn response_crc(bytes: &[u8; 7]) -> u8 {
    let mut crc = if bytes[6] & 0x80 != 0 { 0x1b } else { 3 };
    for byte in &bytes[..6] {
        for bit in (0..8).rev() {
            let input = (byte >> bit) & 1;
            crc = crate::bm1397::crc5_bit(crc, input) ^ (input << 3);
        }
    }
    crc
}

pub fn response_words(
    bytes: &[u8; 7],
    last_work_id: u16,
    midstates: Midstates,
) -> Result<Response, Error> {
    if response_crc(bytes) != bytes[6] & 0x1f {
        return Err(Error::Checksum);
    }
    Ok(decode_response(bytes, last_work_id, midstates))
}

fn decode_response(bytes: &[u8; 7], last_work_id: u16, midstates: Midstates) -> Response {
    let first = u32::from_le_bytes(bytes[..4].try_into().unwrap());
    let second = if bytes[6] & 0x80 != 0 {
        u32::from(bytes[6]) << 24
            | u32::from(extended_work_id(bytes[5], last_work_id, midstates)) << 8
            | u32::from(bytes[4])
    } else {
        u32::from(bytes[6]) << 24 | u32::from(bytes[5]) << 8 | u32::from(bytes[4])
    };
    if bytes[6] & 0x80 != 0 {
        Response::Work([first, second])
    } else {
        Response::Command([first, second])
    }
}

fn extended_work_id(wire_id: u8, last_work_id: u16, midstates: Midstates) -> u16 {
    let low_id = u16::from(wire_id & 0x7f);
    let compare_id = low_id & !(midstates.count() as u16 - 1);
    let mut high_id = last_work_id & !0x7f;
    if compare_id > last_work_id & 0x7f {
        high_id = high_id.wrapping_sub(0x80);
    }
    high_id | low_id
}
/// Seven-byte receive window. A bad CRC increments the wrapping hardware
/// counter, then the next byte shifts the window by one position. There is
/// no magic preamble search, discard-until-valid shortcut, or generated reply.
/// UART timing, backpressure and FIFO interrupts belong to the caller.
#[derive(Default)]
pub struct Receiver {
    window: [u8; 7],
    len: usize,
    errors: u32,
}

impl Receiver {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn clear_errors(&mut self) {
        self.errors = 0;
    }

    pub fn errors(&self) -> u32 {
        self.errors
    }

    pub fn byte(
        &mut self,
        byte: u8,
        last_work_id: u16,
        midstates: Midstates,
    ) -> Option<Result<Response, Error>> {
        self.window.rotate_left(1);
        self.window[6] = byte;
        self.len += 1;
        if self.len < 7 {
            return None;
        }
        let response = response_words(&self.window, last_work_id, midstates);
        if response.is_err() {
            self.errors = self.errors.wrapping_add(1);
            self.len = 6;
        } else {
            self.len = 0;
        }
        Some(response)
    }
}

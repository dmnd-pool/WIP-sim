//! BM1387 VIL packets from archived Braiins source74b80e7a6d862e49.
//! Archive SHA-256 dc5f58721f596d2fc36b50d5049461264986ac339c4b4ff4d383749b98f3a363.
//! `bm1387.rs`, `io.rs`, the Bitcoin word iterator and S9IO HDL jointly
//! define the byte order. This codec is not a complete powered ASIC chain.
use crate::bm1397::{crc16, crc5, reply_crc51};
use md_sim_mining::{work::digest_from_midstate, MidstateWork};
use std::ops::RangeInclusive;

pub mod budget;
pub mod chain;
pub mod filter;
pub mod i2c;
pub mod nonce;
pub mod search;
pub mod wire;

// The archived BM1387 TicketMaskReg documents exactly the same fixed32-bit
// condition and reversed mask bytes as the retained BM1397 reference.
pub use crate::bm1397::ticket_filter;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    SetAddress {
        address: u8,
        reserved: u8,
    },
    Inactive {
        address: u8,
        reserved: u8,
    },
    Read {
        address: u8,
        register: u8,
    },
    Write {
        address: u8,
        register: u8,
        value: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Header,
    Length,
    Checksum,
    MidstateCount,
    NonceRange,
    UnsupportedNonceOrigin,
    WorkId,
    Filter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: u8,
    /// The retained S9IO always sends four zero bytes. Keep other bytes
    /// opaque until their byte order and behavior have independent evidence.
    pub nonce_origin: [u8; 4],
    pub tail: [u8; 12],
    pub states: Vec<[u32; 8]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Packet {
    Command { broadcast: bool, command: Command },
    Job(Job),
}

/// Bounded BM1387 byte stream. There is no preamble on this link.
/// A bad packet produces an error and no command. Framing errors require
/// the enclosing UART to reset this decoder before the next frame.
#[derive(Default)]
pub struct Decoder {
    bytes: Vec<u8>,
}

impl Decoder {
    pub fn reset(&mut self) {
        self.bytes.clear();
    }

    pub fn push(&mut self, byte: u8) -> Option<Result<Packet, Error>> {
        if self.bytes.is_empty() && !matches!(byte, 0x21 | 0x41 | 0x55 | 0x44 | 0x54 | 0x48 | 0x58)
        {
            return Some(Err(Error::Header));
        }
        self.bytes.push(byte);
        if self.bytes.len() == 2 {
            let valid = match self.bytes[0] {
                0x21 => matches!(byte, 54 | 86 | 150),
                0x48 | 0x58 => byte == 9,
                _ => byte == 5,
            };
            if !valid {
                self.reset();
                return Some(Err(Error::Length));
            }
        }
        if self.bytes.len() >= 2 && self.bytes.len() == usize::from(self.bytes[1]) {
            let packet = decode(&self.bytes);
            self.reset();
            Some(packet)
        } else {
            None
        }
    }
}

/// Decode one complete packet. The S9IO wire has no55aa preamble.
/// Unknown opcodes and malformed framing remain explicit errors.
pub fn decode(frame: &[u8]) -> Result<Packet, Error> {
    if frame.len() < 2 || usize::from(frame[1]) != frame.len() {
        return Err(Error::Length);
    }
    if frame[0] == 0x21 {
        return decode_job(frame).map(Packet::Job);
    }
    let length = match frame[0] {
        0x41 | 0x55 | 0x44 | 0x54 => 5,
        0x48 | 0x58 => 9,
        _ => return Err(Error::Header),
    };
    if frame.len() != length {
        return Err(Error::Length);
    }
    if crc5(&frame[..length - 1]) != frame[length - 1] {
        return Err(Error::Checksum);
    }
    let address = frame[2];
    let register = frame[3];
    let command = match frame[0] & 15 {
        1 => Command::SetAddress {
            address,
            reserved: register,
        },
        5 => Command::Inactive {
            address,
            reserved: register,
        },
        4 => Command::Read { address, register },
        8 => Command::Write {
            address,
            register,
            value: u32::from_be_bytes(frame[4..8].try_into().unwrap()),
        },
        _ => unreachable!(),
    };
    Ok(Packet::Command {
        broadcast: frame[0] & 16 != 0,
        command,
    })
}

fn decode_job(frame: &[u8]) -> Result<Job, Error> {
    if frame.len() < 4 {
        return Err(Error::Length);
    }
    let count = usize::from(frame[3]);
    if !matches!(count, 1 | 2 | 4) {
        return Err(Error::MidstateCount);
    }
    if frame.len() != 22 + 32 * count {
        return Err(Error::Length);
    }
    if crc16(&frame[..frame.len() - 2])
        != u16::from_be_bytes(frame[frame.len() - 2..].try_into().unwrap())
    {
        return Err(Error::Checksum);
    }
    let mut tail = [0; 12];
    tail[..4].copy_from_slice(&frame[16..20]);
    tail[4..8].copy_from_slice(&frame[12..16]);
    tail[8..].copy_from_slice(&frame[8..12]);
    let states = frame[20..frame.len() - 2]
        .chunks_exact(32)
        .map(|bytes| {
            let mut state = [0; 8];
            for (word, raw) in state.iter_mut().rev().zip(bytes.chunks_exact(4)) {
                *word = u32::from_be_bytes(raw.try_into().unwrap());
            }
            state
        })
        .collect();
    Ok(Job {
        id: frame[2],
        nonce_origin: frame[4..8].try_into().unwrap(),
        tail,
        states,
    })
}

impl Job {
    fn validate(&self) -> Result<(), Error> {
        if !matches!(self.states.len(), 1 | 2 | 4) {
            return Err(Error::MidstateCount);
        }
        if self.nonce_origin != [0; 4] {
            return Err(Error::UnsupportedNonceOrigin);
        }
        // Firmware leaves the low midstate bits free. A malformed ID is
        // retained by decode, but cannot become an unambiguous result.
        if self.id > 0x7f || usize::from(self.id) & (self.states.len() - 1) != 0 {
            return Err(Error::WorkId);
        }
        Ok(())
    }

    pub fn digest(&self, index: usize, nonce: u32) -> Result<[u8; 32], Error> {
        self.validate()?;
        let state = *self.states.get(index).ok_or(Error::MidstateCount)?;
        Ok(digest_from_midstate(state, self.tail, nonce))
    }

    /// The enclosing topology assigns a range and a unique engine ID. It
    /// must also enforce chip/core nonce ownership before returning a result.
    /// No hash rate or filter adaptation is implicit in this conversion.
    pub fn engine_work(
        &self,
        id: String,
        nonces: RangeInclusive<u32>,
        ticket: u32,
    ) -> md_sim_mining::Result<MidstateWork> {
        self.validate().map_err(|e| format!("BM1387 work: {e:?}"))?;
        let work = MidstateWork {
            id,
            states: self.states.clone(),
            tail: self.tail,
            nonce_start: *nonces.start(),
            nonce_end: *nonces.end(),
            filter: ticket_filter(ticket),
        };
        work.validate()?;
        Ok(work)
    }

    /// Encode only a computed result that meets the programmed filter and
    /// caller's assigned nonce range. This does not establish pool acceptance.
    pub fn response(
        &self,
        index: usize,
        nonce: u32,
        nonces: RangeInclusive<u32>,
        solution_index: u8,
        ticket: u32,
    ) -> Result<[u8; 7], Error> {
        if !nonces.contains(&nonce) {
            return Err(Error::NonceRange);
        }
        let digest = self.digest(index, nonce)?;
        if !ticket_filter(ticket).accepts(&digest) {
            return Err(Error::Filter);
        }
        Ok(self.encode_response(index, nonce, solution_index))
    }

    // The caller must have computed and checked this candidate. Keep this
    // private so external callers cannot encode an unchecked solution.
    fn encode_response(&self, index: usize, nonce: u32, solution_index: u8) -> [u8; 7] {
        let mut response = [0; 7];
        response[..4].copy_from_slice(&nonce.to_le_bytes());
        response[4] = solution_index;
        response[5] = self.id | index as u8;
        response[6] = 0x80 | reply_crc51(response[..6].try_into().unwrap(), 0x80);
        response
    }
}

/// Actual chip register state supplies `value`. BM1387's last two payload
/// bytes are zero; addresses/register IDs in those positions belong to other
/// chips. This function does not implement register or power behavior.
pub fn register_response(value: u32) -> [u8; 7] {
    let mut response = [0; 7];
    response[..4].copy_from_slice(&value.to_be_bytes());
    response[6] = reply_crc51(response[..6].try_into().unwrap(), 0);
    response
}

/// Nonce routing used by the archived firmware. Physical geometry and
/// partition timing are not established by this source interpretation.
/// The enclosing chain must reject addresses absent from its topology.
pub fn nonce_address(nonce: u32) -> (u8, u8) {
    (((nonce >> 2) & 0x3f) as u8, ((nonce >> 24) & 0x7f) as u8)
}

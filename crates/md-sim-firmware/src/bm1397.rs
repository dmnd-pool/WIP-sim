//! BM1397 transmit frames used by archived ESP-Miner64680f8a4da0.
//!
//! The source archive SHA-256 is ccba9a695c3661a7d536cd649c0dcd6935cb0f9d59d24ff667391d65398ce069.
//! Its bm1397.c, crc.c and packed job declaration define this codec. This
//! The codec and bounded initialization state do not establish a complete
//! chip or board. Shared-engine work conversion is separate from the model.
use serde::Serialize;
pub mod core_control;
pub mod core_readback;
mod filter;
mod identify;
mod reply;
pub use filter::ticket_filter;
pub use identify::{IdentificationChain, IdentifyError};
pub use reply::Reply;

const COMMAND_TYPE: u8 = 0x40;
const JOB_TYPE: u8 = 0x20;
const BROADCAST: u8 = 0x10;
const JOB_BYTES: usize = 146;
const MAX_FRAME: usize = JOB_BYTES + 6;

/// Polynomial x^5+x^2+1, initial31, MSB first, no final XOR.
/// A transmitted command carries this value in its final byte. Checking a
/// zero remainder over that entire command is a different, incorrect test.
pub fn crc5(bytes: &[u8]) -> u8 {
    let mut state = 0x1f;
    for byte in bytes {
        for bit in (0..8).rev() {
            state = crc5_bit(state, byte >> bit);
        }
    }
    state
}

pub(crate) fn crc5_bit(state: u8, input: u8) -> u8 {
    let feedback = ((state >> 4) ^ input) & 1;
    ((state << 1) ^ (feedback * 5)) & 0x1f
}

/// Six reply payload bytes followed by the three upper type bits.
/// Shared by the separately framed BM1387 and BM1397 reply formats.
pub(crate) fn reply_crc51(data: &[u8; 6], flags: u8) -> u8 {
    let mut state = crc5(data);
    for bit in (5..8).rev() {
        state = crc5_bit(state, flags >> bit);
    }
    state
}

/// CRC16/IBM-3740, also called CCITT-FALSE. It covers the job header,
/// declared length and payload, with the checksum most significant byte first.
pub fn crc16(bytes: &[u8]) -> u16 {
    let mut state = 0xffff;
    for byte in bytes {
        for bit in (0..8).rev() {
            state = crc16_bit(state, byte >> bit);
        }
    }
    state
}

pub(crate) fn crc16_bit(state: u16, input: u8) -> u16 {
    let feedback = ((state >> 15) ^ u16::from(input)) & 1;
    (state << 1) ^ (feedback * 0x1021)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Command {
    SetAddress {
        address: u8,
        reserved: u8,
    },
    Write {
        address: u8,
        register: u8,
        value: u32,
    },
    Read {
        address: u8,
        register: u8,
    },
    Inactive {
        address: u8,
        reserved: u8,
    },
}

/// Preserve the wire bytes until their representation is converted by the
/// chip model. Inactive midstate slots are not claimed to be initialized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Job {
    pub id: u8,
    pub midstate_count: u8,
    pub starting_nonce: [u8; 4],
    pub nbits: [u8; 4],
    pub ntime: [u8; 4],
    pub merkle_tail: [u8; 4],
    pub midstates: [[u8; 32]; 4],
}

impl Job {
    /// Resume SHA-256 from an active wire midstate, then compute SHA-256 again.
    /// Archived utils.c copies the ESP32's little-endian compression words,
    /// and mining.c reverses their word order for this ASIC. Merkle-tail,
    /// time and nBits bytes already have Bitcoin header byte order.
    pub fn digest(&self, midstate_index: usize, nonce: u32) -> Option<[u8; 32]> {
        let state = self.midstate(midstate_index)?;
        Some(md_sim_mining::work::digest_from_midstate(
            state,
            self.tail(),
            nonce,
        ))
    }

    fn midstate(&self, index: usize) -> Option<[u32; 8]> {
        if !matches!(self.midstate_count, 1 | 4) || index >= usize::from(self.midstate_count) {
            return None;
        }
        let mut state = [0; 8];
        for (word, bytes) in state
            .iter_mut()
            .rev()
            .zip(self.midstates[index].chunks_exact(4))
        {
            *word = u32::from_le_bytes(bytes.try_into().unwrap());
        }
        Some(state)
    }

    fn tail(&self) -> [u8; 12] {
        let mut tail = [0; 12];
        tail[..4].copy_from_slice(&self.merkle_tail);
        tail[4..8].copy_from_slice(&self.ntime);
        tail[8..].copy_from_slice(&self.nbits);
        tail
    }

    /// Use the run's shared engine. The model must set the permitted nonce end
    /// from its topology and retain this work ID until the response is decoded.
    /// The programmed ticket filter, including its fixed32 bits, is preserved.
    pub fn engine_work(
        &self,
        id: String,
        nonce_end: u32,
        ticket_register: u32,
    ) -> md_sim_mining::Result<md_sim_mining::MidstateWork> {
        if !matches!(self.midstate_count, 1 | 4) {
            return Err("unsupported BM1397 midstate count".into());
        }
        let work = md_sim_mining::MidstateWork {
            id,
            states: (0..usize::from(self.midstate_count))
                .map(|index| self.midstate(index).expect("validated midstate count"))
                .collect(),
            tail: self.tail(),
            nonce_start: u32::from_le_bytes(self.starting_nonce),
            nonce_end,
            filter: ticket_filter(ticket_register),
        };
        work.validate()?;
        Ok(work)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Packet {
    Command { broadcast: bool, command: Command },
    Job { broadcast: bool, work: Job },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Error {
    Preamble,
    Header,
    Length,
    Checksum,
    MidstateCount,
}

fn expected_length(header: u8) -> Result<usize, Error> {
    // Only ALL and the low command bits are variable in these source frames.
    match header & !BROADCAST {
        0x40 | 0x42 | 0x43 => Ok(7),
        0x41 => Ok(11),
        0x21 => Ok(MAX_FRAME),
        _ => Err(Error::Header),
    }
}

pub fn decode(frame: &[u8]) -> Result<Packet, Error> {
    if !frame.starts_with(&[0x55, 0xaa]) {
        return Err(Error::Preamble);
    }
    if frame.len() < 4 {
        return Err(Error::Length);
    }
    let length = expected_length(frame[2])?;
    if frame.len() != length || usize::from(frame[3]) + 2 != length {
        return Err(Error::Length);
    }
    let broadcast = frame[2] & BROADCAST != 0;
    if frame[2] & COMMAND_TYPE != 0 {
        if crc5(&frame[2..length - 1]) != frame[length - 1] {
            return Err(Error::Checksum);
        }
        let address = frame[4];
        let register = frame[5];
        let command = match frame[2] & 3 {
            0 => Command::SetAddress {
                address,
                reserved: register,
            },
            1 => Command::Write {
                address,
                register,
                value: u32::from_be_bytes(frame[6..10].try_into().unwrap()),
            },
            2 => Command::Read { address, register },
            3 => Command::Inactive {
                address,
                reserved: register,
            },
            _ => unreachable!(),
        };
        return Ok(Packet::Command { broadcast, command });
    }
    debug_assert_ne!(frame[2] & JOB_TYPE, 0);
    if crc16(&frame[2..length - 2]) != u16::from_be_bytes(frame[length - 2..].try_into().unwrap()) {
        return Err(Error::Checksum);
    }
    let data = &frame[4..length - 2];
    if !matches!(data[1], 1 | 4) {
        return Err(Error::MidstateCount);
    }
    let mut midstates = [[0; 32]; 4];
    for (output, input) in midstates.iter_mut().zip(data[18..].chunks_exact(32)) {
        output.copy_from_slice(input);
    }
    Ok(Packet::Job {
        broadcast,
        work: Job {
            id: data[0],
            midstate_count: data[1],
            starting_nonce: data[2..6].try_into().unwrap(),
            nbits: data[6..10].try_into().unwrap(),
            ntime: data[10..14].try_into().unwrap(),
            merkle_tail: data[14..18].try_into().unwrap(),
            midstates,
        },
    })
}

/// Bounded stream framing. Reset discards an unfinished frame. A corrupt
/// header or length returns an error immediately, then searches for a new
/// preamble. Embedded preambles in a valid payload remain ordinary data.
#[derive(Default)]
pub struct Decoder {
    input: Vec<u8>,
}
impl Decoder {
    pub fn reset(&mut self) {
        self.input.clear();
    }

    pub fn push(&mut self, byte: u8) -> Option<Result<Packet, Error>> {
        if self.input.is_empty() {
            if byte == 0x55 {
                self.input.push(byte);
            }
            return None;
        }
        if self.input.len() == 1 {
            if byte == 0xaa {
                self.input.push(byte);
            } else if byte != 0x55 {
                self.input.clear();
            }
            return None;
        }
        self.input.push(byte);
        let length = match expected_length(self.input[2]) {
            Ok(length) => length,
            Err(error) => {
                self.reset();
                // A rejected header byte can also begin the next preamble.
                if byte == 0x55 {
                    self.input.push(byte);
                }
                return Some(Err(error));
            }
        };
        if self.input.len() < 4 {
            return None;
        }
        if usize::from(self.input[3]) + 2 != length {
            self.reset();
            if byte == 0x55 {
                self.input.push(byte);
            }
            return Some(Err(Error::Length));
        }
        if self.input.len() != length {
            return None;
        }
        let result = decode(&self.input);
        self.reset();
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin_hashes::{sha256d, Hash};
    use md_sim_mining::{work::header_midstate, Header};

    const READ_ID: &[u8] = &[0x55, 0xaa, 0x52, 0x05, 0, 0, 0x0a];

    #[test]
    fn decodes_actual_archived_firmware_uart_query() {
        // Actual original-probe024 UART output. No generated guest response.
        assert_eq!(
            decode(READ_ID),
            Ok(Packet::Command {
                broadcast: true,
                command: Command::Read {
                    address: 0,
                    register: 0
                }
            })
        );
        assert_eq!(crc5(&READ_ID[2..6]), 0x0a);
        assert_ne!(crc5(&READ_ID[2..]), 0);
        assert_eq!(crc16(b"123456789"), 0x29b1);
    }

    #[test]
    fn rejects_each_single_bit_error_and_noncanonical_checksum() {
        for bit in 0..READ_ID.len() * 8 {
            let mut frame = READ_ID.to_vec();
            frame[bit / 8] ^= 1 << (bit % 8);
            assert!(decode(&frame).is_err(), "accepted corrupted bit {bit}");
        }
        let mut checksum = READ_ID.to_vec();
        checksum[6] |= 0x20;
        assert_eq!(decode(&checksum), Err(Error::Checksum));
        let mut extra = READ_ID.to_vec();
        extra.push(0);
        assert_eq!(decode(&extra), Err(Error::Length));
    }

    #[test]
    fn stream_handles_fragments_noise_recovery_and_reset() {
        let expected = decode(READ_ID);
        for split in 0..READ_ID.len() {
            let mut decoder = Decoder::default();
            let mut output = Vec::new();
            for bytes in [&READ_ID[..split], &READ_ID[split..]] {
                output.extend(bytes.iter().filter_map(|b| decoder.push(*b)));
            }
            assert_eq!(output.as_slice(), std::slice::from_ref(&expected));
        }
        let mut decoder = Decoder::default();
        for byte in [0, 0xaa, 0x55, 0x55, 0xaa, 0x52, 0xff] {
            let result = decoder.push(byte);
            if byte == 0xff {
                assert_eq!(result, Some(Err(Error::Length)));
            } else {
                assert!(result.is_none());
            }
        }
        assert_eq!(
            READ_ID
                .iter()
                .filter_map(|b| decoder.push(*b))
                .collect::<Vec<_>>()
                .as_slice(),
            std::slice::from_ref(&expected)
        );
        for byte in &READ_ID[..5] {
            assert!(decoder.push(*byte).is_none());
        }
        decoder.reset();
        for byte in &READ_ID[5..] {
            assert!(decoder.push(*byte).is_none());
        }
        assert_eq!(
            READ_ID
                .iter()
                .filter_map(|b| decoder.push(*b))
                .collect::<Vec<_>>(),
            [expected]
        );
    }

    #[test]
    fn transmitted_midstates_match_independent_complete_header_hashes() {
        // Deterministic regression work, not a fresh mining acceptance claim.
        // The oracle hashes all80header bytes with a different SHA library.
        let mut random = 0x82c6bfa1397e102du64;
        let mut next = || {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            random as u32
        };
        for _ in 0..128 {
            let mut header = Header {
                version: next(),
                previous: [0; 32],
                merkle_root: [0; 32],
                time: next(),
                bits: 0x207fffff,
                nonce: 0,
            };
            for word in header.previous.chunks_exact_mut(4) {
                word.copy_from_slice(&next().to_le_bytes());
            }
            for word in header.merkle_root.chunks_exact_mut(4) {
                word.copy_from_slice(&next().to_le_bytes());
            }
            let mut job = Job {
                id: 4,
                midstate_count: 4,
                starting_nonce: 0u32.to_le_bytes(),
                nbits: header.bits.to_le_bytes(),
                ntime: header.time.to_le_bytes(),
                merkle_tail: header.merkle_root[28..].try_into().unwrap(),
                midstates: [[0; 32]; 4],
            };
            let mut versions = [0; 4];
            for (index, version) in versions.iter_mut().enumerate() {
                *version = next();
                header.version = *version;
                let state = header_midstate(&header);
                // The archived firmware reverses the eight state words.
                for (wire, word) in job.midstates[index]
                    .chunks_exact_mut(4)
                    .zip(state.into_iter().rev())
                {
                    wire.copy_from_slice(&word.to_le_bytes());
                }
            }
            // Exercise the complete packed transmit frame, including its
            // declared146-byte payload and all four stored midstates.
            let mut wire = vec![0x55, 0xaa, 0x21, 150, job.id, job.midstate_count];
            wire.extend(job.starting_nonce);
            wire.extend(job.nbits);
            wire.extend(job.ntime);
            wire.extend(job.merkle_tail);
            for state in &job.midstates {
                wire.extend(state);
            }
            wire.extend(crc16(&wire[2..]).to_be_bytes());
            let Packet::Job {
                broadcast: false,
                work: mut decoded,
            } = decode(&wire).unwrap()
            else {
                panic!("job frame was not decoded as a single-chip job");
            };
            assert_eq!(decoded, job);
            for byte in 2..wire.len() {
                let mut damaged = wire.clone();
                damaged[byte] ^= 1;
                assert!(decode(&damaged).is_err(), "accepted corrupted byte {byte}");
            }
            for nonce in [0, 1, 0x80000000, u32::MAX, next(), next()] {
                header.nonce = nonce;
                for (index, version) in versions.iter().enumerate() {
                    header.version = *version;
                    assert_eq!(
                        decoded.digest(index, nonce),
                        Some(sha256d::Hash::hash(&header.encode()).to_byte_array())
                    );
                }
            }
            decoded.midstate_count = 1;
            assert!(decoded.digest(0, 0).is_some());
            assert!(decoded.digest(1, 0).is_none());
            assert!(decoded.digest(usize::MAX, 0).is_none());
            decoded.midstate_count = 3;
            assert!(decoded.digest(0, 0).is_none());
        }
    }

    #[test]
    fn scheduled_regression_work_preserves_programmed_filter_and_response_bytes() {
        use md_sim_mining::{Engine, EngineEvent};
        use std::time::{Duration, Instant};
        // Required fixed-work regression only. This is not fresh firmware
        // mining: the test constructs the work and retains the full header.
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/bitcoin/genesis-header.json"
        ))
        .unwrap();
        let mut header =
            Header::decode(&hex::decode(fixture["header_hex"].as_str().unwrap()).unwrap()).unwrap();
        let nonce = header.nonce;
        let mut job = Job {
            id: 4,
            midstate_count: 1,
            starting_nonce: nonce.to_le_bytes(),
            nbits: header.bits.to_le_bytes(),
            ntime: header.time.to_le_bytes(),
            merkle_tail: header.merkle_root[28..].try_into().unwrap(),
            midstates: [[0; 32]; 4],
        };
        for (wire, word) in job.midstates[0]
            .chunks_exact_mut(4)
            .zip(header_midstate(&header).into_iter().rev())
        {
            wire.copy_from_slice(&word.to_le_bytes());
        }
        for ticket in [0, u32::MAX] {
            let mut engine = Engine::default();
            engine
                .submit_midstate(
                    job.engine_work("regression".into(), nonce + 2, ticket)
                        .unwrap(),
                )
                .unwrap();
            let mut actual = Vec::new();
            let start = Instant::now();
            while engine.queue_len() != 0 {
                assert!(start.elapsed() < Duration::from_secs(2));
                for event in engine.poll() {
                    if let EngineEvent::MidstateSolution {
                        nonce,
                        midstate_index,
                        digest,
                        ..
                    } = event
                    {
                        assert_eq!(midstate_index, 0);
                        header.nonce = nonce;
                        assert_eq!(
                            digest,
                            sha256d::Hash::hash(&header.encode()).to_byte_array()
                        );
                        let response = Reply::Job {
                            nonce,
                            midstate_tag: 0,
                            job_id: 4,
                            midstate_index: 0,
                        }
                        .encode()
                        .unwrap();
                        // Literal packed ESP32 receive convention, independent
                        // of the response decoder's byte-order logic.
                        assert_eq!(
                            u32::from_le_bytes(response[2..6].try_into().unwrap()),
                            nonce
                        );
                        assert_eq!(response[7], 4);
                        actual.push(nonce);
                    }
                }
                std::thread::sleep(engine.wait_duration());
            }
            let mut expected = Vec::new();
            for candidate in nonce..=nonce + 2 {
                header.nonce = candidate;
                let digest = sha256d::Hash::hash(&header.encode()).to_byte_array();
                let fixed = digest[28..] == [0; 4];
                let programmed = ticket == 0 || digest[24..28] == [0; 4];
                if fixed && programmed {
                    expected.push(candidate);
                }
            }
            assert_eq!(actual, expected);
            if ticket == 0 {
                assert!(actual.contains(&nonce));
            } else {
                assert!(actual.is_empty());
            }
            assert_eq!(engine.hashes_computed, 3);
        }
        assert!(job.engine_work("range".into(), nonce - 1, 0).is_err());
        job.midstate_count = 0;
        assert!(job.engine_work("empty".into(), u32::MAX, 0).is_err());
    }
}

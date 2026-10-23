//! Bitmain S2 controller packets from cgminer b8491c66e7e22f23a9edf095dd1337ee581e88bd.
//! This is the controller-facing stream. It does not model the downstream FPGA
//! or claim a physical S2 board identity. Low-rate signed filter exponents are
//! an explicit protocol adaptation; original unsigned exponents remain default.
use anyhow::{bail, ensure, Result};
use md_sim_mining::Target;
use std::collections::VecDeque;

pub const MAX_PACKET: usize = 8192;
pub const QUEUE_CAPACITY: usize = 512;

pub fn crc16(data: &[u8]) -> u16 {
    let mut crc = 0xffffu16;
    for byte in data {
        crc ^= u16::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ if crc & 1 != 0 { 0xa001 } else { 0 };
        }
    }
    crc
}

fn finish(mut packet: Vec<u8>) -> Vec<u8> {
    let length = u16::try_from(packet.len() - 2).expect("bounded device packet");
    packet[2..4].copy_from_slice(&length.to_le_bytes());
    let crc = crc16(&packet);
    packet.extend_from_slice(&crc.to_le_bytes());
    packet
}

#[derive(Clone, Debug)]
pub struct WorkPacket {
    pub id: u32,
    pub midstate: [u8; 32],
    pub tail: [u8; 12],
    pub exponent: i16,
    pub nonce: u64,
}

impl WorkPacket {
    pub fn digest(&self, nonce: u32) -> [u8; 32] {
        let mut state = [0; 8];
        for (word, bytes) in state.iter_mut().zip(self.midstate.chunks_exact(4)) {
            *word = u32::from_le_bytes(bytes.try_into().unwrap());
        }
        let mut tail = self.tail;
        for word in tail.chunks_exact_mut(4) {
            word.reverse();
        }
        md_sim_mining::work::digest_from_midstate(state, tail, nonce)
    }

    /// Exact powers of two applied to Bitcoin's difficulty-one target.
    pub fn target(&self) -> Target {
        let original = Target::from_compact(0x1d00ffff).expect("constant target").0;
        let mut out = [0u8; 32];
        for bit in 0..256i16 {
            if original[(bit / 8) as usize] & (0x80 >> (bit % 8)) != 0 {
                let dest = bit + self.exponent;
                if dest < 0 {
                    return Target::MAX;
                }
                if dest < 256 {
                    out[(dest / 8) as usize] |= 0x80 >> (dest % 8);
                }
            }
        }
        Target(out)
    }
}

#[derive(Debug)]
pub struct Controller {
    pub chips_per_chain: Vec<u8>,
    pub queue: VecDeque<WorkPacket>,
    pub configured: bool,
    pub fan_pwm: u8,
    pub timeout: u8,
    pub frequency: u16,
    pub voltage: [u8; 2],
    pub crc_errors: u32,
    pub resets: u64,
    pub generations: u64,
    pub computed_hashes: u64,
    pub results: u64,
    pub low_rate_signed_exponents: bool,
    input: Vec<u8>,
}

impl Controller {
    pub fn new(chips_per_chain: Vec<u8>, low_rate_signed_exponents: bool) -> Result<Self> {
        ensure!(
            !chips_per_chain.is_empty() && chips_per_chain.len() <= 16,
            "chain count outside protocol bounds"
        );
        ensure!(
            chips_per_chain.iter().all(|n| *n > 0),
            "zero chip count is not an unknown value"
        );
        Ok(Self {
            chips_per_chain,
            queue: VecDeque::new(),
            configured: false,
            fan_pwm: 0,
            timeout: 0,
            frequency: 0,
            voltage: [0; 2],
            crc_errors: 0,
            resets: 0,
            generations: 0,
            computed_hashes: 0,
            results: 0,
            low_rate_signed_exponents,
            input: Vec::new(),
        })
    }

    /// Partial stream frames are retained. An invalid frame has no state effect.
    /// Unknown commands fail and must block conformance rather than return success.
    pub fn receive(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
        ensure!(
            self.input.len() + bytes.len() <= MAX_PACKET * 2,
            "input FIFO overflow"
        );
        self.input.extend_from_slice(bytes);
        let mut responses = Vec::new();
        while self.input.len() >= 4 {
            let size = usize::from(u16::from_le_bytes([self.input[2], self.input[3]])) + 4;
            ensure!(
                (6..=MAX_PACKET).contains(&size),
                "invalid packet length {size}"
            );
            if self.input.len() < size {
                break;
            }
            let packet: Vec<u8> = self.input.drain(..size).collect();
            if crc16(&packet[..size - 2])
                != u16::from_le_bytes(packet[size - 2..].try_into().unwrap())
            {
                self.crc_errors += 1;
                continue;
            }
            ensure!(packet[1] == 0, "unsupported packet version");
            match packet[0] {
                0x53 => {
                    ensure!(size == 12, "invalid status request size");
                    ensure!(
                        packet[4] & !2 == 0 && packet[5..8] == [0, 0, 0],
                        "unsupported status flags"
                    );
                    // Register access is not implemented. Never invent its value.
                    ensure!(
                        packet[8] == 0 && packet[9] == 0,
                        "unimplemented register read"
                    );
                    responses.push(self.status());
                }
                0x51 => self.configure(&packet)?,
                0x52 => self.work(&packet)?,
                command => bail!("unknown controller command {command:02x}"),
            }
        }
        Ok(responses)
    }

    fn configure(&mut self, packet: &[u8]) -> Result<()> {
        ensure!(packet.len() == 24, "invalid configuration size");
        ensure!(
            usize::from(packet[8]) == self.chips_per_chain.len(),
            "chain configuration differs from profile"
        );
        ensure!(
            self.chips_per_chain.iter().all(|n| *n == packet[9]),
            "chip configuration differs from profile"
        );
        ensure!(
            packet[10] <= 0xa0 && packet[11] > 0,
            "invalid fan or work timeout"
        );
        ensure!(
            packet[20] == 0 && packet[21] == 4,
            "unsupported chip register configuration"
        );
        if packet[4] & 1 != 0 {
            self.queue.clear();
            self.resets += 1;
            self.generations += 1;
        }
        if packet[4] & 2 != 0 {
            self.fan_pwm = packet[10];
        }
        if packet[4] & 4 != 0 {
            self.timeout = packet[11];
        }
        if packet[4] & 8 != 0 {
            self.frequency = u16::from_le_bytes([packet[12], packet[13]]);
        }
        if packet[4] & 16 != 0 {
            self.voltage.copy_from_slice(&packet[14..16]);
        }
        self.configured = true;
        Ok(())
    }

    fn work(&mut self, packet: &[u8]) -> Result<()> {
        ensure!(self.configured, "work before configuration");
        ensure!(
            packet.len() >= 58 && (packet.len() - 10) % 48 == 0,
            "invalid task size"
        );
        ensure!(
            packet[4] & !1 == 0 && packet[6..8] == [0, 0],
            "reserved work bits set"
        );
        let count = (packet.len() - 10) / 48;
        ensure!(count <= 64, "too many work items");
        let exponent = if self.low_rate_signed_exponents {
            i16::from(packet[5] as i8)
        } else {
            i16::from(packet[5])
        };
        ensure!(
            (-31..=12).contains(&exponent),
            "unsupported filter exponent"
        );
        let clean = packet[4] & 1 != 0;
        ensure!(
            count + if clean { 0 } else { self.queue.len() } <= QUEUE_CAPACITY,
            "work FIFO overflow"
        );
        if clean {
            self.queue.clear();
            self.generations += 1;
        }
        for item in packet[8..packet.len() - 2].chunks_exact(48) {
            self.queue.push_back(WorkPacket {
                id: u32::from_le_bytes(item[..4].try_into().unwrap()),
                midstate: item[4..36].try_into().unwrap(),
                tail: item[36..].try_into().unwrap(),
                exponent,
                nonce: 0,
            });
        }
        Ok(())
    }

    pub fn status(&self) -> Vec<u8> {
        let mut p = vec![0u8; 28];
        p[0] = 0xa1;
        p[5] = self.chips_per_chain.len() as u8;
        p[6..8].copy_from_slice(&((QUEUE_CAPACITY - self.queue.len()) as u16).to_le_bytes());
        // Simulator-owned controller identity, not a claimed hardware revision.
        p[8..12].copy_from_slice(&[0x4d, 0x44, 0x53, 0x01]);
        // This interface test exposes no thermal calibration or fan tach claim.
        p[20..24].copy_from_slice(&self.crc_errors.to_le_bytes());
        for _ in 0..2 {
            for chips in &self.chips_per_chain {
                for group in 0..usize::from(*chips).div_ceil(32) {
                    let count = usize::from(*chips).saturating_sub(group * 32).min(32);
                    let bits = if count == 32 {
                        u32::MAX
                    } else {
                        (1u32 << count) - 1
                    };
                    p.extend_from_slice(&bits.to_le_bytes());
                }
            }
        }
        p.extend_from_slice(&self.chips_per_chain);
        finish(p)
    }

    /// Search one actual nonce. The caller owns the aggregate wall-time cap.
    /// Rotate jobs to keep each queued work range progressing without overlap.
    pub fn step(&mut self) -> Option<Vec<u8>> {
        let mut work = self.queue.pop_front()?;
        let nonce = work.nonce as u32;
        let digest = work.digest(nonce);
        self.computed_hashes += 1;
        work.nonce += 1;
        let valid = work.target().accepts(&digest);
        let id = work.id;
        let exponent = work.exponent;
        if work.nonce <= u64::from(u32::MAX) {
            self.queue.push_back(work);
        }
        if !valid {
            return None;
        }
        self.results += 1;
        let mut p = vec![0u8; 16];
        p[0] = 0xa2;
        p[4..6].copy_from_slice(&((QUEUE_CAPACITY - self.queue.len()) as u16).to_le_bytes());
        p[6..8].copy_from_slice(&exponent.to_le_bytes());
        p[8..16].copy_from_slice(&self.results.to_le_bytes());
        p.extend_from_slice(&id.to_le_bytes());
        // cgminer's rebuild_nonce writes this value into its word-swapped buffer.
        p.extend_from_slice(&nonce.swap_bytes().to_le_bytes());
        Some(finish(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modbus_check_vector() {
        assert_eq!(crc16(b"123456789"), 0x4b37);
    }

    #[test]
    fn partial_crc_and_unknown_packets() {
        let mut c = Controller::new(vec![32; 8], false).unwrap();
        let mut request = vec![0; 10];
        request[0] = 0x53;
        let request = finish(request);
        assert!(c.receive(&request[..3]).unwrap().is_empty());
        assert_eq!(c.receive(&request[3..]).unwrap().len(), 1);
        let mut bad = request.clone();
        bad[4] ^= 1;
        assert!(c.receive(&bad).unwrap().is_empty());
        assert_eq!(c.crc_errors, 1);
        let mut unknown = vec![0; 4];
        unknown[0] = 0x54;
        assert!(c.receive(&finish(unknown)).is_err());
    }

    #[test]
    fn filter_is_not_replaced_by_pool_target() {
        let work = WorkPacket {
            id: 1,
            midstate: [0; 32],
            tail: [0; 12],
            exponent: 0,
            nonce: 0,
        };
        assert_eq!(work.target(), Target::from_compact(0x1d00ffff).unwrap());
        let easy = WorkPacket {
            exponent: -31,
            ..work
        };
        assert_eq!(&easy.target().0[..4], &[0x7f, 0xff, 0x80, 0]);
    }

    #[test]
    fn upstream_captured_packets_preserve_queue_and_clean_transition() {
        let decode = |s: &str| hex::decode(s.trim()).unwrap();
        let mut c = Controller::new(vec![32; 8], true).unwrap();
        let enumeration = decode(include_str!(
            "../../../simulator/firmware/fixtures/enumeration.hex"
        ));
        assert_eq!(c.receive(&enumeration).unwrap().len(), 1);
        let configuration = decode(include_str!(
            "../../../simulator/firmware/fixtures/configuration.hex"
        ));
        c.receive(&configuration).unwrap();
        assert!(c.configured);
        assert_eq!(c.resets, 1);
        let initial = decode(include_str!(
            "../../../simulator/firmware/fixtures/initial-work.hex"
        ));
        for chunk in initial.chunks(7) {
            c.receive(chunk).unwrap();
        }
        assert_eq!(c.queue.len(), 61);
        assert!(c.queue.iter().all(|w| w.exponent == -20));
        let old_ids: Vec<_> = c.queue.iter().map(|w| w.id).collect();
        let clean = decode(include_str!(
            "../../../simulator/firmware/fixtures/clean-work.hex"
        ));
        c.receive(&clean).unwrap();
        assert_eq!(c.generations, 2);
        assert!(c
            .queue
            .iter()
            .all(|w| w.exponent == -19 && !old_ids.contains(&w.id)));
        assert_eq!(c.computed_hashes, 0);
        assert_eq!(c.results, 0);
        c.receive(&configuration).unwrap();
        assert!(c.queue.is_empty());
    }
}

//! Receive format in archived ESP-Miner bm1397.c and asic_common.c.
//! The independent Kano driver d689d3a9057e checks a51-bit CRC input for
//! nine-byte replies. Unlike transmit commands, receive type bits precede
//! the five checksum bits. The resulting seven-byte CRC residue is zero.
use super::{crc5, reply_crc51, Error};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Reply {
    Register {
        value: u32,
        asic_address: u8,
        register: u8,
    },
    Job {
        nonce: u32,
        /// Retained byte6. The archived driver names it midstate_num but
        /// selects the active midstate from the low two bits of byte7.
        midstate_tag: u8,
        job_id: u8,
        midstate_index: u8,
    },
}

impl Reply {
    /// Serialize supplied model state. This does not generate work or certify
    /// a nonce. The mining engine must check every result before transmission.
    pub fn encode(&self) -> Result<[u8; 9], Error> {
        let mut frame = [0; 9];
        frame[..2].copy_from_slice(&[0xaa, 0x55]);
        match self {
            Self::Register {
                value,
                asic_address,
                register,
            } => {
                frame[2..6].copy_from_slice(&value.to_be_bytes());
                frame[6] = *asic_address;
                frame[7] = *register;
            }
            Self::Job {
                nonce,
                midstate_tag,
                job_id,
                midstate_index,
            } => {
                if job_id & 3 != 0 || *midstate_index > 3 {
                    return Err(Error::Header);
                }
                // The packed ESP32 field is consumed without a byte swap
                // by the work verifier. A separate ntohl is for core routing.
                frame[2..6].copy_from_slice(&nonce.to_le_bytes());
                frame[6] = *midstate_tag;
                frame[7] = job_id | midstate_index;
                frame[8] = 0x80;
            }
        }
        frame[8] |= reply_crc51(frame[2..8].try_into().unwrap(), frame[8]);
        Ok(frame)
    }

    pub fn decode(frame: &[u8]) -> Result<Self, Error> {
        if frame.len() != 9 {
            return Err(Error::Length);
        }
        if frame[..2] != [0xaa, 0x55] {
            return Err(Error::Preamble);
        }
        if frame[8] & 0x60 != 0 {
            return Err(Error::Header);
        }
        if crc5(&frame[2..]) != 0 {
            return Err(Error::Checksum);
        }
        Ok(if frame[8] & 0x80 == 0 {
            Self::Register {
                value: u32::from_be_bytes(frame[2..6].try_into().unwrap()),
                asic_address: frame[6],
                register: frame[7],
            }
        } else {
            Self::Job {
                nonce: u32::from_le_bytes(frame[2..6].try_into().unwrap()),
                midstate_tag: frame[6],
                job_id: frame[7] & !3,
                midstate_index: frame[7] & 3,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent bit-array implementation from archived Kano bmcrc. Keep
    // its51-bit interface so a mistaken whole-byte transmit CRC cannot pass.
    fn kano_crc(data: &[u8], bits: usize) -> u8 {
        let mut c = [1; 5];
        for i in 0..bits {
            let old = c;
            c[0] = old[4] ^ u8::from(data[i / 8] & (0x80 >> (i % 8)) != 0);
            c[1] = old[0];
            c[2] = old[1] ^ c[0];
            c[3] = old[2];
            c[4] = old[3];
        }
        c.iter().enumerate().map(|(bit, value)| value << bit).sum()
    }

    #[test]
    fn wire_endianness_type_and_midstate_selection_match_archived_drivers() {
        let command = Reply::Register {
            value: 0x12345678,
            asic_address: 0xab,
            register: 0x08,
        };
        let frame = command.encode().unwrap();
        assert_eq!(
            &frame[..8],
            &[0xaa, 0x55, 0x12, 0x34, 0x56, 0x78, 0xab, 0x08]
        );
        assert_eq!(frame[8], kano_crc(&frame[2..], 51));
        assert_eq!(Reply::decode(&frame), Ok(command));
        let job = Reply::Job {
            nonce: 0x12345678,
            midstate_tag: 0x9a,
            job_id: 0x7c,
            midstate_index: 3,
        };
        let frame = job.encode().unwrap();
        assert_eq!(
            &frame[..8],
            &[0xaa, 0x55, 0x78, 0x56, 0x34, 0x12, 0x9a, 0x7f]
        );
        assert_eq!(frame[8], 0x80 | kano_crc(&frame[2..], 51));
        assert_eq!(Reply::decode(&frame), Ok(job));
        assert_ne!(crc5(&frame[2..8]), frame[8] & 0x1f);
    }

    #[test]
    fn crc_rejects_every_single_bit_error_and_noncanonical_flags() {
        let mut seed = 0x80ab12cd_u32;
        for _ in 0..512 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            for reply in [
                Reply::Register {
                    value: seed,
                    asic_address: seed as u8,
                    register: (seed >> 8) as u8,
                },
                Reply::Job {
                    nonce: seed,
                    midstate_tag: (seed >> 16) as u8,
                    job_id: seed as u8 & !3,
                    midstate_index: seed as u8 & 3,
                },
            ] {
                let frame = reply.encode().unwrap();
                assert_eq!(frame[8] & 0x1f, kano_crc(&frame[2..], 51));
                assert_eq!(Reply::decode(&frame), Ok(reply));
                for bit in 0..72 {
                    let mut corrupt = frame;
                    corrupt[bit / 8] ^= 1 << (bit % 8);
                    assert!(Reply::decode(&corrupt).is_err(), "{frame:02x?} bit{bit}");
                }
                for flag in [0x20, 0x40, 0x60] {
                    let mut corrupt = frame;
                    corrupt[8] = (corrupt[8] & 0x80) | flag;
                    corrupt[8] |= kano_crc(&corrupt[2..], 51);
                    assert_eq!(crc5(&corrupt[2..]), 0);
                    assert_eq!(Reply::decode(&corrupt), Err(Error::Header));
                }
            }
        }
        assert!(Reply::Job {
            nonce: 0,
            midstate_tag: 0,
            job_id: 1,
            midstate_index: 0
        }
        .encode()
        .is_err());
        assert!(Reply::Job {
            nonce: 0,
            midstate_tag: 0,
            job_id: 0,
            midstate_index: 4
        }
        .encode()
        .is_err());
        assert_eq!(Reply::decode(&[0xaa, 0x55]), Err(Error::Length));
    }
}

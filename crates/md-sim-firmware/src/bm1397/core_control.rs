//! Register0x3c words from the exact archived T17 miner ad090e7abbf525a7.
//! The original ARM packing routines were executed for49152 input cases.
//! See simulator/firmware/fixtures/t17-core-packing-results.json.
//! This decodes the driver's supported forms. It does not implement core
//! registers, invent reset values or establish completion of a mailbox write.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Register,
    Selector,
    UnsupportedWord,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    /// The vendor routine shifts its requested core number right by one.
    /// Keep the encoded selector separate from physical core geometry.
    Read { selector: u16, register: u8 },
    /// The vendor write routine sets bits31 and15 and clears the selector.
    /// Other write forms have no contract in this evidence.
    Write { register: u8, value: u8 },
}

impl Control {
    pub fn decode(word: u32) -> Result<Self, Error> {
        let register = ((word >> 8) & 15) as u8;
        if word & 0xffff_f000 == 0x8000_8000 {
            Ok(Self::Write {
                register,
                value: word as u8,
            })
        } else if word & !0x01ff_0fff == 0 && word & 255 == 255 {
            Ok(Self::Read {
                selector: ((word >> 16) & 0x1ff) as u16,
                register,
            })
        } else {
            Err(Error::UnsupportedWord)
        }
    }

    /// Reject out-of-range fields. The vendor routines truncate their C ABI
    /// arguments; a decoded wire command already has the narrower fields.
    pub fn encode(self) -> Result<u32, Error> {
        let (register, word) = match self {
            Self::Read { selector, register } => {
                if selector > 511 {
                    return Err(Error::Selector);
                }
                (register, (u32::from(selector) << 16) | 255)
            }
            Self::Write { register, value } => (register, 0x8000_8000 | u32::from(value)),
        };
        if register > 15 {
            return Err(Error::Register);
        }
        Ok(word | (u32::from(register) << 8))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_retained_vendor_execution_word_has_the_independent_fields() {
        let bytes =
            include_bytes!("../../../../simulator/firmware/fixtures/t17-core-packing-results.bin");
        assert_eq!(
            crate::prepare::hash_bytes(bytes),
            "7527abdd4015bbafa0dc1121733a9ff20c37799e142e11b6d2c69f57b45745d1"
        );
        assert_eq!(bytes.len(), 196608);
        let mut words = bytes
            .chunks_exact(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
        for core in 0..1024 {
            for register in 0..32 {
                let word = words.next().unwrap();
                let expected = Control::Read {
                    selector: core / 2,
                    register: register % 16,
                };
                assert_eq!(Control::decode(word), Ok(expected));
                assert_eq!(expected.encode(), Ok(word));
            }
        }
        for register in 0..32 {
            for value in 0..512 {
                let word = words.next().unwrap();
                let expected = Control::Write {
                    register: register % 16,
                    value: (value % 256) as u8,
                };
                assert_eq!(Control::decode(word), Ok(expected));
                assert_eq!(expected.encode(), Ok(word));
            }
        }
        assert!(words.next().is_none());
    }

    #[test]
    fn initialization_word_and_unknown_modes_remain_distinct() {
        assert_eq!(
            Control::decode(0x80008074),
            Ok(Control::Write {
                register: 0,
                value: 0x74
            })
        );
        for word in [
            0x4000,
            0x800000ff,
            0x80008174 | 0x10000,
            0x80ff,
            0x10ff,
            0x020000ff,
            0xfe,
        ] {
            assert_eq!(
                Control::decode(word),
                Err(Error::UnsupportedWord),
                "{word:08x}"
            );
        }
        assert_eq!(
            Control::decode(0x100ff),
            Ok(Control::Read {
                selector: 1,
                register: 0
            })
        );
        assert_eq!(
            Control::Read {
                selector: 512,
                register: 0
            }
            .encode(),
            Err(Error::Selector)
        );
        assert_eq!(
            Control::Read {
                selector: 0,
                register: 16
            }
            .encode(),
            Err(Error::Register)
        );
        assert_eq!(
            Control::Write {
                register: 16,
                value: 0
            }
            .encode(),
            Err(Error::Register)
        );
    }
}

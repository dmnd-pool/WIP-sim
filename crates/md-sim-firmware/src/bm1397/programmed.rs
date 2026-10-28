//! Programmed control state, separate from effective silicon state.
//!
//! Core writes use the executed T17 packing contract. PLL arithmetic uses
//! the unchanged T17 leaf checked by oracle496. No write proves completion,
//! internal core fan-out, clock output or PLL lock. These values are diagnostic
//! state; there is no guest readback until its complete response is known.
use super::{
    core_control::Control,
    uart_control::{FastUartParameter, MiscControlParameter},
    IdentifyError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ArithmeticError {
    ZeroDivisor,
}

/// A received parameter word. In particular, written bit31 is not an
/// observed LOCKED status. No effective-frequency or ready API is provided.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct PllParameter {
    pub written_word: u32,
}

impl PllParameter {
    /// The exact vendor arithmetic with its software-divider argument.
    /// This is a requested integer MHz value, independent of enable/status
    /// bits and physical operating ranges. Output-divider register encoding
    /// is a separate contract; callers must not pass that raw register here.
    pub fn driver_integer_mhz(self, software_divider: u32) -> Result<u32, ArithmeticError> {
        let feedback = (self.written_word >> 16) & 0xfff;
        let reference = (self.written_word >> 8) & 0x3f;
        let post1 = (self.written_word >> 4) & 7;
        let post2 = self.written_word & 7;
        let software = software_divider & 255;
        if reference == 0 || post1 == 0 || post2 == 0 || software == 0 {
            return Err(ArithmeticError::ZeroDivisor);
        }
        // All inputs fit exact f32 integers. The result is finite and in
        // 0..=102375. Do not replace the original sequence with one rational
        // division: oracle496 records four different truncation results.
        Ok(
            ((25.0f32 / reference as f32 * feedback as f32) / (post1 * post2 * software) as f32)
                as u32,
        )
    }
}

/// Raw PLL0 output-divider word. Original T17 at0x28e44..0x28e78
/// writes0x0f0f0f00 OR (software_divider_byte-1) to register0x70.
/// The upper output fields and hardware-valid ranges remain separate
/// contracts. This does not establish an effective divisor or clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Pll0Divider {
    pub written_word: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum DividerError {
    UnrepresentableSoftwareDivider,
}

impl Pll0Divider {
    /// Inverse of the original driver's low-byte packing for its nonzero
    /// software argument. The original arithmetic leaf masks that argument
    /// to8bits. Therefore encoded255 cannot become a usable divisor256 or
    /// silently wrap to zero. Raw register storage still preserves the word.
    pub fn driver_software_divider(self) -> Result<u8, DividerError> {
        (self.written_word as u8)
            .checked_add(1)
            .ok_or(DividerError::UnrepresentableSoftwareDivider)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct ProgrammedControls {
    /// Exact posted core-register0 byte, not committed per-core state.
    pub core0_write: Option<u8>,
    pub pll0_write: Option<PllParameter>,
    pub pll3_write: Option<PllParameter>,
    pub fast_uart_write: Option<FastUartParameter>,
    pub misc_write: Option<MiscControlParameter>,
    pub pll0_divider_write: Option<Pll0Divider>,
}

#[derive(Clone, Copy)]
pub(crate) enum Write {
    Core0(u8),
    Pll0(PllParameter),
    Pll3(PllParameter),
    FastUart(FastUartParameter),
    Misc(MiscControlParameter),
    Pll0Divider(Pll0Divider),
}

impl Write {
    /// Decode before changing any selected chip. Reserved or unknown core
    /// commands remain errors; raw PLL programming does not validate a VCO.
    pub(crate) fn decode(register: u8, value: u32) -> Result<Self, IdentifyError> {
        match register {
            0x3c => match Control::decode(value).map_err(|_| IdentifyError::InvalidControl)? {
                Control::Write { register: 0, value } => Ok(Self::Core0(value)),
                _ => Err(IdentifyError::UnsupportedCommand),
            },
            0x08 => Ok(Self::Pll0(PllParameter {
                written_word: value,
            })),
            0x68 => Ok(Self::Pll3(PllParameter {
                written_word: value,
            })),
            0x28 => Ok(Self::FastUart(FastUartParameter {
                written_word: value,
            })),
            0x18 => Ok(Self::Misc(MiscControlParameter {
                written_word: value,
            })),
            0x70 => Ok(Self::Pll0Divider(Pll0Divider {
                written_word: value,
            })),
            _ => Err(IdentifyError::UnsupportedCommand),
        }
    }

    pub(crate) fn apply(self, bank: &mut ProgrammedControls) {
        match self {
            Self::Core0(value) => bank.core0_write = Some(value),
            Self::Pll0(value) => bank.pll0_write = Some(value),
            Self::Pll3(value) => bank.pll3_write = Some(value),
            Self::FastUart(value) => bank.fast_uart_write = Some(value),
            Self::Misc(value) => bank.misc_write = Some(value),
            Self::Pll0Divider(value) => bank.pll0_divider_write = Some(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_retained_original_arithmetic_result_matches() {
        // Actual original ARM output, not results computed by this model.
        let bytes =
            include_bytes!("../../../../simulator/firmware/fixtures/t17-pll-frequency-results.bin");
        assert_eq!(
            crate::prepare::hash_bytes(bytes),
            "9fd7c05d9f83cdab32094688708aac380fd9a5997a2493be0a2515e6af17b13c"
        );
        assert_eq!(bytes.len(), 354160);
        let word = |chunk: &[u8]| u32::from_le_bytes(chunk.try_into().unwrap());
        let header: Vec<_> = bytes[..16].chunks_exact(4).map(word).collect();
        assert_eq!(header, [0x504c4c31, 1, 22134, 4]);
        for (id, row) in bytes[16..].chunks_exact(16).enumerate() {
            assert_eq!(word(&row[..4]), id as u32);
            let value = PllParameter {
                written_word: word(&row[4..8]),
            };
            assert_eq!(
                value.driver_integer_mhz(word(&row[8..12])),
                Ok(word(&row[12..16]))
            );
        }
    }

    #[test]
    fn invalid_divisors_remain_errors_and_status_bits_do_not_gate_arithmetic() {
        for word in [0x00480021, 0x00480201, 0x00480220] {
            assert_eq!(
                PllParameter { written_word: word }.driver_integer_mhz(1),
                Err(ArithmeticError::ZeroDivisor)
            );
        }
        for divisor in [0, 256, 0xffffff00] {
            assert_eq!(
                PllParameter {
                    written_word: 0xc0700111
                }
                .driver_integer_mhz(divisor),
                Err(ArithmeticError::ZeroDivisor)
            );
        }
        for word in [0x00700111, 0x40700111, 0x80700111, 0xc0700111] {
            assert_eq!(
                PllParameter { written_word: word }.driver_integer_mhz(1),
                Ok(2800)
            );
        }
        assert_eq!(
            PllParameter {
                written_word: 0x000f0311
            }
            .driver_integer_mhz(1),
            Ok(124)
        );
    }
}

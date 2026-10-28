//! Views of posted UART parameters, separate from effective line state.
//!
//! Original T17 code packs BT8D into bits8..12 and24..27 at0x3d30c/0x3d314.
//! Its formatter identifies BCLK_SEL at16 and PLL3_DIV4 at24..27. The archived
//! ESP driver uses CLKI/(8*(BT8D+1)). Its bits9..13 comment and115749 return
//! value disagree with its own0x00007a31 word and25MHz formula. Preserve the
//! word and exact ratio. No UART timing, PLL lock or source switch is inferred.

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct FastUartParameter {
    pub written_word: u32,
}

impl FastUartParameter {
    /// Raw field, not an effective divisor or a PLL-ready indication.
    pub fn pll3_div4_field(self) -> u8 {
        ((self.written_word >> 24) & 15) as u8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum BaudClock {
    Clki,
    Pll3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct MiscControlParameter {
    pub written_word: u32,
}

/// Exact requested bits per second is input_hz/cycles_per_bit. This is a
/// calculation from supplied inputs; it is not an observed operating rate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ClkiBaudRequest {
    pub input_hz: u32,
    pub cycles_per_bit: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum BaudRequestError {
    ZeroInputClock,
    Pll3Unsupported,
}

impl MiscControlParameter {
    pub fn baud_divider_field(self) -> u16 {
        (((self.written_word >> 24) & 15) as u16) * 32 + (((self.written_word >> 8) & 31) as u16)
    }

    pub fn baud_clock(self) -> BaudClock {
        if self.written_word & 0x0001_0000 == 0 {
            BaudClock::Clki
        } else {
            BaudClock::Pll3
        }
    }

    /// The caller must supply its declared CLKI clock. This method does not
    /// use a guessed reset clock or change the serial transport. The PLL3
    /// path is unsupported even if a PLL3 parameter word has been posted.
    pub fn requested_clki_baud(self, input_hz: u32) -> Result<ClkiBaudRequest, BaudRequestError> {
        if self.baud_clock() == BaudClock::Pll3 {
            return Err(BaudRequestError::Pll3Unsupported);
        }
        if input_hz == 0 {
            return Err(BaudRequestError::ZeroInputClock);
        }
        Ok(ClkiBaudRequest {
            input_hz,
            cycles_per_bit: (self.baud_divider_field() + 1) * 8,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_driver_words_preserve_exact_requested_ratios() {
        for (word, cycles) in [(0x00007a31, 216), (0x00006031, 8)] {
            let misc = MiscControlParameter { written_word: word };
            assert_eq!(
                misc.requested_clki_baud(25_000_000),
                Ok(ClkiBaudRequest {
                    input_hz: 25_000_000,
                    cycles_per_bit: cycles,
                })
            );
            assert_eq!(misc.written_word, word);
        }
        // Retain the fractional rate, rather than copying the erroneous
        // ESP return value115749 or silently rounding the wire clock.
        let default = MiscControlParameter {
            written_word: 0x00007a31,
        }
        .requested_clki_baud(25_000_000)
        .unwrap();
        assert_eq!(
            default.input_hz / u32::from(default.cycles_per_bit),
            115_740
        );
        assert_eq!(default.input_hz % u32::from(default.cycles_per_bit), 160);
        let fast = FastUartParameter {
            written_word: 0x0600000f,
        };
        assert_eq!(fast.pll3_div4_field(), 6);
        assert_eq!(fast.written_word, 0x0600000f);
    }

    #[test]
    fn split_divider_and_selector_are_distinct_from_unrelated_bits() {
        let contributions = [
            (8, 1),
            (9, 2),
            (10, 4),
            (11, 8),
            (12, 16),
            (24, 32),
            (25, 64),
            (26, 128),
            (27, 256),
        ];
        for bit in 0..32 {
            let misc = MiscControlParameter {
                written_word: 1 << bit,
            };
            let expected = contributions
                .iter()
                .find(|(b, _)| *b == bit)
                .map_or(0, |(_, v)| *v);
            assert_eq!(misc.baud_divider_field(), expected);
            assert_eq!(
                misc.baud_clock(),
                if bit == 16 {
                    BaudClock::Pll3
                } else {
                    BaudClock::Clki
                }
            );
        }
        for (word, divider) in [
            (0, 0),
            (0x01000000, 32),
            (0x0f001f00, 511),
            (0x0f001fff, 511),
        ] {
            let misc = MiscControlParameter { written_word: word };
            assert_eq!(misc.baud_divider_field(), divider);
            assert_eq!(
                misc.requested_clki_baud(u32::MAX).unwrap().cycles_per_bit,
                (divider + 1) * 8
            );
        }
    }

    #[test]
    fn unknown_clock_and_pll_source_never_get_a_fallback_rate() {
        let clki = MiscControlParameter {
            written_word: 0x00006031,
        };
        assert_eq!(
            clki.requested_clki_baud(0),
            Err(BaudRequestError::ZeroInputClock)
        );
        let pll = MiscControlParameter {
            written_word: 0x00016731,
        };
        for clock in [0, 25_000_000, u32::MAX] {
            assert_eq!(
                pll.requested_clki_baud(clock),
                Err(BaudRequestError::Pll3Unsupported)
            );
        }
        let different = clki.requested_clki_baud(24_000_000).unwrap();
        assert_eq!(
            different,
            ClkiBaudRequest {
                input_hz: 24_000_000,
                cycles_per_bit: 8
            }
        );
    }
}

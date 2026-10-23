//! Explicit low-rate adaptation of the ASIC's fixed filter condition.
//! Guest ticket bits keep their native positions and register readback.
use super::{ticket_filter, Error};
use md_sim_mining::target::Filter;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    fixed_zero_bits: u8,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            fixed_zero_bits: 32,
        }
    }
}

impl Policy {
    /// 32 selects the original contract. Every smaller value is an
    /// adaptation and requires its own run identity and manifest.
    pub fn from_fixed_bits(fixed_zero_bits: u8) -> Result<Self, Error> {
        if !(1..=32).contains(&fixed_zero_bits) {
            return Err(Error::Filter);
        }
        Ok(Self { fixed_zero_bits })
    }

    pub fn fixed_zero_bits(self) -> u8 {
        self.fixed_zero_bits
    }

    pub fn programmed_filter(self, ticket: u32) -> Filter {
        let Filter::ZeroMask(mut mask) = ticket_filter(ticket) else {
            unreachable!("native ticket filter is a zero mask")
        };
        mask[..4].copy_from_slice(&(u32::MAX << (32 - self.fixed_zero_bits)).to_be_bytes());
        Filter::ZeroMask(mask)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use md_sim_mining::{stratum::difficulty_target, Target};

    #[test]
    fn override_keeps_every_programmed_ticket_bit_and_has_exact_probability() {
        assert!(Policy::from_fixed_bits(0).is_err());
        assert!(Policy::from_fixed_bits(33).is_err());
        for bits in [1, 4, 12, 20, 31, 32] {
            let policy = Policy::from_fixed_bits(bits).unwrap();
            for ticket in [0, 0xff, 0x01020304, u32::MAX] {
                let filter = policy.programmed_filter(ticket);
                for byte in 0..32 {
                    for bit in 0..8 {
                        let mut digest = [0; 32];
                        digest[byte] = 1 << bit;
                        let fixed = 255 - (byte * 8 + bit) < usize::from(bits);
                        let programmed = (24..28).contains(&byte)
                            && ticket.to_be_bytes()[byte - 24] & (1 << bit) != 0;
                        assert_eq!(filter.accepts(&digest), !(fixed || programmed));
                    }
                }
                assert_eq!(
                    filter.intersection_probability(Target::MAX),
                    2f64.powi(-i32::from(bits) - ticket.count_ones() as i32)
                );
                if bits == 32 {
                    assert_eq!(filter, ticket_filter(ticket));
                }
            }
        }
        let filter = Policy::from_fixed_bits(12).unwrap().programmed_filter(0);
        // A stricter pool target still bounds the accepted-share rate.
        assert_eq!(
            filter.intersection_probability(difficulty_target(-19).unwrap()),
            difficulty_target(-19).unwrap().probability()
        );
        assert_eq!(
            filter
                .expected_interval_seconds(Target::MAX, 10_000.0)
                .unwrap(),
            4096.0 / 10_000.0
        );
    }
}

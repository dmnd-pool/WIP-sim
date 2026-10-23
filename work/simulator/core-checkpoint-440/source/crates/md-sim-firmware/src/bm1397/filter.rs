//! Ticket register0x14. The retained skot/BM1397 register reference
//! (provider view501c855b43ecf49a37575766454d1e4e439d0911f1b0c2f5bd2bef287642fe67)
//! requires32 leading zero bits and the byte-reversed ticket mask in the
//! following32 bits. Archived ESP-Miner's get_difficulty_mask() agrees with
//! this byte order. This is a reference filter, not physical validation.
use md_sim_mining::target::Filter;

/// Convert the exact programmed32-bit register to the existing engine filter.
/// Zero does not disable the fixed32-bit condition. No low-rate override is
/// implicit here. Such an adaptation needs a separate execution identity.
pub fn ticket_filter(register: u32) -> Filter {
    let mut mask = [0; 32];
    mask[..4].fill(0xff);
    mask[4..8].copy_from_slice(&register.to_le_bytes());
    Filter::ZeroMask(mask)
}

#[cfg(test)]
mod tests {
    use super::*;
    use md_sim_mining::target::Target;

    #[test]
    fn programmed_ticket_bits_and_fixed_filter_are_both_required() {
        // These register values are sent by the archived source for
        // difficulty1,256,512 and1024. Holes in a raw register stay holes.
        for (register, bytes) in [
            (0, [0, 0, 0, 0]),
            (0x000000ff, [0xff, 0, 0, 0]),
            (0x000080ff, [0xff, 0x80, 0, 0]),
            (0x0000c0ff, [0xff, 0xc0, 0, 0]),
            (0x01020304, [4, 3, 2, 1]),
            (u32::MAX, [0xff; 4]),
        ] {
            let filter = ticket_filter(register);
            for byte in 0..32 {
                for bit in 0..8 {
                    let mut digest = [0; 32];
                    digest[byte] = 1 << bit;
                    // Direct raw SHA digest test, independent of ZeroMask's
                    // numeric ordering: final4 bytes fixed; preceding4 ticket.
                    let rejected = byte >= 28
                        || ((24..28).contains(&byte) && bytes[27 - byte] & (1 << bit) != 0);
                    assert_eq!(filter.accepts(&digest), !rejected);
                }
            }
            assert!(filter.accepts(&[0; 32]));
        }
    }

    #[test]
    fn low_rate_deadlines_include_fixed_bits_and_pool_intersection() {
        for (register, bits) in [(0, 32), (0xff, 40), (0x80ff, 41), (u32::MAX, 64)] {
            let filter = ticket_filter(register);
            let probability = 2f64.powi(-bits);
            assert_eq!(filter.intersection_probability(Target::MAX), probability);
            assert_eq!(
                filter.intersection_probability(Target::from_compact(0x207fffff).unwrap()),
                probability
            );
            assert_eq!(
                filter
                    .expected_interval_seconds(Target::MAX, 10_000.0)
                    .unwrap(),
                1.0 / (10_000.0 * probability)
            );
            assert_eq!(
                filter.expected_interval_seconds(Target::MAX, 1.0).unwrap(),
                1.0 / probability
            );
        }
        // A pool target that also requires the first64 bits zero is harder
        // than an unmasked chip's fixed32-bit threshold.
        let mut target = [0xff; 32];
        target[..8].fill(0);
        assert_eq!(
            ticket_filter(0).intersection_probability(Target(target)),
            2f64.powi(-64)
        );
    }
}

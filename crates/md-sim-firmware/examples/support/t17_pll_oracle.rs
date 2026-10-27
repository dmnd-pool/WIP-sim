//! Original T17 driver arithmetic only. The inputs are register words and a
//! software divider. Arithmetic output does not establish any physical clock.
use super::readback::{exact_copy, LoadSegment};
use anyhow::{ensure, Context, Result};
use md_sim_firmware::prepare::hash_bytes;
use serde_json::{json, Value};

pub const DRIVER: &[u8] =
    include_bytes!("../../../../simulator/firmware/fixtures/bm1397-pll-frequency-oracle.c");
pub const LINK_SCRIPT: &[u8] =
    include_bytes!("../../../../simulator/firmware/fixtures/bm1397-pll-frequency-oracle.ld");
const CASE_COUNT: usize = 22134;

pub fn verify_copies(
    executable: &[u8],
    original: &[u8],
    segments: &[LoadSegment],
) -> Result<Vec<Value>> {
    let address = 0x28b2c;
    let offset = address as usize - 0x8000;
    let bytes = original
        .get(offset..offset + 108)
        .context("original PLL arithmetic range absent")?;
    let sha = "650ae97e0c4c042e4e89b91d2bdf995ce29433b0e3f7f8bd47cacd986155f4d7";
    ensure!(hash_bytes(bytes) == sha, "original PLL arithmetic differs");
    let at = exact_copy(executable, bytes, address, segments)?;
    Ok(vec![
        json!({"operation":"PLL word to integer frequency","source_file_offset":offset,
        "original_virtual_address":address,"oracle_virtual_address":address,
        "oracle_file_offset":at,"size":108,"sha256":sha}),
    ])
}

fn inputs() -> Vec<(u32, u32)> {
    let pack = |f: u32, r: u32, p: u32, q: u32| f << 16 | r << 8 | p << 4 | q;
    let mut values = Vec::with_capacity(CASE_COUNT);
    for f in [1, 15, 16, 72, 112, 250, 255, 256, 2047, 4095] {
        for r in [1, 2, 3, 7, 31, 63] {
            for p in 1..8 {
                for q in 1..8 {
                    for d in [1, 2, 3, 15, 16, 255] {
                        values.push((pack(f, r, p, q), d));
                    }
                }
            }
        }
    }
    values.extend((1..4096).map(|f| (pack(f, 1, 1, 1), 1)));
    values.extend((1..64).map(|r| (pack(4095, r, 3, 5), 3)));
    values.extend((1..256).map(|d| (pack(112, 2, 2, 1), d)));
    for word in [0xc0700111, 0x40480221, 0x00010111] {
        for ignored in [
            0,
            8,
            128,
            1 << 14,
            1 << 15,
            1 << 28,
            1 << 29,
            1 << 30,
            1 << 31,
        ] {
            for upper in [0, 0xff00, 0xffffff00] {
                values.push((word ^ ignored, upper | 1));
            }
        }
    }
    values
}

fn frequency(word: u32, divider: u32) -> Result<(u32, f64)> {
    let feedback = (word >> 16) & 4095;
    let reference = (word >> 8) & 63;
    let post1 = (word >> 4) & 7;
    let post2 = word & 7;
    let user = divider & 255;
    ensure!(
        reference != 0 && post1 != 0 && post2 != 0 && user != 0,
        "zero divisor is outside this arithmetic oracle"
    );
    let denominator = reference * post1 * post2 * user;
    let rational = f64::from(25 * feedback) / f64::from(denominator);
    // The driver returns a signed integer after three single-precision
    // operations. The separate rational bound checks the direct-divisor
    // interpretation independently of that operation sequence.
    let rounded = (25.0f32 / reference as f32 * feedback as f32) / (post1 * post2 * user) as f32;
    ensure!(
        rounded.is_finite() && (0.0..=102375.0).contains(&rounded),
        "driver arithmetic range"
    );
    let error_bound = (rational + 1.0) * 3.0 * f64::from(f32::EPSILON);
    ensure!(
        (f64::from(rounded) - rational).abs() <= error_bound,
        "single precision exceeds independent rational error bound"
    );
    Ok((rounded as u32, rational))
}

pub fn verify_output(bytes: &[u8]) -> Result<Value> {
    ensure!(
        bytes.len() == (4 + CASE_COUNT * 4) * 4,
        "PLL arithmetic output length"
    );
    let mut words = bytes
        .chunks_exact(4)
        .map(|x| u32::from_le_bytes(x.try_into().unwrap()));
    ensure!(
        words.next() == Some(0x504c4c31)
            && words.next() == Some(1)
            && words.next() == Some(CASE_COUNT as u32)
            && words.next() == Some(4),
        "PLL arithmetic output header"
    );
    let cases = inputs();
    ensure!(cases.len() == CASE_COUNT, "PLL arithmetic input count");
    let mut truncated_below_rational_floor = 0;
    for (id, (word, divider)) in cases.into_iter().enumerate() {
        ensure!(
            words.next() == Some(id as u32)
                && words.next() == Some(word)
                && words.next() == Some(divider),
            "PLL arithmetic input identity at{id}"
        );
        let actual = words.next().context("PLL arithmetic result absent")?;
        let (expected, rational) = frequency(word, divider)?;
        ensure!(actual==expected,"original PLL arithmetic differs at{id}: word{word:08x}, divider{divider:08x}, got{actual}, expected{expected}");
        if f64::from(actual) < rational.floor() {
            truncated_below_rational_floor += 1;
        }
    }
    ensure!(words.next().is_none(), "PLL arithmetic trailing words");
    Ok(
        json!({"cases":CASE_COUNT,"all_feedback_fields":4095,"all_reference_divisors":63,
        "all_user_divisors":255,"post_divisor_values_each":7,
        "single_precision_truncations_below_exact_rational_floor":truncated_below_rational_floor,
        "input_order":"Cross product; complete feedback sweep; reference sweep; user-divider sweep; ignored-bit/upper-divider probes",
        "reference":"25*feedback/reference/(post1*post2*user); original single-precision integer conversion checked with an independent rational error bound",
        "zero_divisors":"Rejected by verifier; original invalid-divisor behavior is not executed or claimed",
        "physical_validity":"No VCO range, lock, readiness, output routing or timing inference"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut words = vec![0x504c4c31, 1, CASE_COUNT as u32, 4];
        for (i, (word, divider)) in inputs().into_iter().enumerate() {
            words.extend([i as u32, word, divider, frequency(word, divider).unwrap().0]);
        }
        words.into_iter().flat_map(u32::to_le_bytes).collect()
    }
    #[test]
    fn independent_known_frequency_vectors_and_ignored_status_bits() {
        assert_eq!(frequency(0x40480221, 1).unwrap().0, 450);
        assert_eq!(frequency(0xc0700111, 1).unwrap().0, 2800);
        assert_eq!(frequency(0xc0600161, 1).unwrap().0, 400);
        assert_eq!(frequency(0x40480221, 3).unwrap().0, 150);
        assert_eq!(frequency(0x00480221, 0xffffff01).unwrap().0, 450);
        for word in [0x00480021, 0x00480201, 0x00480220] {
            assert!(frequency(word, 1).is_err());
        }
        assert!(frequency(0x40480221, 0).is_err());
        assert!(frequency(0x40480221, 256).is_err());
    }
    #[test]
    fn result_parser_rejects_wrong_values_identity_headers_and_lengths() {
        // Parser fixture only. This does not stand in for original execution.
        let bytes = fixture();
        assert_eq!(verify_output(&bytes).unwrap()["cases"], CASE_COUNT);
        for offset in [0, 4, 8, 12, 16, 20, 24, 28] {
            let mut damaged = bytes.clone();
            damaged[offset] ^= 1;
            assert!(verify_output(&damaged).is_err(), "offset{offset}");
        }
        assert!(verify_output(&bytes[..bytes.len() - 4]).is_err());
        let mut longer = bytes;
        longer.extend([0; 4]);
        assert!(verify_output(&longer).is_err());
    }
}

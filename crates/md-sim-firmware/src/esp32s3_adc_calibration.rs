//! ESP-IDF5.5.3 S3 calibration arithmetic over explicit QEMU eFuse bytes.
//! This decodes supplied data. It does not create trims or model an ADC.
//! Source identities are in reviews/esp32-adc-calibration-contract-509.json.
use serde::Serialize;

/// The pinned QEMU format has two 24-byte blocks and nine 32-byte blocks.
pub const IMAGE_SIZE: usize = 336;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Error {
    ImageSize,
    UnsupportedVersion { stored_major: u8 },
    InvalidSelection,
    RawOutOfRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Point {
    pub init_code: u16,
    pub reference_digit: u16,
    pub reference_mv: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Calibration {
    points: [[Point; 4]; 2],
}

fn field(block: &[u8], start: usize, width: usize) -> u16 {
    (0..width).fold(0, |value, bit| {
        value | (u16::from((block[(start + bit) / 8] >> ((start + bit) % 8)) & 1) << bit)
    })
}

impl Calibration {
    pub fn decode(image: &[u8]) -> Result<Self, Error> {
        if image.len() != IMAGE_SIZE {
            return Err(Error::ImageSize);
        }
        let block1 = &image[24..48];
        let block2 = &image[48..80];
        let major = field(block2, 128, 2) as u8;
        if major != 1 {
            return Err(Error::UnsupportedVersion {
                stored_major: major,
            });
        }
        let mut init = [[0u16; 4]; 2];
        for (unit, values) in init.iter_mut().enumerate() {
            let start = if unit == 0 { 149 } else { 175 };
            values[0] = field(block2, start, 8) + if unit == 0 { 1850 } else { 2020 };
            for atten in 1..4 {
                let offset = if unit == 0 { [0, 90, 0, 70][atten] } else { 0 };
                values[atten] =
                    values[atten - 1] + field(block2, start + 8 + (atten - 1) * 6, 6) + offset;
            }
        }
        let mut digit = [[0u16; 4]; 2];
        digit[0][3] = field(block2, 225, 8) + 900;
        for atten in (0..3).rev() {
            digit[0][atten] =
                digit[0][atten + 1] + field(block2, 201 + atten * 8, 8) + [800, 700, 800][atten];
        }
        for atten in 0..4 {
            let difference = if atten == 3 {
                field(block1, 186, 6)
            } else {
                field(
                    block2,
                    [233, 241, 248][atten],
                    if atten == 0 { 8 } else { 7 },
                )
            };
            digit[1][atten] = digit[0][atten] - difference + [40, 10, 20, 15][atten];
        }
        Ok(Self {
            points: std::array::from_fn(|u| {
                std::array::from_fn(|a| Point {
                    init_code: init[u][a],
                    reference_digit: digit[u][a],
                    reference_mv: 850,
                })
            }),
        })
    }

    /// Unit and attenuation are zero-based, as in the selected IDF API.
    pub fn point(&self, unit: usize, attenuation: usize) -> Result<Point, Error> {
        self.points
            .get(unit)
            .and_then(|p| p.get(attenuation))
            .copied()
            .ok_or(Error::InvalidSelection)
    }

    /// Reproduce the selected driver's integer conversion of a 12-bit sample.
    /// This is arithmetic consistency, not measured analog accuracy.
    pub fn raw_to_mv(&self, unit: usize, attenuation: usize, raw: u16) -> Result<i32, Error> {
        let point = self.point(unit, attenuation)?;
        if raw > 4095 {
            return Err(Error::RawOutOfRange);
        }
        let coefficient = 65536u64 * 850 / u64::from(point.reference_digit);
        let voltage = u64::from(raw) * coefficient / 65536;
        if voltage == 0 {
            return Ok(0);
        }
        let terms = if attenuation == 3 { 5 } else { 3 };
        let mut power = 1u64;
        let mut error = 0i32;
        for (term, &(numerator, denominator, sign)) in COEFFICIENTS[unit][attenuation]
            .iter()
            .take(terms)
            .enumerate()
        {
            // IDF multiplies uint64_t values before dividing. Some valid
            // 12-bit samples overflow that product. Preserve unsigned C
            // wrapping and per-term truncation, rather than widen the math.
            error += ((power.wrapping_mul(numerator) / denominator) as i32) * sign;
            if term + 1 < terms {
                power *= voltage;
            }
        }
        Ok(voltage as i32 - error)
    }
}

type Term = (u64, u64, i32);
const Z: Term = (0, 1, 0);
const COEFFICIENTS: [[[Term; 5]; 4]; 2] = [
    [
        [
            (27856531419538344, 10000000000000000, -1),
            (50871540569528, 10000000000000000, -1),
            (9798249589, 1000000000000000, 1),
            Z,
            Z,
        ],
        [
            (29831022915028695, 10000000000000000, -1),
            (49393185868806, 10000000000000000, -1),
            (101379430548, 10000000000000000, 1),
            Z,
            Z,
        ],
        [
            (23285545746296417, 10000000000000000, -1),
            (147640181047414, 10000000000000000, -1),
            (208385525314, 10000000000000000, 1),
            Z,
            Z,
        ],
        [
            (644403418269478, 1000000000000000, -1),
            (644334888647536, 10000000000000000, -1),
            (1297891447611, 10000000000000000, 1),
            (70769718, 1000000000000000, -1),
            (13515, 1000000000000000, 1),
        ],
    ],
    [
        [
            (25668651654328927, 10000000000000000, -1),
            (1353548869615, 10000000000000000, 1),
            (36615265189, 10000000000000000, 1),
            Z,
            Z,
        ],
        [
            (23690184690298404, 10000000000000000, -1),
            (66319894226185, 10000000000000000, -1),
            (118964995959, 10000000000000000, 1),
            Z,
            Z,
        ],
        [
            (9452499397020617, 10000000000000000, -1),
            (200996773954387, 10000000000000000, -1),
            (259011467956, 10000000000000000, 1),
            Z,
            Z,
        ],
        [
            (12247719764336924, 10000000000000000, 1),
            (755717904943462, 10000000000000000, -1),
            (1478791187119, 10000000000000000, 1),
            (79672528, 1000000000000000, -1),
            (15038, 1000000000000000, 1),
        ],
    ],
];

#[cfg(test)]
mod tests {
    use super::*;

    fn image(fill: u8) -> [u8; IMAGE_SIZE] {
        let mut image = [fill; IMAGE_SIZE];
        image[64] = (image[64] & !3) | 1;
        image
    }

    #[test]
    fn missing_version_and_wrong_sizes_are_errors() {
        assert_eq!(Calibration::decode(&[]), Err(Error::ImageSize));
        assert_eq!(
            Calibration::decode(&[0; IMAGE_SIZE - 1]),
            Err(Error::ImageSize)
        );
        assert_eq!(
            Calibration::decode(&[0; IMAGE_SIZE + 1]),
            Err(Error::ImageSize)
        );
        for version in [0, 2, 3] {
            let mut raw = image(0);
            raw[64] = version;
            assert_eq!(
                Calibration::decode(&raw),
                Err(Error::UnsupportedVersion {
                    stored_major: version
                })
            );
        }
    }

    #[test]
    fn independent_endpoint_points_include_the_cross_block_field() {
        let low = Calibration::decode(&image(0)).unwrap();
        let high = Calibration::decode(&image(255)).unwrap();
        for (cal, init, digit) in [
            (
                &low,
                [[1850, 1940, 1940, 2010], [2020, 2020, 2020, 2020]],
                [[3200, 2400, 1700, 900], [3240, 2410, 1720, 915]],
            ),
            (
                &high,
                [[2105, 2258, 2321, 2454], [2275, 2338, 2401, 2464]],
                [[4220, 3165, 2210, 1155], [4005, 3048, 2103, 1107]],
            ),
        ] {
            for u in 0..2 {
                for a in 0..4 {
                    assert_eq!(
                        cal.point(u, a),
                        Ok(Point {
                            init_code: init[u][a],
                            reference_digit: digit[u][a],
                            reference_mv: 850
                        })
                    );
                }
            }
        }
        // Only BLOCK1 bits186..191 affect ADC2 attenuation3. They must not
        // be taken from the adjacent BLOCK2 tail, or from unrelated bits.
        let mut raw = image(0);
        raw[47] = 0b10101000;
        let cal = Calibration::decode(&raw).unwrap();
        assert_eq!(cal.point(1, 3).unwrap().reference_digit, 873);
        for a in 0..3 {
            assert_eq!(cal.point(1, a), low.point(1, a));
        }
        for a in 0..4 {
            assert_eq!(cal.point(0, a), low.point(0, a));
        }
    }

    #[test]
    fn invalid_sample_and_selection_are_not_clamped() {
        let cal = Calibration::decode(&image(0)).unwrap();
        for (u, a) in [(2, 0), (0, 4), (usize::MAX, usize::MAX)] {
            assert_eq!(cal.point(u, a), Err(Error::InvalidSelection));
            assert_eq!(cal.raw_to_mv(u, a, 0), Err(Error::InvalidSelection));
        }
        for raw in [4096, u16::MAX] {
            assert_eq!(cal.raw_to_mv(0, 0, raw), Err(Error::RawOutOfRange));
        }
        for u in 0..2 {
            for a in 0..4 {
                assert_eq!(cal.raw_to_mv(u, a, 0), Ok(0));
            }
        }
    }

    #[test]
    fn adjacent_seven_bit_fields_do_not_overlap() {
        let baseline = Calibration::decode(&image(0)).unwrap();
        for (bit, delta1, delta2) in [(247, 64, 0), (248, 0, 1), (254, 0, 64), (255, 0, 0)] {
            let mut raw = image(0);
            raw[48 + bit / 8] |= 1 << (bit % 8);
            let cal = Calibration::decode(&raw).unwrap();
            assert_eq!(
                cal.point(1, 1).unwrap().reference_digit,
                baseline.point(1, 1).unwrap().reference_digit - delta1
            );
            assert_eq!(
                cal.point(1, 2).unwrap().reference_digit,
                baseline.point(1, 2).unwrap().reference_digit - delta2
            );
            assert_eq!(cal.point(1, 3), baseline.point(1, 3));
            for a in 0..4 {
                assert_eq!(cal.point(0, a), baseline.point(0, a));
            }
        }
    }

    #[test]
    fn all_samples_at_minimum_and_maximum_trim_return_without_overflow() {
        for fill in [0, 255] {
            let cal = Calibration::decode(&image(fill)).unwrap();
            for u in 0..2 {
                for a in 0..4 {
                    for raw in 0..=4095 {
                        assert!(cal.raw_to_mv(u, a, raw).is_ok());
                    }
                }
            }
        }
    }

    #[test]
    fn original_unsigned_product_wrap_is_preserved() {
        let cal = Calibration::decode(&image(0)).unwrap();
        // IDF uint64 term2 wraps before division:96, not1940. Together
        // with the other signed terms this produces error-1223 at v3867.
        assert_eq!(cal.raw_to_mv(0, 3, 4095), Ok(5090));
    }
}

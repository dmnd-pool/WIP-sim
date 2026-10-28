//! Requested clock view for the S3 UART. Source frequencies are explicit inputs.
//! ESP-IDF5.5.3 uart_ll.h SHA-256:
//! 2e2dd54fe649007daaddc37072af30871b4161254e9e2929028484bdb62b422d.
//! A requested ratio does not establish an effective clock or serial timing.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClockInputs {
    pub apb_hz: Option<u32>,
    pub rtc_hz: Option<u32>,
    pub xtal_hz: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Source {
    Apb,
    Rtc,
    Xtal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Error {
    ReservedRegisterBits,
    ReservedSource,
    MissingSelectedClock,
    FractionalPredividerUnsupported,
    ZeroBaudDivisor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Requests {
    pub source: Source,
    pub source_hz: u32,
    pub predivider: u16,
    pub divider_sixteenths: u16,
    /// Exact requested bits/s is numerator/denominator. Keep the fraction.
    pub numerator: u64,
    pub denominator: u64,
    /// Raw requested gate/reset bits. No FIFO or clock effect is inferred.
    pub clock_enable: bool,
    pub tx_clock_enable: bool,
    pub rx_clock_enable: bool,
    pub core_reset: bool,
    pub tx_reset: bool,
    pub rx_reset: bool,
}

pub fn decode(clk_conf: u32, clkdiv: u32, inputs: ClockInputs) -> Result<Requests, Error> {
    if clk_conf & !0x0fff_ffff != 0 || clkdiv & !0x00f0_0fff != 0 {
        return Err(Error::ReservedRegisterBits);
    }
    // The selected HAL does not configure the fractional source predivider.
    // Do not guess its hardware equation when A or B is nonzero.
    if clk_conf & 0xfff != 0 {
        return Err(Error::FractionalPredividerUnsupported);
    }
    let (source, hz) = match (clk_conf >> 20) & 3 {
        1 => (Source::Apb, inputs.apb_hz),
        2 => (Source::Rtc, inputs.rtc_hz),
        3 => (Source::Xtal, inputs.xtal_hz),
        _ => return Err(Error::ReservedSource),
    };
    let source_hz = hz
        .filter(|hz| *hz != 0)
        .ok_or(Error::MissingSelectedClock)?;
    let predivider = (((clk_conf >> 12) & 255) + 1) as u16;
    let divider_sixteenths = (((clkdiv & 4095) << 4) | ((clkdiv >> 20) & 15)) as u16;
    if divider_sixteenths == 0 {
        return Err(Error::ZeroBaudDivisor);
    }
    Ok(Requests {
        source,
        source_hz,
        predivider,
        divider_sixteenths,
        numerator: u64::from(source_hz) * 16,
        denominator: u64::from(predivider) * u64::from(divider_sixteenths),
        clock_enable: clk_conf & (1 << 22) != 0,
        tx_clock_enable: clk_conf & (1 << 24) != 0,
        rx_clock_enable: clk_conf & (1 << 25) != 0,
        core_reset: clk_conf & (1 << 23) != 0,
        tx_reset: clk_conf & (1 << 26) != 0,
        rx_reset: clk_conf & (1 << 27) != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn inputs() -> ClockInputs {
        ClockInputs {
            apb_hz: Some(80_000_000),
            rtc_hz: None,
            xtal_hz: Some(40_000_000),
        }
    }

    #[test]
    fn actual040_words_keep_both_fractional_baud_requests() {
        for (conf, divider, numerator, denominator) in [
            (0x03501000, 0x007002b6, 1_280_000_000, 22_222),
            (0x03500000, 0x007002b6, 1_280_000_000, 11_111),
            (0x03500000, 0x00900019, 1_280_000_000, 409),
        ] {
            let request = decode(conf, divider, inputs()).unwrap();
            assert_eq!(
                (request.numerator, request.denominator),
                (numerator, denominator)
            );
            assert_eq!(request.source, Source::Apb);
            assert!(request.clock_enable && request.tx_clock_enable && request.rx_clock_enable);
            assert!(!request.core_reset && !request.tx_reset && !request.rx_reset);
        }
        let startup = decode(0x03f01000, 0x2b6, inputs()).unwrap();
        assert_eq!(startup.source, Source::Xtal);
        assert_eq!(
            (startup.numerator, startup.denominator),
            (640_000_000, 22_208)
        );
        assert!(startup.core_reset);
        // A staged frequency is not the executing guest's final baud request.
        let final_request = decode(0x03500000, 0x00900019, inputs()).unwrap();
        assert_eq!(
            final_request.numerator / final_request.denominator,
            3_129_584
        );
        assert_ne!(
            final_request.numerator,
            3_125_000 * final_request.denominator
        );
    }

    #[test]
    fn unknown_sources_and_unsupported_encodings_have_no_fallback_rate() {
        for (conf, div, expected) in [
            (0x03400000, 1, Error::ReservedSource),
            (0x03600000, 1, Error::MissingSelectedClock),
            (0x03500000, 0, Error::ZeroBaudDivisor),
            (0x03500001, 1, Error::FractionalPredividerUnsupported),
            (0x03500040, 1, Error::FractionalPredividerUnsupported),
            (0xf3500000, 1, Error::ReservedRegisterBits),
            (0x03500000, 0x00001001, Error::ReservedRegisterBits),
        ] {
            assert_eq!(decode(conf, div, inputs()), Err(expected));
        }
        let mut clocks = inputs();
        clocks.apb_hz = Some(0);
        assert_eq!(
            decode(0x03500000, 1, clocks),
            Err(Error::MissingSelectedClock)
        );
    }

    #[test]
    fn source_predivider_extremes_and_raw_requests_are_independent() {
        let clocks = ClockInputs {
            apb_hz: Some(u32::MAX),
            rtc_hz: Some(8_000_000),
            xtal_hz: None,
        };
        let highest = decode(0x001ff000, 0x00f00fff, clocks).unwrap();
        assert_eq!(
            (highest.predivider, highest.divider_sixteenths),
            (256, 65535)
        );
        assert_eq!(
            (highest.numerator, highest.denominator),
            (68_719_476_720, 16_776_960)
        );
        let rtc = decode(0x00200000, 1, clocks).unwrap();
        assert_eq!(rtc.source, Source::Rtc);
        for bit in [22, 23, 24, 25, 26, 27] {
            let request = decode(0x00100000 | (1 << bit), 1, clocks).unwrap();
            assert_eq!(request.numerator, u64::from(u32::MAX) * 16);
            assert_eq!(request.denominator, 16);
            assert_eq!(
                [
                    request.clock_enable,
                    request.core_reset,
                    request.tx_clock_enable,
                    request.rx_clock_enable,
                    request.tx_reset,
                    request.rx_reset
                ]
                .into_iter()
                .filter(|b| *b)
                .count(),
                1
            );
        }
    }
}

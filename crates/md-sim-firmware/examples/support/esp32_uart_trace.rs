//! Audit programmed UART observations. This does not infer effective baud.
//! Masks are from the archived ESP-IDF5.5.3 S3 uart_reg.h, SHA-256
//! eef72592d7949e641faf7c1d153a8bb0eb0e851619a5f579b4a3c3c663d67e07.
use anyhow::{ensure, Context, Result};
use serde::Serialize;
use std::collections::BTreeMap;

const WRITE: &str = "esp32s3_uart_config_write ";
const OVERFLOW: &str = "esp32s3_uart_config_overflow ";
const LIMIT: u64 = 256;

#[derive(Debug, Serialize)]
pub struct Write {
    pub device: String,
    pub virtual_ns: u64,
    pub sequence: u64,
    pub address: u64,
    pub size: u64,
    pub value: u64,
    pub stored: u64,
}

#[derive(Default, Debug, Serialize)]
pub struct Device {
    pub writes: u64,
    pub last_virtual_ns: u64,
    pub overflow: bool,
}

#[derive(Debug, Serialize)]
pub struct Trace {
    pub complete: bool,
    pub devices: BTreeMap<String, Device>,
    pub writes: Vec<Write>,
}

fn fields(line: &str, expected: &[&str]) -> Result<BTreeMap<String, String>> {
    let mut fields = BTreeMap::new();
    for token in line.split_whitespace() {
        let (key, value) = token.split_once('=').context("UART trace field")?;
        ensure!(expected.contains(&key), "unknown UART trace field");
        ensure!(
            fields.insert(key.into(), value.into()).is_none(),
            "duplicate UART trace field"
        );
    }
    ensure!(fields.len() == expected.len(), "missing UART trace field");
    Ok(fields)
}

fn number(fields: &BTreeMap<String, String>, key: &str) -> Result<u64> {
    let raw = &fields[key];
    Ok(if let Some(hex) = raw.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)?
    } else {
        raw.parse()?
    })
}

pub fn parse(log: &str) -> Result<Trace> {
    let mut trace = Trace {
        complete: true,
        devices: BTreeMap::new(),
        writes: Vec::new(),
    };
    for line in log.lines() {
        let (data, overflow) = if let Some(data) = line.strip_prefix(WRITE) {
            (data, false)
        } else if let Some(data) = line.strip_prefix(OVERFLOW) {
            (data, true)
        } else {
            ensure!(
                !line.starts_with("esp32s3_uart_config_"),
                "unknown or incomplete UART observation event"
            );
            continue;
        };
        let values = fields(
            data,
            if overflow {
                &["device", "virtual_ns", "limit"]
            } else {
                &[
                    "device",
                    "virtual_ns",
                    "sequence",
                    "address",
                    "size",
                    "value",
                    "stored",
                ]
            },
        )?;
        let name = &values["device"];
        ensure!(name.starts_with("/machine/"), "UART device path absent");
        let virtual_ns = number(&values, "virtual_ns")?;
        ensure!(
            trace.devices.contains_key(name) || trace.devices.len() < 16,
            "UART trace device bound"
        );
        let device = trace.devices.entry(name.clone()).or_default();
        ensure!(
            !device.overflow && virtual_ns >= device.last_virtual_ns,
            "UART trace repeats overflow or reverses device time"
        );
        device.last_virtual_ns = virtual_ns;
        if overflow {
            ensure!(
                number(&values, "limit")? == LIMIT && device.writes == LIMIT,
                "UART overflow does not follow its complete write budget"
            );
            device.overflow = true;
            trace.complete = false;
            continue;
        }
        let write = Write {
            device: name.clone(),
            virtual_ns,
            sequence: number(&values, "sequence")?,
            address: number(&values, "address")?,
            size: number(&values, "size")?,
            value: number(&values, "value")?,
            stored: number(&values, "stored")?,
        };
        ensure!(
            write.sequence == device.writes && device.writes < LIMIT,
            "UART trace sequence is missing, duplicated or beyond its budget"
        );
        ensure!(matches!(write.size, 1 | 2 | 4 | 8), "UART access width");
        let mask = match write.address {
            0x14 => 0x00f00fff,
            0x78 => 0x0fffffff,
            _ => anyhow::bail!("unexpected UART observation address"),
        };
        ensure!(
            write.stored == write.value & mask,
            "UART stored word differs"
        );
        device.writes += 1;
        trace.writes.push(write);
    }
    ensure!(!trace.writes.is_empty(), "UART observations absent");
    Ok(trace)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIRST: &str = "esp32s3_uart_config_write device=/machine/soc/uart1 virtual_ns=100 sequence=0 address=0x78 size=4 value=0xf3501000 stored=0x3501000\n";

    #[test]
    fn literal_register_observations_keep_raw_bits_and_isolate_devices() {
        let input = format!("unrelated guest output\n{FIRST}esp32s3_uart_config_write device=/machine/soc/uart0 virtual_ns=0 sequence=0 address=0x14 size=4 value=0xffa00abc stored=0xa00abc\nesp32s3_uart_config_write device=/machine/soc/uart1 virtual_ns=100 sequence=1 address=0x14 size=4 value=0xffffffff stored=0xf00fff\n");
        let trace = parse(&input).unwrap();
        assert!(trace.complete);
        assert_eq!(trace.devices.len(), 2);
        assert_eq!(trace.devices["/machine/soc/uart1"].writes, 2);
        assert_eq!(trace.writes[0].value, 0xf3501000);
        assert_eq!(trace.writes[0].stored, 0x03501000);
    }

    #[test]
    fn malformed_missing_duplicate_and_reordered_observations_fail() {
        for bad in [
            FIRST.replace("sequence=0", "sequence=1"),
            FIRST.replace("stored=0x3501000", "stored=0xf3501000"),
            FIRST.replace("address=0x78", "address=0x7c"),
            FIRST.replace("size=4", "size=3"),
            FIRST.replace("virtual_ns=100", "virtual_ns=-1"),
            FIRST.replace("stored=0x3501000", "stored=not-a-number"),
            FIRST.replace("stored=0x3501000", ""),
            FIRST.replace("sequence=0", "sequence=0 sequence=0"),
            FIRST.replace("sequence=0", "sequence=0 unknown=1"),
            FIRST.replace("device=/machine/soc/uart1", "device=uart1"),
            format!("{FIRST}{FIRST}"),
            format!(
                "{FIRST}{}",
                FIRST.replace("virtual_ns=100 sequence=0", "virtual_ns=99 sequence=1")
            ),
            format!("{FIRST}esp32s3_uart_config_write"),
            format!("{FIRST}esp32s3_uart_config_unknown device=/machine/soc/uart1"),
            String::new(),
        ] {
            assert!(parse(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn overflow_is_explicit_incomplete_and_reset_cannot_renew_sequence() {
        let mut log = String::new();
        for i in 0..LIMIT {
            log.push_str(&FIRST.replace("sequence=0", &format!("sequence={i}")));
        }
        let marker =
            "esp32s3_uart_config_overflow device=/machine/soc/uart1 virtual_ns=100 limit=256\n";
        let complete = parse(&log).unwrap();
        assert!(complete.complete);
        assert_eq!(complete.writes.len(), 256);
        assert!(parse(&format!("{log}{FIRST}")).is_err());
        log.push_str(marker);
        let trace = parse(&log).unwrap();
        assert!(!trace.complete);
        assert!(trace.devices["/machine/soc/uart1"].overflow);
        assert!(parse(&format!("{FIRST}{marker}")).is_err());
        assert!(parse(&format!("{log}{marker}")).is_err());
        assert!(parse(&format!("{log}{FIRST}")).is_err());
    }
}

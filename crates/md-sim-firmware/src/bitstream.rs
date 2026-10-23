//! Bounded Xilinx .bit container decoding for prepared firmware bundles.
//! Metadata and payload identity do not prove a device model implements the
//! configuration. The separate `series7` module checks a supported configuration
//! packet stream; it does not simulate the programmed logic.
use anyhow::{ensure, Context, Result};
use serde::Serialize;
use std::io::{Cursor, Read};

pub mod port;
pub mod series7;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Header {
    pub design: String,
    pub part: String,
    pub date: String,
    pub time: String,
    pub configuration_offset: usize,
    pub configuration_size: usize,
}

pub struct Container {
    bytes: Vec<u8>,
    header: Header,
}

impl Container {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn configuration(&self) -> &[u8] {
        &self.bytes[self.header.configuration_offset..]
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Decode exactly one gzip member. Reject trailing input, failed gzip CRC,
/// excessive expansion and unsupported container structure.
pub fn from_gzip(input: &[u8], max_bytes: usize) -> Result<Container> {
    ensure!(
        (1..=64 * 1024 * 1024).contains(&max_bytes) && input.len() <= max_bytes,
        "bitstream size policy"
    );
    let mut decoder = flate2::bufread::GzDecoder::new(Cursor::new(input));
    let mut bytes = Vec::new();
    decoder
        .by_ref()
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= max_bytes, "bitstream expansion limit");
    ensure!(
        decoder.into_inner().position() == input.len() as u64,
        "trailing gzip input"
    );
    let header = parse_header(&bytes)?;
    Ok(Container { bytes, header })
}

/// Parse the supported tagged .bit header. Preserve metadata as text, not
/// trusted hardware or source identity. Require exact payload length.
pub fn parse_header(bytes: &[u8]) -> Result<Header> {
    const PREFIX: &[u8] = &[
        0, 9, 0x0f, 0xf0, 0x0f, 0xf0, 0x0f, 0xf0, 0x0f, 0xf0, 0, 0, 1,
    ];
    ensure!(bytes.starts_with(PREFIX), "unsupported bitstream prefix");
    let mut rest = &bytes[PREFIX.len()..];
    fn field(rest: &mut &[u8], key: u8) -> Result<String> {
        ensure!(rest.len() >= 3 && rest[0] == key, "bitstream field tag");
        let size = u16::from_be_bytes([rest[1], rest[2]]) as usize;
        ensure!((2..=1024).contains(&size), "bitstream field size");
        let value = rest.get(3..3 + size).context("short bitstream field")?;
        ensure!(
            value.last() == Some(&0) && value[..size - 1].iter().all(|b| (0x20..=0x7e).contains(b)),
            "bitstream field encoding"
        );
        *rest = &rest[3 + size..];
        Ok(std::str::from_utf8(&value[..size - 1])?.to_owned())
    }
    let design = field(&mut rest, b'a')?;
    let part = field(&mut rest, b'b')?;
    let date = field(&mut rest, b'c')?;
    let time = field(&mut rest, b'd')?;
    ensure!(
        rest.len() >= 5 && rest[0] == b'e',
        "configuration field tag"
    );
    let size = u32::from_be_bytes(rest[1..5].try_into()?) as usize;
    ensure!(
        size > 0 && size % 4 == 0 && rest.len() - 5 == size,
        "configuration size differs"
    );
    Ok(Header {
        design,
        part,
        date,
        time,
        configuration_offset: bytes.len() - size,
        configuration_size: size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture() -> Vec<u8> {
        hex::decode("00090ff00ff00ff00ff00000016100024400620002500063000b323032302f31322f30340064000931353a34363a3237006500000004aa995566").unwrap()
    }

    fn compress(bytes: &[u8]) -> Vec<u8> {
        let mut out = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        out.write_all(bytes).unwrap();
        out.finish().unwrap()
    }

    #[test]
    fn tagged_header_preserves_metadata_and_payload_boundaries() {
        let bytes = fixture();
        let decoded = from_gzip(&compress(&bytes), 4096).unwrap();
        assert_eq!(decoded.bytes, bytes);
        assert_eq!(
            decoded.header,
            Header {
                design: "D".into(),
                part: "P".into(),
                date: "2020/12/04".into(),
                time: "15:46:27".into(),
                configuration_offset: 54,
                configuration_size: 4,
            }
        );
        assert_eq!(decoded.configuration(), [0xaa, 0x99, 0x55, 0x66]);
    }

    #[test]
    fn malformed_fields_truncation_and_extra_bytes_are_rejected() {
        let bytes = fixture();
        for end in 0..bytes.len() {
            assert!(parse_header(&bytes[..end]).is_err(), "truncation{end}");
        }
        for offset in 0..54 {
            let mut changed = bytes.clone();
            changed[offset] ^= 0x80;
            assert!(parse_header(&changed).is_err(), "header byte{offset}");
        }
        let mut extra = bytes;
        extra.push(0);
        assert!(parse_header(&extra).is_err());
    }

    #[test]
    fn gzip_checksum_limits_and_single_member_are_enforced() {
        let bytes = fixture();
        let compressed = compress(&bytes);
        assert!(from_gzip(&compressed, bytes.len() - 1).is_err());
        let mut expanded = bytes[..54].to_vec();
        expanded[50..54].copy_from_slice(&4096u32.to_be_bytes());
        expanded.resize(54 + 4096, 0);
        let small = compress(&expanded);
        assert!(small.len() < 256);
        assert!(from_gzip(&small, 256).is_err());
        let mut bad_crc = compressed.clone();
        let end = bad_crc.len();
        bad_crc[end - 8] ^= 1;
        assert!(from_gzip(&bad_crc, 4096).is_err());
        let mut extra = compressed.clone();
        extra.push(0);
        assert!(from_gzip(&extra, 4096).is_err());
        extra = compressed.clone();
        extra.extend_from_slice(&compressed);
        assert!(from_gzip(&extra, 4096).is_err());
        assert!(from_gzip(&compressed, 0).is_err());
        assert!(from_gzip(&compressed, 64 * 1024 * 1024 + 1).is_err());
    }
}

//! Map one complete, checked FDRI payload using a pinned Project X-Ray part.
//! FAR ordering and two zero frames at row boundaries follow Project X-Ray's
//! xc7series Part and Configuration. This does not decode logic features.
use anyhow::{bail, ensure, Context, Result};
use serde::{de::MapAccess, Deserialize, Deserializer, Serialize};
use std::{collections::BTreeMap, fmt, marker::PhantomData};

const WORDS: usize = 101;
const FRAME_BYTES: usize = WORDS * 4;
const MAX_FRAMES: usize = 131072;

#[derive(Debug)]
pub struct Part {
    idcode: u32,
    sha256: String,
    addresses: Vec<u32>,
}

impl Part {
    /// Identity is mandatory. A silicon ID alone does not select a package.
    pub fn from_pinned_json(bytes: &[u8], expected_sha256: &str) -> Result<Self> {
        ensure!(bytes.len() <= 1024 * 1024, "part database size limit");
        let sha256 = crate::prepare::hash_bytes(bytes);
        ensure!(sha256 == expected_sha256, "part database hash mismatch");
        let raw: RawPart = serde_json::from_slice(bytes).context("part database schema")?;
        ensure!(raw.idcode != 0, "missing silicon identity");
        let mut addresses = Vec::new();
        for (half, region) in [
            (0, &raw.global_clock_regions.top),
            (1, &raw.global_clock_regions.bottom),
        ] {
            ensure!(!region.rows.is_empty(), "empty clock region");
            let mut buses = None;
            for (row_key, row) in &region.rows {
                let row_number = index(row_key, 32)?;
                let row_buses: Vec<_> = row.configuration_buses.keys().collect();
                ensure!(!row_buses.is_empty(), "empty configuration row");
                if let Some(previous) = &buses {
                    ensure!(previous == &row_buses, "unsupported missing bus in row");
                } else {
                    buses = Some(row_buses);
                }
                for (bus, columns) in &row.configuration_buses {
                    let block = match bus.as_str() {
                        "CLB_IO_CLK" => 0,
                        "BLOCK_RAM" => 1,
                        "CFG_CLB" => 2,
                        _ => bail!("unsupported configuration bus {bus}"),
                    };
                    ensure!(
                        columns.configuration_columns.contains_key("0"),
                        "configuration row must start at column zero"
                    );
                    for (column, frames) in &columns.configuration_columns {
                        let column = index(column, 1024)?;
                        ensure!(
                            (1..=128).contains(&frames.frame_count),
                            "frame minor count out of range"
                        );
                        ensure!(
                            addresses.len() + frames.frame_count as usize <= MAX_FRAMES,
                            "part frame count limit"
                        );
                        for minor in 0..frames.frame_count {
                            addresses.push(
                                (block << 23)
                                    | (half << 22)
                                    | (row_number << 17)
                                    | (column << 7)
                                    | minor,
                            );
                        }
                    }
                }
            }
            ensure!(
                region.rows.contains_key("0"),
                "clock region must start at row zero"
            );
        }
        // FAR auto-increment order is block, half, row, column, minor. The
        // source JSON's string keys have a different lexical order.
        ensure!(
            raw.global_clock_regions.top.rows["0"]
                .configuration_buses
                .keys()
                .eq(raw.global_clock_regions.bottom.rows["0"]
                    .configuration_buses
                    .keys()),
            "unsupported missing bus in half"
        );
        addresses.sort_unstable();
        ensure!(
            addresses.first() == Some(&0),
            "full configuration must start at FAR zero"
        );
        Ok(Self {
            idcode: raw.idcode,
            sha256,
            addresses,
        })
    }

    pub fn addresses(&self) -> &[u32] {
        &self.addresses
    }

    pub fn idcode(&self) -> u32 {
        self.idcode
    }
}

fn index(key: &str, limit: u32) -> Result<u32> {
    let value = key.parse::<u32>().context("part index must be numeric")?;
    ensure!(
        value < limit && value.to_string() == key,
        "part index out of range or noncanonical"
    );
    Ok(value)
}

#[derive(Debug, Serialize)]
pub struct Frame {
    pub address: u32,
    /// Offset in the configuration payload, not the .bit container.
    pub byte_offset: usize,
    pub sha256: String,
    pub ecc: u16,
}

#[derive(Debug, Serialize)]
pub struct Padding {
    pub after_address: u32,
    pub byte_offset: usize,
    pub words: usize,
}

#[derive(Debug, Serialize)]
pub struct Mapping {
    pub part_sha256: String,
    pub inspection: super::Inspection,
    pub frames: Vec<Frame>,
    pub padding: Vec<Padding>,
    pub mapped_data_words: usize,
    pub checked_padding_words: usize,
}

/// Check packets and CRCs again, then account for every FDRI word. Reject
/// partial configurations, nonzero padding, missing data and excess data.
pub fn map_configuration(bytes: &[u8], part: &Part) -> Result<Mapping> {
    let inspection = super::inspect(bytes, part.idcode)?;
    ensure!(
        inspection.frame_data.initial_far == 0,
        "partial configuration FAR unsupported"
    );
    let mut at = inspection.frame_data.byte_offset;
    let end = at + inspection.frame_data.words * 4; // inspect checked bounds
    let mut frames = Vec::with_capacity(part.addresses.len());
    let mut padding = Vec::new();
    for (i, &address) in part.addresses.iter().enumerate() {
        ensure!(
            at + FRAME_BYTES <= end,
            "missing data at frame {address:08x}"
        );
        let data = &bytes[at..at + FRAME_BYTES];
        let expected = u32::from_be_bytes(data[200..204].try_into()?) as u16 & 0x1fff;
        let computed = frame_ecc(data)?;
        ensure!(
            expected == computed,
            "frame ECC mismatch at {address:08x}: expected {expected:04x}, computed {computed:04x}"
        );
        frames.push(Frame {
            address,
            byte_offset: at,
            sha256: crate::prepare::hash_bytes(data),
            ecc: computed,
        });
        at += FRAME_BYTES;
        // Column/minor bits are below bit17. Any higher-field change starts
        // another row, half or bus. The final row also needs two zero frames.
        if part
            .addresses
            .get(i + 1)
            .is_none_or(|next| next >> 17 != address >> 17)
        {
            ensure!(
                at + 2 * FRAME_BYTES <= end,
                "missing row padding after {address:08x}"
            );
            ensure!(
                bytes[at..at + 2 * FRAME_BYTES].iter().all(|&b| b == 0),
                "nonzero row padding after {address:08x}"
            );
            padding.push(Padding {
                after_address: address,
                byte_offset: at,
                words: 2 * WORDS,
            });
            at += 2 * FRAME_BYTES;
        }
    }
    ensure!(at == end, "unmapped FDRI data after last part frame");
    Ok(Mapping {
        part_sha256: part.sha256.clone(),
        inspection,
        mapped_data_words: frames.len() * WORDS,
        checked_padding_words: padding.len() * 2 * WORDS,
        frames,
        padding,
    })
}

/// Series-7's 13-bit frame error-correcting code. This checks the code; it
/// never repairs firmware bytes. Bit labels follow Project X-Ray ecc.cc.
pub fn frame_ecc(frame: &[u8]) -> Result<u16> {
    ensure!(
        frame.len() == FRAME_BYTES,
        "ECC requires one 101-word frame"
    );
    let mut ecc = 0u16;
    for (index, bytes) in frame.chunks_exact(4).enumerate() {
        let mut data = u32::from_be_bytes(bytes.try_into()?);
        if index == 50 {
            data &= 0xffffe000; // The stored code does not feed itself.
        }
        let origin = index as u16 * 32
            + if index > 37 {
                0x1360
            } else if index > 6 {
                0x1340
            } else {
                0x1320
            };
        while data != 0 {
            let bit = data.trailing_zeros() as u16;
            ecc ^= origin + bit;
            data &= data - 1;
        }
    }
    Ok(ecc ^ (((ecc & 0xfff).count_ones() as u16 & 1) << 12))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPart {
    idcode: u32,
    global_clock_regions: Regions,
    // Package metadata is retained in the pinned input, not interpreted as
    // evidence for pin wiring. Still reject duplicate keys in this map.
    #[serde(default, deserialize_with = "unique_map", rename = "iobanks")]
    _iobanks: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Regions {
    top: Region,
    bottom: Region,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Region {
    #[serde(deserialize_with = "unique_map")]
    rows: BTreeMap<String, Row>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    #[serde(deserialize_with = "unique_map")]
    configuration_buses: BTreeMap<String, Bus>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bus {
    #[serde(deserialize_with = "unique_map")]
    configuration_columns: BTreeMap<String, Column>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Column {
    frame_count: u32,
}

fn unique_map<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> std::result::Result<BTreeMap<String, T>, D::Error> {
    struct Visitor<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Visitor<T> {
        type Value = BTreeMap<String, T>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a part database map with distinct keys")
        }
        fn visit_map<A: MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, T>()? {
                if result.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate part database key"));
                }
            }
            Ok(result)
        }
    }
    d.deserialize_map(Visitor(PhantomData))
}

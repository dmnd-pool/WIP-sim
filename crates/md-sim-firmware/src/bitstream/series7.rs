//! Strict inspection of an unencrypted Series-7 configuration packet stream.
//! Uses the packet fields and ICAP CRC described by Project X-Ray and UG470.
//! CRC covers register writes, not board compatibility or implemented logic.
//! Unsupported streams fail explicitly. No configuration words are skipped.
use anyhow::{bail, ensure, Context, Result};
use serde::Serialize;

pub mod frames;

const MAX_BYTES: usize = 64 << 20;
const MAX_PACKETS: usize = 65536;
const SYNC: u32 = 0xaa995566;
const NOP: u32 = 0x20000000;

#[derive(Debug, Serialize)]
pub struct RegisterWrite {
    pub byte_offset: usize,
    pub address: u8,
    pub value: u32,
}

#[derive(Debug, Serialize)]
pub struct CrcCheck {
    pub byte_offset: usize,
    pub expected: u32,
    pub computed: u32,
}

#[derive(Debug, Serialize)]
pub struct FrameData {
    /// Offset in the configuration payload, not the .bit container.
    pub byte_offset: usize,
    pub words: usize,
    pub initial_far: u32,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
pub struct Inspection {
    pub sync_byte_offset: usize,
    pub idcode: u32,
    pub packet_count: usize,
    pub nop_count: usize,
    pub zero_padding_words: usize,
    pub register_writes: Vec<RegisterWrite>,
    pub crc_checks: Vec<CrcCheck>,
    pub frame_data: FrameData,
}

/// Inspect one full configuration with a single FDRI write. Frame-address
/// expansion and row padding require a separately pinned part database.
/// An IDCODE match only identifies the silicon, not the package or board.
pub fn inspect(bytes: &[u8], expected_idcode: u32) -> Result<Inspection> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_BYTES && bytes.len() % 4 == 0,
        "Series-7 configuration size policy"
    );
    let word = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
    let sync = (0..bytes.len().min(4096))
        .step_by(4)
        .find(|&at| word(at) == SYNC)
        .context("missing aligned Series-7 sync word")?;
    // Accept the documented width-detection sequence, with optional dummy
    // words around it. Do not hide arbitrary words before the sync marker.
    let prefix: Vec<_> = (0..sync)
        .step_by(4)
        .map(word)
        .filter(|&v| v != u32::MAX)
        .collect();
    ensure!(
        prefix.is_empty() || prefix == [0xbb, 0x11220044],
        "unsupported Series-7 preamble"
    );
    let mut state = State::default();
    let mut at = sync + 4;
    let mut packets = 0;
    let mut nops = 0;
    let mut padding = 0;
    let mut extension = None;
    while at < bytes.len() {
        ensure!(packets < MAX_PACKETS, "Series-7 packet count limit");
        packets += 1;
        let header_at = at;
        let header = word(at);
        at += 4;
        if header == NOP || header == 0 {
            ensure!(extension.is_none(), "missing Type-2 extension");
            if header == NOP {
                nops += 1;
            } else {
                ensure!(state.desync, "zero word before DESYNC");
                padding += 1;
            }
            continue;
        }
        ensure!(!state.desync, "configuration data after DESYNC");
        ensure!((header >> 27) & 3 == 2, "unsupported configuration opcode");
        let (address, count) = match header >> 29 {
            1 => {
                ensure!(extension.is_none(), "missing Type-2 extension");
                ensure!(header & 0x1800 == 0, "reserved Type-1 header bits");
                let address = (header >> 13) & 0x3fff;
                ensure!(address <= 31, "unsupported configuration register address");
                (address as u8, (header & 0x7ff) as usize)
            }
            2 => (
                extension.take().context("orphan Type-2 packet")?,
                (header & 0x7ffffff) as usize,
            ),
            _ => bail!("unsupported configuration packet type"),
        };
        if count == 0 {
            ensure!(header >> 29 == 1 && address == 2, "unsupported empty write");
            extension = Some(address);
            continue;
        }
        let size = count
            .checked_mul(4)
            .context("configuration count overflow")?;
        let end = at
            .checked_add(size)
            .context("configuration offset overflow")?;
        let data = bytes
            .get(at..end)
            .context("truncated configuration packet")?;
        if address == 2 {
            ensure!(
                state.frame_data.is_none() && state.command == 1 && state.crc_reset,
                "unsupported FDRI sequence"
            );
            ensure!(
                state.idcode == Some(expected_idcode),
                "FDRI before matching IDCODE"
            );
            ensure!(count % 101 == 0, "partial Series-7 frame payload");
            state.frame_data = Some(FrameData {
                byte_offset: at,
                words: count,
                initial_far: state.far.context("FDRI before FAR")?,
                sha256: crate::prepare::hash_bytes(data),
            });
            state.frame_crc_checked = false;
            for chunk in data.chunks_exact(4) {
                state.crc = crc_word(state.crc, address, u32::from_be_bytes(chunk.try_into()?));
            }
        } else {
            ensure!(count == 1, "unsupported multiword register write");
            let value = word(at);
            state
                .register(address, value, at, expected_idcode)
                .with_context(|| format!("configuration packet at byte {header_at}"))?;
            state.writes.push(RegisterWrite {
                byte_offset: at,
                address,
                value,
            });
        }
        at = end;
    }
    ensure!(extension.is_none(), "missing Type-2 extension");
    ensure!(state.desync, "missing DESYNC command");
    Ok(Inspection {
        sync_byte_offset: sync,
        idcode: state.idcode.context("missing IDCODE")?,
        packet_count: packets,
        nop_count: nops,
        zero_padding_words: padding,
        register_writes: state.writes,
        crc_checks: state.checks,
        frame_data: state.frame_data.context("missing FDRI")?,
    })
}

#[derive(Default)]
struct State {
    idcode: Option<u32>,
    far: Option<u32>,
    command: u32,
    mask: u32,
    crc: u32,
    crc_reset: bool,
    frame_crc_checked: bool,
    last_crc_checked: bool,
    started: bool,
    desync: bool,
    frame_data: Option<FrameData>,
    writes: Vec<RegisterWrite>,
    checks: Vec<CrcCheck>,
}

impl State {
    fn register(&mut self, address: u8, value: u32, at: usize, expected: u32) -> Result<()> {
        match address {
            0 => {
                ensure!(self.crc_reset, "CRC check before explicit reset");
                ensure!(
                    value == self.crc,
                    "configuration CRC mismatch: expected {value:08x}, computed {:08x}",
                    self.crc
                );
                self.checks.push(CrcCheck {
                    byte_offset: at,
                    expected: value,
                    computed: self.crc,
                });
                self.crc = 0;
                self.frame_crc_checked |= self.frame_data.is_some();
                self.last_crc_checked = true;
                return Ok(());
            }
            1 => self.far = Some(value),
            4 => {
                ensure!(
                    matches!(value, 0 | 1 | 3 | 5 | 7 | 9 | 10 | 13),
                    "unsupported configuration command {value:#x}"
                );
                if value == 7 {
                    ensure!(
                        self.frame_data.is_none(),
                        "CRC reset after FDRI would discard evidence"
                    );
                    self.crc = 0;
                    self.crc_reset = true;
                    self.last_crc_checked = false;
                    self.command = value;
                    return Ok(());
                }
                if value == 5 {
                    ensure!(self.frame_crc_checked, "START before checked FDRI");
                    self.started = true;
                }
                if value == 13 {
                    ensure!(
                        self.started && self.frame_crc_checked && self.last_crc_checked,
                        "DESYNC before checked startup sequence"
                    );
                    self.desync = true;
                }
                self.command = value;
            }
            5 => ensure!(
                value & self.mask & (1 << 6) == 0,
                "encrypted configuration is unsupported"
            ),
            6 => self.mask = value,
            12 => {
                ensure!(value == expected, "configuration IDCODE mismatch");
                ensure!(self.idcode.is_none(), "repeated IDCODE is unsupported");
                self.idcode = Some(value);
            }
            // Nonzero CTL1 changes frame/CRC modes that this inspector does
            // not support. Do not infer their behavior from another design.
            24 => ensure!(value & self.mask == 0, "nonzero CTL1 is unsupported"),
            9 | 14 | 16 | 17 | 19 => (),
            _ => bail!("unsupported configuration register {address:#x}"),
        }
        self.last_crc_checked = false;
        self.crc = crc_word(self.crc, address, value);
        Ok(())
    }
}

// ICAP consumes data bits0..31, then register-address bits0..4. There is no
// conventional CRC32C initial/final complement. Ordinary gzip CRC is distinct.
fn crc_word(mut crc: u32, address: u8, data: u32) -> u32 {
    let value = (u64::from(address) << 32) | u64::from(data);
    for bit in 0..37 {
        let feedback = (crc ^ (value >> bit) as u32) & 1;
        crc = (crc >> 1) ^ (0x82f63b78 & 0u32.wrapping_sub(feedback));
    }
    crc
}

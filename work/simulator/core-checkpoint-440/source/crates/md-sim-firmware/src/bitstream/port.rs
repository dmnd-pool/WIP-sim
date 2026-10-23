//! Bounded, exact-image configuration-port contract. This validates the words
//! sent by a guest. It does not reconstruct the logic implemented by them.
use super::{
    parse_header,
    series7::frames::{map_configuration, Part},
};
use anyhow::{ensure, Result};
use sha2::{Digest, Sha256};

pub struct Port {
    expected: Vec<u8>,
    sync: usize,
    start: Option<usize>,
    received: usize,
    armed: bool,
    failed: bool,
    hash: Sha256,
}

impl Port {
    /// The caller pins the container and part database in the run inputs.
    /// Validate every configuration packet, frame, padding word and ECC before
    /// permitting this image to be a programming reference.
    pub fn new(container: &[u8], part: &Part) -> Result<Self> {
        ensure!(
            container.len() <= 64 << 20,
            "configuration container size limit"
        );
        let header = parse_header(container)?;
        let bytes = &container[header.configuration_offset..];
        let mapping = map_configuration(bytes, part)?;
        Ok(Self {
            expected: bytes.to_vec(),
            sync: mapping.inspection.sync_byte_offset,
            start: None,
            received: 0,
            armed: false,
            failed: false,
            hash: Sha256::new(),
        })
    }

    pub fn reset(&mut self) {
        self.start = None;
        self.received = 0;
        self.armed = false;
        self.failed = false;
        self.hash = Sha256::new();
    }

    pub fn begin(&mut self) {
        self.armed = true;
    }

    /// Latch a rejected bus operation without erasing the words already seen.
    /// Corrected control registers or enable alone cannot clear this state.
    pub fn reject(&mut self) {
        self.failed = true;
    }

    pub fn received(&self) -> usize {
        self.received
    }

    pub fn omitted_preamble(&self) -> usize {
        self.start.unwrap_or(0)
    }

    pub fn complete(&self) -> bool {
        self.armed
            && !self.failed
            && self
                .start
                .is_some_and(|start| start + self.received == self.expected.len())
    }

    /// Digest of actual received words, in canonical big-endian order.
    pub fn received_sha256(&self) -> String {
        hex::encode(self.hash.clone().finalize())
    }

    /// Zynq DMA memory holds little-endian configuration words. Accept the
    /// complete payload or the exact suffix beginning at sync, as used by
    /// the archived U-Boot and Linux drivers. No other prefix is discarded.
    /// A failed transaction latches failure until programming reset.
    pub fn write_dma(&mut self, bytes: &[u8]) -> Result<()> {
        let result = self.write_checked(bytes);
        if result.is_err() {
            self.reject();
        }
        result
    }

    fn write_checked(&mut self, bytes: &[u8]) -> Result<()> {
        ensure!(
            self.armed && !self.failed,
            "configuration port is not ready"
        );
        ensure!(!self.complete(), "configuration data after completion");
        ensure!(
            !bytes.is_empty() && bytes.len() % 4 == 0,
            "configuration DMA word alignment"
        );
        let first = u32::from_le_bytes(bytes[..4].try_into()?);
        let start = *self
            .start
            .get_or_insert(if first == 0xaa995566 { self.sync } else { 0 });
        let offset = start + self.received;
        ensure!(
            bytes.len() <= self.expected.len() - offset,
            "configuration DMA exceeds image"
        );
        // Validate the whole DMA chunk before committing any of it.
        for (index, word) in bytes.chunks_exact(4).enumerate() {
            let canonical = u32::from_le_bytes(word.try_into()?).to_be_bytes();
            let at = offset + index * 4;
            ensure!(
                self.expected[at..at + 4] == canonical,
                "configuration DMA differs at payload byte {at}"
            );
        }
        for word in bytes.chunks_exact(4) {
            self.hash
                .update(u32::from_le_bytes(word.try_into()?).to_be_bytes());
        }
        self.received += bytes.len();
        Ok(())
    }
}

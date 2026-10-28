//! Incremental reader for the probe's run-owned QEMU stderr file.
//! Complete byte lines go through the caller's Rust stderr lock. A partial
//! line is retained privately and never spliced into an ASIC JSON record.
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read, Write},
};

pub const LIMIT: u64 = 16 * 1024 * 1024;
pub struct Tail {
    file: File,
    seen: u64,
    emitted: u64,
    lines: u64,
    pending: Vec<u8>,
    limit: u64,
    digest: Sha256,
}
impl Tail {
    pub fn new(file: File) -> Self {
        Self::with_limit(file, LIMIT)
    }
    fn with_limit(file: File, limit: u64) -> Self {
        Self {
            file,
            seen: 0,
            emitted: 0,
            lines: 0,
            pending: Vec::new(),
            limit,
            digest: Sha256::new(),
        }
    }
    pub fn drain(&mut self, output: &mut impl Write) -> io::Result<()> {
        let size = self.file.metadata()?.len();
        if size < self.seen {
            return Err(io::Error::other("owned QEMU stderr was truncated"));
        }
        if size > self.limit {
            return Err(io::Error::other("QEMU stderr exceeds diagnostic budget"));
        }
        let mut buffer = [0u8; 4096];
        loop {
            let count = self.file.read(&mut buffer)?;
            if count == 0 {
                return Ok(());
            }
            self.seen = self
                .seen
                .checked_add(count as u64)
                .ok_or_else(|| io::Error::other("QEMU stderr byte count overflow"))?;
            if self.seen > self.limit {
                return Err(io::Error::other("QEMU stderr exceeds diagnostic budget"));
            }
            self.digest.update(&buffer[..count]);
            let mut start = 0;
            for (index, byte) in buffer[..count].iter().enumerate() {
                if *byte == b'\n' {
                    self.pending.extend_from_slice(&buffer[start..=index]);
                    output.write_all(&self.pending)?;
                    self.emitted += self.pending.len() as u64;
                    self.lines += 1;
                    self.pending.clear();
                    start = index + 1;
                }
            }
            self.pending.extend_from_slice(&buffer[start..count]);
        }
    }
    pub fn summary(&self) -> serde_json::Value {
        serde_json::json!({"bytes_seen":self.seen,"bytes_emitted":self.emitted,
            "complete_lines":self.lines,"read_sha256":hex::encode(self.digest.clone().finalize()),"partial_line_bytes":self.pending.len(),
            "partial_line_hex":hex::encode(&self.pending),"line_boundary_complete":self.pending.is_empty()})
    }
    pub fn verified_summary(&self, expected: &[u8]) -> io::Result<serde_json::Value> {
        self.require_complete(expected)?;
        let mut receipt = self.summary();
        receipt["complete"] = serde_json::Value::Bool(true);
        Ok(receipt)
    }
    fn require_complete(&self, expected: &[u8]) -> io::Result<()> {
        let expected_bytes = expected.len() as u64;
        if !self.pending.is_empty() {
            return Err(io::Error::other("QEMU stderr ended with a partial line"));
        }
        if self.seen != expected_bytes || self.emitted != expected_bytes {
            return Err(io::Error::other(
                "QEMU stderr live export byte count differs",
            ));
        }
        if self.digest.clone().finalize() != Sha256::digest(expected) {
            return Err(io::Error::other("QEMU stderr live export content changed"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, SeekFrom};
    fn fixture(limit: u64) -> (tempfile::NamedTempFile, Tail) {
        let file = tempfile::NamedTempFile::new().unwrap();
        let reader = File::open(file.path()).unwrap();
        (file, Tail::with_limit(reader, limit))
    }
    #[test]
    fn fragments_emit_only_complete_lines_and_final_bytes_exactly_once() {
        let (mut file, mut tail) = fixture(100);
        let mut output = Vec::new();
        file.write_all(b"first par").unwrap();
        tail.drain(&mut output).unwrap();
        assert!(output.is_empty());
        file.write_all(b"t\nsecond\nthird").unwrap();
        tail.drain(&mut output).unwrap();
        assert_eq!(output, b"first part\nsecond\n");
        assert!(tail.require_complete(b"first part\nsecond\nthird").is_err());
        assert_eq!(tail.summary()["partial_line_hex"], "7468697264");
        file.write_all(b"\n").unwrap();
        tail.drain(&mut output).unwrap();
        tail.drain(&mut output).unwrap();
        assert_eq!(output, b"first part\nsecond\nthird\n");
        tail.require_complete(&output).unwrap();
        assert_eq!(tail.summary()["complete_lines"], 3);
        let mut replaced = output.clone();
        replaced[0] ^= 1;
        assert!(tail.verified_summary(&replaced).is_err());
        assert!(tail.verified_summary(&output[..output.len() - 1]).is_err());
        assert!(tail.summary().get("complete").is_none());
        assert_eq!(tail.verified_summary(&output).unwrap()["complete"], true);
    }
    #[test]
    fn truncated_file_and_input_bounds_fail_without_a_fabricated_newline() {
        let (mut file, mut tail) = fixture(10);
        let mut output = Vec::new();
        file.write_all(b"abc\npartial").unwrap();
        assert!(tail.drain(&mut output).is_err());
        assert!(output.is_empty());
        file.as_file_mut().set_len(0).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(b"abc\n").unwrap();
        tail.drain(&mut output).unwrap();
        assert_eq!(output, b"abc\n");
        file.as_file_mut().set_len(2).unwrap();
        assert!(tail.drain(&mut output).is_err());
        assert_eq!(output, b"abc\n");
    }
    #[test]
    fn non_utf8_and_buffer_split_lines_remain_exact_bytes() {
        let (mut file, mut tail) = fixture(9000);
        let mut bytes = vec![0xff; 8190];
        bytes.extend_from_slice(b"\n\0\n");
        file.write_all(&bytes).unwrap();
        let mut output = Vec::new();
        tail.drain(&mut output).unwrap();
        assert_eq!(output, bytes);
        tail.require_complete(&bytes).unwrap();
    }
}

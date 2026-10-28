//! Stop the diagnostic at the first unsupported-interface message.
//! The full raw stream remains in the run-owned trace file.
use anyhow::{ensure, Result};
use serde_json::{json, Value};
use std::{
    fs::File,
    io::Read,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

// Exact initialization messages from the pinned QEMU build. The final trace
// is emitted by vm_prepare_start before vm_start resumes the vCPUs. It is a
// phase boundary; an in_asm line is not used as an execution boundary.
const STARTUP: [&[u8]; 7] = [
    b"uart_event: uart is unclocked or in reset",
    b"SDHCI: unknown CAPAB mask: 0x0000000068040000",
    b"SDHCI: unknown CAPAB mask: 0x0000000068040000",
    b"qemu: warning: nic cadence_gem.0 has no peer",
    b"qemu: warning: nic cadence_gem.1 has no peer",
    b"md_sim_zynq_rom_map ram_hi=0x0 private_swap=unsupported",
    b"vm_state_notify running 1 reason 9 (running)",
];

pub fn startup_evidence(bytes: &[u8]) -> Value {
    let mut offset = 0;
    let mut records = Vec::new();
    for (record, expected) in bytes.split_inclusive(|&b| b == b'\n').zip(STARTUP) {
        let Some(actual) = record.strip_suffix(b"\n") else {
            break;
        };
        if actual != expected {
            break;
        }
        records.push(json!({"byte_offset":offset,"line_hex":hex::encode(actual),
            "line_sha256":md_sim_firmware::prepare::hash_bytes(actual),"bytes":actual.len()}));
        offset += actual.len() + 1;
    }
    json!({"recognized_prefix_lines":records.len(),"before_vcpu_resume_boundary_complete":records.len()==STARTUP.len(),
        "records":records,"scope":"Exact model initialization messages before vm_prepare_start resumes virtual CPUs. SDHCI capability mask remains a model limitation; absent GEM peers are the declared isolated topology. No guest MMIO diagnostic is accepted by this prefix."})
}

fn metadata_line(line: &[u8]) -> bool {
    if line.is_empty() || line == b"----------------" || line.starts_with(b"IN:") {
        return true;
    }
    let prefix = b"md_sim_zynq_rom_map ram_hi=0x";
    if let Some(rest) = line.strip_prefix(prefix) {
        return rest.len() == 26
            && rest[0].is_ascii_hexdigit()
            && &rest[1..] == b" private_swap=unsupported";
    }
    false
}

// Pinned disas/objdump.c dumps memory-order bytes, without word-endian
// conversion, in chunks of at most32 bytes. This selected public ROM uses
// aligned ARM32 code. Accept only its declared low-ROM/high-handoff trace
// windows; another code mode/window requires a separate reviewed contract.
#[derive(Default)]
struct TraceLines {
    raw: Option<RawBlock>,
}
struct RawBlock {
    next: u64,
    first: bool,
}
fn raw_span(address: u64, size: usize) -> bool {
    address % 4 == 0
        && size > 0
        && size % 4 == 0
        && address.checked_add(size as u64).is_some_and(|end| {
            (address < 0x20000 && end <= 0x20000) || (address >= 0xffff0000 && end <= 0x100000000)
        })
}
impl TraceLines {
    fn accept(&mut self, line: &[u8]) -> bool {
        if let Some(hex) = line.strip_prefix(b"OBJD-T: ") {
            let Some(raw) = &self.raw else {
                return false;
            };
            if hex.is_empty()
                || hex.len() > 64
                || hex.len() % 8 != 0
                || !hex
                    .iter()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
                || !raw_span(raw.next, hex.len() / 2)
            {
                return false;
            }
            let next = raw.next + (hex.len() / 2) as u64;
            self.raw = (hex.len() == 64).then_some(RawBlock { next, first: false });
            return true;
        }
        if self.raw.as_ref().is_some_and(|raw| raw.first) {
            return false;
        }
        self.raw = None;
        if let Some(rest) = line.strip_prefix(b"0x") {
            let Some(end) = rest.iter().position(|&b| b == b':') else {
                return false;
            };
            if ![8, 16].contains(&end)
                || !rest[..end].iter().all(u8::is_ascii_hexdigit)
                || !rest[end + 1..].iter().all(u8::is_ascii_whitespace)
            {
                return false;
            }
            let address = std::str::from_utf8(&rest[..end])
                .ok()
                .and_then(|s| u64::from_str_radix(s, 16).ok());
            let Some(next) = address.filter(|&at| raw_span(at, 4)) else {
                return false;
            };
            self.raw = Some(RawBlock { next, first: true });
            return true;
        }
        metadata_line(line)
    }
}

pub fn watch(
    mut file: File,
    cancelled: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
) -> Result<Option<Value>> {
    ensure!(
        file.metadata()?.is_file(),
        "ROM trace monitor requires a regular file"
    );
    let start = Instant::now();
    let mut bytes = [0u8; 8192];
    let mut pending = Vec::new();
    let mut offset = 0u64;
    let mut startup_at = 0;
    let mut records = TraceLines::default();
    loop {
        let n = file.read(&mut bytes)?;
        if n == 0 {
            if done.load(Ordering::Acquire) {
                if pending.is_empty() {
                    if startup_at != STARTUP.len() {
                        return reject(&pending, offset, "startup boundary absent", &cancelled);
                    }
                    if records.raw.as_ref().is_some_and(|raw| raw.first) {
                        return reject(
                            &pending,
                            offset,
                            "target raw bytes absent after address",
                            &cancelled,
                        );
                    }
                    return Ok(None);
                }
                return reject(&pending, offset, "unterminated diagnostic line", &cancelled);
            }
            if start.elapsed() > Duration::from_secs(15) {
                return reject(&pending, offset, "trace observer deadline", &cancelled);
            }
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        for &byte in &bytes[..n] {
            if byte == b'\n' {
                let accepted = if startup_at < STARTUP.len() {
                    if pending == STARTUP[startup_at] {
                        startup_at += 1;
                        true
                    } else {
                        false
                    }
                } else {
                    records.accept(&pending)
                };
                if !accepted {
                    return reject(
                        &pending,
                        offset,
                        "first unexpected emulator diagnostic",
                        &cancelled,
                    );
                }
                offset += pending.len() as u64 + 1;
                pending.clear();
                if offset > 8 << 20 {
                    return reject(&pending, offset, "trace bound", &cancelled);
                }
            } else {
                pending.push(byte);
                if pending.len() > 16384 {
                    return reject(&pending, offset, "diagnostic line bound", &cancelled);
                }
            }
        }
    }
}

fn reject(line: &[u8], offset: u64, reason: &str, cancelled: &AtomicBool) -> Result<Option<Value>> {
    cancelled.store(true, Ordering::Release);
    Ok(Some(
        json!({"byte_offset":offset,"reason":reason,"line_hex":hex::encode(line),
                   "line_sha256":md_sim_firmware::prepare::hash_bytes(line),"bytes":line.len()}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn prefix() -> Vec<u8> {
        let mut bytes = Vec::new();
        for line in STARTUP {
            bytes.extend_from_slice(line);
            bytes.push(b'\n');
        }
        bytes
    }
    #[test]
    fn unsupported_register_messages_are_not_instruction_records() {
        for line in [
            b"".as_slice(),
            b"----------------",
            b"IN: ",
            b"md_sim_zynq_rom_map ram_hi=0x8 private_swap=unsupported",
        ] {
            assert!(metadata_line(line), "{}", String::from_utf8_lossy(line));
        }
        for line in [
            b"zynq.efuse: unimplemented device read".as_slice(),
            b"zynq_slcr: Invalid read access",
            b"qemu: unsupported ROM OCM configuration0x18",
            b"0xno: invalid",
            b"0x00000000: unable to read memory",
            b"md_sim_zynq_rom_map ram_hi=0x10 private_swap=unsupported",
        ] {
            assert!(!metadata_line(line));
        }
    }
    #[test]
    fn first_error_is_retained_with_its_exact_raw_offset() {
        use std::io::{Seek, SeekFrom, Write};
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(&prefix()).unwrap();
        file.write_all(b"IN: \n0x00000000:  \nOBJD-T: 0f0000ea\nunknown register\nsecond error\n")
            .unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let done = Arc::new(AtomicBool::new(true));
        let event = watch(file, cancel.clone(), done).unwrap().unwrap();
        assert!(cancel.load(Ordering::Acquire));
        assert_eq!(
            event["byte_offset"],
            prefix().len() + b"IN: \n0x00000000:  \nOBJD-T: 0f0000ea\n".len()
        );
        assert_eq!(event["line_hex"], hex::encode(b"unknown register"));
    }

    #[test]
    fn complete_instructions_pass_but_partial_and_long_lines_cancel() {
        use std::io::{Seek, SeekFrom, Write};
        for (bytes, rejected) in [
            (b"IN: \n0x00000000:  \nOBJD-T: 0f0000ea\n".to_vec(), false),
            (
                b"md_sim_zynq_rom_map ram_hi=0x8 private_swap=unsupported\n".to_vec(),
                false,
            ),
            (b"IN: \npartial diagnostic".to_vec(), true),
            (vec![b'x'; 16385], true),
        ] {
            let mut file = tempfile::tempfile().unwrap();
            file.write_all(&prefix()).unwrap();
            file.write_all(&bytes).unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();
            let cancel = Arc::new(AtomicBool::new(false));
            let event = watch(file, cancel.clone(), Arc::new(AtomicBool::new(true))).unwrap();
            assert_eq!(event.is_some(), rejected);
            assert_eq!(cancel.load(Ordering::Acquire), rejected);
        }
    }
    #[test]
    fn startup_messages_require_exact_order_count_and_phase() {
        use std::io::{Seek, SeekFrom, Write};
        let good = prefix();
        assert_eq!(startup_evidence(&good)["recognized_prefix_lines"], 7);
        let truncated = good[..good.len() - 1].to_vec();
        assert_eq!(startup_evidence(&truncated)["recognized_prefix_lines"], 6);
        assert_eq!(
            startup_evidence(&truncated)["before_vcpu_resume_boundary_complete"],
            false
        );
        let mut cases = vec![Vec::new(), b"IN: \n".to_vec(), truncated];
        for index in 0..STARTUP.len() {
            let mut missing = Vec::new();
            let mut duplicate = Vec::new();
            for (n, line) in STARTUP.iter().enumerate() {
                if n != index {
                    missing.extend_from_slice(line);
                    missing.push(b'\n');
                }
                duplicate.extend_from_slice(line);
                duplicate.push(b'\n');
                if n == index {
                    duplicate.extend_from_slice(line);
                    duplicate.push(b'\n');
                }
            }
            cases.push(missing);
            cases.push(duplicate);
        }
        let mut changed = good.clone();
        changed[0] = b'X';
        cases.push(changed);
        for line in STARTUP[..5].iter().chain([&STARTUP[6]]) {
            let mut late = good.clone();
            late.extend_from_slice(b"IN: \n0x00000000:  \nOBJD-T: 0f0000ea\n");
            late.extend_from_slice(line);
            late.push(b'\n');
            cases.push(late);
        }
        let mut guest = good;
        guest.extend_from_slice(b"uart_read: uart is unclocked or in reset\n");
        cases.push(guest);
        for bytes in cases {
            let mut file = tempfile::tempfile().unwrap();
            file.write_all(&bytes).unwrap();
            file.seek(SeekFrom::Start(0)).unwrap();
            let cancel = Arc::new(AtomicBool::new(false));
            assert!(watch(file, cancel.clone(), Arc::new(AtomicBool::new(true)))
                .unwrap()
                .is_some());
            assert!(cancel.load(Ordering::Acquire));
        }
    }
    #[test]
    fn raw_target_dump_is_bounded_and_requires_its_source_defined_frame() {
        let mut records = TraceLines::default();
        assert!(records.accept(b"0x00000000:  "));
        // Original reset branch bytes from pinned public ROM offset0.
        assert!(records.accept(b"OBJD-T: 0f0000ea"));
        assert!(!records.accept(b"OBJD-T: 220000eb"));
        assert!(records.accept(b"0x00000044:  "));
        assert!(records
            .accept(b"OBJD-T: 0000000000000000000000000000000000000000000000000000000000000000"));
        assert!(records.accept(b"OBJD-T: 220000eb"));
        assert!(records.accept(b""));
        for bad in [
            b"OBJD-T: ".as_slice(),
            b"OBJD-T: 0",
            b"OBJD-T: 0G",
            b"OBJD-T: unable to read memory",
            b"OBJD-H: 0f0000ea",
            b"OBJD-T: 000000000000000000000000000000000000000000000000000000000000000000",
        ] {
            let mut r = TraceLines::default();
            assert!(r.accept(b"0x00000000:  "));
            assert!(!r.accept(bad));
        }
        for bad in [
            b"0x00000001:  ".as_slice(),
            b"0x00020000:  ",
            b"0xffff0002:  ",
            b"0x0000000100000000:  ",
            b"0x00000000: unable to read memory",
            b"0x00000000: ea00000f b #0x44",
        ] {
            assert!(!TraceLines::default().accept(bad));
        }
        let mut edge = TraceLines::default();
        assert!(edge.accept(b"0x0001fffc:  "));
        assert!(!edge.accept(b"OBJD-T: 0000000000000000"));
        let mut high = TraceLines::default();
        assert!(high.accept(b"0xfffffffc:  "));
        assert!(high.accept(b"OBJD-T: 00000000"));
        let mut r = TraceLines::default();
        assert!(!r.accept(b"OBJD-T: 0f0000ea"));
        assert!(r.accept(b"0x00000000:  "));
        assert!(!r.accept(b""));
        assert!(!r.accept(b"uart_read: uart is unclocked or in reset"));
    }
    #[test]
    fn raw_target_dump_observer_preserves_next_real_error() {
        use std::io::{Seek, SeekFrom, Write};
        let data = b"0x00000000:  \nOBJD-T: 0f0000ea\n\n";
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(&prefix()).unwrap();
        file.write_all(data).unwrap();
        file.write_all(b"unsupported device read\n").unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        let event = watch(
            file,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicBool::new(true)),
        )
        .unwrap()
        .unwrap();
        assert_eq!(event["byte_offset"], prefix().len() + data.len());
        assert_eq!(event["line_hex"], hex::encode(b"unsupported device read"));
    }
}

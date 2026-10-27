//! Controlled collector oracle. These expectations come from the pinned
//! disassembly in readback analysis442, not from a modeled ASIC response.
use anyhow::{ensure, Context, Result};
use md_sim_firmware::prepare::hash_bytes;
use serde_json::{json, Value};

pub const DRIVER: &[u8] =
    include_bytes!("../../../../simulator/firmware/fixtures/bm1397-readback-oracle.c");
pub const LINK_SCRIPT: &[u8] =
    include_bytes!("../../../../simulator/firmware/fixtures/bm1397-readback-oracle.ld");
const PACKING_DRIVER_SHA: &str = "c1c2a3fd7fd693797fe54eb87faaf4d7ad560710ea35f58fbc147fec12209b71";
pub const WORDS: usize = 4 + 8 * 24;

/// Reuse the existing ARM syscall ABI without changing the retained fixture.
/// The derived header is captured as a separate, hashed process input.
pub fn abi_header(packing_driver: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        hash_bytes(packing_driver) == PACKING_DRIVER_SHA,
        "packing ABI source differs"
    );
    let source = std::str::from_utf8(packing_driver)?;
    let start = source
        .find("__attribute__((noreturn)) static void finish")
        .context("ABI start absent")?;
    let end = source
        .find("__attribute__((noreturn)) void _start")
        .context("ABI end absent")?;
    ensure!(start < end, "ABI source order differs");
    Ok(format!("typedef unsigned int u32;\n{}", &source[start..end]).into_bytes())
}

#[derive(Clone, Copy)]
pub struct LoadSegment {
    pub address: u32,
    pub offset: u32,
    pub file_bytes: u32,
    pub memory_bytes: u32,
    pub flags: u32,
}

pub(super) fn exact_copy(
    executable: &[u8],
    bytes: &[u8],
    address: u32,
    segments: &[LoadSegment],
) -> Result<usize> {
    ensure!(!bytes.is_empty(), "empty original function");
    let copies: Vec<_> = executable
        .windows(bytes.len())
        .enumerate()
        .filter_map(|(at, value)| (value == bytes).then_some(at))
        .collect();
    ensure!(copies.len() == 1, "original function absent or duplicated");
    let at = u32::try_from(copies[0])?;
    let end = at
        .checked_add(u32::try_from(bytes.len())?)
        .context("function file bound")?;
    let virtual_end = address
        .checked_add(u32::try_from(bytes.len())?)
        .context("function address bound")?;
    let overlapping: Vec<_> = segments
        .iter()
        .filter(|s| s.address < virtual_end && s.address.saturating_add(s.memory_bytes) > address)
        .collect();
    ensure!(
        overlapping.len() == 1,
        "function has ambiguous virtual mapping"
    );
    let s = overlapping[0];
    ensure!(
        s.flags == 5
            && at >= s.offset
            && end
                <= s.offset
                    .checked_add(s.file_bytes)
                    .context("load file bound")?
            && s.address.checked_add(at - s.offset) == Some(address),
        "function execution mapping differs"
    );
    Ok(at as usize)
}

pub fn verify_copies(
    executable: &[u8],
    original: &[u8],
    segments: &[LoadSegment],
) -> Result<Vec<Value>> {
    let mut copies = Vec::new();
    for (name, offset, size, address, sha) in [
        (
            "collector",
            82116,
            916,
            0x1c0c4,
            "a6fa7c53118e2e3ca140a56f086044085e57d750b7e6a98873c024409abd3445",
        ),
        (
            "unsigned_division",
            449512,
            168,
            0x75be8,
            "81c5ee0515ea5584984332c432a85952fa793bc7be049e82b188da86226d613f",
        ),
    ] {
        let bytes = original
            .get(offset..offset + size)
            .context("original range absent")?;
        ensure!(hash_bytes(bytes) == sha, "original {name} range differs");
        let at = exact_copy(executable, bytes, address, segments)?;
        copies.push(
            json!({"operation":name,"source_file_offset":offset,"original_virtual_address":address,
            "oracle_virtual_address":address,"oracle_file_offset":at,"size":size,"sha256":sha}),
        );
    }
    Ok(copies)
}

const NAMES: [&str; 8] = [
    "exact_match",
    "wrong_register",
    "same_register_wrong_chain",
    "same_register_wrong_asic",
    "wrong_register_then_match",
    "wrong_chain_then_match",
    "empty",
    "read_index_wrap",
];

// Queue inputs and outputs predicted before this wrapper existed. The software
// FOUND flag can become1 without a copied word; keep those columns separate.
fn expected(id: usize) -> [u32; 24] {
    let (head, count, first_value, first_meta, second_value, second_meta) = match id {
        0 => (0, 1, 0x01230074, 0x02400ca5, 0, 0),
        1 => (0, 1, 0x01230074, 0x02410ca5, 0, 0),
        2 => (0, 1, 0x01230074, 0x03400ca5, 0, 0),
        3 => (0, 1, 0x01230074, 0x024010a5, 0, 0),
        4 => (0, 2, 0x11111111, 0x02410ca5, 0x01230074, 0x02400ca5),
        5 => (0, 2, 0x11111111, 0x03400ca5, 0x01230074, 0x02400ca5),
        6 => (0, 0, 0, 0, 0, 0),
        7 => (510, 1, 0x01230074, 0x02400ca5, 0, 0),
        _ => unreachable!(),
    };
    let (value, found, next, remaining, calls, waits) = match id {
        0 => (0x01230074, 1, 1, 0, 1, 0),
        1 => (0xdeadbeef, 0, 1, 0, 10, 10),
        2 | 3 => (0xdeadbeef, 1, 1, 0, 1, 0),
        4 => (0x01230074, 1, 2, 0, 1, 0),
        5 => (0xdeadbeef, 1, 1, 1, 1, 0),
        6 => (0xdeadbeef, 0, 0, 0, 10, 10),
        7 => (0x01230074, 1, 0, 0, 1, 0),
        _ => unreachable!(),
    };
    [
        id as u32,
        head,
        count,
        2,
        3,
        0x40,
        first_value,
        first_meta,
        second_value,
        second_meta,
        0xdeadbeef,
        0,
        value,
        found,
        next,
        remaining,
        calls,
        calls,
        calls,
        waits,
        0xa5a5a5,
        0x13579bdf,
        0x2468ace0,
        0x11223344,
    ]
}

pub fn verify_output(bytes: &[u8]) -> Result<Vec<Value>> {
    ensure!(bytes.len() == WORDS * 4, "readback output length differs");
    let words: Vec<_> = bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    ensure!(
        words[..4] == [0x52424331, 1, 8, 24],
        "readback output header differs"
    );
    let mut cases = Vec::new();
    for (id, name) in NAMES.iter().enumerate() {
        let row = &words[4 + id * 24..4 + (id + 1) * 24];
        for (field, (observed, expected)) in row.iter().zip(expected(id)).enumerate() {
            ensure!(*observed == expected,
                "readback {name} field{field} differs: observed {observed:#x}, expected {expected:#x}");
        }
        cases.push(
            json!({"case":name,"words":row,"returned_word":row[12],"driver_found":row[13],
            "copied_matching_word":matches!(id, 0 | 4 | 7),"remaining_queue_entries":row[15]}),
        );
    }
    Ok(cases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_abi_source_is_exact_and_reused() {
        let original =
            include_bytes!("../../../../simulator/firmware/fixtures/bm1397-core-oracle.c");
        let header = abi_header(original).unwrap();
        assert!(header.windows(6).any(|w| w == b"svc #0"));
        assert!(!header.windows(6).any(|w| w == b"_start"));
        let mut changed = original.to_vec();
        changed[0] ^= 1;
        assert!(abi_header(&changed).is_err());
    }

    #[test]
    fn verifier_rejects_each_changed_field_and_length() {
        // Validator fixture only. The isolated vendor execution is separate.
        let bytes: Vec<_> = [0x52424331u32, 1, 8, 24]
            .into_iter()
            .chain((0..8).flat_map(expected))
            .flat_map(u32::to_le_bytes)
            .collect();
        let report = verify_output(&bytes).unwrap();
        assert_eq!(report[2]["driver_found"], 1);
        assert_eq!(report[2]["returned_word"], 0xdeadbeefu32);
        assert_eq!(report[2]["copied_matching_word"], false);
        for i in 0..WORDS {
            let mut changed = bytes.clone();
            changed[i * 4] ^= 1;
            assert!(verify_output(&changed).is_err(), "word{i}");
        }
        assert!(verify_output(&bytes[..bytes.len() - 1]).is_err());
        let mut extra = bytes;
        extra.push(0);
        assert!(verify_output(&extra).is_err());
    }

    #[test]
    fn function_requires_exact_unique_executable_mapping() {
        let executable = [0, 0, 3, 5, 7, 11, 0, 0];
        let bytes = &executable[2..6];
        let s = LoadSegment {
            address: 0x8000,
            offset: 0,
            file_bytes: 8,
            memory_bytes: 8,
            flags: 5,
        };
        assert_eq!(exact_copy(&executable, bytes, 0x8002, &[s]).unwrap(), 2);
        assert!(exact_copy(&executable, bytes, 0x8004, &[s]).is_err());
        assert!(exact_copy(&executable, bytes, 0x8002, &[s, s]).is_err());
        assert!(exact_copy(&executable, bytes, 0x8002, &[LoadSegment { flags: 7, ..s }]).is_err());
        assert!(exact_copy(
            &executable,
            bytes,
            0x8002,
            &[LoadSegment { file_bytes: 4, ..s }]
        )
        .is_err());
        assert!(exact_copy(&[executable, executable].concat(), bytes, 0x8002, &[s]).is_err());
        assert!(exact_copy(&executable, &[9, 9, 9], 0x8002, &[s]).is_err());
    }
}

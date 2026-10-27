//! Execute the vendor formatter against caller-owned raw readback words.
//! Expectations come from the retained analysis442 before this wrapper.
//! Software FOUND is kept separate from ASIC data and response provenance.
use super::readback::{exact_copy, LoadSegment};
use anyhow::{ensure, Context, Result};
use md_sim_firmware::bm1397::{core_control::Control, core_readback::Core0Readback, Reply};
use md_sim_firmware::prepare::hash_bytes;
use serde_json::{json, Value};

pub const DRIVER: &[u8] =
    include_bytes!("../../../../simulator/firmware/fixtures/bm1397-core0-oracle.c");
pub const LINK_SCRIPT: &[u8] =
    include_bytes!("../../../../simulator/firmware/fixtures/bm1397-core0-oracle.ld");
pub const CASE_COUNT: usize = 5192;

pub fn verify_copies(
    executable: &[u8],
    original: &[u8],
    segments: &[LoadSegment],
) -> Result<Vec<Value>> {
    let mut copies = Vec::new();
    for (name, address, size, sha) in [
        (
            "formatter",
            0x238f0,
            1024,
            "8d3b1b4e80ef91084c23018fc6055b6bd5249b0e0f91119919f2fb20ef0f8245",
        ),
        (
            "open_mode",
            0x76be0,
            3,
            "bcccb27e349c4055db84a303748952d67745230b6628f186cbf76702a4adc724",
        ),
        (
            "software_status_labels",
            0x7980c,
            12,
            "8588d6d284cad84b69575a2f98562d5b60e887de2845cc7bc3ed6e27954362eb",
        ),
        (
            "field_format",
            0x79848,
            14,
            "1c2de057299f5a1d578f60a9ea43a6198632cf9801e010cec5f64a41b88bc070",
        ),
        (
            "core0_labels",
            0x7aad0,
            180,
            "9f9fbd3b3b32069f5180d91a813476bdc28bb371628418286f74f0f418f68c0a",
        ),
    ] {
        let offset = address as usize - 0x8000;
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

#[derive(Clone, Copy, Debug)]
struct Input {
    raw: u32,
    found: u32,
    debug: u32,
    secondary: u32,
    failures: u32,
}

fn inputs() -> Vec<Input> {
    let mut cases = Vec::with_capacity(CASE_COUNT);
    for value in 0..256 {
        for core in [0, 1, 0x1ff, 0x200, 0xffff] {
            for middle in [0, 0xff] {
                for found in 0..2 {
                    cases.push(Input {
                        raw: core << 16 | middle << 8 | value,
                        found,
                        debug: 4,
                        secondary: 0,
                        failures: 0,
                    });
                }
            }
        }
    }
    for raw in [
        0, 0x74, 0x01230074, 0xabcdff74, 0xffffffff, 0x10008, 0x10004, 0xdeadbeef,
    ] {
        cases.push(Input {
            raw,
            found: u32::from(raw != 0xdeadbeef),
            debug: 4,
            secondary: 0,
            failures: 0,
        });
    }
    for debug in [0, 3, 4, 5] {
        for secondary in 0..2 {
            for failures in 0..4 {
                for found in [0, 255] {
                    cases.push(Input {
                        raw: 0xabcdff74,
                        found,
                        debug,
                        secondary,
                        failures,
                    });
                }
            }
        }
    }
    cases
}

/// Predict the external calls, not callback implementations. The unchanged
/// vendor code supplies their values when the oracle actually executes.
fn expected_events(input: Input) -> Vec<[u32; 6]> {
    let mut events = Vec::new();
    if input.debug <= 3 {
        return events;
    }
    let mut write = |operation: Option<(u32, u32, u32, u32)>| {
        for path in 1..=1 + input.secondary {
            let stream = if input.failures & path == 0 {
                path * 0x100
            } else {
                0
            };
            events.push([
                1,
                if path == 1 { 0xc00000 } else { 0xc00020 },
                0x76be0,
                stream,
                0,
                0,
            ]);
            if stream != 0 {
                events.push(match operation {
                    Some((format, first, second, third)) => {
                        [2, stream, format, first, second, third]
                    }
                    None => [4, 10, stream, 0, 0, 0],
                });
            }
            events.push([3, stream, 0, 0, 0, 0]);
        }
    };
    write(Some((
        0x7aad0,
        input.raw >> 16,
        input.raw & 255,
        if input.found == 0 { 0x7980c } else { 0x79810 },
    )));
    if input.found != 0 {
        for (label, shift, mask) in [
            (0x7ab0c, 6, 3),
            (0x7ab20, 4, 3),
            (0x7ab34, 3, 1),
            (0x7ab4c, 2, 1),
            (0x7ab5c, 1, 1),
            (0x7ab70, 0, 1),
        ] {
            write(Some((0x79848, label, (input.raw >> shift) & mask, 0)));
        }
    }
    write(None);
    events
}

fn compare_view(raw: u32, observed: &[[u32; 6]]) -> Result<(bool, bool)> {
    // Use arguments supplied by the original instructions, independently of
    // the predicted callback builder. A fixed requested selector remains
    // separate from the returned core field; no physical mapping is inferred.
    let view = Core0Readback::from_reply(
        12,
        Control::Read {
            selector: 509,
            register: 0,
        },
        Reply::Register {
            value: raw,
            asic_address: 12,
            register: 0x40,
        },
    )
    .map_err(|e| anyhow::anyhow!("core0 view rejected oracle input: {e:?}"))?;
    let header = observed
        .iter()
        .find(|e| e[0] == 2 && e[1] == 0x100 && e[2] == 0x7aad0);
    if let Some(header) = header {
        ensure!(
            u32::from(view.reported_core) == header[3] && u32::from(view.data) == header[4],
            "core0 view differs from original formatter header"
        );
    }
    let fields: Vec<_> = observed
        .iter()
        .filter(|e| e[0] == 2 && e[1] == 0x100 && e[2] == 0x79848)
        .map(|e| e[4])
        .collect();
    if !fields.is_empty() {
        ensure!(
            fields
                == [
                    u32::from(view.ccdly_sel),
                    u32::from(view.pwth_sel),
                    u32::from(view.hash_clken_bit),
                    u32::from(view.mmen_bit),
                    u32::from(view.reserved_bit),
                    u32::from(view.swpf_mode_bit)
                ],
            "core0 view differs from original formatter fields"
        );
    }
    Ok((header.is_some(), !fields.is_empty()))
}

pub fn verify_output(bytes: &[u8]) -> Result<Value> {
    ensure!(
        bytes.len() <= 3 << 20 && bytes.len() % 4 == 0,
        "formatter output bound"
    );
    let mut words = bytes
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    for expected in [0x43304631, 1, CASE_COUNT as u32, 6] {
        ensure!(words.next() == Some(expected), "formatter header differs");
    }
    let mut callback_count = 0;
    let mut compared_headers = 0;
    let mut compared_fields = 0;
    for (id, input) in inputs().into_iter().enumerate() {
        let events = expected_events(input);
        for expected in [
            id as u32,
            input.raw,
            input.found,
            input.debug,
            input.secondary,
            input.failures,
            events.len() as u32,
            0x13579bdf,
            0x2468ace0,
            0xa5a5a5,
        ] {
            ensure!(
                words.next() == Some(expected),
                "formatter case{id} metadata differs"
            );
        }
        let mut observed = Vec::with_capacity(events.len());
        for (event_id, event) in events.iter().enumerate() {
            let mut actual = [0; 6];
            for (field, expected) in event.iter().enumerate() {
                actual[field] = words.next().context("formatter callback is incomplete")?;
                ensure!(
                    actual[field] == *expected,
                    "formatter case{id} event{event_id} field{field} differs"
                );
            }
            observed.push(actual);
        }
        let (header, fields) =
            compare_view(input.raw, &observed).with_context(|| format!("formatter case{id}"))?;
        compared_headers += usize::from(header);
        compared_fields += usize::from(fields);
        callback_count += events.len();
    }
    ensure!(words.next().is_none(), "extra formatter output");
    Ok(
        json!({"case_count":CASE_COUNT,"callback_count":callback_count,
        "model_header_comparisons":compared_headers,"model_field_comparisons":compared_fields,
        "exhaustive_inputs":5120,"explicit_vectors":8,"debug_secondary_and_open_failure_cases":64,
        "software_found_is_asic_status":false,
        "observed_null_fclose_is_vendor_callback_behavior":true,
        "scope":"Original formatter control flow and raw field extraction only. No core effects, physical timing or readiness."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn predicted_output() -> Vec<u8> {
        let mut words = vec![0x43304631, 1, CASE_COUNT as u32, 6];
        for (id, input) in inputs().into_iter().enumerate() {
            let events = expected_events(input);
            words.extend([
                id as u32,
                input.raw,
                input.found,
                input.debug,
                input.secondary,
                input.failures,
                events.len() as u32,
                0x13579bdf,
                0x2468ace0,
                0xa5a5a5,
            ]);
            words.extend(events.into_iter().flatten());
        }
        words.into_iter().flat_map(u32::to_le_bytes).collect()
    }

    #[test]
    fn parser_rejects_mutated_callbacks_metadata_and_incomplete_output() {
        let bytes = predicted_output();
        assert_eq!(verify_output(&bytes).unwrap()["case_count"], CASE_COUNT);
        // A constructed parser fixture is not vendor execution evidence.
        for offset in [0, 16, 20, 40, 72, 80, bytes.len() - 4] {
            let mut wrong = bytes.clone();
            wrong[offset] ^= 1;
            assert!(verify_output(&wrong).is_err(), "offset{offset}");
        }
        assert!(verify_output(&bytes[..bytes.len() - 1]).is_err());
        assert!(verify_output(&bytes[..bytes.len() - 4]).is_err());
        let mut trailing = bytes;
        trailing.extend([0; 4]);
        assert!(verify_output(&trailing).is_err());
    }

    #[test]
    fn written_control_byte_does_not_create_a_ready_flag() {
        let input = Input {
            raw: 0xabcdff74,
            found: 1,
            debug: 4,
            secondary: 0,
            failures: 0,
        };
        let events = expected_events(input);
        assert_eq!(events[1], [2, 0x100, 0x7aad0, 0xabcd, 0x74, 0x79810]);
        let fields: Vec<_> = events
            .iter()
            .filter(|e| e[0] == 2 && e[2] == 0x79848)
            .map(|e| e[4])
            .collect();
        assert_eq!(fields, [1, 3, 0, 1, 0, 0]);
        assert!(expected_events(Input { debug: 3, ..input }).is_empty());
        assert_eq!(expected_events(Input { found: 0, ..input }).len(), 6);
        assert_eq!(
            expected_events(Input {
                secondary: 1,
                ..input
            })
            .len(),
            48
        );
        assert!(expected_events(Input {
            failures: 1,
            ..input
        })
        .iter()
        .all(|e| e[0] != 2));
    }
}

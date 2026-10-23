//! Static contracts from one exact archived SPL. This does not execute ARM
//! instructions or infer physical latency from the firmware's waits.
use anyhow::{ensure, Context, Result};
use md_sim_firmware::prepare::hash_bytes;
use md_sim_runtime::store::ObjectId;
use serde::Serialize;
use serde_json::{json, Value};

pub const BOOT_SIZE: usize = 79_263;
pub const SPL_SIZE: usize = 77_023;
pub const SOURCE_OFFSET: usize = 2_240;
pub const BOOT_SHA: &str = "950d19c6e5bfd9d8107def130b19fc9a5f64d824b26c0c493d5fdfe8503edac4";
pub const SPL_SHA: &str = "765efb23844156e976ba785c24a6a753c664504a499c64581d1c356a3f4c02df";
const MAX_RECORDS: usize = 512;
const MAX_TABLE_BYTES: usize = 16 * 1024;
const POLL_READS: u32 = 100_000_001;
const TIMER_MULTIPLIER: u32 = 0x27bc86a4;
const TIMER_RECIPROCAL: u32 = 0x10624dd3;

pub fn object(bytes: &[u8]) -> ObjectId {
    ObjectId {
        sha256: hash_bytes(bytes),
        size: bytes.len() as u64,
    }
}

fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    let end = offset.checked_add(4).context("word offset overflow")?;
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..end)
            .context("truncated SPL word")?
            .try_into()?,
    ))
}

/// Matches MUL(low32), UMULL and LSR in the pinned instructions. It is not
/// an unbounded real-number duration formula or a measured timer frequency.
fn delay_ticks(argument: u32) -> (u32, u32) {
    let wrapped = TIMER_MULTIPLIER.wrapping_mul(argument);
    let ticks = (u64::from(wrapped) * u64::from(TIMER_RECIPROCAL)) >> 39;
    (wrapped, ticks as u32)
}

fn poll_ready(value: u32, mask: u32) -> bool {
    value & mask != 0
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum Operation {
    Stop,
    Clear {
        address: u32,
    },
    Write {
        address: u32,
        value: u32,
    },
    MaskedWrite {
        address: u32,
        mask: u32,
        value: u32,
    },
    PollAny {
        address: u32,
        mask: u32,
        maximum_reads: u32,
        timeout_return: u32,
    },
    TimerDelay {
        counter_address: u32,
        argument: u32,
        wrapped_product: u32,
        threshold_ticks: u32,
    },
}

#[derive(Debug, Serialize)]
pub struct Record {
    pub spl_offset: usize,
    pub boot_offset: usize,
    pub header: u32,
    pub arguments: Vec<u32>,
    pub effect: Operation,
}

#[derive(Debug, Serialize)]
pub struct Table {
    pub spl_offset: usize,
    pub end_exclusive: usize,
    pub encoded: ObjectId,
    pub records: Vec<Record>,
}

fn decode_table(bytes: &[u8], start: usize) -> Result<Table> {
    ensure!(start & 3 == 0, "unaligned table start");
    let mut offset = start;
    let mut records = Vec::new();
    for _ in 0..MAX_RECORDS {
        ensure!(
            offset - start < MAX_TABLE_BYTES,
            "table byte bound without terminator"
        );
        let header = word(bytes, offset)?;
        let opcode = header >> 4;
        let count = (header & 15) as usize;
        let expected = match opcode {
            0 => 0,
            1 => 1,
            2 | 4 | 5 => 2,
            3 => 3,
            _ => anyhow::bail!("unknown table operation {opcode} at {offset:#x}"),
        };
        ensure!(
            count == expected,
            "operation {opcode} argument count differs at {offset:#x}"
        );
        let end = offset
            .checked_add((count + 1) * 4)
            .context("table offset overflow")?;
        ensure!(end - start <= MAX_TABLE_BYTES, "table byte bound");
        let arguments = (0..count)
            .map(|n| word(bytes, offset + (n + 1) * 4))
            .collect::<Result<Vec<_>>>()?;
        let effect = match opcode {
            0 => Operation::Stop,
            1 => Operation::Clear {
                address: arguments[0],
            },
            2 => Operation::Write {
                address: arguments[0],
                value: arguments[1],
            },
            3 => Operation::MaskedWrite {
                address: arguments[0],
                mask: arguments[1],
                value: arguments[2],
            },
            4 => Operation::PollAny {
                address: arguments[0],
                mask: arguments[1],
                maximum_reads: POLL_READS,
                timeout_return: 2,
            },
            5 => {
                let (wrapped_product, threshold_ticks) = delay_ticks(arguments[1]);
                Operation::TimerDelay {
                    counter_address: arguments[0],
                    argument: arguments[1],
                    wrapped_product,
                    threshold_ticks,
                }
            }
            _ => unreachable!(),
        };
        records.push(Record {
            spl_offset: offset,
            boot_offset: SOURCE_OFFSET + offset,
            header,
            arguments,
            effect,
        });
        offset = end;
        if opcode == 0 {
            return Ok(Table {
                spl_offset: start,
                end_exclusive: end,
                encoded: object(&bytes[start..end]),
                records,
            });
        }
    }
    anyhow::bail!("table record bound without terminator")
}

fn code_range(
    spl: &[u8],
    name: &str,
    start: usize,
    length: usize,
    expected: &str,
) -> Result<Value> {
    let bytes = spl
        .get(start..start + length)
        .context("code range absent")?;
    let identity = object(bytes);
    ensure!(identity.sha256 == expected, "{name} code identity differs");
    Ok(
        json!({"name":name,"spl_offset":start,"boot_offset":SOURCE_OFFSET+start,
        "end_exclusive":start+length,"object":identity}),
    )
}

/// All exact identities are checked before interpreting data or assigning the
/// separately recovered instruction semantics to any record.
pub fn inspect(boot: &[u8], spl: &[u8]) -> Result<Value> {
    ensure!(
        boot.len() == BOOT_SIZE && hash_bytes(boot) == BOOT_SHA,
        "selected boot.bin differs"
    );
    ensure!(
        spl.len() == SPL_SIZE && hash_bytes(spl) == SPL_SHA,
        "selected SPL differs"
    );
    ensure!(
        boot.get(SOURCE_OFFSET..) == Some(spl),
        "SPL is not the exact parent suffix"
    );
    ensure!(
        word(boot, 0x20)? == 0xaa995566 && word(boot, 0x24)? == 0x584c4e58,
        "boot header magic differs"
    );
    let sum = (0x20..=0x48).step_by(4).try_fold(0u32, |sum, at| {
        Ok::<_, anyhow::Error>(sum.wrapping_add(word(boot, at)?))
    })?;
    ensure!(
        sum == u32::MAX
            && word(boot, 0x30)? as usize == SOURCE_OFFSET
            && word(boot, 0x38)? == 0
            && word(boot, 0x3c)? == 0,
        "boot header placement/checksum differs"
    );
    let mut code = Vec::new();
    for (name, start, length, sha) in [
        (
            "clock_input_constant_and_PLL_arithmetic",
            0x250,
            32,
            "05959ecf98d8d2b107fb3be35043c61386fa929f4c564b54bb3bb2f3264e98c7",
        ),
        (
            "revision_read_and_timer_reset",
            0x460,
            76,
            "a79f93737b8b1c456081be29e5cd8e237c07b7eb4ad802f9eed3ac488ff46fb8",
        ),
        (
            "interpreter_and_constants",
            0x4ac,
            276,
            "e5dac9552205e2a27d8da3fa113f5e83c99945cb35afa3507ab02a4a90a5da1b",
        ),
        (
            "revision_dispatch_and_pointers",
            0x5f4,
            320,
            "4b47742567604ce254bda655676cd7e0400442039f535496a2425c43d8db9666",
        ),
    ] {
        code.push(code_range(spl, name, start, length, sha)?);
    }
    ensure!(
        word(spl, 0x5b4)? == TIMER_MULTIPLIER
            && word(spl, 0x5b8)? == TIMER_RECIPROCAL
            && word(spl, 0x5bc)? == POLL_READS,
        "instruction constants differ"
    );
    ensure!(
        word(spl, 0x554)? == 0xe11c0002
            && word(spl, 0x558)? == 0x1a00000f
            && word(spl, 0x55c)? == 0xe2533001
            && word(spl, 0x560)? == 0x1afffffa
            && word(spl, 0x564)? == 0xe3a00002,
        "poll instruction facts differ"
    );
    // The two remaining groups retain neutral names until their register
    // meanings are independently checked. Call order is already byte-pinned.
    let roles = [
        "register_group0",
        "pll",
        "clocks",
        "register_group3",
        "peripherals",
    ];
    let mut revisions = Vec::new();
    for (condition, pointer_start, expected_starts) in [
        (
            "revision_field == 0",
            0x6f8,
            [0x102a0, 0x10120, 0xed44, 0xfc14, 0x107ec],
        ),
        (
            "revision_field == 1",
            0x70c,
            [0xf628, 0xf4a8, 0x1146c, 0xef78, 0xfaa4],
        ),
        (
            "revision_field != 0 && revision_field != 1",
            0x720,
            [0x10fb4, 0x10e34, 0x1071c, 0x10918, 0xee4c],
        ),
    ] {
        let mut tables = Vec::new();
        for (n, role) in roles.iter().enumerate() {
            let pointer_offset = pointer_start + n * 4;
            let start = word(spl, pointer_offset)? as usize;
            ensure!(
                start == expected_starts[n],
                "revision table pointer differs"
            );
            let table = decode_table(spl, start).with_context(|| format!("{condition} {role}"))?;
            tables.push(json!({"role":role,"pointer_spl_offset":pointer_offset,
                "pointer_boot_offset":SOURCE_OFFSET+pointer_offset,"table":table}));
        }
        revisions.push(json!({"condition":condition,"tables_in_call_order":tables}));
    }
    Ok(json!({"schema":"md-sim-selected-s9-spl-contract/1",
        "scope":"Static source-derived contracts for the exact archived SPL. No ARM instruction execution, selected physical silicon revision, BootROM handoff, physical latency, FPGA equivalence, firmware boot or calibration pass.",
        "boot_parent":object(boot),"spl_source":object(spl),"source_offset":SOURCE_OFFSET,
        "boot_header":{"declared_length":word(boot,0x34)?,"available_suffix_bytes":SPL_SIZE,
            "limit":"The declared length includes the parent file rather than only the suffix. No BootROM copy semantics are inferred."},
        "code_ranges":code,"nominal_input_clock_hz":word(spl,0x26c)?,
        "revision_selector":{"address":0xf8007080u32,"shift":28,"mask":15,
            "selected_physical_revision":Value::Null,"existing_adapted_trace_branch":"neither 0 nor 1; not physical evidence"},
        "poll_contract":{"condition":"(read_value & mask) != 0; any selected bit",
            "maximum_reads":POLL_READS,"timeout_return":2,"budget_unit":"reads, not calibrated time",
            "zero_mask_can_succeed":poll_ready(u32::MAX,0),"timeout_execution_verified":false},
        "delay_contract":{"arithmetic":"u64(argument.wrapping_mul(0x27bc86a4)) * 0x10624dd3 >>39",
            "argument1_ticks":delay_ticks(1).1,"initial_writes":[[0xf8f00208u32,0],[0xf8f00200u32,0],[0xf8f00204u32,0],[0xf8f00208u32,9]],
            "completion":"read the record's counter_address until its unsigned low 32-bit value >= threshold_ticks",
            "physical_time_verified":false},"revisions":revisions,
        "limits":{"maximum_records_per_table":MAX_RECORDS,"maximum_bytes_per_table":MAX_TABLE_BYTES,
            "supported_operations":["stop","clear","write","masked_write","poll_any","timer_delay"]}}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bytes(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_le_bytes()).collect()
    }

    #[test]
    fn exact_clock_table_from_independent_binary_range() {
        // Literal words copied from the archived range1071c..107e8 in450.
        let raw = bytes(&[
            0x22, 0xf8000008, 0xdf0d, 0x33, 0xf8000128, 0x03f03f01, 0x00700f01, 0x33, 0xf8000138,
            0x11, 1, 0x33, 0xf8000140, 0x03f03f71, 0x00100801, 0x33, 0xf8000148, 0x3f31, 0xa01,
            0x33, 0xf8000150, 0x3f33, 0x2801, 0x33, 0xf8000154, 0x3f33, 0xa02, 0x33, 0xf8000168,
            0x3f31, 0x501, 0x33, 0xf8000170, 0x03f03f30, 0x00200500, 0x33, 0xf8000180, 0x03f03f30,
            0x00200500, 0x33, 0xf80001c4, 1, 1, 0x33, 0xf800012c, 0x01ffcccd, 0x016c044d, 0x22,
            0xf8000004, 0x767b, 0,
        ]);
        assert_eq!(
            hash_bytes(&raw),
            "741e08636cf9195f7c27a543c7b6ac76def7211df2e0aec3925f6a413f173084"
        );
        let table = decode_table(&raw, 0).unwrap();
        assert_eq!(table.records.len(), 14);
        assert_eq!(table.end_exclusive, 204);
        assert_eq!(
            table.records[0].effect,
            Operation::Write {
                address: 0xf8000008,
                value: 0xdf0d
            }
        );
        assert_eq!(
            table.records[8].effect,
            Operation::MaskedWrite {
                address: 0xf8000170,
                mask: 0x03f03f30,
                value: 0x00200500
            }
        );
        assert_eq!(
            table.records[9].effect,
            Operation::MaskedWrite {
                address: 0xf8000180,
                mask: 0x03f03f30,
                value: 0x00200500
            }
        );
        assert_eq!(table.records[13].effect, Operation::Stop);
    }

    #[test]
    fn poll_is_any_bit_with_a_read_count_budget_not_elapsed_time() {
        assert!(poll_ready(1, 3));
        assert!(poll_ready(2, 3));
        assert!(!poll_ready(4, 3));
        assert!(!poll_ready(u32::MAX, 0));
        let table = decode_table(&bytes(&[0x42, 0xf800010c, 3, 0]), 0).unwrap();
        assert_eq!(
            table.records[0].effect,
            Operation::PollAny {
                address: 0xf800010c,
                mask: 3,
                maximum_reads: 100_000_001,
                timeout_return: 2
            }
        );
    }

    #[test]
    fn timer_wrap_and_fixed_point_rounding_preserve_instruction_order() {
        assert_eq!(delay_ticks(0), (0, 0));
        assert_eq!(delay_ticks(1), (666_666_660, 333_333));
        assert_eq!(delay_ticks(2), (1_333_333_320, 666_666));
        // Seven nominal units wrap MUL's 32-bit result before UMULL.
        assert_eq!(delay_ticks(7), (371_699_324, 185_849));
        assert!(delay_ticks(7).1 < delay_ticks(6).1);
        let table = decode_table(&bytes(&[0x52, 0xf8f00200, 1, 0]), 0).unwrap();
        assert_eq!(
            table.records[0].effect,
            Operation::TimerDelay {
                counter_address: 0xf8f00200,
                argument: 1,
                wrapped_product: 666_666_660,
                threshold_ticks: 333_333
            }
        );
    }

    #[test]
    fn malformed_unknown_unterminated_and_truncated_tables_fail() {
        for words in [
            &[0x60][..],
            &[0x32, 1, 2, 0],
            &[0x01, 0],
            &[0x21, 1, 0],
            &[0x42, 1],
            &[0x33, 1, 2, 3],
        ] {
            assert!(decode_table(&bytes(words), 0).is_err(), "{words:?}");
        }
        let valid = bytes(&[0x33, 0xf8000100, 1, 0, 0]);
        for length in 0..valid.len() {
            assert!(decode_table(&valid[..length], 0).is_err());
        }
        assert!(decode_table(&valid, 1).is_err());
        assert!(decode_table(&valid, usize::MAX - 3).is_err());
        let unbounded = bytes(&[0x11, 0xf8000000].repeat(MAX_RECORDS + 1));
        assert!(decode_table(&unbounded, 0)
            .unwrap_err()
            .to_string()
            .contains("record bound"));
        // Terminator belongs to this table; following bytes may be another table.
        let terminated = bytes(&[0, 0x60]);
        assert_eq!(decode_table(&terminated, 0).unwrap().end_exclusive, 4);
    }

    #[test]
    fn source_and_instruction_changes_are_rejected_before_semantic_claims() {
        assert!(inspect(&vec![0; BOOT_SIZE], &vec![0; SPL_SIZE]).is_err());
        assert!(inspect(&[], &[]).is_err());
        let known = bytes(&[0xe11c0002, 0x1a00000f]);
        let expected = hash_bytes(&known);
        assert!(code_range(&known, "poll", 0, 8, &expected).is_ok());
        let changed = bytes(&[0xe11c0002, 0x0a00000f]);
        assert!(code_range(&changed, "poll", 0, 8, &expected).is_err());
    }

    #[test]
    #[ignore = "read-only exact retained input check; set MD_SIM_SPL_BUNDLE to the directory containing boot.bin and spl-payload.bin"]
    fn retained_selected_tables_and_parent_identity() {
        let root = std::path::PathBuf::from(
            std::env::var_os("MD_SIM_SPL_BUNDLE").expect("retained bundle required"),
        );
        let report = inspect(
            &std::fs::read(root.join("boot.bin")).unwrap(),
            &std::fs::read(root.join("spl-payload.bin")).unwrap(),
        )
        .unwrap();
        assert_eq!(report["revisions"].as_array().unwrap().len(), 3);
        let tables = &report["revisions"][2]["tables_in_call_order"];
        for (index, count) in [(1, 26), (2, 14), (4, 22)] {
            assert_eq!(
                tables[index]["table"]["records"].as_array().unwrap().len(),
                count
            );
        }
        assert_eq!(
            tables[2]["table"]["encoded"]["sha256"],
            "741e08636cf9195f7c27a543c7b6ac76def7211df2e0aec3925f6a413f173084"
        );
        let mut changed = std::fs::read(root.join("spl-payload.bin")).unwrap();
        changed[0x554] ^= 1;
        assert!(inspect(&std::fs::read(root.join("boot.bin")).unwrap(), &changed).is_err());
    }
}

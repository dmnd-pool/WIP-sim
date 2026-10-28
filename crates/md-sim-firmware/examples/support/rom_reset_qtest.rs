//! Actual QEMU ROM protection and OCM remap/reset checks. No CPU execution.
use super::{qmp, Transactions};
use anyhow::{ensure, Result};
use md_sim_runtime::{
    isolation::Mount,
    store::{ObjectId, Store},
};
use serde_json::Value;
use std::{
    io::{BufRead, Write},
    path::Path,
};

pub const SHA: &str = "c58e2bc1d837654457ed47064accb26c4fb48307737e60d6c2d05253e7ee3aea";
pub const SOURCE: &str =
    "artifacts/files/source/source-zynq-bootrom-3f64d2f/bootrom/zynq_bootrom.bin";
const CFG: u32 = 0xf8000910;

pub fn capture(store: &Store, inputs: &mut Vec<Mount>) -> Result<()> {
    let object = ObjectId {
        sha256: SHA.into(),
        size: 131072,
    };
    store.import(Path::new(SOURCE), &object)?;
    inputs.push(Mount {
        guest_path: "/public-rom.bin".into(),
        object,
    });
    Ok(())
}

fn image(io: &mut Transactions, rom: &[u8]) -> Result<()> {
    for (n, bytes) in rom.chunks_exact(4096).enumerate() {
        io.command_recorded(
            format!("read {:#x} 4096", n * 4096),
            format!("OK 0x{}", hex::encode(bytes)),
            true,
        )?;
    }
    Ok(())
}

fn bank_words(n: u32) -> [(u32, u32); 2] {
    [(0x20, 0x513a0000 | n), (0xfffc, 0xa5130000 | (n << 8))]
}

fn banks(io: &mut Transactions, mask: u32) -> Result<()> {
    for n in 0..4 {
        let base = if mask & (1 << n) == 0 {
            0x40000
        } else {
            0xfffc0000
        };
        for (offset, value) in bank_words(n) {
            io.read(base + n * 0x10000 + offset, value.into())?;
        }
    }
    Ok(())
}

pub fn run(
    io: &mut Transactions,
    output: &mut impl Write,
    input: &mut impl BufRead,
    events: &mut Vec<Value>,
    reject: bool,
) -> Result<()> {
    let rom = std::fs::read("/public-rom.bin")?;
    ensure!(
        rom.len() == 131072 && md_sim_firmware::prepare::hash_bytes(&rom) == SHA,
        "public ROM differs"
    );
    image(io, &rom)?;
    // Distinct writes at both ends and inside the ROM must not modify it.
    for at in [0usize, 0x64, 0x10000, 0x1fffc] {
        let original = u32::from_le_bytes(rom[at..at + 4].try_into()?);
        io.write(at as u32, !original)?;
        io.read(at as u32, original.into())?;
    }
    io.read(CFG, 0)?;
    io.write(0xf8000008, 0xdf0d)?;
    for n in 0..4 {
        for (offset, value) in bank_words(n) {
            io.write(0x40000 + n * 0x10000 + offset, value)?;
            io.read(0x40000 + n * 0x10000 + offset, value.into())?;
        }
    }
    for mask in 0..16 {
        io.write(CFG, mask)?;
        io.read(CFG, mask.into())?;
        banks(io, mask)?;
    }
    // All banks are high after mask15. Write distinct test data to the old
    // low addresses. A stale duplicate low alias would corrupt a high bank.
    // These are our writes to the reference DDR; no power-on value is assumed.
    for n in 0..4 {
        for (offset, value) in bank_words(n) {
            io.write(0x40000 + n * 0x10000 + offset, !value)?;
            io.read(0x40000 + n * 0x10000 + offset, (!value).into())?;
        }
    }
    banks(io, 15)?;
    io.write(CFG, 0)?;
    io.read(CFG, 0)?;
    banks(io, 0)?;
    for cycle in 0..2u32 {
        io.write(CFG, 15)?;
        io.read(CFG, 15)?;
        qmp(output, input, 2 + cycle * 2, "system_reset", events)?;
        qmp(output, input, 3 + cycle * 2, "query-status", events)?;
        ensure!(
            events.iter().filter(|e| e["event"] == "RESET").count() == cycle as usize + 1,
            "actual ROM mapping reset event absent"
        );
        io.read(CFG, 0)?;
        banks(io, 0)?;
        image(io, &rom)?;
        // The next cycle must write through the ordinary SLCR lock policy.
        if cycle == 0 {
            io.write(0xf8000008, 0xdf0d)?;
        }
    }
    if reject {
        io.write(0xf8000008, 0xdf0d)?;
        io.write(CFG, 0x18)?;
        // Synchronize the requested model stop before checking its state.
        let state = qmp(output, input, 7, "query-status", events)?;
        ensure!(
            state["status"] == "internal-error" && state["running"] == false,
            "unsupported private SWAP did not stop the VM: {state}"
        );
        io.read(CFG, 0)?;
        banks(io, 0)?;
        image(io, &rom)?;
    }
    ensure!(
        io.irq_events.is_empty(),
        "unexpected GIC transition in ROM mapping test"
    );
    Ok(())
}

fn reset_count(events: &[Value], count: usize, guest: bool) -> Result<()> {
    let actual: Vec<_> = events.iter().filter(|e| e["event"] == "RESET").collect();
    ensure!(actual.len() == count, "reset-reason event count differs");
    ensure!(
        actual.last().is_some_and(|e| e["data"]["guest"] == guest),
        "reset-reason event source differs"
    );
    Ok(())
}

/// Documented persistent registers only. QMP resets have no physical cause.
/// Actual PSS_RST_CTRL writes must produce guest reset events and SLC_RST.
pub fn run_reason(
    io: &mut Transactions,
    output: &mut impl Write,
    input: &mut impl BufRead,
    events: &mut Vec<Value>,
) -> Result<()> {
    const REASON: u32 = 0xf8000250;
    const CLEAR: u32 = 0xf8000254;
    const REBOOT: u32 = 0xf8000258;
    const SOFT: u32 = 0xf8000200;
    const UNLOCK: u32 = 0xf8000008;
    const LOCK: u32 = 0xf800000c;
    io.read(REASON, 0x40)?;
    io.read(CLEAR, 0)?;
    io.read(REBOOT, 0)?;
    // Read-only bits ignore both clear and set attempts, including reserved bits.
    io.write(REASON, 0)?;
    io.read(REASON, 0x40)?;
    io.write(REASON, u32::MAX)?;
    io.read(REASON, 0x40)?;
    io.write(UNLOCK, 0xdf0d)?;
    io.write(CLEAR, 0)?;
    io.read(CLEAR, 0)?;
    io.read(REASON, 0x40)?;
    io.write(CLEAR, 0xffff_fffe)?;
    io.read(CLEAR, 0)?;
    io.read(REASON, 0x40)?;
    io.write(REBOOT, 0xa513_72c9)?;
    io.read(REBOOT, 0xa513_72c9)?;
    io.write(SOFT, 0)?;
    io.read(REASON, 0x40)?;
    io.write(SOFT, 1)?;
    qmp(output, input, 20, "query-status", events)?;
    qmp(output, input, 21, "query-status", events)?;
    reset_count(events, 1, true)?;
    io.read(REASON, 0x48)?;
    io.read(REBOOT, 0xa513_72c9)?;
    io.read(SOFT, 0)?;
    io.read(LOCK, 1)?;
    io.write(UNLOCK, 0xdf0d)?;
    io.write(CLEAR, 1)?;
    io.read(CLEAR, 0)?;
    io.read(REASON, 0)?;
    io.read(REBOOT, 0xa513_72c9)?;
    qmp(output, input, 22, "system_reset", events)?;
    qmp(output, input, 23, "query-status", events)?;
    reset_count(events, 2, false)?;
    io.read(REASON, 0)?;
    io.read(REBOOT, 0xa513_72c9)?;
    io.read(LOCK, 1)?;
    io.write(UNLOCK, 0xdf0d)?;
    io.write(SOFT, 1)?;
    qmp(output, input, 24, "query-status", events)?;
    qmp(output, input, 25, "query-status", events)?;
    reset_count(events, 3, true)?;
    io.read(REASON, 8)?;
    io.read(REBOOT, 0xa513_72c9)?;
    io.write(UNLOCK, 0xdf0d)?;
    io.write(CLEAR, u32::MAX)?;
    io.read(CLEAR, 0)?;
    io.read(REASON, 0)?;
    io.write(REBOOT, 0)?;
    io.read(REBOOT, 0)?;
    qmp(output, input, 26, "system_reset", events)?;
    qmp(output, input, 27, "query-status", events)?;
    reset_count(events, 4, false)?;
    io.read(REASON, 0)?;
    io.read(REBOOT, 0)?;
    ensure!(
        io.irq_events.is_empty(),
        "unexpected reset-reason GIC event"
    );
    Ok(())
}

/// Only the documented LVTTL register state; no electrical pin model claim.
pub fn run_lvttl(
    io: &mut Transactions,
    output: &mut impl Write,
    input: &mut impl BufRead,
    events: &mut Vec<Value>,
) -> Result<()> {
    const REG: u32 = 0xf8000b10;
    const NORMAL: u32 = 0x0c301166; // Original UG585v1.0 BootROM value table.
    io.read(REG, 0)?;
    io.write(0xf8000008, 0xdf0d)?;
    io.write(REG, NORMAL)?;
    io.read(REG, NORMAL.into())?;
    io.write(REG, NORMAL | 0xf0000000)?;
    io.read(REG, NORMAL.into())?;
    // Exercise every modeled storage bit, including bit0, independently of
    // the original firmware value. This is not an electrical pin stimulus.
    io.write(REG, u32::MAX)?;
    io.read(REG, 0x0fff_ffff)?;
    io.write(REG, 0)?;
    io.read(REG, 0)?;
    io.write(REG, NORMAL)?;
    io.read(REG, NORMAL.into())?;
    for cycle in 0..2 {
        qmp(output, input, 20 + cycle * 2, "system_reset", events)?;
        qmp(output, input, 21 + cycle * 2, "query-status", events)?;
        reset_count(events, cycle as usize + 1, false)?;
        io.read(REG, 0)?;
        io.read(0xf800000c, 1)?;
        if cycle == 0 {
            io.write(0xf8000008, 0xdf0d)?;
            io.write(REG, NORMAL)?;
            io.read(REG, NORMAL.into())?;
        }
    }
    ensure!(io.irq_events.is_empty(), "unexpected LVTTL GIC event");
    Ok(())
}

pub fn verify_diagnostics(text: &str, reject: bool) -> bool {
    let mut errors = 0;
    for line in text.lines() {
        if [
            "md_sim_zynq_fclk ",
            "md_sim_zynq_freset ",
            "md_sim_zynq_rom_map ",
            "md_sim_zynq_cpu_control ",
        ]
        .iter()
        .any(|prefix| line.starts_with(prefix))
        {
            continue;
        }
        if line == "qemu: md-sim: unsupported ROM OCM configuration0x18" {
            errors += 1;
        } else {
            return false;
        }
    }
    errors == usize::from(reject)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reset_reason_requires_actual_count_and_source() {
        let reset = serde_json::json!({"event":"RESET", "data":{"guest":true}});
        assert!(reset_count(std::slice::from_ref(&reset), 1, true).is_ok());
        assert!(reset_count(std::slice::from_ref(&reset), 1, false).is_err());
        assert!(reset_count(&[reset.clone(), reset], 1, true).is_err());
        assert!(reset_count(&[serde_json::json!({"event":"STOP"})], 1, false).is_err());
        assert!(reset_count(&[serde_json::json!({"event":"RESET"})], 1, false).is_err());
    }

    #[test]
    fn unexpected_diagnostics_do_not_pass() {
        let error = "qemu: md-sim: unsupported ROM OCM configuration0x18\n";
        assert!(verify_diagnostics(error, true));
        assert!(!verify_diagnostics(error, false));
        assert!(!verify_diagnostics("", true));
        assert!(!verify_diagnostics(
            "md_sim_unknown_error surprise\n",
            false
        ));
        assert!(!verify_diagnostics(&error.repeat(2), true));
        assert!(!verify_diagnostics(
            &(error.to_owned() + "unrelated failure\n"),
            true
        ));
        assert!(verify_diagnostics(
            "md_sim_zynq_rom_map ram_hi=0x0 private_swap=unsupported\n",
            false
        ));
    }
}

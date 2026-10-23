//! Actual DMA reads, configuration status, programming reset and system reset.
use super::{qmp, Transactions};
use anyhow::{ensure, Result};
use md_sim_firmware::bitstream::{parse_header, series7::inspect};
use md_sim_runtime::{
    isolation::Mount,
    store::{hash_file, Store},
};
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, Write},
    path::Path,
};

const BASE: u32 = 0xf8007000;
const RAM: u32 = 0x100000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    ChangedMemory,
    FixedSource,
    ControlRecovery,
    UnsupportedDma,
    UnlockReset,
}

impl Case {
    pub fn from_kind(kind: &str) -> Option<Self> {
        match kind {
            "pcap" => Some(Self::ChangedMemory),
            "pcap-fixed-source" => Some(Self::FixedSource),
            "pcap-control-recovery" => Some(Self::ControlRecovery),
            "pcap-unsupported-dma" => Some(Self::UnsupportedDma),
            "pcap-unlock-reset" => Some(Self::UnlockReset),
            _ => None,
        }
    }
}

pub fn capture(store: &Store, inputs: &mut Vec<Mount>, output: &Path) -> Result<()> {
    for (path, guest, expected) in [
        (
            "work/simulator/original-zynq/sd-bundle-291/boot/system.bit",
            "/pcap-reference.bit",
            "980532de5eb666508dafb111466f30169a70b8b6ea662bdda61e17dbd55a4421",
        ),
        (
            "fixtures/fpga/xc7z010clg400-1/part.json",
            "/pcap-part.json",
            "1754ec1c7a8e0447a116984505cc422819d3be06389527de96bf192c5175d095",
        ),
    ] {
        let object = hash_file(Path::new(path))?;
        ensure!(object.sha256 == expected, "PCAP fixture identity differs");
        store.import(Path::new(path), &object)?;
        inputs.push(Mount {
            object,
            guest_path: guest.into(),
        });
    }
    let bytes = fs::read("work/simulator/original-zynq/sd-bundle-291/boot/system.bit")?;
    let header = parse_header(&bytes)?;
    let little: Vec<_> = bytes[header.configuration_offset..]
        .chunks_exact(4)
        .flat_map(|w| w.iter().rev().copied())
        .collect();
    let path = output.join("dma-memory.bin");
    fs::write(&path, &little)?;
    let object = hash_file(&path)?;
    store.import(&path, &object)?;
    inputs.push(Mount {
        object,
        guest_path: "/pcap-dma.bin".into(),
    });
    Ok(())
}

fn clear(io: &mut Transactions) -> Result<()> {
    io.write(BASE + 0xc, u32::MAX)?;
    io.read(BASE + 0xc, 0)?;
    io.write(BASE + 0x10, u32::MAX)?;
    io.read(BASE + 0xc, 0) // Mask writes must not create PCFG_DONE.
}

fn begin(io: &mut Transactions) -> Result<()> {
    io.write(BASE, 0x0c006000)?;
    clear(io)?;
    io.write(BASE, 0x4c006000)?;
    io.read(BASE + 0xc, 0)
}

fn dma(io: &mut Transactions, offset: u32, size: u32, expected: u64) -> Result<()> {
    io.write(BASE + 0x18, RAM + offset)?;
    io.write(BASE + 0x1c, u32::MAX)?;
    io.write(BASE + 0x20, size / 4)?;
    io.write(BASE + 0x24, 0)?;
    io.read(BASE + 0xc, expected)
}

fn interrupts(io: &Transactions, count: usize) -> Result<()> {
    // Archived xilinx_zynq.c connects DevC interrupt 40 to GIC input 8.
    let expected: Vec<_> = (0..count).map(|i| (8, i % 2 == 0)).collect();
    ensure!(
        io.irq_events == expected,
        "configuration GIC interrupt history differs"
    );
    Ok(())
}

pub fn run(
    io: &mut Transactions,
    output: &mut impl Write,
    input: &mut impl BufRead,
    events: &mut Vec<Value>,
    case: Case,
) -> Result<()> {
    let bytes = fs::read("/pcap-reference.bit")?;
    let header = parse_header(&bytes)?;
    let payload = &bytes[header.configuration_offset..];
    let sync = inspect(payload, 0x03722093)?.sync_byte_offset as u32;
    let size = payload.len() as u32;
    io.read(BASE + 0xc, 0x88020000)?; // PSS reset state and empty write FIFO.
    clear(io)?;
    begin(io)?;
    io.write(BASE + 0x10, !4)?; // Only configuration completion can raise IRQ.
    dma(io, 0, 1024, 0x3000)?; // Partial image cannot set PCFG_DONE.
    interrupts(io, 0)?;
    clear(io)?;
    io.write(BASE + 0x10, !4)?;
    dma(io, 1024, size - 1024, 0x3004)?;
    interrupts(io, 1)?;
    io.write(BASE + 0x10, u32::MAX)?;
    io.read(BASE + 0xc, 0x3004)?; // Masking leaves the pending status intact.
    interrupts(io, 2)?;
    io.write(BASE + 0x10, !4)?;
    io.read(BASE + 0xc, 0x3004)?;
    interrupts(io, 3)?;
    io.write(BASE + 0xc, 0x3000)?;
    io.read(BASE + 0xc, 4)?; // Clearing DMA flags cannot clear configuration IRQ.
    interrupts(io, 3)?;
    io.write(BASE, 0x0c006000)?;
    io.read(BASE + 0xc, 0)?;
    interrupts(io, 4)?; // The line must agree with the cleared status.
    clear(io)?; // Completion is a sticky interrupt, not invented by readback.
    for cycle in 0..2 {
        qmp(output, input, 2 + cycle * 2, "system_reset", events)?;
        qmp(output, input, 3 + cycle * 2, "query-status", events)?;
        ensure!(
            events.iter().filter(|e| e["event"] == "RESET").count() == cycle as usize + 1,
            "PCAP actual system reset absent"
        );
        io.read(BASE + 0xc, 0x88020000)?;
        interrupts(io, 4 + 2 * cycle as usize)?;
        clear(io)?;
        begin(io)?;
        dma(io, 0, 1024, 0x3000)?;
        begin(io)?; // Programming reset must discard the first partial image.
        io.write(BASE + 0x10, !4)?;
        dma(io, sync, size - sync, 0x3004)?;
        interrupts(io, 5 + 2 * cycle as usize)?;
        if cycle == 1 {
            clear(io)?;
            interrupts(io, 8)?;
        } // The first iteration leaves IRQ asserted across the next QMP reset.
    }
    begin(io)?;
    if case == Case::UnlockReset {
        io.write(BASE + 0x34, 0)?; // Invalid key makes the region inaccessible.
        io.read(BASE + 8, 0)?; // QEMU unassigned read, not a physical bus value.
        io.write(BASE + 0x34, 0x757bdf0d)?;
        io.write(BASE, 0x4c006000)?;
        io.read(BASE + 8, 0)?; // An inaccessible key register cannot unlock it.
        qmp(output, input, 6, "system_reset", events)?;
        qmp(output, input, 7, "query-status", events)?;
        ensure!(
            events.iter().filter(|e| e["event"] == "RESET").count() == 3,
            "unlock recovery system reset absent"
        );
        io.read(BASE + 8, 0x50b)?;
        io.read(BASE + 0xc, 0x88020000)?;
        begin(io)?;
        dma(io, 0, size, 0x3004)?;
        interrupts(io, 8)?;
        return Ok(());
    }
    if case == Case::UnsupportedDma {
        // The upstream non-loopback path cannot consume a nonzero destination
        // length after its source is empty. Strict mode must reject this command.
        io.write(BASE + 0x18, RAM)?;
        io.write(BASE + 0x1c, RAM + 0x400000)?;
        io.write(BASE + 0x20, 0)?;
        io.write(BASE + 0x24, 1)?;
        io.read(BASE + 0xc, 0)?;
        dma(io, 0, size, 0)?; // Changing the destination cannot clear failure.
        begin(io)?;
        // A valid source must not make a different destination acceptable.
        io.write(BASE + 0x18, RAM)?;
        io.write(BASE + 0x1c, RAM + 0x400000)?;
        io.write(BASE + 0x20, size / 4)?;
        io.write(BASE + 0x24, 0)?;
        io.read(BASE + 0xc, 0)?;
        dma(io, 0, size, 0)?;
        begin(io)?;
        dma(io, 0, size, 0x3004)?;
        interrupts(io, 8)?;
        return Ok(());
    }
    if case == Case::ControlRecovery {
        io.write(BASE, 0x44006000)?; // Invalid PCAP_PR, without programming reset.
        dma(io, 0, size, 0)?;
        io.write(BASE, 0x4c006000)?; // Restoring controls cannot clear failure.
        dma(io, 0, size, 0)?;
        begin(io)?;
        dma(io, 0, size, 0x3004)?; // A real reset and enable permit recovery.
        interrupts(io, 8)?;
        return Ok(());
    }
    if case == Case::FixedSource {
        // The first eight words are all ones. The word at byte 32 is not.
        // Fixed reads from byte 16 must supply eight copies of that one word;
        // an incrementing implementation reaches different data after 16 bytes.
        ensure!(payload[..32].iter().all(|&byte| byte == 0xff));
        ensure!(&payload[32..36] == [0, 0, 0, 0xbb]);
        io.write(BASE + 8, 0x52b)?;
        io.read(BASE + 8, 0x52b)?;
        dma(io, 16, 32, 0x3000)?;
        clear(io)?;
        io.write(BASE + 8, 0x50b)?;
        dma(io, 32, size - 32, 0x3004)?;
        clear(io)?;
        begin(io)?;
        io.write(BASE + 8, 0x52b)?;
    } else {
        // Mutate guest RAM only. The pinned reference and archive remain unchanged.
        let old = u32::from_be_bytes(payload[20..24].try_into()?);
        io.write(RAM + 20, old ^ 1)?;
    }
    dma(io, 0, size, 0)?; // Model error; no successful DMA/configuration flags.
    interrupts(io, 8)?;
    Ok(())
}

pub fn verify(log: &str, case: Case) -> bool {
    let completed: Vec<_> = log
        .lines()
        .filter(|l| l.starts_with("md-sim-pcap: dma ") && l.contains(" complete=1 "))
        .collect();
    let extra_image = case != Case::ChangedMemory;
    let images_match = completed.len() == if extra_image { 4 } else { 3 }
        && completed[0].contains("received=2083740 omitted_preamble=0 complete=1 sha256=49d0faa400fbc8bed3f3e651da2c6da5f3184a08f942ec95db59c2526dcbea78")
        && completed[1..3].iter().all(|line| line.contains("received=2083692 omitted_preamble=48 complete=1 sha256=fa7e493d1d764d1b395eb46e2a68961cb8815a8b993e0ff3f8c8d8e8c30ba132"))
        && (!extra_image || completed[3].contains("received=2083740 omitted_preamble=0 complete=1 sha256=49d0faa400fbc8bed3f3e651da2c6da5f3184a08f942ec95db59c2526dcbea78"));
    images_match && match case {
        Case::FixedSource => log.lines().any(|line| line.starts_with("md-sim-pcap: dma address=0x100010 bytes=32 received=32 omitted_preamble=0 complete=0 ") && line.ends_with("source_increment=0"))
            && log.contains("md-sim-pcap: rejected: configuration DMA differs at payload byte 32"),
        Case::ChangedMemory => log.contains("md-sim-pcap: rejected: configuration DMA differs at payload byte 20"),
        Case::ControlRecovery | Case::UnsupportedDma => log.contains("md-sim-pcap: rejected: unsupported PCAP DMA controls or address range")
            && log.contains("md-sim-pcap: rejected: configuration port is not ready"),
        Case::UnlockReset => log.lines().filter(|line| *line == "md-sim-pcap: reset").count() == 4,
    }
}

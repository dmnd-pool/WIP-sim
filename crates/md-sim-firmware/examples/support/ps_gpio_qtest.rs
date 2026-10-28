//! Literal Zynq7000 PS GPIO bus/IRQ vectors. No CPU or physical pad claim.
use super::{qmp, Transactions};
use anyhow::{ensure, Result};
use serde_json::Value;
use std::io::{BufRead, Write};

const BASE: u32 = 0xe000_a000;
#[derive(Clone, Debug)]
enum Action {
    Read(u32, u32),
    Write(u32, u32),
    Pin(u32, bool),
    Reset,
    Fault(&'static str, &'static str, bool, u32, u64),
}

fn positive() -> Vec<Action> {
    use Action::*;
    let mut v = Vec::new();
    for (bank, mask) in [u32::MAX, 0x003f_ffff, u32::MAX, u32::MAX]
        .into_iter()
        .enumerate()
    {
        let c = BASE + 0x200 + bank as u32 * 0x40;
        v.extend([
            Read(c + 4, 0),
            Read(c + 8, 0),
            Read(c + 12, 0),
            Read(c + 28, mask),
            Write(c + 20, mask),
            Read(c + 12, mask),
        ]);
    }
    v.extend([
        Read(BASE + 0x48, 0),
        Read(BASE + 0x4c, 0),
        Write(BASE + 0x40, 0),
        Read(BASE + 0x40, 0),
        // UG58514.3.3 literal masked-write example, with known prior0.
        Write(BASE + 4, 0xbdef_4210),
        Read(BASE + 0x40, 0x4210_0000),
        Write(BASE, 0xfffe_0001),
        Read(BASE + 0x40, 0x4210_0001),
        Read(BASE, 1),
        Read(BASE + 4, 0x4210),
        Write(BASE + 0x44, u32::MAX),
        Read(BASE + 0x44, 0x003f_ffff),
        Write(BASE + 12, 0xfffe_0000),
        Read(BASE + 0x44, 0x003e_ffff),
        Read(BASE + 12, 0x3e),
        Write(BASE + 0x244, u32::MAX),
        Read(BASE + 0x244, 0x003f_ffff),
        Write(BASE + 0x248, u32::MAX),
        Read(BASE + 0x248, 0x003f_ffff),
        // Initial explicit samples can leave an unknown edge; W1C follows.
        Pin(54, false),
        Pin(86, false),
        Write(BASE + 0x2a0, 1),
        Write(BASE + 0x2e0, 1),
        Write(BASE + 0x298, u32::MAX),
        Write(BASE + 0x2d8, u32::MAX),
        Write(BASE + 0x290, 1),
        Write(BASE + 0x2d0, 1),
        Pin(54, true),
        Read(BASE + 0x298, 1),
        Pin(86, true),
        Read(BASE + 0x2d8, 1),
        Write(BASE + 0x298, 1),
        Read(BASE + 0x298, 0),
        Write(BASE + 0x2d4, 1),
        Write(BASE + 0x2d0, 1),
        Write(BASE + 0x2d8, 1),
        // Falling edge, then either edge. Each event is latched and cleared.
        Write(BASE + 0x2a0, 0),
        Pin(54, false),
        Read(BASE + 0x298, 1),
        Write(BASE + 0x298, 1),
        Write(BASE + 0x2a4, 1),
        Pin(54, true),
        Read(BASE + 0x298, 1),
        Write(BASE + 0x298, 1),
        Pin(54, false),
        Read(BASE + 0x298, 1),
        Write(BASE + 0x298, 1),
        // Active-low level; clearing does not suppress a still-active input.
        Write(BASE + 0x294, 1),
        Write(BASE + 0x29c, 0xffff_fffe),
        Read(BASE + 0x298, 1),
        Write(BASE + 0x290, 1),
        Write(BASE + 0x298, 1),
        Read(BASE + 0x298, 1),
        Pin(54, true),
        // Active-high level, without a fabricated transition.
        Write(BASE + 0x2a0, 1),
        Read(BASE + 0x298, 1),
        Write(BASE + 0x298, 1),
        Read(BASE + 0x298, 1),
        Pin(54, false),
        Write(BASE + 0x294, u32::MAX),
        Write(BASE + 0x2d4, u32::MAX),
        Write(BASE + 0x48, 0xf00d),
    ]);
    // A fully supplied EMIO input word is independent of its output latch.
    for bit in 0..32 {
        v.push(Pin(54 + bit, 0x1234_5678u32 & (1 << bit) != 0));
    }
    v.extend([
        Read(BASE + 0x68, 0x1234_5678),
        Read(BASE + 0x48, 0xf00d),
        Write(BASE + 0x284, u32::MAX),
        Write(BASE + 0x288, 0x5555_5555),
        Read(BASE + 0x284, u32::MAX),
        Read(BASE + 0x288, 0x5555_5555),
        Read(BASE + 0x68, 0x1234_5678),
        Write(0xf800_0008, 0xdf0d),
        Write(0xf800_022c, u32::MAX),
        Read(0xf800_022c, 1),
        Write(0xf800_022c, 0),
        Read(0xf800_022c, 0),
        Read(BASE + 0x48, 0xf00d),
        Read(BASE + 0x288, 0x5555_5555),
        Write(0xf800_012c, 0x01bf_cccd),
        Write(0xf800_012c, 0x01ff_cccd),
        Read(BASE + 0x48, 0xf00d),
        Reset,
        Read(BASE + 0x48, 0xf00d),
        Read(BASE + 0x288, 0x5555_5555),
        Read(BASE + 0x28c, u32::MAX),
        Read(BASE + 0x68, 0x1234_5678),
        Reset,
        Read(BASE + 0x48, 0xf00d),
        Read(BASE + 0x288, 0x5555_5555),
        Read(BASE + 0x28c, u32::MAX),
        Read(BASE + 0x68, 0x1234_5678),
    ]);
    v
}

fn negative() -> Vec<Action> {
    use Action::*;
    vec![
        Fault("readl 0xe000a040", "unknown read", false, 0x40, 0),
        Fault("readl 0xe000a068", "unknown read", false, 0x68, 0),
        Fault("writel 0xe000a304 1", "write", true, 0x304, 1),
        Fault("writew 0xe000a204 1", "write", true, 0x204, 1),
        Fault("writel 0xe000a205 1", "write", true, 0x205, 1),
        Fault("readq 0xe000a204", "read", false, 0x204, 0),
        Fault(
            "writeq 0xe000a204 0x100000001",
            "write",
            true,
            0x204,
            0x1_0000_0001,
        ),
        Read(BASE + 0x204, 0),
        Read(BASE + 0x208, 0),
        Write(0xf800_0008, 0xdf0d),
        Write(0xf800_022c, 1),
        Fault("writel 0xe000a204 1", "write", true, 0x204, 1),
        Write(0xf800_022c, 0),
        Read(BASE + 0x204, 0),
        Write(0xf800_012c, 0x01bf_cccd),
        Fault("writel 0xe000a204 1", "write", true, 0x204, 1),
        Write(0xf800_012c, 0x01ff_cccd),
        Read(BASE + 0x204, 0),
    ]
}

pub fn transaction_count(reject: bool) -> usize {
    if reject {
        19
    } else {
        144
    }
}

pub fn run(
    io: &mut Transactions,
    output: &mut impl Write,
    input: &mut impl BufRead,
    events: &mut Vec<Value>,
    reject: bool,
) -> Result<()> {
    let mut resets = 0;
    for (index, action) in if reject { negative() } else { positive() }
        .iter()
        .enumerate()
    {
        match *action {
            Action::Read(addr, value) => io.read(addr, value.into())?,
            Action::Write(addr, value) => io.write(addr, value)?,
            Action::Pin(pin, value) => io.command(
                format!(
                    "set_irq_in /machine/ps-gpio input {pin} {}",
                    u8::from(value)
                ),
                "OK".into(),
            )?,
            Action::Reset => {
                qmp(
                    output,
                    input,
                    100 + index as u32 * 2,
                    "system_reset",
                    events,
                )?;
                qmp(
                    output,
                    input,
                    101 + index as u32 * 2,
                    "query-status",
                    events,
                )?;
                resets += 1;
                ensure!(
                    events.iter().filter(|e| e["event"] == "RESET").count() == resets,
                    "actual GPIO reset count differs"
                );
            }
            Action::Fault(command, _, write, _, _) => {
                io.command(
                    command.into(),
                    if write {
                        "OK".into()
                    } else {
                        "OK 0x0000000000000000".into()
                    },
                )?;
                let state = qmp(output, input, 100 + index as u32, "query-status", events)?;
                ensure!(
                    state["status"] == "internal-error" && state["running"] == false,
                    "unsupported GPIO access did not stop the reference VM"
                );
            }
        }
    }
    let expected: Vec<_> = if reject {
        vec![]
    } else {
        // Two-bank reduction/mask:4; falling/either:6; levels:4 transitions.
        (0..14).map(|i| (20, i % 2 == 0)).collect()
    };
    ensure!(
        io.irq_events == expected,
        "actual GPIO IRQ52 history differs: {:?}",
        io.irq_events
    );
    Ok(())
}

fn expected_diagnostics(reject: bool) -> Vec<String> {
    let mut expected = Vec::new();
    for action in if reject { negative() } else { positive() } {
        let (write, addr, value, error) = match action {
            Action::Read(addr, value) if (BASE..BASE + 4096).contains(&addr) => {
                (false, addr - BASE, u64::from(value), false)
            }
            Action::Write(addr, value) if (BASE..BASE + 4096).contains(&addr) => {
                (true, addr - BASE, u64::from(value), false)
            }
            Action::Fault(_, operation, write, addr, value) => {
                // Write traces precede the stop; read traces follow it.
                let diagnostic =
                    format!("qemu: md-sim: unsupported PS GPIO {operation} at0x{addr:x}");
                let trace = format!(
                    "md_sim_ps_gpio_mmio write={} addr=0x{addr:x} value=0x{value:x} error=1",
                    u8::from(write)
                );
                if write {
                    expected.extend([trace, diagnostic]);
                } else {
                    expected.extend([diagnostic, trace]);
                }
                continue;
            }
            _ => continue,
        };
        expected.push(format!(
            "md_sim_ps_gpio_mmio write={} addr=0x{addr:x} value=0x{value:x} error={}",
            u8::from(write),
            u8::from(error)
        ));
    }
    expected
}

pub fn verify_diagnostics(text: &str, reject: bool) -> bool {
    if !text.ends_with('\n') {
        return false;
    }
    let expected = expected_diagnostics(reject);
    let actual: Vec<_> = text
        .lines()
        .filter(|line| {
            ![
                "md_sim_zynq_fclk ",
                "md_sim_zynq_freset ",
                "md_sim_zynq_rom_map ",
                "md_sim_zynq_cpu_control ",
            ]
            .iter()
            .any(|prefix| line.starts_with(prefix))
        })
        .collect();
    actual == expected
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostics_reject_missing_extra_reordered_or_truncated_records() {
        for reject in [false, true] {
            let expected = expected_diagnostics(reject);
            let text = expected.join("\n") + "\n";
            assert!(verify_diagnostics(&text, reject));
            assert!(!verify_diagnostics(text.trim_end_matches('\n'), reject));
            assert!(!verify_diagnostics(
                &(text.clone() + "qemu: unexpected failure\n"),
                reject
            ));
            assert!(!verify_diagnostics(
                &(expected[1..].join("\n") + "\n"),
                reject
            ));
            let mut reordered = expected.clone();
            reordered.swap(0, 1);
            assert!(!verify_diagnostics(&(reordered.join("\n") + "\n"), reject));
            let mut duplicate = expected.clone();
            duplicate.insert(1, expected[0].clone());
            assert!(!verify_diagnostics(&(duplicate.join("\n") + "\n"), reject));
            let plan = if reject { negative() } else { positive() };
            assert_eq!(
                1 + plan.iter().filter(|a| !matches!(a, Action::Reset)).count(),
                transaction_count(reject)
            );
        }
        assert!(!verify_diagnostics("", false));
        assert_eq!(transaction_count(true), 19);
        assert_eq!(transaction_count(false), 144);
        assert!(positive()
            .iter()
            .any(|a| matches!(a, Action::Write(0xe000a004, 0xbdef4210))));
        assert_eq!(
            positive()
                .iter()
                .filter(|a| matches!(a, Action::Reset))
                .count(),
            2
        );
        assert_eq!(
            negative()
                .iter()
                .filter(|a| matches!(a, Action::Fault(..)))
                .count(),
            9
        );
    }
}

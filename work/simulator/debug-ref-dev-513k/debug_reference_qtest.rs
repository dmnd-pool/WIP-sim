//! Literal historical-QEMU SLCR storage vectors. No physical debug behavior.
use super::{qmp, Transactions};
use anyhow::{ensure, Result};
use serde_json::Value;
use std::io::{BufRead, Write};

const REG: u32 = 0xf800_0500;
const UNLOCK: u32 = 0xf800_0008;
const LOCK: u32 = 0xf800_0004;
const LOCKSTA: u32 = 0xf800_000c;
#[derive(Clone)]
enum Action {
    Read(u32, u32),
    Write(u32, u32),
    Locked(u32),
    Reset,
    Fault(&'static str, bool, u32, u32),
}
fn positive() -> Vec<Action> {
    use Action::*;
    vec![
        Read(REG, 0),
        Read(LOCKSTA, 1),
        Locked(u32::MAX),
        Read(REG, 0),
        Write(UNLOCK, 0),
        Read(LOCKSTA, 1),
        Locked(0x0123_4567),
        Read(REG, 0),
        Write(UNLOCK, 0xabcd_df0d),
        Read(LOCKSTA, 0),
        Write(REG, 0x89ab_cdef),
        Read(REG, 0x89ab_cdef),
        Write(REG, u32::MAX),
        Read(REG, u32::MAX),
        Write(REG, 0),
        Read(REG, 0),
        Write(REG, 0x0123_4567),
        Read(REG, 0x0123_4567),
        Write(LOCK, 0),
        Read(LOCKSTA, 0),
        Write(REG, 0xfedc_ba98),
        Read(REG, 0xfedc_ba98),
        Write(LOCK, 0xabcd_767b),
        Read(LOCKSTA, 1),
        Locked(0),
        Read(REG, 0xfedc_ba98),
        Reset,
        Read(REG, 0xfedc_ba98),
        Read(LOCKSTA, 1),
        Write(UNLOCK, 0xdf0d),
        Write(REG, 0xaaaa_aaaa),
        Read(REG, 0xaaaa_aaaa),
        Reset,
        Read(REG, 0xaaaa_aaaa),
        Read(LOCKSTA, 1),
    ]
}
fn negative() -> Vec<Action> {
    use Action::*;
    vec![
        Write(UNLOCK, 0xdf0d),
        Write(REG, 0x1357_9bdf),
        Fault("readb 0xf8000500", false, 0x500, 1),
        Read(REG, 0x1357_9bdf),
        Fault("writeb 0xf8000501 0xff", true, 0x501, 1),
        Read(REG, 0x1357_9bdf),
        Fault("readw 0xf8000502", false, 0x502, 2),
        Read(REG, 0x1357_9bdf),
        Fault("writew 0xf8000500 0xffff", true, 0x500, 2),
        Read(REG, 0x1357_9bdf),
        Fault("readl 0xf8000501", false, 0x501, 4),
        Read(REG, 0x1357_9bdf),
        Fault("writel 0xf8000501 0xffffffff", true, 0x501, 4),
        Read(REG, 0x1357_9bdf),
        Fault("readq 0xf8000500", false, 0x500, 8),
        Read(REG, 0x1357_9bdf),
        Fault("writeq 0xf8000500 0x100000001", true, 0x500, 8),
        Read(REG, 0x1357_9bdf),
        // An overlapping doubleword must fail before its second word can write.
        Fault("writeq 0xf80004fc 0x100000001", true, 0x4fc, 8),
        Read(REG, 0x1357_9bdf),
    ]
}
fn plan(reject: bool) -> Vec<Action> {
    if reject {
        negative()
    } else {
        positive()
    }
}
pub fn transaction_count(reject: bool) -> usize {
    plan(reject)
        .iter()
        .filter(|a| !matches!(a, Action::Reset))
        .count()
}
pub fn run(
    io: &mut Transactions,
    output: &mut impl Write,
    input: &mut impl BufRead,
    events: &mut Vec<Value>,
    reject: bool,
) -> Result<()> {
    let mut resets = 0;
    for (index, action) in plan(reject).iter().enumerate() {
        match *action {
            Action::Read(address, value) => io.read(address, value.into())?,
            Action::Write(address, value) => io.write(address, value)?,
            Action::Locked(value) => io.write(REG, value)?,
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
                let actual: Vec<_> = events.iter().filter(|e| e["event"] == "RESET").collect();
                ensure!(
                    actual.len() == resets
                        && actual.last().is_some_and(|e| e["data"]["guest"] == false),
                    "actual historical debug-reference reset differs"
                );
            }
            Action::Fault(command, write, _, _) => {
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
                    "unsupported historical debug-reference access did not stop VM"
                );
            }
        }
    }
    ensure!(
        io.irq_events.is_empty(),
        "storage reference generated a hardware interrupt"
    );
    Ok(())
}
fn expected_diagnostics(reject: bool) -> Vec<String> {
    plan(reject)
        .into_iter()
        .filter_map(|a| match a {
            Action::Read(REG, value) => Some(format!(
                "md_sim_zynq_debug_reference write=0 value=0x{value:x} committed=1"
            )),
            Action::Write(REG, value) => Some(format!(
                "md_sim_zynq_debug_reference write=1 value=0x{value:x} committed=1"
            )),
            Action::Locked(value) => Some(format!(
                "md_sim_zynq_debug_reference write=1 value=0x{value:x} committed=0"
            )),
            Action::Fault(_, write, offset, size) => Some(format!(
                "qemu: md-sim: unsupported ROM SLCR {} size{size} at0x{offset:x}",
                if write { "write" } else { "read" }
            )),
            _ => None,
        })
        .collect()
}
fn metadata_line(line: &str) -> bool {
    if let Some(fields) = line.strip_prefix("md_sim_zynq_fclk channel=") {
        let Some((channel, period)) = fields.split_once(" period=") else {
            return false;
        };
        return channel
            .parse::<u8>()
            .is_ok_and(|c| c < 4 && c.to_string() == channel)
            && period.parse::<u64>().is_ok_and(|p| p.to_string() == period);
    }
    if let Some(fields) = line.strip_prefix("md_sim_zynq_freset channel=") {
        let Some((channel, high)) = fields.split_once(" high=") else {
            return false;
        };
        return channel
            .parse::<u8>()
            .is_ok_and(|c| c < 4 && c.to_string() == channel)
            && matches!(high, "0" | "1");
    }
    if let Some(fields) = line.strip_prefix("md_sim_zynq_rom_map ram_hi=0x") {
        let Some(mask) = fields.strip_suffix(" private_swap=unsupported") else {
            return false;
        };
        return mask.len() == 1 && matches!(mask.as_bytes()[0], b'0'..=b'9' | b'a'..=b'f');
    }
    if let Some(fields) = line.strip_prefix("md_sim_zynq_cpu_control cpu=") {
        let Some((cpu, fields)) = fields.split_once(" reset=") else {
            return false;
        };
        let Some((reset, clock)) = fields.split_once(" clock_stop=") else {
            return false;
        };
        return matches!(cpu, "0" | "1")
            && matches!(reset, "0" | "1")
            && matches!(clock, "0" | "1");
    }
    false
}
pub fn verify_diagnostics(text: &str, reject: bool) -> bool {
    if !text.is_empty() && !text.ends_with('\n') {
        return false;
    }
    let mut actual = Vec::new();
    for line in text.lines() {
        if line.starts_with("md_sim_zynq_debug_reference ")
            || line.starts_with("qemu: md-sim: unsupported ROM SLCR ")
        {
            actual.push(line.to_owned());
        } else if !metadata_line(line) {
            return false;
        }
    }
    actual == expected_diagnostics(reject)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_oracle_rejects_changed_missing_extra_and_partial_records() {
        for reject in [false, true] {
            let expected = expected_diagnostics(reject).join("\n") + "\n";
            assert!(verify_diagnostics(&expected, reject));
            assert!(!verify_diagnostics(expected.trim_end(), reject));
            assert!(!verify_diagnostics(
                &(expected.clone() + "unexpected warning\n"),
                reject
            ));
            assert!(!verify_diagnostics(&(expected.clone() + &expected), reject));
            assert!(!verify_diagnostics(
                &expected.replacen("value=0x", "value=0xf", 1),
                reject
            ));
            assert!(!verify_diagnostics(&expected, !reject));
            let mut lines: Vec<_> = expected.lines().collect();
            lines.swap(0, 1);
            assert!(!verify_diagnostics(&(lines.join("\n") + "\n"), reject));
        }
        for line in [
            "md_sim_zynq_fclk unexpected",
            "md_sim_zynq_fclk channel=4 period=10",
            "md_sim_zynq_rom_map ram_hi=0xF private_swap=unsupported",
            "md_sim_zynq_cpu_control cpu=0 reset=0 clock_stop=2",
        ] {
            assert!(!metadata_line(line));
        }
        for line in [
            "md_sim_zynq_fclk channel=3 period=18446744073709551615",
            "md_sim_zynq_freset channel=0 high=0",
            "md_sim_zynq_rom_map ram_hi=0xf private_swap=unsupported",
            "md_sim_zynq_cpu_control cpu=1 reset=1 clock_stop=0",
        ] {
            assert!(metadata_line(line));
        }
        assert_eq!(transaction_count(false), 33);
        assert_eq!(transaction_count(true), 20);
    }
}

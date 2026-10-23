//! Native watchdog fault input and an independent observation check.
//! This does not import or call the watchdog model.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

pub const CONSOLE: &str = r#"(
set -eu
attempts=0
until logread | grep -F 'procd: - init complete -'; do
    attempts=$((attempts + 1))
    [ "$attempts" -le 120 ]
    sleep 1
done
command -v devmem
ubus call system watchdog > /tmp/md-sim-watchdog-original
grep -F '"timeout": 30' /tmp/md-sim-watchdog-original
grep -F '"frequency": 5' /tmp/md-sim-watchdog-original
grep -F '"status": "running"' /tmp/md-sim-watchdog-original
restore_watchdog() {
    # The native service clamps frequency against its current timeout.
    # Restore the timeout before restoring the five-second feed interval.
    ubus call system watchdog '{"timeout":30,"stop":false}' > /tmp/md-sim-watchdog-timeout-restored
    ubus call system watchdog '{"frequency":5,"stop":false}'
}
trap restore_watchdog EXIT
ubus call system watchdog '{"timeout":2,"frequency":1,"stop":false}' > /tmp/md-sim-watchdog-fast
grep -F '"timeout": 2' /tmp/md-sim-watchdog-fast
grep -F '"frequency": 1' /tmp/md-sim-watchdog-fast
grep -F '"status": "running"' /tmp/md-sim-watchdog-fast
sleep 4
printf '\nMD_SIM_WATCHDOG_HEALTHY\n'
devmem 0xf800500c 32 > /tmp/md-sim-watchdog-status
cat /tmp/md-sim-watchdog-status
grep -Fx '0x00000000' /tmp/md-sim-watchdog-status
ubus call system watchdog '{"stop":true}' > /tmp/md-sim-watchdog-stopped
cat /tmp/md-sim-watchdog-stopped
grep -F '"status": "stopped"' /tmp/md-sim-watchdog-stopped
sleep 4
printf '\nMD_SIM_WATCHDOG_EXPIRED\n'
devmem 0xf800500c 32 > /tmp/md-sim-watchdog-status
cat /tmp/md-sim-watchdog-status
grep -Fx '0x00000001' /tmp/md-sim-watchdog-status
dmesg | grep -F 'Watchdog timed out.'
ubus call system watchdog '{"stop":false}' > /tmp/md-sim-watchdog-resumed
grep -F '"status": "running"' /tmp/md-sim-watchdog-resumed
sleep 4
printf '\nMD_SIM_WATCHDOG_RECOVERED\n'
devmem 0xf800500c 32 > /tmp/md-sim-watchdog-status
cat /tmp/md-sim-watchdog-status
grep -Fx '0x00000000' /tmp/md-sim-watchdog-status
restore_watchdog > /tmp/md-sim-watchdog-restored
grep -F '"timeout": 30' /tmp/md-sim-watchdog-restored
grep -F '"frequency": 5' /tmp/md-sim-watchdog-restored
grep -F '"status": "running"' /tmp/md-sim-watchdog-restored
trap - EXIT
printf '\nMD_SIM_WATCHDOG_RESTORED\n'
)
"#;

#[derive(Clone, Copy, Debug)]
enum Operation {
    Read(u32, u32),
    Write(u32, u32),
    Zero(u32),
    Low,
}
#[derive(Clone, Copy, Debug)]
struct Event {
    ns: u64,
    op: Operation,
}

fn field<'a>(line: &'a str, key: &str) -> Result<&'a str> {
    let mut fields = line
        .split_whitespace()
        .filter_map(|part| part.strip_prefix(key));
    let value = fields.next().context("watchdog trace field absent")?;
    ensure!(fields.next().is_none(), "duplicate watchdog trace field");
    Ok(value)
}
fn hex_field(line: &str, key: &str) -> Result<u32> {
    Ok(u32::from_str_radix(
        field(line, key)?
            .strip_prefix("0x")
            .context("trace hex prefix")?,
        16,
    )?)
}
fn events(trace: &str) -> Result<Vec<Event>> {
    let mut out = Vec::new();
    for line in trace
        .lines()
        .filter(|line| line.starts_with("md_sim_swdt_"))
    {
        ensure!(out.len() < 100_000, "watchdog trace bound");
        let ns = field(line, "ns=")?.parse()?;
        let op = if line.starts_with("md_sim_swdt_mmio ") {
            ensure!(field(line, "error=")? == "0", "watchdog bus error");
            let address = hex_field(line, "addr=")?;
            let value = hex_field(line, "value=")?;
            match field(line, "write=")? {
                "0" => Operation::Read(address, value),
                "1" => Operation::Write(address, value),
                _ => anyhow::bail!("invalid watchdog direction"),
            }
        } else if line.starts_with("md_sim_swdt_event ") {
            match field(line, "event=")? {
                "zero" => Operation::Zero(hex_field(line, "value=")?),
                "irq-low" => {
                    ensure!(hex_field(line, "value=")? == 0, "IRQ low trace value");
                    Operation::Low
                }
                _ => anyhow::bail!("unknown watchdog event"),
            }
        } else {
            anyhow::bail!("unknown watchdog trace record");
        };
        ensure!(
            out.last().is_none_or(|last: &Event| ns >= last.ns),
            "watchdog time reversed"
        );
        out.push(Event { ns, op });
    }
    Ok(out)
}

fn feed_span(pings: &[&Event], end: u64, period: u64) -> Result<u64> {
    ensure!(pings.len() >= 3, "keep-alive coverage too short");
    let span = pings.last().unwrap().ns - pings[0].ns;
    ensure!(
        span > period,
        "feeding did not span one full hardware timeout"
    );
    ensure!(
        pings
            .windows(2)
            .all(|pair| pair[1].ns - pair[0].ns < period)
            && end - pings.last().unwrap().ns < period,
        "keep-alive gap exceeded the hardware timeout"
    );
    Ok(span)
}

pub fn assess(guest: &str, trace: &str) -> Result<Value> {
    let mut marker_end = 0;
    for marker in [
        "MD_SIM_WATCHDOG_HEALTHY",
        "MD_SIM_WATCHDOG_EXPIRED",
        "MD_SIM_WATCHDOG_RECOVERED",
        "MD_SIM_WATCHDOG_RESTORED",
    ] {
        // Require complete output lines; an echoed shell command is not evidence.
        let matches: Vec<_> = guest.match_indices(&format!("\n{marker}\r\n")).collect();
        ensure!(
            matches.len() == 1 && matches[0].0 >= marker_end,
            "watchdog output marker absent, repeated or reordered: {marker}"
        );
        marker_end = matches[0].0 + matches[0].1.len();
    }
    ensure!(
        guest
            .lines()
            .any(|line| line.contains("] cdns-wdt f8005000.watchdog: ")
                && line
                    .trim_end()
                    .ends_with("Watchdog timed out. Internal reset not enabled")),
        "actual kernel timeout message absent"
    );
    let events = events(trace)?;
    let statuses: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event.op {
            Operation::Read(12, value) => Some((index, value)),
            _ => None,
        })
        .collect();
    ensure!(
        statuses.len() == 3 && statuses.iter().map(|v| v.1).eq([0, 1, 0]),
        "watchdog status sequence differs"
    );
    let [healthy, expired, recovered] = [statuses[0].0, statuses[1].0, statuses[2].0];
    let timeout_config = events[..healthy]
        .iter()
        .rposition(|e| matches!(e.op, Operation::Write(4, 0x92003b)))
        .context("native two-second timeout configuration absent")?;
    ensure!(
        events[timeout_config..healthy]
            .iter()
            .any(|e| matches!(e.op, Operation::Write(0, 0xabc035))),
        "native IRQ-only watchdog mode absent"
    );
    let healthy_pings: Vec<_> = events[timeout_config..healthy]
        .iter()
        .filter(|e| matches!(e.op, Operation::Write(8, 0x1999)))
        .collect();
    ensure!(
        !events[timeout_config..healthy]
            .iter()
            .any(|e| matches!(e.op, Operation::Zero(_))),
        "watchdog expired while fed"
    );
    let expiries: Vec<_> = events[healthy..expired]
        .iter()
        .filter(|e| matches!(e.op, Operation::Zero(_)))
        .collect();
    ensure!(
        expiries.len() == 1 && matches!(expiries[0].op, Operation::Zero(4)),
        "one IRQ-only expiry required"
    );
    let zero = expiries[0].ns;
    let last_ping = events[..expired]
        .iter()
        .rev()
        .find(|e| e.ns <= zero && matches!(e.op, Operation::Write(8, 0x1999)))
        .context("expiry lacks preceding keep-alive")?
        .ns;
    let elapsed = zero.checked_sub(last_ping).context("expiry order")?;
    // Source driver rounds the hardware reload to4096-count steps. This
    // functional bound is not physical timing calibration.
    ensure!(
        (2_000_000_000..3_000_000_000).contains(&elapsed),
        "watchdog expired outside the programmed timeout bound"
    );
    let healthy_span = feed_span(&healthy_pings, events[healthy].ns, elapsed)?;
    let lows: Vec<_> = events[healthy..expired]
        .iter()
        .filter(|e| matches!(e.op, Operation::Low))
        .collect();
    ensure!(
        lows.len() == 1 && lows[0].ns > zero && lows[0].ns - zero <= 1000,
        "watchdog IRQ pulse did not end"
    );
    let resumed: Vec<_> = events[expired..recovered]
        .iter()
        .filter(|e| matches!(e.op, Operation::Write(8, 0x1999)))
        .collect();
    let recovery_span = feed_span(&resumed, events[recovered].ns, elapsed)?;
    ensure!(
        !events[expired..recovered]
            .iter()
            .any(|e| matches!(e.op, Operation::Zero(_))),
        "watchdog expired after feeding resumed"
    );
    ensure!(
        events[recovered..]
            .iter()
            .any(|e| matches!(e.op, Operation::Write(4, 0x92031f))),
        "original native timeout not restored"
    );
    Ok(
        json!({"scope":"Archived firmware watchdog fault and recovery on the reference model; not physical calibration or complete board acceptance.","trace_records":events.len(),"status_readbacks":[0,1,0],"healthy_keepalives":healthy_pings.len(),"recovery_keepalives":resumed.len(),"healthy_feed_span_ns":healthy_span,"recovery_feed_span_ns":recovery_span,"expiry_after_last_keepalive_ns":elapsed,"irq_pulse_ns":lows[0].ns-zero,"original_timeout_restored":true}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (String, String) {
        let guest = "\nMD_SIM_WATCHDOG_HEALTHY\r\n[ 42.0] cdns-wdt f8005000.watchdog: Watchdog timed out. Internal reset not enabled\n\nMD_SIM_WATCHDOG_EXPIRED\r\n\nMD_SIM_WATCHDOG_RECOVERED\r\n\nMD_SIM_WATCHDOG_RESTORED\r\n".into();
        let mut trace = String::new();
        for (ns, write, address, value) in [
            (0_u64, 1, 4, 0x92003b),
            (1, 1, 0, 0xabc035),
            (2, 1, 8, 0x1999),
            (1_000_000_002, 1, 8, 0x1999),
            (2_000_000_002, 1, 8, 0x1999),
            (3_000_000_002, 1, 8, 0x1999),
            (3_100_000_000, 0, 12, 0),
        ] {
            trace += &format!("md_sim_swdt_mmio ns={ns} write={write} addr=0x{address:x} value=0x{value:x} error=0\n");
        }
        trace += "md_sim_swdt_event ns=5264887300 event=zero value=0x4\nmd_sim_swdt_event ns=5264887336 event=irq-low value=0x0\n";
        for (ns, write, address, value) in [
            (6_000_000_000_u64, 0, 12, 1),
            (7_000_000_000, 1, 8, 0x1999),
            (8_000_000_000, 1, 8, 0x1999),
            (9_000_000_000, 1, 8, 0x1999),
            (10_000_000_000, 1, 8, 0x1999),
            (10_100_000_000, 0, 12, 0),
            (10_200_000_000, 1, 4, 0x92031f),
        ] {
            trace += &format!("md_sim_swdt_mmio ns={ns} write={write} addr=0x{address:x} value=0x{value:x} error=0\n");
        }
        (guest, trace)
    }
    #[test]
    fn rejects_missing_fault_recovery_and_invalid_traces() {
        let (guest, trace) = fixture();
        assess(&guest, &trace).unwrap();
        for broken in [
            trace.replace("value=0x1 error", "value=0x0 error"),
            trace.replace("value=0x4\n", "value=0x100\n"),
            trace.replace("event=irq-low", "event=unknown"),
            trace.replace("error=0", "error=1"),
            trace.replace("value=0x92031f", "value=0x92003b"),
            trace.replace("ns=7000000000", "ns=1"),
            trace.replace("ns=5264887300", "ns=3264887300"),
            trace
                .lines()
                .filter(|line| !line.contains("ns=8000000000 ") && !line.contains("ns=9000000000 "))
                .collect::<Vec<_>>()
                .join("\n"),
        ] {
            assert!(assess(&guest, &broken).is_err());
        }
        assert!(assess(
            &guest.replace("Watchdog timed out.", "unrelated log"),
            &trace
        )
        .is_err());
        assert!(assess(&format!("{guest}\nMD_SIM_WATCHDOG_HEALTHY\r\n"), &trace).is_err());
        assert!(assess(
            &guest.replace(
                "\nMD_SIM_WATCHDOG_EXPIRED\r\n",
                "printf MD_SIM_WATCHDOG_EXPIRED"
            ),
            &trace
        )
        .is_err());
    }
}

//! Actual private QMP reset and bounded UART trace checks. No firmware runs.
use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::net::UnixStream,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub const RESET: &str = "@qmp-system-reset";
const QMP_PREFIX: &str = "MD_SIM_S3_UART_QMP ";

struct Backend(Child);
impl Drop for Backend {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

fn line(input: &mut impl BufRead) -> Result<String> {
    let mut bytes = Vec::new();
    input.take(16_385).read_until(b'\n', &mut bytes)?;
    ensure!(
        bytes.len() <= 16_384 && bytes.last() == Some(&b'\n'),
        "QMP line absent or oversized"
    );
    bytes.pop(); // Remove only the required newline; preserve observed payload.
    Ok(String::from_utf8(bytes)?)
}

struct Qmp {
    input: BufReader<UnixStream>,
    output: UnixStream,
    next_id: u32,
    reset_events: usize,
}
impl Qmp {
    fn connect() -> Result<Self> {
        let deadline = Instant::now() + Duration::from_secs(2);
        let stream = loop {
            match UnixStream::connect("/work/uart-qmp") {
                Ok(stream) => break stream,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                    ) && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(error) => return Err(error.into()),
            }
        };
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        stream.set_write_timeout(Some(Duration::from_secs(2)))?;
        let mut qmp = Self {
            input: BufReader::new(stream.try_clone()?),
            output: stream,
            next_id: 1,
            reset_events: 0,
        };
        let greeting: Value = serde_json::from_str(&line(&mut qmp.input)?)?;
        eprintln!("{QMP_PREFIX}{greeting}");
        ensure!(greeting["QMP"].is_object(), "QMP greeting absent");
        qmp.request("qmp_capabilities")?;
        Ok(qmp)
    }
    fn request(&mut self, command: &str) -> Result<()> {
        let id = self.next_id;
        self.next_id += 1;
        serde_json::to_writer(&mut self.output, &json!({"execute":command,"id":id}))?;
        writeln!(self.output)?;
        self.output.flush()?;
        for _ in 0..16 {
            let value: Value = serde_json::from_str(&line(&mut self.input)?)?;
            eprintln!("{QMP_PREFIX}{value}");
            if value["event"].is_string() {
                if value["event"] == "RESET" {
                    self.reset_events += 1;
                }
                continue;
            }
            ensure!(
                value["id"] == id && value.get("return").is_some(),
                "QMP command failed: {value}"
            );
            return Ok(());
        }
        bail!("QMP response bound")
    }
    fn reset(&mut self) -> Result<()> {
        let before = self.reset_events;
        self.request("system_reset")?;
        // As in the existing TTC reset suite, this round trip drains the
        // actual asynchronous RESET event before any post-reset MMIO read.
        self.request("query-status")?;
        ensure!(
            self.reset_events == before + 1,
            "actual RESET event absent or repeated"
        );
        Ok(())
    }
}

pub fn run(command: &mut Command, script: &str) -> Result<()> {
    ensure!(!fs::exists("/work/uart-qmp")?, "QMP path already exists");
    command.args([
        "-qmp",
        "unix:/work/uart-qmp,server=on,wait=off",
        "-trace",
        "enable=esp32s3_uart_config_*",
    ]);
    let mut backend = Backend(
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?,
    );
    let mut input = BufReader::new(backend.0.stdout.take().context("qtest stdout absent")?);
    let mut output = backend.0.stdin.take().context("qtest stdin absent")?;
    let mut qmp = Qmp::connect()?;
    for instruction in script.lines() {
        if instruction == RESET {
            qmp.reset()?;
            continue;
        }
        writeln!(output, "{instruction}")?;
        output.flush()?;
        let actual = line(&mut input)?;
        ensure!(actual.len() <= 1024, "qtest response bound");
        println!("{actual}");
    }
    ensure!(qmp.reset_events == 2, "two actual resets required");
    qmp.request("quit")?;
    ensure!(backend.0.wait()?.success(), "QEMU reset/trace test failed");
    let mut extra = Vec::new();
    input.take(1025).read_to_end(&mut extra)?;
    ensure!(extra.is_empty(), "unexpected extra qtest response");
    Ok(())
}

fn reset_reads(values: &mut Vec<(String, String)>) {
    for base in [0x6000_0000u32, 0x6001_0000] {
        for (offset, expected) in [(0x14, 0x2b6u32), (0x78, 0x03701000), (0x7c, 0x02008270)] {
            values.push((
                format!("readl {:#x}", base + offset),
                format!("OK 0x{expected:016x}"),
            ));
        }
    }
}

pub fn transactions() -> Vec<(String, String)> {
    let mut values = vec![(RESET.into(), String::new())];
    reset_reads(&mut values);
    // The pinned qtest clock advances only while a virtual timer is pending.
    // Program the same real absent-slave I2C transaction as the baseline:
    // START plus nine address periods completes100000ns later. Do not replace
    // the observed UART time with an expected value when the clock is stopped.
    values.extend([
        ("writel 0x60013004 0x113".into(), "OK".into()),
        ("writel 0x60013000 49".into(), "OK".into()),
        ("writel 0x60013038 0x3018".into(), "OK".into()),
        ("writel 0x60013040 49".into(), "OK".into()),
        ("writel 0x60013044 49".into(), "OK".into()),
        ("writel 0x60013054 0x200003".into(), "OK".into()),
        ("writel 0x60013004 0x913".into(), "OK".into()),
        ("writel 0x60013028 0x400".into(), "OK".into()),
        ("writel 0x60013058 0x3000".into(), "OK".into()),
        ("writel 0x6001305c 0x901".into(), "OK".into()),
        ("writel 0x60013060 0x1000".into(), "OK".into()),
        ("writel 0x6001301c 0x78".into(), "OK".into()),
        ("writel 0x60013004 0x133".into(), "OK".into()),
        ("readl 0x6001302c".into(), "OK 0x0000000000000000".into()),
        ("clock_step 1".into(), "OK 100001".into()),
        ("readl 0x6001302c".into(), "OK 0x0000000000000000".into()),
        ("writel 0x60010014 1".into(), "OK".into()),
        ("writel 0x60000078 0x03500000".into(), "OK".into()),
    ]);
    // Five UART1 writes preceded the reset. Its next write is sequence5.
    // These250 writes fill the lifetime budget; reset must not renew it.
    values.extend((0..250).map(|_| ("writel 0x60010014 0x2b6".into(), "OK".into())));
    values.extend([
        ("writel 0x60010014 0x123".into(), "OK".into()),
        ("readl 0x60010014".into(), "OK 0x0000000000000123".into()),
        ("clock_step 99999".into(), "OK 200000".into()),
        ("readl 0x6001302c".into(), "OK 0x0000000000000400".into()),
        (RESET.into(), String::new()),
    ]);
    reset_reads(&mut values);
    values.extend([
        ("writel 0x60010014 0x456".into(), "OK".into()),
        ("readl 0x60010014".into(), "OK 0x0000000000000456".into()),
    ]);
    values
}

fn expected_trace() -> Vec<String> {
    let mut lines = Vec::new();
    for (sequence, (address, value, stored)) in [
        (0x78, 0x03500000u64, 0x03500000),
        (0x78, 0xf3501000, 0x03501000),
        (0x14, 0xffffffff, 0x00f00fff),
        (0x14, 0x000ff000, 0),
        (0x14, 0xffa00abc, 0x00a00abc),
    ]
    .into_iter()
    .enumerate()
    {
        lines.push(format!("esp32s3_uart_config_write device=/machine/soc/uart1 virtual_ns=100000 sequence={sequence} address=0x{address:x} size=4 value=0x{value:x} stored=0x{stored:x}"));
    }
    lines.push("esp32s3_uart_config_write device=/machine/soc/uart1 virtual_ns=100001 sequence=5 address=0x14 size=4 value=0x1 stored=0x1".into());
    lines.push("esp32s3_uart_config_write device=/machine/soc/uart0 virtual_ns=100001 sequence=0 address=0x78 size=4 value=0x3500000 stored=0x3500000".into());
    for sequence in 6..256 {
        lines.push(format!("esp32s3_uart_config_write device=/machine/soc/uart1 virtual_ns=100001 sequence={sequence} address=0x14 size=4 value=0x2b6 stored=0x2b6"));
    }
    lines.push(
        "esp32s3_uart_config_overflow device=/machine/soc/uart1 virtual_ns=100001 limit=256".into(),
    );
    lines
}

pub fn verify(trace: &str) -> Result<Value> {
    let actual: Vec<_> = trace
        .lines()
        .filter(|line| line.starts_with("esp32s3_uart_config_"))
        .collect();
    ensure!(
        actual == expected_trace(),
        "UART write identity/time/value/sequence or trace cap differs"
    );
    let qmp: Vec<Value> = trace
        .lines()
        .filter_map(|line| line.strip_prefix(QMP_PREFIX))
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    ensure!(
        qmp.iter().filter(|value| value["event"] == "RESET").count() == 2,
        "retained actual reset events absent"
    );
    let replies: Vec<_> = qmp
        .iter()
        .filter_map(|value| value["id"].as_u64())
        .collect();
    ensure!(
        replies == [1, 2, 3, 4, 5, 6],
        "retained QMP replies absent or out of order"
    );
    ensure!(
        qmp.iter()
            .all(|value| value.get("id").is_none() || value.get("return").is_some()),
        "retained QMP error"
    );
    Ok(
        json!({"trace_contract_passed":true,"actual_qmp_resets":2,"uart1_write_events":256,"uart0_write_events":1,"intentional_overflow_events":1,"complete_firmware_capture":false,"scope":"Negative trace-budget regression. An injected overflow is observed and does not renew across reset. This is not complete firmware capture or physical baud evidence."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> String {
        let mut trace = expected_trace().join("\n");
        for id in 1..=6 {
            trace.push_str(&format!("\n{QMP_PREFIX}{}", json!({"id":id,"return":{}})));
        }
        for _ in 0..2 {
            trace.push_str(&format!("\n{QMP_PREFIX}{}", json!({"event":"RESET"})));
        }
        trace
    }
    #[test]
    fn trace_checker_rejects_missing_overflow_and_reset_budget_renewal() {
        let fixture = fixture();
        assert!(verify(&fixture).is_ok());
        assert!(
            verify(&fixture.replace("esp32s3_uart_config_overflow", "missing_overflow")).is_err()
        );
        assert!(verify(&(fixture+"\nesp32s3_uart_config_write device=/machine/soc/uart1 virtual_ns=100001 sequence=0 address=0x14 size=4 value=0x456 stored=0x456")).is_err());
    }
    #[test]
    fn trace_checker_rejects_wrong_device_clock_value_and_missing_reset() {
        let fixture = fixture();
        for (from, to) in [
            ("uart0", "uart2"),
            ("virtual_ns=100001", "virtual_ns=100002"),
            ("stored=0xa00abc", "stored=0xffa00abc"),
            ("\"RESET\"", "\"STOP\""),
        ] {
            assert!(verify(&fixture.replace(from, to)).is_err(), "{from}");
        }
    }
}

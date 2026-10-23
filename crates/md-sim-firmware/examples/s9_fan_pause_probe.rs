//! Diagnose late fan timer service after a stopped host emulator process.
//! Uses project-owned CPU regression code. This is never firmware evidence.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};
#[path = "../src/confinement.rs"]
#[allow(dead_code)] // Shared guard also exposes preparation and VM profiles.
mod confinement;
const FIXTURE: &str = "work/simulator/qemu-s9/cpu-regression-prep-005/test.bin";
const FIXTURE_SHA: &str = "c1f18e0510661f2ed6c0679233b74a48d7641fd67cbfa99cc27c15952e7433de";
struct Owned(Child);
impl Drop for Owned {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
fn bounded(path: &str) -> Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 1024 * 1024, "clock probe output bound");
    Ok(String::from_utf8(bytes)?)
}
fn query(input: &mut impl Write, output: &mut impl BufRead, command: &str) -> Result<String> {
    writeln!(input, "{command}")?;
    input.flush()?;
    let mut line = String::new();
    output.take(4097).read_line(&mut line)?;
    ensure!(line.ends_with('\n') && line.len() <= 4096, "qtest framing");
    Ok(line.trim_end().to_owned())
}
fn inside(seconds: u64) -> Result<()> {
    confinement::require_board()?;
    ensure!(
        matches!(seconds, 0 | 65),
        "only baseline or65second scheduler pause"
    );
    let args = [
        "-machine",
        "md-sim-s9-fan-development",
        "-accel",
        "tcg,thread=single",
        "-m",
        "128M",
        "-smp",
        "2",
        "-nodefaults",
        "-nic",
        "none",
        "-display",
        "none",
        "-monitor",
        "none",
        "-serial",
        "null",
        "-serial",
        "file:/work/guest.log",
        "-device",
        "loader,file=/test.bin,addr=0,cpu-num=0,force-raw=on",
        "-qtest",
        "stdio",
        "-global",
        "md-sim-fan.reference-motors=on",
        "-trace",
        "enable=md_sim_fan_*",
    ];
    println!(
        "MD_SIM_FAN_PAUSE_INPUT {}",
        json!({"argv":args,"pause_seconds":seconds})
    );
    let mut child = Owned(
        Command::new("/qemu")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(fs::File::create("/work/backend.log")?)
            .spawn()?,
    );
    let mut input = child.0.stdin.take().context("qtest input absent")?;
    let mut output = BufReader::new(child.0.stdout.take().context("qtest output absent")?);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if Path::new("/work/guest.log").exists() && bounded("/work/guest.log")? == "ABCDERP\n" {
            break;
        }
        ensure!(
            child.0.try_wait()?.is_none() && Instant::now() < deadline,
            "project-owned CPU fixture did not complete"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut transactions = Vec::new();
    for command in ["writel 0x42800010 100", "writel 0x42800014 100"] {
        let response = query(&mut input, &mut output, command)?;
        ensure!(response == "OK", "initial fan register write failed");
        transactions.push(json!({"command":command,"response":response}));
    }
    std::thread::sleep(Duration::from_secs(2));
    let before = query(&mut input, &mut output, "readl 0x42800004")?;
    ensure!(
        before == "OK 0x0000000000000064",
        "steady reference fan did not reach100RPS: {before}"
    );
    let began = Instant::now();
    if seconds != 0 {
        ensure!(
            unsafe { libc::kill(child.0.id() as i32, libc::SIGSTOP) } == 0,
            "stop owned emulator failed"
        );
        let mut status = 0;
        ensure!(
            unsafe { libc::waitpid(child.0.id() as i32, &mut status, libc::WUNTRACED) }
                == child.0.id() as i32
                && libc::WIFSTOPPED(status),
            "owned emulator did not stop"
        );
        std::thread::sleep(Duration::from_secs(seconds));
        ensure!(
            unsafe { libc::kill(child.0.id() as i32, libc::SIGCONT) } == 0,
            "resume owned emulator failed"
        );
    }
    let host_pause_ms = began.elapsed().as_millis();
    std::thread::sleep(Duration::from_secs(2));
    let after = query(&mut input, &mut output, "readl 0x42800004")?;
    drop(child);
    println!(
        "MD_SIM_FAN_PAUSE_RESULT {}",
        json!({"before":before,"after":after,"transactions":transactions,
        "requested_host_pause_seconds":seconds,"observed_host_pause_ms":host_pause_ms,
        "guest":bounded("/work/guest.log")?,"backend":bounded("/work/backend.log")?,
        "scope":"Project-owned CPU test and actual fan MMIO after a host scheduler pause. No archived firmware, mining, calibration or exact-target gate. A reproduced backend error remains a failure."})
    );
    Ok(())
}

fn assess(observation: &Value, seconds: u64) -> Result<()> {
    ensure!(
        observation["requested_host_pause_seconds"] == seconds
            && observation["observed_host_pause_ms"]
                .as_u64()
                .is_some_and(|elapsed| elapsed >= seconds * 1000),
        "requested scheduler pause was not observed"
    );
    ensure!(
        observation["guest"] == "ABCDERP\n"
            && observation["before"] == "OK 0x0000000000000064"
            && observation["after"] == "OK 0x0000000000000064",
        "CPU execution or steady fan readback failed"
    );
    let backend = observation["backend"]
        .as_str()
        .context("backend log absent")?;
    ensure!(
        backend.lines().all(|line| !line.contains("qemu: ")
            || matches!(
                line,
                // This CPU/fan fixture deliberately has no Ethernet peer.
                "qemu: warning: nic cadence_gem.0 has no peer"
                    | "qemu: warning: nic cadence_gem.1 has no peer"
            ))
            && !backend.contains("error=1"),
        "backend reported a fan or emulator error"
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "__inside") {
        return inside(
            args.get(2)
                .context("pause absent")?
                .to_str()
                .context("pause encoding")?
                .parse()?,
        );
    }
    ensure!(
        args.len() == 4,
        "usage: s9_fan_pause_probe BACKEND NEW_OUTPUT 0|65"
    );
    let seconds: u64 = args[3].to_str().context("pause encoding")?.parse()?;
    ensure!(matches!(seconds, 0 | 65), "pause bound");
    let backend = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    ensure!(!out.exists(), "new output required");
    let report: Value = serde_json::from_slice(&fs::read(backend.join("build-report.json"))?)?;
    let binary = backend.join("qemu-system-arm");
    ensure!(
        report["binary"] == serde_json::to_value(hash_file(&binary)?)?
            && report["build"]["code"] == 0,
        "backend identity differs"
    );
    ensure!(
        hash_file(Path::new(FIXTURE))?.sha256 == FIXTURE_SHA,
        "project-owned CPU fixture differs"
    );
    fs::create_dir_all(out)?;
    let store = Store::open(&out.join("inputs"), 256 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/probe".into(), std::env::current_exe()?),
            ("/qemu".into(), binary),
        ],
    )?;
    let object = hash_file(Path::new(FIXTURE))?;
    store.import(Path::new(FIXTURE), &object)?;
    inputs.push(Mount {
        guest_path: "/test.bin".into(),
        object,
    });
    let spec = ProcessSpec {
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec!["/probe".into(), "__inside".into(), seconds.to_string()],
        inputs,
        environment: BTreeMap::new(),
        stdin_object: None,
        limits: Limits {
            wall_seconds: 95,
            cpu_seconds: 30,
            memory_bytes: 3 * 1024 * 1024 * 1024,
            processes: 32,
            file_bytes: 8 * 1024 * 1024,
            trace_bytes: 4 * 1024 * 1024,
            tmp_bytes: 16 * 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    fs::write(out.join("stdout.log"), hex::decode(&result.stdout)?)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated,
        "clock diagnostic failed; see raw evidence"
    );
    let text = String::from_utf8(hex::decode(&result.stdout)?)?;
    let records: Vec<Value> = text
        .lines()
        .filter_map(|line| line.strip_prefix("MD_SIM_FAN_PAUSE_RESULT "))
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    ensure!(records.len() == 1, "clock observation absent");
    save_json(
        &out.join("observation.json"),
        &json!({"observation":records[0],"process_spec":hash_file(&out.join("spec.json"))?,"execution_result":hash_file(&out.join("result.json"))?,
        "scope":"Diagnostic observation, not a conformance pass."}),
    )?;
    let checked = assess(&records[0], seconds);
    save_json(
        &out.join("assessment.json"),
        &json!({"passed":checked.is_ok(),"error":checked.as_ref().err().map(|e|format!("{e:#}")),
            "observation":hash_file(&out.join("observation.json"))?,
            "scope":"Late fan callback regression only. No firmware or complete target pass."}),
    )?;
    checked
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completed_probe_does_not_hide_failed_device_or_missing_pause() {
        let mut observation = json!({"requested_host_pause_seconds":65,
            "observed_host_pause_ms":65000,"guest":"ABCDERP\n",
            "before":"OK 0x0000000000000064","after":"OK 0x0000000000000000",
            "backend":"qemu: Fan advance rejected"});
        assert!(assess(&observation, 65).is_err());
        observation["after"] = observation["before"].clone();
        assert!(assess(&observation, 65).is_err());
        observation["backend"] = json!("");
        assess(&observation, 65).unwrap();
        observation["backend"] = json!("qemu: warning: nic cadence_gem.0 has no peer\nqemu: warning: nic cadence_gem.1 has no peer\n");
        assess(&observation, 65).unwrap();
        observation["backend"] = json!("qemu: warning: unexpected clock condition");
        assert!(assess(&observation, 65).is_err());
        observation["backend"] = json!("");
        observation["observed_host_pause_ms"] = json!(100);
        assert!(assess(&observation, 65).is_err());
    }
}

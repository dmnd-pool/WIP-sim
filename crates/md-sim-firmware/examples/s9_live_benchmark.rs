//! Measure the existing selected HDL program. This is not physical calibration.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};
#[allow(dead_code)]
#[path = "support/s9_axi_vectors.rs"]
mod axi;
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
// Different callers use either strict PL transactions or raw QEMU requests.
#[allow(dead_code)]
#[path = "support/framed_child.rs"]
mod framed_child;
#[path = "support/s9_live_child.rs"]
mod live_child;
#[path = "support/s9_live_protocol.rs"]
mod live_protocol;
#[allow(dead_code)]
#[path = "support/qemu_build.rs"]
mod qemu_build;

const PROGRAM: (&str, u64) = (
    "f74b392bab34563977af775fa02eeebb4dd60d67efd71c031d6a1788d43a3705",
    212950005,
);
const PLUGIN: (&str, u64) = (
    "c2515601182797cf5a9b5ac096db7541b41e140f0f88e11e796d15b3bdc032ad",
    24464,
);
const ARCHIVE: (&str, u64) = (
    "ce125b6729eb056bb0b537da036ebae2a2dc1c545e742992b11e8cb18f54890c",
    25512922,
);
const TOOLS: (&str, u64) = (
    "3c5631a6d6285affa39d9fddb9acca3758988b9c2fe6cbc4e3ce227f5fc33f61",
    3828781,
);
fn id(pair: (&str, u64)) -> ObjectId {
    ObjectId {
        sha256: pair.0.into(),
        size: pair.1,
    }
}

fn restore_program() -> Result<()> {
    ensure!(
        hash_file(Path::new("/model.tar.gz"))? == id(ARCHIVE),
        "model archive differs"
    );
    let mut tar = tar::Archive::new(
        flate2::read::GzDecoder::new(fs::File::open("/model.tar.gz")?).take(300 << 20),
    );
    fs::create_dir("/work/compiled")?;
    let mut found = [false; 2];
    for (count, entry) in tar.entries()?.enumerate() {
        ensure!(count < 32, "model entry bound");
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let target = match path.to_str() {
            Some("compiled/live.vvp") => Some((0, "/work/compiled/live.vvp", id(PROGRAM))),
            Some("compiled/mdsim_live.vpi") => {
                Some((1, "/work/compiled/mdsim_live.vpi", id(PLUGIN)))
            }
            _ => None,
        };
        if let Some((index, output, expected)) = target {
            ensure!(
                !found[index]
                    && entry.header().entry_type().is_file()
                    && entry.size() == expected.size,
                "duplicate or nonregular model member"
            );
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output)?;
            ensure!(
                std::io::copy(&mut entry, &mut file)? == expected.size,
                "short model member"
            );
            file.sync_all()?;
            ensure!(
                hash_file(Path::new(output))? == expected,
                "compiled model member differs"
            );
            found[index] = true;
        }
    }
    ensure!(found == [true, true], "compiled model members absent");
    Ok(())
}

fn command(
    child: &mut live_child::LiveChild,
    sequence: u64,
    at_ps: u64,
    operation: live_protocol::Op,
    args: [u64; 12],
    report: &mut Vec<Value>,
) -> Result<()> {
    let start = Instant::now();
    let result = child.transact(&live_protocol::Request {
        sequence,
        at_ps,
        operation,
        args,
    });
    let wall_ns = u64::try_from(start.elapsed().as_nanos()).context("host duration overflow")?;
    report.push(
        json!({"sequence":sequence,"requested_time_ps":at_ps,"operation":operation as u64,
        "host_wall_ns":wall_ns,"completed":result.is_ok()}),
    );
    let responses = result?;
    let completion = responses.last().context("benchmark completion absent")?;
    ensure!(
        completion.kind == live_protocol::Kind::Complete && completion.at_ps == at_ps,
        "benchmark completion changed its input timestamp"
    );
    // The phase starts at1ps and its half-period is exactly5000ps. This
    // checks the real clock thread advanced, including commands on an edge.
    let next_edge = ((at_ps - 1) / 5000 + 1)
        .checked_mul(5000)
        .and_then(|edge| edge.checked_add(1))
        .context("clock boundary overflow")?;
    ensure!(
        completion.args[6] == 1 && completion.args[5] == next_edge,
        "benchmark clock is gated or its next actual edge differs"
    );
    let row = report.last_mut().context("command timing record")?;
    row["actual_next_clock_ps"] = json!(completion.args[5]);
    row["clock_progress_checked"] = json!(true);
    Ok(())
}

fn run_case(interval_ns: u64) -> Result<Value> {
    let mut process = Command::new("/work/iverilog-install/bin/vvp");
    process.args([
        "-M/work/compiled",
        "-mmdsim_live",
        "/work/compiled/live.vvp",
    ]);
    let started = Instant::now();
    let mut child = live_child::LiveChild::launch(
        process,
        live_child::ChildLimits {
            command_timeout_ms: 10_000,
            shutdown_timeout_ms: 5_000,
            stdout_limit: 1 << 20,
            stderr_limit: 1 << 20,
            frame_limit: 16384,
        },
        Arc::new(AtomicBool::new(false)),
    )?;
    let mut commands = Vec::new();
    let mut startup_ns = None;
    let result = (|| -> Result<()> {
        child.ready()?;
        startup_ns = Some(u64::try_from(started.elapsed().as_nanos())?);
        let mut drive = [0; 12];
        drive[1] = 10 << 32;
        drive[8] = 2 | 16 | 32;
        command(
            &mut child,
            1,
            1,
            live_protocol::Op::Drive,
            drive,
            &mut commands,
        )?;
        command(
            &mut child,
            2,
            1_000_001,
            live_protocol::Op::Advance,
            [0; 12],
            &mut commands,
        )?;
        drive[1] = 0;
        drive[4] = 15;
        drive[5] = 1;
        drive[8] = 16 | 32;
        command(
            &mut child,
            3,
            1_000_002,
            live_protocol::Op::Drive,
            drive,
            &mut commands,
        )?;
        let end = 1_000_002u64
            .checked_add(interval_ns.checked_mul(1000).context("interval overflow")?)
            .context("target overflow")?;
        command(
            &mut child,
            4,
            end,
            live_protocol::Op::Advance,
            [0; 12],
            &mut commands,
        )?;
        command(
            &mut child,
            5,
            end + 1,
            live_protocol::Op::Stop,
            [0; 12],
            &mut commands,
        )?;
        Ok(())
    })();
    if let Err(error) = &result {
        let _ = child.abort(&format!("benchmark: {error:#}"));
    }
    let evidence = child.evidence();
    let expected_stdout = format!("{}\n", axi::expected_notice());
    let complete = result.is_ok()
        && evidence.failure.is_none()
        && evidence.cleanup_error.is_none()
        && !evidence.stdout_truncated
        && !evidence.stderr_truncated
        && !evidence.protocol_truncated
        && evidence.stderr_hex.is_empty()
        && evidence.stdout_hex == hex::encode(expected_stdout)
        && evidence.stdout_eof
        && evidence.stderr_eof
        && evidence.protocol_eof
        && evidence
            .exit
            .as_ref()
            .is_some_and(|e| e.code == Some(0) && e.signal.is_none());
    Ok(
        json!({"interval_ns":interval_ns,"startup_host_wall_ns":startup_ns,"commands":commands,
        "passed":complete,"error":result.err().map(|e|format!("{e:#}")),"evidence":evidence}),
    )
}

fn inside() -> Result<()> {
    confinement::require_netlist_preparation()?;
    ensure!(
        hash_file(Path::new("/tools.tar.gz"))? == id(TOOLS),
        "Icarus archive differs"
    );
    qemu_build::unpack("/tools.tar.gz", "/work")?;
    restore_program()?;
    let mut cases = Vec::new();
    for interval in [10_000, 100_000, 1_000_000] {
        let case = run_case(interval)?;
        eprintln!(
            "MD_SIM_HDL_BENCH interval_ns={interval} passed={}",
            case["passed"]
        );
        let pass = case["passed"] == true;
        cases.push(case);
        if !pass {
            break;
        }
    }
    let passed = cases.len() == 3 && cases.iter().all(|c| c["passed"] == true);
    let report = json!({"schema":"md-sim-selected-hdl-throughput/1","passed":passed,
        "program":id(PROGRAM),"plugin":id(PLUGIN),"cases":cases,
        "clock_hz":100000000,"clock_rescaled":false,"physical_calibration":false,
        "cpu_time_measured":false,
        "cpu_synchronization_complete":false,"firmware_executed":false,
        "measurement":"Monotonic host wall time, separately for child startup and each real PL command. No extrapolated boot time is asserted."});
    serde_json::to_writer(std::io::stdout(), &report)?;
    ensure!(passed, "bounded HDL benchmark incomplete");
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|s| s == "__inside") {
        return inside();
    }
    ensure!(args.len() == 3, "usage: s9_live_benchmark REPO NEW_OUTPUT");
    let repo = PathBuf::from(&args[1]).canonicalize()?;
    let out = PathBuf::from(&args[2]);
    ensure!(!out.exists(), "new benchmark output required");
    fs::create_dir_all(&out)?;
    let store = Store::open(&out.join("inputs"), 128 << 20)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/guard".into(), std::env::current_exe()?),
            ("/bin/sh".into(), PathBuf::from("/usr/bin/bash")),
            (
                "/usr/lib/libstdc++.so.6".into(),
                PathBuf::from("/usr/lib/libstdc++.so.6"),
            ),
            (
                "/usr/lib/libgcc_s.so.1".into(),
                PathBuf::from("/usr/lib/libgcc_s.so.1"),
            ),
            (
                "/usr/lib/libreadline.so.8".into(),
                PathBuf::from("/usr/lib/libreadline.so.8"),
            ),
            (
                "/usr/lib/libz.so.1".into(),
                PathBuf::from("/usr/lib/libz.so.1"),
            ),
            (
                "/usr/lib/libbz2.so.1.0".into(),
                PathBuf::from("/usr/lib/libbz2.so.1.0"),
            ),
        ],
    )?;
    let tools: Value = serde_json::from_slice(&fs::read(
        repo.join("simulator/firmware/s9-hdl-inputs-512.json"),
    )?)?;
    let tool_path = tools["tools"]["path"].as_str().context("tool path")?;
    for (path, guest, expected) in [
        (
            repo.join("work/simulator/qemu-s9/live-selected-512e/selected-vvp.tar.gz"),
            "/model.tar.gz",
            id(ARCHIVE),
        ),
        (repo.join(tool_path), "/tools.tar.gz", id(TOOLS)),
    ] {
        ensure!(hash_file(&path)? == expected, "benchmark input differs");
        store.import(&path, &expected)?;
        inputs.push(Mount {
            object: expected,
            guest_path: guest.into(),
        });
    }
    let spec = ProcessSpec {
        schema_version: "2.0".into(),
        argv: vec!["/guard".into(), "__inside".into()],
        inputs,
        stdin_object: None,
        environment: BTreeMap::from([
            ("PATH".into(), "/work/iverilog-install/bin".into()),
            ("LC_ALL".into(), "C".into()),
        ]),
        limits: Limits {
            wall_seconds: 900,
            cpu_seconds: 600,
            memory_bytes: 4 << 30,
            processes: 32,
            file_bytes: 1 << 30,
            trace_bytes: 96 << 20,
            tmp_bytes: 1 << 30,
        },
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    let raw = hex::decode(&result.stdout)?;
    fs::write(out.join("stdout.json"), &raw)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    let report: Value =
        serde_json::from_slice(&raw).context("benchmark report absent; raw result retained")?;
    save_json(&out.join("report.json"), &report)?;
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated && report["passed"] == true,
        "benchmark failed; retained actual evidence"
    );
    for input in &spec.inputs {
        store.verified_path(&input.object)?;
    }
    Ok(())
}

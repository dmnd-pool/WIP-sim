//! Real private NAND data+spare backing, restart persistence and boundary tests.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
};
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
const BYTES: usize = 131072 * 2112;
struct Guest(Child);
impl Drop for Guest {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn command(path: &str) -> Command {
    let mut c = Command::new("/qemu");
    c.args([
        "-machine",
        "md-sim-s9-nand-fixture",
        "-accel",
        "qtest",
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
        "null",
        "-qtest",
        "stdio",
        "-drive",
    ])
    .arg(format!(
        "file={path},if=none,id=md-nand,format=raw,cache=writethrough"
    ))
    .args(["-global", "md-sim-pl353.drive-id=md-nand"]);
    c
}
fn exchange(phase: &str, vectors: &[(String, String)]) -> Result<()> {
    let mut c = command("/work/nand.raw");
    c.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut guest = Guest(c.spawn()?);
    let mut input = guest.0.stdin.take().context("qtest stdin")?;
    let mut output = BufReader::new(guest.0.stdout.take().context("qtest stdout")?);
    for (request, expected) in vectors {
        writeln!(input, "{request}")?;
        input.flush()?;
        let mut line = Vec::new();
        output.by_ref().take(1025).read_until(b'\n', &mut line)?;
        ensure!(
            line.len() <= 1024 && line.last() == Some(&b'\n'),
            "bounded qtest response"
        );
        let actual = String::from_utf8(line)?;
        println!(
            "{}",
            json!({"phase":phase,"request":request,"expected":expected,"actual":actual.trim_end()})
        );
        ensure!(actual.trim_end() == expected, "qtest response differs");
    }
    drop(input);
    ensure!(
        unsafe { libc::kill(guest.0.id() as i32, libc::SIGTERM) } == 0,
        "stop qtest process"
    );
    ensure!(
        guest.0.wait()?.success(),
        "QEMU did not close backing cleanly"
    );
    Ok(())
}
fn op(v: &mut Vec<(String, String)>, request: impl Into<String>, response: impl Into<String>) {
    v.push((request.into(), response.into()));
}
fn wait(v: &mut Vec<(String, String)>, now: &mut u64, delta: u64) {
    *now += delta;
    op(v, format!("clock_step {delta}"), format!("OK {now}"));
}
fn address(v: &mut Vec<(String, String)>, command: u32, row: u32, column: u32) {
    op(
        v,
        format!("writel 0x{command:x} 0x{:x}", (row << 16) | column),
        "OK",
    );
    op(v, format!("writel 0x{command:x} 0x{:x}", row >> 16), "OK");
}
fn write_byte(v: &mut Vec<(String, String)>, now: &mut u64, row: u32, column: u32, value: u8) {
    address(v, 0xe1a08400, row, column);
    op(v, format!("writeb 0xe1388000 0x{value:x}"), "OK");
    wait(v, now, 300000);
}
fn read_byte(v: &mut Vec<(String, String)>, now: &mut u64, row: u32, column: u32, value: u8) {
    address(v, 0xe1b18000, row, column);
    wait(v, now, 25000);
    op(v, "readb 0xe1280000", format!("OK 0x{value:016x}"));
}
fn reset(v: &mut Vec<(String, String)>, now: &mut u64) {
    op(v, "writel 0xe10007f8 0", "OK");
    wait(v, now, 5000);
}
fn verify_storage(expected: &BTreeMap<usize, u8>) -> Result<serde_json::Value> {
    let mut file = fs::File::open("/work/nand.raw")?;
    ensure!(
        file.metadata()?.len() == BYTES as u64,
        "backing length changed"
    );
    let mut chunk = [0u8; 65536];
    let mut oracle = [0xffu8; 65536];
    let mut offset = 0;
    loop {
        let n = file.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        oracle.fill(0xff);
        for (&at, &value) in expected.range(offset..offset + n) {
            oracle[at - offset] = value;
        }
        ensure!(
            chunk[..n] == oracle[..n],
            "backing chunk differs at {offset}"
        );
        offset += n;
    }
    Ok(
        json!({"complete_backing":hash_file(Path::new("/work/nand.raw"))?,"checked_bytes":offset,"non_erased_bytes":expected}),
    )
}
fn rejected(path: &str, property: Option<&str>, text: &str) -> Result<serde_json::Value> {
    let mut c = command(path);
    if let Some(property) = property {
        c.args(["-global", property]);
    }
    c.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut guest = Guest(c.spawn()?);
    let mut bytes = Vec::new();
    guest
        .0
        .stderr
        .take()
        .context("negative stderr")?
        .take(65537)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 65536, "negative output budget");
    let status = guest.0.wait()?;
    let error = String::from_utf8_lossy(&bytes);
    ensure!(
        !status.success() && error.contains(text),
        "expected explicit model rejection: {error}"
    );
    Ok(json!({"status":status.code(),"stderr":error,"property":property}))
}
fn inside() -> Result<()> {
    confinement::require_nand_storage()?;
    let mut file = fs::File::create("/work/nand.raw")?;
    let chunk = [0xffu8; 65536];
    for _ in 0..BYTES / chunk.len() {
        file.write_all(&chunk)?;
    }
    file.sync_all()?;
    drop(file);
    let mut vectors = Vec::new();
    let mut now = 0;
    reset(&mut vectors, &mut now);
    for (column, byte) in [(2048, 0xa5), (2050, 0x5a), (2111, 0x3c)] {
        write_byte(&mut vectors, &mut now, 0, column, byte);
    }
    address(&mut vectors, 0xe1a08400, 0, 2047);
    // A second program cannot restore zero bits: 0xa5 & 0xdb == 0x81.
    op(&mut vectors, "writew 0xe1388000 0xdb12", "OK");
    wait(&mut vectors, &mut now, 300000);
    write_byte(&mut vectors, &mut now, 131071, 2111, 0x66);
    // Two bytes at the last spare byte exceed this page and the backing file.
    // The chip must report failure without writing either byte.
    address(&mut vectors, 0xe1a08400, 131071, 2111);
    op(&mut vectors, "writew 0xe1388000 0", "OK");
    wait(&mut vectors, &mut now, 300000);
    op(&mut vectors, "writel 0xe1000380 0", "OK");
    op(&mut vectors, "readb 0xe1080000", "OK 0x00000000000000c1");
    for (column, byte) in [
        (0, 0xff),
        (2047, 0x12),
        (2048, 0x81),
        (2049, 0xff),
        (2050, 0x5a),
        (2111, 0x3c),
    ] {
        read_byte(&mut vectors, &mut now, 0, column, byte);
    }
    read_byte(&mut vectors, &mut now, 131071, 2111, 0x66);
    exchange("program-and-read", &vectors)?;
    let expected = BTreeMap::from([
        (2047, 0x12),
        (2048, 0x81),
        (2050, 0x5a),
        (2111, 0x3c),
        (BYTES - 1, 0x66),
    ]);
    let first = verify_storage(&expected)?;
    vectors.clear();
    now = 0;
    reset(&mut vectors, &mut now);
    for (&offset, &byte) in &expected {
        read_byte(
            &mut vectors,
            &mut now,
            (offset / 2112) as u32,
            (offset % 2112) as u32,
            byte,
        );
    }
    op(&mut vectors, "writel 0xe1768300 0", "OK");
    wait(&mut vectors, &mut now, 2000000);
    read_byte(&mut vectors, &mut now, 0, 2048, 0xff);
    read_byte(&mut vectors, &mut now, 131071, 2111, 0x66);
    exchange("restart-and-erase", &vectors)?;
    let second = verify_storage(&BTreeMap::from([(BYTES - 1, 0x66)]))?;
    vectors.clear();
    now = 0;
    reset(&mut vectors, &mut now);
    read_byte(&mut vectors, &mut now, 0, 2048, 0xff);
    read_byte(&mut vectors, &mut now, 131071, 2111, 0x66);
    exchange("restart-after-erase", &vectors)?;
    let geometry = rejected(
        "/work/nand.raw",
        Some("md-sim-pl353.chip-id=0xba"),
        "only explicit EC/DA",
    )?;
    fs::File::create("/work/data-only.raw")?.set_len(256 * 1024 * 1024)?;
    let format = rejected(
        "/work/data-only.raw",
        None,
        "data-only backing is unsupported",
    )?;
    println!(
        "{}",
        json!({"storage_test_passed":true,"after_program":first,"after_erase":second,"unsupported_geometry":geometry,"unsupported_format":format,"limits":"Generic EC/DA x8 fixture; completed operations survive clean QEMU restart; physical power loss, exact board and migration remain unverified."})
    );
    Ok(())
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "__inside") {
        return inside();
    }
    ensure!(
        args.len() == 3,
        "usage: s9_nand_storage BACKEND_BUILD NEW_OUTPUT"
    );
    let build = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(&out)?;
    save_json(
        &out.join("assessment.json"),
        &json!({"passed":false,"stage":"input validation"}),
    )?;
    save_json(
        &out.join("probe-identity.json"),
        &json!({"binary":hash_file(&std::env::current_exe()?)?,"source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_nand_storage.rs"))}),
    )?;
    ensure!(
        fs::metadata(build.join("build-report.json"))?.len() < 16 * 1024 * 1024,
        "build report bound"
    );
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(build.join("build-report.json"))?)?;
    let binary = build.join("qemu-system-arm");
    ensure!(
        report["build"]["code"] == 0
            && report["nand_timing_patch"]["code"] == 0
            && hash_file(&binary)? == serde_json::from_value::<ObjectId>(report["binary"].clone())?,
        "backend identity"
    );
    let store = Store::open(&out.join("inputs"), 256 * 1024 * 1024)?;
    let inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/probe".into(), std::env::current_exe()?),
            ("/qemu".into(), binary),
        ],
    )?;
    let spec = ProcessSpec {
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec!["/probe".into(), "__inside".into()],
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 45,
            cpu_seconds: 20,
            memory_bytes: 1024 * 1024 * 1024,
            processes: 32,
            file_bytes: 300 * 1024 * 1024,
            trace_bytes: 4 * 1024 * 1024,
            tmp_bytes: 600 * 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    let stdout = hex::decode(&result.stdout)?;
    fs::write(out.join("transactions.jsonl"), &stdout)?;
    fs::write(out.join("backend.log"), hex::decode(&result.stderr)?)?;
    let passed = result.status == "exited"
        && result.exit_code == Some(0)
        && !result.trace_truncated
        && String::from_utf8_lossy(&stdout)
            .lines()
            .last()
            .is_some_and(|line| {
                serde_json::from_str::<serde_json::Value>(line)
                    .is_ok_and(|v| v["storage_test_passed"] == true)
            });
    save_json(
        &out.join("assessment.json"),
        &json!({"passed":passed,"backend_build":hash_file(&build.join("build-report.json"))?,"spec":hash_file(&out.join("spec.json"))?,"result":hash_file(&out.join("result.json"))?,"transactions":hash_file(&out.join("transactions.jsonl"))?,"backend_log":hash_file(&out.join("backend.log"))?,"probe":hash_file(&out.join("probe-identity.json"))?,"scope":"Generic file-backed NAND controller/storage test; original board gate remains incomplete."}),
    )?;
    ensure!(passed, "NAND storage test failed; evidence retained");
    Ok(())
}
fn main() -> Result<()> {
    let out = std::env::args_os().nth(2).map(PathBuf::from);
    let new = out.as_ref().is_some_and(|p| !p.exists());
    let result = run();
    if let (Err(error), Some(out)) = (&result, out) {
        if new && out.is_dir() {
            let mut hashes = BTreeMap::new();
            for name in [
                "spec.json",
                "result.json",
                "transactions.jsonl",
                "backend.log",
                "probe-identity.json",
            ] {
                if out.join(name).is_file() {
                    hashes.insert(name, hash_file(&out.join(name))?);
                }
            }
            save_json(
                &out.join("assessment.json"),
                &json!({"passed":false,"error":format!("{error:#}"),"retained_evidence":hashes}),
            )?;
            save_json(
                &out.join("failure.json"),
                &json!({"passed":false,"error":format!("{error:#}")}),
            )?;
        }
    }
    result
}

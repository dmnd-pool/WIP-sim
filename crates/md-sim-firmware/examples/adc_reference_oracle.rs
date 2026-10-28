//! Compile and execute supplied scalar C in the isolated preparation profile.
//! The source is an arithmetic oracle adapter, not executable miner firmware.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute_with_trace, save_json, Limits, Mount, ProcessSpec, TraceFiles},
    store::{ObjectId, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Write,
    os::unix::process::ExitStatusExt,
    path::Path,
    process::{Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
};

#[path = "support/bounded_file.rs"]
mod bounded_file;
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;

const SOURCE_LIMIT: u64 = 1 << 20;
const FILE_LIMIT: u64 = 2 << 20;
const ARCHIVE_LIMIT: u64 = 8 << 20;
const OUTPUTS: &[&str] = &[
    "oracle",
    "compiler-version.stdout",
    "compiler-version.stderr",
    "linker-version.stdout",
    "linker-version.stderr",
    "compile.stdout",
    "compile.stderr",
    "execution.stdout",
    "execution.stderr",
];

fn compile_arguments() -> Vec<&'static str> {
    vec![
        "--target=x86_64-linux-gnu",
        "-std=c11",
        "-O2",
        "-nostdinc",
        "-nostdlib",
        "-static",
        "-fno-pie",
        "-ffreestanding",
        "-fno-builtin",
        "-fno-stack-protector",
        "-fno-unwind-tables",
        "-fno-asynchronous-unwind-tables",
        // The adapter provides _start directly. Realign its Linux entry stack
        // before compiled C uses ordinary System V stack objects.
        "-mstackrealign",
        "-fsanitize=undefined",
        "-fsanitize-trap=undefined",
        "-fno-sanitize-recover=all",
        "-Wall",
        "-Wextra",
        "-Werror",
        "--ld-path=/tools/ld",
        "-Wl,-e,_start,-z,noexecstack",
        "/oracle.c",
        "-o",
        "/work/oracle",
    ]
}

fn step(program: &str, args: &[&str], name: &str, steps: &mut Vec<Value>) -> Result<()> {
    let stdout = format!("/work/{name}.stdout");
    let stderr = format!("/work/{name}.stderr");
    // Files use the process's2MiB FSIZE bound. They cannot fill unbounded host
    // memory while a compiler or the oracle is still running.
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(File::create_new(&stdout)?)
        .stderr(File::create_new(&stderr)?)
        .status()?;
    let out = bounded_file::capture(Path::new(&stdout), FILE_LIMIT)?;
    let err = bounded_file::capture(Path::new(&stderr), FILE_LIMIT)?;
    steps.push(
        json!({"program":program,"arguments":args,"exit_code":status.code(),
        "signal":status.signal(),"stdout":out.object,"stderr":err.object}),
    );
    ensure!(
        status.code() == Some(0),
        "oracle step {name} failed: {status}"
    );
    ensure!(
        err.bytes.is_empty(),
        "oracle step {name} emitted diagnostics"
    );
    if name == "compile" {
        ensure!(out.bytes.is_empty(), "compiler emitted unexpected stdout");
    }
    if name == "execution" {
        ensure!(!out.bytes.is_empty(), "oracle output is empty");
    }
    Ok(())
}

fn inside() -> Result<()> {
    confinement::require_adc_reference_oracle()?;
    ensure!(
        cfg!(all(target_arch = "x86_64", target_os = "linux")),
        "oracle adapter requires the Linux x86_64 ABI"
    );
    let source = bounded_file::capture(Path::new("/oracle.c"), SOURCE_LIMIT)?;
    let mut steps = Vec::new();
    let arguments = compile_arguments();
    let result = (|| -> Result<()> {
        step(
            "/tools/clang",
            &["--version"],
            "compiler-version",
            &mut steps,
        )?;
        step("/tools/ld", &["--version"], "linker-version", &mut steps)?;
        step("/tools/clang", &arguments, "compile", &mut steps)?;
        let binary = bounded_file::capture(Path::new("/work/oracle"), FILE_LIMIT)?;
        ensure!(
            binary.bytes.starts_with(b"\x7fELF"),
            "compiler output is not ELF"
        );
        step("/work/oracle", &[], "execution", &mut steps)
    })();
    // Capture completed failure diagnostics too. An outer timeout can prevent
    // this final export; its process result then remains incomplete/failed.
    let mut files = BTreeMap::new();
    for name in OUTPUTS {
        let path = Path::new("/work").join(name);
        if path.try_exists()? {
            let captured = bounded_file::capture(&path, FILE_LIMIT)?;
            files.insert(*name, captured);
        }
    }
    let identities: BTreeMap<_, _> = files.iter().map(|(n, f)| (*n, &f.object)).collect();
    let assessment = json!({"schema":"md-sim-adc-scalar-c-oracle/1",
        "passed":result.is_ok(),"source":source.object,"steps":steps,"files":identities,
        "compile_arguments":arguments,"error":result.as_ref().err().map(|e|format!("{e:#}")),
        "scope":"Original-source scalar arithmetic in an explicit native ABI adapter; no miner or physical calibration execution.",
        "abi":"Linux x86_64 freestanding _start; explicit fixed-width scalar types and syscall output",
        "firmware_executed":false,"physical_calibration_gate":false,"original_board_gate":false});
    let assessment = serde_json::to_vec_pretty(&assessment)?;
    let total =
        assessment.len() as u64 + files.values().map(|f| f.object.size + 1024).sum::<u64>() + 2048;
    ensure!(total <= ARCHIVE_LIMIT, "oracle evidence archive bound");
    let mut archive = tar::Builder::new(std::io::stdout());
    for (name, bytes) in std::iter::once(("assessment.json", assessment.as_slice())).chain(
        files
            .iter()
            .map(|(name, file)| (*name, file.bytes.as_slice())),
    ) {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o600);
        header.set_mtime(0);
        header.set_cksum();
        archive.append_data(&mut header, name, bytes)?;
    }
    archive.finish()?;
    std::io::stdout().flush()?;
    result
}

fn run(source_path: &Path, out: &Path) -> Result<()> {
    let source = bounded_file::capture(source_path, SOURCE_LIMIT)?;
    fs::create_dir(out)?;
    let store = Store::open(&out.join("inputs"), 512 << 20)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/probe".into(), "/proc/self/exe".into()),
            ("/tools/clang".into(), "/usr/bin/clang".into()),
            ("/tools/ld".into(), "/usr/bin/ld".into()),
        ],
    )?;
    let offset = store.begin(&source.object)?;
    if offset < source.object.size {
        store.chunk(&source.object, offset, &source.bytes[offset as usize..])?;
    }
    store.finish(&source.object)?;
    inputs.push(Mount {
        object: source.object.clone(),
        guest_path: "/oracle.c".into(),
    });
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        schema_version: "2.0".into(),
        dashboard: None,
        loopback: false,
        argv: vec!["/probe".into(), "__inside".into()],
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 90,
            cpu_seconds: 60,
            memory_bytes: 2 << 30,
            processes: 16,
            file_bytes: FILE_LIMIT,
            trace_bytes: ARCHIVE_LIMIT,
            tmp_bytes: 32 << 20,
        },
    };
    let recipe = spec
        .inputs
        .iter()
        .find(|m| m.guest_path == "/probe")
        .context("captured helper absent")?
        .object
        .clone();
    save_json(
        &out.join("input-identities.json"),
        &json!({"source":source.object,
        "source_path":source_path,"recipe":recipe,"tool_closure":spec.inputs,
        "scope":"Bounded explicit source bytes and captured host compiler/linker; no host execution of supplied C."}),
    )?;
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute_with_trace(
        &store,
        &spec,
        Arc::new(AtomicBool::new(false)),
        Some(TraceFiles {
            stdout: File::create_new(out.join("evidence.tar"))?,
            stderr: File::create_new(out.join("stderr.log"))?,
        }),
    )?;
    save_json(&out.join("result.json"), &result)?;
    ensure!(
        result.status == "exited"
            && result.exit_code == Some(0)
            && !result.trace_truncated
            && result.stderr.is_empty(),
        "isolated C oracle failed; retain result and evidence archive"
    );
    let raw = bounded_file::capture(&out.join("evidence.tar"), ARCHIVE_LIMIT)?;
    ensure!(
        hex::decode(&result.stdout)? == raw.bytes,
        "oracle archive differs from process output"
    );
    // The separate verifier checks every arithmetic word. This runner binds
    // completion, source, and all exported objects before it reports success.
    let mut archive = tar::Archive::new(raw.bytes.as_slice());
    let mut files = BTreeMap::new();
    for entry in archive.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.to_string_lossy().into_owned();
        ensure!(
            (name == "assessment.json" || OUTPUTS.contains(&name.as_str()))
                && !files.contains_key(&name)
                && entry.header().entry_type().is_file()
                && entry.size() <= FILE_LIMIT,
            "oracle archive member type, name or bound"
        );
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes)?;
        files.insert(name, bytes);
    }
    let assessment: Value =
        serde_json::from_slice(files.get("assessment.json").context("assessment absent")?)?;
    ensure!(
        assessment["passed"] == true && assessment["source"] == json!(source.object),
        "oracle source or completion differs"
    );
    for name in OUTPUTS {
        let bytes = files.get(*name).context("oracle output absent")?;
        let object: ObjectId = serde_json::from_value(assessment["files"][*name].clone())?;
        ensure!(
            object.size == bytes.len() as u64
                && object.sha256 == md_sim_firmware::prepare::hash_bytes(bytes),
            "oracle retained object differs: {name}"
        );
    }
    save_json(&out.join("assessment.json"), &assessment)?;
    println!("Isolated C oracle completed; arithmetic comparison is a separate gate.");
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "__inside") {
        ensure!(args.len() == 2, "unexpected oracle internal argument");
        return inside();
    }
    ensure!(
        args.len() == 3,
        "usage: adc_reference_oracle SOURCE_C NEW_OUTPUT"
    );
    run(Path::new(&args[1]), Path::new(&args[2]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiler_recipe_retains_freestanding_traps_and_fixed_entry_paths() {
        let args = compile_arguments();
        for required in [
            "--target=x86_64-linux-gnu",
            "-nostdinc",
            "-nostdlib",
            "-static",
            "-ffreestanding",
            "-fno-builtin",
            "-fno-stack-protector",
            "-mstackrealign",
            "-fsanitize=undefined",
            "-fsanitize-trap=undefined",
            "-fno-sanitize-recover=all",
            "--ld-path=/tools/ld",
            "-Wl,-e,_start,-z,noexecstack",
            "-Werror",
        ] {
            assert_eq!(args.iter().filter(|arg| **arg == required).count(), 1);
        }
        assert_eq!(
            &args[args.len() - 3..],
            &["/oracle.c", "-o", "/work/oracle"]
        );
        // The upstream algorithm deliberately uses defined unsigned wrapping.
        // Trapping unsigned overflow would test a different arithmetic contract.
        assert!(!args
            .iter()
            .any(|arg| arg.contains("unsigned-integer-overflow")));
    }
}

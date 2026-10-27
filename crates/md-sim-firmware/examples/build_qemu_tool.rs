//! Compile the explicitly derived upstream Samurai source using a locked GCC closure.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, Store},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};
#[path = "../src/confinement.rs"]
mod confinement;
const SNAPSHOT: &str = "e2eabd7bd16c22397c37cfbcfd0115223406f7850b5ad8f0733de6ac79e4d12a";
fn unpack(path: &Path) -> Result<()> {
    let mut tar = tar::Archive::new(
        flate2::read::GzDecoder::new(fs::File::open(path)?).take(1024 * 1024 * 1024),
    );
    let mut total = 0u64;
    for (index, entry) in tar.entries()?.enumerate() {
        ensure!(index < 60_000, "entry bound");
        let mut entry = entry?;
        ensure!(entry.size() <= 128 * 1024 * 1024, "member bound");
        total = total.checked_add(entry.size()).context("size overflow")?;
        ensure!(total <= 800 * 1024 * 1024, "expansion bound");
        let directory = entry.header().entry_type().is_dir();
        let target = Path::new("/work").join(entry.path()?);
        ensure!(entry.unpack_in("/work")?, "archive path escape");
        if directory {
            // The captured closure is read-only. Extraction still needs to create
            // children before the isolated build uses those exact file bytes.
            fs::set_permissions(target, fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(())
}
fn inner() -> Result<()> {
    confinement::require(true)?;
    unpack(Path::new("/toolchain.tar.gz"))?;
    unpack(Path::new("/source.tar.gz"))?;
    let mut args = vec![
        "--sysroot=/work/toolchain",
        "-B/work/toolchain/usr/bin",
        "-std=c99",
        "-O2",
        "-Wall",
        "-Wextra",
        "-Wshadow",
        "-Wmissing-prototypes",
        "-Wpedantic",
        "-Wno-unused-parameter",
        "-o",
        "/work/samu",
    ];
    args.extend([
        "build.c",
        "deps.c",
        "env.c",
        "graph.c",
        "htab.c",
        "log.c",
        "parse.c",
        "samu.c",
        "scan.c",
        "tool.c",
        "tree.c",
        "util.c",
        "os-posix.c",
        "-lrt",
    ]);
    let compile = Command::new("/work/toolchain/usr/bin/gcc")
        .args(&args)
        .current_dir("/work/source")
        .output()?;
    let mut report = json!({"compile_exit":compile.status.code(),"compile_stdout":String::from_utf8_lossy(&compile.stdout),"compile_stderr":String::from_utf8_lossy(&compile.stderr),"args":args});
    if compile.status.success() {
        let version = Command::new("/work/samu").arg("--version").output()?;
        report["version"] = json!({"code":version.status.code(),"stdout":String::from_utf8_lossy(&version.stdout),"stderr":String::from_utf8_lossy(&version.stderr)});
        fs::write("/work/build.ninja", "rule write\n  command = /bin/sh -c 'printf fresh > $out'\nrule copy\n  command = /bin/sh -c 'read v < $in; printf %s $$v > $out'\nbuild /work/first: write\nbuild /work/second: copy /work/first\ndefault /work/second\n")?;
        let test = Command::new("/work/samu")
            .args(["-f", "/work/build.ninja", "-j", "2"])
            .output()?;
        report["test"] = json!({"code":test.status.code(),"stdout":String::from_utf8_lossy(&test.stdout),"stderr":String::from_utf8_lossy(&test.stderr),"output":fs::read_to_string("/work/second").ok()});
        let again = Command::new("/work/samu")
            .args(["-f", "/work/build.ninja"])
            .output()?;
        report["incremental"] = json!({"code":again.status.code(),"stdout":String::from_utf8_lossy(&again.stdout),"stderr":String::from_utf8_lossy(&again.stderr)});
        let bytes = fs::read("/work/samu")?;
        report["binary"] = serde_json::to_value(hash_file(Path::new("/work/samu"))?)?;
        report["binary_hex"] = json!(hex::encode(bytes));
    }
    serde_json::to_writer(std::io::stdout(), &report)?;
    ensure!(compile.status.success(), "Samurai source build failed");
    Ok(())
}
fn pack(path: &Path, prefix: &str, out: &Path) -> Result<()> {
    let encoder =
        flate2::write::GzEncoder::new(fs::File::create(out)?, flate2::Compression::fast());
    let mut tar = tar::Builder::new(encoder);
    tar.follow_symlinks(false);
    tar.append_dir_all(prefix, path)?;
    tar.into_inner()?.finish()?.flush()?;
    Ok(())
}
fn verify(repo: &Path, snapshot: &Path) -> Result<serde_json::Value> {
    ensure!(
        hash_file(&snapshot.join("manifest.json"))?.sha256 == SNAPSHOT,
        "unexpected frozen toolchain"
    );
    let output = Command::new(repo.join("target/debug/examples/freeze_cgminer_toolchain"))
        .arg("verify")
        .arg(snapshot)
        .output()?;
    ensure!(
        output.status.success(),
        "snapshot verification failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|x| x == "__inside") {
        return inner();
    }
    ensure!(
        args.len() == 5 || args.len() == 6,
        "usage: build_qemu_tool REPO FROZEN_TOOLCHAIN DERIVED_SOURCE NEW_OUTPUT [PRIOR_CAPTURE_DIRECTORY]"
    );
    let repo = PathBuf::from(&args[1]);
    let snapshot = PathBuf::from(&args[2]);
    let source = PathBuf::from(&args[3]);
    let out = PathBuf::from(&args[4]);
    ensure!(!out.exists(), "output must be new");
    let snapshot_id = verify(&repo, &snapshot)?;
    fs::create_dir_all(&out)?;
    if let Some(prior) = args.get(5) {
        let prior = PathBuf::from(prior);
        let prior_inputs: serde_json::Value =
            serde_json::from_slice(&fs::read(prior.join("input-provenance.json"))?)?;
        ensure!(
            prior_inputs["closure"] == snapshot_id,
            "cached closure differs"
        );
        let prior_spec: ProcessSpec = serde_json::from_slice(&fs::read(prior.join("spec.json"))?)?;
        let expected = &prior_spec
            .inputs
            .iter()
            .find(|m| m.guest_path == "/toolchain.tar.gz")
            .context("cached archive identity absent")?
            .object;
        ensure!(
            &hash_file(&prior.join("toolchain.tar.gz"))? == expected,
            "cached archive changed"
        );
        fs::copy(prior.join("toolchain.tar.gz"), out.join("toolchain.tar.gz"))?;
    } else {
        pack(
            &snapshot.join("tree"),
            "toolchain",
            &out.join("toolchain.tar.gz"),
        )?;
        ensure!(
            verify(&repo, &snapshot)? == snapshot_id,
            "snapshot changed while packing"
        );
    }
    pack(&source, "source", &out.join("source.tar.gz"))?;
    let store = Store::open(&out.join("inputs"), 1024 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/probe".into(), std::env::current_exe()?),
            ("/bin/sh".into(), PathBuf::from("/usr/bin/bash")),
            ("/compiler".into(), snapshot.join("tree/usr/bin/gcc")),
            (
                "/compiler-cc1".into(),
                snapshot.join("tree/usr/lib/gcc/x86_64-pc-linux-gnu/16/cc1"),
            ),
            ("/assembler".into(), snapshot.join("tree/usr/bin/as")),
            ("/linker".into(), snapshot.join("tree/usr/bin/ld")),
        ],
    )?;
    for name in ["toolchain.tar.gz", "source.tar.gz"] {
        let path = out.join(name);
        let object = hash_file(&path)?;
        store.import(&path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: format!("/{name}"),
        });
    }
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec!["/probe".into(), "__inside".into()],
        inputs,
        stdin_object: None,
        environment: BTreeMap::from([("PATH".into(), "/work/toolchain/usr/bin".into())]),
        limits: Limits {
            wall_seconds: 120,
            cpu_seconds: 100,
            memory_bytes: 2 * 1024 * 1024 * 1024,
            processes: 128,
            file_bytes: 64 * 1024 * 1024,
            trace_bytes: 4 * 1024 * 1024,
            tmp_bytes: 1024 * 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    save_json(
        &out.join("input-provenance.json"),
        &json!({"closure":snapshot_id,"verifier":hash_file(&repo.join("target/debug/examples/freeze_cgminer_toolchain"))?,"recipe":hash_file(&std::env::current_exe()?)?,"source_provenance":serde_json::from_slice::<serde_json::Value>(&fs::read(source.join("provenance.json"))?)?,"closure_note":"Compiler tree bytes are frozen; loader shared library closure is separately pinned in ProcessSpec."}),
    )?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    let bytes = hex::decode(&result.stdout)?;
    fs::write(out.join("build-envelope.json"), &bytes)?;
    if result.exit_code == Some(0) && !result.trace_truncated {
        let mut report: serde_json::Value = serde_json::from_slice(&bytes)?;
        let binary = hex::decode(report["binary_hex"].as_str().context("binary absent")?)?;
        ensure!(
            md_sim_firmware::prepare::hash_bytes(&binary) == report["binary"]["sha256"],
            "export hash mismatch"
        );
        fs::write(out.join("samu"), binary)?;
        report.as_object_mut().unwrap().remove("binary_hex");
        save_json(&out.join("build-report.json"), &report)?;
        ensure!(
            report["test"]["code"] == 0
                && report["test"]["output"] == "fresh"
                && report["incremental"]["code"] == 0,
            "tool behavior failed"
        );
    } else {
        anyhow::bail!("isolated Samurai build failed; bounded evidence retained");
    }
    Ok(())
}

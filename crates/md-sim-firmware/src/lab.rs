//! Safe host entry for the isolated source experiment, with locked ELF inputs.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, Store},
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    io::Read,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const LIBRARY_ROOTS: &[&str] = &[
    "/usr/lib",
    "/usr/lib64",
    "/lib/x86_64-linux-gnu",
    "/lib/aarch64-linux-gnu",
];

fn trusted_library(path: &Path) -> Result<PathBuf> {
    let canonical = path.canonicalize()?;
    ensure!(
        LIBRARY_ROOTS
            .iter()
            .filter_map(|p| Path::new(p).canonicalize().ok())
            .any(|root| canonical.starts_with(root)),
        "ELF dependency is outside system library roots"
    );
    ensure!(canonical.is_file(), "ELF dependency must be a regular file");
    let mut magic = [0; 4];
    fs::File::open(&canonical)?.read_exact(&mut magic)?;
    ensure!(&magic == b"\x7fELF", "dependency is not an ELF file");
    Ok(canonical)
}

fn interpreter_path(path: &str) -> Result<PathBuf> {
    ensure!(
        [
            "/lib64/ld-linux-x86-64.so.2",
            "/usr/lib/ld-linux-x86-64.so.2",
            "/lib/ld-linux-x86-64.so.2",
            "/lib/ld-linux-aarch64.so.1"
        ]
        .contains(&path),
        "unsupported ELF interpreter path: {path}"
    );
    trusted_library(Path::new(path))
}

struct Inspector(std::process::Child);
impl Drop for Inspector {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            unsafe {
                libc::kill(-(self.0.id() as i32), libc::SIGKILL);
            }
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

/// Parse untrusted ELF metadata only in a private, bounded preparation environment.
fn inspect_elf(source: &Path) -> Result<String> {
    let mut command = Command::new("/usr/bin/unshare");
    command
        .args([
            "--user",
            "--map-root-user",
            "--net",
            "--mount",
            "--fork",
            "--kill-child",
            "/usr/bin/bwrap",
            "--die-with-parent",
            "--unshare-pid",
            "--ro-bind",
            "/usr",
            "/usr",
            "--symlink",
            "usr/lib",
            "/lib",
            "--symlink",
            "usr/lib64",
            "/lib64",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--ro-bind",
        ])
        .arg(source)
        .args([
            "/input",
            "--cap-drop",
            "ALL",
            "--remount-ro",
            "/",
            "/usr/bin/readelf",
            "--wide",
            "--program-headers",
            "--dynamic",
            "/input",
        ])
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 || libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            for (kind, value) in [
                (libc::RLIMIT_CPU, 5),
                (libc::RLIMIT_AS, 256 * 1024 * 1024),
                (libc::RLIMIT_NPROC, 16),
                (libc::RLIMIT_FSIZE, 0),
                (libc::RLIMIT_CORE, 0),
            ] {
                let limit = libc::rlimit {
                    rlim_cur: value,
                    rlim_max: value,
                };
                if libc::setrlimit(kind, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
    let mut child = Inspector(command.spawn()?);
    let stdout = child
        .0
        .stdout
        .take()
        .context("missing ELF inspector output")?;
    let reader = std::thread::spawn(move || -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        stdout.take(1024 * 1024 + 1).read_to_end(&mut bytes)?;
        Ok(bytes)
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        ensure!(
            start.elapsed() < Duration::from_secs(10),
            "ELF inspector deadline"
        );
        std::thread::sleep(Duration::from_millis(10));
    };
    let bytes = reader
        .join()
        .map_err(|_| anyhow::anyhow!("ELF inspector reader failed"))??;
    ensure!(
        bytes.len() <= 1024 * 1024 && status.success(),
        "ELF inspector failed or exceeded its output limit"
    );
    Ok(String::from_utf8(bytes)?)
}

/// Static ELF metadata only. Never execute ldd on acquired or rebuilt firmware.
pub fn elf_closure(store: &Store, initial: Vec<(String, PathBuf)>) -> Result<Vec<Mount>> {
    let mut pending: VecDeque<_> = initial.into();
    let mut paths = BTreeSet::new();
    let mut inputs = Vec::new();
    while let Some((guest_path, source)) = pending.pop_front() {
        if !paths.insert(guest_path.clone()) {
            continue;
        }
        let object = hash_file(&source)?;
        store.import(&source, &object)?;
        inputs.push(Mount { object, guest_path });
        for line in inspect_elf(&source)?.lines() {
            if line.contains("(RPATH)") || line.contains("(RUNPATH)") {
                let paths = line
                    .split('[')
                    .nth(1)
                    .and_then(|s| s.split(']').next())
                    .context("bad ELF runpath")?;
                for path in paths.split(':') {
                    ensure!(
                        path.starts_with('/')
                            && !path.contains('$')
                            && Path::new(path).canonicalize()?
                                == Path::new("/usr/lib").canonicalize()?,
                        "ELF search-path override is outside the default system library directory"
                    );
                }
            }
            if let Some(rest) = line.split("Requesting program interpreter: ").nth(1) {
                let interpreter = rest.trim_end_matches(']').trim();
                pending.push_back((interpreter.into(), interpreter_path(interpreter)?));
            }
            if line.contains("(NEEDED)") {
                let name = line
                    .split('[')
                    .nth(1)
                    .and_then(|s| s.split(']').next())
                    .context("bad ELF dependency")?;
                ensure!(
                    !name.contains('/') && !name.is_empty(),
                    "unsafe ELF dependency name"
                );
                let library = LIBRARY_ROOTS
                    .iter()
                    .map(|p| Path::new(p).join(name))
                    .find(|p| p.is_file())
                    .with_context(|| format!("library {name} unavailable"))?;
                let trusted = trusted_library(&library)?;
                pending.push_back((library.to_string_lossy().into_owned(), trusted));
            }
        }
        ensure!(inputs.len() <= 256, "ELF closure size limit");
    }
    Ok(inputs)
}

pub fn run(
    prepared: &Path,
    out: &Path,
    cap: f64,
    seed: Option<&str>,
    epoch: Option<u32>,
    exponent: i32,
    regtest: bool,
) -> Result<()> {
    ensure!(!out.exists(), "test output directory must not exist");
    let result = run_inner(
        prepared,
        out,
        SourceRunOptions {
            cap,
            seed: seed.map(str::to_string),
            epoch,
            exponent,
            regtest,
        },
        true,
    );
    if let Err(error) = &result {
        fs::create_dir_all(out)?;
        save_json(
            &out.join("failure.json"),
            &serde_json::json!({"error":format!("{error:#}"),"complete":false}),
        )?;
        save_checksums(out)?;
    }
    result
}

#[derive(Clone, Debug)]
pub struct SourceRunOptions {
    pub cap: f64,
    pub seed: Option<String>,
    pub epoch: Option<u32>,
    pub exponent: i32,
    pub regtest: bool,
}

/// Prepare the same immutable process closure used by `run`, without starting it.
/// The coordinator executes this exact specification locally or on a worker.
pub fn lock_recipe(prepared: &Path, out: &Path, options: SourceRunOptions) -> Result<()> {
    ensure!(!out.exists(), "locked recipe output must be new");
    run_inner(prepared, out, options, false)
}

fn run_inner(
    prepared: &Path,
    out: &Path,
    options: SourceRunOptions,
    execute_firmware: bool,
) -> Result<()> {
    let SourceRunOptions {
        cap,
        seed,
        epoch,
        exponent,
        regtest,
    } = options;
    let seed = seed.as_deref();
    let schedule = crate::schedule::mining_schedule(cap, exponent)?;
    ensure!(
        (-31..=11).contains(&exponent),
        "difficulty exponent must be -31 through 11"
    );
    ensure!(
        cap.is_finite() && cap > 0.0 && cap <= 10_000.0,
        "hash cap must be above zero and at most 10000 H/s"
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(prepared.join("input-manifest.json"))?)?;
    let firmware = prepared.join("cgminer").canonicalize()?;
    ensure!(
        hash_file(&firmware)?.sha256 == manifest["firmware_binary"]["sha256"],
        "prepared firmware differs from input manifest"
    );
    ensure!(
        manifest["source_sha256"] == crate::prepare::SOURCE_SHA256
            && manifest["patch_sha256"] == crate::prepare::hash_bytes(crate::prepare::PATCH)
            && manifest["adaptation_manifest_sha256"]
                == crate::prepare::hash_bytes(crate::prepare::ADAPTATION),
        "prepared source/adaptation identity is not this build"
    );
    fs::create_dir_all(out)?;
    let store = Store::open(&out.join("input-store"), 64 * 1024 * 1024)?;
    let harness = std::env::current_exe()?
        .parent()
        .context("executable has no parent")?
        .join("cgminer-fresh-work")
        .canonicalize()
        .context("build all md-sim-firmware binaries first")?;
    let inputs = elf_closure(
        &store,
        vec![("/firmware".into(), firmware), ("/harness".into(), harness)],
    )?;
    let seed = if let Some(seed) = seed {
        ensure!(hex::decode(seed)?.len() == 32, "seed must contain 32 bytes");
        seed.to_string()
    } else {
        use std::io::Read;
        let mut seed = [0; 32];
        fs::File::open("/dev/urandom")?.read_exact(&mut seed)?;
        hex::encode(seed)
    };
    let spec = ProcessSpec {
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        stdin_object: None,
        argv: vec![
            "/harness".into(),
            "/firmware".into(),
            "/work/results".into(),
            cap.to_string(),
            seed,
            epoch
                .unwrap_or(u32::try_from(
                    SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
                )?)
                .to_string(),
            exponent.to_string(),
            if regtest {
                "regtest-genesis"
            } else {
                "random-parent"
            }
            .into(),
            "--export".into(),
        ],
        inputs,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: schedule.timeout_seconds + 10,
            cpu_seconds: 90,
            memory_bytes: 1024 * 1024 * 1024,
            processes: 128,
            file_bytes: 24 * 1024 * 1024,
            trace_bytes: 64 * 1024 * 1024,
            tmp_bytes: 64 * 1024 * 1024,
        },
    };
    save_json(&out.join("run-spec.json"), &spec)?;
    save_json(&out.join("input-manifest.json"), &manifest)?;
    fs::write(
        out.join("adaptation-manifest.json"),
        crate::prepare::ADAPTATION,
    )?;
    save_json(
        &out.join("execution-identity.json"),
        &serde_json::json!({
            "run_spec_file_sha256":hash_file(&out.join("run-spec.json"))?.sha256,
            "run_spec_structured_sha256":crate::prepare::hash_bytes(&serde_json::to_vec(&spec)?),
            "structured_serialization":"serde_json::to_vec on ProcessSpec, as compiled in harness",
            "suite_source_sha256":crate::prepare::hash_bytes(include_bytes!("bin/cgminer-fresh-work.rs")),
            "model_source_sha256":crate::prepare::hash_bytes(include_bytes!("bitmain_s2.rs")),
            "verifier_source_sha256":crate::prepare::hash_bytes(include_bytes!("../../md-sim-mining/src/pool.rs")),
            "cargo_lock_sha256":crate::prepare::hash_bytes(include_bytes!("../../../Cargo.lock")),
            "preparation_manifest_sha256":crate::prepare::hash_bytes(&serde_json::to_vec(&manifest)?),
            "adaptation_manifest_sha256":crate::prepare::hash_bytes(crate::prepare::ADAPTATION),
            "model_and_verifier_build":spec.inputs.iter().find(|m|m.guest_path=="/harness").map(|m|&m.object),
            "verifier_build":spec.inputs.iter().find(|m|m.guest_path=="/harness").map(|m|&m.object),
            "input_closure":"run-spec.json inputs records every mounted ELF file hash"
        }),
    )?;
    if !execute_firmware {
        save_checksums(out)?;
        return Ok(());
    }
    let result = match execute(&store, &spec, Arc::new(AtomicBool::new(false))) {
        Ok(result) => result,
        Err(error) => {
            save_json(
                &out.join("execution-error.json"),
                &serde_json::json!({"error":format!("{error:#}"),"complete":false}),
            )?;
            save_checksums(out)?;
            return Err(error);
        }
    };
    export_evidence(out, &result)?;
    ensure!(
        result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
        "source experiment failed or incomplete; inspect {}",
        out.display()
    );
    Ok(())
}

pub fn export_evidence(
    out: &Path,
    result: &md_sim_runtime::isolation::ProcessResult,
) -> Result<()> {
    for (name, bytes) in exported_evidence(result)? {
        fs::write(out.join(name), bytes)?;
    }
    save_checksums(out)
}

pub fn exported_evidence(
    result: &md_sim_runtime::isolation::ProcessResult,
) -> Result<BTreeMap<String, Vec<u8>>> {
    fn json_bytes(value: &impl serde::Serialize) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec_pretty(value)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
    let mut output = BTreeMap::from([("execution.json".into(), json_bytes(result)?)]);
    let bytes = hex::decode(&result.stdout)?;
    if !bytes.is_empty() {
        let envelope: serde_json::Value =
            serde_json::from_slice(&bytes).context("malformed bounded evidence export")?;
        if let Some(files) = envelope["evidence_hex"].as_object() {
            for (name, value) in files {
                ensure!(
                    [
                        "session.json",
                        "guest.log",
                        "device-transactions.jsonl",
                        "pool-transactions.jsonl",
                        "pool-report.json",
                        "run-inputs.json",
                        "regtest-blocks.json"
                    ]
                    .contains(&name.as_str()),
                    "unknown evidence filename"
                );
                output.insert(
                    name.clone(),
                    hex::decode(value.as_str().context("invalid evidence hex")?)?,
                );
            }
        }
        output.insert("summary.json".into(), json_bytes(&envelope)?);
    }
    Ok(output)
}

fn save_checksums(out: &Path) -> Result<()> {
    let mut evidence_hashes = BTreeMap::new();
    for item in fs::read_dir(out)? {
        let path = item?.path();
        if path.is_file()
            && path
                .file_name()
                .is_some_and(|name| name != "evidence-checksums.json")
        {
            evidence_hashes.insert(
                path.file_name()
                    .context("missing evidence name")?
                    .to_string_lossy()
                    .into_owned(),
                hash_file(&path)?,
            );
        }
    }
    save_json(&out.join("evidence-checksums.json"), &evidence_hashes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_unrelated_host_interpreters_before_reading() {
        for path in [
            "/home/user/.ssh/id_ed25519",
            "/etc/passwd",
            "/lib64/../../etc/passwd",
        ] {
            assert!(interpreter_path(path).is_err());
        }
        let tmp = tempfile::NamedTempFile::new().unwrap();
        assert!(trusted_library(tmp.path()).is_err());
    }
}

//! Retain the model sources around a Rust library build. This is an
//! implementation checkpoint, not a firmware or release acceptance gate.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::store::{hash_file, ObjectId};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn library_artifact(records: &[u8], source: &Path) -> Result<PathBuf> {
    ensure!(
        records.len() <= 16 << 20,
        "Cargo artifact output exceeds bound"
    );
    let mut libraries = Vec::new();
    for line in records
        .split(|&byte| byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let record: serde_json::Value = serde_json::from_slice(line)?;
        if record["reason"] != "compiler-artifact"
            || record["target"]["src_path"] != source.to_string_lossy().as_ref()
        {
            continue;
        }
        ensure!(
            record["target"]["name"] == "md_sim_qemu_core"
                && record["target"]["crate_types"]
                    .as_array()
                    .is_some_and(|k| k.iter().any(|v| v == "staticlib"))
                && record["profile"]["test"] == false,
            "Cargo model artifact target differs"
        );
        for file in record["filenames"]
            .as_array()
            .context("Cargo artifact paths absent")?
        {
            let path = PathBuf::from(file.as_str().context("Cargo artifact path is not text")?);
            if path
                .file_name()
                .is_some_and(|name| name == "libmd_sim_qemu_core.a")
            {
                ensure!(
                    path.is_absolute(),
                    "Cargo model artifact path is not absolute"
                );
                libraries.push(path);
            }
        }
    }
    ensure!(
        libraries.len() == 1,
        "missing or duplicate Cargo model library artifact"
    );
    Ok(libraries.remove(0))
}

fn copy_tree(
    repo: &Path,
    relative: &Path,
    output: &Path,
    entries: &mut BTreeMap<String, ObjectId>,
    total: &mut u64,
) -> Result<()> {
    if relative.file_name().is_some_and(|name| name == "target") {
        return Ok(());
    }
    let input = repo.join(relative);
    let metadata = fs::symlink_metadata(&input)?;
    ensure!(
        !metadata.is_symlink(),
        "source link is not permitted: {}",
        relative.display()
    );
    if metadata.is_dir() {
        for entry in fs::read_dir(&input)? {
            copy_tree(
                repo,
                &relative.join(entry?.file_name()),
                output,
                entries,
                total,
            )?;
        }
    } else {
        ensure!(
            metadata.is_file() && metadata.len() <= 8 * 1024 * 1024,
            "source file bound"
        );
        ensure!(entries.len() < 4096, "source count bound");
        *total += metadata.len();
        ensure!(*total <= 128 * 1024 * 1024, "source aggregate bound");
        let object = hash_file(&input)?;
        let destination = output.join("source").join(relative);
        fs::create_dir_all(destination.parent().context("source parent")?)?;
        fs::copy(&input, &destination)?;
        ensure!(
            hash_file(&destination)? == object,
            "source changed while copying"
        );
        entries.insert(
            relative
                .to_str()
                .context("source path is not UTF-8")?
                .into(),
            object,
        );
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(args.len() == 2, "usage: checkpoint_qemu_core NEW_OUTPUT");
    let repo = std::env::current_dir()?;
    let output = Path::new(&args[1]);
    ensure!(!output.exists(), "checkpoint must be new");
    fs::create_dir_all(output)?;
    let mut entries = BTreeMap::new();
    let mut total = 0;
    for root in [
        "Cargo.toml",
        "Cargo.lock",
        ".cargo",
        "AGENTS.md",
        "README.md",
        "crates",
        "simulator",
        "fixtures",
    ] {
        copy_tree(&repo, Path::new(root), output, &mut entries, &mut total)?;
    }
    let log = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output.join("build.log"))?;
    let build_args = [
        "build",
        "--message-format=json-render-diagnostics",
        "--offline",
        "--locked",
        "--release",
        "-p",
        "md-sim-qemu-core",
        "--lib",
    ];
    let artifact_log = output.join("cargo-artifacts.jsonl");
    let artifacts = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&artifact_log)?;
    let status = Command::new("cargo")
        .args(build_args)
        .env("CARGO_INCREMENTAL", "0")
        .stdout(Stdio::from(artifacts))
        .stderr(Stdio::from(log))
        .status()?;
    ensure!(status.success(), "core build failed; retained build.log");
    for (path, object) in &entries {
        ensure!(
            hash_file(&repo.join(path))? == *object,
            "source changed during build: {path}"
        );
    }
    ensure!(
        fs::metadata(&artifact_log)?.len() <= 16 << 20,
        "Cargo artifact output exceeds bound"
    );
    let library = library_artifact(
        &fs::read(&artifact_log)?,
        &repo.join("crates/md-sim-qemu-core/src/lib.rs"),
    )?;
    let object = hash_file(&library)?;
    let retained = output.join("libmd_sim_qemu_core.a");
    fs::copy(&library, &retained)?;
    ensure!(
        hash_file(&retained)? == object,
        "library changed while retaining"
    );
    let compiler = Command::new("rustc").arg("--version").output()?;
    ensure!(compiler.status.success(), "rustc version failed");
    let report = json!({
        "sources":entries,"source_bytes":total,"library":object,
        "build_command":"CARGO_INCREMENTAL=0 cargo build --message-format=json-render-diagnostics --offline --locked --release -p md-sim-qemu-core --lib",
        "cargo_artifacts":hash_file(&artifact_log)?,"built_library_path":library,
        "build_log":hash_file(&output.join("build.log"))?,
        "rustc_version":String::from_utf8(compiler.stdout)?,
        "checkpoint_tool":hash_file(Path::new("/proc/self/exe"))?,
        "scope":"Model source checkpoint. Source hashes were checked before and after the build. Cargo.lock and offline vendor checks pin Rust dependencies; this directory does not duplicate the vendor tree. No firmware or release pass is granted."
    });
    fs::write(
        output.join("manifest.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!(
        "Retained {} source files and the built library",
        entries.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn library_comes_from_cargo_for_the_exact_source_not_the_default_directory() -> Result<()> {
        let source = Path::new("/repo/crates/md-sim-qemu-core/src/lib.rs");
        let record = json!({"reason":"compiler-artifact","target":{"name":"md_sim_qemu_core",
            "src_path":source,"crate_types":["staticlib"]},"profile":{"test":false},
            "filenames":["/repo/work/other-target/release/libmd_sim_qemu_core.a"]});
        let bytes = serde_json::to_vec(&record)?;
        assert_eq!(
            library_artifact(&bytes, source)?,
            Path::new("/repo/work/other-target/release/libmd_sim_qemu_core.a")
        );
        assert!(library_artifact(&bytes, Path::new("/other/src/lib.rs")).is_err());
        let duplicate = [bytes.clone(), b"\n".to_vec(), bytes.clone()].concat();
        assert!(library_artifact(&duplicate, source).is_err());
        let mut changed = record.clone();
        changed["profile"]["test"] = json!(true);
        assert!(library_artifact(&serde_json::to_vec(&changed)?, source).is_err());
        changed = record.clone();
        changed["target"]["crate_types"] = json!(["rlib"]);
        assert!(library_artifact(&serde_json::to_vec(&changed)?, source).is_err());
        changed = record;
        changed["filenames"] = json!(["relative/libmd_sim_qemu_core.a"]);
        assert!(library_artifact(&serde_json::to_vec(&changed)?, source).is_err());
        Ok(())
    }
}

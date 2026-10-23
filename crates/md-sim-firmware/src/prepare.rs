//! Preparation of one pinned source archive in a disposable build namespace.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::store::hash_file;
use serde_json::json;
use std::{
    fs::{self, File},
    io::Read,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Component, Path},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub const SOURCE_SHA256: &str = "4b3515ec072904e81e91182f5f8d5f8b5b87361ef2a7e2131643d8207badb996";
pub const SOURCE_ROOT: &str = "cgminer-b8491c66e7e22f23a9edf095dd1337ee581e88bd";
pub const PATCH: &[u8] = include_bytes!("../../../simulator/firmware/cgminer-low-rate.patch");
pub const ADAPTATION: &[u8] =
    include_bytes!("../../../simulator/firmware/cgminer-source-adaptation.json");
const REGRESSION: &[u8] = include_bytes!("../../../simulator/firmware/cgminer-regression.c");

struct BuildChild(std::process::Child);
impl Drop for BuildChild {
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

fn verify_members(root: &Path, adaptation: &serde_json::Value, adapted: bool) -> Result<()> {
    for file in adaptation["files"]
        .as_array()
        .context("missing adapted member list")?
    {
        let member = file["member"]
            .as_str()
            .context("missing adapted member path")?;
        let path = Path::new(member);
        ensure!(
            path.starts_with(SOURCE_ROOT)
                && path.components().all(|c| matches!(c, Component::Normal(_))),
            "unsafe adapted member path"
        );
        let path = root.join(path);
        let hash = &file[if adapted {
            "adapted_sha256"
        } else {
            "original_sha256"
        }];
        if !adapted && hash.is_null() {
            ensure!(
                !path.exists(),
                "new member unexpectedly exists in original archive: {member}"
            );
        } else {
            ensure!(
                hash.as_str().is_some() && hash_file(&path)?.sha256 == *hash,
                "member identity mismatch: {member}"
            );
        }
    }
    Ok(())
}

/// Internal preparation entry. Only the named archive and toolchain are visible.
pub fn inside(ubsan: bool) -> Result<()> {
    crate::confinement::require(true)?;
    let adaptation: serde_json::Value = serde_json::from_slice(ADAPTATION)?;
    ensure!(
        adaptation["patch_sha256"] == hash_bytes(PATCH)
            && adaptation["source_archive_sha256"] == SOURCE_SHA256,
        "adaptation manifest differs from source or patch identity"
    );
    ensure!(
        hash_file(Path::new("/source.tar.gz"))?.sha256 == SOURCE_SHA256,
        "source identity mismatch"
    );
    let gzip = flate2::read::GzDecoder::new(File::open("/source.tar.gz")?);
    let mut archive = tar::Archive::new(gzip);
    let mut total = 0u64;
    let mut seen = std::collections::BTreeSet::new();
    for (index, item) in archive.entries()?.enumerate() {
        ensure!(index < 10_000, "archive member count limit");
        let mut entry = item?;
        if entry.header().entry_type().is_pax_global_extensions() {
            ensure!(entry.size() <= 65536, "archive metadata size limit");
            continue;
        }
        let relative = entry.path()?.into_owned();
        ensure!(
            relative
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
            "unsafe archive path"
        );
        ensure!(
            relative.starts_with(SOURCE_ROOT),
            "unexpected archive root: {}",
            relative.display()
        );
        ensure!(seen.insert(relative.clone()), "duplicate archive path");
        let kind = entry.header().entry_type();
        ensure!(
            kind.is_dir() || kind.is_file(),
            "links and special archive members are prohibited"
        );
        let path = Path::new("/work").join(&relative);
        if kind.is_dir() {
            fs::create_dir_all(path)?;
            continue;
        }
        total = total
            .checked_add(entry.size())
            .context("archive size overflow")?;
        ensure!(
            total <= 128 * 1024 * 1024 && entry.size() <= 16 * 1024 * 1024,
            "archive expansion limit"
        );
        fs::create_dir_all(path.parent().context("missing archive parent")?)?;
        let mut out = File::options().write(true).create_new(true).open(&path)?;
        ensure!(
            std::io::copy(&mut entry, &mut out)? == entry.size(),
            "truncated archive member"
        );
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(if entry.header().mode()? & 0o111 != 0 {
                0o700
            } else {
                0o600
            }),
        )?;
    }
    let root = Path::new("/work").join(SOURCE_ROOT);
    verify_members(Path::new("/work"), &adaptation, false)?;
    fs::write("/work/adaptation.patch", PATCH)?;
    fs::write(root.join("md-sim-regression.c"), REGRESSION)?;
    let log = File::create("/work/build.log")?;
    let cflags = if ubsan {
        "-O1 -g -fcommon -Wno-incompatible-pointer-types -fsanitize=undefined -fno-sanitize-recover=all"
    } else {
        "-O2 -fcommon -Wno-incompatible-pointer-types"
    };
    let commands: Vec<(&str, Vec<String>)> = vec![
        (
            "patch",
            vec![
                "--batch".into(),
                "--fuzz=0".into(),
                "-p1".into(),
                "-i".into(),
                "/work/adaptation.patch".into(),
            ],
        ),
        ("autoreconf", vec!["-fi".into()]),
        (
            "/usr/bin/sh",
            vec![
                "./configure".into(),
                "--enable-ants2".into(),
                "--disable-curses".into(),
                "--without-libudev".into(),
                format!("CFLAGS={cflags}"),
                format!(
                    "LDFLAGS={}",
                    if ubsan { "-fsanitize=undefined" } else { "" }
                ),
            ],
        ),
        ("make", vec!["-j2".into()]),
        (
            "gcc",
            [
                "-D_DEFAULT_SOURCE",
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-fsanitize=undefined",
                "-fno-sanitize-recover=all",
                "md-sim-regression.c",
                "-o",
                "/work/cgminer-regression",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
        ),
        ("/work/cgminer-regression", vec![]),
    ];
    for (program, args) in &commands {
        let status = Command::new(program)
            .args(args)
            .current_dir(&root)
            .env_clear()
            .env("PATH", "/usr/bin")
            .env("HOME", "/tmp")
            .env("LC_ALL", "C")
            .env("SOURCE_DATE_EPOCH", "1700000000")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone()?))
            .stderr(Stdio::from(log.try_clone()?))
            .status()?;
        if !status.success() {
            let log = fs::read("/work/build.log")?;
            let message = String::from_utf8_lossy(&log[log.len().saturating_sub(65536)..]);
            anyhow::bail!("build step {program} failed: {status}\n{message}");
        }
    }
    verify_members(Path::new("/work"), &adaptation, true)?;
    let mut toolchain = std::collections::BTreeMap::new();
    for program in [
        "gcc",
        "as",
        "ld",
        "make",
        "patch",
        "autoconf",
        "automake",
        "autoreconf",
        "perl",
        "pkg-config",
        "sh",
    ] {
        toolchain.insert(program, hash_file(&Path::new("/usr/bin").join(program))?);
    }
    let binary = fs::read(root.join("cgminer"))?;
    let binary_hash = hash_file(&root.join("cgminer"))?;
    let log = fs::read_to_string("/work/build.log")?;
    println!(
        "{}",
        json!({"binary_hex":hex::encode(binary),"build_log":log,"manifest":{
            "source_artifact":"open-ckolivas-cgminer-source","source_sha256":SOURCE_SHA256,
            "source_commit":"b8491c66e7e22f23a9edf095dd1337ee581e88bd","patch_sha256":hash_bytes(PATCH),
            "adaptation_manifest_sha256":hash_bytes(ADAPTATION),
            "firmware_binary":binary_hash,"execution_mode":"source-adapter","firmware_identity":"rebuilt-source",
            "boot_entry":"process","ubsan":ubsan,"commands":commands,"toolchain_programs":toolchain,
            "c_regression":{"source_sha256":hash_bytes(REGRESSION),"binary":hash_file(Path::new("/work/cgminer-regression"))?,"ubsan":true,"passed":true},
            "toolchain_limit":"Read-only installed /usr supplies headers and build dependencies. A frozen toolchain image is still required for a reproducible release.",
            "hardware_identity":"source-derived-controller-interface-fixture-not-an-exact-board"
        }})
    );
    Ok(())
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

/// The build receives no host home, credential, network, archive directory, or
/// writable host bind. Its only writable storage is a bounded scratch tmpfs.
pub fn prepare(repo: &Path, out: &Path, ubsan: bool) -> Result<()> {
    ensure!(!out.exists(), "preparation output must not exist");
    let archive = repo
        .join("artifacts/files/source/open-ckolivas-cgminer-source/cgminer-b8491c66e7e2.tar.gz")
        .canonicalize()?;
    ensure!(
        hash_file(&archive)?.sha256 == SOURCE_SHA256,
        "source archive SHA-256 mismatch"
    );
    let executable = std::env::current_exe()?.canonicalize()?;
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
            "--new-session",
            "--unshare-ipc",
            "--unshare-uts",
            "--clearenv",
            "--ro-bind",
            "/usr",
            "/usr",
            "--symlink",
            "usr/lib",
            "/lib",
            "--symlink",
            "usr/lib64",
            "/lib64",
            "--symlink",
            "usr/bin",
            "/bin",
            "--proc",
            "/proc",
            "--dev",
            "/dev",
            "--size",
            "536870912",
            "--tmpfs",
            "/work",
            "--size",
            "67108864",
            "--tmpfs",
            "/tmp",
        ])
        .arg("--ro-bind")
        .arg(archive)
        .arg("/source.tar.gz")
        .arg("--ro-bind")
        .arg(executable)
        .arg("/prepare")
        .args([
            "--cap-drop",
            "ALL",
            "--remount-ro",
            "/",
            "--setenv",
            "PATH",
            "/usr/bin",
            "--chdir",
            "/work",
            "/prepare",
            "__prepare",
        ]);
    if ubsan {
        command.arg("--ubsan");
    }
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            for (kind, value) in [
                (libc::RLIMIT_CPU, 120),
                (libc::RLIMIT_AS, 2 * 1024 * 1024 * 1024),
                (libc::RLIMIT_NPROC, 256),
                (libc::RLIMIT_FSIZE, 64 * 1024 * 1024),
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
    let mut child = BuildChild(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let stdout = child.0.stdout.take().context("missing build stdout")?;
    let stderr = child.0.stderr.take().context("missing build stderr")?;
    let collect = |pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || -> Result<Vec<u8>> {
            let mut b = Vec::new();
            pipe.take(32 * 1024 * 1024 + 1).read_to_end(&mut b)?;
            ensure!(b.len() <= 32 * 1024 * 1024, "build export size limit");
            Ok(b)
        })
    };
    let stdout = collect(Box::new(stdout));
    let stderr = collect(Box::new(stderr));
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        if start.elapsed() > Duration::from_secs(180) {
            unsafe {
                libc::kill(-(child.0.id() as i32), libc::SIGKILL);
            };
            let _ = child.0.kill();
            break child.0.wait()?;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = stdout
        .join()
        .map_err(|_| anyhow::anyhow!("build stdout panicked"))??;
    let stderr = stderr
        .join()
        .map_err(|_| anyhow::anyhow!("build stderr panicked"))??;
    fs::create_dir_all(out)?;
    fs::write(out.join("preparation-stderr.log"), &stderr)?;
    ensure!(
        status.success(),
        "isolated preparation failed: {status}; {}",
        String::from_utf8_lossy(&stderr)
    );
    let envelope: serde_json::Value = serde_json::from_slice(&stdout)?;
    let binary = hex::decode(
        envelope["binary_hex"]
            .as_str()
            .context("missing build binary")?,
    )?;
    ensure!(
        hash_bytes(&binary) == envelope["manifest"]["firmware_binary"]["sha256"],
        "build export checksum mismatch"
    );
    fs::write(out.join("cgminer"), binary)?;
    fs::set_permissions(out.join("cgminer"), fs::Permissions::from_mode(0o500))?;
    fs::write(
        out.join("build.log"),
        envelope["build_log"]
            .as_str()
            .context("missing build log")?,
    )?;
    fs::write(
        out.join("input-manifest.json"),
        serde_json::to_vec_pretty(&envelope["manifest"])?,
    )?;
    fs::write(out.join("adaptation.patch"), PATCH)?;
    fs::write(out.join("adaptation-manifest.json"), ADAPTATION)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn member_provenance_checks_parent_bytes_and_absent_new_files() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join(SOURCE_ROOT);
        fs::create_dir(&source).unwrap();
        fs::write(source.join("unit.c"), b"original").unwrap();
        let manifest = json!({"files":[
            {"member":format!("{SOURCE_ROOT}/unit.c"),"original_sha256":hash_bytes(b"original"),"adapted_sha256":hash_bytes(b"adapted")},
            {"member":format!("{SOURCE_ROOT}/new.h"),"original_sha256":null,"adapted_sha256":hash_bytes(b"new")}
        ]});
        verify_members(root.path(), &manifest, false).unwrap();
        fs::write(source.join("new.h"), b"new").unwrap();
        assert!(verify_members(root.path(), &manifest, false).is_err());
        fs::write(source.join("unit.c"), b"adapted").unwrap();
        verify_members(root.path(), &manifest, true).unwrap();
        fs::write(source.join("unit.c"), b"tampered").unwrap();
        assert!(verify_members(root.path(), &manifest, true).is_err());
    }
}

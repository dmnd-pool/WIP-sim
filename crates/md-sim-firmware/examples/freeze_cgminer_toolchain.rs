//! Freeze installed package bytes, then rebuild with only the captured /usr.
//! Package metadata selects a conservative input set. A successful isolated
//! replay, not the package dependency graph, establishes this recipe's closure.
use anyhow::{bail, ensure, Context, Result};
use md_sim_firmware::prepare::{hash_bytes, ADAPTATION, PATCH, SOURCE_SHA256};
use md_sim_runtime::store::ObjectId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

const MAX_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_ENTRIES: usize = 60_000;
const ROOTS: &str = "gcc binutils make patch autoconf automake perl pkgconf bash coreutils sed grep gawk m4 diffutils findutils libtool curl libusb ncurses file which";

fn bounded_identity(path: &Path) -> Result<ObjectId> {
    use sha2::{Digest, Sha256};
    const MAX_FILE: u64 = 128 * 1024 * 1024;
    let mut input = File::open(path)
        .with_context(|| format!("open {}", path.display()))?
        .take(MAX_FILE + 1);
    ensure!(
        input.get_ref().metadata()?.is_file(),
        "only regular files may be hashed"
    );
    let mut digest = Sha256::new();
    let mut bytes = 0u64;
    let mut chunk = [0; 65536];
    loop {
        let n = input.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        bytes += n as u64;
        ensure!(bytes <= MAX_FILE, "per-file input bound exceeded");
        digest.update(&chunk[..n]);
    }
    Ok(ObjectId {
        sha256: hex::encode(digest.finalize()),
        size: bytes,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Entry {
    kind: String,
    mode: u32,
    object: Option<ObjectId>,
    link: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    schema: u32,
    method: String,
    max_bytes: u64,
    max_entries: usize,
    bytes: u64,
    roots: Vec<String>,
    packages: BTreeMap<String, Value>,
    entries: BTreeMap<String, Entry>,
}

struct Package {
    directory: PathBuf,
    fields: BTreeMap<String, Vec<String>>,
}

fn bounded(path: &Path, max: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(max + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= max,
        "input exceeds bound: {}",
        path.display()
    );
    Ok(bytes)
}

fn sections(bytes: &[u8]) -> Result<BTreeMap<String, Vec<String>>> {
    let text = std::str::from_utf8(bytes)?;
    let mut out = BTreeMap::new();
    let mut section = None;
    for line in text.lines() {
        if line.starts_with('%') && line.ends_with('%') {
            section = Some(line.trim_matches('%').to_string());
        } else if !line.is_empty() {
            if let Some(name) = &section {
                out.entry(name.clone())
                    .or_insert_with(Vec::new)
                    .push(line.to_string());
            }
        }
    }
    Ok(out)
}

fn name(requirement: &str) -> &str {
    requirement.split(['<', '>', '=']).next().unwrap_or("")
}

fn safe_relative(path: &str) -> Result<PathBuf> {
    ensure!(path.len() <= 4096, "input path length limit");
    let p = PathBuf::from(path.trim_end_matches('/'));
    ensure!(
        p.starts_with("usr") && p.components().all(|c| matches!(c, Component::Normal(_))),
        "input path is outside approved /usr: {path}"
    );
    Ok(p)
}

fn package_set() -> Result<BTreeMap<String, Package>> {
    let mut all = BTreeMap::new();
    for item in fs::read_dir("/var/lib/pacman/local")? {
        let directory = item?.path();
        if !directory.is_dir() {
            continue;
        }
        ensure!(all.len() < 4096, "package database count limit");
        let fields = sections(&bounded(&directory.join("desc"), 65536)?)?;
        let package_name = fields
            .get("NAME")
            .and_then(|v| v.first())
            .context("package name absent")?
            .clone();
        ensure!(
            all.insert(package_name, Package { directory, fields })
                .is_none(),
            "duplicate installed package"
        );
    }
    let mut selected = BTreeSet::new();
    let mut queue: Vec<String> = ROOTS.split_whitespace().map(str::to_string).collect();
    while let Some(requirement) = queue.pop() {
        let dep = name(&requirement);
        let candidates: Vec<String> = if all.contains_key(dep) {
            vec![dep.to_string()]
        } else {
            all.iter()
                .filter(|(_, p)| {
                    p.fields.get("PROVIDES").is_some_and(|values| {
                        values.iter().any(|v| {
                            name(v) == dep && (!requirement.contains('=') || v == &requirement)
                        })
                    })
                })
                .map(|(n, _)| n.clone())
                .collect()
        };
        ensure!(
            !candidates.is_empty(),
            "unresolved installed package requirement: {requirement}"
        );
        // Preserve all matching installed providers. Do not guess an ABI from a filename.
        for candidate in candidates {
            if selected.insert(candidate.clone()) {
                queue.extend(
                    all[&candidate]
                        .fields
                        .get("DEPENDS")
                        .cloned()
                        .unwrap_or_default(),
                );
            }
        }
    }
    Ok(all
        .into_iter()
        .filter(|(n, _)| selected.contains(n))
        .collect())
}

fn capture(out: &Path) -> Result<()> {
    ensure!(!out.exists(), "snapshot output must be new");
    let packages = package_set()?;
    let mut paths = BTreeSet::new();
    let mut metadata = BTreeMap::new();
    for (name, package) in &packages {
        let files = bounded(&package.directory.join("files"), 2 * 1024 * 1024)?;
        let fields = sections(&files)?;
        // Installed metapackages can have dependencies and no owned files.
        let members = fields.get("FILES").cloned().unwrap_or_default();
        let mut excluded = Vec::new();
        for member in &members {
            if member.starts_with("usr/share/factory/") {
                // System installation defaults can include protected credential
                // templates. They are outside this build-input capture policy.
                excluded.push(member.clone());
            } else if member == "usr/" || member.starts_with("usr/") {
                paths.insert(safe_relative(member)?);
                ensure!(paths.len() <= MAX_ENTRIES, "snapshot entry budget exceeded");
            } else {
                excluded.push(member.clone());
            }
        }
        metadata.insert(name.clone(), json!({
            "fields":package.fields,"desc":bounded_identity(&package.directory.join("desc"))?,
            "files":bounded_identity(&package.directory.join("files"))?,
            "excluded_not_mounted":excluded,
            "capture_policy":"Only /usr package entries, excluding /usr/share/factory installation defaults. Excluded inputs are unavailable during replay.",
            "version_constraints":"Retained metadata, not an independent package version compatibility claim. Closed-input replay is required."
        }));
    }
    // Include each parent in the manifest, including directories shared with packages outside the set.
    for path in paths.clone() {
        for parent in path
            .ancestors()
            .skip(1)
            .filter(|p| !p.as_os_str().is_empty())
        {
            paths.insert(parent.to_path_buf());
            ensure!(paths.len() <= MAX_ENTRIES, "snapshot entry budget exceeded");
        }
    }
    ensure!(paths.len() <= MAX_ENTRIES, "snapshot entry budget exceeded");
    let tree = out.join("tree");
    fs::create_dir_all(&tree)?;
    let mut entries = BTreeMap::new();
    let mut bytes = 0u64;
    // Files precede symlinks, so no copy can traverse a newly created link.
    let mut links = Vec::new();
    for relative in &paths {
        let source = Path::new("/").join(relative);
        let stat = fs::symlink_metadata(&source)
            .with_context(|| format!("missing installed package input: {}", source.display()))?;
        let destination = tree.join(relative);
        let key = relative
            .to_str()
            .context("non-UTF8 package path")?
            .to_string();
        if stat.file_type().is_symlink() {
            let link = fs::read_link(&source)?;
            ensure!(link.as_os_str().len() <= 4096, "symlink target limit");
            links.push((destination, link.clone()));
            entries.insert(
                key,
                Entry {
                    kind: "symlink".into(),
                    mode: 0o777,
                    object: None,
                    link: Some(link),
                },
            );
        } else if stat.is_dir() {
            fs::create_dir_all(&destination)?;
            entries.insert(
                key,
                Entry {
                    kind: "directory".into(),
                    mode: 0o555,
                    object: None,
                    link: None,
                },
            );
        } else {
            ensure!(
                stat.is_file(),
                "special package input prohibited: {}",
                source.display()
            );
            ensure!(
                source.canonicalize()?.starts_with("/usr"),
                "package file resolves outside /usr"
            );
            bytes = bytes
                .checked_add(stat.len())
                .context("snapshot byte overflow")?;
            ensure!(
                bytes <= MAX_BYTES && stat.len() <= 128 * 1024 * 1024,
                "snapshot byte budget exceeded"
            );
            fs::create_dir_all(destination.parent().context("input parent absent")?)?;
            let before = bounded_identity(&source)?;
            let mut input = File::open(&source)?.take(stat.len() + 1);
            let mut output = File::options()
                .write(true)
                .create_new(true)
                .open(&destination)?;
            ensure!(
                std::io::copy(&mut input, &mut output)? == stat.len(),
                "source size changed during bounded capture"
            );
            ensure!(
                bounded_identity(&destination)? == before && bounded_identity(&source)? == before,
                "input changed during capture"
            );
            let mode = if stat.permissions().mode() & 0o111 != 0 {
                0o555
            } else {
                0o444
            };
            fs::set_permissions(&destination, fs::Permissions::from_mode(mode))?;
            entries.insert(
                key,
                Entry {
                    kind: "file".into(),
                    mode,
                    object: Some(before),
                    link: None,
                },
            );
        }
    }
    for (destination, link) in links {
        // External link targets stay unavailable in the replay; they are never copied.
        std::os::unix::fs::symlink(link, destination)?;
    }
    for (relative, entry) in entries.iter().rev() {
        if entry.kind == "directory" {
            fs::set_permissions(tree.join(relative), fs::Permissions::from_mode(entry.mode))?;
        }
    }
    let snapshot = Snapshot { schema:1, method:"installed Arch package files and transitive dependency providers; conservative /usr snapshot requiring closed-input replay".into(), max_bytes:MAX_BYTES, max_entries:MAX_ENTRIES, bytes, roots:ROOTS.split_whitespace().map(str::to_string).collect(), packages:metadata, entries };
    fs::write(
        out.join("manifest.json"),
        serde_json::to_vec_pretty(&snapshot)?,
    )?;
    fs::write(
        out.join("manifest.sha256"),
        format!("{}\n", bounded_identity(&out.join("manifest.json"))?.sha256),
    )?;
    verify(out)?;
    println!(
        "{}",
        json!({"snapshot":out,"manifest":bounded_identity(&out.join("manifest.json"))?,"packages":snapshot.packages.len(),"entries":snapshot.entries.len(),"bytes":bytes,"replay":"not yet executed"})
    );
    Ok(())
}

fn verify(out: &Path) -> Result<ObjectId> {
    for path in [out.join("manifest.json"), out.join("manifest.sha256")] {
        let metadata = fs::symlink_metadata(path)?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "snapshot metadata must be regular files"
        );
    }
    let identity = bounded_identity(&out.join("manifest.json"))?;
    ensure!(
        String::from_utf8(bounded(&out.join("manifest.sha256"), 128)?)?.trim() == identity.sha256,
        "snapshot manifest hash mismatch"
    );
    let snapshot: Snapshot =
        serde_json::from_slice(&bounded(&out.join("manifest.json"), 32 * 1024 * 1024)?)?;
    ensure!(
        snapshot.schema == 1
            && snapshot.max_bytes <= MAX_BYTES
            && snapshot.max_entries <= MAX_ENTRIES
            && snapshot.entries.len() <= snapshot.max_entries,
        "unsupported snapshot or budget"
    );
    for path in [out.join("tree"), out.join("tree/usr")] {
        let metadata = fs::symlink_metadata(path)?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "snapshot roots must be real directories"
        );
    }
    ensure!(
        snapshot
            .entries
            .get("usr")
            .is_some_and(|entry| entry.kind == "directory"),
        "snapshot /usr root must be a declared directory"
    );
    let tree = out.join("tree").canonicalize()?;
    let mut visited = BTreeSet::new();
    let mut queue = vec![tree.join("usr")];
    let mut bytes = 0u64;
    while let Some(path) = queue.pop() {
        let relative = path
            .strip_prefix(&tree)?
            .to_str()
            .context("non-UTF8 snapshot path")?
            .to_string();
        safe_relative(&relative)?;
        ensure!(
            visited.insert(relative.clone()) && visited.len() <= MAX_ENTRIES,
            "snapshot traversal budget"
        );
        let entry = snapshot
            .entries
            .get(&relative)
            .context("unrecorded snapshot input")?;
        let stat = fs::symlink_metadata(&path)?;
        match entry.kind.as_str() {
            "symlink" => ensure!(
                stat.file_type().is_symlink()
                    && Some(fs::read_link(&path)?) == entry.link
                    && entry.object.is_none(),
                "snapshot symlink mismatch"
            ),
            "directory" => {
                ensure!(
                    stat.is_dir()
                        && !stat.file_type().is_symlink()
                        && entry.object.is_none()
                        && entry.link.is_none(),
                    "snapshot directory mismatch"
                );
                ensure!(
                    stat.permissions().mode() & 0o7777 == entry.mode && entry.mode == 0o555,
                    "snapshot directory permission mismatch"
                );
                for child in fs::read_dir(&path)? {
                    ensure!(
                        visited.len() + queue.len() < snapshot.max_entries,
                        "snapshot traversal queue budget"
                    );
                    queue.push(child?.path());
                }
            }
            "file" => {
                ensure!(
                    stat.is_file() && !stat.file_type().is_symlink() && entry.link.is_none(),
                    "snapshot regular file mismatch"
                );
                ensure!(
                    matches!(entry.mode, 0o444 | 0o555)
                        && stat.permissions().mode() & 0o7777 == entry.mode,
                    "snapshot file permission mismatch"
                );
                bytes = bytes
                    .checked_add(stat.len())
                    .context("snapshot size overflow")?;
                ensure!(
                    bytes <= snapshot.max_bytes && Some(bounded_identity(&path)?) == entry.object,
                    "snapshot file hash or byte budget mismatch"
                );
            }
            _ => bail!("unsupported snapshot entry"),
        }
    }
    ensure!(
        visited.len() == snapshot.entries.len() && bytes == snapshot.bytes,
        "snapshot is incomplete"
    );
    Ok(identity)
}

struct Child(std::process::Child);
impl Drop for Child {
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

struct Captured {
    bytes: Vec<u8>,
    truncated: bool,
    error: Option<String>,
}

fn capture_output(mut stream: impl Read, limit: usize) -> Captured {
    let mut bytes = Vec::new();
    let error = stream
        .by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .err()
        .map(|e| e.to_string());
    let truncated = bytes.len() > limit;
    bytes.truncate(limit);
    Captured {
        bytes,
        truncated,
        error,
    }
}

fn build(repo: &Path, snapshot: &Path, out: &Path, ubsan: bool) -> Result<()> {
    ensure!(!out.exists(), "build output must be new");
    let snapshot = snapshot.canonicalize()?;
    let closure = verify(&snapshot)?;
    let archive = repo
        .join("artifacts/files/source/open-ckolivas-cgminer-source/cgminer-b8491c66e7e2.tar.gz")
        .canonicalize()?;
    ensure!(
        bounded_identity(&archive)?.sha256 == SOURCE_SHA256,
        "source archive identity differs"
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
        ])
        .arg(snapshot.join("tree/usr"))
        .arg("/usr")
        .args([
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
            "--ro-bind",
        ])
        .arg(&archive)
        .arg("/source.tar.gz")
        .arg("--ro-bind")
        .arg(&executable)
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
            "__inside",
        ]);
    if ubsan {
        command.arg("--ubsan");
    }
    let supervisor_pid = unsafe { libc::getpid() };
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() != supervisor_pid {
                return Err(std::io::Error::from_raw_os_error(libc::ECANCELED));
            }
            if libc::setpgid(0, 0) != 0 || libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            for (kind, value) in [
                (libc::RLIMIT_CPU, 120),
                (libc::RLIMIT_AS, 2 * 1024 * 1024 * 1024),
                (libc::RLIMIT_NPROC, 256),
                (libc::RLIMIT_FSIZE, 64 * 1024 * 1024),
                (libc::RLIMIT_NOFILE, 256),
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
    fs::create_dir_all(out)?;
    fs::write(
        out.join("closure-manifest.json"),
        bounded(&snapshot.join("manifest.json"), 32 * 1024 * 1024)?,
    )?;
    fs::write(
        out.join("preparation-inputs.json"),
        serde_json::to_vec_pretty(
            &json!({"toolchain_closure":closure,"recipe_build":bounded_identity(&executable)?,"source":bounded_identity(&archive)?,"ubsan":ubsan,"read_only_usr_source":"captured tree only; no installed /usr mount","limits":{"wall_seconds":180,"cpu_seconds":120,"address_bytes":2u64*1024*1024*1024,"scratch_bytes":536870912,"temporary_bytes":67108864,"max_processes":256},"supervisor":{"unshare":bounded_identity(Path::new("/usr/bin/unshare"))?,"bwrap":bounded_identity(Path::new("/usr/bin/bwrap"))?}}),
        )?,
    )?;
    let mut child = Child(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let collect = |stream: Box<dyn Read + Send>| {
        std::thread::spawn(move || capture_output(stream, 32 * 1024 * 1024))
    };
    let stdout = collect(Box::new(child.0.stdout.take().context("stdout absent")?));
    let stderr = collect(Box::new(child.0.stderr.take().context("stderr absent")?));
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.0.try_wait()? {
            break status;
        }
        if start.elapsed() >= Duration::from_secs(180) {
            unsafe {
                libc::kill(-(child.0.id() as i32), libc::SIGKILL);
            }
            let _ = child.0.kill();
            break child.0.wait()?;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = stdout.join().unwrap_or_else(|_| Captured {
        bytes: vec![],
        truncated: false,
        error: Some("stdout collector panicked".into()),
    });
    let stderr = stderr.join().unwrap_or_else(|_| Captured {
        bytes: vec![],
        truncated: false,
        error: Some("stderr collector panicked".into()),
    });
    fs::write(out.join("preparation-stderr.log"), &stderr.bytes)?;
    let complete =
        !stdout.truncated && !stderr.truncated && stdout.error.is_none() && stderr.error.is_none();
    if !status.success() || !complete {
        fs::write(out.join("preparation-incomplete-stdout.bin"), &stdout.bytes)?;
    }
    fs::write(
        out.join("preparation-status.json"),
        serde_json::to_vec_pretty(
            &json!({"code":status.code(),"elapsed_seconds":start.elapsed().as_secs_f64(),"stdout":hash_bytes(&stdout.bytes),"stderr":hash_bytes(&stderr.bytes),"evidence_complete":complete,"stdout_truncated":stdout.truncated,"stderr_truncated":stderr.truncated,"stdout_error":stdout.error,"stderr_error":stderr.error}),
        )?,
    )?;
    ensure!(
        verify(&snapshot)? == closure,
        "snapshot changed during replay"
    );
    ensure!(
        complete,
        "build evidence export incomplete; bounded diagnostics retained"
    );
    ensure!(
        status.success(),
        "closed-input build failed: {status}: {}",
        String::from_utf8_lossy(&stderr.bytes)
    );
    let mut envelope: Value = serde_json::from_slice(&stdout.bytes)?;
    let binary = hex::decode(
        envelope["binary_hex"]
            .as_str()
            .context("binary export absent")?,
    )?;
    ensure!(
        hash_bytes(&binary) == envelope["manifest"]["firmware_binary"]["sha256"],
        "binary export hash mismatch"
    );
    let expected = if ubsan {
        "0b18f7f5aa273df38e204690883cf948b6d23be0e294085719b87b9ff6358326"
    } else {
        "88602d125e46310d0e61f64622d50be25df285085c2fe09c63070d852c64a90a"
    };
    let identical = hash_bytes(&binary) == expected;
    fs::write(out.join("cgminer"), binary)?;
    fs::set_permissions(out.join("cgminer"), fs::Permissions::from_mode(0o500))?;
    fs::write(
        out.join("build.log"),
        envelope["build_log"].as_str().context("build log absent")?,
    )?;
    envelope["manifest"]["toolchain_limit"] = Value::Null;
    envelope["manifest"]["toolchain_closure"] = serde_json::to_value(&closure)?;
    envelope["manifest"]["toolchain_scope"] = json!("Closed-input replay of this exact cgminer recipe and variant using the captured installed package bytes; not a minimal closure or package-source reproducibility claim.");
    envelope["manifest"]["matches_v10_firmware_bytes"] = json!(identical);
    fs::write(
        out.join("input-manifest.json"),
        serde_json::to_vec_pretty(&envelope["manifest"])?,
    )?;
    fs::write(out.join("adaptation.patch"), PATCH)?;
    fs::write(out.join("adaptation-manifest.json"), ADAPTATION)?;
    ensure!(
        identical,
        "closed-input build differs from validated v10 binary; investigate the changed inputs"
    );
    println!(
        "{}",
        json!({"output":out,"binary":bounded_identity(&out.join("cgminer"))?,"closure":closure,"c_regression":envelope["manifest"]["c_regression"],"matches_v10_firmware_bytes":true})
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    match args.get(1).and_then(|v| v.to_str()) {
        Some("__inside") if args.len() == 2 || (args.len() == 3 && args[2] == "--ubsan") => md_sim_firmware::prepare::inside(args.len() == 3),
        Some("capture") if args.len() == 3 => capture(Path::new(&args[2])),
        Some("verify") if args.len() == 3 => { println!("{}",serde_json::to_string(&verify(Path::new(&args[2]))?)?); Ok(()) },
        Some("build") if args.len() == 5 || (args.len() == 6 && args[5] == "--ubsan") => build(Path::new(&args[2]),Path::new(&args[3]),Path::new(&args[4]),args.len() == 6),
        _ => bail!("usage: freeze_cgminer_toolchain capture NEW_SNAPSHOT | verify SNAPSHOT | build REPO SNAPSHOT NEW_OUTPUT [--ubsan]"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn package_paths_reject_other_roots_and_traversal() {
        for bad in [
            "etc/shadow",
            "/usr/bin/gcc",
            "usr/../etc/passwd",
            "usr/bin/../../home/key",
            "usr-other/bin/a",
        ] {
            assert!(safe_relative(bad).is_err());
        }
        assert_eq!(
            safe_relative("usr/include/").unwrap(),
            Path::new("usr/include")
        );
    }

    #[test]
    fn replay_verification_rejects_changed_missing_and_extra_inputs() {
        let temp = tempfile::tempdir().unwrap();
        let tree = temp.path().join("tree");
        let directory = tree.join("usr/bin");
        fs::create_dir_all(&directory).unwrap();
        let file = directory.join("tool");
        fs::write(&file, b"tool").unwrap();
        let mut entries = BTreeMap::new();
        for path in ["usr", "usr/bin"] {
            entries.insert(
                path.into(),
                Entry {
                    kind: "directory".into(),
                    mode: 0o555,
                    object: None,
                    link: None,
                },
            );
            fs::set_permissions(tree.join(path), fs::Permissions::from_mode(0o555)).unwrap();
        }
        fs::set_permissions(&file, fs::Permissions::from_mode(0o555)).unwrap();
        entries.insert(
            "usr/bin/tool".into(),
            Entry {
                kind: "file".into(),
                mode: 0o555,
                object: Some(bounded_identity(&file).unwrap()),
                link: None,
            },
        );
        let snapshot = Snapshot {
            schema: 1,
            method: "test".into(),
            max_bytes: 32,
            max_entries: 3,
            bytes: 4,
            roots: vec![],
            packages: BTreeMap::new(),
            entries,
        };
        fs::write(
            temp.path().join("manifest.json"),
            serde_json::to_vec(&snapshot).unwrap(),
        )
        .unwrap();
        fs::write(
            temp.path().join("manifest.sha256"),
            bounded_identity(&temp.path().join("manifest.json"))
                .unwrap()
                .sha256,
        )
        .unwrap();
        verify(temp.path()).unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o4555)).unwrap();
        assert!(verify(temp.path()).is_err());
        fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(&file, b"evil").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o555)).unwrap();
        assert!(verify(temp.path()).is_err());
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
        fs::remove_file(&file).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o555)).unwrap();
        assert!(verify(temp.path()).is_err());
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(&file, b"tool").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o555)).unwrap();
        fs::write(directory.join("unrecorded"), b"extra").unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o555)).unwrap();
        assert!(verify(temp.path())
            .unwrap_err()
            .to_string()
            .contains("queue budget"));
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(tree.join("usr"), fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn replay_rejects_usr_root_symlink_even_with_consistent_manifest() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("tree")).unwrap();
        std::os::unix::fs::symlink("/usr", temp.path().join("tree/usr")).unwrap();
        let snapshot = Snapshot {
            schema: 1,
            method: "test".into(),
            max_bytes: 32,
            max_entries: 10,
            bytes: 0,
            roots: vec![],
            packages: BTreeMap::new(),
            entries: BTreeMap::from([(
                "usr".into(),
                Entry {
                    kind: "symlink".into(),
                    mode: 0o777,
                    object: None,
                    link: Some(PathBuf::from("/usr")),
                },
            )]),
        };
        fs::write(
            temp.path().join("manifest.json"),
            serde_json::to_vec(&snapshot).unwrap(),
        )
        .unwrap();
        fs::write(
            temp.path().join("manifest.sha256"),
            bounded_identity(&temp.path().join("manifest.json"))
                .unwrap()
                .sha256,
        )
        .unwrap();
        assert!(verify(temp.path())
            .unwrap_err()
            .to_string()
            .contains("real directories"));
    }

    #[test]
    fn evidence_limit_retains_partial_diagnostics() {
        let captured = capture_output(&b"partial error log"[..], 7);
        assert_eq!(captured.bytes, b"partial");
        assert!(captured.truncated);
        assert!(captured.error.is_none());
    }
}

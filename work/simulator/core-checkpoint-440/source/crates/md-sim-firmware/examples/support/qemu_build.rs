//! Shared bounded input capture and isolated-build helpers. Callers must invoke
//! firmware confinement before unpacking or executing acquired build tools.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{isolation::save_json, store::hash_file};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
};
pub fn unpack(path: &str, root: &str) -> Result<()> {
    let mut archive = tar::Archive::new(
        flate2::read::GzDecoder::new(fs::File::open(path)?).take(2 * 1024 * 1024 * 1024),
    );
    let mut total = 0u64;
    for (i, e) in archive.entries()?.enumerate() {
        ensure!(i < 100_000, "entry budget");
        let mut e = e?;
        if e.header().entry_type().is_pax_global_extensions() {
            continue;
        }
        total = total.checked_add(e.size()).context("size overflow")?;
        ensure!(
            e.size() <= 128 * 1024 * 1024 && total <= 1500 * 1024 * 1024,
            "expansion bound"
        );
        let d = e.header().entry_type().is_dir();
        let p = Path::new(root).join(e.path()?);
        ensure!(e.unpack_in(root)?, "archive path escape");
        if d {
            fs::set_permissions(p, fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(())
}

pub fn step(report: &mut serde_json::Value, name: &str, mut command: Command) -> Result<bool> {
    ensure!(
        name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
        "invalid step name"
    );
    // The enclosing worker supplies a file-size limit and wall deadline.
    // Child diagnostics never accumulate without a bound in this process.
    let stdout = format!("/work/{name}.stdout.log");
    let stderr = format!("/work/{name}.stderr.log");
    command.stdout(std::process::Stdio::from(fs::File::create(&stdout)?));
    command.stderr(std::process::Stdio::from(fs::File::create(&stderr)?));
    let status = command.status()?;
    let mut row = json!({"code":status.code()});
    for (key, path) in [("stdout", stdout), ("stderr", stderr)] {
        let mut bytes = vec![];
        fs::File::open(&path)?
            .take(2 * 1024 * 1024)
            .read_to_end(&mut bytes)?;
        let object = hash_file(Path::new(&path))?;
        row[key] = json!(String::from_utf8_lossy(&bytes));
        row[format!("{key}_truncated")] = json!(object.size > bytes.len() as u64);
        row[format!("{key}_identity")] = serde_json::to_value(object)?;
    }
    report[name] = row;
    Ok(status.success())
}

pub fn pack_dir(path: &Path, prefix: &str, out: &Path) -> Result<()> {
    let enc = flate2::write::GzEncoder::new(fs::File::create(out)?, flate2::Compression::fast());
    let mut t = tar::Builder::new(enc);
    t.follow_symlinks(false);
    t.append_dir_all(prefix, path)?;
    t.into_inner()?.finish()?.flush()?;
    Ok(())
}

fn bounded_text(path: &Path, max: u64) -> Result<String> {
    let mut value = String::new();
    fs::File::open(path)?
        .take(max + 1)
        .read_to_string(&mut value)?;
    ensure!(value.len() as u64 <= max, "metadata bound");
    Ok(value)
}
fn field(text: &str, section: &str) -> Vec<String> {
    text.lines()
        .skip_while(|line| *line != section)
        .skip(1)
        .take_while(|line| !line.starts_with('%'))
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}
fn dependency_name(value: &str) -> &str {
    value.split(['=', '<', '>']).next().unwrap_or(value)
}
pub fn package_closure(roots: &BTreeSet<&str>) -> Result<BTreeMap<String, (PathBuf, String)>> {
    let mut all = BTreeMap::new();
    for entry in fs::read_dir("/var/lib/pacman/local")? {
        let path = entry?.path();
        if !path.is_dir() {
            continue;
        }
        ensure!(all.len() < 4096, "package database bound");
        let text = bounded_text(&path.join("desc"), 65536)?;
        let names = field(&text, "%NAME%");
        let name = names.first().context("package name absent")?.clone();
        ensure!(
            all.insert(name, (path, text)).is_none(),
            "duplicate package"
        );
    }
    let mut selected = BTreeSet::new();
    let mut queue: Vec<String> = roots.iter().map(|s| s.to_string()).collect();
    while let Some(requirement) = queue.pop() {
        let name = dependency_name(&requirement);
        let candidates: Vec<String> = if all.contains_key(name) {
            vec![name.into()]
        } else {
            all.iter()
                .filter(|(_, (_, text))| {
                    field(text, "%PROVIDES%").iter().any(|v| {
                        dependency_name(v) == name
                            && (!requirement.contains('=') || v == &requirement)
                    })
                })
                .map(|(name, _)| name.clone())
                .collect()
        };
        ensure!(!candidates.is_empty(), "unresolved package {requirement}");
        for candidate in candidates {
            if selected.insert(candidate.clone()) {
                queue.extend(field(&all[&candidate].1, "%DEPENDS%"));
                ensure!(queue.len() < 65536, "dependency queue bound");
            }
        }
    }
    Ok(all
        .into_iter()
        .filter(|(n, _)| selected.contains(n))
        .collect())
}
#[allow(dead_code)]
pub fn capture_overlay(out: &Path) -> Result<()> {
    capture_overlay_with(out, &[])
}
pub fn capture_overlay_with(out: &Path, extra_roots: &[&str]) -> Result<()> {
    let mut requested: BTreeSet<_> = [
        "python",
        "python-pip",
        "python-setuptools",
        "python-wheel",
        "python-packaging",
        "glib2",
        "pixman",
        "dtc",
        "libffi",
        "pcre2",
        "expat",
    ]
    .into_iter()
    .collect();
    requested.extend(extra_roots.iter().copied());
    let selected = package_closure(&requested)?;
    let mut members = BTreeSet::new();
    let mut packages = vec![];
    for (name, (p, description)) in selected {
        let files = bounded_text(&p.join("files"), 2 * 1024 * 1024)?;
        for line in files
            .lines()
            .skip_while(|s| *s != "%FILES%")
            .skip(1)
            .take_while(|s| !s.starts_with('%'))
        {
            if line.starts_with("usr/") && !line.starts_with("usr/share/factory/") {
                let relative = Path::new(line);
                ensure!(
                    relative
                        .components()
                        .all(|c| matches!(c, std::path::Component::Normal(_))),
                    "package path escape"
                );
                members.insert(relative.to_path_buf());
                ensure!(members.len() < 60_000, "overlay member budget");
            }
        }
        packages.push(json!({"name":name,"description":description,"desc":hash_file(&p.join("desc"))?,"files":hash_file(&p.join("files"))?}));
    }
    let enc = flate2::write::GzEncoder::new(
        fs::File::create(out.join("overlay.tar.gz"))?,
        flate2::Compression::fast(),
    );
    let mut tar = tar::Builder::new(enc);
    tar.follow_symlinks(false);
    let mut records = vec![];
    let mut total = 0u64;
    for member in members {
        let source = Path::new("/").join(&member);
        let m = fs::symlink_metadata(&source)?;
        ensure!(
            m.is_file() || m.is_dir() || m.file_type().is_symlink(),
            "special input rejected"
        );
        let object = if m.is_file() {
            ensure!(
                source.canonicalize()?.starts_with("/usr"),
                "input escaped usr"
            );
            total = total.checked_add(m.len()).context("size overflow")?;
            ensure!(
                total <= 1024 * 1024 * 1024 && m.len() <= 128 * 1024 * 1024,
                "overlay byte bound"
            );
            Some(hash_file(&source)?)
        } else {
            None
        };
        tar.append_path_with_name(&source, Path::new("toolchain").join(&member))?;
        if let Some(id) = &object {
            ensure!(&hash_file(&source)? == id, "input changed during capture");
        }
        records.push(json!({"path":member,"object":object,"mode":m.permissions().mode()&0o7777,"link":if m.file_type().is_symlink(){Some(fs::read_link(&source)?)}else{None}}));
    }
    tar.into_inner()?.finish()?.flush()?;
    save_json(
        &out.join("overlay-manifest.json"),
        &json!({"scope":"Only enumerated installed package files under usr; actual isolated build is closure test; no package-source reproducibility claim","packages":packages,"files":records,"file_bytes":total,"max_bytes":1024*1024*1024,"max_members":60_000}),
    )?;
    Ok(())
}

/// Reuse only exact bytes from a previous captured and locked input set.
/// Build success is not assumed: the caller must execute the complete recipe.
#[allow(dead_code)] // Optional for recipes that have not made their first capture.
pub fn reuse_overlay(prior: &Path, out: &Path) -> Result<()> {
    let spec: md_sim_runtime::isolation::ProcessSpec =
        serde_json::from_str(&bounded_text(&prior.join("spec.json"), 1024 * 1024)?)?;
    let identities: serde_json::Value = serde_json::from_str(&bounded_text(
        &prior.join("input-identities.json"),
        1024 * 1024,
    )?)?;
    let expected = &spec
        .inputs
        .iter()
        .find(|m| m.guest_path == "/overlay.tar.gz")
        .context("captured overlay identity missing")?
        .object;
    let manifest = hash_file(&prior.join("overlay-manifest.json"))?;
    ensure!(
        serde_json::to_value(&manifest)? == identities["overlay_manifest"],
        "captured manifest changed"
    );
    ensure!(
        &hash_file(&prior.join("overlay.tar.gz"))? == expected,
        "captured overlay changed"
    );
    for (name, identity) in [
        ("overlay.tar.gz", expected),
        ("overlay-manifest.json", &manifest),
    ] {
        fs::copy(prior.join(name), out.join(name))?;
        ensure!(
            &hash_file(&out.join(name))? == identity,
            "copied capture changed"
        );
    }
    Ok(())
}

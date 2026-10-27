//! Capture the declared Python runtime without unrelated installed packages.
use anyhow::{ensure, Result};
use md_sim_runtime::{
    isolation::Mount,
    store::{hash_file, Store},
};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub const PYTHON: &str = "/usr/bin/python3.14";
const STDLIB: &str = "/usr/lib/python3.14";

pub fn mount_file(
    store: &Store,
    mounts: &mut Vec<Mount>,
    path: &Path,
    guest: String,
) -> Result<()> {
    let object = hash_file(path)?;
    store.import(path, &object)?;
    mounts.push(Mount {
        object,
        guest_path: guest,
    });
    Ok(())
}

fn python_files(root: &Path, paths: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    ensure!(depth < 16, "Python directory depth bound");
    let mut entries = fs::read_dir(root)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        if name == "__pycache__" || name == "site-packages" || name == "lib-dynload" {
            continue;
        }
        let kind = entry.file_type()?;
        ensure!(!kind.is_symlink(), "unexpected Python source symlink");
        if kind.is_dir() {
            python_files(&entry.path(), paths, depth + 1)?;
        } else if kind.is_file() && entry.path().extension().is_some_and(|ext| ext == "py") {
            ensure!(paths.len() < 2048, "Python source count bound");
            paths.push(entry.path());
        }
    }
    Ok(())
}

/// Extra native modules are explicit, bounded names. Pure Python packages are
/// separate inputs; installed site-packages are never captured as a whole.
pub fn capture(store: &Store, extra_modules: &[&str]) -> Result<Vec<Mount>> {
    ensure!(
        extra_modules.len() <= 32,
        "extra Python native module bound"
    );
    let mut native = vec![
        ("/guard".into(), std::env::current_exe()?),
        (PYTHON.into(), PathBuf::from(PYTHON)),
    ];
    let dynamic: Vec<_> =
        fs::read_dir(Path::new(STDLIB).join("lib-dynload"))?.collect::<std::io::Result<_>>()?;
    let mut modules: BTreeSet<&str> = [
        "zlib", "_struct", "_hashlib", "_sha2", "_blake2", "fcntl", "math", "_random", "_bisect",
        "binascii",
    ]
    .into_iter()
    .collect();
    for &module in extra_modules {
        ensure!(
            !module.is_empty()
                && module.len() <= 64
                && module
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
            "Python native module name"
        );
        modules.insert(module);
    }
    for module in modules {
        let matches: Vec<_> = dynamic
            .iter()
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.starts_with(&format!("{module}.")) && name.ends_with(".so")
            })
            .collect();
        ensure!(
            matches.len() == 1,
            "Python native module {module} missing or ambiguous"
        );
        let path = matches[0].path();
        native.push((path.to_string_lossy().into_owned(), path));
    }
    let mut mounts = md_sim_firmware::lab::elf_closure(store, native)?;
    let mut paths = Vec::new();
    python_files(Path::new(STDLIB), &mut paths, 0)?;
    for path in paths {
        mount_file(
            store,
            &mut mounts,
            &path,
            path.to_string_lossy().into_owned(),
        )?;
    }
    let yaml = Path::new(STDLIB).join("site-packages/yaml");
    let mut paths = Vec::new();
    python_files(&yaml, &mut paths, 0)?;
    for path in paths {
        let guest = format!("/yaml-package/yaml/{}", path.strip_prefix(&yaml)?.display());
        mount_file(store, &mut mounts, &path, guest)?;
    }
    Ok(mounts)
}

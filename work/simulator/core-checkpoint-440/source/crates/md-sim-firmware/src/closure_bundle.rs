//! Deterministic transport of the captured build inputs. This does not execute
//! the archived tools and does not claim that every packaged tool is necessary.
use anyhow::{ensure, Context, Result};
use md_sim_coordinator::queue::read_json;
use md_sim_runtime::store::{hash_file, ObjectId};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Cursor, Read},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Component, Path},
};
#[derive(Deserialize)]
struct Entry {
    kind: String,
    mode: u32,
    object: Option<ObjectId>,
    link: Option<String>,
}
#[derive(Deserialize)]
struct Manifest {
    entries: BTreeMap<String, Entry>,
    bytes: u64,
}
pub fn pack(snapshot: &Path, expected: &ObjectId, output: &Path) -> Result<ObjectId> {
    ensure!(
        hash_file(&snapshot.join("manifest.json"))? == *expected,
        "closure manifest identity differs"
    );
    let manifest: Manifest = read_json(&snapshot.join("manifest.json"), 16 * 1024 * 1024)?;
    ensure!(
        manifest.entries.len() <= 60000 && manifest.bytes <= 1024 * 1024 * 1024,
        "closure exceeds transport budget"
    );
    let destination_parent = output
        .parent()
        .context("missing bundle parent")?
        .canonicalize()?;
    ensure!(
        !destination_parent.starts_with(snapshot.canonicalize()?),
        "derived bundle output must be outside captured inputs"
    );
    let tree = snapshot.join("tree");
    ensure!(
        fs::symlink_metadata(&tree)?.is_dir(),
        "closure root must be a real directory"
    );
    let tree = tree.canonicalize()?;
    let mut seen = BTreeSet::new();
    let mut queue = vec![tree.clone()];
    while let Some(dir) = queue.pop() {
        for item in fs::read_dir(dir)? {
            let item = item?;
            let path = item.path();
            let name = path
                .strip_prefix(&tree)?
                .to_str()
                .context("non-UTF8 closure path")?
                .to_owned();
            ensure!(
                seen.len() + queue.len() < 60001,
                "closure traversal count exceeded"
            );
            ensure!(
                manifest.entries.contains_key(&name),
                "unrecorded closure input: {name}"
            );
            seen.insert(name);
            if fs::symlink_metadata(&path)?.is_dir() {
                queue.push(path)
            }
        }
    }
    ensure!(
        seen.len() == manifest.entries.len(),
        "closure inputs are missing"
    );
    let file = File::options().write(true).create_new(true).open(output)?;
    let mut archive = tar::Builder::new(file);
    let mut total = 0u64;
    for (name, entry) in manifest.entries {
        ensure!(
            Path::new(&name)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
                && (name == "usr" || name.starts_with("usr/")),
            "unsafe closure member"
        );
        let path = tree.join(&name);
        let metadata = fs::symlink_metadata(&path)?;
        ensure!(
            path.parent()
                .context("missing parent")?
                .canonicalize()?
                .starts_with(&tree),
            "closure parent escapes tree"
        );
        ensure!(
            metadata.permissions().mode() & 0o7777 == entry.mode,
            "closure mode differs"
        );
        let mut header = tar::Header::new_gnu();
        header.set_mode(entry.mode);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        match entry.kind.as_str() {
            "directory" => {
                ensure!(metadata.is_dir(), "closure directory changed");
                header.set_entry_type(tar::EntryType::Directory);
                header.set_size(0);
                header.set_cksum();
                archive.append_data(&mut header, &name, std::io::empty())?;
            }
            "symlink" => {
                ensure!(metadata.file_type().is_symlink(), "closure symlink changed");
                let link = entry.link.context("missing link target")?;
                ensure!(
                    fs::read_link(path)? == Path::new(&link),
                    "closure link differs"
                );
                header.set_entry_type(tar::EntryType::Symlink);
                header.set_size(0);
                archive.append_link(&mut header, &name, link)?;
            }
            "file" => {
                ensure!(metadata.is_file(), "closure file changed");
                let object = entry.object.context("missing file identity")?;
                ensure!(
                    object.size <= 128 * 1024 * 1024,
                    "closure file exceeds bound"
                );
                total = total
                    .checked_add(object.size)
                    .context("closure byte overflow")?;
                ensure!(total <= 1024 * 1024 * 1024, "closure total exceeds bound");
                let mut bytes = Vec::new();
                File::options()
                    .read(true)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(&path)?
                    .take(object.size + 1)
                    .read_to_end(&mut bytes)?;
                ensure!(
                    bytes.len() as u64 == object.size
                        && crate::prepare::hash_bytes(&bytes) == object.sha256,
                    "closure file identity differs"
                );
                header.set_entry_type(tar::EntryType::Regular);
                header.set_size(object.size);
                header.set_cksum();
                archive.append_data(&mut header, &name, Cursor::new(bytes))?;
            }
            _ => anyhow::bail!("unsupported closure entry kind"),
        }
    }
    ensure!(total == manifest.bytes, "closure byte total differs");
    archive.finish()?;
    archive.into_inner()?.sync_all()?;
    hash_file(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn closure_identity_and_output_boundary_are_enforced() {
        let temp = tempfile::tempdir().unwrap();
        let snapshot = temp.path().join("snapshot");
        fs::create_dir_all(snapshot.join("tree/usr")).unwrap();
        fs::write(snapshot.join("tree/usr/input"), b"frozen tool input").unwrap();
        fs::set_permissions(snapshot.join("tree/usr"), fs::Permissions::from_mode(0o555)).unwrap();
        fs::set_permissions(
            snapshot.join("tree/usr/input"),
            fs::Permissions::from_mode(0o444),
        )
        .unwrap();
        let object = hash_file(&snapshot.join("tree/usr/input")).unwrap();
        let manifest = json!({"bytes":object.size,"entries":{"usr":{"kind":"directory","mode":365,"object":null,"link":null},"usr/input":{"kind":"file","mode":292,"object":object,"link":null}}});
        fs::write(
            snapshot.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let expected = hash_file(&snapshot.join("manifest.json")).unwrap();
        assert!(pack(&snapshot, &expected, &snapshot.join("derived.tar")).is_err());
        assert!(!snapshot.join("derived.tar").exists());
        let output = temp.path().join("bundle.tar");
        let id = pack(&snapshot, &expected, &output).unwrap();
        let members: Vec<_> = tar::Archive::new(File::open(&output).unwrap())
            .entries()
            .unwrap()
            .map(|entry| entry.unwrap().path().unwrap().to_path_buf())
            .collect();
        assert_eq!(
            members,
            vec![
                Path::new("usr").to_path_buf(),
                Path::new("usr/input").to_path_buf()
            ]
        );
        assert!(pack(&snapshot, &expected, &output).is_err());
        assert_eq!(hash_file(&output).unwrap(), id);
        fs::set_permissions(
            snapshot.join("tree/usr/input"),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        fs::write(snapshot.join("tree/usr/input"), b"changed tool input").unwrap();
        assert!(pack(&snapshot, &expected, &temp.path().join("changed.tar")).is_err());
    }
}

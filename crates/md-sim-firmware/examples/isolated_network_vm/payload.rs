//! Bounded newc payload for installed support tools. No payload program runs here.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{save_json, Mount},
    store::{hash_file, ObjectId, Store},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

// Captured Firefox support files plus the exact prepared S9 inputs exceed
// 1GiB. Keep this support-image bound explicit and retain the 2GiB combined
// input limit. This does not increase guest execution or browser memory limits.
const PAYLOAD_LIMIT: u64 = 1280 * 1024 * 1024;
const COMBINED_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayloadFile {
    pub source: PathBuf,
    pub object: ObjectId,
    pub mode: u32,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    #[serde(deserialize_with = "unique_map")]
    pub files: BTreeMap<String, PayloadFile>,
    #[serde(deserialize_with = "unique_map")]
    pub symlinks: BTreeMap<String, String>,
}
fn unique_map<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> std::result::Result<BTreeMap<String, T>, D::Error> {
    struct Unique<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Unique<T> {
        type Value = BTreeMap<String, T>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a map with unique paths")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, T>()? {
                if result.insert(key.clone(), value).is_some() {
                    return Err(serde::de::Error::custom(format!("duplicate path {key}")));
                }
            }
            Ok(result)
        }
    }
    deserializer.deserialize_map(Unique(std::marker::PhantomData))
}
fn path_ok(path: &str) -> Result<()> {
    ensure!(
        (path.starts_with("/usr/") || path.starts_with("/etc/"))
            && path.len() <= 4095
            && !path.bytes().any(|b| b < 32 || b == 127)
            && !path
                .split('/')
                .skip(1)
                .any(|p| p.is_empty() || p == "." || p == ".."),
        "unsafe payload path {path:?}"
    );
    Ok(())
}
fn parents(path: &str) -> impl Iterator<Item = String> + '_ {
    Path::new(path)
        .ancestors()
        .skip(1)
        .filter(|p| *p != Path::new("/"))
        .map(|p| p.to_string_lossy().into_owned())
}
fn link_target(name: &str, target: &str) -> Result<String> {
    ensure!(
        !target.is_empty() && target.len() <= 4095 && !target.bytes().any(|b| b < 32 || b == 127),
        "unsafe payload symlink target"
    );
    let mut parts = if target.starts_with('/') {
        Vec::new()
    } else {
        name.trim_start_matches('/')
            .split('/')
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    if !target.starts_with('/') {
        parts.pop();
    }
    for part in target.split('/') {
        match part {
            "" | "." => (),
            ".." => {
                ensure!(parts.pop().is_some(), "symlink escapes root");
            }
            _ => parts.push(part.to_string()),
        }
    }
    let path = format!("/{}", parts.join("/"));
    path_ok(&path)?;
    Ok(path)
}

fn read_manifest(path: &Path) -> Result<(Manifest, Vec<u8>)> {
    ensure!(
        fs::metadata(path)?.len() <= 4 * 1024 * 1024,
        "payload manifest too large"
    );
    let bytes = fs::read(path)?;
    ensure!(
        bytes.len() <= 4 * 1024 * 1024,
        "payload manifest grew beyond bound"
    );
    let manifest: Manifest = serde_json::from_slice(&bytes)?;
    ensure!(
        manifest.schema == "md-sim-vm-payload/1"
            && !manifest.files.is_empty()
            && manifest.files.len() + manifest.symlinks.len() <= 8192,
        "unsupported payload schema or file count"
    );
    Ok((manifest, bytes))
}

/// ELF dependency capture stores resolved bytes at each loader path. When
/// the payload preserves that path as a symlink, use the captured link only
/// if its terminal regular file has identical bytes and permissions. This
/// runs before the base archive is made. No conflicting entry is overwritten.
pub fn reconcile_base_links(
    base: &mut Vec<Mount>,
    base_modes: &BTreeMap<String, u32>,
    manifest_path: &Path,
) -> Result<Vec<Mount>> {
    let (manifest, _) = read_manifest(manifest_path)?;
    let mut removed = Vec::new();
    for input in base
        .iter()
        .filter(|input| manifest.symlinks.contains_key(&input.guest_path))
    {
        let mut resolved = input.guest_path.clone();
        let mut seen = BTreeSet::new();
        while let Some(target) = manifest.symlinks.get(&resolved) {
            path_ok(&resolved)?;
            ensure!(seen.insert(resolved.clone()), "payload symlink cycle");
            resolved = link_target(&resolved, target)?;
        }
        let file = manifest
            .files
            .get(&resolved)
            .context("base alias does not resolve to a captured regular file")?;
        file.object.validate()?;
        ensure!(
            input.object == file.object
                && base_modes.get(&input.guest_path).copied().unwrap_or(0o555) == file.mode,
            "payload alias conflicts with base bytes or permissions: {}",
            input.guest_path
        );
        removed.push(input.clone());
    }
    base.retain(|input| !removed.iter().any(|old| old.guest_path == input.guest_path));
    Ok(removed)
}

struct Newc<W> {
    writer: W,
    bytes: u64,
    inode: u32,
}
impl<W: Write> Newc<W> {
    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len() as u64)
            .context("cpio size overflow")?;
        ensure!(
            self.bytes <= PAYLOAD_LIMIT,
            "payload archive exceeds 1280MiB"
        );
        self.writer.write_all(bytes)?;
        Ok(())
    }
    fn pad(&mut self) -> Result<()> {
        let padding = (4 - self.bytes % 4) % 4;
        self.write(&[0; 3][..padding as usize])
    }
    fn entry(&mut self, name: &str, mode: u32, size: u64, reader: &mut impl Read) -> Result<()> {
        ensure!(size <= u32::MAX as u64, "cpio member too large");
        self.inode = self.inode.checked_add(1).context("cpio inode overflow")?;
        let header = format!(
            "070701{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}{:08x}",
            self.inode,
            mode,
            0,
            0,
            if mode & 0o170000 == 0o040000 { 2 } else { 1 },
            0,
            size,
            0,
            0,
            0,
            0,
            name.len() + 1,
            0
        );
        ensure!(header.len() == 110, "invalid newc header width");
        self.write(header.as_bytes())?;
        self.write(name.as_bytes())?;
        self.write(&[0])?;
        self.pad()?;
        let mut remaining = size;
        let mut buffer = [0u8; 65536];
        while remaining > 0 {
            let count = remaining.min(buffer.len() as u64) as usize;
            reader.read_exact(&mut buffer[..count])?;
            self.write(&buffer[..count])?;
            remaining -= count as u64;
        }
        ensure!(reader.read(&mut buffer[..1])? == 0, "payload member grew");
        self.pad()
    }
    fn finish(mut self) -> Result<(W, u64)> {
        self.entry("TRAILER!!!", 0, 0, &mut std::io::empty())?;
        Ok((self.writer, self.bytes))
    }
}

pub fn append(
    store: &Store,
    base: &[Mount],
    base_modes: &BTreeMap<String, u32>,
    manifest_path: &Path,
    base_image: &Path,
    out: &Path,
) -> Result<(PathBuf, Vec<Mount>)> {
    let (manifest, bytes) = read_manifest(manifest_path)?;
    fs::write(out.join("payload-manifest.json"), &bytes)?;
    let mut base_paths =
        BTreeSet::from(["/dev".into(), "/proc".into(), "/sys".into(), "/tmp".into()]);
    for input in base {
        base_paths.insert(input.guest_path.clone());
        base_paths.extend(parents(&input.guest_path));
    }
    let mut directories = BTreeSet::new();
    let mut total = 0u64;
    let mut extra = Vec::new();
    let mut saved = BTreeMap::new();
    for (name, file) in &manifest.files {
        path_ok(name)?;
        file.object.validate()?;
        ensure!(
            file.object.size <= 512 * 1024 * 1024 && matches!(file.mode, 0o444 | 0o555),
            "payload file size or mode outside bound"
        );
        ensure!(
            file.source.is_absolute() && fs::symlink_metadata(&file.source)?.is_file(),
            "payload source must be absolute regular file: {name}"
        );
        total = total
            .checked_add(file.object.size)
            .context("payload size overflow")?;
        ensure!(total <= PAYLOAD_LIMIT, "payload files exceed 1280MiB");
        ensure!(
            !manifest.symlinks.contains_key(name),
            "payload duplicate file/link path"
        );
        directories.extend(parents(name));
        if let Some(old) = base.iter().find(|input| input.guest_path == *name) {
            ensure!(
                old.object == file.object
                    && base_modes.get(name).copied().unwrap_or(0o555) == file.mode,
                "payload conflicts with base file {name}"
            );
        } else {
            ensure!(
                !base_paths.contains(name),
                "payload file replaces base directory: {name}"
            );
            saved.insert(name.clone(), file.source.clone());
            extra.push(Mount {
                guest_path: name.clone(),
                object: file.object.clone(),
            });
        }
    }
    for (name, target) in &manifest.symlinks {
        path_ok(name)?;
        ensure!(
            !base_paths.contains(name),
            "payload link replaces base entry: {name}"
        );
        directories.extend(parents(name));
        link_target(name, target)?;
    }
    let files = base
        .iter()
        .map(|i| i.guest_path.clone())
        .chain(manifest.files.keys().cloned())
        .collect::<BTreeSet<_>>();
    for directory in &directories {
        ensure!(
            !files.contains(directory) && !manifest.symlinks.contains_key(directory),
            "payload path traverses a file or symlink: {directory}"
        );
    }
    for name in manifest.symlinks.keys() {
        let mut seen = BTreeSet::new();
        let mut resolved = name.clone();
        while let Some(target) = manifest.symlinks.get(&resolved) {
            ensure!(seen.insert(resolved.clone()), "payload symlink cycle");
            resolved = link_target(&resolved, target)?;
        }
        ensure!(
            files.contains(&resolved)
                || directories.contains(&resolved)
                || base_paths.contains(&resolved),
            "dangling payload link {name} resolves to absent {resolved}"
        );
        ensure!(
            !parents(&resolved).any(|p| manifest.symlinks.contains_key(&p)),
            "payload link {name} resolves through another directory symlink: {resolved}"
        );
    }
    // Reject the complete metadata graph before copying large captured files.
    // Exact source hashes, including omitted base duplicates, remain mandatory.
    for (name, file) in &manifest.files {
        let verified = store.import(&file.source, &file.object)?;
        if let Some(path) = saved.get_mut(name) {
            *path = verified;
        }
    }
    let payload_path = out.join("payload.cpio.gz");
    let encoder = flate2::write::GzEncoder::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&payload_path)?,
        flate2::Compression::fast(),
    );
    let mut archive = Newc {
        writer: encoder,
        bytes: 0,
        inode: 0,
    };
    for directory in directories.difference(&base_paths) {
        archive.entry(
            directory.trim_start_matches('/'),
            0o040555,
            0,
            &mut std::io::empty(),
        )?;
    }
    for (name, path) in saved {
        let file = &manifest.files[&name];
        archive.entry(
            name.trim_start_matches('/'),
            0o100000 | file.mode,
            file.object.size,
            &mut File::open(&path)?,
        )?;
        ensure!(
            hash_file(&path)? == file.object,
            "stored payload changed during archive write"
        );
    }
    for (name, target) in &manifest.symlinks {
        archive.entry(
            name.trim_start_matches('/'),
            0o120777,
            target.len() as u64,
            &mut target.as_bytes(),
        )?;
    }
    let (encoder, uncompressed_bytes) = archive.finish()?;
    encoder.finish()?.sync_all()?;
    let combined = out.join("combined-initramfs.cpio.gz");
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&combined)?;
    let mut combined_bytes = 0u64;
    for image in [base_image, &payload_path] {
        combined_bytes = combined_bytes
            .checked_add(fs::metadata(image)?.len())
            .context("combined size overflow")?;
        ensure!(
            combined_bytes <= COMBINED_LIMIT,
            "combined initramfs exceeds 2GiB"
        );
        std::io::copy(&mut File::open(image)?, &mut output)?;
    }
    output.sync_all()?;
    save_json(
        &out.join("payload-report.json"),
        &json!({
            "schema":"md-sim-vm-payload-result/1","manifest":hash_file(&out.join("payload-manifest.json"))?,
            "base_initramfs":hash_file(base_image)?,"payload_initramfs":hash_file(&payload_path)?,"combined_initramfs":hash_file(&combined)?,
            "payload_files_bytes":total,"payload_archive_bytes":uncompressed_bytes,"payload_limit":PAYLOAD_LIMIT,"combined_limit":COMBINED_LIMIT,
            "archive_format":"concatenated gzip members containing newc archives","host_execution":"trusted Rust byte preparation only; no payload program executed",
            "source":{"sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("payload.rs")),"size":include_bytes!("payload.rs").len()},
            "symlinks":manifest.symlinks,"new_regular_files":extra.len(),"base_duplicates_omitted":manifest.files.len()-extra.len()
        }),
    )?;
    Ok((combined, extra))
}

#[cfg(test)]
mod tests {
    use super::*;
    use md_sim_runtime::isolation::{execute, Limits, ProcessSpec};
    use std::sync::{atomic::AtomicBool, Arc};
    #[test]
    fn expanded_support_image_keeps_its_declared_byte_bound() -> Result<()> {
        let mut archive = Newc {
            writer: Vec::new(),
            bytes: PAYLOAD_LIMIT - 2,
            inode: 0,
        };
        archive.write(b"ab")?;
        assert!(archive.write(b"c").is_err());
        assert_eq!(archive.writer, b"ab");
        let mut overflowing = Newc {
            writer: Vec::new(),
            bytes: u64::MAX,
            inode: 0,
        };
        assert!(overflowing.write(b"a").is_err());
        assert!(overflowing.writer.is_empty());
        Ok(())
    }
    #[test]
    fn newc_is_read_by_independent_isolated_cpio() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let store = Store::open(&temp.path().join("store"), 64 * 1024 * 1024)?;
        let mut archive = Newc {
            writer: Vec::new(),
            bytes: 0,
            inode: 0,
        };
        archive.entry("usr/share/test", 0o100444, 8, &mut b"payload\n".as_slice())?;
        archive.entry("usr/share/link", 0o120777, 4, &mut b"test".as_slice())?;
        let (bytes, _) = archive.finish()?;
        let path = temp.path().join("test.cpio");
        fs::write(&path, bytes)?;
        let stdin = hash_file(&path)?;
        store.import(&path, &stdin)?;
        let inputs = md_sim_firmware::lab::elf_closure(
            &store,
            vec![
                ("/usr/bin/sh".into(), "/usr/bin/sh".into()),
                ("/usr/bin/cpio".into(), "/usr/bin/cpio".into()),
            ],
        )?;
        let spec=ProcessSpec{private_channel: None, stdin_stream:None,namespace_uid:0,dashboard:None,loopback:false,schema_version:"2.0".into(),stdin_object:Some(stdin),argv:vec!["/usr/bin/sh".into(),"-c".into(),"/usr/bin/cpio -idm --quiet && IFS= read -r a < usr/share/test && IFS= read -r b < usr/share/link && test \"$a\" = payload && test \"$a\" = \"$b\" && test -L usr/share/link && printf 'CPIO_CONTENT_AND_SYMLINK_PASS\\n'".into()],inputs,environment:BTreeMap::new(),limits:Limits{wall_seconds:10,cpu_seconds:5,memory_bytes:256*1024*1024,processes:16,file_bytes:1024*1024,trace_bytes:65536,tmp_bytes:4*1024*1024}};
        let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
        ensure!(
            result.exit_code == Some(0) && !result.trace_truncated && result.stderr.is_empty(),
            "independent cpio failed {result:?}"
        );
        ensure!(
            hex::decode(result.stdout)? == b"CPIO_CONTENT_AND_SYMLINK_PASS\n",
            "independent file/link check failed"
        );
        Ok(())
    }
    #[test]
    fn rejects_duplicate_paths_escapes_and_unresolved_links() -> Result<()> {
        assert!(serde_json::from_str::<Manifest>(
            r#"{"schema":"md-sim-vm-payload/1","files":{},"symlinks":{"/usr/a":"b","/usr/a":"c"}}"#
        )
        .is_err());
        assert!(path_ok("/usr/../tmp/evil").is_err());
        assert!(link_target("/usr/lib/link", "../../../tmp/evil").is_err());
        assert_eq!(
            link_target("/usr/bin/link", "../lib/file")?,
            "/usr/lib/file"
        );
        let temp = tempfile::tempdir()?;
        let store = Store::open(&temp.path().join("store"), 64 * 1024 * 1024)?;
        let source = temp.path().join("file");
        fs::write(&source, b"fixture")?;
        let base = temp.path().join("base");
        fs::write(&base, b"irrelevant before validation")?;
        for (index, links) in [
            BTreeMap::from([("/usr/link".into(), "missing".into())]),
            BTreeMap::from([("/usr/a".into(), "b".into()), ("/usr/b".into(), "a".into())]),
            BTreeMap::from([("/usr/data".into(), "file".into())]),
        ]
        .into_iter()
        .enumerate()
        {
            let manifest = Manifest {
                schema: "md-sim-vm-payload/1".into(),
                files: BTreeMap::from([(
                    "/usr/data/file".into(),
                    PayloadFile {
                        source: source.clone(),
                        object: hash_file(&source)?,
                        mode: 0o444,
                    },
                )]),
                symlinks: links,
            };
            let path = temp.path().join(format!("manifest-{index}.json"));
            save_json(&path, &manifest)?;
            let out = temp.path().join(format!("out-{index}"));
            fs::create_dir(&out)?;
            assert!(append(&store, &[], &BTreeMap::new(), &path, &base, &out).is_err());
        }
        Ok(())
    }

    #[test]
    fn base_alias_requires_identical_terminal_bytes_and_mode() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let source = temp.path().join("library");
        fs::write(&source, b"exact captured dependency")?;
        let object = hash_file(&source)?;
        let original = vec![Mount {
            guest_path: "/usr/lib/library.so.1".into(),
            object: object.clone(),
        }];
        let mut manifest = Manifest {
            schema: "md-sim-vm-payload/1".into(),
            files: BTreeMap::from([(
                "/usr/lib/library.so.1.0".into(),
                PayloadFile {
                    source,
                    object,
                    mode: 0o555,
                },
            )]),
            symlinks: BTreeMap::from([(
                "/usr/lib/library.so.1".into(),
                "/usr/lib/library.so.1.0".into(),
            )]),
        };
        let path = temp.path().join("manifest.json");
        for mutation in 0..5 {
            let mut value = serde_json::to_value(&manifest)?;
            match mutation {
                0 => {
                    value["files"]["/usr/lib/library.so.1.0"]["object"]["sha256"] =
                        json!("ab".repeat(32))
                }
                1 => value["files"]["/usr/lib/library.so.1.0"]["mode"] = json!(0o444),
                2 => value["symlinks"]["/usr/lib/library.so.1"] = json!("/usr/lib/absent"),
                3 => value["symlinks"]["/usr/lib/library.so.1"] = json!("/usr/lib/library.so.1"),
                _ => value["symlinks"]["/usr/lib/library.so.1"] = json!("../../../tmp/escape"),
            }
            save_json(&path, &value)?;
            let mut base = original.clone();
            assert!(reconcile_base_links(&mut base, &BTreeMap::new(), &path).is_err());
            assert_eq!(base.len(), 1);
            assert_eq!(base[0].object, original[0].object);
        }
        save_json(&path, &manifest)?;
        let mut base = original.clone();
        let removed = reconcile_base_links(&mut base, &BTreeMap::new(), &path)?;
        assert!(base.is_empty());
        assert_eq!(removed[0].object, original[0].object);
        // A relative link chain is safe only after its final bytes are checked.
        manifest
            .symlinks
            .insert("/usr/lib/library.so.1".into(), "alias".into());
        manifest
            .symlinks
            .insert("/usr/lib/alias".into(), "library.so.1.0".into());
        save_json(&path, &manifest)?;
        let mut base = original;
        assert_eq!(
            reconcile_base_links(&mut base, &BTreeMap::new(), &path)?.len(),
            1
        );
        Ok(())
    }
}

//! Extract one exact regular file with the existing isolated 7zip tool.
use anyhow::{ensure, Context, Result};
use std::{
    io::{Read, Write},
    path::{Component, Path},
    process::{Command, Stdio},
};

fn bounded(args: &[&str], limit: u64) -> Result<Vec<u8>> {
    let mut child = Command::new("/usr/lib/7zip/7z")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .context("7zip stdout")?
        .take(limit + 1)
        .read_to_end(&mut bytes);
    if read.is_err() || bytes.len() as u64 > limit {
        let _ = child.kill();
        let _ = child.wait();
        read?;
        anyhow::bail!("7zip output exceeds member budget");
    }
    ensure!(child.wait()?.success(), "7zip operation failed");
    Ok(bytes)
}

pub fn extract(member: &str) -> Result<()> {
    ensure!(
        !member.is_empty()
            && member.len() <= 1024
            && !member.starts_with('-')
            && !member.chars().any(|c| c.is_control() || c == '\\')
            && Path::new(member)
                .components()
                .all(|c| matches!(c, Component::Normal(_))),
        "member must be one relative archive path"
    );
    let listing = bounded(
        &["l", "-ba", "-slt", "-spd", "/ramdisk", member],
        1024 * 1024,
    )?;
    let size = member_size(std::str::from_utf8(&listing)?, member)?;
    ensure!(
        size <= 7 * 1024 * 1024,
        "member exceeds retained-output budget"
    );
    let bytes = bounded(&["x", "-so", "-spd", "/ramdisk", member], size)?;
    ensure!(
        bytes.len() as u64 == size,
        "extracted size differs from exact member"
    );
    std::io::stdout().write_all(&bytes)?;
    Ok(())
}

fn member_size(listing: &str, member: &str) -> Result<u64> {
    let normalized = listing.replace("\r\n", "\n");
    let mut selected = None;
    for block in normalized.split("\n\n") {
        let fields: Vec<_> = block.lines().filter_map(|s| s.split_once(" = ")).collect();
        if !fields.contains(&("Path", member)) {
            continue;
        }
        ensure!(selected.is_none(), "duplicate exact archive member");
        for key in ["Path", "Folder", "Mode", "Size"] {
            ensure!(
                fields.iter().filter(|(k, _)| *k == key).count() == 1,
                "missing or duplicate member field: {key}"
            );
        }
        ensure!(
            fields.contains(&("Folder", "-"))
                && fields
                    .iter()
                    .any(|(k, v)| *k == "Mode" && v.starts_with('-')),
            "selected archive member is not a regular file"
        );
        let sizes: Vec<_> = fields.iter().filter(|(k, _)| *k == "Size").collect();
        ensure!(sizes.len() == 1, "missing or duplicate archive size");
        selected = Some(sizes[0].1.parse::<u64>()?);
    }
    selected.context("exact archive member was not found")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_regular_member_required_even_when_tool_returns_zero() {
        let listing =
            "Path = lib/antminer/hash_s8_app.txt\nFolder = -\nSize = 19200\nMode = -rw-r--r--\n";
        assert_eq!(
            member_size(listing, "lib/antminer/hash_s8_app.txt").unwrap(),
            19200
        );
        assert!(member_size(listing, "etc/bosminer/hash_s8_app.txt").is_err());
        assert!(member_size("", "missing").is_err());
        assert!(member_size(
            &format!("{listing}\n{listing}"),
            "lib/antminer/hash_s8_app.txt"
        )
        .is_err());
        assert!(member_size(
            &format!("Path = different\n{listing}"),
            "lib/antminer/hash_s8_app.txt"
        )
        .is_err());
        assert!(member_size(
            &listing.replace("Mode = -", "Mode = l"),
            "lib/antminer/hash_s8_app.txt"
        )
        .is_err());
        assert!(member_size(
            &listing.replace("Folder = -", "Folder = +"),
            "lib/antminer/hash_s8_app.txt"
        )
        .is_err());
        assert_eq!(
            member_size(
                "Path = empty\nFolder = -\nSize = 0\nMode = -rw-------\n",
                "empty"
            )
            .unwrap(),
            0
        );
    }
}

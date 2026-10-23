//! Run-owned SD storage for the adapted native-service diagnostic.
//! Both boots execute the original archived kernel and UCI. The caller
//! selects the restart path. This is not an original physical storage claim.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{isolation::save_json, store::hash_file};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
#[path = "s9_sd_image.rs"]
mod sd;

const DISK: &str = "/work/persistent.img";
const BYTES: u64 = sd::BYTES;
const PREFIX: &str = "MD_SIM_STORAGE_IMAGE ";
const SETUP: &str = r#"
set -e
mount -t proc proc /proc
mount -t sysfs sysfs /sys
mount -t tmpfs tmpfs /tmp
mount -t tmpfs tmpfs /dev
mknod /dev/null c 1 3
mknod /dev/console c 5 1
mknod /dev/mmcblk0 b 179 0
mkdir -p /tmp/persistent
mount -t ext2 /dev/mmcblk0 /tmp/persistent
"#;

fn image(stage: &str) -> Result<()> {
    println!("\n{PREFIX}{}", sd::capture(Path::new(DISK), stage)?);
    Ok(())
}

pub fn run(mut boot: impl FnMut(Vec<u8>, &Path) -> Result<()>) -> Result<()> {
    let mut token = String::new();
    std::io::stdin().take(65).read_to_string(&mut token)?;
    sd::format(Path::new(DISK), Path::new("/mke2fs"), &token)?;
    image("formatted")?;
    // Token is checked hexadecimal data, not shell text.
    let value = 101 + u32::from_str_radix(&token[..8], 16)? % 900;
    let first = format!(
        r#"(
{SETUP}
cp -a /etc /tmp/persistent/etc
cp /etc/config/uhttpd /tmp/persistent/uhttpd.before
mount --bind /tmp/persistent/etc /etc
test "$(uci get uhttpd.main.max_connections)" = '100'
uci set uhttpd.main.max_connections='{value}'
uci commit uhttpd
test "$(uci get uhttpd.main.max_connections)" = '{value}'
printf '%s\n' '{token}' > /tmp/persistent/fresh-marker
printf '\nMD_SIM_STORAGE_FIRST_VALUE:'
uci get uhttpd.main.max_connections
sync
umount /etc
umount /tmp/persistent
printf '\nMD_SIM_STORAGE_FIRST_COMMITTED\n'
)
"#
    );
    boot(first.into_bytes(), Path::new(DISK))?;
    image("committed")?;
    let second = format!(
        r#"(
{SETUP}
test "$(cat /tmp/persistent/fresh-marker)" = '{token}'
mount --bind /tmp/persistent/etc /etc
test "$(uci get uhttpd.main.max_connections)" = '{value}'
printf '\nMD_SIM_STORAGE_SECOND_VALUE:'
uci get uhttpd.main.max_connections
cp /tmp/persistent/uhttpd.before /etc/config/uhttpd
cmp /tmp/persistent/uhttpd.before /etc/config/uhttpd
test "$(uci get uhttpd.main.max_connections)" = '100'
printf '\nMD_SIM_STORAGE_RESTORED_VALUE:'
uci get uhttpd.main.max_connections
sync
umount /etc
umount /tmp/persistent
printf '\nMD_SIM_STORAGE_SECOND_RESTORED\n'
)
"#
    );
    boot(second.into_bytes(), Path::new(DISK))?;
    image("restored")?;
    Ok(())
}

/// Preserve every exported disk even when a later boot fails. Never infer a
/// storage pass from an image or a completed emulator process.
pub fn retain_images(out: &Path) -> Result<()> {
    ensure!(
        fs::metadata(out.join("stdout.bin"))?.len() <= 16 * 1024 * 1024,
        "storage trace exceeds bound"
    );
    let stdout = fs::read_to_string(out.join("stdout.bin"))?;
    let mut manifest = Vec::new();
    for line in stdout.lines() {
        let Some(record) = line.strip_prefix(PREFIX) else {
            continue;
        };
        let value: Value = serde_json::from_str(record)?;
        let stage = value["stage"].as_str().context("storage stage absent")?;
        ensure!(
            matches!(stage, "formatted" | "committed" | "restored"),
            "unknown storage stage"
        );
        let encoded = value["gzip_hex"].as_str().context("storage image absent")?;
        ensure!(
            encoded.len() <= 2 * 1024 * 1024,
            "storage image exceeds bound"
        );
        let bytes = hex::decode(encoded)?;
        ensure!(
            value["gzip_sha256"] == md_sim_firmware::prepare::hash_bytes(&bytes),
            "storage image hash differs"
        );
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(bytes.as_slice())
            .take(BYTES + 1)
            .read_to_end(&mut raw)?;
        ensure!(
            raw.len() as u64 == BYTES
                && value["raw"]["size"] == BYTES
                && value["raw"]["sha256"] == md_sim_firmware::prepare::hash_bytes(&raw),
            "raw storage image differs"
        );
        let path = out.join(format!("sd-{stage}.img.gz"));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(&bytes)?;
        manifest.push(
            json!({"stage":stage,"raw":value["raw"],"compressed":hash_file(&path)?,
            "file":path.file_name().unwrap().to_str()}),
        );
    }
    save_json(&out.join("storage-images.json"), &manifest)?;
    Ok(())
}

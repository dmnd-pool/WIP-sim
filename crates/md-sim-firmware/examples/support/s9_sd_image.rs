//! A new SD fixture. This is not an image of physical miner storage.
use anyhow::{ensure, Result};
use flate2::{write::GzEncoder, Compression};
use md_sim_runtime::store::hash_file;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    process::Command,
};

pub const BYTES: u64 = 8 * 1024 * 1024;

pub fn format(disk: &Path, tool: &Path, token: &str) -> Result<Value> {
    ensure!(
        token.len() == 64
            && token
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "fresh storage marker differs"
    );
    let uuid = format!(
        "{}-{}-{}-{}-{}",
        &token[..8],
        &token[8..12],
        &token[12..16],
        &token[16..20],
        &token[20..32]
    );
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(disk)?
        .set_len(BYTES)?;
    let args = [
        "-t", "ext2", "-F", "-b", "1024", "-I", "128", "-O", "none", "-U", &uuid,
    ];
    let output = Command::new(tool)
        .env_clear()
        .env("MKE2FS_CONFIG", "/dev/null")
        .args(args)
        .arg(disk)
        .output()?;
    let mut argv = args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    argv.push(disk.to_string_lossy().into_owned());
    let record = json!({"tool":hash_file(tool)?,"argv":argv,
        "exit_code":output.status.code(),"stdout_hex":hex::encode(output.stdout),
        "stderr_hex":hex::encode(output.stderr),"bytes":BYTES});
    println!("MD_SIM_STORAGE_FORMAT {record}");
    ensure!(output.status.success(), "isolated SD format failed");
    Ok(record)
}

pub fn capture(disk: &Path, stage: &str) -> Result<Value> {
    let mut raw = Vec::new();
    fs::File::open(disk)?
        .take(BYTES + 1)
        .read_to_end(&mut raw)?;
    ensure!(raw.len() as u64 == BYTES, "SD fixture changed size");
    let identity = json!({"sha256":md_sim_firmware::prepare::hash_bytes(&raw),"size":raw.len()});
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&raw)?;
    let bytes = encoder.finish()?;
    ensure!(
        bytes.len() <= 1024 * 1024,
        "compressed SD export exceeds bound"
    );
    Ok(json!({"stage":stage,"raw":identity,
        "gzip_sha256":md_sim_firmware::prepare::hash_bytes(&bytes),
        "gzip_hex":hex::encode(bytes)}))
}

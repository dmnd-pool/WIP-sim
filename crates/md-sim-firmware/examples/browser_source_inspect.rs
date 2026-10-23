//! Inspect exact captured Firefox source assets with an isolated archive tool.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, ObjectId, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};

// Firefox's optimized archive puts its directory before the stored members.
// Installed unzip extracts these bytes but rejects that directory placement.
// Verify just the requested stored member against both ZIP headers and its
// CRC. This does not turn unzip's nonzero result into a successful tool run.
fn verify_stored_member(archive: &[u8], name: &[u8], output: &[u8]) -> Result<Value> {
    fn take(bytes: &[u8], offset: usize, count: usize) -> Result<&[u8]> {
        bytes
            .get(offset..offset.checked_add(count).context("ZIP range overflow")?)
            .context("ZIP range exceeds archive")
    }
    fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(take(bytes, offset, 2)?.try_into()?))
    }
    fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(take(bytes, offset, 4)?.try_into()?))
    }
    ensure!(archive.len() >= 22, "short ZIP archive");
    let end = archive.len() - 22;
    ensure!(take(archive, end, 4)? == b"PK\x05\x06", "ZIP end absent");
    ensure!(
        u16_at(archive, end + 4)? == 0
            && u16_at(archive, end + 6)? == 0
            && u16_at(archive, end + 20)? == 0,
        "unsupported ZIP disk or comment"
    );
    let count = u16_at(archive, end + 10)? as usize;
    ensure!(
        count > 0 && count <= 8192 && u16_at(archive, end + 8)? as usize == count,
        "ZIP directory entry bound"
    );
    let start = u32_at(archive, end + 16)? as usize;
    let length = u32_at(archive, end + 12)? as usize;
    ensure!(
        start == 4 && length <= 4 << 20,
        "not the supported optimized ZIP layout"
    );
    let directory = take(archive, start, length)?;
    let directory_end = start
        .checked_add(length)
        .context("ZIP directory overflow")?;
    ensure!(directory_end <= end, "ZIP directory overlaps end");
    let mut cursor = 0;
    let mut found = None;
    for _ in 0..count {
        ensure!(
            take(directory, cursor, 4)? == b"PK\x01\x02",
            "ZIP central signature"
        );
        let name_len = u16_at(directory, cursor + 28)? as usize;
        let extra_len = u16_at(directory, cursor + 30)? as usize;
        let comment_len = u16_at(directory, cursor + 32)? as usize;
        let member_name = take(directory, cursor + 46, name_len)?;
        if member_name == name {
            ensure!(found.is_none(), "duplicate requested ZIP member");
            ensure!(
                u16_at(directory, cursor + 8)? == 0
                    && u16_at(directory, cursor + 10)? == 0
                    && u16_at(directory, cursor + 34)? == 0,
                "member is not plain stored data"
            );
            let crc = u32_at(directory, cursor + 16)?;
            let size = u32_at(directory, cursor + 24)? as usize;
            ensure!(
                size <= 4 << 20 && u32_at(directory, cursor + 20)? as usize == size,
                "stored member size bound"
            );
            let local = u32_at(directory, cursor + 42)? as usize;
            ensure!(
                local >= directory_end && take(archive, local, 4)? == b"PK\x03\x04",
                "local ZIP header absent or overlaps directory"
            );
            ensure!(
                u16_at(archive, local + 6)? == 0
                    && u16_at(archive, local + 8)? == 0
                    && u32_at(archive, local + 14)? == crc
                    && u32_at(archive, local + 18)? as usize == size
                    && u32_at(archive, local + 22)? as usize == size
                    && u16_at(archive, local + 26)? as usize == name.len(),
                "local and central member headers differ"
            );
            ensure!(
                take(archive, local + 30, name.len())? == name,
                "local member name differs"
            );
            let data_start = local + 30 + name.len() + u16_at(archive, local + 28)? as usize;
            ensure!(
                data_start.checked_add(size).is_some_and(|n| n <= end),
                "member data overlaps end"
            );
            let data = take(archive, data_start, size)?;
            let mut checksum = flate2::Crc::new();
            checksum.update(data);
            ensure!(
                checksum.sum() == crc && data == output,
                "member CRC or extracted bytes differ"
            );
            found = Some(
                json!({"verified":true,"method":"stored member, central/local header agreement, CRC32 and exact output bytes",
                "directory_offset":start,"directory_bytes":length,"local_header_offset":local,
                "data_offset":data_start,"data_bytes":size,"crc32":format!("{crc:08x}")}),
            );
        }
        cursor = cursor
            .checked_add(46 + name_len + extra_len + comment_len)
            .context("ZIP cursor overflow")?;
        ensure!(cursor <= directory.len(), "ZIP directory member bounds");
    }
    ensure!(
        cursor == directory.len(),
        "ZIP directory count/size differs"
    );
    found.context("requested stored member absent")
}

fn run() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    ensure!(
        args.len() == 5,
        "usage: browser_source_inspect CAPTURE_DIRECTORY NEW_OUTPUT core|browser list|MEMBER"
    );
    let capture = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(out)?;
    save_json(
        &out.join("assessment.json"),
        &json!({"passed":false,"stage":"preflight"}),
    )?;
    let manifest_path = capture.join("payload.json");
    ensure!(
        fs::metadata(&manifest_path)?.len() <= 4 << 20,
        "manifest bound"
    );
    let manifest: Value = serde_json::from_slice(&fs::read(&manifest_path)?)?;
    ensure!(
        manifest["schema"] == "md-sim-vm-payload/1",
        "payload schema"
    );
    let archive = match args[3].as_str() {
        "core" => "/usr/lib/firefox/omni.ja",
        "browser" => "/usr/lib/firefox/browser/omni.ja",
        _ => anyhow::bail!("archive must be core or browser"),
    };
    let entry = &manifest["files"][archive];
    let object: ObjectId = serde_json::from_value(entry["object"].clone())?;
    object.validate()?;
    let source = Path::new(
        entry["source"]
            .as_str()
            .context("captured archive source absent")?,
    );
    let store = Store::open(&out.join("inputs"), 256 << 20)?;
    store.import(source, &object)?;
    let driver = hash_file(&std::env::current_exe()?)?;
    store.import(&std::env::current_exe()?, &driver)?;
    fs::write(
        out.join("driver-source.rs"),
        include_bytes!("browser_source_inspect.rs"),
    )?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![("/tools/unzip".into(), PathBuf::from("/usr/bin/unzip"))],
    )?;
    inputs.push(Mount {
        guest_path: "/omni.ja".into(),
        object: object.clone(),
    });
    let argv = if args[4] == "list" {
        vec![
            "/tools/unzip".into(),
            "-Z".into(),
            "-1".into(),
            "/omni.ja".into(),
        ]
    } else {
        ensure!(
            args[4].len() <= 4095
                && args[4]
                    .split('/')
                    .all(|p| !p.is_empty() && p != "." && p != "..")
                && !args[4]
                    .bytes()
                    .any(|b| b < 32 || b == 127 || b == b'*' || b == b'?')
                && !args[4].starts_with('-'),
            "invalid exact archive member"
        );
        vec![
            "/tools/unzip".into(),
            "-p".into(),
            "/omni.ja".into(),
            args[4].clone(),
        ]
    };
    let spec = ProcessSpec {
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv,
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 15,
            cpu_seconds: 5,
            memory_bytes: 512 << 20,
            processes: 16,
            file_bytes: 4 << 20,
            trace_bytes: 4 << 20,
            tmp_bytes: 16 << 20,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    let stdout = hex::decode(&result.stdout)?;
    fs::write(out.join("output.txt"), &stdout)?;
    fs::write(out.join("stderr.txt"), hex::decode(&result.stderr)?)?;
    let tool_passed = result.status == "exited"
        && result.exit_code == Some(0)
        && !result.trace_truncated
        && result.stderr.is_empty()
        && !stdout.is_empty();
    let independent_member = if args[4] == "list" {
        None
    } else {
        ensure!(object.size <= 256 << 20, "archive verification bound");
        let mut bytes = Vec::new();
        fs::File::open(store.verified_path(&object)?)?
            .take(object.size + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 == object.size
                && md_sim_firmware::prepare::hash_bytes(&bytes) == object.sha256,
            "captured archive changed during inspection"
        );
        Some(verify_stored_member(&bytes, args[4].as_bytes(), &stdout)?)
    };
    let passed = (tool_passed || independent_member.is_some())
        && result.status == "exited"
        && !result.trace_truncated
        && !stdout.is_empty();
    save_json(
        &out.join("assessment.json"),
        &json!({"passed":passed,"tool_passed":tool_passed,"independent_member":independent_member,"archive_path":archive,"archive":object,
        "captured_manifest":hash_file(&manifest_path)?,"spec":hash_file(&out.join("spec.json"))?,
        "result":hash_file(&out.join("result.json"))?,"output":hash_file(&out.join("output.txt"))?,
        "member":args[4],"driver":driver,
        "driver_source":md_sim_firmware::prepare::hash_bytes(include_bytes!("browser_source_inspect.rs")),
        "scope":"Read-only exact installed browser source asset inspection; no browser or firmware execution claim"}),
    )?;
    ensure!(
        passed,
        "isolated source inspection failed; evidence retained"
    );
    Ok(())
}
fn main() -> Result<()> {
    let out = std::env::args_os().nth(2).map(PathBuf::from);
    let new = out.as_ref().is_some_and(|p| !p.exists());
    let result = run();
    if let (Err(error), Some(out)) = (&result, out) {
        if new && out.is_dir() {
            save_json(
                &out.join("failure.json"),
                &json!({"passed":false,"error":format!("{error:#}")}),
            )?;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let name = b"source.js";
        let data = b"actual bytes\n";
        let mut crc = flate2::Crc::new();
        crc.update(data);
        let mut central = vec![0u8; 46];
        central[..4].copy_from_slice(b"PK\x01\x02");
        central[16..20].copy_from_slice(&crc.sum().to_le_bytes());
        central[20..24].copy_from_slice(&(data.len() as u32).to_le_bytes());
        central[24..28].copy_from_slice(&(data.len() as u32).to_le_bytes());
        central[28..30].copy_from_slice(&(name.len() as u16).to_le_bytes());
        central[42..46].copy_from_slice(&((4 + 46 + name.len()) as u32).to_le_bytes());
        central.extend_from_slice(name);
        let mut local = vec![0u8; 30];
        local[..4].copy_from_slice(b"PK\x03\x04");
        local[14..18].copy_from_slice(&crc.sum().to_le_bytes());
        local[18..22].copy_from_slice(&(data.len() as u32).to_le_bytes());
        local[22..26].copy_from_slice(&(data.len() as u32).to_le_bytes());
        local[26..28].copy_from_slice(&(name.len() as u16).to_le_bytes());
        let mut end = vec![0u8; 22];
        end[..4].copy_from_slice(b"PK\x05\x06");
        end[8..10].copy_from_slice(&1u16.to_le_bytes());
        end[10..12].copy_from_slice(&1u16.to_le_bytes());
        end[12..16].copy_from_slice(&(central.len() as u32).to_le_bytes());
        end[16..20].copy_from_slice(&4u32.to_le_bytes());
        [
            vec![0; 4],
            central,
            local,
            name.to_vec(),
            data.to_vec(),
            end,
        ]
        .concat()
    }
    #[test]
    fn exact_stored_member_and_changed_inputs() {
        let bytes = fixture();
        assert!(verify_stored_member(&bytes, b"source.js", b"actual bytes\n").is_ok());
        assert!(verify_stored_member(&bytes, b"source.js", b"altered bytes").is_err());
        assert!(verify_stored_member(&bytes, b"other.js", b"actual bytes\n").is_err());
        let mut changed = bytes.clone();
        let data = changed.len() - 22 - 13;
        changed[data] ^= 1;
        assert!(verify_stored_member(&changed, b"source.js", &changed[data..data + 13]).is_err());
        let mut changed = bytes;
        changed[46..50].copy_from_slice(&0u32.to_le_bytes());
        assert!(verify_stored_member(&changed, b"source.js", b"actual bytes\n").is_err());
    }
}

//! Retain bounded, run-owned block overlays across emulator starts.
//! Each storage kind has a fixed backing path, geometry and snapshot namespace.
use anyhow::{ensure, Context, Result};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use md_sim_firmware::serial_console::{GuestInterruption, GuestRestart};
use md_sim_runtime::{
    isolation::{save_json, ProcessResult},
    store::{hash_file, ObjectId},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
};

const MARKER: &str = "MD_SIM_BOOT_SD_SNAPSHOT ";
// A native same-release update rewrites the compressed 19 MiB firmware into
// the private overlay. Its compressed export can exceed the old 8 MiB cap.
const GZIP_LIMIT: u64 = 24 * 1024 * 1024;
const VIRTUAL_BYTES: u64 = 128 * 1024 * 1024;
const DISK: &str = "/work/sd.qcow2";
#[derive(Clone, Copy)]
pub struct Storage {
    name: &'static str,
    schema: &'static str,
    marker: &'static str,
    backing_path: &'static str,
    disk: &'static str,
    input_json: &'static str,
    input_gzip: &'static str,
    virtual_bytes: u64,
    raw_limit: u64,
}
pub const SD: Storage = Storage {
    name: "sd",
    schema: "md-sim-original-boot-sd",
    marker: MARKER,
    backing_path: "/sd.img",
    disk: DISK,
    input_json: "/sd-state.json",
    input_gzip: "/sd-state.qcow2.gz",
    virtual_bytes: VIRTUAL_BYTES,
    raw_limit: 32 * 1024 * 1024,
};
pub const NAND: Storage = Storage {
    name: "nand",
    schema: "md-sim-reference-boot-nand",
    marker: "MD_SIM_BOOT_NAND_SNAPSHOT ",
    backing_path: "/nand-erased.raw",
    disk: "/work/nand.qcow2",
    input_json: "/nand-state.json",
    input_gzip: "/nand-state.qcow2.gz",
    virtual_bytes: 131072 * 2112,
    // NAND includes 264 MiB of data+spare bytes. Its qcow2 export has a
    // separate bound, still within the unchanged sandbox file allowance.
    raw_limit: 64 * 1024 * 1024,
};
const CHECKPOINT_SCRIPT: &str = r#"(
set -eu
printf '\nMD_SIM_NATIVE_STORAGE_CHECKPOINT_BEGIN\n'
cat /proc/mounts
sync
/bin/mount -o remount,ro /overlay
/bin/mount -o remount,ro /tmp/mnt/boot
cat /proc/mounts
awk '$2 == "/overlay" && $3 == "ext4" && $4 ~ /(^|,)ro(,|$)/ {found++} END {exit(found != 1)}' /proc/mounts
awk '$2 == "/tmp/mnt/boot" && $3 == "vfat" && $4 ~ /(^|,)ro(,|$)/ {found++} END {exit(found != 1)}' /proc/mounts
printf '\nMD_SIM_NATIVE_STORAGE_CHECKPOINT_VERIFIED\n'
)
"#;

pub fn checkpoint_identity() -> Value {
    json!({"script_sha256":md_sim_firmware::prepare::hash_bytes(CHECKPOINT_SCRIPT.as_bytes()),
        "script_bytes":CHECKPOINT_SCRIPT.len(),
        "operation":"Explicit native sync and read-only remount of the ext4 overlay and FAT boot filesystem, with actual mount-state assertions, before normal reboot.",
        "scope":"Storage maintenance through unchanged native tools. Not an unassisted shutdown or power-loss recovery claim. Independent raw filesystem inspection is still required."})
}

pub fn checkpoint_native(guest: &mut md_sim_firmware::serial_console::SerialGuest) -> Result<()> {
    guest.script(CHECKPOINT_SCRIPT.as_bytes().to_vec())?;
    println!("MD_SIM_NATIVE_SD_CHECKPOINT {}", checkpoint_identity());
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    schema: String,
    stage: usize,
    backing: ObjectId,
    raw: ObjectId,
    gzip: ObjectId,
    file: String,
    virtual_bytes: u64,
    /// None means the older snapshot did not record this observation.
    #[serde(default)]
    guest_sync_completed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    guest_restart: Option<GuestRestart>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    guest_interruption: Option<GuestInterruption>,
    scope: String,
}

fn read(path: &Path, bound: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(bound + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= bound,
        "storage file {} exceeds {bound} bytes",
        path.display()
    );
    Ok(bytes)
}

pub fn input(path: &Path, backing: &ObjectId) -> Result<Snapshot> {
    input_storage(SD, path, backing)
}

pub fn input_storage(storage: Storage, path: &Path, backing: &ObjectId) -> Result<Snapshot> {
    let state: Snapshot = serde_json::from_slice(&read(path, 16384)?)?;
    validate_storage(storage, &state, backing)?;
    let compressed = path
        .parent()
        .context("snapshot parent absent")?
        .join(&state.file);
    ensure!(
        hash_file(&compressed)? == state.gzip,
        "SD compressed object differs"
    );
    decode(storage, &state, &read(&compressed, GZIP_LIMIT)?)?;
    Ok(state)
}

#[cfg(test)]
fn validate(state: &Snapshot, backing: &ObjectId) -> Result<()> {
    validate_storage(SD, state, backing)
}

fn validate_storage(storage: Storage, state: &Snapshot, backing: &ObjectId) -> Result<()> {
    let version = state
        .schema
        .strip_prefix(storage.schema)
        .and_then(|suffix| suffix.strip_prefix('/'));
    ensure!(
        matches!(version, Some("1" | "2" | "3" | "4"))
            && (version == Some("1") || state.guest_sync_completed.is_some())
            && state.backing == *backing
            && state.virtual_bytes == storage.virtual_bytes
            && state.stage <= 2
            && state.file == format!("{}-snapshot-{}.qcow2.gz", storage.name, state.stage)
            && state.raw.size > 0
            && state.raw.size <= storage.raw_limit
            && state.gzip.size > 0
            && state.gzip.size <= GZIP_LIMIT,
        "SD snapshot identity or bounds differ"
    );
    if version == Some("3") {
        ensure!(
            state.guest_sync_completed == Some(false)
                && state.guest_interruption.is_none()
                && state
                    .guest_restart
                    .as_ref()
                    .is_some_and(|restart| restart.emulator_exit_code == 0
                        && restart.serial.size > 0
                        && restart.serial.size <= 1024 * 1024),
            "native-restart snapshot lacks its exit observation"
        );
    } else if version == Some("4") {
        ensure!(
            state.guest_sync_completed == Some(false)
                && state.guest_restart.is_none()
                && state
                    .guest_interruption
                    .as_ref()
                    .is_some_and(|interruption| interruption.emulator_signal == libc::SIGKILL
                        && interruption.serial.size <= 1024 * 1024),
            "interrupted snapshot lacks its signal-exit observation"
        );
    } else {
        ensure!(
            state.guest_restart.is_none() && state.guest_interruption.is_none(),
            "legacy snapshot has a conflicting restart claim"
        );
    }
    Ok(())
}

fn decode(storage: Storage, state: &Snapshot, compressed: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        state.raw.size <= storage.raw_limit
            && compressed.len() as u64 == state.gzip.size
            && md_sim_firmware::prepare::hash_bytes(compressed) == state.gzip.sha256,
        "compressed SD digest differs"
    );
    let mut raw = Vec::new();
    GzDecoder::new(compressed)
        .take(storage.raw_limit + 1)
        .read_to_end(&mut raw)?;
    ensure!(
        raw.len() as u64 == state.raw.size
            && md_sim_firmware::prepare::hash_bytes(&raw) == state.raw.sha256,
        "decoded SD digest differs"
    );
    Ok(raw)
}

pub fn file(state: &Snapshot) -> &str {
    &state.file
}

/// Deterministic development backing. This is generated erased media, not a
/// physical factory dump. All data and spare bytes are FF, including bad-block
/// markers. The original boot must create its own tables and persistent data.
pub fn prepare_erased_nand(path: &Path) -> Result<ObjectId> {
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let chunk = [0xff; 65536];
    let mut remaining = NAND.virtual_bytes;
    while remaining != 0 {
        let count = remaining.min(chunk.len() as u64) as usize;
        output.write_all(&chunk[..count])?;
        remaining -= count as u64;
    }
    output.sync_all()?;
    drop(output);
    hash_file(path)
}

pub fn create(storage: Storage) -> Result<()> {
    ensure!(
        !Path::new(storage.disk).exists(),
        "private overlay already exists"
    );
    let mut command = Command::new("/qemu-img");
    command.args([
        "create",
        "-q",
        "-f",
        "qcow2",
        "-F",
        "raw",
        "-b",
        storage.backing_path,
    ]);
    if storage.name == NAND.name {
        command.args(["-o", "cluster_size=4096"]);
    }
    let status = command
        .arg(storage.disk)
        .arg(storage.virtual_bytes.to_string())
        .stdin(Stdio::null())
        .status()?;
    ensure!(status.success(), "private block overlay creation failed");
    info(storage)?;
    Ok(())
}

pub fn create_sd() -> Result<()> {
    create(SD)
}

pub fn same_termination(left: &Snapshot, right: &Snapshot) -> Result<()> {
    ensure!(
        (left.guest_restart.is_some()
            && left.guest_restart == right.guest_restart
            && left.guest_interruption.is_none()
            && right.guest_interruption.is_none())
            || (left.guest_interruption.is_some()
                && left.guest_interruption == right.guest_interruption
                && left.guest_restart.is_none()
                && right.guest_restart.is_none()),
        "paired storage snapshots must reference the same observed termination"
    );
    Ok(())
}

/// Bind restored media to the actual parent output. Equal shutdown text alone
/// cannot identify a run: two independent boots can emit identical text.
pub fn verify_pair(
    sd_path: &Path,
    sd: &Snapshot,
    nand_path: &Path,
    nand: &Snapshot,
) -> Result<Value> {
    validate_storage(SD, sd, &sd.backing)?;
    validate_storage(NAND, nand, &nand.backing)?;
    same_termination(sd, nand)?;
    let parent = |storage: Storage,
                  path: &Path,
                  state: &Snapshot|
     -> Result<(ObjectId, ObjectId, std::path::PathBuf)> {
        let directory = path.parent().context("snapshot parent absent")?;
        let report: Value = serde_json::from_slice(&read(
            &directory.join(format!("{}-storage.json", storage.name)),
            65536,
        )?)?;
        let manifest = serde_json::to_value(hash_file(path)?)?;
        let snapshot = serde_json::to_value(state)?;
        ensure!(
            report["snapshots"]
                .as_array()
                .context("parent snapshot list absent")?
                .iter()
                .filter(|entry| entry["manifest"] == manifest && entry["snapshot"] == snapshot)
                .count()
                == 1,
            "snapshot differs from retained parent report"
        );
        let result_path = directory.join("result.json");
        let result = hash_file(&result_path)?;
        let spec = hash_file(&directory.join("process-spec.json"))?;
        ensure!(
            report["execution_result"] == serde_json::to_value(&result)?
                && report["process_specification"] == serde_json::to_value(&spec)?,
            "parent execution identity differs"
        );
        Ok((result, spec, result_path))
    };
    let (result_id, spec_id, result_path) = parent(SD, sd_path, sd)?;
    let (nand_result, nand_spec, _) = parent(NAND, nand_path, nand)?;
    ensure!(
        result_id == nand_result && spec_id == nand_spec && sd.stage == nand.stage,
        "SD and NAND belong to different executions or stages"
    );
    let parent_bytes = read(
        &result_path,
        2 * md_sim_runtime::isolation::MAX_TRACE_BYTES + 1024 * 1024,
    )?;
    ensure!(
        parent_bytes.len() as u64 == result_id.size
            && md_sim_firmware::prepare::hash_bytes(&parent_bytes) == result_id.sha256,
        "parent execution changed during snapshot validation"
    );
    let result: ProcessResult = serde_json::from_slice(&parent_bytes)?;
    ensure!(
        !result.trace_truncated,
        "paired input parent trace is truncated"
    );
    let bytes = hex::decode(result.stdout)?;
    let text = std::str::from_utf8(&bytes)?;
    let marker = if sd.guest_interruption.is_some() {
        "MD_SIM_GUEST_INTERRUPTION "
    } else {
        "MD_SIM_GUEST_RESTART "
    };
    let restarts: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix(marker))
        .collect();
    ensure!(restarts.len() <= 3, "parent restart count bound");
    let restart = restarts
        .get(sd.stage)
        .context("parent native restart missing")?;
    ensure!(
        restart.len() <= 2 * 1024 * 1024 + 4096,
        "parent restart record bound"
    );
    let restart: Value = serde_json::from_str(restart)?;
    let serial = hex::decode(
        restart["serial_hex"]
            .as_str()
            .context("parent native restart serial absent")?,
    )?;
    if sd.guest_interruption.is_some() {
        let observation: GuestInterruption =
            serde_json::from_value(restart["observation"].clone())?;
        observation.verify_serial(&serial)?;
        ensure!(
            sd.guest_interruption.as_ref() == Some(&observation),
            "parent interruption differs"
        );
    } else {
        let observation: GuestRestart = serde_json::from_value(restart["observation"].clone())?;
        observation.verify_serial(&serial)?;
        ensure!(
            sd.guest_restart.as_ref() == Some(&observation),
            "parent native restart differs"
        );
    }
    for (storage, state) in [(SD, sd), (NAND, nand)] {
        let mut matching = 0;
        for line in text
            .lines()
            .filter_map(|line| line.strip_prefix(storage.marker))
        {
            ensure!(
                line.len() as u64 <= 2 * GZIP_LIMIT + 65536,
                "parent snapshot record bound"
            );
            let record: Value = serde_json::from_str(line)?;
            if record["snapshot"]["stage"] == state.stage {
                ensure!(
                    record["snapshot"] == serde_json::to_value(state)?,
                    "snapshot is absent from actual parent output"
                );
                decode(
                    storage,
                    state,
                    &hex::decode(
                        record["gzip_hex"]
                            .as_str()
                            .context("parent snapshot bytes absent")?,
                    )?,
                )?;
                matching += 1;
            }
        }
        ensure!(matching == 1, "parent snapshot missing or repeated");
    }
    Ok(
        json!({"execution_result":result_id,"process_specification":spec_id,
        "stage":sd.stage,"actual_parent_exports_verified":true}),
    )
}

fn info(storage: Storage) -> Result<Value> {
    info_at(storage, storage.disk)
}

fn info_at(storage: Storage, disk: &str) -> Result<Value> {
    let output = Command::new("/qemu-img")
        .args(["info", "--output=json", disk])
        .stdin(Stdio::null())
        .output()?;
    ensure!(
        output.status.success() && output.stdout.len() <= 65536,
        "isolated SD image inspection failed"
    );
    let info: Value = serde_json::from_slice(&output.stdout)?;
    ensure!(
        info["format"] == "qcow2"
            && info["virtual-size"] == storage.virtual_bytes
            && info["backing-filename"] == storage.backing_path
            && info["backing-filename-format"] == "raw",
        "saved SD overlay has a different backing chain or geometry"
    );
    Ok(info)
}

/// Native UBI writes small headers across many erase blocks. Compress its
/// allocated erased bytes and use4KiB clusters to limit subsequent write
/// allocation. No logical data or backing bytes may change.
/// Both operations run after QEMU exits, inside its existing sandbox.
fn export_disk(storage: Storage) -> Result<(&'static str, Option<Value>)> {
    if storage.name != NAND.name {
        return Ok((storage.disk, None));
    }
    const OUTPUT: &str = "/work/nand-export.qcow2";
    ensure!(!Path::new(OUTPUT).exists(), "NAND export already exists");
    let original = hash_file(Path::new(storage.disk))?;
    // This is also the sandbox's unchanged per-file limit.
    ensure!(original.size <= 64 * 1024 * 1024, "NAND source file bound");
    let backing = hash_file(Path::new(storage.backing_path))?;
    let original_info = info(storage)?;
    let convert_args = [
        "convert",
        "-q",
        "-f",
        "qcow2",
        "-O",
        "qcow2",
        "-c",
        "-o",
        "cluster_size=4096,compression_type=zlib",
        "-B",
        storage.backing_path,
        "-F",
        "raw",
        storage.disk,
        OUTPUT,
    ];
    let convert = Command::new("/qemu-img")
        .args(convert_args)
        .stdin(Stdio::null())
        .status()?;
    ensure!(convert.success(), "NAND overlay conversion failed");
    let compare_args = [
        "compare",
        "-q",
        "-f",
        "qcow2",
        "-F",
        "qcow2",
        storage.disk,
        OUTPUT,
    ];
    let compare = Command::new("/qemu-img")
        .args(compare_args)
        .stdin(Stdio::null())
        .status()?;
    ensure!(compare.success(), "NAND export logical bytes differ");
    ensure!(
        hash_file(Path::new(storage.disk))? == original
            && hash_file(Path::new(storage.backing_path))? == backing,
        "NAND source or backing changed during export"
    );
    Ok((
        OUTPUT,
        Some(json!({
            "source_overlay":original,"source_info":original_info,
            "export_overlay":hash_file(Path::new(OUTPUT))?,"backing":backing,
            "tool":hash_file(Path::new("/qemu-img"))?,
            "convert_argv":convert_args,"logical_compare_argv":compare_args,
            "convert_exit_code":convert.code(),"logical_compare_exit_code":compare.code(),
            "scope":"Compressed qcow2 layout with4KiB clusters over the unchanged backing. Every logical byte is compared. Source overlay and backing remain unchanged during conversion."
        })),
    ))
}

/// The caller must run in the board sandbox before QEMU opens this disk.
pub fn restore() -> Result<()> {
    restore_storage(SD)
}

pub fn restore_storage(storage: Storage) -> Result<()> {
    let backing = hash_file(Path::new(storage.backing_path))?;
    let state: Snapshot = serde_json::from_slice(&read(Path::new(storage.input_json), 16384)?)?;
    validate_storage(storage, &state, &backing)?;
    let bytes = read(Path::new(storage.input_gzip), GZIP_LIMIT)?;
    let raw = decode(storage, &state, &bytes)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(storage.disk)?;
    file.write_all(&raw)?;
    file.sync_all()?;
    drop(file);
    let restore_conversion = if storage.name == NAND.name {
        let (converted, record) = export_disk(storage)?;
        // Only the private decoded copy is replaced. The archived snapshot
        // input and its backing remain read-only and retain their identities.
        fs::rename(converted, storage.disk)?;
        record
    } else {
        None
    };
    println!(
        "MD_SIM_BOOT_{}_RESTORED {}",
        storage.name.to_ascii_uppercase(),
        json!({"snapshot":state,"image_info":info(storage)?,"restore_conversion":restore_conversion})
    );
    Ok(())
}

/// QEMU has exited. Record whether the native sync command completed.
/// Failed runs can export unsynced images. Neither case establishes guest
/// shutdown, host-crash durability or NAND retention.
pub fn capture(stage: usize, guest_sync_completed: bool) -> Result<()> {
    capture_storage(SD, stage, guest_sync_completed, None)
}

/// Native Linux shutdown is a distinct observation. It must not be reported
/// as a separately issued guest sync command. The next boot must check data.
pub fn capture_restart(stage: usize, restart: GuestRestart) -> Result<()> {
    capture_storage(SD, stage, false, Some(restart))
}

pub fn capture_storage(
    storage: Storage,
    stage: usize,
    guest_sync_completed: bool,
    guest_restart: Option<GuestRestart>,
) -> Result<()> {
    capture_observed(storage, stage, guest_sync_completed, guest_restart, None)
}

pub fn capture_interrupted(
    storage: Storage,
    stage: usize,
    interruption: GuestInterruption,
) -> Result<()> {
    capture_observed(storage, stage, false, None, Some(interruption))
}

fn capture_observed(
    storage: Storage,
    stage: usize,
    guest_sync_completed: bool,
    guest_restart: Option<GuestRestart>,
    guest_interruption: Option<GuestInterruption>,
) -> Result<()> {
    let (disk, conversion) = export_disk(storage)?;
    println!(
        "MD_SIM_STORAGE_EXPORT {}",
        json!({"storage":storage.name,"raw_limit":storage.raw_limit,"conversion":conversion})
    );
    let raw = read(Path::new(disk), storage.raw_limit)?;
    let metadata = info_at(storage, disk)?;
    if conversion.is_some() {
        fs::remove_file(disk)?;
    }
    let mut compressor = GzEncoder::new(Vec::new(), Compression::default());
    compressor.write_all(&raw)?;
    let compressed = compressor.finish()?;
    ensure!(
        compressed.len() as u64 <= GZIP_LIMIT,
        "SD compressed export exceeds bound"
    );
    let object = |bytes: &[u8]| ObjectId {
        sha256: md_sim_firmware::prepare::hash_bytes(bytes),
        size: bytes.len() as u64,
    };
    let state = Snapshot {
        schema:format!("{}/{}",storage.schema,if guest_interruption.is_some() {4} else if guest_restart.is_some() {3} else {2}),stage,backing:hash_file(Path::new(storage.backing_path))?,
        raw:object(&raw),gzip:object(&compressed),file:format!("{}-snapshot-{stage}.qcow2.gz",storage.name),virtual_bytes:storage.virtual_bytes,
        guest_sync_completed:Some(guest_sync_completed),
        guest_restart,
        guest_interruption,
        scope:format!("The read-only {} backing is unchanged. Capture follows emulator termination. guest_sync_completed records a separately issued native sync command; it does not prove internal device operations completed. guest_restart records kernel restart text, serial EOF and actual exit0. guest_interruption records a separate intentional SIGKILL and complete serial EOF, without a requested guest sync or reboot. Only this overlay is retained. Other disks and volatile state need separate evidence. No update correctness, exact physical geometry or crash durability claim.",storage.name),
    };
    validate_storage(storage, &state, &state.backing)?;
    println!(
        "\n{}{}",
        storage.marker,
        json!({"snapshot":state,"image_info":metadata,"export_conversion":conversion,"gzip_hex":hex::encode(compressed)})
    );
    Ok(())
}

pub fn retain(out: &Path, result: &ProcessResult, expected: usize) -> Result<()> {
    retain_storage(SD, out, result, expected)
}

pub fn retain_storage(
    storage: Storage,
    out: &Path,
    result: &ProcessResult,
    expected: usize,
) -> Result<()> {
    ensure!((1..=3).contains(&expected), "SD snapshot count bound");
    let text = String::from_utf8(hex::decode(&result.stdout)?)?;
    let mut restarts = Vec::new();
    let mut interruptions = Vec::new();
    for line in text
        .lines()
        .filter_map(|line| line.strip_prefix("MD_SIM_GUEST_INTERRUPTION "))
    {
        ensure!(
            line.len() <= 2 * 1024 * 1024 + 4096 && interruptions.len() < expected,
            "guest interruption evidence bound"
        );
        let record: Value = serde_json::from_str(line)?;
        let observation: GuestInterruption = serde_json::from_value(record["observation"].clone())?;
        observation.verify_serial(&hex::decode(
            record["serial_hex"]
                .as_str()
                .context("interruption serial bytes absent")?,
        )?)?;
        interruptions.push(observation);
    }
    for line in text
        .lines()
        .filter_map(|line| line.strip_prefix("MD_SIM_GUEST_RESTART "))
    {
        ensure!(
            line.len() <= 2 * 1024 * 1024 + 4096 && restarts.len() < expected,
            "guest restart evidence bound"
        );
        let record: Value = serde_json::from_str(line)?;
        let observation: GuestRestart = serde_json::from_value(record["observation"].clone())?;
        let bytes = hex::decode(
            record["serial_hex"]
                .as_str()
                .context("restart serial bytes absent")?,
        )?;
        observation.verify_serial(&bytes)?;
        restarts.push(observation);
    }
    let spec: Value = serde_json::from_slice(&read(&out.join("process-spec.json"), 1 << 20)?)?;
    let backing: ObjectId = serde_json::from_value(
        spec["inputs"]
            .as_array()
            .context("inputs absent")?
            .iter()
            .find(|mount| mount["guest_path"] == storage.backing_path)
            .context("SD input absent")?["object"]
            .clone(),
    )?;
    let mut snapshots = Vec::new();
    for line in text
        .lines()
        .filter_map(|line| line.strip_prefix(storage.marker))
    {
        ensure!(
            line.len() as u64 <= 2 * GZIP_LIMIT + 65536 && snapshots.len() < expected,
            "SD snapshot export count or size bound"
        );
        let value: Value = serde_json::from_str(line)?;
        let state: Snapshot = serde_json::from_value(value["snapshot"].clone())?;
        validate_storage(storage, &state, &backing)?;
        ensure!(
            state.stage == snapshots.len(),
            "SD snapshot sequence differs"
        );
        let compressed = hex::decode(
            value["gzip_hex"]
                .as_str()
                .context("SD image bytes absent")?,
        )?;
        decode(storage, &state, &compressed)?;
        let path = out.join(&state.file);
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?
            .write_all(&compressed)?;
        let manifest = out.join(format!("{}-snapshot-{}.json", storage.name, state.stage));
        save_json(&manifest, &state)?;
        snapshots.push(json!({"manifest":hash_file(&manifest)?,"snapshot":state,"image_info":value["image_info"],"export_conversion":value["export_conversion"]}));
    }
    save_json(
        &out.join(format!("{}-storage.json", storage.name)),
        &json!({"expected_snapshots":expected,"snapshots":snapshots,
        "guest_restarts":restarts,"guest_interruptions":interruptions,
        "execution_result":hash_file(&out.join("result.json"))?,"process_specification":hash_file(&out.join("process-spec.json"))?,
        "scope":"Snapshot integrity only. Persistence requires actual guest write, restart, readback and restoration evidence. Failed execution remains failed."}),
    )?;
    if result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated {
        ensure!(
            snapshots.len() == expected,
            "successful storage run lacks expected snapshots"
        );
        let mut restart_index = 0;
        let mut interruption_index = 0;
        for value in &snapshots {
            if value["snapshot"]["schema"] == format!("{}/4", storage.schema) {
                let interruption = interruptions
                    .get(interruption_index)
                    .context("successful storage run lacks interruption")?;
                ensure!(
                    value["snapshot"]["guest_interruption"] == serde_json::to_value(interruption)?,
                    "snapshot references a different interruption"
                );
                interruption_index += 1;
            } else if value["snapshot"]["schema"] == format!("{}/3", storage.schema) {
                let restart = restarts
                    .get(restart_index)
                    .context("successful storage run lacks native restart")?;
                ensure!(
                    value["snapshot"]["guest_restart"] == serde_json::to_value(restart)?,
                    "SD snapshot references a different native restart"
                );
                restart_index += 1;
            } else {
                ensure!(
                    value["snapshot"]["guest_sync_completed"] == true,
                    "successful storage run lacks completed guest sync"
                );
            }
        }
        ensure!(
            restart_index == restarts.len() && interruption_index == interruptions.len(),
            "unmatched native restart or interruption observation"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_rejects_changed_backing_path_bytes_and_decode_size() {
        let raw = b"unit fixture, not a real qcow2 disk";
        let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
        gzip.write_all(raw).unwrap();
        let bytes = gzip.finish().unwrap();
        let object = |data: &[u8]| ObjectId {
            sha256: md_sim_firmware::prepare::hash_bytes(data),
            size: data.len() as u64,
        };
        let backing = object(b"original fixture");
        let mut state = Snapshot {
            schema: "md-sim-original-boot-sd/1".into(),
            stage: 0,
            backing: backing.clone(),
            raw: object(raw),
            gzip: object(&bytes),
            file: "sd-snapshot-0.qcow2.gz".into(),
            virtual_bytes: VIRTUAL_BYTES,
            scope: "unit fixture".into(),
            guest_sync_completed: None,
            guest_restart: None,
            guest_interruption: None,
        };
        validate(&state, &backing).unwrap();
        assert_eq!(decode(SD, &state, &bytes).unwrap(), raw);
        state.schema = "md-sim-original-boot-sd/2".into();
        assert!(validate(&state, &backing).is_err());
        // An interrupted image remains usable as an explicit recovery input.
        // Its integrity is separate from successful guest synchronization.
        state.guest_sync_completed = Some(false);
        validate(&state, &backing).unwrap();
        state.guest_sync_completed = Some(true);
        validate(&state, &backing).unwrap();
        state.schema = "md-sim-original-boot-sd/3".into();
        assert!(validate(&state, &backing).is_err());
        state.guest_sync_completed = Some(false);
        state.guest_restart = Some(GuestRestart {
            emulator_exit_code: 0,
            serial: object(b"reboot: Restarting system\n"),
        });
        validate(&state, &backing).unwrap();
        state.guest_sync_completed = Some(true);
        assert!(validate(&state, &backing).is_err());
        state.schema = "md-sim-original-boot-sd/2".into();
        assert!(validate(&state, &backing).is_err());
        state.guest_restart = None;
        validate(&state, &backing).unwrap();
        assert!(validate(&state, &object(b"different image")).is_err());
        state.file = "../elsewhere.gz".into();
        assert!(validate(&state, &backing).is_err());
        state.raw.size += 1;
        assert!(decode(SD, &state, &bytes).is_err());
        let mut changed = bytes.clone();
        changed[0] ^= 1;
        assert!(decode(SD, &state, &changed).is_err());

        // A NAND overlay must never be consumed as an SD overlay. The
        // geometry includes all spare bytes, not just256MiB of data.
        state.schema = "md-sim-reference-boot-nand/2".into();
        state.file = "nand-snapshot-0.qcow2.gz".into();
        state.raw = object(raw);
        state.virtual_bytes = 131072 * (2048 + 64);
        validate_storage(NAND, &state, &backing).unwrap();
        assert!(validate_storage(SD, &state, &backing).is_err());
        state.raw.size = 32 * 1024 * 1024 + 1;
        validate_storage(NAND, &state, &backing).unwrap();
        state.raw.size = 64 * 1024 * 1024;
        validate_storage(NAND, &state, &backing).unwrap();
        state.raw.size += 1;
        assert!(validate_storage(NAND, &state, &backing).is_err());
        assert!(decode(NAND, &state, &bytes).is_err());
        state.raw = object(raw);
        state.virtual_bytes = 256 * 1024 * 1024;
        assert!(validate_storage(NAND, &state, &backing).is_err());
        state.virtual_bytes = NAND.virtual_bytes;
        assert!(same_termination(&state, &state).is_err());
        state.schema = "md-sim-reference-boot-nand/3".into();
        state.guest_sync_completed = Some(false);
        state.guest_restart = Some(GuestRestart {
            emulator_exit_code: 0,
            serial: object(b"reboot: Restarting system\n"),
        });
        validate_storage(NAND, &state, &backing).unwrap();
        same_termination(&state, &state).unwrap();
        let mut other: Snapshot =
            serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
        other.guest_restart.as_mut().unwrap().serial =
            object(b"different reboot: Restarting system\n");
        assert!(same_termination(&state, &other).is_err());

        // Manifest edits plus recomputed sidecar hashes must not substitute
        // data which the parent execution never exported.
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let mut sd: Snapshot =
            serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
        sd.schema = "md-sim-original-boot-sd/3".into();
        sd.file = "sd-snapshot-0.qcow2.gz".into();
        sd.virtual_bytes = VIRTUAL_BYTES;
        let sd_path = root.join("sd-snapshot-0.json");
        let nand_path = root.join("nand-snapshot-0.json");
        save_json(&sd_path, &sd).unwrap();
        save_json(&nand_path, &state).unwrap();
        fs::write(root.join("process-spec.json"), b"unit evidence fixture").unwrap();
        let trace = format!(
            "MD_SIM_GUEST_RESTART {}\nMD_SIM_BOOT_SD_SNAPSHOT {}\nMD_SIM_BOOT_NAND_SNAPSHOT {}\n",
            json!({"observation":sd.guest_restart,"serial_hex":hex::encode(b"reboot: Restarting system\n")}),
            json!({"snapshot":sd,"gzip_hex":hex::encode(&bytes)}),
            json!({"snapshot":state,"gzip_hex":hex::encode(&bytes)})
        );
        let mut result = ProcessResult {
            support_virtualization: None,
            private_channel: None,
            stdin_stream: None,
            dashboard: None,
            status: "exited".into(),
            exit_code: Some(0),
            wall_milliseconds: 1,
            stdout: hex::encode(trace),
            stderr: String::new(),
            trace_truncated: false,
            isolation: "unit fixture".into(),
            limits_scope: "unit fixture".into(),
            tools: Default::default(),
            aggregate_limits: None,
        };
        save_json(&root.join("result.json"), &result).unwrap();
        let sidecar = |storage: Storage, path: &Path, state: &Snapshot| {
            save_json(
                &root.join(format!("{}-storage.json", storage.name)),
                &json!({
                    "execution_result":hash_file(&root.join("result.json")).unwrap(),
                    "process_specification":hash_file(&root.join("process-spec.json")).unwrap(),
                    "snapshots":[{"manifest":hash_file(path).unwrap(),"snapshot":state}]
                }),
            )
            .unwrap();
        };
        sidecar(SD, &sd_path, &sd);
        sidecar(NAND, &nand_path, &state);
        verify_pair(&sd_path, &sd, &nand_path, &state).unwrap();
        state.scope = "forged metadata absent from actual output".into();
        save_json(&nand_path, &state).unwrap();
        sidecar(NAND, &nand_path, &state);
        assert!(verify_pair(&sd_path, &sd, &nand_path, &state).is_err());
        // An intentional interruption has its own receipt. It cannot borrow
        // the previous native restart or substitute another captured signal.
        sd.schema = "md-sim-original-boot-sd/4".into();
        state.schema = "md-sim-reference-boot-nand/4".into();
        for snapshot in [&mut sd, &mut state] {
            snapshot.guest_restart = None;
            snapshot.guest_interruption = Some(GuestInterruption {
                emulator_signal: libc::SIGKILL,
                serial: object(b"pending serial\n"),
            });
        }
        save_json(&sd_path, &sd).unwrap();
        save_json(&nand_path, &state).unwrap();
        result.trace_truncated = false;
        result.stdout = hex::encode(format!(
            "MD_SIM_GUEST_INTERRUPTION {}\nMD_SIM_BOOT_SD_SNAPSHOT {}\nMD_SIM_BOOT_NAND_SNAPSHOT {}\n",
            json!({"observation":sd.guest_interruption,"serial_hex":hex::encode(b"pending serial\n")}),
            json!({"snapshot":sd,"gzip_hex":hex::encode(&bytes)}),
            json!({"snapshot":state,"gzip_hex":hex::encode(&bytes)})
        ));
        save_json(&root.join("result.json"), &result).unwrap();
        sidecar(SD, &sd_path, &sd);
        sidecar(NAND, &nand_path, &state);
        verify_pair(&sd_path, &sd, &nand_path, &state).unwrap();
        for snapshot in [&mut sd, &mut state] {
            snapshot.guest_interruption.as_mut().unwrap().serial = object(b"another execution\n");
        }
        save_json(&sd_path, &sd).unwrap();
        save_json(&nand_path, &state).unwrap();
        sidecar(SD, &sd_path, &sd);
        sidecar(NAND, &nand_path, &state);
        assert!(verify_pair(&sd_path, &sd, &nand_path, &state).is_err());
        state.scope = "unit fixture".into();
        save_json(&nand_path, &state).unwrap();
        result.trace_truncated = true;
        save_json(&root.join("result.json"), &result).unwrap();
        sidecar(SD, &sd_path, &sd);
        sidecar(NAND, &nand_path, &state);
        assert!(verify_pair(&sd_path, &sd, &nand_path, &state).is_err());
    }
}

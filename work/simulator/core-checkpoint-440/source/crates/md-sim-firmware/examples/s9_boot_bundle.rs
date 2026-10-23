//! Resolve the archived S9 SD image and execute its actual bootloader in isolation.
//! Direct loading of a boot stage is recorded; it is not an original reset gate.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{
        execute_dashboard_recorded, save_json, Limits, Mount, ProcessResult, ProcessSpec,
        TraceFiles,
    },
    store::{hash_file, ObjectId, Store},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
};
#[allow(dead_code)] // The common pool bundle contract uses this artifact codec.
#[path = "support/worker_channel_artifacts.rs"]
mod channel_artifacts;
#[allow(dead_code)]
#[path = "support/worker_retention_frame.rs"]
mod retention_frame;
#[allow(dead_code)]
#[path = "support/worker_shared_retention.rs"]
mod shared_retention;
use shared_pool as shared_manifest;
#[path = "support/s9_boot_storage.rs"]
mod boot_storage;
#[path = "support/s9_carrier_dashboard.rs"]
mod carrier_dashboard;
#[path = "support/s9_execution_report.rs"]
mod execution_report;
#[path = "support/s9_monitor_adaptation.rs"]
mod monitor_adaptation;
#[path = "support/s9_pool_input.rs"]
mod pool_input;
#[path = "support/s9_pool_report.rs"]
mod pool_report;
use pool_vm::{browser_budget, wall_clock};
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "support/s9_fan_plan.rs"]
mod fan_plan;
#[allow(dead_code)] // The shared module also supplies the independent link probe.
#[path = "support/pool_vm.rs"]
mod pool_vm;
#[path = "support/s9_private_pool.rs"]
mod private_pool;
#[path = "s9_boot_bundle/profiles.rs"]
mod profiles;
#[path = "support/s9_shared_pool.rs"]
mod shared_pool;
#[path = "support/s9_temperature_plan.rs"]
mod temperature_plan;

fn object(bytes: &[u8]) -> ObjectId {
    ObjectId {
        sha256: format!("{:x}", Sha256::digest(bytes)),
        size: bytes.len() as u64,
    }
}
fn le32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..offset + 4)
            .context("short u32")?
            .try_into()?,
    ))
}
fn be32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(
        bytes
            .get(offset..offset + 4)
            .context("short big-endian field")?
            .try_into()?,
    ))
}
fn crc32(bytes: &[u8]) -> u32 {
    let mut value = !0_u32;
    for byte in bytes {
        value ^= *byte as u32;
        for _ in 0..8 {
            value = (value >> 1) ^ (0xedb88320 & (0_u32.wrapping_sub(value & 1)));
        }
    }
    !value
}

#[derive(Debug)]
struct Partition {
    index: usize,
    kind: u8,
    start: usize,
    size: usize,
}
fn partitions(image: &[u8]) -> Result<Vec<Partition>> {
    ensure!(
        image.get(510..512) == Some(&[0x55, 0xaa]),
        "invalid MBR signature"
    );
    let mut output: Vec<Partition> = Vec::new();
    for index in 0..4 {
        let entry = image
            .get(446 + index * 16..462 + index * 16)
            .context("short MBR")?;
        if entry[4] == 0 {
            continue;
        }
        ensure!(
            entry[0] == 0 || entry[0] == 0x80,
            "invalid partition boot flag"
        );
        let start = (le32(entry, 8)? as usize)
            .checked_mul(512)
            .context("partition offset overflow")?;
        let size = (le32(entry, 12)? as usize)
            .checked_mul(512)
            .context("partition size overflow")?;
        let end = start.checked_add(size).context("partition end overflow")?;
        ensure!(
            start >= 512 && size > 0 && end <= image.len(),
            "partition outside image"
        );
        ensure!(
            output
                .iter()
                .all(|p| end <= p.start || start >= p.start + p.size),
            "overlapping partitions"
        );
        output.push(Partition {
            index: index + 1,
            kind: entry[4],
            start,
            size,
        });
    }
    ensure!(!output.is_empty(), "no partitions");
    Ok(output)
}

fn prepare_inside(profile: &profiles::Image, inspect: bool) -> Result<()> {
    confinement::require(true)?;
    ensure!(
        hash_file(Path::new("/sd.img"))? == profile.object(),
        "original SD image identity differs"
    );
    let image = fs::read("/sd.img")?;
    let parts = partitions(&image)?;
    let mut files = BTreeMap::new();
    let mut part_records = Vec::new();
    let mut inspections = BTreeMap::new();
    for part in parts {
        let bytes = &image[part.start..part.start + part.size];
        part_records.push(json!({"index":part.index,"mbr_type":part.kind,"offset":part.start,"size":part.size,"object":object(bytes),"first64_hex":hex::encode(&bytes[..64]),"nonzero_bytes":bytes.iter().filter(|byte|**byte!=0).count(),"non_ff_bytes":bytes.iter().filter(|byte|**byte!=0xff).count()}));
        if matches!(part.kind, 0x06 | 0x0b | 0x0c | 0x0e) {
            ensure!(files.is_empty(), "multiple boot FAT volumes");
            fs::write("/work/boot.fat", bytes)?;
            let extraction = Command::new("/tools/7z")
                .args(["x", "-y", "-o/work/boot", "/work/boot.fat"])
                .stdin(Stdio::null())
                .output()?;
            ensure!(
                extraction.status.success(),
                "FAT extraction failed: {}",
                String::from_utf8_lossy(&extraction.stderr)
            );
            ensure!(
                extraction.stdout.len() + extraction.stderr.len() < 1024 * 1024,
                "tool diagnostic budget exceeded"
            );
            for (index, entry) in fs::read_dir("/work/boot")?.enumerate() {
                ensure!(index < 64, "too many boot files");
                let entry = entry?;
                ensure!(
                    entry.file_type()?.is_file() && entry.metadata()?.len() <= 32 * 1024 * 1024,
                    "unexpected boot entry"
                );
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("invalid boot filename"))?;
                ensure!(
                    !name.contains(['/', '\\']) && name != "." && name != "..",
                    "unsafe boot name"
                );
                files.insert(name, fs::read(entry.path())?);
            }
        } else {
            let path = format!("/work/partition-{}.img", part.index);
            fs::write(&path, bytes)?;
            let result = Command::new("/tools/7z")
                .args(["l", "-slt", &path])
                .stdin(Stdio::null())
                .output()?;
            ensure!(
                result.stdout.len() + result.stderr.len() < 1024 * 1024,
                "storage inspection output exceeds budget"
            );
            inspections.insert(format!("partition-{}-inspection.json",part.index),serde_json::to_vec_pretty(&json!({"exit_code":result.status.code(),"stdout":String::from_utf8(result.stdout)?,"stderr":String::from_utf8(result.stderr)?,"parser":"pinned 7-Zip executed in isolated preparation","input":object(bytes)}))?);
        }
    }
    ensure!(
        !files.is_empty(),
        "no supported boot FAT volume: {part_records:?}"
    );
    let members: BTreeMap<_, _> = files
        .iter()
        .map(|(name, bytes)| (name, object(bytes)))
        .collect();
    if inspect {
        let manifest = json!({
            "schema_version":1,"artifact_id":profile.artifact_id,
            "image":object(&image),"partitions":part_records,"boot_files":members,
            "claim":"Isolated archive inspection only. No executable bundle or hardware compatibility claim."
        });
        files.extend(inspections);
        files.insert(
            "inspection.json".into(),
            serde_json::to_vec_pretty(&manifest)?,
        );
        return export_files(files);
    }
    let (spl_hash, uboot_hash) = profile
        .boot_members
        .context("boot members require separate exact-image inspection")?;
    let spl = files.get("boot.bin").context("missing SPL")?;
    ensure!(
        object(spl).sha256 == spl_hash,
        "SPL differs from verified member"
    );
    ensure!(
        le32(spl, 0x20)? == 0xaa995566 && le32(spl, 0x24)? == 0x584c4e58,
        "invalid Zynq header magic"
    );
    let sum = (0x20..=0x48)
        .step_by(4)
        .try_fold(0_u32, |sum, offset| -> Result<u32> {
            Ok(sum.wrapping_add(le32(spl, offset)?))
        })?;
    ensure!(sum == u32::MAX, "Zynq header checksum differs");
    let source_offset = le32(spl, 0x30)? as usize;
    ensure!(
        source_offset == 0x8c0 && le32(spl, 0x38)? == 0 && le32(spl, 0x3c)? == 0,
        "unexpected SPL placement"
    );
    let spl_payload = spl
        .get(source_offset..)
        .context("SPL source offset outside file")?
        .to_vec();
    let uboot = files.get("u-boot.img").context("missing U-Boot")?;
    ensure!(
        object(uboot).sha256 == uboot_hash,
        "U-Boot differs from verified member"
    );
    ensure!(
        be32(uboot, 0)? == 0x27051956
            && be32(uboot, 12)? as usize == uboot.len() - 64
            && be32(uboot, 16)? == 0x4000000
            && be32(uboot, 20)? == 0x4000000,
        "unexpected U-Boot placement"
    );
    let mut header = uboot[..64].to_vec();
    header[4..8].fill(0);
    ensure!(
        crc32(&header) == be32(uboot, 4)? && crc32(&uboot[64..]) == be32(uboot, 24)?,
        "U-Boot CRC mismatch"
    );
    let uboot_payload = uboot[64..].to_vec();
    let compressed_fpga = files.get("system.bit.gz").context("missing FPGA image")?;
    let fpga = md_sim_firmware::bitstream::from_gzip(compressed_fpga, 4 * 1024 * 1024)?;
    let configuration =
        md_sim_firmware::bitstream::series7::inspect(fpga.configuration(), 0x03722093)?;
    let part = md_sim_firmware::bitstream::series7::frames::Part::from_pinned_json(
        include_bytes!("../../../fixtures/fpga/xc7z010clg400-1/part.json"),
        "1754ec1c7a8e0447a116984505cc422819d3be06389527de96bf192c5175d095",
    )?;
    let frames = md_sim_firmware::bitstream::series7::frames::map_configuration(
        fpga.configuration(),
        &part,
    )?;
    let frame_manifest = serde_json::to_vec(&frames)?;
    let fpga_record = json!({
        "compressed":object(compressed_fpga),
        "decompressed":object(fpga.bytes()),
        "configuration":object(fpga.configuration()),
        "header":fpga.header(),
        "packet_inspection":configuration,
        "frame_mapping":{"file":"fpga-frames.json","object":object(&frame_manifest),"part_sha256":frames.part_sha256,"frames":frames.frames.len(),"mapped_data_words":frames.mapped_data_words,"checked_padding_words":frames.checked_padding_words,"all_frame_ecc_checked":true},
        "claim":"Gzip CRC, container, Series-7 packets, configuration CRCs, xc7z010 IDCODE, complete part-based frame addresses, zero padding and frame ECC checked. No frame-to-logic reconstruction or model compatibility claim."
    });
    let stages = json!({"spl":{"source_offset":source_offset,"header_declared_length":le32(spl,0x34)?,"available_payload_bytes":spl_payload.len(),"load":0,"entry":0,"object":object(&spl_payload),"limit":"Header declares total file length rather than available bytes after offset; direct-stage probe loads only original available suffix, not an emulated boot-ROM copy."},"uboot":{"source_offset":64,"load":0x4000000_u32,"entry":0x4000000_u32,"header_crc_checked":true,"payload_crc_checked":true,"object":object(&uboot_payload)}});
    let manifest = json!({"schema_version":1,"artifact_id":profile.artifact_id,"image":object(&image),"partitions":part_records,"boot_files":members,"stages":stages,"fpga":fpga_record,"claim":"Original archived SD bytes; hardware compatibility and boot execution remain unverified."});
    files.insert("spl-payload.bin".into(), spl_payload);
    files.insert("uboot-payload.bin".into(), uboot_payload);
    files.insert("system.bit".into(), fpga.into_bytes());
    files.insert("fpga-frames.json".into(), frame_manifest);
    files.extend(inspections);
    files.insert("bundle.json".into(), serde_json::to_vec_pretty(&manifest)?);
    export_files(files)
}

fn export_files(files: BTreeMap<String, Vec<u8>>) -> Result<()> {
    let mut output = tar::Builder::new(std::io::stdout().lock());
    for (name, bytes) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o444);
        header.set_cksum();
        output.append_data(&mut header, name, Cursor::new(bytes))?;
    }
    output.finish()?;
    Ok(())
}

struct PayloadContents {
    members: BTreeMap<String, serde_json::Value>,
    files: BTreeMap<String, Vec<u8>>,
}
fn inspect_payload(payload: &[u8], retain_firmware: bool) -> Result<PayloadContents> {
    let mut members = BTreeMap::new();
    let mut files = BTreeMap::new();
    let mut total = 0_u64;
    let mut preview_bytes = 0;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(Cursor::new(payload)));
    for (index, entry) in archive.entries()?.enumerate() {
        ensure!(index < 4096, "too many firmware payload members");
        let mut entry = entry?;
        let path = entry.path()?.to_string_lossy().into_owned();
        ensure!(
            path.len() <= 512 && !members.contains_key(&path),
            "duplicate or oversized member path"
        );
        let size = entry.size();
        total = total.checked_add(size).context("payload size overflow")?;
        ensure!(
            size <= 64 * 1024 * 1024 && total <= 256 * 1024 * 1024,
            "payload expansion exceeds bound"
        );
        let mut data = Vec::new();
        entry.read_to_end(&mut data)?;
        ensure!(data.len() as u64 == size, "short member");
        let preview = if size <= 65536
            && preview_bytes + size <= 128 * 1024
            && entry.header().entry_type().is_file()
        {
            let text = std::str::from_utf8(&data).ok();
            if text.is_some() {
                preview_bytes += size;
            }
            text
        } else {
            None
        };
        members.insert(path.clone(), json!({"object":object(&data),"tar_type":entry.header().entry_type().as_byte(),"mode":entry.header().mode()?,"link_target":entry.link_name()?.map(|p|p.to_string_lossy().into_owned()),"small_utf8_contents":preview}));
        if retain_firmware && entry.header().entry_type().is_file() {
            if let Some((_, leaf)) = path.split_once("/firmware/") {
                ensure!(
                    !leaf.is_empty() && !leaf.contains(['/', '\\']) && leaf != "." && leaf != "..",
                    "unsafe firmware component path"
                );
                ensure!(
                    files.insert(leaf.into(), data).is_none(),
                    "duplicate firmware component"
                );
            }
        }
    }
    Ok(PayloadContents { members, files })
}

fn storage_inside(components: bool) -> Result<()> {
    confinement::require(true)?;
    ensure!(
        hash_file(Path::new("/sd.img"))? == profiles::select("sd2022")?.object(),
        "original SD image identity differs"
    );
    let image = fs::read("/sd.img")?;
    let parts = partitions(&image)?;
    let overlay = parts
        .iter()
        .find(|p| p.index == 2 && p.kind == 0x83)
        .context("overlay partition missing")?;
    let overlay_bytes = &image[overlay.start..overlay.start + overlay.size];
    ensure!(
        object(overlay_bytes).sha256
            == "663b345610b7aecd9843f8a96fc5dc5c1d630b18f218b5f70812129af4d9acae",
        "overlay partition differs"
    );
    fs::write("/work/overlay.img", overlay_bytes)?;
    let extraction = Command::new("/tools/7z")
        .args([
            "x",
            "-so",
            "/work/overlay.img",
            "upper/usr/share/upgrade/firmware.tar.gz",
        ])
        .stdin(Stdio::null())
        .output()?;
    ensure!(
        extraction.status.success()
            && extraction.stdout.len() == 32_090_330
            && extraction.stderr.len() < 65536,
        "firmware payload extraction failed or size differs"
    );
    let payload = extraction.stdout;
    let contents = inspect_payload(&payload, components)?;
    let members = contents.members;
    let mut exports = contents.files;
    let stage2_members = if components {
        Some(inspect_payload(exports.get("stage2.tgz").context("stage2 missing")?, false)?.members)
    } else {
        None
    };
    let third = parts
        .iter()
        .find(|p| p.index == 3 && p.kind == 0xbb)
        .context("third partition missing")?;
    let third_bytes = &image[third.start..third.start + third.size];
    let uboot_prefix = object(
        third_bytes
            .get(..573212)
            .context("short raw boot partition")?,
    );
    let report = json!({"schema_version":1,"original_image":object(&image),"overlay_partition":object(overlay_bytes),"firmware_member":"upper/usr/share/upgrade/firmware.tar.gz","firmware_payload":object(&payload),"payload_members":members,"stage2_members":stage2_members,"third_partition":{"object":object(third_bytes),"uboot_sized_prefix":uboot_prefix,"prefix_matches_verified_uboot_payload":uboot_prefix.sha256=="ce991c8b0255a287fb78163ac3763d496a5a473ec6810e35976d6ce42d100644","tail_after_uboot_bytes":third_bytes.len()-573212,"tail_all_zero":third_bytes[573212..].iter().all(|v|*v==0),"tail_all_ff":third_bytes[573212..].iter().all(|v|*v==255)},"claim":"Exact archived storage payload inspection; no installer execution, NAND geometry selection or board compatibility inference."});
    let report_bytes = serde_json::to_vec_pretty(&report)?;
    ensure!(
        report_bytes.len() < 1024 * 1024,
        "storage report exceeds export budget"
    );
    let mut output = tar::Builder::new(std::io::stdout().lock());
    if !components {
        exports.insert("firmware.tar.gz".into(), payload);
    }
    exports.insert("storage.json".into(), report_bytes);
    ensure!(
        exports.values().map(|bytes| bytes.len()).sum::<usize>() < 32 * 1024 * 1024 - 65536,
        "component export exceeds trace budget"
    );
    for (name, bytes) in exports {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o444);
        header.set_cksum();
        output.append_data(&mut header, name, Cursor::new(bytes))?;
    }
    output.finish()?;
    Ok(())
}

fn add(store: &Store, inputs: &mut Vec<Mount>, path: &Path, guest: &str) -> Result<()> {
    let object = hash_file(path)?;
    store.import(path, &object)?;
    inputs.push(Mount {
        object,
        guest_path: guest.into(),
    });
    Ok(())
}

fn execute_recorded(
    store: &Store,
    spec: &ProcessSpec,
    out: &Path,
    trace: Option<TraceFiles>,
) -> Result<ProcessResult> {
    match execute_dashboard_recorded(
        store,
        spec,
        Arc::new(AtomicBool::new(false)),
        trace,
        None,
        None,
        Some(&out.join("dashboard.json")),
    ) {
        Ok(result) => Ok(result),
        Err(error) => {
            save_json(
                &out.join("infrastructure-error.json"),
                &json!({"error":format!("{error:#}"),"original_board_gate":"incomplete"}),
            )?;
            Err(error)
        }
    }
}

fn bundle_image(manifest: &serde_json::Value) -> Result<&'static profiles::Image> {
    ensure!(manifest["schema_version"] == 1, "unsupported bundle schema");
    let profile = profiles::select(
        manifest["artifact_id"]
            .as_str()
            .context("missing artifact ID")?,
    )?;
    ensure!(
        serde_json::from_value::<ObjectId>(manifest["image"].clone())? == profile.object(),
        "bundle image identity differs from selected archive"
    );
    Ok(profile)
}

fn check_stage_manifest(
    manifest: &serde_json::Value,
    stage: &str,
    parent_name: &str,
    parent: &ObjectId,
    payload_offset: usize,
) -> Result<()> {
    let load = if stage == "spl" { 0 } else { 0x4000000 };
    ensure!(
        manifest["stages"][stage]["source_offset"] == payload_offset
            && manifest["stages"][stage]["load"] == load
            && manifest["stages"][stage]["entry"] == load,
        "stage placement differs from executed placement"
    );
    ensure!(
        serde_json::from_value::<ObjectId>(manifest["boot_files"][parent_name].clone())? == *parent,
        "boot member manifest identity differs from original bytes"
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|a| a == "__pool-session") {
        ensure!(args.len() == 2, "pool session takes no arguments");
        return private_pool::session();
    }
    if args
        .get(1)
        .is_some_and(|a| a == "__prepare" || a == "__inspect")
    {
        let profile = profiles::select(
            args.get(2)
                .and_then(|a| a.to_str())
                .context("missing image selection")?,
        )?;
        return prepare_inside(profile, args[1] == "__inspect");
    }
    if args.get(1).is_some_and(|a| a == "__storage") {
        return storage_inside(false);
    }
    if args.get(1).is_some_and(|a| a == "__payload") {
        return storage_inside(true);
    }
    if args
        .get(1)
        .is_some_and(|a| a == "__boot" || a == "__boot-console")
    {
        let shared = shared_pool::enabled()?;
        let shared_endpoint = shared.then(shared_pool::take_endpoint).transpose()?;
        let local_pool = std::env::var("MD_SIM_S9_LOCAL_POOL").as_deref() == Ok("1");
        let batch_console = match std::env::var("MD_SIM_S9_BATCH_CONSOLE") {
            Err(std::env::VarError::NotPresent) => false,
            Ok(value) if value == "0" => false,
            Ok(value) if value == "1" => true,
            _ => anyhow::bail!("invalid batch console policy"),
        };
        let use_pool_vm = std::env::var("MD_SIM_S9_POOL_VM").as_deref() == Ok("1");
        let browser = std::env::var("MD_SIM_S9_BROWSER").as_deref() == Ok("1");
        let nested_tcg = confinement::nested_mining_profile()?;
        ensure!(
            !nested_tcg || (local_pool && !browser && !use_pool_vm && !shared),
            "nested TCG profile requires the pipe pool"
        );
        ensure!(!browser || use_pool_vm, "browser requires support VM");
        ensure!(
            !use_pool_vm || local_pool,
            "pool VM requires local pool mode"
        );
        ensure!(
            !shared || (local_pool && !use_pool_vm && !browser && args[1] == "__boot-console"),
            "shared pool requires one native console miner without an embedded pool VM"
        );
        if browser {
            confinement::require_mining_browser_board()?;
        } else if local_pool {
            confinement::require_mining_execution()?;
        } else {
            confinement::require_board()?;
        }
        let save_sd = std::env::var("MD_SIM_S9_SAVE_SD").as_deref() == Ok("1");
        let nand_storage = match std::env::var("MD_SIM_S9_NAND_STORAGE") {
            Err(std::env::VarError::NotPresent) => false,
            Ok(value) if value == "0" => false,
            Ok(value) if value == "1" => true,
            _ => anyhow::bail!("invalid NAND storage policy"),
        };
        ensure!(
            !nand_storage || save_sd,
            "NAND storage requires the paired SD capture"
        );
        let sd_native_reboot = std::env::var("MD_SIM_S9_SD_NATIVE_REBOOT").as_deref() == Ok("1");
        let sd_interrupt = std::env::var("MD_SIM_S9_SD_INTERRUPT").as_deref() == Ok("1");
        let sd_checkpoint = std::env::var("MD_SIM_S9_SD_CHECKPOINT").as_deref() == Ok("1");
        let dashboard_probe = std::env::var("MD_SIM_S9_DASHBOARD_PROBE").as_deref() == Ok("1");
        ensure!(
            !sd_native_reboot || save_sd,
            "native SD reboot requires saved storage"
        );
        ensure!(
            !sd_checkpoint || sd_native_reboot,
            "storage checkpoint requires native reboot"
        );
        let restarts: usize = std::env::var("MD_SIM_S9_RESTARTS")
            .unwrap_or_else(|_| "0".into())
            .parse()?;
        ensure!(
            !sd_interrupt || (save_sd && !sd_native_reboot && !sd_checkpoint && restarts == 0),
            "interrupted storage requires one saved boot, without native restart or checkpoint"
        );
        ensure!(
            restarts <= 2 && (restarts == 0 || (save_sd && !local_pool)),
            "unsupported SD restart contract"
        );
        if std::env::var("MD_SIM_S9_RESTORE_SD").as_deref() == Ok("1") {
            boot_storage::restore()?;
        } else {
            boot_storage::create_sd()?;
        }
        if nand_storage {
            if Path::new("/nand-state.json").exists() {
                boot_storage::restore_storage(boot_storage::NAND)?;
            } else {
                boot_storage::create(boot_storage::NAND)?;
            }
        }
        let mut command = Command::new("/qemu");
        command.args(&args[2..]);
        let _dashboard_agent = md_sim_runtime::dashboard::Agent::start_from_environment()?;
        let mut pool = if local_pool && !use_pool_vm && !shared {
            Some(private_pool::LocalPool::start()?)
        } else {
            None
        };
        let clock = std::env::var(wall_clock::ENVIRONMENT)
            .ok()
            .map(|value| wall_clock::Clock::new(value.parse()?))
            .transpose()?;
        ensure!(
            clock.is_none() || (use_pool_vm && args[1] == "__boot-console"),
            "wall-clock adaptation requires a pool VM and original guest console"
        );
        let rtc = clock.as_ref().map(wall_clock::Clock::rtc).transpose()?;
        let (mut vm_pool, miner_endpoint) = if use_pool_vm {
            let (pool, endpoint) = pool_vm::MinerPool::start(browser, rtc.as_deref())?;
            pool_vm::attach(&mut command, &endpoint);
            (Some(pool), Some(endpoint))
        } else {
            (None, None)
        };
        let mut shared_observation = shared_endpoint
            .map(|endpoint| {
                let input = pool_input::read(Path::new("/private-pool.json"))?;
                shared_pool::Observation::attach(&mut command, endpoint, input.seconds)
            })
            .transpose()?;
        let _miner_deadline = if browser {
            let seconds: u64 = std::env::var("MD_SIM_S9_MINER_WALL_SECONDS")?.parse()?;
            ensure!((30..=360).contains(&seconds), "miner wall bound differs");
            let cpu = (seconds - 10).min(330);
            let guard = browser_budget::MiningDeadline::start(seconds, cpu)?;
            println!(
                "MD_SIM_MINER_EXECUTION_BOUNDS {}",
                json!({"wall_seconds":seconds,"cpu_seconds":cpu,"address_bytes":browser_budget::MINER_ADDRESS_BYTES,"browser_started_before_miner":true})
            );
            Some(guard)
        } else {
            None
        };
        if args[1] == "__boot-console" {
            ensure!(
                fs::metadata("/console-script")?.len() <= 65536,
                "console script exceeds limit"
            );
            let mut first = Some(command);
            let mut result = Ok(());
            for stage in 0..=restarts {
                let command = first.take().unwrap_or_else(|| {
                    let mut next = Command::new("/qemu");
                    next.args(&args[2..]);
                    next
                });
                println!("\nMD_SIM_BOOT_SD_STAGE {stage}");
                let mut guest = if let Some(observation) = shared_observation.as_mut() {
                    md_sim_firmware::serial_console::SerialGuest::start_mining_with_spawned(
                        command,
                        true,
                        || observation.spawned(),
                    )?
                } else if local_pool {
                    md_sim_firmware::serial_console::SerialGuest::start_mining(command, true)?
                } else {
                    md_sim_firmware::serial_console::SerialGuest::start(command, true)?
                };
                if batch_console {
                    guest.enter_batch_shell()?;
                }
                let script = if stage == 0 {
                    "/console-script".into()
                } else {
                    format!("/restart-console-{}", stage - 1)
                };
                ensure!(
                    fs::metadata(&script)?.len() <= 65536,
                    "restart script exceeds limit"
                );
                result = if let Some(clock) = &clock {
                    guest.script(clock.script()?)
                } else {
                    Ok(())
                };
                if result.is_ok() && Path::new("/monitor-adaptation-script").exists() {
                    result = guest.script(fs::read("/monitor-adaptation-script")?);
                }
                if result.is_ok() {
                    result = guest.script(fs::read(script)?);
                }
                if result.is_ok() {
                    if let Some(pool) = pool.as_mut() {
                        result = guest.drain_while(|| pool.wait_mining_window());
                    }
                }
                if result.is_ok() && browser {
                    result = guest.drain_while(|| {
                        vm_pool
                            .as_mut()
                            .context("browser support VM absent")?
                            .wait_browser()
                    });
                }
                let mut native_restart = None;
                let mut interruption = None;
                let update_action = pool_vm::update_validation::selected_action()?;
                ensure!(
                    !sd_interrupt || update_action.is_none(),
                    "controlled interruption cannot replace an update operation"
                );
                ensure!(
                    !sd_checkpoint || update_action.is_none(),
                    "storage checkpoint cannot precede a native update operation"
                );
                ensure!(
                    !sd_native_reboot
                        || update_action != Some(pool_vm::update_validation::Action::InstallKeep),
                    "native SD reboot cannot replace the installer restart"
                );
                if result.is_ok() && update_action.is_some() {
                    let wait = || {
                        vm_pool
                            .as_mut()
                            .context("update support VM absent")?
                            .wait_update(update_action.context("update action absent")?)
                    };
                    result =
                        if update_action == Some(pool_vm::update_validation::Action::InstallKeep) {
                            guest
                                .wait_for_restart_exit(
                                    pool_vm::update_validation::RESTART_SECONDS,
                                    wait,
                                )
                                .map(|((), observation)| native_restart = Some(observation))
                        } else {
                            guest.drain_while(wait)
                        };
                }
                // The pool's work window applies to every live miner. A
                // short diagnostic script must not kill it while the TCP
                // support VM still waits for low-rate work. An observed
                // native restart has already ended this guest process.
                if result.is_ok() && native_restart.is_none() {
                    if let Some(pool) = vm_pool.as_mut() {
                        result = guest.drain_while(|| pool.wait_mining_window());
                    }
                    if let Some(observation) = shared_observation.as_ref() {
                        result = guest.drain_while(|| observation.wait_window());
                    }
                }
                if save_sd && result.is_ok() && native_restart.is_none() && sd_checkpoint {
                    if dashboard_probe && result.is_ok() {
                        result = guest.drain_while(|| {
                            vm_pool
                                .as_mut()
                                .context("checkpoint HTTP support VM absent")?
                                .wait_dashboard()
                        });
                    }
                    if result.is_ok() {
                        result = boot_storage::checkpoint_native(&mut guest);
                    }
                }
                if save_sd && result.is_ok() && native_restart.is_none() {
                    result = if sd_interrupt {
                        guest
                            .interrupt_and_exit(30)
                            .map(|observation| interruption = Some(observation))
                    } else if sd_native_reboot {
                        guest
                            .reboot_and_exit(120)
                            .map(|observation| native_restart = Some(observation))
                    } else {
                        guest.script(b"sync\n".to_vec())
                    };
                }
                drop(guest);
                if save_sd {
                    let nand_capture = if nand_storage {
                        if let Some(observation) = &interruption {
                            boot_storage::capture_interrupted(
                                boot_storage::NAND,
                                stage,
                                observation.clone(),
                            )
                        } else {
                            boot_storage::capture_storage(
                                boot_storage::NAND,
                                stage,
                                native_restart.is_none() && result.is_ok(),
                                native_restart.clone(),
                            )
                        }
                    } else {
                        Ok(())
                    };
                    let sd_capture = if let Some(observation) = interruption {
                        boot_storage::capture_interrupted(boot_storage::SD, stage, observation)
                    } else if let Some(observation) = native_restart {
                        boot_storage::capture_restart(stage, observation)
                    } else {
                        boot_storage::capture(stage, result.is_ok())
                    };
                    for (storage, capture) in [("nand", &nand_capture), ("sd", &sd_capture)] {
                        if let Err(error) = capture {
                            eprintln!(
                                "MD_SIM_STORAGE_CAPTURE_FAILURE {}",
                                json!({"storage":storage,"stage":stage,"error":format!("{error:#}")})
                            );
                        }
                    }
                    // Preserve the other disk and pool evidence even if one
                    // export fails. Any failure still makes this run fail.
                    result = result.and(nand_capture).and(sd_capture);
                }
                if result.is_err() {
                    break;
                }
            }
            drop(miner_endpoint);
            if let Some(pool) = pool {
                pool.finish()?;
            }
            if let Some(pool) = vm_pool {
                pool.finish()?;
            }
            return result;
        }
        let status = command.stdin(Stdio::null()).status()?;
        ensure!(status.success(), "QEMU exited: {status}");
        return Ok(());
    }
    if args.get(1).is_some_and(|a| a == "boot") {
        return boot(&args);
    }
    ensure!(
        (3..=4).contains(&args.len())
            && (args[1] == "inspect"
                || args[1] == "prepare"
                || args[1] == "storage"
                || args[1] == "payload"),
        "usage: s9_boot_bundle inspect|prepare|storage|payload NEW_OUTPUT [sd2022|sd2020]"
    );
    let inspect = args[1] == "inspect";
    let storage = args[1] == "storage" || args[1] == "payload";
    let components = args[1] == "payload";
    let profile = profiles::select(
        args.get(3)
            .map(|s| s.to_str().context("invalid image selection"))
            .transpose()?
            .unwrap_or("sd2022"),
    )?;
    ensure!(
        !storage || profile.name == "sd2022",
        "storage payload inspection requires its pinned sd2022 layout"
    );
    ensure!(
        inspect || storage || profile.boot_members.is_some(),
        "inspect and pin boot members before preparing this image"
    );
    ensure!(
        hash_file(Path::new(profile.path))? == profile.object(),
        "original SD image identity differs"
    );
    let out = PathBuf::from(&args[2]);
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(&out)?;
    let store = Store::open(&out.join("input-store"), 512 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/prepare".into(), std::env::current_exe()?),
            ("/tools/7z".into(), PathBuf::from("/usr/lib/7zip/7z")),
            ("/tools/7z.so".into(), PathBuf::from("/usr/lib/7zip/7z.so")),
        ],
    )?;
    add(&store, &mut inputs, Path::new(profile.path), "/sd.img")?;
    let spec = ProcessSpec {
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        stdin_object: None,
        argv: vec![
            "/prepare".into(),
            if components {
                "__payload"
            } else if storage {
                "__storage"
            } else if inspect {
                "__inspect"
            } else {
                "__prepare"
            }
            .into(),
            profile.name.into(),
        ],
        inputs,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 30,
            cpu_seconds: 20,
            memory_bytes: 2 * 1024 * 1024 * 1024,
            processes: 32,
            file_bytes: 64 * 1024 * 1024,
            trace_bytes: 64 * 1024 * 1024,
            tmp_bytes: 256 * 1024 * 1024,
        },
    };
    save_json(&out.join("preparation-spec.json"), &spec)?;
    let result = execute_recorded(&store, &spec, &out, None)?;
    let exported = hex::decode(&result.stdout)?;
    fs::write(
        out.join("preparation-stderr.log"),
        hex::decode(&result.stderr)?,
    )?;
    save_json(
        &out.join("preparation-result.json"),
        &json!({"status":result.status,"exit_code":result.exit_code,"trace_truncated":result.trace_truncated,"elapsed_ms":result.wall_milliseconds,"export":object(&exported),"tools":result.tools}),
    )?;
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated,
        "isolated preparation failed; see retained output"
    );
    let export_dir = out.join(if storage { "storage" } else { "boot" });
    fs::create_dir(&export_dir)?;
    let mut archive = tar::Archive::new(Cursor::new(exported));
    for (index, entry) in archive.entries()?.enumerate() {
        ensure!(index < 64, "too many exported members");
        let mut entry = entry?;
        let name = entry.path()?.to_string_lossy().into_owned();
        ensure!(
            !name.contains(['/', '\\'])
                && name != "."
                && name != ".."
                && entry.header().entry_type().is_file()
                && entry.size() <= 32 * 1024 * 1024,
            "invalid exported member"
        );
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        let destination = export_dir.join(name);
        ensure!(!destination.exists(), "duplicate exported member");
        fs::write(destination, bytes)?;
    }
    println!(
        "{}",
        json!({"output":export_dir,"manifest":hash_file(&export_dir.join(if storage {"storage.json"} else if inspect {"inspection.json"} else {"bundle.json"}))?,"original_board_gate":"incomplete"})
    );
    Ok(())
}

fn boot(args: &[std::ffi::OsString]) -> Result<()> {
    ensure!(
        (5..=35).contains(&args.len()),
        "usage: s9_boot_bundle boot BUNDLE NEW_OUTPUT spl|uboot [trace|nand-instructions|nand-registers|nand-pins] [single-cpu] [nand-fixture|nand-onfi|legacy-io|sd2022-io|sd2022-v1] [backend=BUILD_DIRECTORY] [budget=30..360|nested-tcg budget=1200|1800|2400|4800] [nested-tcg-two-cpu] [pool-seconds=20..300|nested-tcg:20..900] [private-loopback [dashboard-forward] [pool-keep-open]] [batch-console] [console=SCRIPT] [restart-console=SCRIPT] [save-sd] [nand-storage] [nand-state=SNAPSHOT_JSON] [sd-native-reboot|sd-controlled-interruption] [sd-read-only-checkpoint] [sd-state=SNAPSHOT_JSON] [presence-pin-fixture=HEX] [fan-fixture [fan-plan=JSON_FILE]] [pic-contract] [asic-contract] [configuration-contract] [asic-events|asic-packets] [local-pool] [pool-vm=BUILD_DIRECTORY|shared-pool=MANIFEST prepare-only] [shared-dashboard=POLICY] [dashboard-probe] [dashboard-luci] [dashboard-update-validation|dashboard-update-install|dashboard-update-corruption=IMAGE] [dashboard-state=PLAN_JSON] [browser-payload=MANIFEST] [adapt-clock-epoch=UNIX_SECONDS] [adapt-monitor-fan-source=EXTRACTED_MEMBER] [temperature-fixture [temperature-plan=JSON_FILE]] [pool-difficulty=EXPONENT,EXPONENT] [regtest-genesis] [icount-shift=0..6] [hash-cap-hps=RATE] [adapt-fixed-filter-bits=1..31] [prepare-only]"
    );
    let bundle = PathBuf::from(&args[2]);
    let out = PathBuf::from(&args[3]);
    let stage = args[4].to_str().context("invalid stage")?;
    ensure!(matches!(stage, "spl" | "uboot"), "invalid stage");
    ensure!(
        args[5..].iter().all(|arg| arg == "trace"
            || arg == "nand-instructions"
            || arg == "nand-registers"
            || arg == "nand-pins"
            || arg == "single-cpu"
            || arg == "nand-fixture"
            || arg == "nand-onfi"
            || arg == "legacy-io"
            || arg == "sd2022-io"
            || arg == "sd2022-v1"
            || arg == "fan-fixture"
            || arg.to_str().is_some_and(|s| s.starts_with("fan-plan="))
            || arg == "pic-contract"
            || arg == "asic-contract"
            || arg == "asic-events"
            || arg == "asic-packets"
            || arg == "configuration-contract"
            || arg == "local-pool"
            || arg == "nested-tcg"
            || arg == "nested-tcg-two-cpu"
            || arg.to_str().is_some_and(|s| s.starts_with("pool-seconds="))
            || arg == "private-loopback"
            || arg == "pool-keep-open"
            || arg == "dashboard-forward"
            || arg == "batch-console"
            || arg == "regtest-genesis"
            || arg == "dashboard-probe"
            || arg == "dashboard-luci"
            || arg == "dashboard-update-validation"
            || arg == "dashboard-update-install"
            || arg
                .to_str()
                .is_some_and(|s| s.starts_with("dashboard-update-corruption="))
            || arg == "temperature-fixture"
            || arg
                .to_str()
                .is_some_and(|s| s.starts_with("temperature-plan="))
            || arg == "prepare-only"
            || arg == "save-sd"
            || arg == "nand-storage"
            || arg == "sd-native-reboot"
            || arg == "sd-controlled-interruption"
            || arg == "sd-read-only-checkpoint"
            || arg
                .to_str()
                .is_some_and(|s| s.starts_with("presence-pin-fixture="))
            || arg.to_str().is_some_and(|s| s.starts_with("console="))
            || arg
                .to_str()
                .is_some_and(|s| s.starts_with("restart-console="))
            || arg.to_str().is_some_and(|s| s.starts_with("sd-state="))
            || arg.to_str().is_some_and(|s| s.starts_with("nand-state="))
            || arg.to_str().is_some_and(|s| s.starts_with("icount-shift="))
            || arg.to_str().is_some_and(|s| s.starts_with("hash-cap-hps="))
            || arg
                .to_str()
                .is_some_and(|s| s.starts_with("pool-difficulty="))
            || arg
                .to_str()
                .is_some_and(|s| s.starts_with("adapt-fixed-filter-bits="))
            || arg.to_str().is_some_and(|s| s.starts_with("budget="))
            || arg.to_str().is_some_and(|s| s.starts_with("pool-vm="))
            || arg.to_str().is_some_and(|s| s.starts_with("shared-pool="))
            || arg
                .to_str()
                .is_some_and(|s| s.starts_with("shared-dashboard="))
            || arg
                .to_str()
                .is_some_and(|s| s.starts_with("pool-startup-seconds="))
            || arg
                .to_str()
                .is_some_and(|s| s.starts_with("browser-payload="))
            || arg
                .to_str()
                .is_some_and(|s| s.starts_with("dashboard-state="))
            || arg
                .to_str()
                .is_some_and(|s| s.starts_with("adapt-clock-epoch="))
            || arg
                .to_str()
                .is_some_and(|s| s.starts_with("adapt-monitor-fan-source="))
            || arg.to_str().is_some_and(|s| s.starts_with("backend="))),
        "unknown diagnostic mode"
    );
    let trace = args[5..].iter().any(|arg| arg == "trace");
    let asic_events = args[5..].iter().any(|arg| arg == "asic-events");
    let asic_packets = args[5..].iter().any(|arg| arg == "asic-packets");
    let configuration_contract = args[5..].iter().any(|arg| arg == "configuration-contract");
    ensure!(!(asic_events && asic_packets), "choose one ASIC trace mode");
    let local_pool = args[5..].iter().any(|arg| arg == "local-pool");
    let private_loopback = args[5..].iter().any(|arg| arg == "private-loopback");
    let pool_keep_open = args[5..].iter().any(|arg| arg == "pool-keep-open");
    let dashboard_forward = args[5..].iter().any(|arg| arg == "dashboard-forward");
    let dashboard_probe = args[5..].iter().any(|arg| arg == "dashboard-probe");
    let dashboard_luci = args[5..].iter().any(|arg| arg == "dashboard-luci");
    let dashboard_validate = args[5..]
        .iter()
        .any(|arg| arg == "dashboard-update-validation");
    let dashboard_install = args[5..]
        .iter()
        .any(|arg| arg == "dashboard-update-install");
    let dashboard_corruptions: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| {
            arg.to_str()
                .and_then(|s| s.strip_prefix("dashboard-update-corruption="))
        })
        .collect();
    ensure!(
        usize::from(dashboard_validate)
            + usize::from(dashboard_install)
            + dashboard_corruptions.len()
            <= 1,
        "select one native update action"
    );
    let dashboard_corruption = dashboard_corruptions.first().map(Path::new);
    if let Some(path) = dashboard_corruption {
        pool_vm::update_validation::Action::RejectTampered.checked_image(path)?;
    }
    let dashboard_update =
        dashboard_validate || dashboard_install || dashboard_corruption.is_some();
    ensure!(
        !dashboard_update || dashboard_luci,
        "native update requires dashboard-luci"
    );
    ensure!(
        !dashboard_luci || dashboard_probe,
        "dashboard-luci requires dashboard-probe"
    );
    let dashboard_states: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| {
            arg.to_str()
                .and_then(|s| s.strip_prefix("dashboard-state="))
        })
        .collect();
    ensure!(
        dashboard_states.len() <= 1 && (dashboard_states.is_empty() || dashboard_probe),
        "one dashboard-state plan requires dashboard-probe"
    );
    let dashboard_state = dashboard_states.first().map(Path::new);
    let dashboard_suite = if let Some(path) = dashboard_corruption {
        pool_vm::DashboardSuite::UpdateRejected(path)
    } else if dashboard_install {
        pool_vm::DashboardSuite::UpdateInstall
    } else if dashboard_validate {
        pool_vm::DashboardSuite::UpdateValidation
    } else if dashboard_luci {
        pool_vm::DashboardSuite::Luci
    } else if dashboard_probe {
        pool_vm::DashboardSuite::Basic
    } else {
        pool_vm::DashboardSuite::None
    };
    let regtest = args[5..].iter().any(|arg| arg == "regtest-genesis");
    ensure!(
        !regtest || local_pool,
        "regtest-genesis requires local-pool"
    );
    let regtest_anchor = regtest.then(md_sim_mining::regtest::RegtestAnchor::genesis);
    let pool_vm_args: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| arg.to_str().and_then(|s| s.strip_prefix("pool-vm=")))
        .collect();
    ensure!(
        pool_vm_args.len() <= 1 && (pool_vm_args.is_empty() || local_pool),
        "one pool VM selection requires local-pool"
    );
    let pool_vm_backend = pool_vm_args.first().map(Path::new);
    let shared_paths: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| arg.to_str().and_then(|s| s.strip_prefix("shared-pool=")))
        .collect();
    ensure!(
        shared_paths.len() <= 1
            && (shared_paths.is_empty()
                || (local_pool
                    && pool_vm_backend.is_none()
                    && args[5..].iter().any(|arg| arg == "prepare-only"))),
        "one shared-pool manifest requires local-pool prepare-only without pool-vm"
    );
    let shared_input = shared_paths
        .first()
        .map(|path| shared_pool::read(Path::new(path)))
        .transpose()?;
    let shared = shared_input.as_ref().map(|(manifest, _)| manifest);
    let dashboard_policies: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| {
            arg.to_str()
                .and_then(|s| s.strip_prefix("shared-dashboard="))
        })
        .collect();
    ensure!(
        dashboard_policies.len() <= 1 && (dashboard_policies.is_empty() || shared.is_some()),
        "one shared dashboard policy requires the selected shared pool"
    );
    let wall_epochs: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| {
            arg.to_str()
                .and_then(|s| s.strip_prefix("adapt-clock-epoch="))
        })
        .collect();
    ensure!(
        wall_epochs.len() <= 1 && (wall_epochs.is_empty() || pool_vm_backend.is_some()),
        "one explicit wall-clock epoch requires pool-vm"
    );
    let wall_epoch = wall_epochs
        .first()
        .map(|value| wall_clock::validate(value.parse()?))
        .transpose()?;
    ensure!(
        wall_epoch.is_none()
            || args[5..]
                .iter()
                .any(|arg| arg.to_str().is_some_and(|s| s.starts_with("console="))),
        "wall-clock adaptation requires the original guest console"
    );
    ensure!(
        !dashboard_probe || pool_vm_backend.is_some(),
        "dashboard-probe requires pool-vm"
    );
    let browser_payloads: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| {
            arg.to_str()
                .and_then(|arg| arg.strip_prefix("browser-payload="))
        })
        .collect();
    ensure!(
        browser_payloads.len() <= 1 && (browser_payloads.is_empty() || dashboard_probe),
        "one browser payload requires dashboard-probe"
    );
    let browser_payload = browser_payloads.first().map(Path::new);
    let browser = browser_payload.is_some();
    let pool_startup_values: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| {
            arg.to_str()
                .and_then(|s| s.strip_prefix("pool-startup-seconds="))
        })
        .collect();
    ensure!(
        pool_startup_values.len() <= 1
            && (pool_startup_values.is_empty() || (pool_vm_backend.is_some() && !trace)),
        "one explicit pool startup deadline requires an untraced pool VM"
    );
    let pool_startup_seconds =
        pool_vm::startup_seconds(browser, pool_startup_values.first().copied())?;
    ensure!(
        !browser || !trace,
        "browser startup cannot use the instruction trace budget"
    );
    let pool_difficulty_args: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| {
            arg.to_str()
                .and_then(|s| s.strip_prefix("pool-difficulty="))
        })
        .collect();
    ensure!(
        pool_difficulty_args.len() <= 1 && (pool_difficulty_args.is_empty() || local_pool),
        "one pool difficulty selection requires local-pool"
    );
    let pool_difficulty = if let Some(value) = pool_difficulty_args.first() {
        let (first, second) = value
            .split_once(',')
            .context("pool-difficulty needs two exponents")?;
        [first.parse::<i32>()?, second.parse::<i32>()?]
    } else {
        [-20, -19]
    };
    ensure!(
        pool_difficulty
            .iter()
            .all(|value| (-31..=0).contains(value))
            && pool_difficulty[0] != pool_difficulty[1],
        "pool requires distinct difficulty exponents from-31 through0"
    );
    ensure!(
        !local_pool || args[5..].iter().any(|arg| arg == "asic-contract"),
        "local-pool requires asic-contract"
    );
    ensure!(
        !(asic_events || asic_packets) || args[5..].iter().any(|arg| arg == "asic-contract"),
        "ASIC trace selection requires asic-contract"
    );
    let nand_instructions = args[5..].iter().any(|arg| arg == "nand-instructions");
    let nand_registers = args[5..].iter().any(|arg| arg == "nand-registers");
    let nand_pins = args[5..].iter().any(|arg| arg == "nand-pins");
    ensure!(
        u8::from(trace)
            + u8::from(nand_instructions)
            + u8::from(nand_registers)
            + u8::from(nand_pins)
            <= 1,
        "choose one instruction trace mode"
    );
    let single_cpu = args[5..].iter().any(|arg| arg == "single-cpu");
    let clock_shifts: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| arg.to_str().and_then(|s| s.strip_prefix("icount-shift=")))
        .collect();
    ensure!(clock_shifts.len() <= 1, "duplicate instruction clock");
    let clock_shift = clock_shifts.first().map(|s| s.parse::<u8>()).transpose()?;
    ensure!(
        clock_shift.is_none_or(|shift| shift <= 6),
        "instruction clock shift must be0..6"
    );
    let budgets: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| arg.to_str().and_then(|s| s.strip_prefix("budget=")))
        .collect();
    ensure!(budgets.len() <= 1, "duplicate budget");
    let budget = budgets.first().map_or(Ok(30_u64), |value| value.parse())?;
    let carrier_dashboard = dashboard_policies
        .first()
        .map(|path| {
            let support = object(&shared_input.as_ref().context("shared manifest absent")?.1);
            carrier_dashboard::read(Path::new(path), &support, budget)
        })
        .transpose()?;
    let nested_tcg = args[5..].iter().any(|arg| arg == "nested-tcg");
    ensure!(
        shared.is_none()
            || (!nested_tcg
                && !trace
                && !private_loopback
                && !browser
                && !dashboard_probe
                && !dashboard_forward),
        "shared pool requires the native bounded miner without runner-owned network tests"
    );
    let two_cpu = args[5..].iter().any(|arg| arg == "nested-tcg-two-cpu");
    ensure!(
        !two_cpu
            || (nested_tcg
                && budget == confinement::NESTED_MINING_LONG_BOOT_SECONDS
                && !single_cpu),
        "nested-tcg-two-cpu requires nested-tcg budget4800 and both guest CPUs"
    );
    let nested_profile = if two_cpu {
        "nested-tcg-4800-two-cpu"
    } else if budget == confinement::NESTED_MINING_LONG_BOOT_SECONDS {
        "nested-tcg-4800"
    } else if budget == confinement::NESTED_MINING_SLOW_WALL_SECONDS {
        "nested-tcg-2400"
    } else if budget == confinement::NESTED_MINING_LONG_WALL_SECONDS {
        "nested-tcg-1800"
    } else {
        "nested-tcg"
    };
    ensure!(
        !private_loopback || (local_pool && pool_vm_backend.is_none()),
        "private-loopback is for the isolated pipe pool only"
    );
    ensure!(
        !dashboard_forward || private_loopback,
        "dashboard-forward requires the private-loopback pool"
    );
    ensure!(
        !pool_keep_open || private_loopback,
        "pool-keep-open requires the private-loopback pool"
    );
    ensure!(
        if nested_tcg { [confinement::NESTED_MINING_WALL_SECONDS,confinement::NESTED_MINING_LONG_WALL_SECONDS,confinement::NESTED_MINING_SLOW_WALL_SECONDS,confinement::NESTED_MINING_LONG_BOOT_SECONDS].contains(&budget) && local_pool && pool_vm_backend.is_none() && !browser && !trace } else { (30..=360).contains(&budget) },
        "normal boot budget must be30..360; nested-tcg requires budget1200,1800,2400 or4800 and the untraced pipe pool"
    );
    let pool_windows: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| arg.to_str().and_then(|s| s.strip_prefix("pool-seconds=")))
        .collect();
    ensure!(pool_windows.len() <= 1, "duplicate explicit pool window");
    let pool_seconds = pool_windows.first().map_or(
        Ok((budget - 10).min(if dashboard_update {
            pool_vm::update_validation::POOL_SECONDS
        } else {
            230
        })),
        |value| value.parse::<u64>(),
    )?;
    private_pool::validate_execution_window(pool_seconds, budget, nested_tcg)?;
    if let Some(manifest) = shared {
        manifest.observation_seconds(pool_seconds, budget)?;
    }
    ensure!(
        !dashboard_update || budget >= 330,
        "signed update validation needs at least 330 seconds of host observation budget"
    );
    let console_scripts: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| arg.to_str().and_then(|s| s.strip_prefix("console=")))
        .collect();
    ensure!(console_scripts.len() <= 1, "duplicate console script");
    let console_script = console_scripts.first().map(Path::new);
    let batch_console = args[5..].iter().any(|arg| arg == "batch-console");
    ensure!(
        !batch_console || console_script.is_some(),
        "batch-console requires a console script"
    );
    let restart_scripts: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| {
            arg.to_str()
                .and_then(|s| s.strip_prefix("restart-console="))
        })
        .map(Path::new)
        .collect();
    ensure!(
        restart_scripts.len() <= 2
            && (restart_scripts.is_empty() || (console_script.is_some() && !local_pool)),
        "up to two restart scripts require console mode without a local pool"
    );
    let states: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| arg.to_str().and_then(|s| s.strip_prefix("sd-state=")))
        .map(Path::new)
        .collect();
    ensure!(states.len() <= 1, "only one saved SD state allowed");
    let save_sd = args[5..].iter().any(|arg| arg == "save-sd") || !restart_scripts.is_empty();
    let nand_storage = args[5..].iter().any(|arg| arg == "nand-storage");
    let nand_states: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| arg.to_str().and_then(|s| s.strip_prefix("nand-state=")))
        .map(Path::new)
        .collect();
    ensure!(
        nand_states.len() <= 1 && (nand_states.is_empty() || (nand_storage && states.len() == 1)),
        "NAND restore requires one paired SD state and nand-storage"
    );
    ensure!(!nand_storage || save_sd, "nand-storage requires save-sd");
    let sd_native_reboot = args[5..].iter().any(|arg| arg == "sd-native-reboot");
    let sd_interrupt = args[5..]
        .iter()
        .any(|arg| arg == "sd-controlled-interruption");
    ensure!(!sd_interrupt || (save_sd && !sd_native_reboot && !dashboard_update && restart_scripts.is_empty()),
        "sd-controlled-interruption requires save-sd, excludes native restart, update and repeated boot");
    ensure!(
        !nand_storage || sd_native_reboot || dashboard_install || sd_interrupt,
        "NAND persistence requires an observed native restart or controlled interruption"
    );
    let sd_checkpoint = args[5..].iter().any(|arg| arg == "sd-read-only-checkpoint");
    ensure!(
        !sd_native_reboot || (save_sd && !dashboard_install),
        "sd-native-reboot requires save-sd and cannot replace the native installer restart"
    );
    ensure!(
        !sd_checkpoint || (sd_native_reboot && !dashboard_update && (!dashboard_probe || pool_vm_backend.is_some())),
        "sd-read-only-checkpoint requires sd-native-reboot, excludes update operations, and needs the private VM for an HTTP probe"
    );
    ensure!(
        !dashboard_install || save_sd,
        "native update install requires save-sd"
    );
    ensure!(
        console_script.is_some() || (!save_sd && states.is_empty()),
        "SD persistence requires a guest console script"
    );
    ensure!(
        !local_pool || console_script.is_some(),
        "local-pool requires a guest configuration script"
    );
    if let Some(script) = console_script {
        ensure!(
            fs::metadata(script)?.len() <= 65536,
            "console script exceeds limit"
        );
    }
    for script in &restart_scripts {
        ensure!(
            fs::metadata(script)?.len() <= 65536,
            "restart script exceeds limit"
        );
    }
    let reference_v1 = args[5..].iter().any(|arg| arg == "sd2022-v1");
    let fan_fixture = args[5..].iter().any(|arg| arg == "fan-fixture");
    let fan_paths: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| arg.to_str().and_then(|s| s.strip_prefix("fan-plan=")))
        .collect();
    ensure!(
        fan_paths.len() <= 1 && (fan_paths.is_empty() || fan_fixture),
        "one fan input plan requires the explicit motor fixture"
    );
    let fan_plan = fan_paths
        .first()
        .map(|path| fan_plan::Plan::read(Path::new(path)))
        .transpose()?;
    let monitor_sources: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| {
            arg.to_str()
                .and_then(|s| s.strip_prefix("adapt-monitor-fan-source="))
        })
        .collect();
    ensure!(
        monitor_sources.len() <= 1
            && (monitor_sources.is_empty()
                || (fan_fixture && reference_v1 && console_script.is_some())),
        "one monitor script adaptation requires the explicit two-PWM reference fixture and console"
    );
    let pic_contract = args[5..].iter().any(|arg| arg == "pic-contract");
    let asic_contract = args[5..].iter().any(|arg| arg == "asic-contract");
    let temperature_fixture = args[5..].iter().any(|arg| arg == "temperature-fixture");
    ensure!(
        !temperature_fixture || asic_contract,
        "temperature-fixture requires asic-contract"
    );
    let temperature_paths: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| {
            arg.to_str()
                .and_then(|s| s.strip_prefix("temperature-plan="))
        })
        .collect();
    ensure!(
        temperature_paths.len() <= 1 && (temperature_paths.is_empty() || temperature_fixture),
        "one temperature input plan requires the explicit sensor fixture"
    );
    let temperature_plan = temperature_paths
        .first()
        .map(|path| temperature_plan::Plan::read(Path::new(path)))
        .transpose()?;
    let filter_args: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| {
            arg.to_str()
                .and_then(|s| s.strip_prefix("adapt-fixed-filter-bits="))
        })
        .collect();
    ensure!(
        filter_args.len() <= 1 && (filter_args.is_empty() || asic_contract),
        "one fixed-filter adaptation requires asic-contract"
    );
    let fixed_filter_bits = filter_args
        .first()
        .map(|text| text.parse::<u8>())
        .transpose()?;
    ensure!(
        fixed_filter_bits.is_none_or(|bits| (1..32).contains(&bits)),
        "adapted fixed filter must have1..31 zero bits; omit for native32"
    );
    let hash_caps: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| arg.to_str().and_then(|s| s.strip_prefix("hash-cap-hps=")))
        .collect();
    ensure!(
        hash_caps.len() <= 1 && (hash_caps.is_empty() || asic_contract),
        "one hash cap requires asic-contract"
    );
    let hash_cap: Option<f64> = hash_caps.first().map(|text| text.parse()).transpose()?;
    if let Some(rate) = hash_cap {
        // A diagnostic cap above the default needs a separate resource
        // study. This runner accepts the default and every lower supported cap.
        ensure!(rate <= 10000.0, "diagnostic cap cannot exceed10000 H/s");
        md_sim_firmware::bm1387::budget::Budget::new(rate).map_err(anyhow::Error::msg)?;
    }
    ensure!(
        !asic_contract || pic_contract,
        "ASIC contract requires explicit pic-contract"
    );
    let mut asic_identity = serde_json::Value::Null;
    ensure!(
        !pic_contract || reference_v1,
        "PIC contract requires explicit sd2022-v1"
    );
    let pic_app = Path::new("simulator/firmware/pic16/braiins-s9-app.txt");
    let mut pic_identity = serde_json::Value::Null;
    ensure!(
        !fan_fixture || reference_v1,
        "fan fixture requires explicit sd2022-v1"
    );
    let mut fan_identity = serde_json::Value::Null;
    ensure!(
        !reference_v1 || !args[5..].iter().any(|arg| arg == "sd2022-io"),
        "choose one controller revision"
    );
    let sd2022_io = reference_v1 || args[5..].iter().any(|arg| arg == "sd2022-io");
    ensure!(
        !sd2022_io || !args[5..].iter().any(|arg| arg == "legacy-io"),
        "choose one controller layout"
    );
    let pin_fixtures: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| {
            arg.to_str()
                .and_then(|s| s.strip_prefix("presence-pin-fixture="))
        })
        .collect();
    ensure!(pin_fixtures.len() <= 1, "duplicate pin fixture");
    ensure!(
        pin_fixtures.is_empty() || sd2022_io,
        "pin fixture requires the explicit SD2022 development profile"
    );
    let pins = pin_fixtures.first().map_or(Ok(0), |value| {
        u32::from_str_radix(value.strip_prefix("0x").unwrap_or(value), 16)
    })?;
    ensure!(pins <= 0x1ff, "pin fixture exceeds nine GPIO inputs");
    let legacy_io = sd2022_io || args[5..].iter().any(|arg| arg == "legacy-io");
    let nand_onfi = legacy_io || args[5..].iter().any(|arg| arg == "nand-onfi");
    ensure!(
        !nand_onfi || !args[5..].iter().any(|arg| arg == "nand-fixture"),
        "choose one NAND profile"
    );
    let nand_fixture = nand_onfi || args[5..].iter().any(|arg| arg == "nand-fixture");
    ensure!(
        !nand_storage || nand_fixture,
        "NAND storage requires a declared NAND model"
    );
    ensure!(
        !(nand_instructions || nand_registers || nand_pins) || nand_fixture,
        "NAND instructions require explicit fixture"
    );
    let cpus = if single_cpu { "1" } else { "2" };
    let selections: Vec<_> = args[5..]
        .iter()
        .filter_map(|arg| arg.to_str().and_then(|s| s.strip_prefix("backend=")))
        .collect();
    ensure!(selections.len() <= 1, "duplicate backend selection");
    let mut backend_identity = serde_json::Value::Null;
    let mut watchdog_trace = false;
    let backend = if let Some(directory) = selections.first() {
        let directory = Path::new(directory);
        ensure!(
            fs::metadata(directory.join("build-report.json"))?.len() <= 16 * 1024 * 1024,
            "backend report limit"
        );
        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.join("build-report.json"))?)?;
        ensure!(
            report["source"] == "2a40e6705e59fd8807012631bc6b2c0877f10481b12768f78e4f9d7b88376cce"
                && report["patch"]["code"] == 0
                && report["cpu_patch"]["code"] == 0
                && report["build"]["code"] == 0
                && report["strip"]["code"] == 0,
            "pinned backend preparation did not pass"
        );
        let binary = directory.join("qemu-system-arm");
        ensure!(
            !configuration_contract || report["pcap_patch"]["code"] == 0,
            "configuration contract requires the checked PCAP backend"
        );
        ensure!(
            !nand_fixture || report["pl353_patch"]["code"] == 0,
            "NAND fixture requires compiled PL353 backend"
        );
        ensure!(
            !nand_onfi || report["onfi_fixture_smoke"]["code"] == 0,
            "ONFI profile requires its own compiled machine startup check"
        );
        ensure!(
            !legacy_io || report["legacy_io_patch"]["code"] == 0,
            "legacy S9IO profile requires its compiled backend"
        );
        ensure!(
            !sd2022_io
                || (report["sd2022_io_patch"]["code"] == 0 && report["gpio_patch"]["code"] == 0),
            "SD2022 development profile requires its compiled layout and GPIO"
        );
        ensure!(
            !reference_v1 || report["s9io_v1_smoke"]["code"] == 0,
            "reference S9IO revision requires its compiled backend startup check"
        );
        if fan_fixture {
            ensure!(
                report["fan_smoke"]["code"] == 0,
                "fan fixture absent from backend"
            );
            fan_identity = report["fan_reference"].clone();
            ensure!(fan_identity.is_object(), "fan contract missing from build");
            fan_identity["motors_connected"] = json!(true);
            fan_identity["physical_calibration"] = json!("unvalidated");
            ensure!(
                fan_plan.is_none()
                    || (report["fan_input_plan_smoke"]["code"] == 0
                        && fan_identity["input_plan_contract"] == "md-sim-fan-input-plan/1"),
                "fan input plan is absent from the tested backend"
            );
        }
        if pic_contract {
            ensure!(
                report["pic16_smoke"]["code"] == 0,
                "PIC contract absent from backend"
            );
            pic_identity = report["pic16_reference"].clone();
            let application = hash_file(pic_app)?;
            ensure!(
                pic_identity.is_object()
                    && application.sha256
                        == "ec84da07ef98665c6258cc531dc2d74aa8be7018925c041325ca233aec52b46a"
                    && application
                        == serde_json::from_value::<ObjectId>(pic_identity["application"].clone())?,
                "PIC application differs from tested backend input"
            );
        }
        if asic_contract {
            ensure!(
                report["bm1387_smoke"]["code"] == 0,
                "ASIC contract absent from tested backend"
            );
            asic_identity = report["bm1387_reference"].clone();
            ensure!(asic_identity.is_object(), "ASIC build identity absent");
            if temperature_fixture {
                ensure!(
                    report["bm1387_temperature_smoke"]["code"] == 0
                        && asic_identity["temperature_fixture"]["profile"]["sha256"]
                            == object(include_bytes!(
                                "../../../simulator/backends/qemu/s9-tmp421-reference.json"
                            ))
                            .sha256,
                    "temperature fixture differs from tested backend input"
                );
            }
            ensure!(
                temperature_plan.is_none()
                    || (report["bm1387_temperature_plan_smoke"]["code"] == 0
                        && asic_identity["temperature_input_plan_contract"]
                            == "md-sim-temperature-input-plan/1"),
                "temperature input plan is absent from the tested backend"
            );
            ensure!(
                fixed_filter_bits.is_none()
                    || (report["bm1387_adapted_filter_smoke"]["code"] == 0
                        && asic_identity["low_rate_filter_contract"]
                            == "md-sim-bm1387-fixed-filter/1"),
                "fixed-filter adaptation is absent from this backend"
            );
            asic_identity["hashing"]["override"] = json!(fixed_filter_bits.is_some());
            asic_identity["hashing"]["fixed_zero_bits"] = json!(fixed_filter_bits.unwrap_or(32));
            ensure!(
                !asic_packets || asic_identity["compact_trace"] == "md-sim-bm1387-packets/1",
                "compact packet traces are absent from this backend"
            );
            pic_identity["power_enable_output_connected"] = json!(true);
        }
        if local_pool && (pool_vm_backend.is_some() || shared.is_some()) {
            ensure!(
                report["private_stream"]["contract"] == "md-sim-private-inherited-ethernet/1"
                    && report["private_stream"]["patch"]["sha256"]
                        == object(include_bytes!(
                            "../../../simulator/backends/qemu/private-stream.patch"
                        ))
                        .sha256,
                "miner backend has no matching private stream adapter"
            );
        } else if local_pool {
            ensure!(
                report["private_network_smoke"]["code"] == 0
                    && report["slirp"]["version"] == "4.9.5",
                "selected backend has no tested pinned private network"
            );
            ensure!(
                hash_file(Path::new("/usr/lib/libslirp.so.0"))?
                    == serde_json::from_value::<ObjectId>(
                        report["slirp"]["files"]["libslirp.so.0"].clone()
                    )?,
                "runtime network library differs from build input"
            );
        }
        ensure!(
            hash_file(&binary)? == serde_json::from_value::<ObjectId>(report["binary"].clone())?,
            "backend binary differs"
        );
        watchdog_trace = report["swdt_reference"].is_object() && report["swdt_patch"]["code"] == 0;
        backend_identity = json!({"binary":hash_file(&binary)?,"report":hash_file(&directory.join("build-report.json"))?,"spec":hash_file(&directory.join("spec.json"))?,"inputs":hash_file(&directory.join("input-identities.json"))?,"adaptation":"Experimental S9 machine with AXI IIC and SLCR CPU reset; explicit secondary CPU park replaces BootROM-stage setup. Archived boot payload bytes remain unchanged. Any native-script change is declared separately in the run adaptation."});
        binary
    } else {
        PathBuf::from("/usr/bin/qemu-system-arm")
    };
    if !backend_identity.is_null() {
        backend_identity["watchdog_transactions_traced"] = json!(watchdog_trace);
    }
    let selected_backend = !backend_identity.is_null();
    ensure!(
        !nand_fixture || selected_backend,
        "NAND fixture requires pinned backend selection"
    );
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(&out)?;
    if let Some(plan) = &temperature_plan {
        plan.retain(&out)?;
    }
    if let Some(plan) = &fan_plan {
        plan.retain(&out)?;
    }
    ensure!(
        fs::metadata(bundle.join("boot/bundle.json"))?.len() <= 65536,
        "bundle manifest exceeds limit"
    );
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("boot/bundle.json"))?)?;
    let profile = bundle_image(&manifest)?;
    ensure!(
        monitor_sources.is_empty() || profile.name == "sd2022",
        "monitor script adaptation requires the exact2022 image"
    );
    let monitor_identity = monitor_sources
        .first()
        .map(|source| monitor_adaptation::prepare(Path::new(source), &out))
        .transpose()?;
    ensure!(
        !dashboard_probe || profile.name == "sd2022",
        "dashboard probe requires the pinned2022 firmware"
    );
    let (spl_hash, uboot_hash) = profile
        .boot_members
        .context("boot members are not pinned")?;
    ensure!(
        profile.name == "sd2022" || !(nand_instructions || nand_registers),
        "kernel instruction ranges apply only to the pinned sd2022 image"
    );
    let expected: ObjectId = serde_json::from_value(manifest["stages"][stage]["object"].clone())?;
    let (parent_name, parent_hash, payload_offset) = if stage == "spl" {
        ("boot.bin", spl_hash, 0x8c0)
    } else {
        ("u-boot.img", uboot_hash, 64)
    };
    let parent_path = bundle.join("boot").join(parent_name);
    ensure!(
        fs::metadata(&parent_path)?.len() <= 1024 * 1024,
        "boot parent too large"
    );
    let parent = fs::read(&parent_path)?;
    check_stage_manifest(
        &manifest,
        stage,
        parent_name,
        &object(&parent),
        payload_offset,
    )?;
    ensure!(
        object(&parent).sha256 == parent_hash
            && object(parent.get(payload_offset..).context("short boot stage")?) == expected,
        "stage is not the exact original member suffix"
    );
    let payload = bundle.join("boot").join(format!("{stage}-payload.bin"));
    ensure!(
        hash_file(&payload)? == expected,
        "derived stage hash differs"
    );
    let image = Path::new(profile.path);
    ensure!(hash_file(image)? == profile.object(), "SD input differs");
    let store = Store::open(&out.join("input-store"), 512 * 1024 * 1024)?;
    let programs = vec![
        ("/qemu".into(), backend),
        ("/qemu-img".into(), PathBuf::from("/usr/bin/qemu-img")),
        ("/boot-runner".into(), std::env::current_exe()?),
    ];
    let mut inputs = md_sim_firmware::lab::elf_closure(&store, programs)?;
    let configuration_identity = if configuration_contract {
        let bitstream = bundle.join("boot/system.bit");
        let expected: ObjectId = serde_json::from_value(manifest["fpga"]["decompressed"].clone())?;
        ensure!(
            hash_file(&bitstream)? == expected,
            "configuration input differs from bundle"
        );
        let part = Path::new("fixtures/fpga/xc7z010clg400-1/part.json");
        ensure!(
            hash_file(part)?.sha256
                == "1754ec1c7a8e0447a116984505cc422819d3be06389527de96bf192c5175d095",
            "configuration part input differs"
        );
        add(&store, &mut inputs, &bitstream, "/pcap-reference.bit")?;
        add(&store, &mut inputs, part, "/pcap-part.json")?;
        Some(json!({"container":expected,"part":hash_file(part)?,
            "contract":hash_file(Path::new("simulator/backends/qemu/PCAP.md"))?,
            "status_adaptation":"This reference contract restricts configuration-done status to a complete checked payload. Real initialization can also set that status before a transfer. Exact initialization, event/status semantics and timing remain incomplete; the independent transfer audit is required.",
            "scope":"Actual guest DMA must deliver the selected complete CRC/ECC-validated image. FPGA logic, reset sequencer and physical PCAP timing remain open."}))
    } else {
        None
    };
    if monitor_identity.is_some() {
        add(
            &store,
            &mut inputs,
            &out.join("monitor-adaptation-script.sh"),
            "/monitor-adaptation-script",
        )?;
    }
    add(&store, &mut inputs, &payload, "/stage.bin")?;
    add(&store, &mut inputs, image, "/sd.img")?;
    let mut nand_parent_execution = None;
    let nand_backing = if nand_storage {
        let path = out.join("nand-erased.raw");
        let object = boot_storage::prepare_erased_nand(&path)?;
        add(&store, &mut inputs, &path, "/nand-erased.raw")?;
        if let Some(state_path) = nand_states.first() {
            let nand = boot_storage::input_storage(boot_storage::NAND, state_path, &object)?;
            let sd = boot_storage::input(states[0], &profile.object())?;
            nand_parent_execution = Some(boot_storage::verify_pair(
                states[0], &sd, state_path, &nand,
            )?);
            add(&store, &mut inputs, state_path, "/nand-state.json")?;
            add(
                &store,
                &mut inputs,
                &state_path
                    .parent()
                    .context("NAND state parent absent")?
                    .join(boot_storage::file(&nand)),
                "/nand-state.qcow2.gz",
            )?;
        }
        Some(object)
    } else {
        None
    };
    if let Some(path) = states.first() {
        let state = boot_storage::input(path, &profile.object())?;
        add(&store, &mut inputs, path, "/sd-state.json")?;
        add(
            &store,
            &mut inputs,
            &path
                .parent()
                .context("SD state parent absent")?
                .join(boot_storage::file(&state)),
            "/sd-state.qcow2.gz",
        )?;
    }
    if local_pool {
        let path = out.join("pool-input.json");
        save_json(
            &path,
            &if pool_keep_open {
                private_pool::prepare_window(pool_seconds, pool_difficulty, regtest_anchor.clone())?
            } else {
                private_pool::prepare(pool_seconds, pool_difficulty, regtest_anchor.clone())?
            },
        )?;
        add(&store, &mut inputs, &path, "/private-pool.json")?;
        if let Some((_, bytes)) = &shared_input {
            let path = out.join("shared-pool.json");
            fs::write(&path, bytes)?;
            add(&store, &mut inputs, &path, shared_pool::GUEST_MANIFEST)?;
        }
        if let Some((_, bytes)) = &carrier_dashboard {
            let path = out.join(carrier_dashboard::FILE);
            fs::write(&path, bytes)?;
            add(&store, &mut inputs, &path, carrier_dashboard::GUEST)?;
        }
        if let Some(backend) = pool_vm_backend {
            let prepared = out.join("pool-vm-image");
            pool_vm::prepare(
                &prepared,
                &path,
                dashboard_suite,
                dashboard_state,
                browser_payload,
                wall_epoch,
            )?;
            pool_vm::capture(
                &store,
                &mut inputs,
                &prepared,
                backend,
                browser_payload.is_some(),
            )?;
        }
    }
    if pic_contract {
        add(&store, &mut inputs, pic_app, "/pic16-app.txt")?;
    }
    if let Some(script) = console_script {
        add(&store, &mut inputs, script, "/console-script")?;
    }
    for (index, script) in restart_scripts.iter().enumerate() {
        add(
            &store,
            &mut inputs,
            script,
            &format!("/restart-console-{index}"),
        )?;
    }
    let address = if stage == "spl" { "0" } else { "0x4000000" };
    let pipe_nic = private_pool::nic(dashboard_forward);
    let mut argv = vec![
        "/boot-runner",
        if console_script.is_some() {
            "__boot-console"
        } else {
            "__boot"
        },
        "-machine",
        if asic_contract {
            "md-sim-s9-bm1387-development,boot-mode=sd"
        } else if pic_contract {
            "md-sim-s9-pic-development,boot-mode=sd"
        } else if fan_fixture {
            "md-sim-s9-fan-development,boot-mode=sd"
        } else if sd2022_io {
            "md-sim-s9-sd2022-io-development,boot-mode=sd"
        } else if legacy_io {
            "md-sim-s9-legacy-io-development,boot-mode=sd"
        } else if nand_onfi {
            "md-sim-s9-m69a-development,boot-mode=sd"
        } else if nand_fixture {
            "md-sim-s9-nand-fixture,boot-mode=sd"
        } else if selected_backend {
            "md-sim-s9-zynq-a9,boot-mode=sd"
        } else {
            "xilinx-zynq-a9,boot-mode=sd"
        },
        "-accel",
        "tcg,thread=single",
        "-m",
        "1024M",
        "-smp",
        cpus,
        "-nodefaults",
        "-nic",
        if pool_vm_backend.is_some() || shared.is_some() {
            pool_vm::MINER_NIC
        } else if local_pool {
            &pipe_nic
        } else {
            "none"
        },
        "-display",
        "none",
        "-monitor",
        "none",
        "-serial",
        "null",
        "-serial",
        "stdio",
        "-no-reboot",
        "-drive",
        "file=/work/sd.qcow2,if=sd,index=0,format=qcow2,cache=writethrough",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    if nand_storage {
        argv.extend([
            "-drive".into(),
            "file=/work/nand.qcow2,if=none,id=md-nand,format=qcow2,cache=writethrough".into(),
            "-global".into(),
            "md-sim-pl353.drive-id=md-nand".into(),
        ]);
    }
    if let Some(rate) = hash_cap {
        argv[3] = format!("md-sim-s9-bm1387-development,boot-mode=sd,hash-cap-hps={rate}");
    }
    if let Some(shift) = clock_shift {
        argv.extend([
            "-icount".into(),
            format!("shift={shift},align=off,sleep=off"),
        ]);
    }
    if watchdog_trace {
        argv.extend(["-trace".into(), "enable=md_sim_swdt_*".into()]);
    }
    if configuration_contract {
        argv.extend([
            "-global".into(),
            "driver=xlnx.ps7-dev-cfg,property=md-bitstream,value=/pcap-reference.bit".into(),
            "-global".into(),
            "driver=xlnx.ps7-dev-cfg,property=md-part,value=/pcap-part.json".into(),
        ]);
    }
    if sd2022_io {
        argv.extend([
            "-global".into(),
            format!("md-sim-axi-gpio.input-levels={pins}"),
            "-trace".into(),
            "enable=md_sim_gpio_*".into(),
        ]);
    }
    if reference_v1 {
        argv.extend([
            "-global".into(),
            "md-sim-s9io.reference-v1-bm1387=on".into(),
            "-trace".into(),
            "enable=md_sim_axi_iic_*".into(),
        ]);
    }
    if fan_fixture {
        argv.extend([
            "-global".into(),
            "md-sim-fan.reference-motors=on".into(),
            "-trace".into(),
            "enable=md_sim_fan_*".into(),
        ]);
        if let Some(plan) = &fan_plan {
            argv.extend([
                "-global".into(),
                format!("md-sim-fan.input-plan={}", plan.json),
            ]);
        }
    }
    if pic_contract {
        argv.extend([
            "-global".into(),
            "md-sim-s9-pic16.application-text=/pic16-app.txt".into(),
            "-trace".into(),
            "enable=md_sim_pic_*".into(),
        ]);
    }
    if asic_packets {
        argv.extend([
            "-trace".into(),
            "enable=md_sim_bm1387_packet".into(),
            "-trace".into(),
            "enable=md_sim_bm1387_state".into(),
        ]);
    } else if asic_contract {
        argv.extend(["-trace".into(), "enable=md_sim_bm1387_event".into()]);
    }
    if let Some(bits) = fixed_filter_bits {
        argv.extend([
            "-global".into(),
            format!("md-sim-s9io.fixed-filter-zero-bits={bits}"),
        ]);
    }
    if temperature_fixture {
        argv.extend(["-global".into(), "md-sim-s9io.tmp421-reference=on".into()]);
        if let Some(plan) = &temperature_plan {
            argv.extend([
                "-global".into(),
                format!("md-sim-s9io.temperature-input-plan={}", plan.json),
            ]);
        }
    }
    if selected_backend && !asic_packets {
        argv.extend([
            "-trace".into(),
            if nand_instructions || nand_registers || nand_pins {
                "enable=md_sim_pl353_pin"
            } else if legacy_io {
                if asic_events {
                    "enable=md_sim_s9io_event"
                } else {
                    "enable=md_sim_s9io_*"
                }
            } else {
                "enable=md_sim_*"
            }
            .into(),
        ]);
        if nand_pins {
            argv.extend(["-trace".into(), "enable=md_sim_pl353_onfi".into()]);
        }
    } else if !selected_backend {
        argv.extend([
            "-sandbox".into(),
            "on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny".into(),
        ]);
    }
    argv.extend([
        "-device".into(),
        format!("loader,file=/stage.bin,addr={address},cpu-num=0,force-raw=on"),
    ]);
    if trace {
        argv.extend(["-d".into(), "in_asm".into()]);
    }
    if nand_instructions {
        // Exact original kernel's command-table reference is at c02f7a34.
        // Static evidence: kernel-binary-inspection-005; no guest bytes change.
        argv.extend([
            "-d".into(),
            "in_asm".into(),
            "-dfilter".into(),
            "0xc02f7000..0xc02f7fff".into(),
        ]);
    }
    if nand_registers {
        argv.extend([
            "-d".into(),
            "in_asm,cpu".into(),
            "-dfilter".into(),
            "0xc02f7884..0xc02f7930".into(),
        ]);
    }
    let mut spec = ProcessSpec {
        private_channel: shared.map(|manifest| manifest.carrier.clone()),
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: dashboard_forward.then_some(md_sim_runtime::dashboard::Spec {
            source: md_sim_runtime::dashboard::Source::AdaptedNative,
            guest_port: 18080,
            listen_port: 0,
        }),
        loopback: private_loopback,
        schema_version: "2.0".into(),
        stdin_object: None,
        argv,
        inputs,
        environment: BTreeMap::from([
            (
                "MD_SIM_S9_BATCH_CONSOLE".into(),
                if batch_console { "1" } else { "0" }.into(),
            ),
            (
                "MD_SIM_S9_SAVE_SD".into(),
                if save_sd { "1" } else { "0" }.into(),
            ),
            (
                "MD_SIM_S9_SD_NATIVE_REBOOT".into(),
                if sd_native_reboot { "1" } else { "0" }.into(),
            ),
            (
                "MD_SIM_S9_SD_INTERRUPT".into(),
                if sd_interrupt { "1" } else { "0" }.into(),
            ),
            (
                "MD_SIM_S9_SD_CHECKPOINT".into(),
                if sd_checkpoint { "1" } else { "0" }.into(),
            ),
            (
                "MD_SIM_S9_DASHBOARD_PROBE".into(),
                if dashboard_probe { "1" } else { "0" }.into(),
            ),
            (
                "MD_SIM_S9_RESTARTS".into(),
                restart_scripts.len().to_string(),
            ),
            (
                "MD_SIM_S9_RESTORE_SD".into(),
                if states.is_empty() { "0" } else { "1" }.into(),
            ),
            (
                "MD_SIM_S9_POOL_VM".into(),
                if pool_vm_backend.is_some() { "1" } else { "0" }.into(),
            ),
            ("TMPDIR".into(), "/tmp".into()),
            (
                "MD_SIM_S9_BROWSER".into(),
                if browser { "1" } else { "0" }.into(),
            ),
            ("MD_SIM_S9_MINER_WALL_SECONDS".into(), budget.to_string()),
            (
                "MD_SIM_S9_LOCAL_POOL".into(),
                if local_pool { "1" } else { "0" }.into(),
            ),
        ]),
        limits: Limits {
            wall_seconds: if trace {
                15
            } else {
                budget
                    + if browser {
                        browser_budget::STARTUP_SECONDS
                    } else {
                        0
                    }
            },
            // Probe002 needed119.919s wall for97.280s emulator CPU, with
            // concurrent guest SSH key generation. Permit more wall time
            // without changing the CPU ceiling, guest clocks or deadlines.
            cpu_seconds: if trace {
                10
            } else if nested_tcg {
                confinement::mining_profile_cpu(Some(nested_profile))?
                    .context("nested CPU allowance absent")?
            } else if browser {
                browser_budget::SUPPORT_CPU_SECONDS
            } else if local_pool {
                // Pool015 retained110.212CPU seconds before SIGKILL.
                // This affects only host execution budget, not guest clocks,
                // the ASIC hash cap, watchdogs or mining assertions.
                // Run031 hit the230s CPU ceiling after real shares, before
                // its full console/snapshot path finished. Bound the larger
                // allowance separately from the selected pool observation window.
                (budget - 10).min(330)
            } else {
                (budget - 10).min(110)
            },
            memory_bytes: if browser {
                browser_budget::SUPPORT_ADDRESS_BYTES
            } else {
                browser_budget::MINER_ADDRESS_BYTES
            },
            processes: 64,
            file_bytes: 64 * 1024 * 1024,
            // A post-update snapshot can contain19MiB of compressed firmware
            // and needs38MiB for its hexadecimal export, in addition to the
            // live ASIC and browser traces. Run078 exceeded the old64MiB cap.
            trace_bytes: if dashboard_update || save_sd {
                pool_vm::update_validation::TRACE_BYTES / (1024 * 1024)
            } else if asic_events || asic_packets || save_sd {
                64
            } else if pic_contract {
                32
            } else if nand_pins {
                16
            } else {
                4
            } * 1024
                * 1024,
            tmp_bytes: 128 * 1024 * 1024,
        },
    };
    if let Some(epoch) = wall_epoch {
        spec.environment
            .insert(wall_clock::ENVIRONMENT.into(), epoch.to_string());
    }
    if shared.is_some() {
        spec.environment
            .insert(shared_pool::ENVIRONMENT.into(), "1".into());
    }
    if nand_storage {
        spec.environment
            .insert("MD_SIM_S9_NAND_STORAGE".into(), "1".into());
    }
    if nested_tcg {
        spec.environment.insert(
            confinement::MINING_PROFILE_ENVIRONMENT.into(),
            nested_profile.into(),
        );
    }
    if !pool_startup_values.is_empty() {
        spec.environment.insert(
            pool_vm::STARTUP_ENVIRONMENT.into(),
            pool_startup_seconds.to_string(),
        );
    }
    if dashboard_update {
        spec.environment.insert(
            pool_vm::update_validation::ENVIRONMENT.into(),
            if dashboard_corruption.is_some() {
                pool_vm::update_validation::Action::RejectTampered
            } else if dashboard_install {
                pool_vm::update_validation::Action::InstallKeep
            } else {
                pool_vm::update_validation::Action::ValidateCancel
            }
            .environment()
            .into(),
        );
    }
    save_json(&out.join("process-spec.json"), &spec)?;
    let nand_state_identity = nand_states
        .first()
        .map(|path| hash_file(path))
        .transpose()?;
    if let Some(script) = console_script {
        save_json(
            &out.join("console.json"),
            &json!({
                "script":hash_file(script)?,
                "restart_scripts":restart_scripts.iter().map(|path|hash_file(path)).collect::<Result<Vec<_>>>()?,
                "saved_sd_input":states.first().map(|path|hash_file(path)).transpose()?,
                "nand_storage":nand_storage.then(||json!({"backing":nand_backing,
                    "saved_input":nand_state_identity,
                    "parent_execution":nand_parent_execution,
                    "geometry":"131072 pages, 2048 data bytes plus 64 spare bytes; exact physical part remains unresolved",
                    "initialization":"All data and spare bytes FF in a generated read-only backing. The unchanged guest creates its own persistent contents.",
                    "capture":"Paired SD and NAND overlays after the same emulator termination. Saved input pairs must reference the same observed native restart or controlled interruption and exact parent execution.",
                    "scope":"Persistent reference NAND, not a factory dump, complete machine snapshot or physical power-loss test."})),
                "dashboard_state_plan":dashboard_state.map(|_|hash_file(&out.join("pool-vm-image.dashboard-state.json"))).transpose()?,
                "dashboard_luci_contract":dashboard_luci.then(||hash_file(&out.join("pool-vm-image.luci-contract"))).transpose()?,
                "dashboard_update_validation_contract":dashboard_update.then(||hash_file(&out.join("pool-vm-image.update-validation.json"))).transpose()?,
                "update_packet_capture_limit":dashboard_update.then_some(pool_vm::update_validation::PACKET_BYTES),
                "browser_payload_manifest":browser_payload.map(hash_file).transpose()?,
                "wall_clock_adaptation":wall_epoch.map(wall_clock::identity),
                "monitor_script_adaptation":monitor_identity,
                "browser_support":browser.then(||json!({"startup_wall_seconds":browser_budget::STARTUP_SECONDS,"support_cpu_seconds":browser_budget::SUPPORT_CPU_SECONDS,"support_address_bytes":browser_budget::SUPPORT_ADDRESS_BYTES,"miner_address_bytes":browser_budget::MINER_ADDRESS_BYTES,"browser_profiles":2,"browser_completion_seconds":browser_budget::COMPLETION_SECONDS,"guest_ram_mib":2048,"guest_cpus":4,"browser_cpus_per_profile":2,"miner_wall_seconds":budget,"miner_cpu_seconds":(budget-10).min(330),"packet_capture_limit":browser_budget::PACKET_BYTES,"browser_uid":1001,"pool_uid":1000,"native_browser_scope":"login, archived asset, advancing HTTP telemetry and WebSocket delivery; remaining dashboard gates stay explicit"})),
                "sd_snapshot_export":save_sd,
                "sd_native_reboot":sd_native_reboot,
                "sd_controlled_interruption":sd_interrupt.then(||json!({
                    "signal":"SIGKILL","guest_sync_requested":false,"guest_reboot_requested":false,
                    "trigger":"After successful console commands and the declared mining collection window.",
                    "required_evidence":"Owned emulator was live, signal9 exit, complete serial EOF, paired storage export. Recovery requires a separate boot and readback.",
                    "scope":"Software interruption of the emulator. No interrupted-write atomicity, update recovery or physical power-loss claim."})),
                "sd_read_only_checkpoint":sd_checkpoint.then(boot_storage::checkpoint_identity),
                "sd_cache_policy":"writethrough: each acknowledged SD write flushes the image backend; the SD frontend has no separate guest cache-flush command",
                "sd_persistence_scope":if sd_interrupt {"Saved overlays follow an observed intentional SIGKILL without a final guest sync or reboot. The model uses writethrough SD storage. A separate recovery boot and readback are required; no physical power-loss claim."} else if nand_storage {"Paired SD and NAND overlays follow the same observed native restart. Volatile state resets. Independent filesystem checks and NAND readback are still required. No host-crash durability claim."} else {"Saved SD runs use guest sync then owned termination, unless an explicit native reboot or native update installation observes kernel restart, serial EOF and actual exit0. Restart scripts use a new emulator with the same SD overlay. NAND and volatile state reset. Filesystem consistency needs independent inspection; no host-crash durability claim."},
                "entry":"Original firmware console activation; no init replacement",
                "batch_console_adaptation":batch_console.then(||json!({
                    "operation":"exec /bin/sh +i after the original console starts. The next actual shell command must report that its interactive flag is absent.",
                    "reason":"Test whether disabling interactive line editing reduces nested-emulation console delay. Noninteractive input retains the same archived shell and exact script bytes. No calibrated performance claim.",
                    "firmware_bytes_changed":false,
                    "boot_and_device_clocks_changed":false,
                    "scope":"Console environment adaptation. It is not an unassisted console or boot claim."
                })),
                "completion":"Guest must emit the script status line; timeout is not success",
                "live_tcp_pool_collection_wait":pool_vm_backend.is_some(),
                "stop":if sd_interrupt {"Success requires the selected software interruption, complete serial EOF and actual SIGKILL status. No guest shutdown is requested."} else if sd_native_reboot || dashboard_install {"Success requires native restart, serial EOF and emulator exit0. Failure cleanup kills and reaps the owned emulator."} else {"The runner kills and reaps its QEMU child after the console and selected service checks. A live TCP-pool miner remains running until its declared collection completes. The outer execution limits remain fixed."}
            }),
        )?;
    }
    let mut nand_identity = if nand_onfi {
        json!({"kind":"m69a-onfi-development-profile","manufacturer_id":"2c","device_id":"da","read_id_hex":"2cda909506","capacity_bytes":268435456_u64,"data_bytes_per_page":2048,"spare_bytes_per_page":64,"pages_per_block":64,"initial_state":"all erased; private memory only","parameter_vendor_suffix":"derived zero-fill after documented prefix; not physical dump","features":"normal array, internal ECC disabled, timing0 only; missing modes fail visibly","contract":hash_file(Path::new("simulator/backends/qemu/ONFI.md"))?,"source_observations":hash_file(Path::new("work/simulator/qemu-s9/micron-m69a-observation-001.json"))?,"exact_physical_s9_part":"unresolved; partial profile does not establish compatibility"})
    } else if nand_fixture {
        json!({"kind":"generic-controller-development-fixture","manufacturer_id":"ec","device_id":"da","capacity_bytes":268435456_u64,"data_bytes_per_page":2048,"spare_bytes_per_page":64,"pages_per_block":64,"initial_state":"all erased; private memory only","exact_physical_s9_part":"unresolved; fixture does not establish compatibility"})
    } else {
        serde_json::Value::Null
    };
    if nand_fixture {
        if nand_storage {
            nand_identity["initial_state"] = json!(if nand_states.is_empty() {
                "Generated erased data and spare bytes; private writable qcow2 overlay"
            } else {
                "Exact saved NAND overlay paired with the selected SD native restart"
            });
            nand_identity["storage"] = json!({"backing":nand_backing,
                "saved_input":nand_state_identity,"parent_execution":nand_parent_execution,"format":"qcow2 over raw complete data+spare pages",
                "cache":"writethrough","snapshot_export":true,
                "scope":"Reference geometry and storage. No physical factory dump, migration or power-loss claim."});
        }
        nand_identity["trace_selection"] = json!(if nand_pins {
            "All NAND command/address pin bytes and ONFI data/feature bytes; ordinary array data and polling register reads are omitted. This diagnostic does not claim a complete register-level bus trace."
        } else {
            "Backend md_sim trace selection is retained exactly in process-spec argv."
        });
    }
    let shared_identity = shared
        .map(|_| hash_file(&out.join("shared-pool.json")))
        .transpose()?;
    let mut legacy_identity = if legacy_io {
        json!({
            "kind":if reference_v1 { "s9io-v1-bm1387-reference-contract-development" } else if sd2022_io { "sd2022-layout-legacy-core-development" } else { "legacy-s9io-development" },
            "version":if reference_v1 { "00901002" } else { "00900200" },
            "build_id":"74657374",
            "clock_hz":if reference_v1 { 200_000_000 } else { 50_000_000 },
            "common_fifo_implementation":if reference_v1 { "Legacy HDL reuse is an explicit unvalidated inference. See contract for limits." } else { "Archived legacy HDL" },
            "atomic_mmio":true,
            "guest_fabric_clock_reset_coupling":false,
                        "asic_chain":asic_contract,
            "temperature_fixture":temperature_fixture,
            "fan_fixture":fan_identity,
            "monitor_script_adaptation":monitor_identity,
            "pic_contract":pic_identity,
            "asic_contract":asic_identity,
            "hash_cap_hps_total":if asic_contract {Some(hash_cap.unwrap_or(10000.0))} else {None},
            "filter_override":fixed_filter_bits.is_some(),
            "fixed_filter_zero_bits":fixed_filter_bits.unwrap_or(32),
            "filter_adaptation":if fixed_filter_bits.is_some() {Some(json!({
                "kind":"md-sim-bm1387-fixed-filter/1",
                "reason":"Native fixed32-bit condition needs at least429496.7296 seconds per response at10000H/s.",
                "change":"Only the fixed leading-zero condition changes. Guest ticket mask bits, positions, writes and readback remain unchanged.",
                "readback":"There is no guest register for the fixed condition. The run manifest and QEMU property record it. Ticket register0x18 reads the actual applied ticket mask.",
                "firmware_bytes_changed":false,
                "scope":"Adapted device environment; not original ASIC behavior. Hashes, nonce ownership and serial responses remain computed."
            }))} else {None},
            "local_pool":local_pool,
            "private_loopback":private_loopback,
            "dashboard_forward":spec.dashboard,
            "pipe_pool_network_scope":"libslirp command forwarding needs TCP sockets on the sandbox loopback. An explicit loopback policy enables only that fresh network namespace; no host interface, route or external pool is attached. Host syscall restrictions still apply.",
            "pool_difficulty_exponents":if local_pool {Some(pool_difficulty)} else {None},
            "regtest_anchor":regtest_anchor,
            "pool_network":if pool_vm_backend.is_some() || shared.is_some() {Some(pool_vm::MINER_NIC)} else if local_pool {Some(pipe_nic.as_str())} else {None},
            "pool_vm":pool_vm_backend.map(|_|"Separate captured Linux VM at10.0.2.100/24; one private Unix Ethernet socket pair; no host listener or external route."),
            "pool_startup_deadline_seconds":pool_vm_backend.map(|_|pool_startup_seconds),
            "pool_startup_scope":"Support readiness budget only. The complete process budget, firmware clocks, bus timing and mining assertions remain separately fixed.",
            "execution_resource_profile":if nested_tcg {nested_profile} else {"standard"},
            "pool_boot_wait_seconds":if nested_tcg {budget-pool_seconds-70} else {0},
            "pool_work_seconds":if local_pool {Some(pool_seconds)} else {None},
            "private_pool_completion_policy":if private_loopback {Some(if pool_keep_open {"keep-open"} else {"close-after-work"})} else {None},
            "execution_resource_scope":"Nested TCG permits more CPU instruction translation and boot observation. Firmware instructions, clocks, bus timing, hash limits and acceptance checks stay unchanged. An explicit pool-seconds value changes only the independent pool observation window; it is part of the resolved execution identity.",
            "trace_selection":if asic_packets {
                "Actual ASIC receive packets at apply, rejection, fault or reset boundaries; serial reply bytes, command state, faults, pins and every computed hash. S9IO MMIO and pin edges are omitted. A partial receive packet at a stopped trace has no completion boundary. Other selected device traces remain enabled. This is not a complete bus trace."
            } else if asic_events {
                "ASIC and S9IO state/serial events; S9IO MMIO reads and writes are omitted. Other selected device traces remain enabled. This is a diagnostic event trace, not a complete bus trace."
            } else {
                "Full selected device trace events; exact selection is retained in process-spec argv."
            },
            "selected_2022_bitstream_correspondence":"unresolved",
            "address_irq_layout":if sd2022_io { "Original2022 image declarations" } else { "Archived legacy system.tcl" },
            "gpio_fixture":if sd2022_io { Some(json!({
                "input_levels":pins,"width":9,
                "meaning":if asic_contract { "Explicit settled presence-pin stimulus. Reference ASIC peers use PIC power and GPIO reset; presence is not a physical detection measurement." } else { "Explicit settled pin stimulus. No powered hashboard or ASIC chain is attached." },
                "complete_hardware_claim":false
            })) } else { None },
            "contract":hash_file(Path::new(if reference_v1 { "simulator/backends/qemu/S9IO-V1.md" } else { "simulator/backends/qemu/S9IO.md" }))?
        })
    } else {
        serde_json::Value::Null
    };
    if let Some(manifest) = shared {
        legacy_identity["shared_pool"] = json!({
            "manifest":shared_identity,
            "support":manifest,
            "observation_seconds":pool_seconds + manifest.observation_margin_seconds,
            "runner_pool_processes":0,
            "scope":"Selected external server namespace. Runtime supplies one private carrier. Pool readiness, helper execution, session collection and mining verification require separately bound evidence. The miner runner makes no pool success assertion."
        });
    }
    if let Some((policy, _)) = &carrier_dashboard {
        legacy_identity["carrier_dashboard"] = json!({
            "policy":hash_file(&out.join(carrier_dashboard::FILE))?,
            "selected":policy,
            "scope":"Captured external access policy. Firmware services and the native observation window are unchanged. Provider liveness, selected-run access and browser checks require separate actual evidence."
        });
    }
    if let Some(plan) = &temperature_plan {
        legacy_identity["temperature_input_plan"] = plan.manifest.clone();
    }
    if let Some(plan) = &fan_plan {
        legacy_identity["fan_input_plan"] = plan.manifest.clone();
    }

    let clock_identity = if let Some(shift) = clock_shift {
        json!({
            "kind":"qemu-fixed-instruction-count-diagnostic",
            "nanoseconds_per_instruction":1u64 << shift,
            "align":false,"sleep":false,
            "effect":"Guest instruction count drives virtual time; idle virtual time can warp to the next device deadline. Device timers retain their virtual nanosecond periods.",
            "physical_cpu_timing_calibrated":false,
            "whole_board_determinism_claim":false,
            "limits":"Instruction time is a diagnostic policy, not measured Cortex-A9 timing. External inputs and random firmware behavior are not controlled for deterministic replay."
        })
    } else {
        json!({"kind":"qemu-default-virtual-realtime","physical_cpu_timing_calibrated":false,"whole_board_determinism_claim":false})
    };
    save_json(
        &out.join("adaptation.json"),
        &json!({"requested_stage":stage,"bundle_manifest":hash_file(&bundle.join("boot/bundle.json"))?,"original_sd":hash_file(image)?,"payload":expected,"parent":object(&parent),"original_payload_bytes":true,"direct_stage_loader":true,"boot_rom_executed":false,"preceding_spl_skipped":stage=="uboot","sd_storage":{"backing":"Original image read-only; private qcow2 overlay,128MiB virtual capacity,zero extension beyond original bytes","cache":"writethrough","snapshot_export":save_sd,"read_only_checkpoint":sd_checkpoint.then(boot_storage::checkpoint_identity),"termination":if dashboard_install {"native-update-restart"} else if sd_native_reboot {"native-reboot"} else if save_sd {"guest-sync-owned-termination"} else {"owned-termination"},"restart_count":restart_scripts.len(),"saved_input":states.first().map(|path|hash_file(path)).transpose()?,"scope":if nand_storage {"Paired SD and NAND overlays persist. Volatile devices reset. No host-crash durability claim."} else {"Only SD writes persist when the snapshot or restart path is selected. NAND and volatile devices reset. No host-crash durability claim."}},"execution_claim":"Requires retained actual guest output; no stage success inferred from launch","sd_strap":5,"cpu_count":cpus,"clock":clock_identity,"wall_clock_adaptation":wall_epoch.map(wall_clock::identity),"single_cpu_diagnostic":single_cpu,"generic_backend":!selected_backend,"configuration_transfer":configuration_identity,"backend_build":backend_identity,"backend_seccomp":!selected_backend,"nand_fixture":nand_identity,"legacy_io":legacy_identity,"isolation":"Required outer namespace and resource isolation applies to both backends.","original_board_gate":"incomplete"}),
    )?;
    if args[5..].iter().any(|arg| arg == "prepare-only") {
        println!(
            "{}",
            json!({"prepared_process_specification":hash_file(&out.join("process-spec.json"))?,"executed":false})
        );
        return Ok(());
    }
    // Retain bounded live output even if the outer diagnostic is interrupted.
    // These host-owned descriptors are not mounted into the guest.
    let trace = TraceFiles {
        stdout: fs::File::create_new(out.join("guest.log"))?,
        stderr: fs::File::create_new(out.join("backend.log"))?,
    };
    let result = execute_recorded(&store, &spec, &out, Some(trace))?;
    execution_report::retain(&out, &spec, &result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_dashboard_cannot_be_requested_without_the_selected_carrier() {
        let args = [
            "s9_boot_bundle",
            "boot",
            "/absent",
            "/absent",
            "uboot",
            "shared-dashboard=/absent",
        ]
        .map(std::ffi::OsString::from);
        assert!(boot(&args)
            .unwrap_err()
            .to_string()
            .contains("one shared dashboard policy requires"));
    }

    #[test]
    fn shared_pool_requires_explicit_preparation_and_excludes_embedded_pool() {
        for options in [
            vec!["shared-pool=/absent"],
            vec!["local-pool", "shared-pool=/absent"],
            vec!["prepare-only", "shared-pool=/absent"],
            vec![
                "local-pool",
                "prepare-only",
                "pool-vm=/absent",
                "shared-pool=/absent",
            ],
            vec![
                "local-pool",
                "prepare-only",
                "shared-pool=/absent",
                "shared-pool=/absent",
            ],
        ] {
            let args: Vec<_> = ["s9_boot_bundle", "boot", "/absent", "/absent", "uboot"]
                .into_iter()
                .chain(options)
                .map(std::ffi::OsString::from)
                .collect();
            let error = boot(&args).unwrap_err().to_string();
            assert!(
                error.contains("one shared-pool manifest requires"),
                "{error}"
            );
        }
    }

    #[test]
    fn browser_startup_does_not_extend_miner_wall_budget() {
        use std::os::unix::process::ExitStatusExt;
        use std::time::{Duration, Instant};
        const CHILD: &str = "MD_SIM_MINER_BOUND_TEST_CHILD";
        if let Ok(mode) = std::env::var(CHILD) {
            let guard = browser_budget::MiningDeadline::start(30, 20).unwrap();
            for (resource, expected) in [
                (libc::RLIMIT_CPU, 20),
                (libc::RLIMIT_AS, 3 * 1024 * 1024 * 1024),
            ] {
                let mut actual = libc::rlimit {
                    rlim_cur: 0,
                    rlim_max: 0,
                };
                assert_eq!(unsafe { libc::getrlimit(resource, &mut actual) }, 0);
                assert_eq!(actual.rlim_cur, expected);
                assert_eq!(actual.rlim_max, expected);
            }
            if mode == "cancel" {
                drop(guard);
                return;
            }
            std::thread::sleep(Duration::from_secs(40));
            panic!("run-owned deadline did not stop its process");
        }
        for mode in ["cancel", "expire"] {
            let start = Instant::now();
            let result = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "tests::browser_startup_does_not_extend_miner_wall_budget",
                    "--nocapture",
                ])
                .env(CHILD, mode)
                .output()
                .unwrap();
            if mode == "cancel" {
                assert!(result.status.success());
                assert!(start.elapsed() < Duration::from_secs(5));
            } else {
                assert_eq!(result.status.signal(), Some(libc::SIGKILL));
                assert!(start.elapsed() >= Duration::from_secs(30));
                assert!(start.elapsed() < Duration::from_secs(40));
                assert!(String::from_utf8_lossy(&result.stderr)
                    .contains("MD_SIM_MINER_WALL_DEADLINE: 30"));
            }
        }
    }
    #[test]
    fn cross_image_manifest_and_false_stage_metadata_are_rejected() {
        let old = profiles::select("sd2020").unwrap();
        let new = profiles::select("sd2022").unwrap();
        let mut manifest = json!({
            "schema_version":1,"artifact_id":old.artifact_id,"image":old.object(),
            "boot_files":{"boot.bin":{"sha256":"12".repeat(32),"size":79263}},
            "stages":{"spl":{"source_offset":2240,"load":0,"entry":0}}
        });
        assert_eq!(bundle_image(&manifest).unwrap().name, "sd2020");
        manifest["artifact_id"] = json!(new.artifact_id);
        assert!(bundle_image(&manifest).is_err());
        manifest["artifact_id"] = json!("braiins-s9");
        assert!(bundle_image(&manifest).is_err());
        manifest["artifact_id"] = json!(old.artifact_id);
        manifest["image"]["size"] = json!(old.size + 1);
        assert!(bundle_image(&manifest).is_err());
        manifest["image"] = json!(old.object());
        manifest["schema_version"] = json!(2);
        assert!(bundle_image(&manifest).is_err());
        manifest["schema_version"] = json!(1);
        let parent: ObjectId =
            serde_json::from_value(manifest["boot_files"]["boot.bin"].clone()).unwrap();
        check_stage_manifest(&manifest, "spl", "boot.bin", &parent, 2240).unwrap();
        for field in ["load", "entry", "source_offset"] {
            let original = manifest["stages"]["spl"][field].clone();
            manifest["stages"]["spl"][field] = json!(16);
            assert!(check_stage_manifest(&manifest, "spl", "boot.bin", &parent, 2240).is_err());
            manifest["stages"]["spl"][field] = original;
        }
        manifest["boot_files"]["boot.bin"]["sha256"] = json!("34".repeat(32));
        assert!(check_stage_manifest(&manifest, "spl", "boot.bin", &parent, 2240).is_err());
    }

    #[test]
    fn crc_matches_standard_reference() {
        assert_eq!(crc32(b"123456789"), 0xcbf43926);
    }
    #[test]
    fn mbr_rejects_bounds_overlap_and_bad_signature() {
        let mut bytes = vec![0; 4096];
        bytes[510..512].copy_from_slice(&[0x55, 0xaa]);
        bytes[450] = 6;
        bytes[454..458].copy_from_slice(&1_u32.to_le_bytes());
        bytes[458..462].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(partitions(&bytes).unwrap()[0].size, 1024);
        bytes[466] = 6;
        bytes[470..474].copy_from_slice(&2_u32.to_le_bytes());
        bytes[474..478].copy_from_slice(&1_u32.to_le_bytes());
        assert!(partitions(&bytes).is_err());
        bytes[466] = 0;
        bytes[458..462].copy_from_slice(&99_u32.to_le_bytes());
        assert!(partitions(&bytes).is_err());
        bytes[510] = 0;
        assert!(partitions(&bytes).is_err());
    }
}

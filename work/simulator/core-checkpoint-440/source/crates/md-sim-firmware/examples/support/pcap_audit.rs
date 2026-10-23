//! Check actual configuration-transfer records against frozen image inputs.
use anyhow::{ensure, Context, Result};
use md_sim_firmware::{
    bitstream::{parse_header, series7::inspect},
    prepare::hash_bytes,
};
use md_sim_runtime::{
    isolation::{save_json, ProcessResult, ProcessSpec},
    store::{hash_file, ObjectId},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};

fn records(
    input: impl BufRead,
    payload: &[u8],
    sync: usize,
    minimum_images: usize,
) -> Result<Value> {
    let mut input = input;
    let mut armed = false;
    let mut reset_seen = false;
    let mut received = 0usize;
    let mut start = None;
    let mut complete = false;
    let mut images = Vec::new();
    let mut transfers = 0;
    let mut fixed_source_commands = 0;
    let mut legacy_source_mode_commands = 0;
    loop {
        let mut bytes = Vec::new();
        let size = input.by_ref().take(16385).read_until(b'\n', &mut bytes)?;
        if size == 0 {
            break;
        }
        ensure!(size <= 16384, "configuration trace line exceeds bound");
        let line = std::str::from_utf8(&bytes)?.trim();
        ensure!(
            !line.contains("md-sim-pcap: rejected:"),
            "configuration transfer was rejected"
        );
        let Some(record) = line.strip_prefix("md-sim-pcap: ") else {
            continue;
        };
        if record == "reset" || record == "programming-reset" {
            reset_seen = true;
            armed = false;
            received = 0;
            start = None;
            complete = false;
        } else if record == "programming-enable" {
            ensure!(
                reset_seen && !armed,
                "configuration enable lacks preceding reset"
            );
            armed = true;
        } else {
            let fields = record
                .strip_prefix("dma ")
                .context("unknown configuration trace record")?;
            let mut row = BTreeMap::new();
            for field in fields.split_whitespace() {
                let (key, value) = field
                    .split_once('=')
                    .context("configuration field format")?;
                ensure!(
                    row.insert(key, value).is_none(),
                    "duplicate configuration field"
                );
            }
            let increment = match row.remove("source_increment") {
                Some("0") => {
                    fixed_source_commands += 1;
                    false
                }
                Some("1") => true,
                None => {
                    // Historical reference backends only supported increments.
                    // Preserve that trace format without claiming mode readback.
                    legacy_source_mode_commands += 1;
                    true
                }
                _ => anyhow::bail!("invalid configuration source increment"),
            };
            ensure!(
                row.keys().copied().collect::<Vec<_>>()
                    == [
                        "address",
                        "bytes",
                        "complete",
                        "omitted_preamble",
                        "received",
                        "sha256"
                    ],
                "configuration trace fields differ"
            );
            let number = |key: &str| -> Result<usize> { Ok(row[key].parse()?) };
            let count = number("bytes")?;
            let skipped = number("omitted_preamble")?;
            let total = number("received")?;
            let address = u32::from_str_radix(
                row["address"]
                    .strip_prefix("0x")
                    .context("configuration DMA address format")?,
                16,
            )?;
            ensure!(
                armed && !complete && count > 0 && count % 4 == 0 && address % 4 == 0,
                "invalid configuration transfer phase or alignment"
            );
            ensure!(
                (if increment { count as u64 } else { 4 }) <= (1u64 << 32) - u64::from(address),
                "configuration DMA exceeds 32-bit address space"
            );
            ensure!(
                skipped == 0 || skipped == sync,
                "unrecognized configuration preamble"
            );
            ensure!(
                start.is_none_or(|s| s == skipped),
                "configuration preamble changed mid-transfer"
            );
            start = Some(skipped);
            ensure!(
                total
                    == received
                        .checked_add(count)
                        .context("configuration count overflow")?,
                "configuration transfer count differs"
            );
            let end = skipped
                .checked_add(total)
                .context("configuration offset overflow")?;
            let actual_prefix = payload
                .get(skipped..end)
                .context("configuration transfer exceeds image")?;
            ensure!(
                row["sha256"] == hash_bytes(actual_prefix),
                "configuration transfer digest differs"
            );
            complete = end == payload.len();
            ensure!(
                row["complete"] == if complete { "1" } else { "0" },
                "configuration completion is premature or missing"
            );
            received = total;
            transfers += 1;
            ensure!(
                transfers <= 65536 && images.len() < 256,
                "configuration evidence count limit"
            );
            if complete {
                images.push(
                    json!({"received":total,"omitted_preamble":skipped,"sha256":row["sha256"]}),
                );
            }
        }
    }
    ensure!(
        complete && images.len() >= minimum_images,
        "complete configuration transfer evidence absent"
    );
    Ok(
        json!({"passed":true,"completed_images":images,"dma_commands":transfers,
        "fixed_source_commands":fixed_source_commands,"legacy_source_mode_commands":legacy_source_mode_commands,
        "scope":"Checks actual DMA trace ordering, counts and received-word digests against the selected validated image. No FPGA logic reconstruction or physical timing claim."}),
    )
}

fn retain(output: &Path, container: &Path, minimum_images: usize) -> Result<()> {
    let bytes = fs::read(container)?;
    let header = parse_header(&bytes)?;
    let payload = &bytes[header.configuration_offset..];
    let sync = inspect(payload, 0x03722093)?.sync_byte_offset;
    let log = output.join("backend.log");
    ensure!(
        fs::metadata(&log)?.len() <= 512 << 20,
        "configuration log bound"
    );
    let result = records(
        BufReader::new(fs::File::open(&log)?),
        payload,
        sync,
        minimum_images,
    );
    let report = match &result {
        Ok(report) => report.clone(),
        Err(error) => json!({"passed":false,"error":format!("{error:#}")}),
    };
    save_json(
        &output.join("configuration-audit.json"),
        &json!({"result":report,
        "backend_log":hash_file(&log)?,"container":hash_file(container)?,
        "execution":hash_file(&output.join("result.json"))?,
        "verifier_source_sha256":hash_bytes(include_bytes!("pcap_audit.rs")),
        "verifier_binary":hash_file(&std::env::current_exe()?)?}),
    )?;
    result.map(|_| ())
}

fn require_property(argv: &[String], property: &str, name: &str) -> Result<()> {
    let expected = format!("driver=xlnx.ps7-dev-cfg,property={property},value={name}");
    ensure!(
        argv.iter().filter(|a| a.contains(property)).count() == 1,
        "configuration property has competing or noncanonical options"
    );
    ensure!(
        argv.windows(2)
            .filter(|p| p[0] == "-global" && p[1] == expected)
            .count()
            == 1,
        "configuration property missing, duplicate or noncanonical"
    );
    Ok(())
}

/// Shared by direct execution, coordination and result import. The reference
/// copy remains available after disposable stores are cleaned.
pub fn retain_if_selected(
    output: &Path,
    spec: &ProcessSpec,
    result: &ProcessResult,
    images: usize,
) -> Result<()> {
    let selected = spec
        .inputs
        .iter()
        .any(|m| m.guest_path.starts_with("/pcap-"))
        || spec
            .argv
            .iter()
            .any(|a| a.contains("md-bitstream") || a.contains("md-part"));
    if !selected {
        return Ok(());
    }
    ensure!(
        result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
        "configuration proof requires complete successful execution"
    );
    for (property, name) in [
        ("md-bitstream", "/pcap-reference.bit"),
        ("md-part", "/pcap-part.json"),
    ] {
        require_property(&spec.argv, property, name)?;
        ensure!(
            spec.inputs.iter().filter(|m| m.guest_path == name).count() == 1,
            "configuration input absent or duplicated"
        );
    }
    let object = &spec
        .inputs
        .iter()
        .find(|m| m.guest_path == "/pcap-reference.bit")
        .unwrap()
        .object;
    let container = retain_reference(output, object)?;
    retain(output, &container, images)
}

fn retain_reference(output: &Path, object: &ObjectId) -> Result<PathBuf> {
    object.validate()?;
    ensure!(object.size <= 64 << 20, "configuration reference bound");
    let container = output.join("configuration-reference.bit");
    if !container.try_exists()? {
        let source = output.join("input-store/objects").join(&object.sha256);
        ensure!(
            fs::symlink_metadata(&source)?.is_file() && hash_file(&source)? == *object,
            "configuration reference differs from frozen process input"
        );
        fs::copy(source, &container)?;
    }
    ensure!(
        fs::symlink_metadata(&container)?.is_file() && hash_file(&container)? == *object,
        "retained configuration reference differs from frozen process input"
    );
    Ok(container)
}

#[cfg(test)]
mod tests {
    use super::*;
    const GOOD: &str = "md-sim-pcap: reset\nmd-sim-pcap: programming-enable\nmd-sim-pcap: dma address=0x1000 bytes=4 received=4 omitted_preamble=0 complete=1 sha256=88d4266fd4e6338d13b845fcf289579d209c897823b9217da3e161936f031589\n";
    #[test]
    fn missing_false_early_reordered_and_corrupt_records_cannot_pass() {
        assert!(records(GOOD.as_bytes(), b"abcd", 0, 1).is_ok());
        assert!(records(GOOD.as_bytes(), b"abcd", 0, 2).is_err());
        for text in [
            String::new(),
            GOOD.replace("programming-enable", "reset"),
            GOOD.replace("bytes=4", "bytes=8"),
            GOOD.replace("received=4", "received=3"),
            GOOD.replace("complete=1", "complete=0"),
            GOOD.replace("0x1000", "0x1001"),
            GOOD.replace("md-sim-pcap: reset\n", ""),
            GOOD.replace("omitted_preamble=0", "omitted_preamble=4"),
            GOOD.replace("88d426", "00d426"),
            GOOD.replace("bytes=4", "bytes=4 bytes=4"),
            format!("{GOOD}md-sim-pcap: programming-reset\n"),
            format!("{GOOD}qemu: md-sim-pcap: rejected: damaged input\n"),
            format!("{GOOD}{GOOD}").replace("md-sim-pcap: reset\n", ""),
        ] {
            assert!(records(text.as_bytes(), b"abcd", 0, 1).is_err(), "{text}");
        }
        assert!(records(GOOD.as_bytes(), b"abcdefgh", 0, 1).is_err());
        let overflow = GOOD
            .replace("address=0x1000", "address=0xfffffffc")
            .replace("bytes=4 received=4", "bytes=8 received=8")
            .replace(&hash_bytes(b"abcd"), &hash_bytes(b"abcdefgh"));
        assert!(records(overflow.as_bytes(), b"abcdefgh", 0, 1).is_err());
    }

    #[test]
    fn source_mode_controls_address_extent_and_is_reported() {
        let fixed = GOOD
            .replace("address=0x1000", "address=0xfffffffc")
            .replace("bytes=4 received=4", "bytes=8 received=8")
            .replace(&hash_bytes(b"abcd"), &hash_bytes(b"abcdabcd"))
            .replace("complete=1", "complete=1 source_increment=0");
        let report = records(fixed.as_bytes(), b"abcdabcd", 0, 1).unwrap();
        assert_eq!(report["fixed_source_commands"], 1);
        assert_eq!(report["legacy_source_mode_commands"], 0);
        for text in [
            fixed.replace("source_increment=0", "source_increment=1"),
            fixed.replace("source_increment=0", "source_increment=2"),
            fixed.replace(
                "source_increment=0",
                "source_increment=0 source_increment=0",
            ),
            fixed.replace("address=0xfffffffc", "address=0xfffffffd"),
        ] {
            assert!(records(text.as_bytes(), b"abcdabcd", 0, 1).is_err());
        }
        let legacy = records(GOOD.as_bytes(), b"abcd", 0, 1).unwrap();
        assert_eq!(legacy["legacy_source_mode_commands"], 1);
        assert_eq!(legacy["fixed_source_commands"], 0);
    }

    #[test]
    fn retained_reference_survives_store_cleanup_and_rejects_substitution() {
        use std::os::unix::fs::symlink;
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let objects = root.join("input-store/objects");
        fs::create_dir_all(&objects).unwrap();
        let id = ObjectId {
            sha256: hash_bytes(b"abcd"),
            size: 4,
        };
        let source = objects.join(&id.sha256);
        fs::write(&source, b"abcd").unwrap();
        let retained = retain_reference(root, &id).unwrap();
        fs::remove_file(&source).unwrap();
        assert_eq!(
            fs::read(retain_reference(root, &id).unwrap()).unwrap(),
            b"abcd"
        );
        fs::write(&retained, b"abce").unwrap();
        assert!(retain_reference(root, &id).is_err());
        fs::remove_file(&retained).unwrap();
        fs::write(&source, b"abcd").unwrap();
        symlink(&source, &retained).unwrap();
        assert!(retain_reference(root, &id).is_err());
        fs::remove_file(&retained).unwrap();
        fs::write(&source, b"abce").unwrap();
        assert!(retain_reference(root, &id).is_err());
        assert!(!retained.exists());
    }

    #[test]
    fn ignored_repeated_and_overriding_device_options_fail() {
        let option = "driver=xlnx.ps7-dev-cfg,property=md-bitstream,value=/pcap-reference.bit";
        let canonical = vec!["-global".to_owned(), option.to_owned()];
        let check = |args: &[String]| require_property(args, "md-bitstream", "/pcap-reference.bit");
        assert!(check(&canonical).is_ok());
        for args in [
            vec![],
            vec![option.into()],
            vec![
                "-global".into(),
                "xlnx.ps7-dev-cfg.md-bitstream=/pcap-reference.bit".into(),
            ],
            [canonical.clone(), canonical.clone()].concat(),
            [
                canonical.clone(),
                vec![
                    "-global".into(),
                    option.replace("reference.bit", "other.bit"),
                ],
            ]
            .concat(),
            [
                canonical.clone(),
                vec![
                    "-global".into(),
                    "xlnx.ps7-dev-cfg.md-bitstream=/other.bit".into(),
                ],
            ]
            .concat(),
        ] {
            assert!(check(&args).is_err(), "{args:?}");
        }
    }
}

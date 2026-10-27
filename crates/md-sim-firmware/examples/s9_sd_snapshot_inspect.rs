//! Independent SD data inspection with captured qemu-img and 7zip tools.
//! Both tools execute only in the preparation namespace. No guest code runs.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
};
#[allow(dead_code)] // Shared snapshot codec also supplies the boot runner/exporter.
#[path = "support/s9_boot_storage.rs"]
mod boot_storage;
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "s9_boot_bundle/profiles.rs"]
mod profiles;

fn command(program: &str, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()?;
    ensure!(
        output.stdout.len() <= 2 * 1024 * 1024 && output.stderr.len() <= 65536,
        "inspection output exceeds bound"
    );
    println!(
        "MD_SIM_SD_INSPECTION_TOOL {}",
        json!({"program":program,"argv":args,"object":hash_file(Path::new(program))?,
        "exit_code":output.status.code(),"stdout_hex":hex::encode(&output.stdout),"stderr_hex":hex::encode(&output.stderr)})
    );
    ensure!(
        output.status.success(),
        "isolated inspection command failed: {program}"
    );
    Ok(output.stdout)
}

fn inner(replay_journal: bool, baseline: bool, observe_recovery: bool) -> Result<()> {
    confinement::require(true)?;
    ensure!(
        !observe_recovery || (replay_journal && !baseline),
        "recovery observation requires a saved snapshot and journal replay"
    );
    if baseline {
        command(
            "/qemu-img",
            &[
                "create",
                "-f",
                "qcow2",
                "-F",
                "raw",
                "-b",
                "/sd.img",
                "/work/sd.qcow2",
                "128M",
            ],
        )?;
    } else {
        boot_storage::restore()?;
    }
    command("/qemu-img", &["--version"])?;
    command(
        "/qemu-img",
        &[
            "dd",
            "-f",
            "qcow2",
            "if=/work/sd.qcow2",
            "of=/work/mbr",
            "bs=512",
            "count=1",
        ],
    )?;
    let mbr = fs::read("/work/mbr")?;
    ensure!(
        mbr.len() == 512 && mbr[510..] == [0x55, 0xaa],
        "snapshot MBR invalid"
    );
    let entry = &mbr[462..478];
    let start = u32::from_le_bytes(entry[8..12].try_into()?);
    let sectors = u32::from_le_bytes(entry[12..16].try_into()?);
    ensure!(
        entry[4] == 0x83 && start == 83968 && sectors == 131072,
        "selected SD overlay partition differs from archived2022 geometry"
    );
    command(
        "/qemu-img",
        &[
            "dd",
            "-f",
            "qcow2",
            "if=/work/sd.qcow2",
            "of=/work/overlay.ext4",
            "bs=512",
            &format!("skip={start}"),
            // QEMU img_dd bounds the input end by count before applying
            // skip. It is not POSIX dd's output-block count semantics.
            &format!("count={}", start + sectors),
        ],
    )?;
    ensure!(
        fs::metadata("/work/overlay.ext4")?.len() == u64::from(sectors) * 512,
        "extracted partition size differs"
    );
    let raw_partition = hash_file(Path::new("/work/overlay.ext4"))?;
    command("/dumpe2fs", &["-h", "/work/overlay.ext4"])?;
    if replay_journal {
        // This extracted regular file is a disposable inspection copy. The
        // archived image and the saved emulator overlay are read-only inputs.
        // journal_only permits committed journal replay, not general repair.
        command(
            "/e2fsck",
            &["-p", "-E", "journal_only", "/work/overlay.ext4"],
        )?;
        command("/dumpe2fs", &["-h", "/work/overlay.ext4"])?;
    }
    let checked_partition = hash_file(Path::new("/work/overlay.ext4"))?;
    let check = command("/e2fsck", &["-f", "-n", "/work/overlay.ext4"])?;
    ensure!(
        hash_file(Path::new("/work/overlay.ext4"))? == checked_partition,
        "read-only filesystem check changed the inspection image"
    );
    if observe_recovery {
        let mismatches = recovery_counters(&check)?;
        println!(
            "MD_SIM_SD_INSPECTION_RESULT {}",
            json!({
            "recovery_observation_only":true,"partition":checked_partition,"raw_partition":raw_partition,
            "journal_replay_requested":replay_journal,"inspection_copy_changed":raw_partition != checked_partition,
            "read_only_full_check":{"tool":"/e2fsck","exit_code":0,"clean":mismatches.is_empty(),
                "counter_mismatches":mismatches,"unchanged_partition":checked_partition},
            "scope":"Pre-recovery observation only. Known declined free-count repairs remain explicit defects. No clean-filesystem, inventory, boot or recovery pass is granted. The actual recovery boot must use the original saved overlay and pass a separate strict inspection."})
        );
        return Ok(());
    }
    verify_read_only_check(&check)?;
    let listing = String::from_utf8(command(
        "/usr/lib/7zip/7z",
        &["l", "-slt", "/work/overlay.ext4"],
    )?)?;
    let paths: Vec<_> = listing
        .lines()
        .filter_map(|line| line.strip_prefix("Path = "))
        .collect();
    let mut inventory = overlay_entries(&listing)?;
    for (name, entry) in &mut inventory {
        if !entry["mode"]
            .as_str()
            .is_some_and(|mode| mode.starts_with('-'))
        {
            continue;
        }
        let expected = entry["size"]
            .as_u64()
            .context("regular file size missing")?;
        let args = ["x", "-so", "-spd", "/work/overlay.ext4", name.as_str()];
        let result = Command::new("/usr/lib/7zip/7z")
            .args(args)
            .stdin(Stdio::null())
            .stdout(fs::File::create_new("/work/inventory-member")?)
            .output()?;
        let object = hash_file(Path::new("/work/inventory-member"))?;
        println!(
            "MD_SIM_SD_INSPECTION_TOOL {}",
            json!({"program":"/usr/lib/7zip/7z",
            "argv":args,"object":hash_file(Path::new("/usr/lib/7zip/7z"))?,
            "exit_code":result.status.code(),"stdout_file":object,
            "stderr_hex":hex::encode(&result.stderr)})
        );
        ensure!(
            result.status.success() && result.stderr.len() <= 65536 && object.size == expected,
            "exact overlay member extraction failed: {name}"
        );
        entry["object"] = serde_json::to_value(object)?;
        fs::remove_file("/work/inventory-member")?;
    }
    let mut files = BTreeMap::new();
    for name in [
        "upper/etc/md-sim-sd-marker",
        "upper/etc/md-sim-uhttpd-before",
        "upper/etc/config/uhttpd",
        "upper/etc/web_storage.json",
        "upper/etc/config/network",
        ".last_mac",
        ".last_uenv",
    ] {
        let count = paths.iter().filter(|path| **path == name).count();
        ensure!(count <= 1, "ambiguous partition member");
        if count == 0 {
            files.insert(name, json!({"present":false}));
            continue;
        }
        let bytes = command(
            "/usr/lib/7zip/7z",
            &["x", "-so", "/work/overlay.ext4", name],
        )?;
        ensure!(bytes.len() <= 65536, "configuration member exceeds bound");
        files.insert(name,json!({"present":true,"hex":hex::encode(&bytes),"object":{"sha256":md_sim_firmware::prepare::hash_bytes(&bytes),"size":bytes.len()}}));
    }
    let partition = hash_file(Path::new("/work/overlay.ext4"))?;
    // Release the data partition before reading the boot partition. The
    // inspection remains within its declared128MiB temporary storage bound.
    fs::remove_file("/work/overlay.ext4")?;
    let boot = boot_members(&mbr)?;
    println!(
        "MD_SIM_SD_INSPECTION_RESULT {}",
        json!({"files":files,"partition":partition,"boot_members":boot,
        "raw_partition":raw_partition,"journal_replay_requested":replay_journal,
        "inspection_copy_changed":raw_partition != checked_partition,
        "read_only_full_check":{"tool":"/e2fsck","argv":["-f","-n","/work/overlay.ext4"],"exit_code":0,"unchanged_partition":checked_partition},
        "overlay_inventory":inventory,
        "overlay_inventory_scope":"All entries exposed by the captured ext4 reader, with exact bytes hashed for regular files. Symbolic links and whiteouts are separate entries. OverlayFS extended attributes are not assessed; this alone does not establish executable integrity.",
        "mbr":hash_file(Path::new("/work/mbr"))?,"partition_start_sector":start,"partition_sectors":sectors,
        "scope":"Independent captured qcow2 reader, e2fsck and ext4 parser inspect saved data. Optional committed-journal replay changes only a disposable extracted inspection copy, never the snapshot used by the firmware. No general filesystem repair is permitted. Raw clean-shutdown and recovered consistency are distinct results. Absence is not a persistence pass."})
    );
    Ok(())
}

fn verify_read_only_check(output: &[u8]) -> Result<()> {
    // e2fsck1.47.4 can return0 after declining to fix free-space counters.
    // Require the actual five-pass, diagnostic-free output as well as exit0.
    let text = std::str::from_utf8(output)?;
    let lines: Vec<_> = text.lines().filter(|line| !line.is_empty()).collect();
    const PASSES: [&str; 5] = [
        "Pass 1: Checking inodes, blocks, and sizes",
        "Pass 2: Checking directory structure",
        "Pass 3: Checking directory connectivity",
        "Pass 4: Checking reference counts",
        "Pass 5: Checking group summary information",
    ];
    ensure!(
        lines.len() == 6
            && lines[..5] == PASSES
            && lines[5].starts_with("overlay: ")
            && lines[5].ends_with(" blocks"),
        "read-only ext4 check has diagnostics or an incomplete pass sequence"
    );
    Ok(())
}

/// Keep known free-count damage explicit. Do not accept other diagnostics or
/// call this a clean check. The normal inspector keeps its strict assertion.
fn recovery_counters(output: &[u8]) -> Result<BTreeMap<String, Value>> {
    let text = std::str::from_utf8(output)?;
    let lines: Vec<_> = text.lines().filter(|line| !line.is_empty()).collect();
    let mut kept = Vec::new();
    let mut counters = BTreeMap::new();
    let mut index = 0;
    while index < lines.len() {
        let found = [
            ("blocks", "Free blocks count wrong ("),
            ("inodes", "Free inodes count wrong ("),
        ]
        .into_iter()
        .find_map(|(kind, prefix)| lines[index].strip_prefix(prefix).map(|value| (kind, value)));
        if let Some((kind, value)) = found {
            ensure!(
                kept.len() == 5 && lines.get(index + 1) == Some(&"Fix? no"),
                "unexpected counter diagnostic position or repair"
            );
            let (stored, counted) = value
                .strip_suffix(").")
                .and_then(|s| s.split_once(", counted="))
                .context("invalid free-count diagnostic")?;
            ensure!(
                !stored.is_empty()
                    && !counted.is_empty()
                    && stored
                        .bytes()
                        .chain(counted.bytes())
                        .all(|b| b.is_ascii_digit()),
                "invalid free-count values"
            );
            let stored: u64 = stored.parse()?;
            let counted: u64 = counted.parse()?;
            ensure!(
                stored != counted
                    && counters
                        .insert(
                            kind.into(),
                            json!({"stored":stored,"counted":counted,"repair_declined":true})
                        )
                        .is_none(),
                "duplicate or unchanged free-count diagnostic"
            );
            index += 2;
        } else {
            kept.push(lines[index]);
            index += 1;
        }
    }
    verify_read_only_check(kept.join("\n").as_bytes())?;
    Ok(counters)
}

fn overlay_entries(listing: &str) -> Result<BTreeMap<String, Value>> {
    let normalized = listing.replace("\r\n", "\n");
    let (header, body) = normalized
        .split_once("\n----------\n")
        .context("ext4 entry separator missing")?;
    ensure!(
        !header.lines().any(|line| {
            line.starts_with("WARNINGS:")
                || line.starts_with("ERRORS:")
                || line
                    .strip_prefix("Incompatible Features = ")
                    .is_some_and(|features| {
                        features
                            .split_whitespace()
                            .any(|feature| feature == "RECOVER")
                    })
        }),
        "ext4 reader reports errors, warnings or a journal requiring recovery"
    );
    let mut entries = BTreeMap::new();
    for block in body.split("\n\n").filter(|block| !block.trim().is_empty()) {
        let mut fields = BTreeMap::new();
        for line in block.lines() {
            if let Some((name, value)) = line.split_once(" = ") {
                ensure!(
                    fields.insert(name, value).is_none(),
                    "duplicate ext4 entry field"
                );
            }
        }
        let field = |name| {
            fields
                .get(name)
                .copied()
                .context("ext4 entry field missing")
        };
        let name = field("Path")?;
        ensure!(
            !name.is_empty()
                && name.len() <= 1024
                && !name.starts_with('-')
                && !name.chars().any(|c| c.is_control() || c == '\\')
                && Path::new(name)
                    .components()
                    .all(|part| matches!(part, Component::Normal(_))),
            "invalid ext4 member path"
        );
        let mode = field("Mode")?;
        ensure!(
            mode.len() == 10
                && matches!(
                    mode.as_bytes()[0],
                    b'-' | b'd' | b'l' | b'c' | b'b' | b'p' | b's'
                ),
            "unknown ext4 member type"
        );
        ensure!(
            field("Folder")? == if mode.starts_with('d') { "+" } else { "-" },
            "ext4 member type conflict"
        );
        let size = if mode.starts_with('d') {
            None
        } else {
            Some(field("Size")?.parse::<u64>()?)
        };
        ensure!(
            size.is_none_or(|size| size <= 64 * 1024 * 1024),
            "ext4 member size bound"
        );
        let entry = json!({"mode":mode,"size":size,"symbolic_link":field("Symbolic Link")?,
            "inode":field("iNode")?.parse::<u64>()?,"links":field("Links")?.parse::<u64>()?});
        ensure!(
            entries.len() < 1024 && entries.insert(name.to_owned(), entry).is_none(),
            "duplicate member or ext4 inventory bound"
        );
    }
    ensure!(!entries.is_empty(), "empty ext4 inventory");
    Ok(entries)
}

fn boot_members(mbr: &[u8]) -> Result<BTreeMap<&'static str, Value>> {
    let entry = &mbr[446..462];
    let start = u32::from_le_bytes(entry[8..12].try_into()?);
    let sectors = u32::from_le_bytes(entry[12..16].try_into()?);
    ensure!(
        entry[4] == 12 && start == 2048 && sectors == 81920,
        "saved SD boot partition geometry differs"
    );
    command(
        "/qemu-img",
        &[
            "dd",
            "-f",
            "qcow2",
            "if=/work/sd.qcow2",
            "of=/work/boot.fat",
            "bs=512",
            "skip=2048",
            "count=83968",
        ],
    )?;
    ensure!(
        fs::metadata("/work/boot.fat")?.len() == 41943040,
        "extracted boot partition size differs"
    );
    let listing = String::from_utf8(command(
        "/usr/lib/7zip/7z",
        &["l", "-slt", "/work/boot.fat"],
    )?)?;
    let mut files = BTreeMap::new();
    for name in [
        "boot.bin",
        "u-boot.img",
        "fit.itb",
        "system.bit.gz",
        "uEnv.txt",
    ] {
        ensure!(
            listing
                .lines()
                .filter(|line| *line == format!("Path = {name}"))
                .count()
                == 1,
            "saved boot member missing or ambiguous: {name}"
        );
        let argv = ["x", "-so", "-spd", "/work/boot.fat", name];
        let output = Command::new("/usr/lib/7zip/7z")
            .args(argv)
            .stdin(Stdio::null())
            .stdout(fs::File::create_new("/work/member")?)
            .output()?;
        ensure!(output.stderr.len() <= 65536, "boot reader diagnostic bound");
        let object = hash_file(Path::new("/work/member"))?;
        println!(
            "MD_SIM_SD_INSPECTION_TOOL {}",
            json!({"program":"/usr/lib/7zip/7z",
            "argv":argv,"object":hash_file(Path::new("/usr/lib/7zip/7z"))?,
            "exit_code":output.status.code(),"stdout_file":object,"stderr_hex":hex::encode(output.stderr)})
        );
        ensure!(
            output.status.success() && object.size <= 20 * 1024 * 1024,
            "saved boot member extraction failed"
        );
        let content = if name == "uEnv.txt" {
            ensure!(
                object.size <= 65536,
                "boot environment exceeds inspection bound"
            );
            Some(hex::encode(fs::read("/work/member")?))
        } else {
            None
        };
        files.insert(name, json!({"object":object,"hex":content}));
        fs::remove_file("/work/member")?;
    }
    Ok(files)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| {
        arg == "__inspect" || arg == "__inspect-journal" || arg == "__inspect-recovery"
    }) {
        ensure!(
            args.len() == 3 && (args[2] == "snapshot" || args[2] == "baseline"),
            "unexpected inner inspection arguments"
        );
        return inner(
            args[1] != "__inspect",
            args[2] == "baseline",
            args[1] == "__inspect-recovery",
        );
    }
    ensure!(
        args.len() == 3 || (args.len() == 4 && (args[3] == "replay-journal" || args[3] == "observe-recovery")),
        "usage: s9_sd_snapshot_inspect SNAPSHOT_JSON|baseline NEW_OUTPUT [replay-journal|observe-recovery]"
    );
    let replay_journal = args.len() == 4;
    let baseline = args[1] == "baseline";
    let observe_recovery = args.get(3).is_some_and(|arg| arg == "observe-recovery");
    ensure!(
        !observe_recovery || !baseline,
        "recovery observation requires a saved snapshot"
    );
    let source = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    ensure!(!out.exists(), "output must be new");
    let profile = profiles::select("sd2022")?;
    ensure!(
        hash_file(Path::new(profile.path))? == profile.object(),
        "original SD differs"
    );
    let state = if baseline {
        None
    } else {
        Some(boot_storage::input(source, &profile.object())?)
    };
    let snapshot = if baseline {
        None
    } else {
        Some(hash_file(source)?)
    };
    fs::create_dir_all(out)?;
    save_json(
        &out.join("input-origin.json"),
        &json!({"artifact_id":profile.artifact_id,
        "original_image":profile.object(),"boot_members":profile.boot_members,"snapshot":snapshot,
        "archived_baseline_only":baseline,
        "inspection_journal_replay":replay_journal,"recovery_observation_only":observe_recovery,
        "e2fsck_manual":hash_file(Path::new("/usr/share/man/man8/e2fsck.8.gz"))?}),
    )?;
    let store = Store::open(&out.join("inputs"), 512 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/probe".into(), std::env::current_exe()?),
            ("/qemu-img".into(), PathBuf::from("/usr/bin/qemu-img")),
            ("/e2fsck".into(), PathBuf::from("/usr/bin/e2fsck")),
            ("/dumpe2fs".into(), PathBuf::from("/usr/bin/dumpe2fs")),
            ("/usr/lib/7zip/7z".into(), PathBuf::from("/usr/lib/7zip/7z")),
            (
                "/usr/lib/7zip/7z.so".into(),
                PathBuf::from("/usr/lib/7zip/7z.so"),
            ),
        ],
    )?;
    let mut data = vec![(PathBuf::from(profile.path), "/sd.img")];
    if let Some(state) = state {
        data.push((source.to_path_buf(), "/sd-state.json"));
        data.push((
            source
                .parent()
                .context("state parent absent")?
                .join(boot_storage::file(&state)),
            "/sd-state.qcow2.gz",
        ));
    }
    for (path, guest) in data {
        let object = hash_file(&path)?;
        store.import(&path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: guest.into(),
        });
    }
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec![
            "/probe".into(),
            if observe_recovery {
                "__inspect-recovery"
            } else if replay_journal {
                "__inspect-journal"
            } else {
                "__inspect"
            }
            .into(),
            if baseline { "baseline" } else { "snapshot" }.into(),
        ],
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 60,
            cpu_seconds: 40,
            memory_bytes: 512 * 1024 * 1024,
            processes: 16,
            file_bytes: 64 * 1024 * 1024,
            trace_bytes: 8 * 1024 * 1024,
            tmp_bytes: 128 * 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    let stdout = hex::decode(&result.stdout)?;
    fs::write(out.join("stdout.log"), &stdout)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    ensure!(
        result.status == "exited" && result.exit_code == Some(0) && !result.trace_truncated,
        "isolated SD inspection failed"
    );
    let text = String::from_utf8(stdout)?;
    let records: Vec<Value> = text
        .lines()
        .filter_map(|line| line.strip_prefix("MD_SIM_SD_INSPECTION_RESULT "))
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    ensure!(records.len() == 1, "exactly one inspection result required");
    save_json(
        &out.join("inspection.json"),
        &json!({"snapshot":snapshot,"archived_baseline_only":baseline,"process_specification":hash_file(&out.join("spec.json"))?,
        "execution_result":hash_file(&out.join("result.json"))?,"inspection":records[0]}),
    )?;
    println!("Independent SD inspection retained");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn successful_exit_cannot_hide_declined_counter_repairs() {
        let passes = "Pass 1: Checking inodes, blocks, and sizes\nPass 2: Checking directory structure\nPass 3: Checking directory connectivity\nPass 4: Checking reference counts\nPass 5: Checking group summary information\n";
        let summary = "overlay: 78/16384 files (0.0% non-contiguous), 7525/65536 blocks\n";
        verify_read_only_check(format!("{passes}{summary}").as_bytes()).unwrap();
        let dirty = format!("{passes}Free blocks count wrong (26726, counted=26698).\nFix? no\nFree inodes count wrong (16366, counted=16329).\nFix? no\n{summary}");
        let observed = recovery_counters(dirty.as_bytes()).unwrap();
        assert_eq!(observed["blocks"]["stored"], 26726);
        assert_eq!(observed["inodes"]["counted"], 16329);
        assert!(verify_read_only_check(dirty.as_bytes()).is_err());
        assert!(recovery_counters(format!("{passes}{summary}").as_bytes())
            .unwrap()
            .is_empty());
        for bad in [
            dirty.replace("Fix? no", "Fix? yes"),
            dirty.replace("counted=26698", "counted=26726"),
            dirty.replace("counted=26698", "counted=-1"),
            dirty.replace("Free inodes", "Free blocks"),
            dirty.replace("Pass 4: Checking reference counts", "Pass 4: WARNING"),
            dirty.replace(summary, &format!("Filesystem still has errors\n{summary}")),
        ] {
            assert!(recovery_counters(bad.as_bytes()).is_err());
        }
        for bad in [
            format!("{passes}Free blocks count wrong (26726, counted=26698).\nFix? no\n{summary}"),
            format!("{passes}WARNING: Filesystem still has errors\n{summary}"),
            passes.to_string(),
            summary.to_string(),
            format!("{passes}{summary}{summary}"),
        ] {
            assert!(verify_read_only_check(bad.as_bytes()).is_err());
        }
    }
    #[test]
    fn inventory_keeps_links_and_whiteouts_distinct_from_regular_files() {
        let regular = "Path = upper/etc/web_storage.json\nFolder = -\nSize = 106\nMode = -rw-r--r--\niNode = 43\nLinks = 1\nSymbolic Link = \n";
        let listing = format!("header\n----------\n{regular}\nPath = .fs_state\nFolder = -\nSize = 1\nMode = lrwxrwxrwx\niNode = 44\nLinks = 1\nSymbolic Link = 2\n\nPath = upper/etc/uci-defaults/10-fstab\nFolder = -\nSize = 0\nMode = c---------\niNode = 45\nLinks = 1\nSymbolic Link = \n");
        let entries = overlay_entries(&listing).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[".fs_state"]["symbolic_link"], "2");
        assert_eq!(
            entries["upper/etc/uci-defaults/10-fstab"]["mode"],
            "c---------"
        );
        for bad in [
            listing.replace("upper/etc/web_storage.json", "../escaped"),
            listing.replace("Size = 106", "Size = 106\nSize = 107"),
            format!("{listing}\n{regular}"),
            listing.replace("Folder = -", "Folder = +"),
            listing.replace("header", "WARNINGS:\nHeaders Error"),
            listing.replace("header", "ERRORS:\nData Error"),
            listing.replace("header", "Incompatible Features = FILETYPE RECOVER EXTENTS"),
        ] {
            assert!(overlay_entries(&bad).is_err());
        }
    }
}

//! Inspect and exercise the archived S9 native services in a bounded guest.
//! This diagnostic environment is separate from the original-board gate.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute_with_trace, save_json, Limits, Mount, ProcessSpec, TraceFiles},
    store::{hash_file, Store},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};

#[path = "../src/confinement.rs"]
#[allow(dead_code)] // Shared guard also has preparation entry points.
mod confinement;
#[path = "support/s9_service_report.rs"]
mod service_report;
#[path = "support/squashfs_member.rs"]
mod squashfs_member;
#[path = "support/s9_storage.rs"]
mod storage;

const COMPONENTS: &[(&str, &str)] = &[
    (
        "kernel",
        "f85eba975b58b4333e99a656ad661667cbedaf3111af3deb8572b902b88cd276",
    ),
    (
        "ramdisk",
        "2bd49c70b915b7e25f6c01013d95a5ffea8d499b73c6c6ec5539fda4247154b3",
    ),
    (
        "board.dtb",
        "550c87818cb0db41c00a8351dc4ac7b5dce5bf6d5336539c6951163ebf3cb62a",
    ),
];
const BOOTARGS: &str = "earlycon=cdns,mmio,0xe0001000 console=ttyPS0,115200n8 initcall_blacklist=xiic_i2c_driver_init root=/dev/ram0 init=/bin/sh";
const BOSMINER_MEMBER: &str = "usr/bin/bosminer";
const BOSMINER_SIZE: u64 = 12_699_280;
use md_sim_firmware::serial_console::SerialGuest;

fn add_input(
    store: &Store,
    inputs: &mut Vec<Mount>,
    source: &Path,
    guest: &str,
    expected: &str,
) -> Result<()> {
    let object = hash_file(source)?;
    ensure!(
        object.sha256 == expected,
        "captured component differs from archived FIT: {guest}"
    );
    store.import(source, &object)?;
    inputs.push(Mount {
        object,
        guest_path: guest.into(),
    });
    Ok(())
}

fn serial_inside() -> Result<()> {
    confinement::require_board()?;
    let mut script = Vec::new();
    std::io::stdin().take(65537).read_to_end(&mut script)?;
    ensure!(
        script.len() <= 65536 && !script.contains(&0),
        "serial script exceeds policy"
    );
    serial_once(script, None)
}

fn serial_once(script: Vec<u8>, disk: Option<&Path>) -> Result<()> {
    serial_guest(disk, false)?.script(script)
}

fn serial_guest(disk: Option<&Path>, allow_reset: bool) -> Result<SerialGuest> {
    let mut command = Command::new("/qemu-system-arm");
    command.args([
        "-machine",
        "xilinx-zynq-a9",
        "-accel",
        "tcg,thread=single",
        "-m",
        "1024M",
        "-smp",
        "1",
        "-nodefaults",
        "-nic",
        "none",
        "-display",
        "none",
        "-monitor",
        "none",
        "-serial",
        "null",
        "-serial",
        "stdio",
        "-kernel",
        "/kernel",
        "-initrd",
        "/ramdisk",
        "-dtb",
        "/board.dtb",
        "-append",
        BOOTARGS,
        "-sandbox",
        "on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny",
    ]);
    if !allow_reset {
        command.arg("-no-reboot");
    }
    if let Some(disk) = disk {
        ensure!(disk == Path::new("/work/persistent.img"), "unexpected disk");
        command.args([
            "-drive",
            "file=/work/persistent.img,if=sd,index=0,format=raw",
        ]);
    }
    SerialGuest::start(command, false)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|s| s == "__extract-bosminer") {
        confinement::require_board()?;
        ensure!(
            args.len() == 2,
            "fixed Bosminer extraction takes no member override"
        );
        return squashfs_member::extract_sized(BOSMINER_MEMBER, BOSMINER_SIZE);
    }
    if args.get(1).is_some_and(|s| s == "__extract") {
        confinement::require_board()?;
        ensure!(args.len() == 3, "exact extraction needs one member");
        return squashfs_member::extract(args[2].to_str().context("invalid member")?);
    }
    if args.get(1).is_some_and(|s| s == "__serial") {
        return serial_inside();
    }
    if args
        .get(1)
        .is_some_and(|s| s == "__storage-cycle" || s == "__storage-warm-cycle")
    {
        confinement::require_board()?;
        let warm = args[1] == "__storage-warm-cycle";
        let mut guest: Option<SerialGuest> = None;
        return storage::run(|script, disk| {
            if !warm {
                return serial_once(script, Some(disk));
            }
            if let Some(guest) = guest.as_mut() {
                guest.reboot()?;
            } else {
                guest = Some(serial_guest(Some(disk), true)?);
            }
            guest.as_mut().unwrap().script(script)
        });
    }
    if args.get(1).is_some_and(|s| s == "report") {
        ensure!(
            args.len() == 4,
            "usage: s9_native_service report RUN_DIRECTORY NEW_REPORT"
        );
        let report = service_report::report(Path::new(&args[2]), Path::new(&args[3]))?;
        println!(
            "{}",
            json!({"scope":report["scope"],"passed_component_checks":report["passed_component_checks"],"full_native_dashboard_gate":"incomplete"})
        );
        return Ok(());
    }
    ensure!((4..=5).contains(&args.len()), "usage: s9_native_service inspect|extract|extract-bosminer|shell|storage-cycle|storage-warm-cycle COMPONENT_DIRECTORY NEW_OUTPUT [MEMBER|SCRIPT_FILE]");
    let mode = args[1].to_str().context("mode is not UTF-8")?;
    let is_storage = matches!(mode, "storage-cycle" | "storage-warm-cycle");
    ensure!(
        matches!(mode, "inspect" | "extract" | "extract-bosminer" | "shell") || is_storage,
        "unknown mode"
    );
    let component_dir = PathBuf::from(&args[2]);
    let out = PathBuf::from(&args[3]);
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(&out)?;
    let store = Store::open(&out.join("inputs"), 512 * 1024 * 1024)?;
    for (name, expected) in COMPONENTS {
        ensure!(
            hash_file(&component_dir.join(name))?.sha256 == *expected,
            "{name} differs from archived FIT component"
        );
    }
    let mut programs = if mode == "shell" || is_storage {
        vec![
            ("/probe".into(), std::env::current_exe()?),
            (
                "/qemu-system-arm".into(),
                PathBuf::from("/usr/bin/qemu-system-arm"),
            ),
        ]
    } else {
        vec![
            ("/probe".into(), std::env::current_exe()?),
            ("/usr/lib/7zip/7z".into(), PathBuf::from("/usr/lib/7zip/7z")),
            (
                "/usr/lib/7zip/7z.so".into(),
                PathBuf::from("/usr/lib/7zip/7z.so"),
            ),
        ]
    };
    if is_storage {
        programs.push(("/mke2fs".into(), PathBuf::from("/usr/bin/mke2fs")));
    }
    let mut inputs = md_sim_firmware::lab::elf_closure(&store, programs)?;
    for (name, expected) in COMPONENTS {
        add_input(
            &store,
            &mut inputs,
            &component_dir.join(name),
            &format!("/{name}"),
            expected,
        )?;
    }
    let mut stdin_object = None;
    let argv = match mode {
        "inspect" => vec![
            "/usr/lib/7zip/7z".into(),
            "l".into(),
            "-slt".into(),
            "/ramdisk".into(),
        ],
        "extract" => {
            ensure!(args.len() == 5, "extract needs one exact member");
            vec![
                "/probe".into(),
                "__extract".into(),
                args[4].to_str().context("invalid member")?.into(),
            ]
        }
        "extract-bosminer" => {
            ensure!(
                args.len() == 4,
                "fixed Bosminer extraction takes no member override"
            );
            vec!["/probe".into(), "__extract-bosminer".into()]
        }
        "shell" => {
            ensure!(args.len() == 5, "shell needs a script file");
            let path = Path::new(&args[4]);
            let object = hash_file(path)?;
            ensure!(object.size <= 65536, "serial script too large");
            store.import(path, &object)?;
            stdin_object = Some(object);
            vec!["/probe".into(), "__serial".into()]
        }
        "storage-cycle" | "storage-warm-cycle" => {
            ensure!(args.len() == 4, "storage-cycle takes no script");
            let mut entropy = [0; 32];
            fs::File::open("/dev/urandom")?.read_exact(&mut entropy)?;
            let path = out.join("fresh-marker.txt");
            fs::write(&path, hex::encode(entropy))?;
            let object = hash_file(&path)?;
            store.import(&path, &object)?;
            stdin_object = Some(object);
            vec!["/probe".into(), format!("__{mode}")]
        }
        _ => unreachable!(),
    };
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv,
        inputs,
        stdin_object,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 90,
            cpu_seconds: 75,
            memory_bytes: 3 * 1024 * 1024 * 1024,
            processes: 64,
            file_bytes: 32 * 1024 * 1024,
            trace_bytes: if mode == "extract-bosminer" { 32 } else { 16 } * 1024 * 1024,
            tmp_bytes: 32 * 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    save_json(
        &out.join("adaptation.json"),
        &json!({
            "artifact_id":"braiins-feed-25bba3b4a505ccbc0e54",
            "archive_sha256":"469743ebcd5faf3445e4f4209ad8af58cc7adeff37c7df28af3c53a5b255b56e",
            "fit_sha256":"8b2a32020281aa67490c01633adf99c1a2ca6c9897e4c210bb6d0d001c15caca",
            "component_bytes_unchanged":true, "components":COMPONENTS,
            "mode":mode, "bootargs":if mode=="shell" || is_storage {Some(BOOTARGS)} else {None},
            "execution_mode":if mode=="shell" || is_storage {"board diagnostic; direct Linux loader, generic machine, one CPU, missing XIIC driver disabled, init shell override"} else {"isolated data inspection; no firmware instructions execute"},
            "network":"no host network; guest loopback only; no remote dashboard claim",
            "original_board_gate":"incomplete", "native_dashboard_gate":"incomplete",
            "script_completion_is_not_service_health":true,
            "storage":if is_storage {Some("New8MiB ext2 SD fixture. Native UCI writes and restores /etc/config/uhttpd. Original physical boot storage is not established.")} else {None},
            "restart_requested":match mode {"storage-cycle"=>Some("two separate QEMU processes"),"storage-warm-cycle"=>Some("guest reboot in the same QEMU process"),_=>None}
        }),
    )?;
    let trace = TraceFiles {
        stdout: fs::File::create(out.join("stdout.bin"))?,
        stderr: fs::File::create(out.join("stderr.log"))?,
    };
    let result = execute_with_trace(&store, &spec, Arc::new(AtomicBool::new(false)), Some(trace))?;
    save_json(&out.join("result.json"), &result)?;
    if is_storage {
        storage::retain_images(&out)?;
    }
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated,
        "isolated {mode} failed; see saved result"
    );
    if matches!(mode, "extract" | "extract-bosminer") {
        save_json(
            &out.join("member.json"),
            &json!({
                "member":if mode=="extract-bosminer" { BOSMINER_MEMBER } else { args[4].to_str().context("invalid member")? },
                "object":hash_file(&out.join("stdout.bin"))?,
                "parent_ramdisk":spec.inputs.iter().find(|input|input.guest_path=="/ramdisk")
                    .map(|input|&input.object).context("captured ramdisk identity absent")?,
                "kind":"Exact regular SquashFS member; isolated listing and extracted size agree",
                "result":hash_file(&out.join("result.json"))?,
            }),
        )?;
    }
    println!(
        "{}",
        json!({"mode":mode,"output":out,"status":result.status,"original_board_gate":"incomplete"})
    );
    Ok(())
}

#[cfg(test)]
mod component_capture_tests {
    use super::*;
    #[test]
    fn a_component_changed_after_preflight_cannot_enter_the_specification() {
        let root = std::env::temp_dir().join(format!(
            "md-sim-component-511-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let file = root.join("ramdisk");
        fs::write(&file, b"original component").unwrap();
        let expected = hash_file(&file).unwrap();
        fs::write(&file, b"changed component").unwrap();
        let store = Store::open(&root.join("objects"), 1024).unwrap();
        let mut mounts = Vec::new();
        assert!(add_input(&store, &mut mounts, &file, "/ramdisk", &expected.sha256).is_err());
        assert!(mounts.is_empty());
        drop(store);
        fs::remove_dir_all(root).unwrap();
    }
}

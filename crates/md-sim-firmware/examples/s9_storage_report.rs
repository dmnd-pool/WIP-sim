//! Verify adapted cold/warm restart tests with an independent ext2 reader.
//! The captured7z implementation runs in a separate closed namespace.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, Store},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};
const DISK_BYTES: u64 = 8 * 1024 * 1024;

fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "input exceeds bound");
    Ok(bytes)
}

fn extract(
    store: &Store,
    tools: &[Mount],
    disk: &Path,
    member: &str,
    out: &Path,
) -> Result<Vec<u8>> {
    fs::create_dir(out)?;
    let object = hash_file(disk)?;
    store.import(disk, &object)?;
    let mut inputs = tools.to_vec();
    inputs.push(Mount {
        object,
        guest_path: "/sd.img".into(),
    });
    let spec = ProcessSpec {
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec![
            "/usr/lib/7zip/7z".into(),
            "x".into(),
            "-so".into(),
            "/sd.img".into(),
            member.into(),
        ],
        inputs,
        stdin_object: None,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 10,
            cpu_seconds: 5,
            memory_bytes: 256 * 1024 * 1024,
            processes: 16,
            file_bytes: 1024 * 1024,
            trace_bytes: 1024 * 1024,
            tmp_bytes: 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute(store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated,
        "independent ext2 extraction failed"
    );
    let bytes = hex::decode(result.stdout)?;
    ensure!(
        !bytes.is_empty() && bytes.len() < 65536,
        "member absent or too large"
    );
    fs::write(out.join("member.bin"), &bytes)?;
    Ok(bytes)
}

fn verify(run: &Path, out: &Path) -> Result<Value> {
    let result: Value = serde_json::from_slice(&read(&run.join("result.json"), 32 << 20)?)?;
    ensure!(
        result["status"] == "exited"
            && result["exit_code"] == 0
            && result["trace_truncated"] == false,
        "guest storage cycle did not pass"
    );
    let token = String::from_utf8(read(&run.join("fresh-marker.txt"), 64)?)?;
    ensure!(
        token.len() == 64
            && token
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "fresh marker differs"
    );
    let expected = 101 + u32::from_str_radix(&token[..8], 16)? % 900;
    let spec: ProcessSpec = serde_json::from_slice(&read(&run.join("spec.json"), 1 << 20)?)?;
    spec.validate()?;
    let warm = spec.argv == ["/probe", "__storage-warm-cycle"];
    ensure!(
        (warm || spec.argv == ["/probe", "__storage-cycle"])
            && spec.stdin_object.as_ref() == Some(&hash_file(&run.join("fresh-marker.txt"))?),
        "selected driver command or fresh input differs"
    );
    let run_store = Store::open(&run.join("inputs"), 512 * 1024 * 1024)?;
    for input in &spec.inputs {
        run_store.verified_path(&input.object)?;
    }
    run_store.verified_path(spec.stdin_object.as_ref().unwrap())?;
    let mut firmware = BTreeMap::new();
    for (name, expected) in [
        (
            "/kernel",
            "f85eba975b58b4333e99a656ad661667cbedaf3111af3deb8572b902b88cd276",
        ),
        (
            "/ramdisk",
            "2bd49c70b915b7e25f6c01013d95a5ffea8d499b73c6c6ec5539fda4247154b3",
        ),
        (
            "/board.dtb",
            "550c87818cb0db41c00a8351dc4ac7b5dce5bf6d5336539c6951163ebf3cb62a",
        ),
    ] {
        let input = spec
            .inputs
            .iter()
            .find(|input| input.guest_path == name)
            .context("firmware input absent")?;
        ensure!(input.object.sha256 == expected, "selected firmware differs");
        firmware.insert(name, run_store.verified_path(&input.object)?);
    }
    let serial_bytes = read(&run.join("stdout.bin"), 16 << 20)?;
    ensure!(
        hex::decode(result["stdout"].as_str().context("result trace absent")?)? == serial_bytes
            && hex::decode(result["stderr"].as_str().context("result trace absent")?)?
                == read(&run.join("stderr.log"), 16 << 20)?,
        "saved trace differs from process result"
    );
    let serial = String::from_utf8(serial_bytes)?;
    let resets: Vec<_> = serial
        .lines()
        .filter_map(|line| line.strip_prefix("MD_SIM_STORAGE_WARM_RESET "))
        .collect();
    if warm {
        ensure!(resets.len() == 1, "one warm-reset observation required");
        let reset: Value = serde_json::from_str(resets[0])?;
        ensure!(
            reset["command"] == "/sbin/reboot -f"
                && reset["emulator_pid_before"]
                    .as_u64()
                    .is_some_and(|pid| pid > 0)
                && reset["emulator_pid_before"] == reset["emulator_pid_after"]
                && reset["kernel_restart_and_new_shell_observed"] == true,
            "emulator changed or guest reset was not observed"
        );
        let committed = serial
            .find("\r\nMD_SIM_STORAGE_FIRST_COMMITTED\r\n")
            .context("pre-reset commit marker absent")?;
        let reset_line = serial
            .find("reboot: Restarting system")
            .context("actual kernel restart line absent")?;
        let second = serial
            .find("\r\nMD_SIM_STORAGE_SECOND_VALUE:")
            .context("post-reset configuration absent")?;
        ensure!(
            committed < reset_line && reset_line < second,
            "reset order differs"
        );
    } else {
        ensure!(resets.is_empty(), "unexpected warm-reset evidence");
    }
    for marker in [
        format!("MD_SIM_STORAGE_FIRST_VALUE:{expected}"),
        format!("MD_SIM_STORAGE_SECOND_VALUE:{expected}"),
        "MD_SIM_STORAGE_RESTORED_VALUE:100".into(),
        "MD_SIM_STORAGE_FIRST_COMMITTED".into(),
        "MD_SIM_STORAGE_SECOND_RESTORED".into(),
    ] {
        ensure!(
            serial
                .lines()
                .filter(|line| line.trim_end() == marker)
                .count()
                == 1,
            "actual guest observation absent or duplicated: {marker}"
        );
    }
    ensure!(
        serial
            .lines()
            .filter(|line| *line == "MD_SIM_SERIAL_SCRIPT_FINISHED:0")
            .count()
            == 2,
        "two successful guest scripts required"
    );
    let images: Vec<Value> =
        serde_json::from_slice(&read(&run.join("storage-images.json"), 16384)?)?;
    ensure!(images.len() == 3, "three disk stages required");
    let store = Store::open(&out.join("inputs"), 128 * 1024 * 1024)?;
    let tools = md_sim_firmware::lab::elf_closure(
        &store,
        vec![
            ("/usr/lib/7zip/7z".into(), PathBuf::from("/usr/lib/7zip/7z")),
            (
                "/usr/lib/7zip/7z.so".into(),
                PathBuf::from("/usr/lib/7zip/7z.so"),
            ),
        ],
    )?;
    let mut raw_paths = BTreeMap::new();
    for (image, stage) in images.iter().zip(["formatted", "committed", "restored"]) {
        let filename = format!("sd-{stage}.img.gz");
        ensure!(
            image["stage"] == stage && image["file"] == filename,
            "disk stages differ"
        );
        let compressed = read(&run.join(&filename), 1024 * 1024)?;
        ensure!(
            serde_json::to_value(hash_file(&run.join(&filename))?)? == image["compressed"],
            "compressed disk identity differs"
        );
        let mut raw = Vec::new();
        flate2::read::GzDecoder::new(compressed.as_slice())
            .take(DISK_BYTES + 1)
            .read_to_end(&mut raw)?;
        ensure!(
            raw.len() as u64 == DISK_BYTES
                && image["raw"]["size"] == DISK_BYTES
                && image["raw"]["sha256"] == md_sim_firmware::prepare::hash_bytes(&raw),
            "raw disk identity differs"
        );
        let path = out.join(format!("sd-{stage}.img"));
        fs::write(&path, raw)?;
        raw_paths.insert(stage, path);
    }
    let mut configs = BTreeMap::new();
    let mut members = Vec::new();
    for stage in ["committed", "restored"] {
        for (label, member) in [
            ("config", "etc/config/uhttpd"),
            ("before", "uhttpd.before"),
            ("marker", "fresh-marker"),
        ] {
            let directory = out.join(format!("{stage}-{label}"));
            let bytes = extract(&store, &tools, &raw_paths[stage], member, &directory)?;
            members.push(json!({"stage":stage,"member":member,
                "identity":hash_file(&directory.join("member.bin"))?}));
            if label == "marker" {
                ensure!(
                    bytes == format!("{token}\n").as_bytes(),
                    "persisted marker differs"
                );
            } else {
                configs.insert((stage, label), bytes);
            }
        }
    }
    let original = &configs[&("committed", "before")];
    let archived = extract(
        &store,
        &tools,
        &firmware["/ramdisk"],
        "etc/config/uhttpd",
        &out.join("archived-config"),
    )?;
    ensure!(
        original == &archived
            && original == &configs[&("restored", "before")]
            && original == &configs[&("restored", "config")],
        "restored file differs from original bytes"
    );
    let committed = &configs[&("committed", "config")];
    ensure!(committed != original, "configuration did not change");
    let text = std::str::from_utf8(committed)?;
    let wanted = format!("option max_connections '{expected}'");
    ensure!(
        text.lines().filter(|line| line.trim() == wanted).count() == 1,
        "independently extracted configuration has the wrong value"
    );
    let mut inputs = BTreeMap::new();
    for name in [
        "result.json",
        "spec.json",
        "adaptation.json",
        "stdout.bin",
        "stderr.log",
        "fresh-marker.txt",
        "storage-images.json",
    ] {
        inputs.insert(name, hash_file(&run.join(name))?);
    }
    Ok(
        json!({"schema":"md-sim-adapted-s9-storage-report/1","passed":true,
        "scope":"Original archived kernel and UCI in an adapted environment with a new ext2 SD fixture",
        "restart_kind":if warm {"guest-requested warm reset in one QEMU instance"} else {"two QEMU processes with flushed SD data"},
        "independent_reader":"Captured7z in a separate closed namespace",
        "reader_inputs":tools,"reporter":hash_file(&std::env::current_exe()?)?,
        "source_sha256":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_storage_report.rs")),
        "inputs":inputs,"images":images,"members":members,
        "archived_config":hash_file(&out.join("archived-config/member.bin"))?,
        "verified_execution_inputs":spec.inputs,
        "fresh_value":expected,"restored_exact_bytes":true,
        "native_dashboard_reboot_gate":false,"original_board_gate":false,
        "warm_reset":warm,"unclean_power_loss":false,"mining_gate":false}),
    )
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(args.len() == 3, "usage: s9_storage_report RUN NEW_OUTPUT");
    let run = Path::new(&args[1]);
    let out = Path::new(&args[2]);
    ensure!(!out.exists(), "report output must be new");
    fs::create_dir_all(out)?;
    let report = verify(run, out);
    match report {
        Ok(report) => {
            save_json(&out.join("report.json"), &report)?;
            println!(
                "{}",
                json!({"passed":true,"output":out,"scope":report["scope"]})
            );
            Ok(())
        }
        Err(error) => {
            save_json(
                &out.join("failure.json"),
                &json!({"passed":false,"error":format!("{error:#}")}),
            )?;
            Err(error).context("storage evidence verification failed")
        }
    }
}

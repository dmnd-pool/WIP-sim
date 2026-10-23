//! Three original SPL/bootloader/kernel starts test the actual SD overlay.
//! The native guest writes, reads after restart, restores, then reads again.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{isolation::save_json, store::hash_file};
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    path::Path,
    process::{Command, Stdio},
};

fn scripts(token: &str, value: u32) -> [String; 3] {
    let first = format!(
        r#"(
set -eu
test "$(uci get uhttpd.main.max_connections)" = 100
test ! -e /etc/md-sim-sd-marker
test ! -e /etc/md-sim-uhttpd-before
cp /etc/config/uhttpd /etc/md-sim-uhttpd-before
uci set uhttpd.main.max_connections='{value}'
uci commit uhttpd
test "$(uci get uhttpd.main.max_connections)" = '{value}'
printf '%s\n' '{token}' > /etc/md-sim-sd-marker
printf '\nMD_SIM_SD_FIRST_VALUE:'
uci get uhttpd.main.max_connections
printf '\nMD_SIM_SD_ORIGINAL_CONFIG:'
sha256sum /etc/md-sim-uhttpd-before
sync
printf '\nMD_SIM_SD_FIRST_COMMITTED\n'
)
"#
    );
    let second = format!(
        r#"(
set -eu
test "$(cat /etc/md-sim-sd-marker)" = '{token}'
test "$(uci get uhttpd.main.max_connections)" = '{value}'
printf '\nMD_SIM_SD_RESTART_VALUE:'
uci get uhttpd.main.max_connections
cp /etc/md-sim-uhttpd-before /etc/config/uhttpd
cmp /etc/md-sim-uhttpd-before /etc/config/uhttpd
test "$(uci get uhttpd.main.max_connections)" = 100
rm /etc/md-sim-sd-marker /etc/md-sim-uhttpd-before
printf '\nMD_SIM_SD_RESTORED_CONFIG:'
sha256sum /etc/config/uhttpd
sync
printf '\nMD_SIM_SD_SECOND_RESTORED\n'
)
"#
    );
    let third = r#"(
set -eu
test ! -e /etc/md-sim-sd-marker
test ! -e /etc/md-sim-uhttpd-before
test "$(uci get uhttpd.main.max_connections)" = 100
printf '\nMD_SIM_SD_FINAL_VALUE:'
uci get uhttpd.main.max_connections
printf '\nMD_SIM_SD_FINAL_CONFIG:'
sha256sum /etc/config/uhttpd
printf '\nMD_SIM_SD_THIRD_VERIFIED\n'
)
"#
    .to_owned();
    [first, second, third]
}

fn assess(log: &str, value: u32) -> Result<()> {
    let mut previous = 0;
    for marker in [
        format!("MD_SIM_SD_FIRST_VALUE:{value}"),
        "MD_SIM_SD_FIRST_COMMITTED".into(),
        format!("MD_SIM_SD_RESTART_VALUE:{value}"),
        "MD_SIM_SD_SECOND_RESTORED".into(),
        "MD_SIM_SD_FINAL_VALUE:100".into(),
        "MD_SIM_SD_THIRD_VERIFIED".into(),
    ] {
        let needle = format!("\r\n{marker}\r\n");
        let matches: Vec<_> = log.match_indices(&needle).collect();
        ensure!(
            matches.len() == 1 && matches[0].0 > previous,
            "native SD observation absent, repeated or out of order: {marker}"
        );
        previous = matches[0].0;
    }
    for marker in [
        "U-Boot SPL 2016.03-g8728fee00a",
        "U-Boot 2016.03-gf30f5b605c",
        "Linux version 4.4.0-xilinx-g518c00cd977f-dirty",
    ] {
        ensure!(
            log.matches(marker).count() == 3,
            "three actual boot stages required: {marker}"
        );
    }
    let mut checksums = Vec::new();
    for marker in [
        "MD_SIM_SD_ORIGINAL_CONFIG:",
        "MD_SIM_SD_RESTORED_CONFIG:",
        "MD_SIM_SD_FINAL_CONFIG:",
    ] {
        let needle = format!("\r\n{marker}");
        let (_, tail) = log
            .split_once(&needle)
            .context("native config checksum absent")?;
        let digest = tail.get(..64).context("short config checksum")?;
        ensure!(
            digest.bytes().all(|b| b.is_ascii_hexdigit()),
            "invalid guest checksum"
        );
        checksums.push(digest);
    }
    ensure!(
        checksums.iter().all(|hash| *hash == checksums[0]),
        "restored configuration differs across restart"
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        args.len() == 4,
        "usage: s9_boot_storage_cycle BUNDLE BACKEND NEW_OUTPUT"
    );
    let out = Path::new(&args[3]);
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(out)?;
    let mut entropy = [0; 32];
    fs::File::open("/dev/urandom")?.read_exact(&mut entropy)?;
    let token = hex::encode(entropy);
    let value = 101 + u32::from_le_bytes(entropy[..4].try_into().unwrap()) % 900;
    save_json(
        &out.join("fresh-input.json"),
        &json!({"token":token,"max_connections":value}),
    )?;
    let paths: Vec<_> = scripts(&token, value)
        .iter()
        .enumerate()
        .map(|(index, script)| {
            let path = out.join(format!("console-{index}.sh"));
            fs::write(&path, script)?;
            Ok(path)
        })
        .collect::<Result<_>>()?;
    let runner = std::env::current_exe()?
        .parent()
        .context("runner directory absent")?
        .join("s9_boot_bundle");
    let runner_id = hash_file(&runner)?;
    let mut argv = vec![
        "boot".into(),
        args[1].clone(),
        out.join("run").into_os_string(),
        "spl".into(),
        "sd2022-v1".into(),
        format!("backend={}", Path::new(&args[2]).display()).into(),
        "budget=360".into(),
        "save-sd".into(),
        format!("console={}", paths[0].display()).into(),
    ];
    for script in &paths[1..] {
        argv.push(format!("restart-console={}", script.display()).into());
    }
    save_json(
        &out.join("driver-input.json"),
        &json!({"runner":runner_id,"argv":argv,
        "scripts":paths.iter().map(|path|hash_file(path)).collect::<Result<Vec<_>>>()?,
        "scope":"Original boot path with unchanged archived SD backing and native UCI. New emulator per phase. Reference NAND resets; only SD persistence is tested."}),
    )?;
    let status = Command::new(&runner)
        .args(&argv)
        .stdin(Stdio::null())
        .stdout(fs::File::create_new(out.join("runner.stdout"))?)
        .stderr(fs::File::create_new(out.join("runner.stderr"))?)
        .status()?;
    ensure!(
        hash_file(&runner)? == runner_id,
        "runner changed during test"
    );
    let run = out.join("run");
    let log = fs::read_to_string(run.join("guest.log"))?;
    let checked = assess(&log, value);
    let result: Value = serde_json::from_slice(&fs::read(run.join("result.json"))?)?;
    ensure!(
        hex::decode(
            result["stdout"]
                .as_str()
                .context("execution stdout absent")?
        )? == log.as_bytes(),
        "retained guest log differs from execution result"
    );
    let storage: Value = serde_json::from_slice(&fs::read(run.join("sd-storage.json"))?)?;
    let snapshots = storage["snapshots"]
        .as_array()
        .context("snapshot reports absent")?;
    let passed = status.success()
        && checked.is_ok()
        && result["status"] == "exited"
        && result["exit_code"] == 0
        && result["trace_truncated"] == false
        && snapshots.len() == 3;
    save_json(
        &out.join("assessment.json"),
        &json!({"passed":passed,"error":checked.err().map(|error|error.to_string()),
        "driver_exit_code":status.code(),"execution_result":hash_file(&run.join("result.json"))?,
        "guest_log":hash_file(&run.join("guest.log"))?,"input":hash_file(&out.join("fresh-input.json"))?,
        "storage_manifest":hash_file(&run.join("sd-storage.json"))?,
        "scope":"Actual original SPL, U-Boot and kernel ran three times. Native UCI writes and reads a fresh value, restores exact original file bytes, and checks restoration after another emulator start. No full-board persistence, NAND retention, update/recovery or calibrated physical-board claim."}),
    )?;
    ensure!(
        passed,
        "original boot SD cycle failed; see retained evidence"
    );
    println!(
        "{}",
        json!({"sd_restart_cycle":"passed","original_board_gate":"incomplete"})
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observations_require_actual_ordered_output_and_three_boots() {
        let boot = "U-Boot SPL 2016.03-g8728fee00a\nU-Boot 2016.03-gf30f5b605c\nLinux version 4.4.0-xilinx-g518c00cd977f-dirty\n";
        let checksum = "ab".repeat(32);
        let trace = format!("{boot}\r\nMD_SIM_SD_FIRST_VALUE:234\r\n\r\nMD_SIM_SD_ORIGINAL_CONFIG:{checksum}  before\r\nMD_SIM_SD_FIRST_COMMITTED\r\n{boot}\r\nMD_SIM_SD_RESTART_VALUE:234\r\n\r\nMD_SIM_SD_RESTORED_CONFIG:{checksum}  current\r\nMD_SIM_SD_SECOND_RESTORED\r\n{boot}\r\nMD_SIM_SD_FINAL_VALUE:100\r\n\r\nMD_SIM_SD_FINAL_CONFIG:{checksum}  current\r\nMD_SIM_SD_THIRD_VERIFIED\r\n");
        assess(&trace, 234).unwrap();
        assert!(assess(&trace, 235).is_err());
        assert!(assess(&trace.replacen(boot, "", 1), 234).is_err());
        assert!(assess(&trace.replacen(&checksum, &"cd".repeat(32), 1), 234).is_err());
        assert!(assess(&format!("{trace}\r\nMD_SIM_SD_THIRD_VERIFIED\r\n"), 234).is_err());
    }
}

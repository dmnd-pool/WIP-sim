//! Three original firmware boots, native settings persistence, and fresh mining
//! verification for each boot. This remains an adapted reference environment.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::save_json,
    store::{hash_file, ObjectId},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
#[path = "support/s9_cycle_update.rs"]
mod cycle_update;
use md_sim_firmware::s9_dashboard_state as state;
use md_sim_firmware::s9_update_validation as update;

#[derive(Default)]
struct Options {
    update_monitor: Option<PathBuf>,
    browser: Option<PathBuf>,
    configuration: bool,
}
impl Options {
    fn parse(args: &[OsString]) -> Result<Self> {
        let mut options = Self::default();
        for arg in args {
            let value = arg.to_str().context("cycle option encoding")?;
            if value == "configuration-contract" {
                ensure!(!options.configuration, "duplicate configuration contract");
                options.configuration = true;
            } else if let Some(path) = value.strip_prefix("signed-update=") {
                ensure!(
                    !path.is_empty() && options.update_monitor.is_none(),
                    "duplicate or empty update source"
                );
                options.update_monitor = Some(path.into());
            } else if let Some(path) = value.strip_prefix("browser-payload=") {
                ensure!(
                    !path.is_empty() && options.browser.is_none(),
                    "duplicate or empty browser payload"
                );
                options.browser = Some(path.into());
            } else {
                anyhow::bail!("unknown cycle option: {value}");
            }
        }
        ensure!(
            options.update_monitor.is_none() || options.browser.is_some(),
            "signed update cycle requires the native browser phase"
        );
        Ok(options)
    }
}

#[path = "support/s9_cycle_evidence.rs"]
mod evidence;
use evidence::{
    boot_evidence, command, inspect_disk, read_json, single_record, verify_inputs, verify_mining,
};
#[path = "support/s9_cycle_console.rs"]
mod cycle_console;
use cycle_console::CONSOLE;
const CORE: &str = "work/simulator/bitcoin-core/source-build-002/bitcoind";

struct BootPolicy {
    wall_epoch: u64,
    configuration: bool,
}

fn console_scripts(
    out: &Path,
    mac: &str,
    update: Option<&cycle_update::Prepared>,
) -> Result<[PathBuf; 2]> {
    let parent = fs::read_to_string(CONSOLE)?;
    let check = format!(
        "[ \"$(cat /sys/class/net/eth0/address)\" = '{mac}' ]\n\
         [ \"$(cat /tmp/mnt/boot/uEnv.txt)\" = 'ethaddr={mac}' ]\n\
         printf '\\nMD_SIM_STABLE_MAC:{mac}\\n'\n"
    );
    ensure!(
        parent.matches("set -eu\n").count() == 1,
        "console template changed"
    );
    let mut initial = parent.replacen("set -eu\n", &format!("set -eu\n{check}"), 1);
    initial = cycle_console::persisted_pool_console(&initial)?;
    let mut resume = initial.clone();
    let (before, after) =
        ("uci set network.lan.proto=static\nuci set network.lan.ipaddr=10.0.2.15\nuci set network.lan.netmask=255.255.255.0\nuci -q delete network.lan.gateway || true\nuci -q delete network.lan.dns || true\nuci commit network\n/etc/init.d/network restart\n",
         "[ \"$(uci get network.lan.proto)\" = static ]\n[ \"$(uci get network.lan.ipaddr)\" = 10.0.2.15 ]\n[ \"$(uci get network.lan.netmask)\" = 255.255.255.0 ]\n! uci -q get network.lan.gateway\n! uci -q get network.lan.dns\n");
    ensure!(
        resume.matches(before).count() == 1,
        "console template changed; review restart adaptation"
    );
    resume = resume.replacen(before, after, 1);
    if let Some(update) = update {
        initial = initial.replacen("set -eu\n", &format!("set -eu\n{}", update.initial), 1);
        resume = resume.replacen("set -eu\n", &format!("set -eu\n{}", update.resumed), 1);
    }
    let paths = [
        out.join("console-initial.sh"),
        out.join("console-resume.sh"),
    ];
    fs::write(&paths[0], initial)?;
    fs::write(&paths[1], resume)?;
    save_json(
        &out.join("console-adaptation.json"),
        &json!({"parent":hash_file(Path::new(CONSOLE))?,
        "initial":hash_file(&paths[0])?,"resume":hash_file(&paths[1])?,
        "changes":["Both phases require the provisioned MAC and unchanged native boot environment.",
            "All test phases reuse the exact private mining configuration from provisioning.",
            "Require the exact saved mining configuration; do not overwrite it.",
            "The initial phase writes static network settings and restarts the native network service after native preinit applies the provisioned MAC.",
            "Only resumed phases require saved static network settings without reconfiguration or network restart."],
        "update_configuration":update.map(|prepared|&prepared.manifest),
        "console_changes_firmware_executables":false,"separate_monitor_adaptation":update.is_some()}),
    )?;
    Ok(paths)
}

/// The native preinit deletes network configuration when MAC or uEnv changes.
/// Provision first, then let a new native boot apply its normal cleanup before
/// the actual configuration test. Do not edit .last_mac or .last_uenv records.
fn provision_mac(
    bundle: &Path,
    backend: &Path,
    pool: &Path,
    bin: &Path,
    out: &Path,
    mac: &str,
    policy: &BootPolicy,
) -> Result<PathBuf> {
    let script = out.join("console-provision.sh");
    let content = format!("ethaddr={mac}\n");
    let sha = md_sim_firmware::prepare::hash_bytes(content.as_bytes());
    fs::write(
        &script,
        format!(
            "(\nset -eu\n[ -f /tmp/mnt/boot/uEnv.txt ]\n[ ! -s /tmp/mnt/boot/uEnv.txt ]\n\
         printf 'ethaddr={mac}\\n' > /tmp/mnt/boot/uEnv.txt\n\
         [ \"$(sha256sum /tmp/mnt/boot/uEnv.txt | cut -d ' ' -f 1)\" = '{sha}' ]\n\
         {}\n\
         attempts=0\n\
         until logread | grep -F 'procd: - init complete -'; do\n\
           attempts=$((attempts + 1))\n\
           [ \"$attempts\" -le 120 ]\n\
           sleep 1\n\
         done\n\
         [ \"$(sha256sum /tmp/mnt/boot/uEnv.txt | cut -d ' ' -f 1)\" = '{sha}' ]\n\
         printf '\\nMD_SIM_PROVISIONED_UENV:{sha}\\n'\nsync\n)\n",
            fs::read_to_string(CONSOLE)?
        ),
    )?;
    save_json(
        &out.join("network-adaptation.json"),
        &json!({
        "kind":"Native SD boot environment configuration",
        "mac":mac,"selection":"Fresh locally administered unicast address; not a physical device identity",
        "path":"/tmp/mnt/boot/uEnv.txt","parent":{"sha256":md_sim_firmware::prepare::hash_bytes(b""),"size":0},
        "derived":{"sha256":sha,"size":content.len()},"content":content,
        "script":hash_file(&script)?,"firmware_executable_bytes_changed":false,
        "mining_configuration":"Use the same explicit private pool, fixed frequency and disabled autotuning as the test phases. The archived native miner starts and performs work before shutdown. All later phases require its saved configuration without replacing it.",
        "shutdown_readiness":"Wait for the native procd init-complete log, at most120 guest one-second polling intervals. Recheck uEnv before requesting native reboot. No startup service is invoked or replaced.",
        "reason":"Blank NAND has no factory MAC. Native preinit otherwise selects a new MAC and removes saved network configuration on every boot.",
        "scope":"Explicit adapted environment. Native bootloader imports ethaddr. Native preinit records the new identity before the tested configuration is written."}),
    )?;
    let run = out.join("provision");
    let mut argv = vec![
        "boot".into(),
        bundle.as_os_str().into(),
        run.as_os_str().into(),
        "spl".into(),
        "sd2022-v1".into(),
        format!("backend={}", backend.display()).into(),
        "budget=360".into(),
        "presence-pin-fixture=e0".into(),
        "fan-fixture".into(),
        "pic-contract".into(),
        "asic-contract".into(),
        "asic-packets".into(),
        "temperature-fixture".into(),
        "local-pool".into(),
        format!("pool-vm={}", pool.display()).into(),
        "adapt-fixed-filter-bits=4".into(),
        "hash-cap-hps=1000".into(),
        "pool-difficulty=-28,-27".into(),
        "regtest-genesis".into(),
        format!("adapt-clock-epoch={}", policy.wall_epoch).into(),
        "save-sd".into(),
        "sd-native-reboot".into(),
        "sd-read-only-checkpoint".into(),
        format!("console={}", script.display()).into(),
    ];
    if policy.configuration {
        argv.push("configuration-contract".into());
    }
    command(
        &bin.join("s9_boot_bundle"),
        &argv,
        &out.join("provision-boot"),
    )?;
    let guest = boot_evidence(&run, None, true, false)?;
    ensure!(
        guest
            .matches(&format!("\nMD_SIM_PROVISIONED_UENV:{sha}\r\n"))
            .count()
            == 1,
        "native provisioning did not verify the boot environment"
    );
    Ok(run.join("sd-snapshot-0.json"))
}

fn retained_record(run: &Path, guest: &str, file: &str, marker: &str) -> Result<Value> {
    let report = read_json(&run.join(file), 4 * 1024 * 1024)?;
    ensure!(
        report["process_specification"]
            == serde_json::to_value(hash_file(&run.join("process-spec.json"))?)?
            && report["execution_result"]
                == serde_json::to_value(hash_file(&run.join("result.json"))?)?,
        "service report identity differs"
    );
    let records = report["observations"]
        .as_array()
        .context("service observations absent")?;
    ensure!(
        records.len() == 1 && records[0]["passed"] == true && records[0]["error"].is_null(),
        "native service test failed"
    );
    ensure!(
        records[0] == single_record(guest, marker, 4 * 1024 * 1024)?,
        "service report differs from actual guest output"
    );
    Ok(records[0].clone())
}

fn dashboard(
    run: &Path,
    expected_phase: &str,
    previous: Option<&Path>,
    mac: &str,
    installed: bool,
) -> Result<Value> {
    let guest = boot_evidence(run, previous, !installed, false)?;
    ensure!(
        guest
            .matches(&format!("\nMD_SIM_STABLE_MAC:{mac}\r\n"))
            .count()
            == 1,
        "guest did not verify stable native network identity"
    );
    let record = retained_record(
        run,
        &guest,
        "dashboard-result.json",
        "MD_SIM_DASHBOARD_PROBE ",
    )?;
    if installed {
        update::verify_completion(&record, update::Action::InstallKeep)?;
    }
    let configuration = &record["result"]["configuration"];
    ensure!(
        configuration["phase"] == expected_phase,
        "native configuration phase differs"
    );
    state::storage(&configuration["original"])?;
    Ok(configuration.clone())
}

fn cycle(
    bundle: &Path,
    backend: &Path,
    pool: &Path,
    out: &Path,
    options: &Options,
) -> Result<Value> {
    let bin = std::env::current_exe()?
        .parent()
        .context("driver path")?
        .to_owned();
    let names = [
        "s9_boot_bundle",
        "s9_pool_guest",
        "isolated_network_vm",
        "s9_asic_audit",
        "node_prerequisite_probe",
        "s9_sd_snapshot_inspect",
        "s9_update_bundle",
        "s9_browser_check",
    ];
    let programs: BTreeMap<_, _> = names
        .into_iter()
        .map(|name| Ok((name, hash_file(&bin.join(name))?)))
        .collect::<Result<_>>()?;
    let mut frozen_inputs: BTreeMap<PathBuf, ObjectId> = programs
        .iter()
        .map(|(name, object)| (bin.join(name), object.clone()))
        .collect();
    for path in [
        bundle.join("boot/bundle.json"),
        backend.join("build-envelope.json"),
        backend.join("build-report.json"),
        backend.join("qemu-system-arm"),
        pool.join("build-envelope.json"),
        pool.join("build-report.json"),
        pool.join("qemu-system-x86_64"),
        PathBuf::from(CORE),
        PathBuf::from(CONSOLE),
    ] {
        frozen_inputs.insert(path.clone(), hash_file(&path)?);
    }
    for path in options.update_monitor.iter().chain(options.browser.iter()) {
        frozen_inputs.insert(path.clone(), hash_file(path)?);
    }
    let prepared_update = if let Some(monitor) = &options.update_monitor {
        let inspection = out.join("signed-update-inspection");
        command(
            &bin.join("s9_update_bundle"),
            &[
                bundle.join("boot/bundle.json").into_os_string(),
                inspection.clone().into_os_string(),
            ],
            &out.join("inspect-signed-update"),
        )?;
        frozen_inputs.insert(
            PathBuf::from(update::ARCHIVE_IMAGE),
            update::checked_image(Path::new(update::ARCHIVE_IMAGE))?,
        );
        Some(cycle_update::prepare(
            monitor,
            &read_json(&inspection.join("inspection.json"), 1024 * 1024)?,
            &bundle.join("boot/bundle.json"),
            &out.join("update-adaptation"),
        )?)
    } else {
        None
    };
    let mut random = [0u8; 7];
    fs::File::open("/dev/urandom")?.read_exact(&mut random)?;
    let theme = if random[0] & 1 == 0 { "DARK" } else { "LIGHT" };
    random[1] = (random[1] & 0xfe) | 2;
    let mac = random[1..]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(":");
    let consoles = console_scripts(out, &mac, prepared_update.as_ref())?;
    for path in &consoles {
        frozen_inputs.insert(path.clone(), hash_file(path)?);
    }
    let wall_epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let boot_policy = BootPolicy {
        wall_epoch,
        configuration: options.configuration,
    };
    save_json(
        &out.join("driver-input.json"),
        &json!({"schema":"md-sim-s9-native-dashboard-cycle/5","programs":programs,"driver":hash_file(&std::env::current_exe()?)?,
        "driver_source":md_sim_firmware::prepare::hash_bytes(include_bytes!("s9_dashboard_cycle.rs")),
        "bundle":bundle,"backend":backend,"pool_backend":pool,"core":hash_file(Path::new(CORE))?,
        "console":hash_file(Path::new(CONSOLE))?,"theme":theme,"mac":mac,"frozen_inputs":frozen_inputs,
        "hash_cap_hps":1000,"wall_clock_epoch":wall_epoch,"browser_payload":options.browser,
        "configuration_contract":options.configuration,
        "signed_update":prepared_update.as_ref().map(|prepared|&prepared.manifest),
        "storage_policy":"Provisioning and normal test boots request an explicit native read-only remount before reboot, then require strict raw filesystem checks. The signed install phase uses its own restart and an explicitly journal-replayed inspection copy with no further repair. The untouched saved snapshot is the next firmware input. Unassisted shutdown and power-loss recovery remain separate open gates.",
        "scope":"One native SD network and private mining provisioning boot, then three new emulator processes and private client VMs. All four boots require native restart, independent disk inspection, fresh firmware mining, independent replay and two Core node tests. Optional signed installation and native browser are explicit phases. Exact physical target remains unresolved."}),
    )?;
    verify_inputs(&frozen_inputs)?;
    let mut original = Value::Null;
    let mut changed = Value::Null;
    let mut previous = Some(provision_mac(
        bundle,
        backend,
        pool,
        &bin,
        out,
        &mac,
        &boot_policy,
    )?);
    let provisioning_configuration =
        configuration_evidence(&out.join("provision"), options.configuration)?;
    let provisioning_disk =
        inspect_disk(&bin, &out.join("provision"), out, "provision", false, false)?;
    let provisioning_mining = verify_mining(
        &bin,
        &out.join("provision"),
        out,
        "provision",
        Path::new(CORE),
    )?;
    let mut phases = Vec::new();
    for (index, phase) in ["write", "restore", "verify"].into_iter().enumerate() {
        verify_inputs(&frozen_inputs)?;
        let action = match index {
            0 => state::Action::Write {
                theme: theme.into(),
            },
            1 => state::Action::Restore {
                original: original.clone(),
                changed: changed.clone(),
            },
            _ => state::Action::Verify {
                original: original.clone(),
            },
        };
        let plan = state::Plan { version: 1, action };
        state::parse(&serde_json::to_vec(&plan)?)?;
        let plan_path = out.join(format!("plan-{index}.json"));
        save_json(&plan_path, &plan)?;
        let run = out.join(format!("run-{index}"));
        let mut argv: Vec<OsString> = vec![
            "boot".into(),
            bundle.as_os_str().into(),
            run.as_os_str().into(),
            "spl".into(),
            "sd2022-v1".into(),
            format!("backend={}", backend.display()).into(),
            "budget=360".into(),
            "presence-pin-fixture=e0".into(),
            "fan-fixture".into(),
            "pic-contract".into(),
            "asic-contract".into(),
            "asic-packets".into(),
            "local-pool".into(),
            format!("pool-vm={}", pool.display()).into(),
            "temperature-fixture".into(),
            "adapt-fixed-filter-bits=4".into(),
            "hash-cap-hps=1000".into(),
            "pool-difficulty=-28,-27".into(),
            "regtest-genesis".into(),
            "save-sd".into(),
            "dashboard-probe".into(),
            format!("dashboard-state={}", plan_path.display()).into(),
            format!("console={}", consoles[usize::from(index != 0)].display()).into(),
            format!("adapt-clock-epoch={wall_epoch}").into(),
        ];
        if options.configuration {
            argv.push("configuration-contract".into());
        }
        let installs = index == 0 && prepared_update.is_some();
        if installs {
            argv.extend([
                "dashboard-luci".into(),
                "dashboard-update-install".into(),
                format!(
                    "adapt-monitor-fan-source={}",
                    options.update_monitor.as_ref().unwrap().display()
                )
                .into(),
            ]);
        } else {
            argv.push("sd-native-reboot".into());
            argv.push("sd-read-only-checkpoint".into());
        }
        let browser = if index == 1 {
            options.browser.as_ref()
        } else {
            None
        };
        if let Some(payload) = browser {
            argv.push(format!("browser-payload={}", payload.display()).into());
        }
        if let Some(path) = &previous {
            argv.push(format!("sd-state={}", path.display()).into());
        }
        command(
            &bin.join("s9_boot_bundle"),
            &argv,
            &out.join(format!("boot-{index}")),
        )?;
        let configuration = dashboard(&run, phase, previous.as_deref(), &mac, installs)?;
        let configuration_transfer = configuration_evidence(&run, options.configuration)?;
        let browser_evidence = if browser.is_some() {
            let guest = boot_evidence(&run, previous.as_deref(), true, false)?;
            let record = retained_record(
                &run,
                &guest,
                "browser-result.json",
                "MD_SIM_BOSER_BROWSER_RESULT ",
            )?;
            ensure!(
                record["origin"] == "http://10.0.2.15" && record["isolation"]["passed"] == true,
                "browser did not check this native origin and isolated profile"
            );
            Some(hash_file(&run.join("browser-result.json"))?)
        } else {
            None
        };
        let disk = inspect_disk(&bin, &run, out, &index.to_string(), installs, false)?;
        if index == 0 {
            original = configuration["original"].clone();
            changed = configuration["changed"].clone();
        } else {
            ensure!(
                configuration["original"] == original,
                "original settings changed between phases"
            );
        }
        let mining = verify_mining(&bin, &run, out, &index.to_string(), Path::new(CORE))?;
        previous = Some(run.join("sd-snapshot-0.json"));
        phases.push(json!({"phase":phase,"configuration":configuration,
            "process_specification":hash_file(&run.join("process-spec.json"))?,"execution_result":hash_file(&run.join("result.json"))?,
            "dashboard":hash_file(&run.join("dashboard-result.json"))?,"snapshot":hash_file(previous.as_ref().unwrap())?,
            "mining":mining,"disk_inspection":disk,
            "signed_installation":installs,"browser":browser_evidence,"configuration_transfer":configuration_transfer}));
        save_json(
            &out.join("progress.json"),
            &json!({"completed_phases":phases,"required_phases":3}),
        )?;
    }
    verify_inputs(&frozen_inputs)?;
    Ok(
        json!({"phases":phases,"provisioning_disk":provisioning_disk,"provisioning_mining":provisioning_mining,"provisioning_configuration":provisioning_configuration,"original_board_gate":"incomplete","full_native_dashboard_gate":"incomplete",
        "scope":"Native settings write, native restart, readback, restoration and second restart readback. Signed update and browser evidence appear only when selected. Every phase has independent disk inspection, fresh mining and two Core-accepted blocks. Update faults/recovery, remote publication and exact hardware gates remain separate."}),
    )
}

fn configuration_evidence(run: &Path, selected: bool) -> Result<Option<ObjectId>> {
    if !selected {
        return Ok(None);
    }
    let path = run.join("configuration-audit.json");
    let report = read_json(&path, 1024 * 1024)?;
    ensure!(
        report["result"]["passed"] == true
            && report["execution"] == serde_json::to_value(hash_file(&run.join("result.json"))?)?
            && report["backend_log"] == serde_json::to_value(hash_file(&run.join("backend.log"))?)?
            && report["container"]
                == serde_json::to_value(hash_file(&run.join("configuration-reference.bit"))?)?,
        "configuration audit is missing, failed or belongs to a different execution"
    );
    Ok(Some(hash_file(&path)?))
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    ensure!(
        (5..=8).contains(&args.len()),
        "usage: s9_dashboard_cycle BUNDLE BACKEND POOL_BACKEND NEW_OUTPUT [signed-update=ORIGINAL_MONITOR_SOURCE browser-payload=MANIFEST] [configuration-contract]"
    );
    let options = Options::parse(&args[5..])?;
    let out = Path::new(&args[4]);
    ensure!(!out.exists(), "new cycle output required");
    fs::create_dir_all(out)?;
    let executable = std::env::current_exe()?;
    fs::copy(&executable, out.join("driver-binary"))?;
    ensure!(
        hash_file(&executable)? == hash_file(&out.join("driver-binary"))?,
        "driver capture differs"
    );
    let result = cycle(
        Path::new(&args[1]),
        Path::new(&args[2]),
        Path::new(&args[3]),
        out,
        &options,
    );
    save_json(
        &out.join("assessment.json"),
        &json!({"passed":result.is_ok(),"result":result.as_ref().ok(),"error":result.as_ref().err().map(|e|format!("{e:#}"))}),
    )?;
    result.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn update_cycle_requires_explicit_browser_and_unique_inputs() {
        let parse =
            |args: &[&str]| Options::parse(&args.iter().map(OsString::from).collect::<Vec<_>>());
        assert!(parse(&[]).is_ok());
        assert!(parse(&["signed-update=monitor", "browser-payload=browser"]).is_ok());
        assert!(parse(&["configuration-contract"]).unwrap().configuration);
        for args in [
            vec!["signed-update=monitor"],
            vec!["browser-payload="],
            vec!["signed-update=a", "signed-update=b", "browser-payload=c"],
            vec!["browser-payload=a", "browser-payload=b"],
            vec!["unexpected=flag"],
            vec!["configuration-contract", "configuration-contract"],
        ] {
            assert!(parse(&args).is_err());
        }
    }

    #[test]
    fn configuration_result_cannot_be_reused_after_execution_or_input_changes() {
        let directory = tempfile::tempdir().unwrap();
        let run = directory.path();
        assert_eq!(configuration_evidence(run, false).unwrap(), None);
        assert!(configuration_evidence(run, true).is_err());
        for name in ["result.json", "backend.log", "configuration-reference.bit"] {
            fs::write(run.join(name), b"test evidence").unwrap();
        }
        let mut report = json!({"result":{"passed":true},
            "execution":hash_file(&run.join("result.json")).unwrap(),
            "backend_log":hash_file(&run.join("backend.log")).unwrap(),
            "container":hash_file(&run.join("configuration-reference.bit")).unwrap()});
        let path = run.join("configuration-audit.json");
        save_json(&path, &report).unwrap();
        assert!(configuration_evidence(run, true).unwrap().is_some());
        for name in ["result.json", "backend.log", "configuration-reference.bit"] {
            fs::write(run.join(name), b"changed evidence").unwrap();
            assert!(configuration_evidence(run, true).is_err());
            fs::write(run.join(name), b"test evidence").unwrap();
        }
        report["result"]["passed"] = json!(false);
        save_json(&path, &report).unwrap();
        assert!(configuration_evidence(run, true).is_err());
    }
}

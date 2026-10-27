//! Bounded feasibility probe for original archived S9 Linux components.
//! A generic machine and a direct kernel entry cannot pass original board boot.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, Store},
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};

const ARTIFACT: &str = "braiins-feed-25bba3b4a505ccbc0e54";
const ARCHIVE_HASH: &str = "469743ebcd5faf3445e4f4209ad8af58cc7adeff37c7df28af3c53a5b255b56e";
const FIT_HASH: &str = "8b2a32020281aa67490c01633adf99c1a2ca6c9897e4c210bb6d0d001c15caca";
const COMPONENTS: &[(&str, usize, usize, &str)] = &[
    (
        "kernel",
        228,
        2827776,
        "f85eba975b58b4333e99a656ad661667cbedaf3111af3deb8572b902b88cd276",
    ),
    (
        "ramdisk",
        2828304,
        15491980,
        "2bd49c70b915b7e25f6c01013d95a5ffea8d499b73c6c6ec5539fda4247154b3",
    ),
    (
        "board.dtb",
        18320556,
        12364,
        "550c87818cb0db41c00a8351dc4ac7b5dce5bf6d5336539c6951163ebf3cb62a",
    ),
];

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn extract_inside() -> Result<()> {
    ensure!(
        hash_file(Path::new("/source.tar"))?.sha256 == ARCHIVE_HASH,
        "archive hash differs"
    );
    let mut archive = tar::Archive::new(fs::File::open("/source.tar")?);
    let mut fit = None;
    for (count, entry) in archive.entries()?.enumerate() {
        ensure!(count < 100, "unexpected archive member count");
        let mut entry = entry?;
        if entry.path()? == Path::new("sysupgrade-am1-s9/fit") {
            ensure!(
                fit.is_none() && entry.header().entry_type().is_file() && entry.size() == 18333708,
                "unexpected FIT member identity"
            );
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            ensure!(sha(&bytes) == FIT_HASH, "FIT parent hash differs");
            fit = Some(bytes);
        }
    }
    let fit = fit.context("archive lacks selected FIT")?;
    let mut output = std::io::stdout().lock();
    for (name, offset, size, hash) in COMPONENTS {
        let bytes = fit
            .get(*offset..offset + size)
            .context("component exceeds verified FIT")?;
        ensure!(sha(bytes) == *hash, "component hash differs: {name}");
        output.write_all(bytes)?;
    }
    Ok(())
}

fn add_input(store: &Store, inputs: &mut Vec<Mount>, path: &Path, guest: &str) -> Result<()> {
    let object = hash_file(path)?;
    store.import(path, &object)?;
    inputs.push(Mount {
        object,
        guest_path: guest.into(),
    });
    Ok(())
}

fn reference_source_inside() -> Result<()> {
    let mut references = BTreeMap::new();
    for (path, expected, compressed, selected) in [
        (
            "/qemu.tar.gz",
            "2a40e6705e59fd8807012631bc6b2c0877f10481b12768f78e4f9d7b88376cce",
            true,
            vec!["/hw/arm/xilinx_zynq.c", "/hw/arm/boot.c", "/VERSION"],
        ),
        (
            "/kernel.tar.gz",
            "5aa5c0915c93c6078984b13367edd12670e8ad25d922d352654f768d7005db8f",
            true,
            vec!["/drivers/i2c/busses/i2c-xiic.c"],
        ),
        ("/source.tar", ARCHIVE_HASH, false, vec!["/uenv"]),
    ] {
        ensure!(
            hash_file(Path::new(path))?.sha256 == expected,
            "reference archive differs"
        );
        let file = fs::File::open(path)?;
        let reader: Box<dyn Read> = if compressed {
            Box::new(flate2::read::GzDecoder::new(file))
        } else {
            Box::new(file)
        };
        let mut archive = tar::Archive::new(reader.take(2 * 1024 * 1024 * 1024));
        let mut found = BTreeMap::new();
        for (index, entry) in archive.entries()?.enumerate() {
            ensure!(index < 100_000, "reference archive member limit exceeded");
            let mut entry = entry?;
            let name = entry.path()?.to_string_lossy().into_owned();
            let relative = name.split_once('/').map(|(_, rest)| rest);
            let Some(suffix) = selected
                .iter()
                .find(|suffix| Some(suffix.trim_start_matches('/')) == relative)
            else {
                continue;
            };
            ensure!(
                entry.header().entry_type().is_file() && entry.size() <= 1024 * 1024,
                "reference member is not bounded regular text"
            );
            ensure!(!found.contains_key(*suffix), "duplicate reference member");
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            found.insert(*suffix, json!({"archive_member":name,"sha256":sha(&bytes),"text":String::from_utf8(bytes)?}));
        }
        ensure!(
            found.len() == selected.len() || path == "/source.tar",
            "required reference member missing from {path}"
        );
        let missing: Vec<_> = selected
            .iter()
            .filter(|suffix| !found.contains_key(**suffix))
            .collect();
        references.insert(path, json!({"archive_sha256":expected,"members":found,"missing_members":missing,"archive_fully_scanned":true}));
    }
    serde_json::to_writer(std::io::stdout(), &references)?;
    Ok(())
}

fn save_reference_source(repo: &Path, out: &Path) -> Result<()> {
    let store = Store::open(&out.join("inputs"), 512 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![("/probe".into(), std::env::current_exe()?)],
    )?;
    for (relative, guest) in [
        ("artifacts/files/source/tool-qemu/qemu-11661d857254.tar.gz", "/qemu.tar.gz"),
        ("artifacts/files/source/bitmain-source-kernel-soc/master", "/kernel.tar.gz"),
        ("artifacts/files/firmware/braiins-feed-25bba3b4a505ccbc0e54/sd-firmware_2022-04-29-0-5eb2c5fe-22.02.4-plus_arm_cortex-a9_neon.tar", "/source.tar"),
    ] {
        add_input(&store, &mut inputs, &repo.join(relative), guest)?;
    }
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        stdin_object: None,
        schema_version: "2.0".into(),
        argv: vec!["/probe".into(), "__references".into()],
        inputs,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 45,
            cpu_seconds: 35,
            memory_bytes: 512 * 1024 * 1024,
            processes: 32,
            file_bytes: 8 * 1024 * 1024,
            trace_bytes: 8 * 1024 * 1024,
            tmp_bytes: 16 * 1024 * 1024,
        },
    };
    save_json(&out.join("reference-spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("reference-result.json"), &result)?;
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated,
        "reference extraction failed"
    );
    let value: serde_json::Value = serde_json::from_slice(&hex::decode(&result.stdout)?)?;
    save_json(&out.join("references.json"), &value)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|s| s == "__extract") {
        return extract_inside();
    }
    if args.get(1).is_some_and(|s| s == "__references") {
        return reference_source_inside();
    }
    ensure!(
        args.len() == 3
            || (args.len() == 4
                && matches!(
                    args[3].to_str(),
                    Some("single-cpu" | "diagnostic" | "skip-xiic" | "ramdisk-root" | "ramdisk-trace" | "root-progress" | "references")
                )),
        "usage: original_zynq_probe REPOSITORY NEW_OUTPUT [single-cpu|diagnostic|skip-xiic|ramdisk-root|ramdisk-trace|root-progress|references]"
    );
    let repo = PathBuf::from(&args[1]);
    let out = PathBuf::from(&args[2]);
    ensure!(!out.exists(), "probe output must be new");
    fs::create_dir_all(&out)?;
    if args.get(3).is_some_and(|s| s == "references") {
        return save_reference_source(&repo, &out);
    }
    let archive=repo.join(format!("artifacts/files/firmware/{ARTIFACT}/sd-firmware_2022-04-29-0-5eb2c5fe-22.02.4-plus_arm_cortex-a9_neon.tar"));
    ensure!(
        hash_file(&archive)?.sha256 == ARCHIVE_HASH,
        "archive hash differs"
    );
    let store = Store::open(&out.join("inputs"), 512 * 1024 * 1024)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![("/probe".into(), std::env::current_exe()?)],
    )?;
    add_input(&store, &mut inputs, &archive, "/source.tar")?;
    let preparation = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        stdin_object: None,
        schema_version: "2.0".into(),
        argv: vec!["/probe".into(), "__extract".into()],
        inputs,
        environment: BTreeMap::new(),
        limits: Limits {
            wall_seconds: 20,
            cpu_seconds: 15,
            memory_bytes: 512 * 1024 * 1024,
            processes: 32,
            file_bytes: 32 * 1024 * 1024,
            trace_bytes: 64 * 1024 * 1024,
            tmp_bytes: 32 * 1024 * 1024,
        },
    };
    save_json(&out.join("preparation-spec.json"), &preparation)?;
    let prepared = execute(&store, &preparation, Arc::new(AtomicBool::new(false)))?;
    let exported = hex::decode(&prepared.stdout)?;
    let mut preparation_result = serde_json::to_value(&prepared)?;
    preparation_result["stdout"] = json!({
        "format": "concatenated kernel, ramdisk, board.dtb bytes; successful output retained as components",
        "size_bytes": exported.len(),
        "sha256": sha(&exported),
    });
    save_json(&out.join("preparation-result.json"), &preparation_result)?;
    ensure!(
        prepared.exit_code == Some(0) && !prepared.trace_truncated,
        "isolated preparation failed: status={}, exit={:?}, truncated={}: {}",
        prepared.status,
        prepared.exit_code,
        prepared.trace_truncated,
        String::from_utf8_lossy(&hex::decode(&prepared.stderr)?)
    );
    ensure!(
        exported.len() == COMPONENTS.iter().map(|(_, _, size, _)| size).sum::<usize>(),
        "export length differs"
    );
    let mut manifest = json!({"artifact_id":ARTIFACT,"archive_sha256":ARCHIVE_HASH,"fit_sha256":FIT_HASH,"components":{}});
    fs::create_dir(out.join("components"))?;
    let mut export_offset = 0;
    for (name, offset, size, hash) in COMPONENTS {
        let bytes = &exported[export_offset..export_offset + size];
        export_offset += size;
        ensure!(sha(bytes) == *hash, "export hash differs");
        fs::write(out.join("components").join(name), bytes)?;
        manifest["components"][*name] =
            json!({"sha256":hash,"size_bytes":size,"fit_offset":offset});
    }
    manifest["preparation_tools"] = serde_json::to_value(&prepared.tools)?;
    manifest["preparation_spec_hash"] = json!(sha(&serde_json::to_vec(&preparation)?));
    manifest["execution_limit"]=json!("Original component bytes; direct kernel entry on generic QEMU machine. No exact board identity, original reset path, bootloader, FPGA, NAND state, or mining pass.");
    save_json(&out.join("component-manifest.json"), &manifest)?;
    let mut inputs = md_sim_firmware::lab::elf_closure(
        &store,
        vec![(
            "/qemu-system-arm".into(),
            PathBuf::from("/usr/bin/qemu-system-arm"),
        )],
    )?;
    for (name, _, _, _) in COMPONENTS {
        add_input(
            &store,
            &mut inputs,
            &out.join("components").join(name),
            &format!("/{name}"),
        )?;
    }
    for (name, append, cpus) in [
        ("original-bootargs", "earlycon", "2"),
        (
            "adapted-console",
            "earlycon=cdns,mmio,0xe0001000 console=ttyPS0,115200n8",
            "2",
        ),
        (
            "adapted-console-single-cpu",
            "earlycon=cdns,mmio,0xe0001000 console=ttyPS0,115200n8",
            "1",
        ),
        (
            "adapted-initcall-diagnostic",
            "earlycon=cdns,mmio,0xe0001000 console=ttyPS0,115200n8 keep_bootcon ignore_loglevel loglevel=8 initcall_debug",
            "1",
        ),
        (
            "adapted-skip-unmodeled-xiic",
            "earlycon=cdns,mmio,0xe0001000 console=ttyPS0,115200n8 keep_bootcon ignore_loglevel loglevel=8 initcall_debug initcall_blacklist=xiic_i2c_driver_init",
            "1",
        ),
        (
            "adapted-ramdisk-root",
            "earlycon=cdns,mmio,0xe0001000 console=ttyPS0,115200n8 keep_bootcon ignore_loglevel loglevel=8 initcall_debug initcall_blacklist=xiic_i2c_driver_init root=/dev/ram0",
            "1",
        ),
        (
            "adapted-ramdisk-exception-trace",
            "earlycon=cdns,mmio,0xe0001000 console=ttyPS0,115200n8 ignore_loglevel loglevel=8 initcall_blacklist=xiic_i2c_driver_init root=/dev/ram0",
            "1",
        ),
        (
            "adapted-root-progress",
            "earlycon=cdns,mmio,0xe0001000 console=ttyPS0,115200n8 ignore_loglevel loglevel=8 initcall_debug initcall_blacklist=xiic_i2c_driver_init root=/dev/ram0",
            "1",
        ),
    ] {
        let selected = match args.get(3).and_then(|s| s.to_str()) {
            Some("single-cpu") => name == "adapted-console-single-cpu",
            Some("diagnostic") => name == "adapted-initcall-diagnostic",
            Some("skip-xiic") => name == "adapted-skip-unmodeled-xiic",
            Some("ramdisk-root") => name == "adapted-ramdisk-root",
            Some("ramdisk-trace") => name == "adapted-ramdisk-exception-trace",
            Some("root-progress") => name == "adapted-root-progress",
            None => matches!(name, "original-bootargs" | "adapted-console" | "adapted-console-single-cpu"),
            _ => false,
        };
        if !selected {
            continue;
        }
        let args = [
            "/qemu-system-arm",
            "-machine",
            "xilinx-zynq-a9",
            "-accel",
            "tcg,thread=single",
            "-m",
            "1024M",
            "-smp",
            cpus,
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
            "-no-reboot",
            "-kernel",
            "/kernel",
            "-initrd",
            "/ramdisk",
            "-dtb",
            "/board.dtb",
            "-append",
            append,
            "-sandbox",
            "on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny",
        ];
        let exception_trace = name == "adapted-ramdisk-exception-trace";
        let mut argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        if exception_trace {
            argv.extend(["-d".into(), "int,guest_errors,unimp".into()]);
        }
        let spec = ProcessSpec {
            support_virtualization: None,
            private_channel: None, stdin_stream: None,
            namespace_uid: 0,
            dashboard: None,
            loopback: false, stdin_object: None,
            schema_version: "2.0".into(),
            argv,
            inputs: inputs.clone(),
            environment: BTreeMap::new(),
            limits: Limits {
                wall_seconds: if exception_trace { 15 } else if name == "adapted-root-progress" { 60 } else { 30 },
                cpu_seconds: if exception_trace { 10 } else if name == "adapted-root-progress" { 45 } else { 25 },
                memory_bytes: 3 * 1024 * 1024 * 1024,
                processes: 64,
                file_bytes: 16 * 1024 * 1024,
                trace_bytes: 4 * 1024 * 1024,
                tmp_bytes: 32 * 1024 * 1024,
            },
        };
        save_json(&out.join(format!("{name}-spec.json")), &spec)?;
        save_json(
            &out.join(format!("{name}-adaptation.json")),
            &json!({
                "input_components_unchanged":true,
                "direct_kernel_loader":true,
                "machine":"generic QEMU xilinx-zynq-a9; not an exact S9 board",
                "cpu_count":cpus,
                "dtb_declared_cpu_count":2,
                "chosen_bootargs_override":append,
                "purpose":"bounded feasibility and init-call diagnosis",
                "disabled_driver_initialization":if append.contains("initcall_blacklist=") { Some("xiic_i2c_driver_init; FPGA I2C is not modeled; diagnostic bypass only") } else { None },
                "original_board_gate":"incomplete",
                "complete_firmware_boot":false
            }),
        )?;
        let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
        fs::write(
            out.join(format!("{name}-guest.log")),
            hex::decode(&result.stdout)?,
        )?;
        fs::write(
            out.join(format!("{name}-backend.log")),
            hex::decode(&result.stderr)?,
        )?;
        save_json(&out.join(format!("{name}-result.json")), &result)?;
        println!(
            "{}",
            json!({"probe":name,"status":result.status,"exit_code":result.exit_code,"wall_milliseconds":result.wall_milliseconds,"original_board_gate":"blocked","mining_gate":"not-run"})
        );
    }
    Ok(())
}

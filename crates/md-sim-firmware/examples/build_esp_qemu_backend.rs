//! Closed-input build of the archived Espressif QEMU fork.
//! No acquired tool executes on the host. A backend build is not a miner boot.
use anyhow::{ensure, Context, Result};
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, Store},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{atomic::AtomicBool, Arc},
};
#[allow(dead_code)]
#[path = "../src/confinement.rs"]
mod confinement;
#[path = "support/qemu_build.rs"]
mod qemu_build;
use qemu_build::{capture_overlay_with, pack_dir, reuse_overlay, step, unpack};
const SOURCE: &str = "5456034eb8a9f348b1f4c8820dc79f55677bddfe0d1c66566b6c42ceb9df66f0";
const PREFIX: &str = "qemu-febae182e132e4055529be423a818225ebddaa3a";
fn inner(sar: bool, i2c: bool, ds4432: bool, emc2101: bool, ina260: bool) -> Result<()> {
    confinement::require_board()?;
    ensure!(
        hash_file(Path::new("/qemu.tar.gz"))?.sha256 == SOURCE,
        "QEMU source differs"
    );
    unpack("/toolchain.tar.gz", "/work")?;
    unpack("/overlay.tar.gz", "/work")?;
    unpack("/qemu.tar.gz", "/work")?;
    let src = Path::new("/work").join(PREFIX);
    ensure!(
        fs::read_to_string(src.join("subprojects/keycodemapdb.wrap"))?
            .contains("revision = f5772a62ec52591ff6870b7e8ef32482371f22c6"),
        "keycode provider revision differs from archived fork"
    );
    let build = Path::new("/work/build");
    fs::create_dir_all(build)?;
    unpack(
        "/keycodes.tar.gz",
        src.join("subprojects").to_str().unwrap(),
    )?;
    let mut report = json!({"source":SOURCE,"source_commit":"febae182e132e4055529be423a818225ebddaa3a","version":"9.2.2","limitations":["Provider-derived Samurai and keycodemapdb normalization is recorded separately.","Build success is not firmware execution or exact board support.","The upstream OpenETH interface is virtual; it is not the physical ESP32 Wi-Fi interface."]});
    report["build_adaptation"] = json!({"kind":"diagnostic-build-only","patch":hash_file(Path::new("/diagnostic-fp-tests.patch"))?,"reason":"Missing archived Berkeley SoftFloat and TestFloat subprojects prevent generation of upstream FP test executables. Runtime CPU and floating-point implementations are unchanged.","upstream_fp_tests":"untested; required dependency and test gate remains incomplete"});
    let mut patch = Command::new("/work/toolchain/usr/bin/patch");
    patch.current_dir(&src).args([
        "-p1",
        "--batch",
        "--forward",
        "--input=/diagnostic-fp-tests.patch",
    ]);
    let mut success = step(&mut report, "diagnostic_build_patch", patch)?;
    if success && sar {
        fs::create_dir_all(src.join("mdsim"))?;
        fs::copy(
            "/libmd_sim_esp32s3_sar.a",
            src.join("mdsim/libmd_sim_esp32s3_sar.a"),
        )?;
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/rtc-sar.patch"]);
        success = step(&mut report, "rtc_sar_patch", patch)?;
        if success {
            let mut trace = fs::OpenOptions::new()
                .append(true)
                .open(src.join("hw/misc/trace-events"))?;
            trace.write_all(&fs::read("/rtc-sar-trace-events")?)?;
        }
        report["rtc_sar_model"] = json!({"library":hash_file(Path::new("/libmd_sim_esp32s3_sar.a"))?,"patch":hash_file(Path::new("/rtc-sar.patch"))?,"scope":"Development RTC single-conversion model. External inputs default disconnected; explicit adcN-channelM-uv properties supply independent initial voltages. Ground mode follows the programmed internal mux. Timing is an uncalibrated functional approximation.","incomplete":["Physical regulator and board-input coupling", "Physical ADC calibration and initial-code transfer", "RTC interrupt wiring", "Digital ADC and DMA", "RTC power-field encoding conflict with TRM wording"]});
    }
    if success && i2c {
        let mut reselect = Command::new("/work/toolchain/usr/bin/patch");
        reselect.current_dir(&src).args([
            "-p1",
            "--batch",
            "--forward",
            "--input=/i2c-reselect.patch",
        ]);
        success = step(&mut report, "i2c_reselect_patch", reselect)?;
    }
    if success && i2c {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/i2c-master.patch"]);
        success = step(&mut report, "i2c_master_patch", patch)?;
        if success {
            fs::OpenOptions::new()
                .append(true)
                .open(src.join("hw/misc/trace-events"))?
                .write_all(&fs::read("/i2c-trace-events")?)?;
        }
        report["i2c_master_model"] = json!({"library":hash_file(Path::new("/libmd_sim_esp32s3_sar.a"))?,"patch":hash_file(Path::new("/i2c-master.patch"))?,"reselect_patch":hash_file(Path::new("/i2c-reselect.patch"))?,"scope":"Development FIFO master with QEMU I2C bus. No default slave devices attached; missing devices must NACK. Explicit address reselect preserves repeated START versus STOP.","incomplete":["Slave/10bit/nonFIFO/LSB modes", "Electrical arbitration and clock stretching", "System clock/reset wiring", "Mid-transaction FIFO starvation recovery", "Physical support slaves"]});
    }
    if success && ds4432 {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/ds4432.patch"]);
        success = step(&mut report, "ds4432_patch", patch)?;
        if success {
            fs::OpenOptions::new()
                .append(true)
                .open(src.join("hw/misc/trace-events"))?
                .write_all(&fs::read("/ds4432-trace-events")?)?;
        }
        report["ds4432_model"] = json!({"library":hash_file(Path::new("/libmd_sim_esp32s3_sar.a"))?,"patch":hash_file(Path::new("/ds4432.patch"))?,"scope":"Explicit optional DS4432 register/current model. No default board attachment; no ASIC voltage telemetry.","incomplete":["Analog settling, leakage, thermal drift, tolerances", "Power-on pointer and undocumented transaction behavior", "Physical regulator connection and unresolved Max R14 value"]});
    }
    if success && emc2101 {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/emc2101.patch"]);
        success = step(&mut report, "emc2101_patch", patch)?;
        if success {
            fs::OpenOptions::new()
                .append(true)
                .open(src.join("hw/misc/trace-events"))?
                .write_all(&fs::read("/emc2101-trace-events")?)?;
        }
        report["emc2101_model"] = json!({"library":hash_file(Path::new("/libmd_sim_esp32s3_sar.a"))?,"patch":hash_file(Path::new("/emc2101.patch"))?,"scope":"Explicit optional EMC2101-R host-load model with real virtual conversion timing and sensor inputs. No default board attachment. Fan request is separate from measured tachometer edges.","incomplete":["Physical thermal/fan coupling and PWM waveform", "LUT, averaging and analog compensation modes", "EEPROM autoload, SMBus timeout and ARA", "Unspecified low-speed tachometer wrap"]});
    }
    if success && ina260 {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/ina260.patch"]);
        success = step(&mut report, "ina260_patch", patch)?;
        if success {
            fs::OpenOptions::new()
                .append(true)
                .open(src.join("hw/misc/trace-events"))?
                .write_all(&fs::read("/ina260-trace-events")?)?;
        }
        report["ina260_model"] = json!({"library":hash_file(Path::new("/libmd_sim_esp32s3_sar.a"))?,"patch":hash_file(Path::new("/ina260.patch"))?,"scope":"Explicit optional INA260 sensor at0x40. Real virtual conversion deadlines, configured averages and explicit VIN/shunt current. No inferred ASIC load or core-voltage telemetry.","incomplete":["Physical PSU/regulator/load coupling", "ADC integration/noise/thermal calibration", "SMBus ARA/SCL-low timeout/HS filters", "Dynamic straps and migration", "Undefined partial-word and power-overflow behavior is flagged"]});
    }
    if success {
        let mut config = Command::new("/bin/sh");
        config.current_dir(build).arg(src.join("configure")).args([
            "--target-list=xtensa-softmmu",
            "--without-default-features",
            "--disable-docs",
            "--disable-download",
            "--disable-werror",
            "--disable-tools",
            "-Dmdsim_build_fp_tests=false",
            "--enable-fdt",
            "--enable-pixman",
            "--enable-slirp",
            "--enable-gcrypt",
            "--enable-tcg",
            "--enable-trace-backends=log",
            "--ninja=/samu",
            "--python=/work/toolchain/usr/bin/python3",
            "--cc=/work/toolchain/usr/bin/gcc --sysroot=/work/toolchain -B/work/toolchain/usr/bin",
            "--cxx=/work/toolchain/usr/bin/g++ --sysroot=/work/toolchain -B/work/toolchain/usr/bin",
        ]);
        success = step(&mut report, "configure", config)?;
    }
    if success {
        let mut c = Command::new("/samu");
        c.current_dir(build).args(["-j", "2", "qemu-system-xtensa"]);
        success = step(&mut report, "build", c)?;
    }
    for n in ["config.log", "meson-logs/meson-log.txt"] {
        let p = build.join(n);
        if p.is_file() {
            let mut b = String::new();
            fs::File::open(p)?
                .take(2 * 1024 * 1024)
                .read_to_string(&mut b)?;
            report[n] = json!(b);
        }
    }
    if success {
        let binary = build.join("qemu-system-xtensa");
        let mut strip = Command::new("/work/toolchain/usr/bin/strip");
        strip.arg(&binary);
        success = step(&mut report, "strip", strip)?;
        if success {
            let b = fs::read(&binary)?;
            ensure!(b.len() < 28 * 1024 * 1024, "binary export budget");
            report["binary"] = serde_json::to_value(hash_file(&binary)?)?;
            let mut compressed =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            compressed.write_all(&b)?;
            let compressed = compressed.finish()?;
            ensure!(
                compressed.len() < 10 * 1024 * 1024,
                "compressed backend export budget"
            );
            report["binary_gzip"] = json!({"sha256":md_sim_firmware::prepare::hash_bytes(&compressed),"size":compressed.len()});
            report["binary_gzip_hex"] = json!(hex::encode(compressed));
        }
    }
    if success {
        let rom = src.join("pc-bios/esp32s3_rev0_rom.bin");
        let bytes = fs::read(&rom)?;
        ensure!(bytes.len() <= 512 * 1024, "ROM export budget");
        report["rom"] = serde_json::to_value(hash_file(&rom)?)?;
        report["rom_hex"] = json!(hex::encode(bytes));
        let mut probe = Command::new(build.join("qemu-system-xtensa"));
        probe.args(["-machine", "help"]);
        success = step(&mut report, "machine_list", probe)?;
        success &= report["machine_list"]["stdout"].as_str().is_some_and(|s| {
            s.lines()
                .any(|l| l.split_whitespace().next() == Some("esp32s3"))
        });
    }
    serde_json::to_writer(std::io::stdout(), &report)?;
    ensure!(success, "QEMU build failed");
    Ok(())
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|v| {
        v == "__inside"
            || v == "__inside-sar"
            || v == "__inside-i2c"
            || v == "__inside-ds4432"
            || v == "__inside-emc2101"
            || v == "__inside-ina260"
    }) {
        return inner(
            args[1] != "__inside",
            args[1] == "__inside-i2c"
                || args[1] == "__inside-ds4432"
                || args[1] == "__inside-emc2101"
                || args[1] == "__inside-ina260",
            args[1] == "__inside-ds4432"
                || args[1] == "__inside-emc2101"
                || args[1] == "__inside-ina260",
            args[1] == "__inside-emc2101" || args[1] == "__inside-ina260",
            args[1] == "__inside-ina260",
        );
    }
    ensure!(
        args.len() == 4 || ((args.len() == 5 || args.len() == 6) && (args[4] == "rtc-sar" || args[4] == "rtc-sar-i2c" || args[4] == "rtc-sar-i2c-ds4432" || args[4] == "rtc-sar-i2c-ds4432-emc2101" || args[4] == "rtc-sar-i2c-ds4432-emc2101-ina260")),
        "usage: build_esp_qemu_backend REPO SAMURAI_BUILD_DIRECTORY NEW_OUTPUT [rtc-sar|rtc-sar-i2c|rtc-sar-i2c-ds4432|rtc-sar-i2c-ds4432-emc2101|rtc-sar-i2c-ds4432-emc2101-ina260 [PRIOR_OVERLAY_BUILD]]"
    );
    let repo = PathBuf::from(&args[1]);
    let prior = PathBuf::from(&args[2]);
    let out = PathBuf::from(&args[3]);
    let sar = args.len() >= 5;
    let ina260 = sar && args[4] == "rtc-sar-i2c-ds4432-emc2101-ina260";
    let emc2101 = sar && (args[4] == "rtc-sar-i2c-ds4432-emc2101" || ina260);
    let ds4432 = sar && (args[4] == "rtc-sar-i2c-ds4432" || emc2101);
    let i2c = sar && (args[4] == "rtc-sar-i2c" || ds4432);
    ensure!(!out.exists(), "new output required");
    fs::create_dir_all(&out)?;
    // The archived S3 machine realizes its AES/RSA devices unconditionally.
    // Their upstream compilation is conditional on libgcrypt.
    if let Some(prior_overlay) = args.get(5) {
        reuse_overlay(Path::new(prior_overlay), &out)?;
    } else {
        capture_overlay_with(&out, &["libslirp", "libgcrypt"])?;
    }
    pack_dir(
        &repo.join("work/simulator/qemu-s9/keycodemapdb-source-002"),
        "keycodemapdb",
        &out.join("keycodes.tar.gz"),
    )?;
    let prior_spec: ProcessSpec = serde_json::from_slice(&fs::read(prior.join("spec.json"))?)?;
    let expected = &prior_spec
        .inputs
        .iter()
        .find(|v| v.guest_path == "/toolchain.tar.gz")
        .context("prior toolchain absent")?
        .object;
    ensure!(
        &hash_file(&prior.join("toolchain.tar.gz"))? == expected,
        "prior captured tree differs"
    );
    let store = Store::open(&out.join("inputs"), 2 * 1024 * 1024 * 1024)?;
    let roots = vec![
        ("/probe".into(), std::env::current_exe()?),
        ("/bin/sh".into(), PathBuf::from("/usr/bin/bash")),
        ("/usr/bin/env".into(), PathBuf::from("/usr/bin/env")),
        ("/compiler".into(), PathBuf::from("/usr/bin/gcc")),
        (
            "/cc1".into(),
            PathBuf::from("/usr/lib/gcc/x86_64-pc-linux-gnu/16/cc1"),
        ),
        ("/python".into(), PathBuf::from("/usr/bin/python3")),
        ("/glib".into(), PathBuf::from("/usr/lib/libglib-2.0.so.0")),
        ("/samu".into(), prior.join("samu")),
    ];
    let mut inputs = md_sim_firmware::lab::elf_closure(&store, roots)?;
    for (guest, host) in [
        (
            "/qemu.tar.gz",
            repo.join("artifacts/files/source/tool-espressif-qemu/qemu-febae182e132.tar.gz"),
        ),
        ("/toolchain.tar.gz", prior.join("toolchain.tar.gz")),
        ("/overlay.tar.gz", out.join("overlay.tar.gz")),
        ("/keycodes.tar.gz", out.join("keycodes.tar.gz")),
        (
            "/diagnostic-fp-tests.patch",
            repo.join("simulator/backends/esp32/diagnostic-fp-tests.patch"),
        ),
    ] {
        let object = hash_file(&host)?;
        store.import(&host, &object)?;
        inputs.push(Mount {
            object,
            guest_path: guest.into(),
        });
    }
    if sar {
        for (guest, host) in [
            (
                "/libmd_sim_esp32s3_sar.a",
                repo.join("work/simulator/bitaxe/sar-target/release/libmd_sim_esp32s3_sar.a"),
            ),
            (
                "/rtc-sar.patch",
                repo.join("simulator/backends/esp32/rtc-sar.patch"),
            ),
            (
                "/rtc-sar-trace-events",
                repo.join("simulator/backends/esp32/rtc-sar-trace-events"),
            ),
        ] {
            let object = hash_file(&host)?;
            store.import(&host, &object)?;
            inputs.push(Mount {
                object,
                guest_path: guest.into(),
            });
        }
    }
    if i2c {
        for (guest, host) in [
            (
                "/i2c-reselect.patch",
                repo.join("simulator/backends/esp32/i2c-reselect.patch"),
            ),
            (
                "/i2c-master.patch",
                repo.join("simulator/backends/esp32/i2c-master.patch"),
            ),
            (
                "/i2c-trace-events",
                repo.join("simulator/backends/esp32/i2c-trace-events"),
            ),
        ] {
            let object = hash_file(&host)?;
            store.import(&host, &object)?;
            inputs.push(Mount {
                object,
                guest_path: guest.into(),
            });
        }
    }
    if ds4432 {
        for name in ["ds4432.patch", "ds4432-trace-events"] {
            let host = repo.join("simulator/backends/esp32").join(name);
            let object = hash_file(&host)?;
            store.import(&host, &object)?;
            inputs.push(Mount {
                object,
                guest_path: format!("/{name}"),
            });
        }
    }
    if emc2101 {
        for name in ["emc2101.patch", "emc2101-trace-events"] {
            let host = repo.join("simulator/backends/esp32").join(name);
            let object = hash_file(&host)?;
            store.import(&host, &object)?;
            inputs.push(Mount {
                object,
                guest_path: format!("/{name}"),
            });
        }
    }
    if ina260 {
        for name in ["ina260.patch", "ina260-trace-events"] {
            let host = repo.join("simulator/backends/esp32").join(name);
            let object = hash_file(&host)?;
            store.import(&host, &object)?;
            inputs.push(Mount {
                object,
                guest_path: format!("/{name}"),
            });
        }
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
            if ina260 {
                "__inside-ina260"
            } else if emc2101 {
                "__inside-emc2101"
            } else if ds4432 {
                "__inside-ds4432"
            } else if i2c {
                "__inside-i2c"
            } else if sar {
                "__inside-sar"
            } else {
                "__inside"
            }
            .into(),
        ],
        inputs,
        stdin_object: None,
        environment: BTreeMap::from([
            ("PATH".into(), "/work/toolchain/usr/bin:/usr/bin".into()),
            ("PYTHONHOME".into(), "/work/toolchain/usr".into()),
            ("PKG_CONFIG_SYSROOT_DIR".into(), "/work/toolchain".into()),
            (
                "PKG_CONFIG_LIBDIR".into(),
                "/work/toolchain/usr/lib/pkgconfig:/work/toolchain/usr/share/pkgconfig".into(),
            ),
            ("LD_LIBRARY_PATH".into(), "/work/toolchain/usr/lib".into()),
            ("TMPDIR".into(), "/tmp".into()),
        ]),
        limits: Limits {
            wall_seconds: 900,
            cpu_seconds: 120,
            memory_bytes: 3 * 1024 * 1024 * 1024,
            processes: 128,
            file_bytes: 64 * 1024 * 1024,
            trace_bytes: 64 * 1024 * 1024,
            tmp_bytes: 2 * 1024 * 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    save_json(
        &out.join("input-identities.json"),
        &json!({"recipe":hash_file(&std::env::current_exe()?)?,"source":SOURCE,"samurai_build_report":hash_file(&prior.join("build-report.json"))?,"overlay_manifest":hash_file(&out.join("overlay-manifest.json"))?}),
    )?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    let bytes = hex::decode(&result.stdout)?;
    fs::write(out.join("build-envelope.json"), &bytes)?;
    if !bytes.is_empty() {
        let mut r: serde_json::Value = serde_json::from_slice(&bytes)?;
        if let Some(b) = r["binary_gzip_hex"].as_str() {
            let compressed = hex::decode(b)?;
            ensure!(
                compressed.len() < 10 * 1024 * 1024
                    && md_sim_firmware::prepare::hash_bytes(&compressed)
                        == r["binary_gzip"]["sha256"]
                    && r["binary_gzip"]["size"] == compressed.len(),
                "compressed export differs"
            );
            let mut b = Vec::new();
            flate2::read::GzDecoder::new(compressed.as_slice())
                .take(28 * 1024 * 1024 + 1)
                .read_to_end(&mut b)?;
            ensure!(
                b.len() < 28 * 1024 * 1024
                    && r["binary"]["size"] == b.len()
                    && md_sim_firmware::prepare::hash_bytes(&b) == r["binary"]["sha256"],
                "binary identity mismatch"
            );
            fs::write(out.join("qemu-system-xtensa"), b)?;
            r.as_object_mut().unwrap().remove("binary_gzip_hex");
        }
        if let Some(b) = r["rom_hex"].as_str() {
            let b = hex::decode(b)?;
            ensure!(
                md_sim_firmware::prepare::hash_bytes(&b) == r["rom"]["sha256"],
                "ROM identity mismatch"
            );
            fs::write(out.join("esp32s3_rev0_rom.bin"), b)?;
            r.as_object_mut().unwrap().remove("rom_hex");
        }
        save_json(&out.join("build-report.json"), &r)?;
    }
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated,
        "isolated QEMU build failed; evidence retained"
    );
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    let new_output = args.get(3).is_some_and(|p| !Path::new(p).exists());
    let result = run();
    if new_output && args.get(1).is_none_or(|a| a != "__inside") {
        if let Some(out) = args.get(3).map(PathBuf::from).filter(|p| p.is_dir()) {
            let assessment = json!({"backend_build_passed":result.is_ok(),"firmware_boot_passed":false,"exact_physical_target_passed":false,"error":result.as_ref().err().map(|e|format!("{e:#}"))});
            save_json(&out.join("assessment.json"), &assessment)?;
        }
    }
    result
}

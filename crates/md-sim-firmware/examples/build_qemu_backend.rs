//! Closed-input experimental QEMU build. No acquired tool executes on the host.
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
#[path = "support/qemu_slirp.rs"]
mod qemu_slirp;
use qemu_build::{capture_overlay, pack_dir, reuse_overlay, step, unpack};
const SOURCE: &str = "2a40e6705e59fd8807012631bc6b2c0877f10481b12768f78e4f9d7b88376cce";
const PREFIX: &str = "qemu-11661d857254b27fca5568c6deacf4f6b6f3460f";
const DECODETREE_SHA256: &str = "91e068b1993166ba9bf259573f457342c54de70a117f7db42355a3a50fcc0c68";
// Skip the complete ARM-only stage before touching any ARM source or tool.
// This function is also the host-side regression boundary; the actual patch
// command is reached only after both exact ARM parents are verified.
fn patch_ps_gpio(src: &Path, report: &mut serde_json::Value, pool_x86: bool) -> Result<bool> {
    if pool_x86 {
        return Ok(true);
    }
    let identities = [
        (
            "hw/arm/xilinx_zynq.c",
            "c51200a1fdbab897a190b764a8f9caaa483b846e5174ebe2d7d4c97d6721efb4",
            "013ba4de47b293d60cc9667c98c85998f7fc7a53ebba1be047ce1cbff29c4a48",
        ),
        (
            "hw/misc/zynq_slcr.c",
            "dd74ec65e82a06c5791510024bbb20bedc99f461f55cdd32ffc38db272dc975a",
            "369ca416465d43abad564d0fcbae78568022eabc3c7d267617ac168e70fd522b",
        ),
    ];
    for (member, parent, _) in identities {
        ensure!(
            hash_file(&src.join(member))?.sha256 == parent,
            "PS GPIO parent differs: {member}"
        );
    }
    let mut patch = Command::new("/work/toolchain/usr/bin/patch");
    patch.current_dir(src).args([
        "-p1",
        "--fuzz=0",
        "--batch",
        "--forward",
        "--input=/ps-gpio.patch",
    ]);
    let success = step(report, "ps_gpio_patch", patch)?;
    let mut sources = BTreeMap::new();
    for (member, _, derived) in identities {
        let object = hash_file(&src.join(member))?;
        ensure!(
            !success || object.sha256 == derived,
            "PS GPIO derived source differs: {member}"
        );
        sources.insert(member, object);
    }
    report["ps_gpio_reference"] = json!({
        "contract":"md-sim-zynq7000-ps-gpio/1",
        "patch":hash_file(Path::new("/ps-gpio.patch"))?,
        "basis":hash_file(Path::new("/ps-gpio-contract.md"))?,
        "binding":hash_file(&src.join("hw/i2c/md-sim-ps-gpio.c"))?,
        "header":hash_file(&src.join("hw/i2c/md-sim-ps-gpio.h"))?,
        "sources":sources,"default_enabled":false,
        "bank_widths":[32,22,32,32],"gic_spi":20,
        "clock":"Existing CPU_1x with APER bit22; explicit settled samples only",
        "reset":"Construction cold state; GPIO bus reset and QMP warm reset preserve controller state",
        "unknown":"Unknown guest reads and IRQs stop; external pin levels are never synthesized",
        "limits":["No physical MIO mux/straps/pulls or S9 pin wiring","No synchronizer/physical timing calibration","No migration, firmware boot or exact-board pass"]
    });
    Ok(success)
}

fn inner() -> Result<()> {
    confinement::require_emulator_build()?;
    ensure!(
        std::env::var("PYTHONHASHSEED").as_deref() == Ok("0"),
        "QEMU source generators require the frozen Python hash seed"
    );
    let core: serde_json::Value =
        serde_json::from_reader(fs::File::open("/core-checkpoint.json")?)?;
    ensure!(
        core["library"] == serde_json::to_value(hash_file(Path::new("/libmd_sim_qemu_core.a"))?)?,
        "captured model library differs from its source checkpoint"
    );
    let ubsan = std::env::var("MD_SIM_QEMU_UBSAN").as_deref() == Ok("1");
    let pool_x86 = std::env::var("MD_SIM_QEMU_POOL_X86").as_deref() == Ok("1");
    let binary_name = if pool_x86 {
        "qemu-system-x86_64"
    } else {
        "qemu-system-arm"
    };
    ensure!(
        hash_file(Path::new("/qemu.tar.gz"))?.sha256 == SOURCE,
        "QEMU source differs"
    );
    unpack("/toolchain.tar.gz", "/work")?;
    unpack("/overlay.tar.gz", "/work")?;
    let slirp = qemu_slirp::stage()?;
    unpack("/qemu.tar.gz", "/work")?;
    let src = Path::new("/work").join(PREFIX);
    ensure!(
        hash_file(&src.join("scripts/decodetree.py"))?.sha256 == DECODETREE_SHA256,
        "pinned CPU source generator differs"
    );
    let build = Path::new("/work/build");
    fs::create_dir_all(build)?;
    unpack(
        "/keycodes.tar.gz",
        src.join("subprojects").to_str().unwrap(),
    )?;
    fs::create_dir_all(src.join("mdsim"))?;
    fs::copy(
        "/libmd_sim_qemu_core.a",
        src.join("mdsim/libmd_sim_qemu_core.a"),
    )?;
    for name in [
        "md-sim-s9io.c",
        "md-sim-s9io.h",
        "md-sim-axi-gpio.c",
        "md-sim-axi-gpio.h",
        "md-sim-fan.c",
        "md-sim-fan.h",
        "md-sim-pic16.c",
        "md-sim-pic16.h",
        "md-sim-swdt.c",
        "md-sim-swdt.h",
        "md-sim-ps-gpio.c",
        "md-sim-ps-gpio.h",
    ] {
        fs::copy(format!("/{name}"), src.join("hw/i2c").join(name))?;
    }
    fs::copy("/md-sim-pcap.h", src.join("hw/dma/md-sim-pcap.h"))?;
    const NAND_ARCHIVE: &str = "5456034eb8a9f348b1f4c8820dc79f55677bddfe0d1c66566b6c42ceb9df66f0";
    const NAND_MEMBER: &str = "6ef0d6f3bafea8f37c0d9307b8cc981c0cb08e3273a44f3e1e5d4e3d5c1f1667";
    ensure!(
        hash_file(Path::new("/legacy-qemu.tar.gz"))?.sha256 == NAND_ARCHIVE,
        "legacy NAND archive differs"
    );
    let mut legacy = tar::Archive::new(
        flate2::read::GzDecoder::new(fs::File::open("/legacy-qemu.tar.gz")?)
            .take(1024 * 1024 * 1024),
    );
    let mut nand = None;
    for (index, entry) in legacy.entries()?.enumerate() {
        ensure!(index < 100_000, "legacy archive entry budget");
        let mut entry = entry?;
        if entry.path()?
            == Path::new("qemu-febae182e132e4055529be423a818225ebddaa3a/hw/block/nand.c")
        {
            ensure!(
                nand.is_none()
                    && entry.header().entry_type().is_file()
                    && entry.size() < 128 * 1024,
                "NAND member bound"
            );
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            ensure!(
                md_sim_firmware::prepare::hash_bytes(&bytes) == NAND_MEMBER,
                "NAND member differs"
            );
            nand = Some(String::from_utf8(bytes)?);
        }
    }
    let original_nand = nand.context("archived NAND member missing")?;
    let nand = original_nand
        .replace("\"hw/hw.h\"", "\"hw/core/cpu.h\"")
        .replace("\"hw/qdev-properties.h\"", "\"hw/core/qdev-properties.h\"")
        .replace(
            "\"hw/qdev-properties-system.h\"",
            "\"hw/core/qdev-properties-system.h\"",
        )
        .replace("\"hw/block/flash.h\"", "\"../md-sim-nand.h\"")
        .replace("\"sysemu/block-backend.h\"", "\"system/block-backend.h\"")
        .replace(
            "static Property nand_properties[]",
            "static const Property nand_properties[]",
        )
        .replace("    DEFINE_PROP_END_OF_LIST(),\n", "")
        .replace(
            "nand_class_init(ObjectClass *klass, void *data)",
            "nand_class_init(ObjectClass *klass, const void *data)",
        )
        .replace(
            "        hw_error(\"%s: Unsupported NAND chip ID.\\n\", __func__);",
            "        error_report(\"%s: Unsupported NAND chip ID.\", __func__);\n        abort();",
        );
    fs::create_dir_all(src.join("hw/i2c/mdsim"))?;
    fs::write(src.join("hw/i2c/mdsim/nand.c"), &nand)?;
    let mut report = json!({"source":SOURCE,"limitations":["Provider-derived Samurai and keycodemapdb source normalization recorded separately.","Build success alone is not firmware boot evidence.",if pool_x86 {"This build selects the x86 PC and Ethernet support VM; it does not execute miner firmware."} else {"This build selects Zynq and its required devices only; it does not claim other ARM board support."}]});
    report["generator_environment"] = json!({
        "PYTHONHASHSEED":"0",
        "reason":"The pinned decodetree generator emits independent field operations from a Python set. Freeze its order without changing upstream CPU code.",
        "decodetree_sha256":DECODETREE_SHA256,
        "scope":"Build reproducibility setting. Guest clocks, firmware, CPU semantics and device behavior are unchanged."
    });
    report["core_checkpoint"] = json!({"manifest":hash_file(Path::new("/core-checkpoint.json"))?,"library":core["library"]});
    report["legacy_nand"] = json!({"archive_sha256":NAND_ARCHIVE,"member_sha256":NAND_MEMBER,"adapted_sha256":md_sim_firmware::prepare::hash_bytes(nand.as_bytes()),"changes":["QEMU include-path port","property arrays use explicit length and const elements","class-init callback uses const data","removed hw_error API replaced by error_report plus fatal abort for unsupported ID"],"storage_logic":"Original pin-level NAND storage engine retained","hardware_identity":"generic QEMU x8 256MiB NAND fixture; not a resolved physical S9 part"});
    report["prior_build_diagnosis"] = json!("Attempt002 exposed missing libexpat during Python distlib import; capturing expat fixes the toolchain. No pip compatibility patch is retained.");
    let mut patch = Command::new("/work/toolchain/usr/bin/patch");
    patch
        .current_dir(&src)
        .args(["-p1", "--batch", "--forward", "--input=/backend.patch"]);
    let mut success = step(&mut report, "patch", patch)?;
    if success {
        for (member, expected) in [
            (
                "net/socket.c",
                "182194a1b5c0ffb8f94ff5dab41647ac0775215fcb6d58c0aa6d7fd1bbc5d87d",
            ),
            (
                "qapi/net.json",
                "01f716c110d0b5f4cc21468cf76b32c4da2bf3658b0bc405e5c65dfea8727f08",
            ),
        ] {
            ensure!(
                hash_file(&src.join(member))?.sha256 == expected,
                "private stream baseline differs"
            );
        }
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch.current_dir(&src).args([
            "-p1",
            "--batch",
            "--forward",
            "--input=/private-stream.patch",
        ]);
        success = step(&mut report, "private_stream_patch", patch)?;
        report["private_stream"] = json!({
            "contract":"md-sim-private-inherited-ethernet/1",
            "patch":hash_file(Path::new("/private-stream.patch"))?,
            "source_members":{
                "net/socket.c":hash_file(&src.join("net/socket.c"))?,
                "qapi/net.json":hash_file(&src.join("qapi/net.json"))?
            },
            "transport":"Caller-created connected AF_UNIX/SOCK_STREAM pair; POSIX read/writev; original QEMU packet framing and backpressure.",
            "address_queries":false,"host_listener":false,"cpu_implementation_changed":false
        });
    }
    if success {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/cpu-reset.patch"]);
        success = step(&mut report, "cpu_patch", patch)?;
    }
    if success {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/cpu-timers.patch"]);
        success = step(&mut report, "cpu_timers_patch", patch)?;
        report["cpu_timer_clocks"] = json!({
            "contract":hash_file(Path::new("/cpu-timers-contract.md"))?,
            "patch":hash_file(Path::new("/cpu-timers.patch"))?,
            "ps_clock_hz":33_333_333,
            "path":"SLCR PLL/mux/divider, cpu_3or2x gate25 and fixed /2, A9 global/private/watchdog timers",
            "implementation":"Existing QEMU Clock, global timer and PTimer; fractional periods retained",
            "limits":["Clock changes restart fractional cycle phase","PLL lock timing and complete fabric clock tree are not modeled","Migration and physical calibration are not validated"]
        });
    }
    if success {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch.current_dir(&src).args([
            "-p1",
            "--batch",
            "--forward",
            "--input=/ttc-deadline.patch",
        ]);
        success = step(&mut report, "ttc_deadline_patch", patch)?;
        report["ttc_deadline"] = json!({
            "patch":hash_file(Path::new("/ttc-deadline.patch"))?,
            "parent_source_sha256":"a35ec96362cd3e0289d2a2f2796866f2d1cbb29534f4f84ef41fc48b11309fbd",
            "derived_source":hash_file(&src.join("hw/timer/cadence_ttc.c"))?,
            "change":"Round fractional event deadlines upward to prevent zero-time callback loops; cancel scheduled callbacks when disabled.",
            "limits":["Intermediate deadline patch; ttc_clock_tree records the final clock and counter precision","No physical calibration claim"]
        });
    }
    if success {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/ttc-clock.patch"]);
        success = step(&mut report, "ttc_clock_patch", patch)?;
        report["ttc_clock_tree"] = json!({
            "contract":hash_file(Path::new("/ttc-clock-contract.md"))?,
            "patch":hash_file(Path::new("/ttc-clock.patch"))?,
            "path":"SLCR ARM PLL/mux/divider, cpu_1x gate27 and /4 or /6 from CLK_621_TRUE, both TTC blocks",
            "precision":"QEMU32.32ns periods, existing16.16 counter, retained division remainder between reads",
            "source_members":{
                "hw/misc/zynq_slcr.c":hash_file(&src.join("hw/misc/zynq_slcr.c"))?,
                "hw/arm/xilinx_zynq.c":hash_file(&src.join("hw/arm/xilinx_zynq.c"))?,
                "hw/timer/cadence_ttc.c":hash_file(&src.join("hw/timer/cadence_ttc.c"))?,
                "include/hw/timer/cadence_ttc.h":hash_file(&src.join("include/hw/timer/cadence_ttc.h"))?
            },
            "limits":["External clock pins have no connected source","Clock or prescaler changes restart sub-counter phase","PLL lock timing, full migration and physical calibration remain unverified"]
        });
    }
    if success {
        let timer = src.join("hw/timer/cadence_ttc.c");
        ensure!(
            hash_file(&timer)?.sha256
                == "9bc8b78fd369452fdda16583b18a7ddc68d953d77deca40b453b5afb3a3d2893",
            "TTC reset parent differs"
        );
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/ttc-reset.patch"]);
        success = step(&mut report, "ttc_reset_patch", patch)?;
        ensure!(
            !success
                || hash_file(&timer)?.sha256
                    == "d0542840eaec50b1731041686d298abe2169d3988cba019a20447e42be8ff0cd",
            "TTC reset output differs"
        );
        report["ttc_reset"] = json!({
            "patch":hash_file(Path::new("/ttc-reset.patch"))?,
            "derived_source":hash_file(&timer)?,
            "behavior":"QEMU system reset restores the original model cold register defaults, clears the fractional count, cancels pending callbacks and lowers each timer IRQ.",
            "limits":["Reference device reset; not physical reset calibration or a firmware recovery pass"]
        });
    }
    if success {
        let timer = src.join("hw/timer/cadence_ttc.c");
        ensure!(
            hash_file(&timer)?.sha256
                == "d0542840eaec50b1731041686d298abe2169d3988cba019a20447e42be8ff0cd",
            "TTC mode parent differs"
        );
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/ttc-modes.patch"]);
        success = step(&mut report, "ttc_modes_patch", patch)?;
        ensure!(
            !success
                || hash_file(&timer)?.sha256
                    == "7ae1a27bef1633b62689e3c38ab7c1e49eb9ae9fb51a9693039631d6f3d37ded",
            "TTC mode output differs"
        );
        report["ttc_modes"] = json!({
            "patch":hash_file(Path::new("/ttc-modes.patch"))?,
            "derived_source":hash_file(&timer)?,
            "behavior":"Control bit3 gates match scheduling and status. Clock and counter control bit6 retain readback. Latched status still needs read acknowledgement.",
            "limits":["External clock pins and waveform outputs remain unconnected","Decrement mode needs separate conformance work","No firmware or whole-board claim"]
        });
    }
    if success {
        let timer = src.join("hw/timer/cadence_ttc.c");
        ensure!(
            hash_file(&timer)?.sha256
                == "7ae1a27bef1633b62689e3c38ab7c1e49eb9ae9fb51a9693039631d6f3d37ded",
            "TTC decrement parent differs"
        );
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch.current_dir(&src).args([
            "-p1",
            "--batch",
            "--forward",
            "--input=/ttc-decrement.patch",
        ]);
        success = step(&mut report, "ttc_decrement_patch", patch)?;
        ensure!(
            !success
                || hash_file(&timer)?.sha256
                    == "050ca5932fa7892efe12a5306540a22886fca2cff1705f8c05eff49f1de54277",
            "TTC decrement output differs"
        );
        report["ttc_decrement"] = json!({
            "patch":hash_file(Path::new("/ttc-decrement.patch"))?,
            "derived_source":hash_file(&timer)?,
            "behavior":"Down-counter reset loads interval top or65535. Complementary16-bit phase preserves integer counts until completed input ticks. Match deadlines use the same count edge. Direction changes retain integer count and sub-tick phase.",
            "limits":["No physical calibration, external clock or waveform output claim"]
        });
    }
    if success && !pool_x86 {
        let timer = src.join("util/qemu-timer.c");
        ensure!(
            hash_file(&timer)?.sha256
                == "2b8436271d4638cf97927981c1ab26934488c3c819e44d6aa5b870fa0621317c",
            "timer diagnostic parent differs"
        );
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch.current_dir(&src).args([
            "-p1",
            "--batch",
            "--forward",
            "--input=/timer-diagnostics.patch",
        ]);
        success = step(&mut report, "timer_diagnostics_patch", patch)?;
        report["timer_diagnostics"] = json!({"patch":hash_file(Path::new("/timer-diagnostics.patch"))?,
            "parent_source_sha256":"2b8436271d4638cf97927981c1ab26934488c3c819e44d6aa5b870fa0621317c",
            "derived_source":hash_file(&timer)?,
            "scope":"Report callback offsets during prolonged timer dispatch. No callback, deadline, clock or guest result is changed."});
    }
    if success {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/pl353.patch"]);
        success = step(&mut report, "pl353_patch", patch)?;
    }
    if success {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/nand-timing.patch"]);
        success = step(&mut report, "nand_timing_patch", patch)?;
        report["legacy_nand"]["final_source"] =
            serde_json::to_value(hash_file(&src.join("hw/i2c/mdsim/nand.c"))?)?;
        report["legacy_nand"]["timing_adaptation"] = json!(
            "Deferred original program/erase functions, separate pending data, NAND reset abort, live status and backend error bit. Controller reset preserves chip operation. No migration support."
        );
    }

    if success {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/legacy-io.patch"]);
        success = step(&mut report, "legacy_io_patch", patch)?;
        report["legacy_s9io"] = json!({
            "scope":"Legacy S9IO0.2 development fixture only. No mapping to2022 firmware bitstream.",
            "version":0x00900200u32,
            "build_id":0x74657374u32,
            "clock_hz":50_000_000,
            "mmio":"Atomic transactions. Elapsed core edges precede same-timestamp MMIO. AXI channel stalls are not modeled.",
            "missing":["ASIC chain","exact selected bitstream mapping","physical timing calibration","migration"],
            "rust_library":hash_file(Path::new("/libmd_sim_qemu_core.a"))?,
            "binding":hash_file(&src.join("hw/i2c/md-sim-s9io.c"))?,
            "header":hash_file(&src.join("hw/i2c/md-sim-s9io.h"))?,
        });
        report["reference_s9io_v1"] = json!({
            "scope":"BM1387 reference-contract development profile; selected FPGA correspondence unvalidated",
            "version":0x00901002u32,"clock_hz":200_000_000,
            "build_id":0x74657374u32,
            "enable_low_txd":false,
            "unsupported":["BM139X dataplane","destructive disable with changed midstate field after transmitted UART bytes"],
            "inference":"Common FIFO internals, timing and reset defaults reused from legacy HDL",
            "missing":["selected bitstream equivalence","dynamic fabric clock/reset","powered ASIC chain","physical calibration"],
            "contract":hash_file(Path::new("/s9io-v1-contract.md"))?
        });
    }
    if success {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/gpio.patch"]);
        success = step(&mut report, "gpio_patch", patch)?;
        report["gpio"] = json!({
            "scope":"Fixed-direction single-channel AXI GPIO; no interrupt block",
            "input_width":9,"output_width":13,
            "external_inputs":"Development fixture defaults to all low; no ASIC chain attached",
            "timing":"Atomic word MMIO with settled inputs; no synchronizer or AXI channel timing",
            "missing":["clock/reset-domain coupling","physical timing calibration","migration"],
            "binding":hash_file(&src.join("hw/i2c/md-sim-axi-gpio.c"))?,
            "header":hash_file(&src.join("hw/i2c/md-sim-axi-gpio.h"))?,
        });
    }
    if success {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/sd2022-io.patch"]);
        success = step(&mut report, "sd2022_io_patch", patch)?;
        report["sd2022_io_layout"] = json!({
            "source_dtb_sha256":"550c87818cb0db41c00a8351dc4ac7b5dce5bf6d5336539c6951163ebf3cb62a",
            "bases":[0x43c00000u32,0x43c10000u32,0x43c20000u32],
            "tx_rx_command_irqs":[[62,63,64],[65,66,67],[68,84,85]],
            "controller_core":"Reused legacy S9IO0.2 state machine; explicit development hypothesis, not established2022 bitstream correspondence",
            "version":0x00900200u32,"build_id":0x74657374u32,
            "claim":"Address and interrupt routing from the selected image; no exact-board compatibility or mining pass"
        });
    }

    if success {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/fan.patch"]);
        success = step(&mut report, "fan_patch", patch)?;
        report["fan_reference"] = json!({
            "contract":hash_file(Path::new("/fan-contract.md"))?,
            "binding":hash_file(&src.join("hw/i2c/md-sim-fan.c"))?,
            "header":hash_file(&src.join("hw/i2c/md-sim-fan.h"))?,
            "scope":"AM1 source register contract and explicit uncalibrated motor fixture; selected FPGA correspondence unvalidated",
            "sample_nanoseconds":1_000_000_000u64,
            "motors":{"minimum_rpm":900,"maximum_rpm":6000,"slew_rpm_per_second":12000,"pulses_per_revolution":2,"pwm_channels":[0,1],"tach_channels":[null,1]},
            "default_motors_connected":false,
            "input_plan_contract":"md-sim-fan-input-plan/1"
        });
    }
    if success {
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch
            .current_dir(&src)
            .args(["-p1", "--batch", "--forward", "--input=/pic16.patch"]);
        success = step(&mut report, "pic16_patch", patch)?;
        report["pic16_reference"] = json!({
            "contract":hash_file(Path::new("/pic16-contract.md"))?,
            "binding":hash_file(&src.join("hw/i2c/md-sim-pic16.c"))?,
            "header":hash_file(&src.join("hw/i2c/md-sim-pic16.h"))?,
            "application":hash_file(Path::new("/pic16-app.txt"))?,
            "addresses":[85,86,87],
            "scope":"Known application protocol contract; no PIC instruction execution, no factory loader or calibration dump",
            "storage":"Application words from pinned input; all other words explicitly erased development baseline",
            "persistence":"Committed words survive device reset; process storage is temporary; migration unsupported",
            "power_enable_output_connected":false
        });
    }
    if success {
        // This freezes the combined ARM board after all preceding patches.
        // A successful fuzzy patch application is not sufficient evidence.
        ensure!(
            hash_file(&src.join("hw/arm/xilinx_zynq.c"))?.sha256
                == "923433140d2366336decb9e48ff5cf3f9af7e9e22eb7771ca76879c51f33373a",
            "BM1387 parent board source differs"
        );
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch.current_dir(&src).args([
            "-p1",
            "--fuzz=0",
            "--batch",
            "--forward",
            "--input=/bm1387.patch",
        ]);
        success = step(&mut report, "bm1387_patch", patch)?;
        let board_source = hash_file(&src.join("hw/arm/xilinx_zynq.c"))?;
        ensure!(
            board_source.sha256
                == "89abaa2623a9a5a93311baa1c2f6c35c204a1bb763db5d7de6ced7ff4a938cf6",
            "BM1387 derived board source differs"
        );
        report["bm1387_reference"] = json!({
            "contract":hash_file(Path::new("/bm1387-contract.md"))?,
            "core_map":hash_file(Path::new("/bm1387-core-map.json"))?,
            "board_source":board_source,
            "patch":hash_file(Path::new("/bm1387.patch"))?,
            "chains":3,"chips_per_chain":63,"cores_per_chip":114,
            "oscillator_hz":25_000_000,"initial_baud_divisor":26,
            "identity_reserved":144,"reply_byte_capacity":1024,
            "power_addresses":[85,86,87],"reset_gpio_output_bits":[9,10,11],
            "serial":"Midpoint8N1 ASIC contract, coupled to archived S9IO UART on one Rust timeline",
            "clock_contract":"Independent physical ASIC deadlines; Q32 fabric edges; old levels sampled on ties; explicit fabric-clock-reference property defaults off",
            "compact_trace":"md-sim-bm1387-packets/1",
            "low_rate_filter_contract":"md-sim-bm1387-fixed-filter/1",
            "temperature_input_plan_contract":"md-sim-temperature-input-plan/1",
            "temperature_fixture":{
                "contract":hash_file(Path::new("/tmp421-contract.md"))?,
                "profile":hash_file(Path::new("/tmp421-profile.json"))?,
                "identity":serde_json::from_slice::<serde_json::Value>(&fs::read("/tmp421-profile.json")?)?
            },
            "hashing":{"mode":"functional","default_cap_hps_total":10000,
                "scope":"One shared wall-clock budget across all three chains",
                "distribution":"weighted-by-enabled-cores",
                "guest_clock_polling":"One independent virtual interval per chain",
                "filter":"Native fixed32-bit condition and each guest-programmed ticket mask",
                "override":false,"work_replacement":"New work replaces the search cursor; prior serial replies retain wire time"},
            "unsupported":["PLL status/lock timing","I2C without explicit temperature fixture","unresolved sensor wiring and NACK behavior","divergent per-chip baud","unknown reset registers","gate transitions that change active topology during work"],
            "claim":"Reference development profile; no exact board, calibration or mining acceptance"
        });
    }
    if success && !pool_x86 {
        let board = src.join("hw/arm/xilinx_zynq.c");
        let slcr = src.join("hw/misc/zynq_slcr.c");
        ensure!(
            hash_file(&board)?.sha256
                == "89abaa2623a9a5a93311baa1c2f6c35c204a1bb763db5d7de6ced7ff4a938cf6"
                && hash_file(&slcr)?.sha256
                    == "95301bc40f1bc0db82fbc7124ee6a3df5e61018923cde270dbf4f3bb758efd8c",
            "SWDT parent board or clock source differs"
        );
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch.current_dir(&src).args([
            "-p1",
            "--fuzz=0",
            "--batch",
            "--forward",
            "--input=/swdt.patch",
        ]);
        success = step(&mut report, "swdt_patch", patch)?;
        ensure!(
            hash_file(&slcr)?.sha256
                == "68a7229709ef69db7187a54ba0d5361d8edc6a20fd5c7321a2cf0936ad71e1fd",
            "SWDT derived clock source differs"
        );
        report["swdt_reference"] = json!({
            "contract":hash_file(Path::new("/swdt-contract.md"))?,
            "patch":hash_file(Path::new("/swdt.patch"))?,
            "binding":hash_file(&src.join("hw/i2c/md-sim-swdt.c"))?,
            "header":hash_file(&src.join("hw/i2c/md-sim-swdt.h"))?,
            "board_source":hash_file(&board)?,"slcr_source":hash_file(&slcr)?,
            "address":0xf8005000u32,"irq":41,"clock":"SLCR CPU_1x or unconnected external input",
            "timer":"Pinned QEMU PTIMER one-shot counter and interrupt pulse",
            "limits":["Reset hidden count uses default reload assumption","No external reset waveform or BootROM reset-reason emulation","Absent clock returns bus error instead of APB hang","Clock changes restart fractional phase","Migration rejected","No physical calibration or exact-board acceptance"]
        });
    }
    if success && !pool_x86 {
        let slcr = src.join("hw/misc/zynq_slcr.c");
        ensure!(
            hash_file(&slcr)?.sha256
                == "68a7229709ef69db7187a54ba0d5361d8edc6a20fd5c7321a2cf0936ad71e1fd",
            "fabric clock parent differs"
        );
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch.current_dir(&src).args([
            "-p1",
            "--fuzz=0",
            "--batch",
            "--forward",
            "--input=/fabric.patch",
        ]);
        success = step(&mut report, "fabric_patch", patch)?;
        report["fabric_outputs"] = json!({
            "patch":hash_file(Path::new("/fabric.patch"))?,
            "contract":hash_file(Path::new("/fabric-contract.md"))?,
            "source":hash_file(&slcr)?,
            "scope":"Four SLCR fabric clocks and four active-low reset output signals",
            "limits":["Counted throttle mode stops with model error",
                "PL reset synchronizer and exact selected bitstream mapping remain open",
                "Legacy S9IO uses FCLK1; optional BM1387 fabric-clock-reference uses explicit test wiring; selected2020 defaults retain fixed clocks"]
        });
    }
    if success && !pool_x86 {
        let source = src.join("hw/dma/xlnx-zynq-devcfg.c");
        ensure!(
            hash_file(&source)?.sha256
                == "59592eae97c41c38a7f2bc08d2bb827fc19fa79445036919e7ad745dd04cf42d",
            "PCAP parent source differs"
        );
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch.current_dir(&src).args([
            "-p1",
            "--fuzz=0",
            "--batch",
            "--forward",
            "--input=/pcap.patch",
        ]);
        success = step(&mut report, "pcap_patch", patch)?;
        report["pcap_configuration"] = json!({
            "patch":hash_file(Path::new("/pcap.patch"))?,
            "contract":hash_file(Path::new("/pcap-contract.md"))?,
            "header":hash_file(&src.join("hw/dma/md-sim-pcap.h"))?,
            "source":hash_file(&source)?,
            "scope":"Explicit md-bitstream/md-part contract; actual guest DMA bytes must match the complete CRC/ECC-validated configuration",
            "limits":["No reconstructed logic or exact board claim", "Synchronous upstream DMA timing; PCAP calibration open", "Unsupported data stops with a model error", "Migration rejected when strict PCAP is selected"]
        });
    }
    if success && !pool_x86 {
        let identities = [
            (
                "hw/arm/xilinx_zynq.c",
                "943b7cf7ac9f2b93b246b0ca4e073c4f442ee43ab4cb10172027056be03bcd21",
                "c51200a1fdbab897a190b764a8f9caaa483b846e5174ebe2d7d4c97d6721efb4",
            ),
            (
                "hw/misc/zynq_slcr.c",
                "d28d0a65580b353dc3664418cd4656fb567479c260ba7f9017394a4e718c797f",
                "dd74ec65e82a06c5791510024bbb20bedc99f461f55cdd32ffc38db272dc975a",
            ),
            (
                "include/hw/arm/xilinx_zynq.h",
                "293904617a00c991e264e640bd58da0f192d7fbc31bcd4ae62c601a7d68d87b6",
                "fc6bf9d4b15419ab8f643bfeb323ded68f4bf04c7c5a07dd577984412678fd49",
            ),
            (
                "hw/arm/trace-events",
                "40f9c13e75844e755b910ea82964a66b30ae17b8df578b7d00cce8d6e65b3bde",
                "f28abcb79a79acaebd3dc5d92638d55dbc73a004b35a934ce7d51880dc6ac277",
            ),
        ];
        for (member, parent, _) in identities {
            ensure!(
                hash_file(&src.join(member))?.sha256 == parent,
                "ROM reset parent differs: {member}"
            );
        }
        let mut patch = Command::new("/work/toolchain/usr/bin/patch");
        patch.current_dir(&src).args([
            "-p1",
            "--fuzz=0",
            "--batch",
            "--forward",
            "--input=/rom-reset.patch",
        ]);
        success = step(&mut report, "rom_reset_patch", patch)?;
        let mut sources = BTreeMap::new();
        for (member, _, derived) in identities {
            let object = hash_file(&src.join(member))?;
            ensure!(
                !success || object.sha256 == derived,
                "ROM reset derived source differs: {member}"
            );
            sources.insert(member, object);
        }
        report["rom_reset_reference"] = json!({
            "contract":"md-sim-public-zynq-rom-reset/1",
            "patch":hash_file(Path::new("/rom-reset.patch"))?,
            "basis":hash_file(Path::new("/rom-reset-contract.md"))?,
            "sources":sources,"default_enabled":false,
            "reset_reason_contract":"POR at realization; persistent RST_REASON/REBOOT_STATUS across reference warm reset; actual PSS_SOFT_RST records SLC_RST; QMP reset has no invented hardware cause",
            "scope":"Read-only public ROM, architectural CPU reset and internal OCM mapping. No direct stage entry. Private SWAP is rejected.",
            "limits":["Exact S9 silicon and ROM binding unresolved","Other support devices remain reference models","No physical timing, complete reset-to-firmware, board or mining pass"]
        });
    }
    if success {
        success = patch_ps_gpio(&src, &mut report, pool_x86)?;
    }
    if success {
        if pool_x86 {
            let devices = src.join("configs/devices/x86_64-softmmu/mdsim-pool.mak");
            // Use upstream virtio serial for the private worker channel. No
            // miner bus or timing-sensitive device operation crosses it.
            fs::write(&devices,
                "CONFIG_I440FX=y\nCONFIG_E1000_PCI=y\nCONFIG_SERIAL_ISA=y\nCONFIG_VIRTIO_PCI=y\nCONFIG_VIRTIO_SERIAL=y\n")?;
            report["support_device_configuration"] = json!({
                "input":hash_file(&devices)?, "text":fs::read_to_string(&devices)?,
                "scope":"Support VM devices only; upstream virtio serial carries worker control"});
        }
        report["target"] = json!(if pool_x86 {
            "x86_64-private-pool-support"
        } else {
            "arm-s9-development"
        });
        let mut config = Command::new("/bin/sh");
        config.current_dir(build).arg(src.join("configure")).args([
            if pool_x86 {
                "--target-list=x86_64-softmmu"
            } else {
                "--target-list=arm-softmmu"
            },
            "--without-default-devices",
            if pool_x86 {
                "--with-devices-x86_64=mdsim-pool"
            } else {
                "--with-devices-arm=mdsim"
            },
            "--without-default-features",
            "--disable-docs",
            "--disable-rust",
            "--disable-download",
            "--disable-werror",
            "--disable-tools",
            "--enable-fdt",
            "--enable-pixman",
            "--enable-tcg",
            "--enable-slirp",
            "--enable-trace-backends=log",
            "--ninja=/samu",
            "--python=/work/toolchain/usr/bin/python3",
            "--cc=/work/toolchain/usr/bin/gcc --sysroot=/work/toolchain -B/work/toolchain/usr/bin",
            "--cxx=/work/toolchain/usr/bin/g++ --sysroot=/work/toolchain -B/work/toolchain/usr/bin",
        ]);
        if pool_x86 {
            config.arg("--enable-kvm");
            report["support_accelerators"] = json!({
                "compiled":["tcg","kvm"],
                "selection":"Explicit runtime policy; no automatic fallback",
                "scope":"Only the x86 support environment. ARM miner execution and device models are unchanged."});
        }
        let sanitizer = if ubsan {
            " -fsanitize=undefined -fno-sanitize-recover=all"
        } else {
            ""
        };
        config.arg(format!(
            "--extra-cflags=--sysroot=/work/toolchain -B/work/toolchain/usr/bin{sanitizer}"
        ));
        config.arg(format!(
            "--extra-ldflags=--sysroot=/work/toolchain -B/work/toolchain/usr/bin{sanitizer}"
        ));
        report["undefined_behavior_sanitizer"] = json!(ubsan);
        report["slirp"] = slirp;
        success = step(&mut report, "configure", config)?;
    }
    if success {
        let mut c = Command::new("/samu");
        c.current_dir(build).args(["-j", "2", binary_name]);
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
        fs::write(
            "/work/smoke.stdin",
            b"{\"execute\":\"qmp_capabilities\"}\n{\"execute\":\"quit\"}\n",
        )?;
        for (name, machine) in [
            ("pool_machine_smoke", "pc"),
            ("machine_smoke", "md-sim-s9-zynq-a9"),
            ("private_network_smoke", "md-sim-s9-zynq-a9"),
            ("nand_fixture_smoke", "md-sim-s9-nand-fixture"),
            ("onfi_fixture_smoke", "md-sim-s9-m69a-development"),
            ("s9io_v1_smoke", "md-sim-s9-sd2022-io-development"),
            ("fan_smoke", "md-sim-s9-fan-development"),
            ("fan_input_plan_smoke", "md-sim-s9-fan-development"),
            ("pic16_smoke", "md-sim-s9-pic-development"),
            ("bm1387_smoke", "md-sim-s9-bm1387-development"),
            ("bm1387_temperature_smoke", "md-sim-s9-bm1387-development"),
            (
                "bm1387_temperature_plan_smoke",
                "md-sim-s9-bm1387-development",
            ),
            (
                "bm1387_adapted_filter_smoke",
                "md-sim-s9-bm1387-development",
            ),
            (
                "bm1387_slow_smoke",
                "md-sim-s9-bm1387-development,hash-cap-hps=0.1",
            ),
        ] {
            if pool_x86 != (name == "pool_machine_smoke") {
                continue;
            }
            if !success {
                break;
            }
            let mut smoke = Command::new(build.join(binary_name));
            smoke.args([
                "-machine",
                machine,
                "-accel",
                "tcg,tb-size=32",
                "-m",
                "128M",
                "-smp",
                "2",
                "-nodefaults",
                "-nic",
                "none",
                "-display",
                "none",
                "-serial",
                "null",
                "-serial",
                "null",
                "-qmp",
                "stdio",
                "-S",
            ]);
            if name == "s9io_v1_smoke" {
                smoke.args(["-global", "md-sim-s9io.reference-v1-bm1387=on"]);
            }
            if name == "private_network_smoke" {
                smoke.args(["-netdev", "user,id=private,restrict=on,ipv6=off"]);
            }
            if pool_x86 {
                smoke.args([
                    "-L",
                    "/pool-qemu-data",
                    "-bios",
                    "/pool-qemu-data/bios-256k.bin",
                    "-chardev",
                    "null,id=worker",
                    "-device",
                    "virtio-serial-pci,id=serial",
                    "-device",
                    "virtserialport,bus=serial.0,chardev=worker,name=md-sim.worker",
                ]);
            }
            if name == "fan_smoke" || name == "fan_input_plan_smoke" {
                smoke.args(["-global", "md-sim-fan.reference-motors=on"]);
            }
            if name == "fan_input_plan_smoke" {
                smoke.args(["-global",r#"md-sim-fan.input-plan=[{"at_ns":150000000000,"motor":1,"powered":null,"stalled":true}]"#]);
            }
            if name == "bm1387_adapted_filter_smoke" {
                smoke.args(["-global", "md-sim-s9io.fixed-filter-zero-bits=12"]);
            }
            if name == "bm1387_temperature_smoke" || name == "bm1387_temperature_plan_smoke" {
                smoke.args(["-global", "md-sim-s9io.tmp421-reference=on"]);
            }
            if name == "bm1387_temperature_plan_smoke" {
                smoke.args(["-global", r#"md-sim-s9io.temperature-input-plan=[{"at_ns":300000000,"inputs":{"local_millicelsius":25000,"remote_millicelsius":105000,"diode":"Connected","supply_valid":true}}]"#]);
            }
            if name == "pic16_smoke" || name.starts_with("bm1387_") {
                smoke.args(["-global", "md-sim-s9-pic16.application-text=/pic16-app.txt"]);
            }
            smoke.stdin(std::process::Stdio::from(fs::File::open(
                "/work/smoke.stdin",
            )?));
            success = step(&mut report, name, smoke)?;
        }
    }
    if success {
        let binary = build.join(binary_name);
        let nm = Path::new("/work/toolchain/usr/bin/nm");
        let symbols = Command::new(nm)
            .args(["--numeric-sort", "--defined-only"])
            .arg(&binary)
            .output()?;
        ensure!(
            symbols.status.success()
                && symbols.stdout.len() <= 4 * 1024 * 1024
                && symbols.stderr.is_empty(),
            "backend symbol map failed or exceeded bound"
        );
        report["symbol_map"] = json!({"tool":hash_file(nm)?,
            "unstripped_binary":hash_file(&binary)?,
            "text":String::from_utf8(symbols.stdout)?,
            "scope":"Link addresses before stripping. Timer diagnostic offsets refer to this same build. No raw emulator control endpoint is opened."});
        let mut strip = Command::new("/work/toolchain/usr/bin/strip");
        strip.arg(&binary);
        success = step(&mut report, "strip", strip)?;
        if success {
            let b = fs::read(&binary)?;
            ensure!(b.len() < 28 * 1024 * 1024, "binary export budget");
            report["binary"] = serde_json::to_value(hash_file(&binary)?)?;
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
            encoder.write_all(&b)?;
            let compressed = encoder.finish()?;
            ensure!(
                compressed.len() < 12 * 1024 * 1024,
                "compressed binary export budget"
            );
            report["binary_gzip_hex"] = json!(hex::encode(compressed));
        }
    }
    serde_json::to_writer(std::io::stdout(), &report)?;
    ensure!(success, "QEMU build failed");
    Ok(())
}
fn main() -> Result<()> {
    let mut args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|v| v == "__inside") {
        return inner();
    }
    let core = args.pop().context("core checkpoint argument absent")?;
    let core = PathBuf::from(
        core.to_str()
            .and_then(|arg| arg.strip_prefix("core="))
            .filter(|path| !path.is_empty())
            .context("final argument must be core=CHECKPOINT_DIRECTORY")?,
    );
    ensure!(
        (4..=7).contains(&args.len()) && args.get(5).is_none_or(|a|a == "ubsan")
            && args.get(6).is_none_or(|a|a == "x86-pool"),
        "usage: build_qemu_backend REPO SAMURAI_BUILD_DIRECTORY NEW_OUTPUT [PRIOR_OVERLAY_BUILD [ubsan [x86-pool]]] core=CHECKPOINT_DIRECTORY"
    );
    let repo = PathBuf::from(&args[1]);
    let prior = PathBuf::from(&args[2]);
    let out = PathBuf::from(&args[3]);
    let pool_x86 = args.len() == 7;
    let core_manifest = core.join("manifest.json");
    let model_library = core.join("libmd_sim_qemu_core.a");
    let checkpoint: serde_json::Value = serde_json::from_reader(fs::File::open(&core_manifest)?)?;
    ensure!(
        checkpoint["library"] == serde_json::to_value(hash_file(&model_library)?)?,
        "model library differs from selected checkpoint"
    );
    ensure!(!out.exists(), "new output required");
    fs::create_dir_all(&out)?;
    if let Some(prior_overlay) = args.get(4) {
        reuse_overlay(Path::new(prior_overlay), &out)?;
    } else {
        capture_overlay(&out)?;
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
    qemu_slirp::capture(&store, &mut inputs)?;
    for (guest, host) in [
        (
            "/qemu.tar.gz",
            repo.join("artifacts/files/source/tool-qemu/qemu-11661d857254.tar.gz"),
        ),
        (
            "/legacy-qemu.tar.gz",
            repo.join("artifacts/files/source/tool-espressif-qemu/qemu-febae182e132.tar.gz"),
        ),
        (
            "/pl353.patch",
            repo.join("simulator/backends/qemu/s9-pl353.patch"),
        ),
        (
            "/nand-timing.patch",
            repo.join("simulator/backends/qemu/nand-timing.patch"),
        ),
        ("/toolchain.tar.gz", prior.join("toolchain.tar.gz")),
        ("/overlay.tar.gz", out.join("overlay.tar.gz")),
        ("/keycodes.tar.gz", out.join("keycodes.tar.gz")),
        (
            "/cpu-reset.patch",
            repo.join("simulator/backends/qemu/s9-cpu-reset.patch"),
        ),
        (
            "/rom-reset.patch",
            repo.join("simulator/backends/qemu/s9-rom-reset.patch"),
        ),
        (
            "/rom-reset-contract.md",
            repo.join("simulator/backends/qemu/ROM-RESET.md"),
        ),
        (
            "/cpu-timers.patch",
            repo.join("simulator/backends/qemu/s9-cpu-timers.patch"),
        ),
        (
            "/ttc-deadline.patch",
            repo.join("simulator/backends/qemu/s9-ttc-deadline.patch"),
        ),
        (
            "/ttc-clock.patch",
            repo.join("simulator/backends/qemu/s9-ttc-clock.patch"),
        ),
        (
            "/ttc-reset.patch",
            repo.join("simulator/backends/qemu/s9-ttc-reset.patch"),
        ),
        (
            "/ttc-modes.patch",
            repo.join("simulator/backends/qemu/s9-ttc-modes.patch"),
        ),
        (
            "/ttc-decrement.patch",
            repo.join("simulator/backends/qemu/s9-ttc-decrement.patch"),
        ),
        (
            "/ttc-clock-contract.md",
            repo.join("simulator/backends/qemu/TTC-CLOCK.md"),
        ),
        (
            "/timer-diagnostics.patch",
            repo.join("simulator/backends/qemu/s9-timer-diagnostics.patch"),
        ),
        (
            "/cpu-timers-contract.md",
            repo.join("simulator/backends/qemu/CPU-TIMERS.md"),
        ),
        (
            "/backend.patch",
            repo.join("simulator/backends/qemu/s9-axi-iic.patch"),
        ),
        (
            "/ps-gpio.patch",
            repo.join("simulator/backends/qemu/s9-ps-gpio.patch"),
        ),
        (
            "/ps-gpio-contract.md",
            repo.join("simulator/backends/qemu/PS-GPIO.md"),
        ),
        (
            "/md-sim-ps-gpio.c",
            repo.join("simulator/backends/qemu/md-sim-ps-gpio.c"),
        ),
        (
            "/md-sim-ps-gpio.h",
            repo.join("simulator/backends/qemu/md-sim-ps-gpio.h"),
        ),
        ("/libmd_sim_qemu_core.a", model_library),
        ("/core-checkpoint.json", core_manifest),
        (
            "/pcap.patch",
            repo.join("simulator/backends/qemu/s9-pcap.patch"),
        ),
        (
            "/pcap-contract.md",
            repo.join("simulator/backends/qemu/PCAP.md"),
        ),
        (
            "/md-sim-pcap.h",
            repo.join("simulator/backends/qemu/md-sim-pcap.h"),
        ),
        (
            "/fabric.patch",
            repo.join("simulator/backends/qemu/s9-fabric.patch"),
        ),
        (
            "/fabric-contract.md",
            repo.join("simulator/backends/qemu/FABRIC.md"),
        ),
        (
            "/swdt.patch",
            repo.join("simulator/backends/qemu/s9-swdt.patch"),
        ),
        (
            "/swdt-contract.md",
            repo.join("simulator/backends/qemu/SWDT.md"),
        ),
        (
            "/md-sim-swdt.c",
            repo.join("simulator/backends/qemu/md-sim-swdt.c"),
        ),
        (
            "/md-sim-swdt.h",
            repo.join("simulator/backends/qemu/md-sim-swdt.h"),
        ),
        (
            "/legacy-io.patch",
            repo.join("simulator/backends/qemu/s9-legacy-io.patch"),
        ),
        (
            "/md-sim-s9io.c",
            repo.join("simulator/backends/qemu/md-sim-s9io.c"),
        ),
        (
            "/md-sim-s9io.h",
            repo.join("simulator/backends/qemu/md-sim-s9io.h"),
        ),
        (
            "/s9io-v1-contract.md",
            repo.join("simulator/backends/qemu/S9IO-V1.md"),
        ),
        (
            "/gpio.patch",
            repo.join("simulator/backends/qemu/s9-gpio.patch"),
        ),
        (
            "/sd2022-io.patch",
            repo.join("simulator/backends/qemu/s9-sd2022-io.patch"),
        ),
        (
            "/md-sim-axi-gpio.c",
            repo.join("simulator/backends/qemu/md-sim-axi-gpio.c"),
        ),
        (
            "/md-sim-axi-gpio.h",
            repo.join("simulator/backends/qemu/md-sim-axi-gpio.h"),
        ),
        (
            "/fan.patch",
            repo.join("simulator/backends/qemu/s9-fan.patch"),
        ),
        (
            "/fan-contract.md",
            repo.join("simulator/backends/qemu/FAN.md"),
        ),
        (
            "/md-sim-fan.c",
            repo.join("simulator/backends/qemu/md-sim-fan.c"),
        ),
        (
            "/md-sim-fan.h",
            repo.join("simulator/backends/qemu/md-sim-fan.h"),
        ),
        (
            "/pic16.patch",
            repo.join("simulator/backends/qemu/s9-pic16.patch"),
        ),
        (
            "/bm1387.patch",
            repo.join("simulator/backends/qemu/s9-bm1387.patch"),
        ),
        (
            "/bm1387-contract.md",
            repo.join("simulator/backends/qemu/BM1387.md"),
        ),
        (
            "/bm1387-core-map.json",
            repo.join("simulator/backends/qemu/bm1387-reference-core-map.json"),
        ),
        (
            "/tmp421-profile.json",
            repo.join("simulator/backends/qemu/s9-tmp421-reference.json"),
        ),
        (
            "/tmp421-contract.md",
            repo.join("simulator/backends/qemu/TMP421.md"),
        ),
        (
            "/pic16-contract.md",
            repo.join("simulator/backends/qemu/PIC16.md"),
        ),
        (
            "/md-sim-pic16.c",
            repo.join("simulator/backends/qemu/md-sim-pic16.c"),
        ),
        (
            "/md-sim-pic16.h",
            repo.join("simulator/backends/qemu/md-sim-pic16.h"),
        ),
        (
            "/private-stream.patch",
            repo.join("simulator/backends/qemu/private-stream.patch"),
        ),
        (
            "/pic16-app.txt",
            repo.join("simulator/firmware/pic16/braiins-s9-app.txt"),
        ),
    ] {
        let object = hash_file(&host)?;
        store.import(&host, &object)?;
        inputs.push(Mount {
            object,
            guest_path: guest.into(),
        });
    }
    if pool_x86 {
        for name in ["bios-256k.bin", "linuxboot_dma.bin", "kvmvapic.bin"] {
            let path = Path::new("/usr/share/qemu").join(name);
            let object = hash_file(&path)?;
            store.import(&path, &object)?;
            inputs.push(Mount {
                object,
                guest_path: format!("/pool-qemu-data/{name}"),
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
        argv: vec!["/probe".into(), "__inside".into()],
        inputs,
        stdin_object: None,
        environment: BTreeMap::from([
            (
                "MD_SIM_QEMU_UBSAN".into(),
                if args.len() >= 6 {
                    "1".into()
                } else {
                    "0".into()
                },
            ),
            (
                "MD_SIM_QEMU_POOL_X86".into(),
                if pool_x86 { "1" } else { "0" }.into(),
            ),
            ("PATH".into(), "/work/toolchain/usr/bin:/usr/bin".into()),
            ("PYTHONHOME".into(), "/work/toolchain/usr".into()),
            ("PYTHONHASHSEED".into(), "0".into()),
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
            // The combined Rust standard runtime plus UBSan exceeded64MiB
            // before stripping in build035. Keep a bounded link allowance;
            // the separate28MiB exported executable bound remains unchanged.
            file_bytes: confinement::EMULATOR_BUILD_FILE_BYTES,
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
                compressed.len() < 12 * 1024 * 1024,
                "compressed binary input budget"
            );
            let mut b = Vec::new();
            flate2::read::GzDecoder::new(compressed.as_slice())
                .take(28 * 1024 * 1024 + 1)
                .read_to_end(&mut b)?;
            ensure!(
                b.len() < 28 * 1024 * 1024 && Some(b.len() as u64) == r["binary"]["size"].as_u64(),
                "decoded binary size differs"
            );
            ensure!(
                md_sim_firmware::prepare::hash_bytes(&b) == r["binary"]["sha256"],
                "binary identity mismatch"
            );
            fs::write(
                out.join(if pool_x86 {
                    "qemu-system-x86_64"
                } else {
                    "qemu-system-arm"
                }),
                b,
            )?;
            r.as_object_mut().unwrap().remove("binary_gzip_hex");
        }
        save_json(&out.join("build-report.json"), &r)?;
    }
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated,
        "isolated QEMU build failed; evidence retained"
    );
    Ok(())
}

#[cfg(test)]
mod ps_gpio_selection_tests {
    use super::*;
    #[test]
    fn support_target_does_not_require_arm_tree_or_patch_tool() {
        let temp = tempfile::tempdir().unwrap();
        let absent = temp.path().join("no-arm-source-tree");
        let mut report = json!({"sentinel": 17});
        assert!(patch_ps_gpio(&absent, &mut report, true).unwrap());
        assert_eq!(report, json!({"sentinel": 17}));
        assert!(!absent.exists());
        assert!(patch_ps_gpio(&absent, &mut report, false).is_err());
        std::fs::create_dir_all(absent.join("hw/arm")).unwrap();
        std::fs::write(absent.join("hw/arm/xilinx_zynq.c"), b"changed source").unwrap();
        assert!(patch_ps_gpio(&absent, &mut report, false).is_err());
        assert_eq!(report, json!({"sentinel": 17}));
    }
}

//! Pinned QEMU source inspection and isolated backend preparation for S9 models.
use anyhow::{ensure, Context, Result};
#[path = "../src/confinement.rs"]
mod confinement;
use md_sim_runtime::{
    isolation::{execute, save_json, Limits, Mount, ProcessSpec},
    store::{hash_file, Store},
};
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};

const ARCHIVE: &str = "artifacts/files/source/tool-qemu/qemu-11661d857254.tar.gz";
const SOURCE_HASH: &str = "2a40e6705e59fd8807012631bc6b2c0877f10481b12768f78e4f9d7b88376cce";
const PREFIX: &str = "qemu-11661d857254b27fca5568c6deacf4f6b6f3460f/";

const KERNEL_ARCHIVE: &str = "artifacts/files/source/bitmain-source-kernel-soc/master";
const KERNEL_HASH: &str = "5aa5c0915c93c6078984b13367edd12670e8ad25d922d352654f768d7005db8f";
const UBOOT_ARCHIVE: &str =
    "artifacts/files/source/bitmain-source-u-boot-soc/20170516_support_sub-page_nand";
const UBOOT_HASH: &str = "647dae6a8c07b52f209f753b7898c7a0fd5e1d9b6dffe3addfd11416066c6cba";
const BRAIINS_ARCHIVE: &str = "artifacts/files/source/braiins-legacy-bosminer-fork/bos-devel";
const BRAIINS_HASH: &str = "dc5f58721f596d2fc36b50d5049461264986ac339c4b4ff4d383749b98f3a363";
const IMAGE_HASH: &str = "f85eba975b58b4333e99a656ad661667cbedaf3111af3deb8572b902b88cd276";
const ESP_ARCHIVE: &str = "artifacts/files/source/tool-espressif-qemu/qemu-febae182e132.tar.gz";
const ESP_HASH: &str = "5456034eb8a9f348b1f4c8820dc79f55677bddfe0d1c66566b6c42ceb9df66f0";
const ESP_MINER_ARCHIVE: &str = "artifacts/files/source/open-bitaxeorg-esp-miner-release-source-351118297/ESP-Miner-64680f8a4da0b9a3b532051f0aa18429fcf04e82.tar.gz";
const ESP_MINER_HASH: &str = "ccba9a695c3661a7d536cd649c0dcd6935cb0f9d59d24ff667391d65398ce069";
const GEKKO_ARCHIVE: &str =
    "artifacts/files/source/open-kanoi-cgminer-source/cgminer-d689d3a9057e.tar.gz";
const GEKKO_HASH: &str = "d90dd77a58bea2dc04d0a4f2afd5750d10d79943d391d5f7629fd32e3b15aed0";
const DCENT_ARCHIVE: &str = "artifacts/files/source/source-dcentraltech-dcent-os/main";
const DCENT_HASH: &str = "5501e53fc05648ce4a03da2b47ba799c0fe1e0c6dd4e358db5a18504d419f25c";
const EXTRA_SOURCES: &[(&str, &str, &str, &str)] = &[
    (
        "sri-stratum",
        "artifacts/files/source/open-stratum-mining-stratum-source/stratum-31f80744907c.tar.gz",
        "42d1742ca392944fe1ca1462bb65bbbb2544877dd0e80e33c74a7294aa448233",
        "stratum-31f80744907c6e21f4ae491ca6a0d86797bce8b0/",
    ),
    (
        "slush-mining",
        "artifacts/files/source/open-slush0-stratum-mining-source/stratum-mining-b2a24d742478.tar.gz",
        "7de97fcbc2f3da0e5a65a7ad673c05a6bbe2f9cda9148726882f6164e12074f5",
        "stratum-mining-b2a24d7424784cada95010232cdb79cfed481da6/",
    ),
    (
        "slush-extensions",
        "artifacts/files/source/open-slushpool-stratumprotocol-source/stratumprotocol-04679d0a85e7.tar.gz",
        "44325c134f86f94b84102a43f38d4a313ccb52084e76d4d2d08022b954aeaab4",
        "stratumprotocol-04679d0a85e7fb0f64b9995d7192c966aa03466d/",
    ),
    (
        "bitcoin-bips",
        "artifacts/files/source/open-bitcoin-bips-source/bips-927b6de9915c.tar.gz",
        "f46ae5c782077fd37eb68d90ce7268a1dfa1137f744382db58deca352224b922",
        "bips-927b6de9915c9262615a6399de51b200f81e5aa4/",
    ),
    (
        "bmminer-mix",
        "artifacts/files/source/bitmain-source-bmminer-mix/master",
        "47191dc1e2b90e7b3fe6b1e8c2e363800bca24ad46ac2ee6bd06bbed254fc5a2",
        "bmminer-mix-master/",
    ),
    (
        "single-board-test",
        "artifacts/files/source/bitmain-source-single-board-test/master",
        "624b757759055f309c86429dcf3f5e1f29afb47896e7f15bd5030c47c2ccf8db",
        "single-board-test-master/",
    ),
    (
        "braiins-io",
        "artifacts/files/source/source-braiins-zynq-io/master",
        "d2fd4a51f1966fcf7161aa02f6ca838e8594694e5eb46e2fa3a6d4d952831010",
        "zynq-io-master/",
    ),
    (
        "braiins-io-bin",
        "artifacts/files/source/source-braiins-zynq-io-bin/bos-release",
        "1d73e72fdc56a55eddce77d7e63e24fc62b231a57c38f744c5f26bc5ea612ba6",
        "zynq-io-bin-bos-release/",
    ),
    (
        "braiins-docs",
        "artifacts/files/source/braiins-legacy-docs/master",
        "f8a6ac2d7764f068004d61b9dc0e7cf1dfad64c470cdef4a7be997082a03a5c6",
        "braiins-os-docs-master/",
    ),
];
fn inspect_image() -> Result<()> {
    ensure!(
        hash_file(Path::new("/source.tar.gz"))?.sha256 == IMAGE_HASH,
        "kernel image differs"
    );
    let image = fs::read("/source.tar.gz")?;
    ensure!(image.len() < 8 * 1024 * 1024, "compressed kernel bound");
    let offsets: Vec<_> = image
        .windows(6)
        .enumerate()
        .filter_map(|(i, b)| (b == [0xfd, 0x37, 0x7a, 0x58, 0x5a, 0]).then_some(i))
        .take(17)
        .collect();
    ensure!(offsets.len() <= 16, "XZ candidate bound");
    let mut candidates = Vec::new();
    let mut signatures = BTreeMap::new();
    for (name, signature) in [
        ("xz", &[0xfd, 0x37, 0x7a, 0x58, 0x5a, 0][..]),
        (
            "lzo",
            &[0x89, 0x4c, 0x5a, 0x4f, 0, 0x0d, 0x0a, 0x1a, 0x0a][..],
        ),
        ("lz4", &[0x02, 0x21, 0x4c, 0x18][..]),
        ("lzma", &[0x5d, 0, 0, 0][..]),
    ] {
        let matches: Vec<_> = image
            .windows(signature.len())
            .enumerate()
            .filter_map(|(offset, bytes)| (bytes == signature).then_some(offset))
            .take(32)
            .collect();
        signatures.insert(name, matches);
    }
    for offset in offsets {
        fs::write("/work/kernel.xz", &image[offset..])?;
        let mut child = std::process::Command::new("/xz")
            .args([
                "--decompress",
                "--stdout",
                "--single-stream",
                "--memlimit-decompress=128MiB",
                "/work/kernel.xz",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()?;
        let mut bytes = Vec::new();
        let read = child
            .stdout
            .take()
            .context("XZ stdout")?
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes);
        if read.is_err() || bytes.len() > 64 * 1024 * 1024 {
            let _ = child.kill();
            let _ = child.wait();
            read?;
            anyhow::bail!("kernel expansion bound");
        }
        if !child.wait()?.success() {
            continue;
        }
        ensure!(bytes.len() <= 64 * 1024 * 1024, "kernel expansion bound");
        let mut strings = Vec::new();
        let mut start = 0;
        for end in 0..=bytes.len() {
            if end == bytes.len() || !(bytes[end].is_ascii_graphic() || bytes[end] == b' ') {
                if end - start >= 8 && end - start <= 512 {
                    let s = std::str::from_utf8(&bytes[start..end])?;
                    if s.contains("pl35") || s.contains("Linux version") {
                        ensure!(strings.len() < 256, "kernel string count");
                        strings.push(json!({"offset":start,"text":s}));
                    }
                }
                start = end + 1;
            }
        }
        let pattern = [
            0, 0, 0, 0, 0x30, 0, 0, 0, 5, 1, 0, 0, 5, 0, 0, 0, 0xe0, 0, 0, 0, 2, 1, 0, 0,
        ];
        let tables: Vec<_> = bytes
            .windows(pattern.len())
            .enumerate()
            .filter(|(_, b)| *b == pattern)
            .take(16)
            .map(|(i, _)| {
                    // The unchanged guest prints .text at c0008000; retain that
                    // observed mapping separately from source-build identity.
                    let address = 0xc0008000u32 + i as u32;
                    let references: Vec<_> = bytes.chunks_exact(4).enumerate()
                        .filter_map(|(at, word)| (word == address.to_le_bytes()).then_some(at * 4))
                        .take(16).collect();
                    json!({"offset":i,"hex":hex::encode(&bytes[i..(i+144).min(bytes.len())]),"observed_text_base":0xc0008000u32,"address":address,"literal_reference_offsets":references})
            })
            .collect();
        candidates.push(json!({"compressed_offset":offset,"expanded_sha256":md_sim_firmware::prepare::hash_bytes(&bytes),"expanded_bytes":bytes.len(),"driver_strings":strings,"reference_command_table_matches":tables}));
    }
    serde_json::to_writer(
        std::io::stdout(),
        &json!({"original_kernel_sha256":IMAGE_HASH,"candidates":candidates,"compression_signature_offsets":signatures,"claim":"Static bounded inspection only; signatures are hypotheses and a matching reference table does not establish the source build identity."}),
    )?;
    Ok(())
}
fn inspect_inside(kind: &str) -> Result<()> {
    let (source_hash, prefix) = match kind {
        "kernel" => (KERNEL_HASH, "Kernel_SOC-master/"),
        "uboot" => (UBOOT_HASH, "u-boot-soc-20170516_support_sub-page_nand/"),
        "braiins" => (BRAIINS_HASH, "braiins-bos-devel/"),
        "esp" => (ESP_HASH, "qemu-febae182e132e4055529be423a818225ebddaa3a/"),
        "esp-miner" => (
            ESP_MINER_HASH,
            "ESP-Miner-64680f8a4da0b9a3b532051f0aa18429fcf04e82/",
        ),
        "gekko" => (
            GEKKO_HASH,
            "cgminer-d689d3a9057e8852de51d5c7f402ce26cbceaea5/",
        ),
        "dcent" => (DCENT_HASH, "DCENT_OS-main/"),
        _ => EXTRA_SOURCES
            .iter()
            .find(|source| source.0 == kind)
            .map_or((SOURCE_HASH, PREFIX), |source| (source.2, source.3)),
    };
    ensure!(
        hash_file(Path::new("/source.tar.gz"))?.sha256 == source_hash,
        "archive hash differs"
    );
    let mut archive = tar::Archive::new(
        flate2::read::GzDecoder::new(fs::File::open("/source.tar.gz")?).take(1024 * 1024 * 1024),
    );
    let mut texts = BTreeMap::new();
    let mut inventory = vec![];
    for (index, entry) in archive.entries()?.enumerate() {
        ensure!(index < 100_000, "archive entry budget");
        let mut entry = entry?;
        let name = entry.path()?.into_owned();
        let name = name.to_str().context("archive path encoding")?;
        let Some(relative) = name.strip_prefix(prefix) else {
            continue;
        };
        if EXTRA_SOURCES.iter().any(|source| source.0 == kind)
            || relative.starts_with("hw/i2c/")
            || relative.starts_with("hw/sensor/")
            || relative.starts_with("hw/watchdog/")
            || relative.starts_with("include/hw/watchdog/")
            || relative.starts_with("python/wheels/")
            || relative.starts_with("subprojects/")
            || relative.starts_with("include/hw/i2c/")
            || relative.contains("zynq_nand")
            || relative.contains("pl353")
            || relative.to_lowercase().contains("nand")
            || (kind == "dcent"
                && relative.starts_with("DCENT_OS_Antminer/dcentrald/dcentrald-asic/"))
            || (kind == "braiins"
                && (relative.starts_with("braiins-os/")
                    || relative.starts_with("open/hw/zynq-io-am1-s9/")
                    || relative.starts_with("open/bosminer/bosminer-am1-s9/")))
            || (kind == "esp-miner"
                && (relative.starts_with("components/asic/")
                    || relative.contains("create_jobs")
                    || relative.contains("mining")
                    || relative.contains("utils")))
        {
            inventory.push(json!({"path":relative,"size":entry.size(),"type":entry.header().entry_type().as_byte()}));
        }
        if matches!(kind, "braiins-io" | "braiins-io-bin") && entry.header().entry_type().is_file()
        {
            ensure!(entry.size() <= 8 * 1024 * 1024, "bitstream member bound");
            let mut bytes = vec![];
            entry.read_to_end(&mut bytes)?;
            let identity = inventory.last_mut().context("inventory entry absent")?;
            identity["sha256"] = json!(md_sim_firmware::prepare::hash_bytes(&bytes));
            identity["first128_hex"] = json!(hex::encode(&bytes[..bytes.len().min(128)]));
            if kind == "braiins-io" || relative == ".gitmodules" {
                match String::from_utf8(bytes) {
                    Ok(text) => {
                        texts.insert(
                            relative.to_string(),
                            json!({"sha256":identity["sha256"],"text":text}),
                        );
                    }
                    Err(_) => identity["text_encoding"] = json!("binary-or-non-UTF-8"),
                }
            }
            continue;
        }
        if [
            "sv1/src/error.rs",
            "sv1/src/json_rpc.rs",
            "sv1/src/lib.rs",
            "sv1/src/methods/client_to_server.rs",
            "mining/service.py",
            "mining/interfaces.py",
            "lib/exceptions.py",
            "lib/template_registry.py",
            "stratum-extensions.mediawiki",
            "driver-bitmain.c",
            "bip-0310.mediawiki",
            "bip-0320.mediawiki",
            "driver-bitmain.h",
            "driver-btm-c5.c",
            "driver-btm-c5.h",
            "clement-bitmain.c",
            "main.c",
            "miner_type.h",
            "DCENT_OS_Antminer/dcentrald/dcentrald-asic/src/drivers/bm1387.rs",
            "DCENT_OS_Antminer/dcentrald/dcentrald-asic/src/bm1387.rs",
            "DCENT_OS_Antminer/dcentrald/dcentrald-asic/src/work_tx.rs",
            "DCENT_OS_Antminer/dcentrald/dcentrald-asic/src/protocol.rs",
            "DCENT_OS_Antminer/dcentrald/dcentrald-asic/tests/bm1387_beta_wire_replay.rs",
            "DCENT_OS_Antminer/dcentrald/dcentrald-asic/src/pic1704/protocol.rs",
            "DCENT_OS_Antminer/dcentrald/dcentrald-hal/src/fpga_chain.rs",
            "DCENT_OS_Antminer/dcentrald/dcentrald-hal/src/fan.rs",
            "DCENT_OS_Antminer/dcentrald/dcentrald-asic/src/pic/mod.rs",
            "DCENT_OS_Antminer/dcentrald/dcentrald-asic/src/pic/braiinsos.rs",
            "DCENT_OS_Antminer/dcentrald/dcentrald-asic/src/pic/stock.rs",
            "DCENT_OS_Antminer/dcentrald/dcentrald-asic/tests/sim_pic16_runtime.rs",
            "DCENT_OS_Antminer/docs/DCENTRALD_ARCHITECTURE.md",
            "DCENT_OS_Antminer/br2_external_dcentos/board/zynq/rootfs-overlay/root/tools/asic_init.py",
            "braiins-os-en/source/index.rst",
            "DCENT_OS_Antminer/dcentrald/dcentrald-asic/src/drivers/bm1397.rs",
            "DCENT_OS_Antminer/dcentrald/dcentrald-common/src/bm1397_s17_bhb07601.rs",
            "DCENT_OS_Antminer/dcentrald/dcentrald-silicon-profiles/src/bm1397.rs",
            "DCENT_OS_ESP/dcentaxe-asic/src/bm1397.rs",
            "driver-gekko.c",
            "driver-gekko.h",
            "components/asic/bm1397.c",
            "components/asic/bm1397.h",
            "components/asic/include/bm1397.h",
            "components/asic/crc.c",
            "components/asic/crc.h",
            "components/asic/include/crc.h",
            "components/asic/asic_common.c",
            "components/asic/serial.c",
            "main/adc.c",
            "main/adc.h",
            "main/Kconfig.projbuild",
            "components/asic/asic_common.h",
            "components/asic/include/asic_common.h",
            "components/asic/pll.c",
            "components/asic/include/pll.h",
            "components/asic/test/test_pll.c",
            "components/stratum/mining.c",
            "components/stratum/mining.h",
            "components/stratum/include/mining.h",
            "components/stratum/utils.c",
            "components/stratum/include/utils.h",
            "main/asic_task.c",
            "main/create_jobs_task.c",
            "system/qtest.c",
            "hw/sensor/tmp421.c",
            "hw/sensor/meson.build",
            "hw/i2c/core.c",
            "include/hw/i2c/i2c.h",
            "net/slirp.c",
            "net/stream.c",
            "net/socket.c",
            "qapi/net.json",
            "hw/sd/sd.c",
            "block/block-backend.c",
            "block/io.c",
            "system/memory.c",
            "accel/tcg/cputlb.c",
            "util/qemu-timer.c",
            "system/cpu-timers.c",
            "system/cpus.c",
            "accel/tcg/tcg-accel-ops.c",
            "accel/tcg/tcg-accel-ops-rr.c",
            "block/qcow2-cache.c",
            "qemu-img.c",
            "hw/i386/Kconfig",
            "hw/net/Kconfig",
            "configs/devices/i386-softmmu/default.mak",
            "configs/devices/x86_64-softmmu/default.mak",
            "net/stream_data.c",
            "net/stream_data.h",
            "io/channel-socket.c",
            "util/qemu-sockets.c",
            "chardev/char-socket.c",
            "qemu-options.hx",
            "hw/intc/arm_gic.c",
            "hw/intc/arm_gic_common.c",
            "hw/intc/arm_gic_internal.h",
            "hw/cpu/a9mpcore.c",
            "include/hw/cpu/a9mpcore.h",
            "hw/timer/arm_mptimer.c",
            "include/hw/timer/arm_mptimer.h",
            "hw/timer/a9gtimer.c",
            "include/hw/timer/a9gtimer.h",
            "hw/timer/cadence_ttc.c",
            "include/hw/timer/cadence_ttc.h",
            "include/hw/core/clock.h",
            "drivers/clocksource/cadence_ttc_timer.c",
            "drivers/watchdog/cadence_wdt.c",
            "include/hw/clock.h",
            "include/hw/core/clock.h",
            "include/hw/core/ptimer.h",
            "hw/core/ptimer.c",
            "include/hw/core/qdev-clock.h",
            "accel/qtest/qtest.c",
            "accel/tcg/icount-common.c",
            "util/qemu-timer.c",
            "braiins-os/configs/default.yml",
            "open/hw/zynq-io-am1-s9/README.md",
            "open/hw/zynq-io-am1-s9/design/system.tcl",
            "open/hw/zynq-io-am1-s9/fpga-io/map.svd",
            "open/hw/zynq-io-am1-s9/design/src/s9io_0.2/hdl/s9io_core.vhd",
            "open/hw/zynq-io-am1-s9/design/src/s9io_0.2/hdl/s9io_v0_2_S00_AXI.vhd",
            "open/hw/zynq-io-am1-s9/design/src/s9io_0.2/hdl/uart.vhd",
            "open/hw/zynq-io-am1-s9/design/src/s9io_0.2/hdl/uart_rx.vhd",
            "open/hw/zynq-io-am1-s9/design/src/s9io_0.2/hdl/uart_tx.vhd",
            "open/hw/zynq-io-am1-s9/design/src/s9io_0.2/hdl/mod_divider.vhd",
            "open/hw/zynq-io-am1-s9/design/src/s9io_0.2/hdl/fifo_block.vhd",
            "open/hw/zynq-io-am1-s9/design/src/s9io_0.2/hdl/fifo_block_thr.vhd",
            "open/hw/zynq-io-am1-s9/design/src/s9io_0.2/hdl/fifo_distr.vhd",
            "open/hw/zynq-io-am1-s9/design/src/s9io_0.2/hdl/crc5_serial.vhd",
            "open/hw/zynq-io-am1-s9/design/src/s9io_0.2/hdl/crc5_resp_serial.vhd",
            "open/hw/zynq-io-am1-s9/design/src/s9io_0.2/hdl/crc16_serial.vhd",
            "open/hw/zynq-io-am1-s9/design/src/s9io_0.2/fve/s9io_tb.sv",
            "open/hw/zynq-io-am1-s9/design/src/s9io_0.2/fve/s9io_pkg.sv",
            "open/bosminer/bosminer-am1-s9/src/io.rs",
            "open/bosminer/bosminer-am1-s9/src/io/ext_work_id.rs",
            "open/bosminer/bosminer-am1-s9/src/io/uio.rs",
            "open/bosminer/bosminer-am1-s9/src/lib.rs",
            "open/bosminer/bosminer-am1-s9/src/null_work.rs",
            "open/bosminer/bosminer-am1-s9/src/test/work_generation.rs",
            "open/bosminer/bosminer/src/work.rs",
            "open/coins/bitcoin/src/lib.rs",
            "open/coins/bitcoin/src/test_blocks.rs",
            "open/coins/bitcoin/tests/hash_test.rs",
            "open/bosminer/bosminer-am1-s9/src/command.rs",
            "open/bosminer/bosminer-am1-s9/src/bm1387.rs",
            "open/bosminer/bosminer-am1-s9/src/bm1387/i2c.rs",
            "open/bosminer/bosminer-am1-s9/src/i2c.rs",
            "open/bosminer/bosminer-am1-s9/src/halt.rs",
            "open/bosminer/bosminer-am1-s9/src/monitor.rs",
            "open/bosminer/bosminer-am1-s9/src/sensor.rs",
            "open/bosminer/bosminer-am1-s9/src/sensor/tmp42x.rs",
            "open/bosminer/bosminer-am1-s9/src/sensor/tmp451.rs",
            "open/bosminer/bosminer-am1-s9/src/power.rs",
            "open/bosminer/bosminer-am1-s9/src/power/firmware.rs",
            "braiins-os/package/mining/bosminer/files/hash_s8_app.txt",
            "braiins-os/package/mining/bmminer/files/hash_s8_app.txt",
            "open/bosminer/bosminer-am1-s9/src/fan.rs",
            "open/bosminer/bosminer-am1-s9/src/gpio.rs",
            "braiins-os/package/system/uboot-miner/patches/202-zynq_nand.patch",
            "braiins-os/package/system/uboot-miner/patches/206-zynq_nand_spl.patch",
            "drivers/mtd/nand/zynq_nand.c",
            "drivers/mtd/nand/pl353_nand.c",
            "drivers/memory/pl353-smc.c",
            "drivers/memory/pl35x-smc.c",
            "drivers/clk/zynq/clkc.c",
            "drivers/gpio/gpio-xilinx.c",
            "drivers/clk/clk-divider.c",
            "drivers/char/xilinx_devcfg.c",
            "hw/dma/xlnx-zynq-devcfg.c",
            "include/hw/dma/xlnx-zynq-devcfg.h",
            "drivers/clk/zynq/pll.c",
            "drivers/reset/reset-zynq.c",
            "drivers/mtd/nand/pl35x_nand.c",
            "mm/vmalloc.c",
            "arch/arm/mm/ioremap.c",
            "arch/arm/include/asm/vmalloc.h",
            "arch/arm/include/asm/io.h",
            "include/linux/vmalloc.h",
            "arch/arm/boot/dts/zynq-7000.dtsi",
            "include/configs/zynq-common.h",
            "arch/arm/cpu/armv7/zynq/slcr.c",
            "hw/block/nand.c",
            "blockdev.c",
            "hw/core/qdev-properties-system.c",
            "hw/core/qdev.c",
            "hw/core/clock.c",
            "include/hw/core/clock.h",
            "include/hw/core/qdev-clock.h",
            "system/vl.c",
            "system/qtest.c",
            "hw/block/nandcore.c",
            "hw/block/nand_ecc.c",
            "hw/block/nandchip.c",
            "hw/misc/nand.c",
            "include/hw/block/nand.h",
            "hw/char/cadence_uart.c",
            "hw/net/cadence_gem.c",
            "hw/timer/cadence_ttc.c",
            "net/socket.c",
            "util/qemu-timer.c",
            "hw/misc/Kconfig",
            "hw/core/Kconfig",
            "include/hw/block/flash.h",
            "target/arm/internals.h",
            "target/arm/cpu.h",
            "arch/arm/mach-zynq/platsmp.c",
            "arch/arm/mach-zynq/slcr.c",
            "arch/arm/mach-zynq/common.c",
            "VERSION",
            "configure",
            "meson.build",
            "pythondeps.toml",
            "python/scripts/mkvenv.py",
            "subprojects/meson.wrap",
            "subprojects/dtc.wrap",
            "subprojects/keycodemapdb.wrap",
            "ui/meson.build",
            "ui/keymaps.c",
            "ui/input-keymap.c",
            "tests/unit/meson.build",
            "target/arm/arm-powerctl.h",
            "target/arm/arm-powerctl.c",
            "include/hw/misc/zynq_slcr.h",
            "hw/arm/boot.c",
            "include/hw/arm/boot.h",
            "hw/arm/xilinx_zynq.c",
            "hw/i2c/meson.build",
            "hw/i2c/Kconfig",
            "include/hw/i2c/i2c.h",
            "hw/misc/zynq_slcr.c",
            "block.c",
            "util/oslib-posix.c",
            "hw/arm/Kconfig",
            "hw/i2c/trace-events",
        ]
        .contains(&relative)
            || ((relative.starts_with("hw/watchdog/")
                || relative.starts_with("include/hw/watchdog/"))
                && (relative.ends_with(".c")
                    || relative.ends_with(".h")
                    || relative.ends_with("/Kconfig")
                    || relative.ends_with("/meson.build")))
            || relative.ends_with("/nand.c")
            || relative.ends_with("/nand.h")
            || (relative.starts_with("hw/i2c/")
                && (relative.contains("xilinx") || relative.contains("xlnx")))
        {
            ensure!(
                entry.header().entry_type().is_file() && entry.size() < 1024 * 1024,
                "reference member bound"
            );
            let mut bytes = vec![];
            entry.read_to_end(&mut bytes)?;
            texts.insert(relative.to_string(),json!({"sha256":md_sim_firmware::prepare::hash_bytes(&bytes),"text":String::from_utf8(bytes)?}));
        }
    }
    serde_json::to_writer(
        std::io::stdout(),
        &json!({"archive_sha256":source_hash,"inventory":inventory,"text":texts}),
    )?;
    Ok(())
}

fn stage_inside() -> Result<()> {
    confinement::require(true)?;
    ensure!(
        hash_file(Path::new("/source.tar.gz"))?.sha256 == SOURCE_HASH,
        "source differs"
    );
    let mut archive = tar::Archive::new(
        flate2::read::GzDecoder::new(fs::File::open("/source.tar.gz")?).take(1024 * 1024 * 1024),
    );
    let mut total = 0u64;
    for (index, entry) in archive.entries()?.enumerate() {
        ensure!(index < 100_000, "archive entry bound");
        let mut entry = entry?;
        if entry.header().entry_type().is_pax_global_extensions() {
            continue;
        }
        ensure!(entry.size() <= 64 * 1024 * 1024, "archive member bound");
        total = total
            .checked_add(entry.size())
            .context("archive size overflow")?;
        ensure!(total <= 700 * 1024 * 1024, "archive expansion bound");
        ensure!(
            entry.path()?.starts_with(PREFIX.trim_end_matches('/')),
            "unexpected archive root: {} ({:?})",
            entry.path()?.display(),
            entry.header().entry_type()
        );
        ensure!(entry.unpack_in("/work")?, "archive path escape");
    }
    let source = Path::new("/work").join(PREFIX.trim_end_matches('/'));
    let output = std::process::Command::new("/patch")
        .current_dir(&source)
        .args(["-p1", "--batch", "--forward", "--input=/backend.patch"])
        .output()?;
    let mut members = BTreeMap::new();
    for name in [
        "hw/arm/xilinx_zynq.c",
        "hw/arm/Kconfig",
        "hw/i2c/meson.build",
        "hw/i2c/trace-events",
        "hw/i2c/md-sim-axi-iic.c",
        "hw/i2c/md-sim-axi-iic.h",
    ] {
        if source.join(name).exists() {
            members.insert(name, hash_file(&source.join(name))?);
        }
    }
    serde_json::to_writer(
        std::io::stdout(),
        &json!({"source":SOURCE_HASH,"patch":hash_file(Path::new("/backend.patch"))?,"expanded_member_bytes":total,"patch_exit":output.status.code(),"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr),"derived_members":members,"backend_build":"not performed; Ninja prerequisite unresolved"}),
    )?;
    ensure!(output.status.success(), "patch application failed");
    Ok(())
}

fn probe_tool(path: &Path, out: &Path) -> Result<()> {
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(out)?;
    let store = Store::open(&out.join("inputs"), 256 * 1024 * 1024)?;
    let inputs =
        md_sim_firmware::lab::elf_closure(&store, vec![("/tool".into(), path.to_path_buf())])?;
    let spec = ProcessSpec {
        support_virtualization: None,
        private_channel: None,
        stdin_stream: None,
        namespace_uid: 0,
        dashboard: None,
        loopback: false,
        schema_version: "2.0".into(),
        argv: vec!["/tool".into(), "--help".into()],
        inputs,
        environment: BTreeMap::new(),
        stdin_object: None,
        limits: Limits {
            wall_seconds: 10,
            cpu_seconds: 5,
            memory_bytes: 256 * 1024 * 1024,
            processes: 32,
            file_bytes: 1024 * 1024,
            trace_bytes: 1024 * 1024,
            tmp_bytes: 1024 * 1024,
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    fs::write(out.join("stdout.log"), hex::decode(&result.stdout)?)?;
    fs::write(out.join("stderr.log"), hex::decode(&result.stderr)?)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().collect();
    let command = args.get(1).and_then(|arg| arg.to_str());
    let extra = EXTRA_SOURCES
        .iter()
        .find(|source| command.and_then(|s| s.strip_prefix("inspect-")) == Some(source.0));
    if let Some(source) = EXTRA_SOURCES
        .iter()
        .find(|source| command.and_then(|s| s.strip_prefix("__inspect-")) == Some(source.0))
    {
        return inspect_inside(source.0);
    }
    if args.len() == 4 && args[1] == "probe-tool" {
        return probe_tool(Path::new(&args[2]), Path::new(&args[3]));
    }
    if args.get(1).is_some_and(|v| v == "__stage") {
        return stage_inside();
    }
    if args.get(1).is_some_and(|v| v == "__inspect-kernel") {
        return inspect_inside("kernel");
    }
    if args.get(1).is_some_and(|v| v == "__inspect") {
        return inspect_inside("qemu");
    }
    if args.get(1).is_some_and(|v| v == "__inspect-uboot") {
        return inspect_inside("uboot");
    }
    if args.get(1).is_some_and(|v| v == "__inspect-braiins") {
        return inspect_inside("braiins");
    }
    if args.get(1).is_some_and(|v| v == "__inspect-esp") {
        return inspect_inside("esp");
    }
    if args.get(1).is_some_and(|v| v == "__inspect-esp-miner") {
        return inspect_inside("esp-miner");
    }
    if args.get(1).is_some_and(|v| v == "__inspect-gekko") {
        return inspect_inside("gekko");
    }
    if args.get(1).is_some_and(|v| v == "__inspect-dcent") {
        return inspect_inside("dcent");
    }
    if args.get(1).is_some_and(|v| v == "__inspect-image") {
        return inspect_image();
    }
    ensure!(
        args.len() == 4
            && (args[1] == "inspect"
                || args[1] == "inspect-kernel"
                || args[1] == "inspect-uboot"
                || args[1] == "inspect-braiins"
                || args[1] == "inspect-esp"
                || args[1] == "inspect-esp-miner"
                || args[1] == "inspect-gekko"
                || args[1] == "inspect-dcent"
                || args[1] == "inspect-image"
                || extra.is_some()
                || args[1] == "stage"),
        "usage: qemu_s9_backend inspect REPO NEW_OUTPUT"
    );
    let repo = PathBuf::from(&args[2]);
    let out = PathBuf::from(&args[3]);
    ensure!(!out.exists(), "output must be new");
    fs::create_dir_all(&out)?;
    let store = Store::open(&out.join("inputs"), 256 * 1024 * 1024)?;
    let stage = args[1] == "stage";
    let mut programs = vec![("/probe".into(), std::env::current_exe()?)];
    if stage {
        programs.push(("/patch".into(), PathBuf::from("/usr/bin/patch")));
    }
    if args[1] == "inspect-image" {
        programs.push(("/xz".into(), PathBuf::from("/usr/bin/xz")));
    }
    let mut inputs = md_sim_firmware::lab::elf_closure(&store, programs)?;
    if stage {
        let path = repo.join("simulator/backends/qemu/s9-axi-iic.patch");
        let object = hash_file(&path)?;
        store.import(&path, &object)?;
        inputs.push(Mount {
            object,
            guest_path: "/backend.patch".into(),
        });
    }
    let kernel = args[1] == "inspect-kernel";
    let uboot = args[1] == "inspect-uboot";
    let braiins = args[1] == "inspect-braiins";
    let esp = args[1] == "inspect-esp";
    let esp_miner = args[1] == "inspect-esp-miner";
    let gekko = args[1] == "inspect-gekko";
    let dcent = args[1] == "inspect-dcent";
    let image = args[1] == "inspect-image";
    let archive = repo.join(if kernel {
        KERNEL_ARCHIVE
    } else if uboot {
        UBOOT_ARCHIVE
    } else if braiins {
        BRAIINS_ARCHIVE
    } else if esp {
        ESP_ARCHIVE
    } else if esp_miner {
        ESP_MINER_ARCHIVE
    } else if gekko {
        GEKKO_ARCHIVE
    } else if dcent {
        DCENT_ARCHIVE
    } else if image {
        "work/simulator/original-zynq/attempt-008/components/kernel"
    } else if let Some(source) = extra {
        source.1
    } else {
        ARCHIVE
    });
    let object = hash_file(&archive)?;
    ensure!(
        object.sha256
            == if kernel {
                KERNEL_HASH
            } else if uboot {
                UBOOT_HASH
            } else if braiins {
                BRAIINS_HASH
            } else if esp {
                ESP_HASH
            } else if esp_miner {
                ESP_MINER_HASH
            } else if gekko {
                GEKKO_HASH
            } else if dcent {
                DCENT_HASH
            } else if image {
                IMAGE_HASH
            } else if let Some(source) = extra {
                source.2
            } else {
                SOURCE_HASH
            },
        "source differs"
    );
    store.import(&archive, &object)?;
    inputs.push(Mount {
        object,
        guest_path: "/source.tar.gz".into(),
    });
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
            if stage {
                "__stage".into()
            } else if uboot {
                "__inspect-uboot".into()
            } else if kernel {
                "__inspect-kernel".into()
            } else if braiins {
                "__inspect-braiins".into()
            } else if esp {
                "__inspect-esp".into()
            } else if esp_miner {
                "__inspect-esp-miner".into()
            } else if gekko {
                "__inspect-gekko".into()
            } else if dcent {
                "__inspect-dcent".into()
            } else if image {
                "__inspect-image".into()
            } else if let Some(source) = extra {
                format!("__inspect-{}", source.0)
            } else {
                "__inspect".into()
            },
        ],
        inputs,
        environment: BTreeMap::new(),
        stdin_object: None,
        limits: Limits {
            wall_seconds: 90,
            cpu_seconds: 60,
            memory_bytes: 1024 * 1024 * 1024,
            processes: 32,
            file_bytes: 64 * 1024 * 1024,
            trace_bytes: 16 * 1024 * 1024,
            tmp_bytes: if stage {
                768 * 1024 * 1024
            } else {
                16 * 1024 * 1024
            },
        },
    };
    save_json(&out.join("spec.json"), &spec)?;
    let result = execute(&store, &spec, Arc::new(AtomicBool::new(false)))?;
    save_json(&out.join("result.json"), &result)?;
    ensure!(
        result.exit_code == Some(0) && !result.trace_truncated,
        "source inspection did not complete"
    );
    fs::write(out.join("references.json"), hex::decode(&result.stdout)?)?;
    Ok(())
}

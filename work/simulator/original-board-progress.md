# Original board feasibility checkpoint

Date: 19 October 2026.
The original board boot gate is incomplete. No exact hardware target passes this probe.

## Selected archived inputs

The selected record is `bitmain-platform-s9-braiins-22-02-4` in `catalog/parts/bitmain/platform-boot-profiles.jsonl`.
Its archive is `artifacts/files/firmware/braiins-feed-25bba3b4a505ccbc0e54/sd-firmware_2022-04-29-0-5eb2c5fe-22.02.4-plus_arm_cortex-a9_neon.tar`.
The archive has 19,179,888 bytes. Its SHA-256 is `469743ebcd5faf3445e4f4209ad8af58cc7adeff37c7df28af3c53a5b255b56e`.
The selected FIT member is `sysupgrade-am1-s9/fit`. Its SHA-256 is `8b2a32020281aa67490c01633adf99c1a2ca6c9897e4c210bb6d0d001c15caca`.

The Rust probe checks the archive and FIT bytes. It extracts the following original bytes in a private, bounded process. It does not run an archive script.

| Component | FIT byte offset | Size | SHA-256 |
| --- | ---: | ---: | --- |
| ARM kernel (FIT label: Linux 4.4.92) | 228 | 2,827,776 | `f85eba975b58b4333e99a656ad661667cbedaf3111af3deb8572b902b88cd276` |
| SquashFS root image, named ramdisk by the FIT | 2,828,304 | 15,491,980 | `2bd49c70b915b7e25f6c01013d95a5ffea8d499b73c6c6ec5539fda4247154b3` |
| AntMiner S9 device tree | 18,320,556 | 12,364 | `550c87818cb0db41c00a8351dc4ac7b5dce5bf6d5336539c6951163ebf3cb62a` |

The root image starts with SquashFS magic `68 73 71 73`. Attempts 001–003 did not reach a root mount or user-space start. The later adapted diagnostics below reached both.
The complete runtime input set is in each saved process specification. Each file has a content hash and size.
QEMU is version 11.1.2. Its executable has SHA-256 `4b7508549dad5fc48fc80ddd731ce8673825dcedf5d6bdea37b7caf501611242` and 31,097,464 bytes.
This pins the installed backend binary. It does not establish reproducible QEMU source-build provenance.

## Executed commands and results

These commands were run from the repository root. For a new run, replace `attempt-003` with a new output directory.

```sh
cargo run -p md-sim-firmware --example original_zynq_probe --locked --offline -- . work/simulator/original-zynq/attempt-003 single-cpu
cargo clippy -p md-sim-firmware --example original_zynq_probe --locked --offline -- -D warnings
```

The invocation selects only the single-CPU probe. Earlier attempt 002 tested both two-CPU variants. Omitting the final selector will run all three variants; that combined invocation has not been run after the single-CPU option was added.
The Rust example is `crates/md-sim-firmware/examples/original_zynq_probe.rs`.
Its tested source SHA-256 is `8ecbfeab427ebe4af5cd7f61276a8e1444118e7a35b7df68e74bff1ca13416d8`.
Formatting and the lint command passed.

Attempt 001 exposed a bounded export defect: JSON hex expansion exceeded the 32 MiB stdout half of the preparation trace budget. No firmware ran. The export now returns bounded binary component bytes. It saves preparation status, byte count and hash before checking success.

Attempt 002 checked both two-CPU variants. Each backend exited with code 1 before guest entry. The error identifies overlapping generated ROM regions at address zero: `bootloader` and `smpboot`. Empty guest logs are retained. These are failed feasibility probes.
The historical label `original-bootargs` means that `-append earlycon` supplies the same text stored in the DTB. QEMU still sets the chosen property through a direct-loader override. It is not a retained original bootloader argument path.

Attempt 003 used one CPU and an explicit console argument. The archived kernel executed and printed actual UART, device configuration, Ethernet, USB and I2C driver messages. The guest log has SHA-256 `a0aa75fc9570068bca38f74b950d3216060f6cc948854ce769f3a00412191980`.
The process exited with code 137 after 25.568 seconds. This is consistent with the configured 25 CPU-second limit. There is no observed init process, miner service, watchdog success, share, block or dashboard.
This result proves only that archived kernel code ran in an adapted environment on a generic machine. It does not prove a complete firmware boot.

See `original-zynq/attempt-003/component-manifest.json`, `preparation-result.json`, `adapted-console-single-cpu-spec.json`, `adapted-console-single-cpu-result.json`, `adapted-console-single-cpu-guest.log`, and `adaptation-manifest.json` in this directory.
Attempt 002 preserves both original two-CPU failures and their full input specifications.

The strict runtime provides private user, mount, PID, network, IPC and UTS namespaces. It mounts only locked files. The root is read-only. Scratch filesystems and process resources have limits. No NIC has a network peer. QEMU has no monitor or graphical display. No physical miner or host directory is mounted in the guest runtime.
The CPU and memory limits apply per process. This probe makes no aggregate cgroup resource claim.

## Exact blockers and next actions

1. The DTB declares two Cortex-A9 CPUs and 1 GiB of memory. The actual installed RAM and PCB revision are null in the evidence. An older U-Boot source profile declares 512 MiB. These profiles cannot be merged. Obtain an exact controller revision and its measured memory layout before creating an executable exact target.
2. The selected archive includes SPL, U-Boot and FPGA members, but the board reset path, boot ROM behavior, storage placement, environment, NAND geometry and initial persistent contents are not resolved. Verify the original complete boot chain and baseline storage. Do not substitute the direct kernel loader for this work.
3. The DTB declares a PL353 NAND controller at `0xe000e000` and flash at `0xe1000000`. It also declares FPGA GPIO at `0x41200000` and `0x41210000`, I2C at `0x41600000`, a fan interface at `0x42800000`, chain interfaces in `0x43c00000` through `0x43c23000`, and a glitch interface at `0x43d00000`. The generic QEMU machine does not supply a verified miner FPGA model. Implement these models from the archived RTL, driver and board evidence. Unimplemented models are implementation work, not incompatibility evidence.
4. The original chosen console points to `serial9`, but the DTB aliases include only `serial0` at `0xe0001000` and `serial1` at `0x43c70000`. Determine whether the original bootloader edits this value. The explicit console override remains a recorded environment adaptation.
5. The two-CPU generic loader fails before entry. Investigate the selected QEMU machine's secondary boot address handling with pinned backend source. A single-CPU adaptation does not satisfy the original two-CPU profile.
6. The native update script requires firmware `2018-10-24-0-9e5687a2` before upgrade. Its major version is `2020-03-29-0-6ec1a631`. Resolve and verify that baseline and its storage identity before an update/recovery test. The script was read as data, never executed.
7. Exact hashboard revisions, ASIC topology, PSU protocol, EEPROM/calibration data and cooling identities remain unresolved for this controller/firmware combination. Resolve each from evidence. Then run original firmware through real device models, fresh-work pool verification, Core regtest, persistence, fault and native dashboard suites.

Evidence file hashes used for this assessment:

| File | SHA-256 |
| --- | --- |
| `platform-boot-profiles.jsonl` | `a4314b718606ff85476579da1db79bfd5d43032a8888e674e55596236cb4c04e` |
| `platform-fit-properties.jsonl` | `edbb9080168086c5e2552a4cf7db7cfcda2014f09fb3039f142a0c1dfbf9acf9` |
| `platform-fdt-properties.jsonl` | `90216e80e6309e8f93810b9e44cc8888beada4f8f5368e8b975796eb1af8d04f` |
| `platform-image-evidence.jsonl` | `b587c9db8bc57e1fad4a31a1c8460f1466aa4ffde2a285424763232c28b3c281` |
| Selected archive's extracted `COMMAND` | `82b03ae21d63222d070c48cee66dcf29c0855b98e4b80bf5cd45ec98eded0f7c` |

The four platform evidence files are under `catalog/parts/bitmain/`.
`COMMAND` is under `work/bitmain/platform-evidence/images/braiins-feed-25bba3b4a505ccbc0e54/`.
No catalogue pass status was changed by this probe.

## Continued driver and root-image diagnosis

Attempt 004 adds the kernel's `initcall_debug` logging. It uses the same original kernel, root image and DTB bytes, the existing explicit console override and one CPU. It stops inside `xiic_i2c_driver_init` at guest time 1.618641 seconds. The preceding Cadence I2C driver initialization returns. The last call has no return before the 25 CPU-second limit.
This isolates the wait to the Xilinx FPGA I2C driver. The original DTB enables that controller at `0x41600000`. The generic backend does not provide a verified implementation of that FPGA device. The result does not prove a root filesystem defect.

Attempt 005 adds only `initcall_blacklist=xiic_i2c_driver_init` to the diagnostic boot arguments. This is an explicit diagnostic bypass of an unimplemented device. It is not an acceptable original-board configuration.
The actual archived kernel passes the remaining initialization calls, identifies and loads the 15,129 KiB SquashFS image, and mounts it read-only on device `1:0`. It then panics because the command line has no final `root=` device. The exact message is `VFS: Cannot open root device "(null)" or unknown-block(0,0): error -6`.
The panic identifies the running kernel as `4.4.0-xilinx-g518c00cd977f-dirty`. That differs from the FIT description `ARM OpenWrt Linux-4.4.92`. The component bytes are unchanged and verified. Do not use the description as proof of the executed kernel version.

Both runs retain their process specifications, automatic adaptation records, original component manifests, guest logs, backend logs and process results in `original-zynq/attempt-004/` and `original-zynq/attempt-005/`.
Attempt 006 added the single further argument `root=/dev/ram0`, which selects the root device that the prior actual kernel log mounted. It mounted the root and freed kernel initialization memory. The next message was an undefined-instruction kernel Oops. The FPGA I2C bypass remained explicit.

Attempt 007 removed `keep_bootcon` and `initcall_debug`, and added a bounded QEMU exception/device trace. It reached actual archived init and preinit output. The trace filled its byte limit during repeated unimplemented NAND-controller reads. This timed-out, truncated diagnostic remains incomplete evidence; no complete trace is claimed.

Attempt 008 isolated the console change. Relative to attempt 006 it removes only `keep_bootcon` from the guest arguments, retains initialization logging, and raises the bounded window to 60 wall seconds and 45 CPU seconds. It reaches actual original init, preinit, `procd`, ubus initialization, module loading and guest bridge setup. The prior Oops does not recur in this run. This supports the retained boot console as the cause of the prior diagnostic fault; it is not a general kernel correctness claim.
The kernel reports PL353 NAND identification timeouts. The original user space reports missing `mmcblk0p1` boot storage and `mmcblk0p2` extroot storage. It creates an ephemeral RAM-disk overlay and continues. No original persistent storage or baseline is supplied. Watchdog text is actual service output, not proof that watchdog behavior passed. No miner health, share, native HTTP service or login test was performed.
The backend exits 137 after 53,872 milliseconds. This is consistent with the configured CPU limit, but the exact signal cause is not measured. Output is not truncated. Its NIC has no peer. No raw emulator control port, host directory or physical device is exposed.
The complete original-board and target gates remain incomplete. See [the bounded assessment](original-zynq/attempt-008/assessment.json) and [artifact hashes](original-zynq/attempt-008/evidence-checksums.json).

Pinned QEMU and BITMAIN kernel references were extracted as data in a separate bounded preparation process. `references-001` failed closed because a suffix-only selector matched more than one `VERSION` file. The corrected selector compares the exact relative member path. `references-002` confirmed that the selected firmware archive has no optional `uenv` member. `references-003` retains the required source text and explicitly records that missing optional member. Both failed probes remain intact.
These source archives do not prove the installed backend's build provenance or that the reference kernel is the selected binary's exact source.

| Source reference | Parent archive SHA-256 | Extracted member SHA-256 |
| --- | --- | --- |
| QEMU `11661d857254b27fca5568c6deacf4f6b6f3460f`, `hw/arm/xilinx_zynq.c` | `2a40e6705e59fd8807012631bc6b2c0877f10481b12768f78e4f9d7b88376cce` | `b0461f9663a7c926155a7e80301f904c8e4d73cc2adf13b66eb117c01583b4f5` |
| Same QEMU archive, `hw/arm/boot.c` | Same | `3ee33e386c78e816410d7f897d616a1f2c44203e7a609671184a95153412311f` |
| BITMAIN Kernel_SOC `e8a2f7145f1031ee910932fab48c614642cb5442`, `drivers/i2c/busses/i2c-xiic.c` | `5aa5c0915c93c6078984b13367edd12670e8ad25d922d352654f768d7005db8f` | `055aacc6d5259377c8e46661f0d55b4f1935ec43555e8ee36ab9b13661f41aae` |

The QEMU source identifies as 11.1.50. The installed binary identifies as 11.1.2. They are different inputs.
The reference machine sets primary `loader_start` to zero but sets no secondary-loader address. The common ARM loader defaults to writing the secondary stub at the unset address. This explains the recorded overlap as a source-supported hypothesis, not proof of a matching installed build.
The reference I2C driver calls `xiic_reinit` during probe and waits without a timeout for the receive FIFO empty bit. Its compatible string matches the archived DTB's FPGA I2C node. That code and the measured init-call stall support the need for a real FPGA I2C model. A zero register imitation or permanent init-call blacklist cannot satisfy that requirement.

The tested continuation commands were:

```sh
cargo run -p md-sim-firmware --example original_zynq_probe --locked --offline -- . work/simulator/original-zynq/attempt-004 diagnostic
cargo run -p md-sim-firmware --example original_zynq_probe --locked --offline -- . work/simulator/original-zynq/attempt-005 skip-xiic
cargo run -p md-sim-firmware --example original_zynq_probe --locked --offline -- . work/simulator/original-zynq/attempt-006 ramdisk-root
cargo run -p md-sim-firmware --example original_zynq_probe --locked --offline -- . work/simulator/original-zynq/attempt-007 ramdisk-trace
cargo run -p md-sim-firmware --example original_zynq_probe --locked --offline -- . work/simulator/original-zynq/attempt-008 root-progress
cargo run -p md-sim-firmware --example original_zynq_probe --locked --offline -- . work/simulator/original-zynq/references-003 references
cargo clippy -p md-sim-firmware --example original_zynq_probe --locked --offline -- -D warnings
```

Use fresh output directories for repeats. The final tested probe source hash is `60a4c6208f20c7ca19c741aca082be2dfcc8914cf21a80b21744e5dfb2b5b994`.
The bounded continuation is complete. The next implementation needs exact boot storage, reset and secondary-CPU behavior, and real FPGA I2C, NAND, fan and chain models. The original board identity and baseline blockers above remain open.

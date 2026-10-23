# S9 boot and storage continuation

The original-board gate remains incomplete. This work continues from original-zynq attempt 008. It does not count a generic loader or a diagnostic bypass as an original reset path.

## Exact SD input now resolved

The archive contains the complete SD image for the same selected release, not only its update tar:

`artifacts/files/firmware/braiins-feed-4366569e66ab54bc0d15/braiins-os_am1-s9_sd_2022-04-29-0-5eb2c5fe-22.02.4-plus.img`

Its SHA-256 is `97a257571af4fb9e62f709e2d6b8f36a191fdd07f1cd12ae780bd1c73af7eaef`. Its length is 112,197,632 bytes. The Rust example verifies both before preparation. It parses and bounds the four MBR partition records. It reuses the installed 7-Zip parser to extract the FAT boot files inside strict isolation. The parser and its shared module are locked runtime inputs. No archive script runs during preparation.

| Partition | Type | Byte offset | Byte size | SHA-256 |
| --- | --- | ---: | ---: | --- |
| 1 | MBR `0x0c`; actual BPB says FAT16 | 1,048,576 | 41,943,040 | `968b6e6fdb7e36a023bca47d1126eaf059b36feabf157cd8242da01af2ab741b` |
| 2 | Linux, `0x83` | 42,991,616 | 67,108,864 | `663b345610b7aecd9843f8a96fc5dc5c1d630b18f218b5f70812129af4d9acae` |
| 3 | `0xbb`, contents not yet interpreted | 110,100,480 | 2,097,152 | `50c8a6a63985f88f178fcd666838499cebc4a8fb290c458ca69bd58935596ebe` |

The original `boot.bin`, `u-boot.img`, `fit.itb` and `system.bit.gz` match the previously selected tar's SPL, U-Boot, FIT and FPGA bytes. This is measured byte identity, not filename inference. `uEnv.txt` is an empty original file.
The complete extraction and component manifest is [sd-bundle-004/boot/bundle.json](original-zynq/sd-bundle-004/boot/bundle.json).

The Zynq SPL header has valid magic and additive checksum. Its source offset is 2,240 bytes. Its length field is 79,263, which equals the entire file rather than the 77,023 bytes available after that offset. The direct-stage diagnostic loads only the available unchanged suffix. It does not claim to implement the internal boot ROM copy semantics.
The extracted SPL suffix has SHA-256 `765efb23844156e976ba785c24a6a753c664504a499c64581d1c356a3f4c02df`.
The U-Boot legacy image declares load and entry address `0x04000000`. Its header and data CRCs pass. The unchanged 573,212-byte payload has SHA-256 `ce991c8b0255a287fb78163ac3763d496a5a473ec6810e35976d6ce42d100644`.

## Commands and retained attempts

```sh
cargo run -p md-sim-firmware --example s9_boot_bundle --offline --locked -- prepare work/simulator/original-zynq/sd-bundle-004
cargo run -p md-sim-firmware --example s9_boot_bundle --offline --locked -- boot work/simulator/original-zynq/sd-bundle-004 work/simulator/original-zynq/spl-boot-001 spl
cargo test -p md-sim-firmware --example s9_boot_bundle --offline --locked
cargo clippy -p md-sim-firmware --example s9_boot_bundle --offline --locked -- -D warnings
```

Use new output directories for repeat runs. The two current tests check MBR bounds, overlaps and signatures, plus the standard CRC32 reference vector. Both tests and strict lint checks pass.

Preparations 001 and 002 rejected MBR type `0x0c` because the initial selector handled only the usual FAT16 MBR types. They remain saved. That temporary parser was removed. Preparation 003 uses the existing 7-Zip implementation and exports the exact original boot files. Preparation 004 also exports and hashes the two boot-stage payloads. Preparation 005 inspects the other partitions. It records the first 64 bytes and measured content counts. The actual first partition BPB says FAT16, despite its MBR type. The second partition is an Ext filesystem with label `overlay`. Its preserved listing contains `upper/usr/share/upgrade/firmware.tar.gz`, with length 32,090,330 bytes. The third partition is not a recognized filesystem. Its first bytes are ARM vector instructions; its exact relation to the bootloader remains under investigation.

SPL boot 001 failed before guest execution. QEMU attempted to create its run-owned snapshot in `/var/tmp`, which is absent in the locked root. The next run sets `TMPDIR=/tmp`, the private bounded scratch mount. It keeps the original image read-only and uses QEMU snapshot writes. No host directory is added.
SPL boot 002 retained the same error. The pinned QEMU source explains it: its temporary-file helper converts exact `/tmp` to `/var/tmp`. No more retry uses that implicit snapshot helper.
SPL boot 003 creates an explicit `/work/sd.qcow2` overlay with pinned `qemu-img`, backed by the unchanged archived SD image. Its virtual capacity is 128 MiB. Reads beyond the archived image return zero and guest writes remain in the private temporary overlay. It failed before guest execution because a 2 GiB address-space limit could not reserve the guest memory. Attempt 004 retained that limit due to an implementation edit mistake; its failure is also preserved.

SPL boot 005 uses the shared board-confinement guard and a 3 GiB address-space limit. Actual original SPL instructions execute. The guest prints:

```text
U-Boot SPL 2016.03-g8728fee00a (Dec 05 2020 - 17:42:30)
mmc boot
Trying to boot from MMC
### ERROR ### Please RESET the board ###
```

The SPL enters its original reset error loop. The bounded process ends with QEMU killed by a signal; the wrapper retains the failure. No firmware success is inferred. The next diagnostic records translated original instructions, while a separate direct U-Boot stage tests whether storage can proceed past the SPL failure. Both skip only the prior boot stages identified in their adaptation records; neither claims the internal boot ROM ran.

SPL boot 006 retains the same two-CPU outcome and 234,567 bytes of untruncated translated-instruction trace. The reference QEMU machine creates both CPUs without starting CPU 1 in a powered-off state. Its SLCR CPU reset register does not implement the reset/clock-stop callbacks required by the reference kernel. These are backend implementation gaps. They are not incompatible firmware evidence.

The separate `uboot-boot-001` probe starts the original U-Boot payload directly at its declared address. U-Boot identifies the S9 model and 1 GiB RAM. It recognizes the SD strap, reads the empty original `uEnv.txt`, reads all 164,570 bytes of `system.bit.gz`, and decompresses the bitstream. It identifies part `7z010clg400`. FPGA loading then fails because the generic backend does not supply a valid device descriptor/identity. U-Boot reaches its actual command prompt. No kernel starts in this run. NAND detection fails early because the skipped SPL has not initialized its MIO settings.

`spl-boot-007` adds a clearly recorded one-CPU diagnostic. The original SPL now reads `u-boot.img` from the supplied SD image and transfers control to that original U-Boot. The guest logs both real boot banners. U-Boot identifies the S9 model and 1 GiB RAM, then waits at NAND initialization. Unlike the direct U-Boot entry, this path executes the SPL's MIO setup. This separates the two-CPU reset issue from the next missing NAND device behavior. No NAND bypass or fake response was added.

```sh
target/debug/examples/s9_boot_bundle boot work/simulator/original-zynq/sd-bundle-004 work/simulator/original-zynq/spl-boot-006 spl trace
target/debug/examples/s9_boot_bundle boot work/simulator/original-zynq/sd-bundle-004 work/simulator/original-zynq/uboot-boot-001 uboot
cargo run -p md-sim-firmware --example s9_boot_bundle --offline --locked -- boot work/simulator/original-zynq/sd-bundle-004 work/simulator/original-zynq/spl-boot-007 spl single-cpu
```

The firmware backend task now has measured next interfaces: CPU 1 reset and release, PL353 NAND after real SPL MIO initialization, and the Zynq FPGA device identity and configuration path. The full original SD image supplies the missing boot and extroot storage bytes. Its third partition and exact NAND baseline remain to be interpreted. The 2018 minimum-version archive is also present at `artifacts/files/firmware/braiins-feed-625a0280e609e1a317c5/firmware_2018-10-24-0-9e5687a2_arm_cortex-a9_neon.tar`, with catalogue SHA-256 `c071771fe4d6f3afd39a5b0e321dfdd4cba1bab06f836eaa9bea0a16c7c4892f`; it is not yet a verified prepared update baseline.

The original SPL and U-Boot instructions have now executed. The remaining boot work is to resolve the boot-ROM handoff and implement the measured CPU reset, NAND and FPGA interfaces. The firmware agent is separately building the pinned QEMU source and real FPGA I2C model. No driver blacklist is present in this boot-stage path. Full exact-board identity, FPGA and ASIC interfaces, native services and mining gates remain open.

## Prepared NAND recovery payload

The following commands passed in isolated preparation. They reuse the exact archived SD image, pinned 7-Zip and the Rust gzip/tar libraries. They do not execute any installer.

```sh
cargo run -p md-sim-firmware --example s9_boot_bundle --offline --locked -- storage work/simulator/original-zynq/sd-storage-001
cargo run -p md-sim-firmware --example s9_boot_bundle --offline --locked -- payload work/simulator/original-zynq/nand-payload-001
```

The extracted `firmware.tar.gz` is 32,090,330 bytes. Its SHA-256 is `37ade53b6181b69cab42f0e551cdd2957a7375b8e6eb276e9121a2560bb497a6`. The [payload manifest](original-zynq/nand-payload-001/storage/storage.json) records every member, its archive type and its hash. The component files are exported separately under the same directory. The manifest itself is 36,091 bytes, with SHA-256 `f27d41088bf01c28ef5ec2b2f6a6b17f48d22b10b7c9de1c7ab7a6f7f4cae71d`.

The NAND U-Boot is 575,828 bytes, SHA-256 `60dd933089a203209507e8ea57f51721766d1a22daef04b3649b965e54809662`. It differs from the SD U-Boot. The first NAND FIT is 5,891,644 bytes, SHA-256 `d5431eabf4fd54d58f8a30c3425706a636aa8dc8f3242fac3ac6e19f898b332c`. It also differs from the SD FIT. These files are not substituted into the tested SD path.

The exact `CONTROL` reference declares SPL offset `0`, U-Boot `0x80000`, FPGA data `0x300000`, and two environment copies at `0x700000` and `0x720000`. It requires controller text `XILINX`, a recognized miner model, and one of two exact MTD-layout MD5 values before installation. Those constraints remain requirements. They do not prove a physical NAND part or geometry.

The second-stage archive is 24,796,786 bytes, SHA-256 `c625317338cc47adb5c9ab7404e5431947032af5f5e61ea3edee92d4de195837`. Its measured member index contains another FIT, configuration images, compressed boot files, a script and `sysupgrade.tar.gz` (18,414,497 bytes, SHA-256 `ee8cb1488f921599041a3f75baed25a2e428430bc7d260c46171fc7be08c12d5`). The exact installation transition remains unexecuted until the NAND model and layout are resolved.

The third SD partition is now resolved by byte comparison: its first 573,212 bytes exactly equal the previously verified SD U-Boot payload. All remaining 1,523,940 bytes are zero. This is measured identity, not an inference from ARM vectors.

The two boot/storage unit tests and strict lint checks passed after the component export change. Coordinator behavior checks also passed separately. The next storage action is to inspect the second-stage sysupgrade image and map it to the measured MTD layout, then exercise the actual update scripts inside the guest once that storage model exists.

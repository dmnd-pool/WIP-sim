# Commercial Bitcoin ASIC sources

This guide covers the non-Bitmain part of the Bitcoin ASIC catalogue. It is a research snapshot taken on 19 October 2026. It does not establish that every public file or every hardware revision has been found. The main catalogue excludes miners for other algorithms.

The raw vendor part also contains material collected before the user changed the scope to Bitcoin. Each record has a `scope` field. Use `bitcoin-asic`, `supporting-tool`, or `mixed-reference` for this project. Keep `supplementary-out-of-scope` material separate. No collected firmware or vendor tool was executed.

## Find a record

| Need | File or directory |
| --- | --- |
| Devices, boards, chips, PSUs, firmware and tools | [entities.jsonl](../catalog/parts/vendors/entities.jsonl) |
| Hardware and firmware links, with conditions | [relationships.jsonl](../catalog/parts/vendors/relationships.jsonl) |
| Primary pages, archive sources and local evidence | [sources.jsonl](../catalog/parts/vendors/sources.jsonl) |
| Missing files, access errors and unknown hardware | [gaps.jsonl](../catalog/parts/vendors/gaps.jsonl) |
| Scope of each acquired or attempted artifact | [artifact-scope.jsonl](../catalog/parts/vendors/artifact-scope.jsonl) |
| Download status, original URL, file path and SHA-256 | [artifact manifests](../artifacts/manifests/) |
| Original downloaded bytes | [artifact files](../artifacts/files/) |
| English translations and vendor English editions | [translation register](../translations/vendors/register.jsonl) |
| Reproducible vendor record builder | [build_records.py](../work/vendors/build_records.py) |

Start with the device ID. Find its links in `relationships.jsonl`. Read each link's `status` and `constraints`. Then open each source record. A source record or a firmware name can exist when a download failed. Only a manifest with `status: downloaded`, a real file, and a matching SHA-256 proves that the bytes were saved. A Git LFS pointer is not a firmware image.

A family record is not an exact product. A controller target is not always a PCB revision. A public binary is not automatically open-source software. Source archives can include proprietary binary parts or files under different licences.

## Coverage by vendor

| Vendor | Material collected or indexed | Main limits |
| --- | --- | --- |
| MicroBT / WhatsMiner | Official download index; current and older update images; H3, H6, H6os and H616 SD images; 50 English product manuals; air, hydro and immersion guides; API documents; WhatsMinerTool; dated launch notices | Model names do not give a full hashboard or PSU revision matrix. Current firmware is controller-specific. |
| Canaan / Avalon | Official mining source repositories; legacy MM FPGA firmware; cgminer and OpenWrt packages; Avalon6, Avalon7 and Avalon10 documents; pinned Avalon7 wiki; Nano3S and modern Avalon_mm sources; test and diagnostic tools | Current download portal access failed. Some modern model configuration values are placeholders. Some source files have a restricted licence. |
| Innosilicon | Pinned independent archive of Bitcoin T-series SWU images and English vendor manuals; G19 and SoC target labels; model-code mapping from the archive maintainer | This is archive evidence. Vendor signatures, exact PCB revisions and the full hardware matrix remain unverified. |
| Halong / DragonMint | T1 device record and community `dragon-rest` API wrapper | The API wrapper is not a full firmware image. T1 names in Innosilicon archives need hardware identity checks. |
| iPollo | Official B1 instructions; B-series firmware and SD recovery links; ShowIP link | Public Zoho firmware links returned server errors. Exact ASIC and controller parts are unknown. |
| Ebang / Ebit | Company prospectus with exact model release months and ASIC process sizes; E9 manual download lead | The old miner site timed out. A usable complete firmware set and board matrix were not obtained. |
| Bitfury | Clarke ASIC and BlockBox AC technical records from vendor datasheets | The original PDF URLs returned 404 to the downloader. Searchable vendor text remains evidence, but this is not a saved PDF. Full controller firmware and the ASIC command set are incomplete. |
| GMO | Official B2 and B3 launch notice, chip process and product specifications | Standalone firmware and controller documentation were not found. |
| StrongU | Bitcoin device discovery gap | A reliable primary STU-U8 firmware and hardware source was not obtained. |

The main catalogue has other parts for Bitmain, Bitdeer, Auradine, ePIC, Proto, community firmware and open mining hardware. Use the project index to find those parts.

## WhatsMiner compatibility

Keep the controller target with the firmware image. The official index treats H3, H6, H6os and H616 as different targets. It lists the current `20260908.12` all-series image for H616 / CB6 only. This image has an earlier-version rollback limit. The older files have their own model and version limits. Read the English release note in [wm-index.raw](../work/vendors/wm-index.raw).

The [official spare-parts FAQ](https://support.whatsminer.com/en-US/article/676) gives separate air, hydro and immersion control-board designations. It also confirms CB4_V10 as a replacement in one M30S++ example. This does not prove that all control boards or all M30S++ production batches are identical.

The vendor-hosted M30S 88T sample review is useful board evidence. It identifies H3, three hashboards with 148 chips each, and PSU P21-GB-12-3300. The catalogue attaches these facts to the reviewed sample. It does not apply them to every later M30S unit. Read [article 235](../work/vendors/wm-news-235.json).

Use the saved API version that matches the selected firmware. The current vendor index includes API v3 HTML documentation and example code, plus a later PDF user manual. The documents have firmware-version conditions. A management API model can run without a complete ASIC model. It cannot prove that the original firmware will boot on an emulated controller.

## Canaan compatibility and source use

The legacy Avalon7 system has separate layers: an external Raspberry Pi controller, an AUC3 adapter, the miner MM board, and the HU hashboard. The Avalon721 wiki identifies 72 A3212 chips and the external controller options. Use the [pinned wiki](../work/vendors/avalon7-docs-wiki/) to keep those layers separate.

The legacy `MM` project targets a LatticeMico32 soft CPU in a Xilinx FPGA. The modern `Avalon_mm` project targets a different platform. The similar names do not establish compatibility.

The official Nano3S repository describes network-layer source and a K230 controller. Its USB update applies to a device with a self-built system image. The modern Avalon_mm repository has placeholder board and chip counts. It also has both BSD and BUSL licence terms. Inspect the file licence and configuration before planning a source build.

The `Avalon-extras` archive contains `fake-avalon`, test data, `mm-tools`, and a miner debug system. These are useful starting points for protocol fixtures. Their existence does not make them a simulator for every Avalon generation.

The [independent Nano3S report](https://github.com/CryptoIceMLH/Nano3S) describes one MM-NANO3S-V0.2 device, a dual-core K230D design, a 12-chip A3197S board, and separate Linux and real-time paths. The author withholds some command constants and byte layouts. Use these facts as reported evidence. The report alone is insufficient for a complete ASIC emulator.

## Innosilicon archive use

Use [inno-fw-tree.raw](../work/vendors/inno-fw-tree.raw) for the pinned file list. The source is [mekas17/fw](https://github.com/mekas17/fw), an independent archive. The acquisition script selects only the Bitcoin T-series paths. It excludes the other algorithms in that repository.

The archive separates `g19` and `soc` firmware. Keep that target in the image key. The word `soc` does not name an exact CPU. The README maps some printed model codes to shared firmware folders. Those links have `community-reported` status. A matching family name does not prove that a board revision accepts the image.

The deduplication record is [innosilicon-dedup.json](../work/vendors/innosilicon-dedup.json). It records exact Git blob matches with the general archive. Other files with the same name can be Git LFS pointers or different bytes. Compare content hashes, not only filenames.

## Dates

`release_year` is null when the collected evidence does not establish the year. `date_basis` states what the date means. A firmware build date, a manual revision date, a product announcement, and a first shipment are different events.

Examples from primary vendor evidence are M20S on 19 May 2019, M50/M50S/M53 on 6 April 2022, and the first M60 group on 24 October 2023. The vendor says M30S had already launched in December 2019; the global M30 event was scheduled for April 2020. Read the saved [WhatsMiner news JSON files](../work/vendors/).

Canaan's annual report gives series release years. A separate investor chart has conflicting dates for some older models. The catalogue preserves the conflict. Do not replace an exact model date with its series date. The Ebang prospectus gives release months for 13 models; the catalogue does not invent the day.

## Use this material to design the simulator

1. Select one exact device and hardware revision.
2. Select a firmware artifact and verify its hash and target.
3. Identify the image type. An update package can require an installed base system. It can lack the bootloader, kernel, partition table, or recovery code.
4. Read the boot and hardware requirements before running an image. Model the controller CPU, memory, flash layout, buses, fan inputs, sensors, PSU and ASIC chain as separate parts.
5. Start with an API or protocol fixture when full hardware evidence is missing. Label the result as a model. Do not label it as original firmware execution.
6. For original firmware execution, add device models and deterministic bus responses. Record each missing peripheral and each assumption.
7. Check the result against traces from the same firmware and board revision. Keep timing, power and thermal claims separate from functional protocol tests.

The collection supports that work. It does not yet prove that any selected firmware can run with any selected board. Unsupported combinations must return an explicit reason. Unknown combinations must remain unknown.

## Language and preservation

Original binary and source bytes are unchanged. Firmware language is not inferred from an English download page. Vendor English document editions are retained where available. The Avalon721 packaging text has a marked English companion translation. The translation register states its scope and review status. Text inside some source-code comments and document images has not received a complete translation review.

The cancelled all-algorithm queues and their partial files remain in the work area. They are outside the main Bitcoin scope. Do not use them as successful downloads. Acquisition logs and manifests retain errors so that later work can retry a specific public source.

## Static image inspection and diagnostic translations

The [Innosilicon inspection index](../work/vendors/innosilicon-inspection.jsonl) records the downloaded SWU files that have received read-only inspection. It lists CPIO member offsets, sizes, SHA-256 values, CPIO checksums, descriptor hash checks, declared hardware targets, and decoded device-tree identifiers. The [inspection directory](../work/vendors/inspection/) holds the original descriptor text and decoded device trees. The parser does not execute the image or its scripts. A component hash match proves internal integrity, not the publisher's identity.

Some G19 packages include `BOOT.bin`, `system.bit`, a device tree, a kernel and a UBI root filesystem. Their descriptors select main and alternate flash partitions. Keep these details with the specific firmware artifact. Do not apply one descriptor's flash layout to every T-series board.

The [Innosilicon diagnostic translations](../translations/vendors/innosilicon-error-list-notes.md) preserve separate Chinese and Russian error lists. The documents disagree about several repair responses. Their descriptions give evidence for SPI to hashboards and I2C to the power supply. They do not define the bus protocol or establish support for every model.

The [Avalon 1056 / 1066 release-note translation](../translations/vendors/canaan-a1066-release-note-en.md) covers all Chinese text. The archive also includes an English counterpart. The note describes the move from W5500 to lwIP, power-overload control, three pools, Extranonce, a watchdog, and fan control. The note itself does not give a build version or release date.

The K230 Burning Tool has a documented mining role: the official Nano3S `readme.md` names the saved tool and instructs the user to select `Avalon_Nano3s_2025012122_b377540.kdimg` and SPI NAND. This evidence applies to that Nano3S procedure.

The official Nano3S source archive also includes three intact component bundles. Their child artifact manifests preserve the parent archive hash and member name:

- `vendors-canaan-nano3s-resource-sysimages-2025012001-21e0e59`: Linux and real-time system images, SPL, U-Boot, SPI-NAND environment, and UBI root filesystem.
- `vendors-canaan-nano3s-resource-app-25012001-85ce93f`: Linux miners, real-time ASIC miner, UI resources and configuration.
- `vendors-canaan-nano3s-resource-tools-2025012201`: image-generation tools and configuration.

Use [canaan-nano3s-resources.json](../work/vendors/inspection/canaan-nano3s-resources.json) for the member hashes and ELF header fields. The bundles require assembly according to the source README. They have not been built, flashed, or run.

## Legacy manufacturer leads

[The breadth check](../work/vendors/breadth-discovery.json) records StrongU STU-U8/U8 Pro, Aixin/Aisen/Love Core, Holic, Esonics, ASICminer 8 Nano, and BitCrane leads. It gives the source URLs and the missing evidence for each name. A lead does not create a verified product or compatibility edge.

Holic H9 was not established; secondary lists identify H22/H28. Esonics results did not establish a Bitcoin ASIC device. Aixin/Aisen model names must remain distinct from Innosilicon and Bitmine CoinCraft A1. The old `asicminer.co` domain redirected to unrelated content when checked. Do not use its current contents as manufacturer evidence.

## SFARDS, CoinCraft and Ebang board evidence

The official SFARDS `ASIC-SF3301` snapshot contains the SF3301 data sheet, SF100 hashboard CAD and bills of materials, power-supply drawings, and a USB dual-miner board. The SF3301 supports SHA-256 and Scrypt. It is in the Bitcoin scope because it has a documented SHA-256 role. Do not treat the two algorithms as the same work format. The nested `cpuminer-2.4.2-sfards` and `sfminer-0.95-13` packages have separate artifact records. Use [the nested-package index](../work/vendors/inspection/sfards-nested-packages.json) to find their original archive members and hashes.

The SFARDS English companions cover the Chinese README text, the single-chip board guide, Chinese schematic annotations, and the two collected spreadsheet bills of materials. The original CAD, spreadsheets and source files remain unchanged. The translation register gives the coverage of each companion.

The official Bitmine `cgminer` source names CoinCraft Desk and CoinCraft Rig. Its board selector defines different I2C addresses and chain selection for the two designs. These source definitions are useful simulation inputs. They are not a complete PCB revision map. Keep CoinCraft A1, Aixin/Aisen A1 and Innosilicon models separate.

The community EBAZ4205 archive identifies an Ebit E9+ controller. It contains schematic/PCB files and captured Linux information. The reported board has a Xilinx Zynq XC7Z010CLG400, 256 MiB DDR3, 128 MiB NAND and an IP101GA Ethernet PHY. The evidence is a preserved owner/community archive. Do not apply its board map to every Ebang model.

The BitCrane T110 owner review identifies two Golden Nonce chips, a custom Cubie controller, a custom power supply and two liquid-cooling loops. It does not identify CoinCraft A1 chips. Its connections have community evidence status.

## KnCMiner and Spondoolies source

KnCMiner's image repository has separate Jupiter-generation and Neptune images. The Jupiter `0.99.2-E` image is for the November Ericsson DC/DC board cohort. The `0.99.2-tune` image is for the October GE cohort. The `1.00` README supports either cohort, but explicitly excludes a miner with mixed cohorts. The catalogue records both supported and incompatible edges. The cohort names do not establish an exact PCB revision.

KnC `.bin` update files are archives. For example, the collected Neptune `1.06s` file contains a kernel, root image, checksums and an update script. Nothing in the archive was run. SD recovery ZIP files have a separate role. A web update and an SD recovery bundle must not share one simulator loader definition.

The pinned `knc-asic` snapshot contains direct ASIC commands, transport code and BeagleBone I/O-board support. It also contains later Raspberry Pi/Titan paths. Use the Jupiter and Neptune paths for Bitcoin work. The separate `jupiter-hw-docs` snapshot contains the FPGA SPI specification and component documents. Its 384-bit request frames and 96-bit response frames describe that FPGA transport. They do not establish that every later ASIC command uses the same format.

The `meta-kncminer` README describes an OpenEmbedded/Ångström BeagleBone build. It needs external layers, tools and source dependencies. The collected default KnC Linux repository is a Raspberry Pi branch with no proven Bitcoin controller role. That snapshot is supplementary. Do not use its existence as proof that it is the kernel for the BeagleBone images.

The Spondoolies repositories include separate SP10 and SP30 miner-gate code, CGMiner, system software, U-Boot and build material. The SP30 source has an SP2x compile-time configuration. The catalogue preserves these source configurations separately from physical board records.

For a first simulator interface, inspect `miner_gate/mg_proto_parser_sp30.h`. It defines protocol version 30 on `/tmp/connection_pipe_sp30`, request and response structures, and queue limits. The SPI and I2C code is a second layer below that daemon. Check target ABI, alignment and byte order before serializing the C structures. The code has not been built or tested against hardware in this project.

## Historical CoinTerra and coverage gaps

The saved CoinTerra FAQ describes TerraMiner IV with four GoldStrike I ASICs, a BeagleBone Black, two Power-One supplies and two liquid-cooling loops. These are historic manufacturer-site contents preserved during this search. Current domain ownership and unchanged historical publication are not independently verified.

The old firmware index lists four releases and MD5 values. Its download links now return the home page. Firmware entities therefore have `binary_acquired: false`. The catalogue does not turn the index page into a firmware payload. AIRE is retained as an announced device; the source does not prove that it shipped.

[The discovery coverage audit](../work/vendors/discovery-coverage-audit.jsonl) compares the broad Bitcoin model list with documented exact models and family records. It is a name/evidence audit, not a completeness certificate. Records owned by the Bitmain and open-mining parts can be marked `other-part-or-unresolved` by this conservative vendor audit. Read the integrated catalogue before treating such a row as absent.

The gap list explicitly retains missing exact models and revisions. Examples include Spondoolies SP31/SP35/SP50, KnC Super Jupiter, HashFast Sierra Evo, additional Rockminer products, and the controller/ASIC maps of ASICMINER Tube and Prisma. A related driver's existence does not prove a model's whole hardware design.

## Avalon Nano3, Nano3S and current Avalon series

Nano3 and Nano3S are separate devices. The Nano3 FCC filing `2BGFZANANO` supplies public internal photographs, a bilingual manual, a label and a model-difference letter. The photographs identify a sample with:

| Part | Visible evidence |
|---|---|
| Network/display controller | `MM-NANO3-V1.0`, printed date `2024/01/24`, Kendryte K230D |
| Power/subordinate control board | `POWER NANO3 V0.3 240124`, Kendryte K210 |
| Hashboard | Ten package positions, A1 through A10; exact ASIC mark and PCB revision unresolved |

These observations apply to the photographed sample. The filing's model aliases do not prove that all named Nano4–Nano9 retail products exist or accept Nano3 firmware. The schematic, block-diagram and operation-description entries are metadata-only. Their contents were not obtained.

The official `AvalonNano3_Recovery_Tool_V1.0.zip` is saved. Its nested recovery package includes `heater_nano3_master_image.img`, a model-specific Windows burning tool, driver support and Zadig 2.8. The image is exactly 128 MiB. Child artifacts record both archive levels and hashes. The tutorial labels the program “Just For Avalon Nano3” and shows USB boot identifiers `29f1:0230`. Use [the recovery member index](../work/vendors/inspection/canaan-nano3-recovery.json) for file hashes. The archive and video were inspected; the tool and image were not run.

The Avalon8 and Avalon9 Git wikis are saved at pinned commits. They distinguish controller/OpenWrt, MM, PMU and hash-test releases. The catalogue creates separate release entries for those roles. An entry without an acquired payload says so. Old directory links still need an exact filename/hash match to preserved images.

Current Canaan model records include industrial air, immersion and hydro products, plus Avalon Q, Mini3 and Nano devices. Release dates use dated manufacturer announcements where available. A first shipment, future batch date, firmware build date or FCC filing date is retained as that type of evidence. It does not silently become a product launch date.

## Fluminer, Heatbit and Jingle Miner

Fluminer T3 has an official product page, operation manual, update notice, firmware and group-management package. The product page and Chinese manual give different hash-rate and power profiles. The English companion preserves the difference. The `1.15` firmware is a complete downloaded AES-encrypted ZIP; its 120-entry file directory is indexed, but its contents were not decrypted. It is not ready to run in a simulator. The group-management archive publishes its password in directory labels. A read-only integrity check with that password passed; the executable was not run.

The Fluminer upgrade notice is bilingual and describes an older `1.12` procedure. Do not use its version as the version of the collected `1.15` package. The cache-cleaner row has no usable download link. Exact T3 ASIC, hashboard and controller parts remain unknown.

Heatbit Trio, Maxi, Maxi Pro and Canvas have saved official product evidence. The Trio FAQ separates approximately 400 W of mining from 1100 W of conventional electric heat. A 1500 W appliance rating is not 1500 W of mining. The collected material does not identify the exact ASIC/controller boards or a standalone firmware payload.

The Hashcard manual states that its PCIe-shaped edge is mechanical only. It has no electrical or data connection. The miner uses Ethernet or Wi-Fi and a separate 12 V supply. Do not model it as a PCIe accelerator. The open-mining part supplies Jingle Miner Lite and Pro source, factory-image and board evidence. RGB branding alone does not prove that the generic Pro firmware supports the RGB model.

## Payload integrity and document editions

[The payload integrity audit](../work/vendors/payload-integrity-audit.jsonl) compares downloaded Innosilicon, legacy WhatsMiner and KnC files with the size and Git blob SHA-1 from pinned repository trees. Artifact manifests also hold the locally computed SHA-256. A Git blob match identifies exact archived bytes. It does not prove the firmware's manufacturer signature or safety.

Two incomplete Innosilicon downloads were found during this audit. Their partial bytes were moved to `work/vendors/incomplete-payloads/`. Both files were then downloaded in full and matched the pinned Git blob hashes. The final payload audit has 148 files and no mismatch. All 93 downloaded Innosilicon SWUs pass the static archive parser. Always read the current manifest status. A `.part` file, failed transfer or Git LFS pointer is not a usable firmware image.

The Chinese and English M31S V1.1 manuals are different specification editions. The Chinese copy lists 74 TH/s, 3450 W and a `P21-GB-12-3300-D-V01` supply. The English copy lists 72 TH/s, 3312 W and `P21D-GB-12-3300-D-V01`. They also give different heights. The Chinese text has a separate English companion. Do not treat the vendor's English edition as a byte-for-byte translation of the Chinese edition.

## HashFast MCU firmware and Rockminer distributions

The official HashFast `hashfast-uc` snapshot contains MCU firmware, a loader, FPGA source and the HashFast protocol headers. Its Makefile has AVR32 UC3B0 variants for 128, 256 and 512 KiB flash. The build needs the historical AVR32 toolchain and Atmel headers. The saved source is not a complete reproducible build environment.

The `hashfast-g1-emulator` entity points to the exact `src/asic_emulator.c` archive member. This manufacturer test emulator handles G-1 packets, work queues and status. It uses four emulated dies with 96 cores each. Its nonce values are artificial: `0x4242ddcc`, where `dd` is the die and `cc` is the core. It requires USART loopback and a modified host driver. It does not compute valid Bitcoin proof of work. Its timing constants must not become a hardware performance curve.

The HashFast tools distinguish application USB mode `297c:0001`, HFU loader mode `297c:8001` and Atmel DFU mode `03eb:2ff6`. Preserve these mode changes in any simulator. The historical release/debug indices are saved at gist revisions. Their public Google Drive binary URLs currently return 404. Application HFU, loader HFU and loader HEX remain separate unavailable release entries.

The official Rockminer support repository now establishes exact R3 and R4 product names. Its shared handbook describes a Raspberry Pi, a four-port USB hub and MCU connections to blades. The support pages also link separate Cubieboard images. These are named host configurations, not proof of arbitrary controller interchange.

Rockminer repositories preserve CGMiner variants, RockWeb, NewRbox and AM_manager. Some distribution snapshots contain executable bundles and manuals as well as source. Use each artifact's member listing and acquisition status. The Chinese support pages have complete English companions in `translations/vendors/rockminer-*-en.md`. The shared Chinese PDF has a preserved vendor English handbook. The complete blade MCU and hashboard revision map remains a gap.

## Image-specific controller platforms

The catalogue now has five WhatsMiner image platform profiles. Use the exact
profile and artifact hash. Do not use the controller family name as a full
hardware specification.

| Image profile | DTB processor evidence | DTB RAM declaration | Saved boot evidence |
| --- | --- | --- | --- |
| H3 20220422.18 | `sun8iw7p1`, `ALLWINNER_H3`, four Cortex-A7 nodes | 1 GiB at `0x40000000` | `IMAGEWTY`; default environment selects NAND; root `/dev/nandd` |
| H6 20210109.22 | `sun50iw6p1`, `ALLWINNER_H6`, four Cortex-A53 nodes | 512 MiB at `0x40000000` | `IMAGEWTY`; default environment selects NAND; root `/dev/system` |
| H6os 20220422.18 | `sun50iw6p1`, `ALLWINNER_H6OS`, four Cortex-A53 nodes | 512 MiB at `0x40000000` | `IMAGEWTY`; default environment selects NAND; root `/dev/system` |
| H616 20220729.11 | `allwinner,h616`, `sun50iw9p1`, four Cortex-A53 nodes | 512 MiB at `0x40000000` | `IMAGEWTY`; default environment selects NAND; root `/dev/nand0p4`; GPT flag |
| H616 20251209.16 | `allwinner,h616`, `sun50iw9p1`, four Cortex-A53 nodes | 512 MiB at `0x40000000` | `CHIPSENIMA_v1.0`; two distinct DTBs and two U-Boot build dates |

These RAM values are image defaults. They are not measurements of installed
RAM. A loader can change the DTB after DRAM detection. The H616 images contain
31 numbered DRAM profiles plus a default profile. The selected physical memory
part remains unknown. H6 and H6os use the same SoC identifier but have different
platform labels and DRAM parameters. This does not establish interchangeability.

See [WhatsMiner platform evidence](../work/vendors/whatsminer-platform-evidence.jsonl)
for the archive member names, image hashes, DTB offsets and hashes, CPU nodes,
storage-controller states, partition settings, and selected boot environment
fields. The four `IMAGEWTY` images have member indexes with byte offsets and
SHA-256 values. Their partition configurations each name eight partitions.
English companions translate all Chinese partition comments. Comments in the
larger peripheral configuration files are not fully translated. The 2025
`CHIPSENIMA` container is only partly decoded. Its nested `.ima` matches the
published MD5 in the vendor package. No image was run.

The Innosilicon G19 pass covers 31 DTBs. All declare Zynq-7000, two Cortex-A9
nodes, 256 MiB RAM at address zero, an enabled PL353 NAND controller, and CPU
operating points of 666666 and 333333 kHz. These are configuration values.
The physical Zynq package, DRAM part, NAND part, and measured clock rates remain
unknown. The archive has three NAND layout groups: 11 partitions, 12 partitions
with `reserve`, and 13 partitions with `events` and `miners`. Main and alternate
DTB, FPGA, kernel, and UBI rootfs targets are explicit in the SWU descriptors.

Use [Innosilicon platform evidence](../work/vendors/innosilicon-platform-evidence.jsonl)
for each image's exact DTB hash and partition offsets. The existing G19 platform
entity now links all 31 samples. Three separate layout entities prevent an
implicit assumption that one flash map works for every G19 image. SDHCI0 is
enabled and SDHCI1 is disabled in these trees. This does not establish a real
board's boot straps or normal boot source.

## Final acquisition check

The [final vendor verification](../work/vendors/final-verification.json) reports
no queued vendor artifacts and no missing catalogue references or saved-file
size errors. It records 614 downloaded vendor artifacts and 96 failed fetches.
These counts include supplementary material from the earlier scope. They do
not mean that every public firmware version was found.

The [legacy archive check](../work/vendors/legacy-archive-inspection.jsonl) reads
42 completed archives without an error. The final Spondoolies and KnC kernel
archives also pass SHA-256, complete gzip CRC/EOF, full TAR parsing, and exact
pinned PAX commit checks. See [the kernel audit](../work/vendors/final-kernel-audit.json).
The KnC Raspberry Pi kernel remains supplementary because its role in a Bitcoin
controller is not established. All downloaded executables remain unexecuted.

# Open Bitcoin ASIC mining: archive guide

This guide covers Bitcoin ASIC devices, firmware, board designs, and support tools.
The main records have one of these scope values:

| Scope | Meaning |
|---|---|
| `bitcoin-asic` | Bitcoin ASIC hardware or device firmware |
| `supporting-tool` | A protocol, component document, or tool needed for this work |
| `mixed-reference` | Useful ASIC evidence in a source that also covers other systems |
| `supplementary-out-of-scope` | CPU, GPU, FPGA, or non-Bitcoin material obtained before the scope correction |

NerdAxe is an ASIC device family. NerdMiner V2 hashes on an ESP32 processor.
Keep these systems separate. The main catalogue excludes NerdMiner V2.

The guide uses short technical English. It follows the requested communication style.
It is not a certified ASD-STE100 document.

## Find a record

Use the main catalogue tool from the repository root:

```sh
python scripts/catalog.py search bitaxe
python scripts/catalog.py show bitaxe-gamma
python scripts/catalog.py links esp-miner-axeos --relation supports
```

The source records for this area are in [catalog/parts/open](../catalog/parts/open/).
Use the files below for more detail.

| File | Use |
|---|---|
| `entities.jsonl` | Find a device, PCB, chip, firmware, tool, or protocol |
| `relationships.jsonl` | Find directed support and component relationships |
| `sources.jsonl` | Find the original URL, source commit, and local evidence |
| `artifact-scope.jsonl` | Determine whether an artifact belongs in the main scope |
| `board-configurations.jsonl` | Find exact Bitaxe board IDs and source configuration values |
| `nmaxe-board-configurations.jsonl` | Find six NMAxe board classes and their pin and power settings |
| `nmaxe-release-package-members.jsonl` | Find all 28 NMAxe ZIPs, model folders, member hashes, and image/ELF checks |
| `amlogic-interface-contract.json` | Find the observed A113D boot, GPIO, UART, I2C, and storage contract |
| `amlogic-prebuilt-verification.jsonl` | Find SHA-256 checks for seven embedded diagnostic tools |
| `jingle-interface-contract.json` | Find Jingle Lite/Pro power, startup, sensor, and flash-map behaviour |
| `jingle-board-configurations.jsonl` | Find literal board assignments from the fixed Jingle source |
| `amlogic-git-tree-files.jsonl` | Find preserved Amlogic document paths and verified Git blob IDs |
| `archive-integrity.jsonl` | Find full gzip/ZIP checks and independently recomputed file hashes |
| `historical-source-configurations.jsonl` | Find release-specific board, partition, and build source evidence |
| `esp-image-layouts.jsonl` | Find image headers, segment addresses, partitions, and internal checksum checks |
| `esp-image-hash-anomalies.jsonl` | Find two preserved upstream bootloader header/hash conflicts |
| `source-identity.jsonl` | Find source commits verified against Git archive markers |
| `hardware-design-dates.jsonl` | Find model-specific CAD history dates and their limits |
| `hardware-source-dates.jsonl` | Find repository ancestry dates; these can predate the model |
| `bfl-interface-contract.json` | Find AVR32 controller registers, commands, build defaults, and conflicts |
| `bfl-simulator-contract.json` | Find the BFL/Icarus laboratory simulator model and its accuracy limits |
| `bfl-fixtures.jsonl` | Find 16 captured or derived BFL characterization data files |
| `bm13xx-interface-contract.json` | Find the structured community protocol summary and its validation limits |
| `archive-members.jsonl` | Find schematics, BOMs, PCB files, PDFs, and embedded images inside archives |
| `release-events.jsonl` | Find published release tags, dates, and asset URLs |
| `firmware-release-assets.jsonl` | Find the original five ASIC firmware histories |
| `extra-firmware-release-assets.jsonl` | Find the added NMAxe and Serpent histories |
| `other-firmware-release-assets.jsonl` | Find HexOS and BAP-GT-TOUCH histories |
| `jingle-firmware-release-assets.jsonl` | Find the two public Jingle Lite/Pro release sets |
| `release-source-plan.jsonl` | Find planned release-source archives and pinned tags |
| `release-source-status.jsonl` | Find the current manifest result for every planned release source |
| `eeprom-topology-claims.jsonl` | Find 50 independent community board configuration claims |
| `eeprom-fixtures.jsonl` | Find six raw EEPROM fixtures for parser work |
| `apw12-dump-inventory.jsonl` | Find 14 PIC PSU dumps and Intel HEX record checks |
| `firmware-asset-status.jsonl` | Find image use, structural header checks, and upstream digest comparisons |
| `release-source-archives.jsonl` / `extra-release-source-archives.jsonl` | Find source commits and download results for firmware releases |
| `all-archive-lfs-audit.jsonl` | Find the LFS audit result for each completed source or package archive |
| `lfs-pointers.jsonl` / `all-archive-lfs-pointers.jsonl` | Find Git LFS references whose target bytes are missing |
| `submodule-dependencies.jsonl` | Find dependencies omitted by source archive downloads |
| `translation-index.jsonl` | Find an English document and its original |
| `gaps.jsonl` | Find missing evidence and unresolved conflicts |

Each source archive uses a fixed Git commit.
The short extracted files in `work/open/inspect/` help with review.
They do not replace the complete archive.

Each downloaded artifact has a JSON record in `artifacts/manifests/`.
The record gives its path, URL, SHA-256, byte count, and download status.
An asset in a release index is only a known URL. It is available locally only when its manifest says `downloaded`.
The archive process did not run firmware, miners, or downloaded tools.
The LFS audit covers captured HEAD sources, completed release sources, direct dependency archives, and completed release packages.
The audit index records each checked archive hash.
The completed archive audits found no Git LFS pointers.
The image index covers 2,885 binary assets, 4,356 ESP images, and 1,709 partition tables.
All parsed image XOR checks and partition MD5 checks pass.
Two old ESP-Miner v1.0 bootloaders have a stored SHA-256 that differs from the image.
Both stored hashes match if header byte 3 is `0x1f` instead of `0x10`.
This byte contains flash size and speed fields. Only the speed field differs in these cases.
The application hashes pass. The archive preserves the original bytes.
This finding does not establish whether a device will boot those images.
An integrity monitor rejects files whose size or digest differs from the upstream release metadata.
The monitor repairs incomplete transfer results after a server ends the response early.
A repaired file must match its expected size and published digest.
New downloads and nested archives need a new audit pass.

## Hardware coverage

| Device family | Hash chips | Main evidence |
|---|---|---|
| Bitaxe Max | 1 × BM1397 | PCB design and ESP-Miner configuration |
| Bitaxe Ultra | 1 × BM1366 | PCB design, BOM, schematics, source |
| Bitaxe Supra | 1 × BM1368 | PCB design and source configurations |
| Bitaxe Gamma | 1 × BM1370 | PCB design, source, board guide |
| Bitaxe Gamma Duo 650 | 2 × BM1370 family | Separate 650 branch and configuration |
| Bitaxe GT 801 | 2 × BM1370 | Separate GT design and configuration |
| Bitaxe Ultra Hex | 6 × BM1366 | PCB design and revision notes |
| Bitaxe Supra Hex TCH | 6 × BM1368 | TCH design and firmware |
| Bitaxe Gamma Hex TCH | 6 × BM1370 | Separate TCH design and firmware |
| Bitaxe Gamma Hex 1300 | 6 × BM1370 | Two voltage domains, three chips per domain |
| NerdAxe Ultra / Gamma | 1 × BM1366 / BM1370 | Separate hardware branches |
| NerdQAxe / NerdQAxe+ / NerdQAxe++ | 4 × BM1366 / BM1368 / BM1370 | Hardware designs and board-specific firmware classes |
| NerdOCTAXE+ / Gamma | 8 × BM1368 / BM1370 | Separate board designs |
| NerdQX | 4 × BM1370 | Separate design and firmware support |
| PiAxe | 1 × BM1366 | Raspberry Pi HAT design and Python host miner |
| BitForge Nano | 2 × BM1370 | PCB design and ForgeOS |
| EmberOne/00 | 12 × BM1362 | Hashboard design and host interface |
| Libre Board | Separate control board | CM5 connector design and KiCad files |
| NerdHAXE Gamma | 6 × BM1370 | Hardware design; LilyGO T-Display S3 controller |
| NerdEKO Gamma | 12 × BM1370 | Hardware design; source has a noncommercial restriction |
| NMAxe / NMAxeGamma | 1 × BM1366 / BM1370 | Separate board classes, manuals, and schematics |
| NMQAxe++ | 4 × BM1370 | Revision-specific firmware and display control |
| NMQAxe++ Nexus | 2 × BM1373 | Two series voltage domains; three-phase regulator |
| Naja Duo | 2 × BM1373 | Published CAD; pinned readme says untested prototype |
| Bitaxe Bonanza | 8 × Intel BZM2 | Published CAD; pinned readme says it does not work |
| Bitaxe Proto | 4 × Block Proto MC3 | Published CAD; untested prototype |
| PC-AXE | 12 × BM1370 target | Unfinished PCIe-form design; noncommercial restriction |
| Bitaxe Flux A0 | 1 × BM1373 target | Engineering draft; firmware inhibits ASIC power |
| GekkoAxe GT / Gamma 5V / Gamma 12V | 2 / 1 / 1 × BM1370 | Three separate HexOS source classes and release targets |
| Jingle BTC Solo Lite / Pro | 1 / 4 × BM1370 | Vendor product pages, manuals, and separate source board targets |
| Jingle BTC Solo Pro RGB | 4 ASICs; exact chip not established | Separate vendor product page; dedicated firmware target unresolved |
| Bitaxe Turbo Touch | Separate display accessory | ESP32-S3, ST7262 display, GT911 touch, and BAP protocol |

The Nerd source also defines NerdAxe Gaia, Q1370, and Q1373.
These records identify code targets. A complete primary board design is not yet archived for each target.
A published design can be incomplete or restricted. Read its status and licence before use.

The archive contains legacy ASIC drivers and guides from CGMiner and BFGMiner.
These cover USB sticks, early miners, and separate hashboards.
Examples include GekkoScience, ASICMINER, Butterfly Labs, Bitfury boards, HashFast, KnCMiner, and Rockminer.
Use the driver-specific constraints. A fork can support a device that upstream CGMiner does not support.

## Select a hardware and firmware combination

Use the exact device model, PCB revision, control processor, and memory variant.
Then select a firmware image for that combination.
Do not infer support from the ASIC model alone.

ESP-Miner uses board configuration files such as `config-601.csv`.
The captured source has 22 explicit board configurations.
The normal current build requires an ESP32-S3-WROOM-1 N16R8 module.
That module has 16 MB flash and 8 MB octal PSRAM.
Other memory variants can require a different build.
See the [archived ESP-Miner guide](../work/open/inspect/open-bitaxeorg-esp-miner/readme.md).

Factory images, application images, and web UI images have different purposes.
Do not interchange these files.
Older AxeOS releases use a separate web UI image.
The captured HEAD source also describes a unified image with an embedded web UI.
A rollback can leave an incompatible old web partition in place.
The selected release binary and the captured source HEAD can differ.
The release-source index records the separate commit named by each published release tag.
That association does not prove that a new build will match the published binary byte for byte.

Nerd firmware uses board-specific builds.
For example, NerdQAxe+ and NerdQAxe++ have different ASICs.
Their filenames differ by a plus sign.
Artifact IDs use the numeric GitHub asset ID to preserve that distinction.
The source also has revision-specific power and temperature control.
See `main/boards/` in the archived NerdQAxePlus source.

NMAxe firmware stopped universal images at version v3.1.01.
Select the exact device and PCB revision.
NMQAxe++ Nexus uses two BM1373 chips in series voltage domains.
Its requested per-chip voltage is scaled for the full rail.
The separate Rev8.1 source class uses four BM1373 chips in parallel.
Its build environment is disabled in the captured default configuration.
These two designs must not share one power model.

Serpent Quad Miner schedules complete jobs across up to four pools.
It does not dedicate each physical ASIC to a separate pool.
The maintainer reports a factory-image power-start fault in v0.1.0 and v0.1.1.
The stated correction is v0.1.2.
Keep this failure in a version-specific simulation case.

Jingle Lite and Pro need different supply and image profiles.
Lite uses a 5 V supply and the `NERDAXEGAMMA` source target.
Pro uses a 12 V supply and the `NERDQAXEPLUS2` source target.
The public release files are factory images. The release names identify the product.
The source applies an estimated 5 W board loss to Lite power telemetry.
It also adds a 10 °C correction to Pro temperature telemetry.
Preserve these source behaviours. Do not treat them as measured calibration.
The separate Pro RGB product does not yet have a verified image mapping in this archive.
The current vendor web flasher requires sign-in. Public GitHub release files are archived separately.

PiAxe runs a host miner on a Raspberry Pi.
It uses UART for ASIC data and I2C for board functions.
Its reference design uses a fixed 1.2 V ASIC rail.
It does not use the same power-control circuit as Bitaxe Ultra.

EmberOne/00 needs a separate USB host.
The board designer identifies BM1362 AA, AB, AC, and AD as usable variants.
AI and BD are untested in that guide. AJ and AK are excluded.
Preserve these suffixes in a simulator profile.
See the [EmberOne readme](../work/open/inspect/open-256foundation-emberone00-pcb/README.md).

## Important evidence conflicts

The FOSS project list names BM1370XP for Gamma Duo.
The firmware configuration names BM1370.
The catalogue keeps both claims and an open gap.
It does not assert that every BM1370 variant is interchangeable.

The Supra readme has old BM1366 text and an old statement that BM1368 support is pending.
The captured ESP-Miner source contains BM1368 support configurations.
Use the source date and board revision to resolve this difference.

The Ultra Hex guide identifies hardware faults in revisions 300, 301, and 304.
A working driver does not correct a hardware design fault.
The same guide describes a correction for revision 304.
See the [Ultra Hex readme](../work/open/inspect/open-bitaxeorg-ultrahex/readme.md).

The BAP-GT-TOUCH readme specifies 8 MB flash. Its captured default build specifies 16 MB.
The default partition table extends beyond 12 MiB. An older saved configuration specifies 8 MB.
Select a matching accessory board, image, and partition map before simulation.

The CGMiner guide states that its ASICMINER Tube/Prisma driver is broken.
Those relationships have status `unsupported`.
No relationship in this area has status `tested`.

Current Gekko product records include KBox, KBeast, KBuddy, Mein Coffee, Terminus A2X, and Compac A1/A2.
Their product pages do not identify every fitted board, chip count, or firmware package.
Some pages describe preorders. An announced availability date is not a shipment date.
The Compac A1 and A2 chip mappings come from the Kano driver.
The Compac A2 V2 temperature-calibration patch link returned HTTP 404.
Five Kano Windows builds and their exact source commits are archived.
Each ZIP and embedded executable matches the maintainer's published SHA-256 value.

The APW12 archive has 14 PIC16F1704 firmware dumps.
All files pass Intel HEX record checks. Some filenames are marked Bad.
A valid record checksum does not prove correct PSU behaviour.
The EEPROM tool archive contains six binary fixtures and 50 topology configurations.
Those configurations are independent community evidence.
Its board table contains disputed model and chip claims.
Full English translations preserve those claims and state their limits.

## Controller interface tools

The Amlogic control-board tools provide Rust source and seven AArch64 binaries.
They cover an observed A113D control board with S19j Pro hashboards and an APW12 PSU.
The source identifies each UART, reset line, board-detect line, I2C address, fan output, and tachometer input.
The boot note identifies 256 MiB RAM, Micron NAND, U-Boot, UBIFS, and SysV init.
It separates observed facts from boot-chain inferences.
Use [amlogic-interface-contract.json](../catalog/parts/open/amlogic-interface-contract.json) as a reference model input.
The archive process did not run its installers or service-disable scripts.
The APW12 contract also records frame bytes, command IDs, and parser behaviour.
The source checks only the low receive-checksum byte. Preserve this behaviour in a source-specific parser test.
Its voltage formulas are implementation assumptions. They are not universal PSU calibration values.

The A113X1 development-kit guide is archived separately. It is an English mirror of an Amlogic document.
It describes a development kit, not an exact Antminer control-board design.
The larger Amlogic archive includes other SoCs and legacy software. Do not apply those pin maps to an A113D board.
That archive was assembled locally from 77 verified Git blobs at one fixed commit.
It preserves all 105 file paths and their bytes. It is not a byte-identical GitHub codeload file.
Its manifest and Git-tree inventory record this distinction.

DCENT Toolbox is a management and installation-planning tool.
Its public documentation has different installation limits for each controller family.
S9 is public-gated. AM2 S19 Pro and S19j Pro have lab routes.
Plain S19 has vendor recovery evidence, but no established DCENT install path.
BeagleBone requires physical media. Amlogic support depends on the stock version and proof gates.
CV1835 OS installation remains denied in the captured source.
A native-mining claim does not prove that a public installer is ready.
See the [captured platform matrix](../work/open/inspect/open-dcentraltech-dcent-toolbox/docs/PLATFORMS.md).

## Use this evidence to design a simulator

Use separate models for the network, control processor, board peripherals, ASIC chain, and physical response.
Select the required model level before work starts.

| Model level | Required behaviour | Useful archived evidence |
|---|---|---|
| Management API | Requests, responses, state, events, and errors | CGMiner API guide; AxeOS OpenAPI file; Mujina API guide |
| Pool client | Job lifecycle, difficulty, target, nonce fields, reconnect, and share submission | Stratum V1 code; V1 extension spec; Stratum V2 spec and implementation |
| Board interface | Reset, UART, I2C, GPIO, ADC, PWM, fan tachometer, and power state | Schematics, BOMs, board classes, component manuals |
| ASIC chain | Enumeration, addressing, registers, jobs, CRCs, nonce responses, and version rolling | BM13xx reference; CGMiner drivers; ESP-Miner ASIC code |
| Firmware execution | CPU, ROM, flash map, RAM, interrupts, timers, DMA, and device models | ESP32-S3 technical reference manual, source configuration, firmware images |
| Thermal and power response | Power loss, sensor delay, cooling response, frequency limits, and faults | Board topology and measured device traces; most calibration traces are still missing |

An API simulator does not prove that an unchanged firmware image will boot.
A CPU emulator also needs the board peripheral models used by that firmware.
Firmware boot support does not prove realistic power or thermal behaviour.
Keep these claims separate in the test report.

The [BM13xx reference](../work/open/inspect/open-256foundation-mujina/mujina-miner/src/asic/bm13xx/REFERENCE.md) is a strong starting point.
It is a community reconstruction, not a Bitmain datasheet.
It explains chip chains, voltage domains, register commands, CRCs, byte order, nonce allocation, and version rolling.
BM1362 and BM1366 derive nonce allocation from chip addressing.
BM1370 also uses an explicit nonce-offset register.
The simulator must preserve these model-specific differences.

Mujina currently identifies Bitaxe Gamma and its CPU backend as working targets.
Its broad hardware support statement is a project objective.
EmberOne support is described as in progress. Libre Board and S19 images are target work.
Do not turn those roadmap statements into tested compatibility.

For Bitaxe Gamma, Mujina uses RHAP firmware on the ESP32-S3.
RHAP exposes two USB serial channels.
One channel controls board functions. The other passes ASIC UART data.
RHAP replaces the stock miner application and performs no mining by itself.
See the [RHAP guide](../work/open/inspect/open-256foundation-rhapd-bitaxe-gamma/README.md).
This interface is useful for a host-side ASIC model and for later hardware trace capture.

The BFL laboratory source has two useful host-protocol simulators and saved characterization data.
Its BFL nonce model hashes serialized midstate bytes as message data and uses an artificial one-byte target.
Its Icarus model returns a fixed known nonce for its golden input and a deterministic value for other inputs.
These models support protocol tests. They do not prove Bitcoin share validity.
Their thermal model changes temperature per work item and status poll. It is not a calibrated physical model.
The separate BitForce SC firmware archive provides AVR32 C source and an Atmel Studio project.
Its default build selects Single, but one identification string says Jalapeno.
Use the compile options and board profile to identify the target.
The December 2012 vendor protocol PDF is archived as a draft document.

Use known block headers and expected nonces for deterministic tests.
Test invalid CRCs, old jobs, duplicate shares, timeout paths, disconnects, missing chips, bad sensors, and fan faults.
Record the exact firmware hash and board profile in each result.
Synthetic hashrate and temperature values must be labelled as simulation inputs.

## Dates, languages, and build limits

A hardware-source release date is separate from the first retail shipment date.
Twenty-one device or accessory records have model-specific CAD history dates.
Records that use a CAD history date have date basis `earliest-known-model-design-file-commit`.
A published hardware-source release date takes priority in the main date fields.
The evidence gives the exact file, commit, and date. File renames and imports can hide earlier history.
Common repository ancestry dates are in a separate index.
For example, several Bitaxe models share a 2022 ancestor. That does not make 2022 their release year.
Most first retail dates remain unknown.
The catalogue uses `null` where the evidence is missing.
Release metadata keeps exact published timestamps for firmware versions.
An Atom feed update time is not used as a product launch date.

English companion files are in [translations/open](../translations/open/README.md).
The original archives stay unchanged.
The translation records state whether a file is an upstream English version, a translated document, or an extracted bilingual section.
The four flagged NMAxe HTML files have English prose companions.
The two primary Russian EEPROM documents have full English prose translations.
Code bytes stay unchanged.
Unreviewed PDFs, code comments, and UI locale text remain a language-review gap.

GitHub source archives omit submodule contents.
The dependency index records paths, URLs, exact gitlinks, and archive status.
The direct dependencies of the main captured HEAD sources are archived at their exact commits.
This includes CCAN from its original non-GitHub source.
Nested dependencies, historical release dependencies, SDKs, and package dependencies need further review.
A complete source archive is therefore not yet a reproducible offline build environment.

Use `work/open/firmware-history.log`, `work/open/firmware-history-helper.log`, and `work/open/extra-firmware-history.log` to inspect ASIC download progress.
All 3,148 known release assets from ten projects are downloaded, with 34.80 GB of payload bytes.
All 257 tagged source-release archives are downloaded.
They represent 256 unique repository and commit pairs.
The separate source-release status index gives the exact commit and result for each release.
This is a snapshot of those public release histories. It is not proof that every mining project on the Internet has been found.
Use each artifact manifest to determine the actual result.
The job is repeatable and reuses completed files.
Historical CPU, GPU, FPGA, and NerdMiner collection stopped after the scope correction.
Existing supplemental files are retained with explicit scope labels.

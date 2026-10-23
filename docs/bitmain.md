# Bitmain Bitcoin ASIC catalogue

This area covers Bitmain Bitcoin ASIC miners and their support software. The main scope is SHA-256d mining. It includes devices sold for BTC, BCH, and BSV. It also includes control boards, hashboards, power supplies, ASIC chips, firmware, and tools that these devices use.

The first discovery pass covered other algorithms. Those records and files remain in the archive. Their scope is `supplementary-out-of-scope`. They are not Bitcoin compatibility evidence.

This is an evidence catalogue. It is not a list of tested combinations. No downloaded mining firmware or mining tool was run.

## Find a record

| Path | Use |
| --- | --- |
| `catalog/parts/bitmain/entities.jsonl` | Find a model, board, chip, PSU, firmware build, or tool. |
| `catalog/parts/bitmain/relationships.jsonl` | Find a compatibility claim. Read its source, status, and constraints. |
| `catalog/parts/bitmain/sources.jsonl` | Find the source URL and archive ID for a claim. |
| `catalog/parts/bitmain/gaps.jsonl` | Find missing evidence and the next collection step. |
| `catalog/parts/bitmain/artifact-scope.jsonl` | Separate Bitcoin files from broad references and supplementary files. |
| `catalog/parts/bitmain/oem-download-index.jsonl` | Find each exact OEM model-group assignment, file name, URL path, and index date. |
| `catalog/parts/bitmain/hiveon-download-index.jsonl` | Find historical Hiveon files, directory context, sizes, and symlink aliases. |
| `catalog/parts/bitmain/awesome-download-index.jsonl` | Find Awesome Miner model, controller, version, fee, and install-method variants. |
| `catalog/parts/bitmain/dcent-release-index.jsonl` | Find each DCENT_OS release asset, its date, and its release tag. |
| `catalog/parts/bitmain/braiins-feed-index.jsonl` | Find each public Braiins feed URL, exact build label, target, listed size, checksum or signature file. |
| `catalog/parts/bitmain/braiins-release-index.jsonl` | Find 73 releases with exact release dates, supported models, package roles, and SHA-256 values. |
| `catalog/parts/bitmain/braiins-final-url-status.jsonl` | Check the final result for all 2,444 feed, release, and historical package URL jobs. |
| `catalog/parts/bitmain/platform-boot-profiles.jsonl` | Find seven exact controller examples, declared memory, boot data, partitions, and evidence limits. |
| `catalog/parts/bitmain/platform-fdt-properties.jsonl` | Search 772 literal device-tree nodes from six saved DTBs. |
| `catalog/parts/bitmain/platform-fit-properties.jsonl` | Read FIT image and configuration metadata; image bytes are represented by hashes and offsets. |
| `catalog/parts/bitmain/news-index.jsonl` | Find 37 saved OEM announcement bodies and their publication labels. |
| `catalog/parts/bitmain/nicehash-bundle-index.jsonl` | Find extracted custom firmware and restore members with separate roles and targets. |
| `catalog/parts/bitmain/mskminer-checksums.jsonl` | Find expected and actual MD5 values, including mismatches. |
| `catalog/parts/bitmain/additional-download-index.jsonl` | Find NiceHash and MSKMINER firmware, tools, recovery packages, and manuals. |
| `catalog/parts/bitmain/braiins-profiles.jsonl` | Find vendor power profiles. These values are settings, not physical calibration measurements. |
| `catalog/parts/bitmain/repair-document-index.jsonl` | Find repair-shop manuals. These are secondary sources. |
| `catalog/parts/bitmain/repair-image-index.jsonl` | Find archived diagrams linked from those manuals. |
| `catalog/parts/bitmain/ocr-register.jsonl` | Find OCR page results, confidence, and text paths. |
| `catalog/parts/bitmain/document-languages.jsonl` | Find PDF text-layer language checks and page counts. |
| `catalog/parts/bitmain/translations.jsonl` | Find English companion files and their translation scope. |
| `artifacts/manifests/<artifact-id>.json` | Check download status, file path, size, source URL, and SHA-256. |
| `translations/bitmain/` | Read English translations. Keep the linked original for diagrams and source checks. |
| `work/bitmain/pdf-text/` | Search extracted PDF text. Text extraction can omit diagrams. |

An index row does not prove that a file downloaded. Check the artifact manifest. A failed request remains recorded. A symlink alias can be index-only. The same bytes can occur at several URLs.

## Read compatibility correctly

Start with the exact device ID. Follow its `uses-hashboard`, `uses-control-board`, `uses-chip`, and `uses-psu` records. Then find a firmware `supports` record for that device. Read all constraints on that record.

A control-board family is not a PCB revision. A model can use Xilinx Zynq, Amlogic, BeagleBone, or CVITEK boards. A package for one family does not establish support for the other families. A hashboard ID can end in a dash. That dash is part of the source identifier.

The OEM firmware list gives a model-group claim. It does not prove support for every board that was sold under that model name. Braiins and LuxOS give more specific board tables. Their tables have different support limits. Keep the claims separate.

PSU replacement can be one-way. For APW121215, the OEM separates a/b/c revisions from d/e/f revisions with voltage feedback. The former cannot replace the latter when feedback is required. A firmware upgrade can be needed for a permitted replacement in the other direction. Check the exact output-voltage range. The APW121417 a/b rule is also one-way.

The `unsupported` status means that the cited source says the mining path is absent or blocked. It is not evidence that a simulator has tested and rejected a combination. A missing edge means that evidence is absent. It does not mean that the hardware is electrically compatible.

## Main firmware groups

| Group | Evidence retained | Main limit |
| --- | --- | --- |
| Bitmain stock firmware | Public OEM model tree, per-model file lists, binary updates, recovery files, manuals, compliance files, chip data sheets, and public source repositories. | Complete matching source is not established for each current binary. |
| Braiins OS | Current and legacy support tables; exact hashboard IDs; chip geometry; PSU constraints; API and install documents; public images; build components. | Current commercial firmware and old open source are separate products. Licence renewal can affect runtime behaviour. |
| Historical Braiins OS / BOSminer | Community-preserved source fork with the S9 backend, FPGA I/O design, SVD register map, and a Python Stratum simulator. | No byte-equivalence to current binaries. The old Stratum V2 draft differs from the current protocol. |
| LuxOS | Model and controller matrix, hashboard limits, change log, installer, Commander, and stock recovery images. | The actual current LuxOS custom firmware payload is not available through the collected anonymous download route. Stock recovery images are not LuxOS custom firmware. |
| VNish | Public model aliases and exact version/controller/install-method rows; public install and update links. | Some historical release-candidate links return HTTP 403. A failed link is not an archived binary. |
| Hiveon | Version index and historical directory index; custom firmware, stock-plus-client packages, recovery files, and tools. | Stock-plus-client is not the same mining implementation as Hiveon custom firmware. |
| Awesome Miner | Public model/controller/version/method matrix and distinct fee/licence package variants. | A NAND installer is not a direct stock-firmware update. Read the vendor install constraints. |
| NiceHash Firmware powered by MARA | Full English guide from its public content API, current multi-model bundle, NiceHashTools, ASIC Manager, and historical 2020 guide. | A bundle has multiple algorithms and control-board targets. The Bitcoin support list does not verify every physical board revision. |
| MSKMINER | Current model and controller claims, HTTP API reference, installer tools, legacy images, recovery packages, and translated Russian manuals. | New models use the vendor utility. The S9 six-board conversion labelled S10 is not established as an OEM S10 model. |
| DCENT_OS | Source snapshot, exact platform-status document, beta release assets, proof files, and checksums. | A package or a dashboard does not prove an operational mining backend. Read the per-model status. |

The Braiins public feed scan found 236 directory responses and 1,968 file URLs. The index includes historical images, update packages, toolbox versions, checksums, and signatures. A separate primary release page lists 73 releases and 477 assets. It exposes 160 additional public URLs, including current Amlogic packages. Thus, the directory index alone is not complete. Exact build targets have separate platform records. A target name is not a board pinout. Artifact manifests distinguish queued files, downloaded files, and failed requests. Supplier SHA-256 checksums permit byte sharing between duplicate URLs after a checksum match.

The final Braiins pass has 2,444 distinct URL jobs. Payloads are available for 2,306 jobs, including 182 aliases with a matching supplier SHA256. These records contain 2,111 distinct payload hashes and 36,064,121,174 unique bytes. The remaining failures are two firmware URLs with HTTP 403, 134 historical metadata or package URLs with HTTP 404, and two empty metadata responses. No job remains queued. All 785 available artifacts with a supplier SHA256 match it.

The NiceHash/MARA bundle contains seven custom firmware images. Each image matches its bundled SHA256. The member index separates these from stock restore files. All 103 selected DCENT_OS release assets were saved and match the publisher SHA256. These checks establish byte identity, not a successful boot.

Some old Hiveon version links now return HTTP 503. Twelve files were found under the vendor archive paths and matched to the old index by the published MD5. Their manifests retain the failed original URL and the successful download URL. Four old Bitcoin package URLs remain unavailable.

MSKMINER publishes MD5 values for 22 collected files. Twelve match and ten do not. The mismatches remain explicit in the manifests and firmware records. The actual SHA-256 identifies the saved bytes. It does not resolve the supplier checksum conflict. Do not use a mismatched payload as a verified reference fixture until that conflict is resolved.

A VNish release-candidate probe returned an unsigned S3 URL with `AccessDenied`. A cache-busting repeat returned the same result. This failure is not an expired signed download URL. The audit is `work/bitmain/vnish-403-audit.json`.

DCENT_OS has maintainer reports of accepted pool shares for selected S9, S19j Pro, S21, and S19k Pro setups. These reports have different dates and test limits. Some do not apply to the latest release binaries. Several other platforms are management-only, package-only, or blocked. The catalogue preserves these limits. It does not label the whole family production-ready.

## Read dates

`release_year` is the year of the event stated by the source. Read `date_basis`, `release_event_type`, and the notes. A launch announcement, sale opening, planned delivery, and first shipment are different events.

The OEM retrospective history supplies many years. Contemporaneous announcements take precedence when the dates conflict. S7 sales were announced for 30 August 2015. S19 and S19 Pro were announced on 27 February 2020. S19j and S19j Pro were announced in November 2020, with deliveries planned for 2021. The latter date is not proof of actual shipment.

The area has 44 Bitcoin device records with a supported release year. Saved OEM news establishes separate S17/S17 Pro sales on 9 April 2019 and T17 sales on 28 April 2019. It also supplies dated S21-family announcements. Where an article gives an event range or an offer for an existing model, the record preserves that distinction. Unknown years remain null.

An OEM index `createTime` can predate the binary that is now at that index entry. The catalogue stores it as an index date. It does not silently use it as the firmware release date. A timestamp inside a filename is build evidence only until a release notice confirms it.

## Use this evidence for a simulator

Build a device profile from a fixed set of component revisions. Use the binary SHA-256 as the firmware identity. Keep the update package, recovery image, and extracted root filesystem as different artifact types.

For each profile, record these items:

1. The control-board PCB and SoC. Record CPU architecture, RAM, flash geometry, and boot media.
2. The boot chain. Record the bootloader, kernel, device tree, root filesystem, and signature policy.
3. The hashboard PCB. Record connector pinout, ASIC type, ASIC count, voltage domains, PIC or other MCU, and EEPROM layout.
4. The transport. Record UART, SPI, GPIO, I2C, FPGA registers, job packet layout, nonce format, checksums, and timing.
5. The sensors and actuators. Record temperature sensors, fan tachometers, fan control, PSU commands, and fault responses.
6. The network interfaces. Record Stratum version, management API, configuration format, licence service, and developer-fee behaviour.
7. The physical parameters. Record measured frequency, voltage, power, temperature, errors, and valid shares under a stated ambient condition.
8. The test evidence. Record the exact firmware, board serial or revision, trace method, test duration, and observed result.

The present archive supplies source material for many of these fields. It does not supply a complete physical model for every device. Vendor power profiles do not establish thermal capacitance, airflow, regulator loss, or silicon error curves.

Start with an API simulator when the goal is fleet-management tests. Add a job-and-share model for pool tests. Use a full-system emulator only when the goal requires the original kernel and processes. A Linux image that reaches a login prompt is not proof that mining works.

Useful source entry points follow. Read their licences before reuse.

| Artifact ID | Path inside the source archive | Use |
| --- | --- | --- |
| `bitmain-source-bmminer-mix` | `driver-btm-c5.c`, `driver-btm-c5.h`, `readme.txt` | Historical S9/R4/T9/T9+ target selection and stock FPGA device interfaces. |
| `bitmain-source-ar9331-s1s3-compile-env` | OEM S1/S3 build environment | Early AR9331/MIPS control-board software. |
| `bitmain-source-cgminer` | `API-README`, `ASIC-README`, `driver-*.c` | Historical management API and ASIC transport drivers. Select only Bitcoin ASIC backends for the main simulator. |
| `braiins-legacy-bosminer-fork` | `open/bosminer/bosminer-am1-s9/` | Historical Rust S9 mining backend. |
| `braiins-legacy-bosminer-fork` | `open/hw/zynq-io-am1-s9/fpga-io/map.svd` | Register map for that exact Braiins FPGA design. |
| `braiins-legacy-bosminer-fork` | `open/protocols/stratum/sim/` | Historical discrete-event share, latency, and proxy models. |
| `source-braiins-bcb100` | PCB, schematic, and README files | Open BCB100 replacement controller. The source claims support for S19 models except Hydro. |
| `source-dcentraltech-dcent-os` | `DCENT_OS_Antminer/docs/PLATFORMS.md` and backend crates | Modern experimental backend design and explicit per-platform evidence limits. |

Do not apply the Braiins S9 SVD to a stock Bitmain FPGA image. They can expose different registers. Do not replace missing chip behaviour with a constant hash rate when the test depends on clock changes, nonce validation, thermal control, or faults.

Seven controller profiles have direct image or observed-board evidence. The machine-readable register preserves archive SHA256, member SHA256, DTB offset, decoded text path, and raw property values. The six DTBs contain 772 nodes. All six internal FIT DTB hash checks that are present match. FIT signature bytes are retained; trusted-key signature verification was not done.

| Exact example | CPU and memory evidence | Limit |
| --- | --- | --- |
| S9 Braiins OS 22.02.4 | Zynq-7000 profile; two Cortex-A9 nodes; DTB declares 1 GiB at address zero. | Installed RAM is unverified. An older U-Boot source patch declares 512 MiB. Do not apply this profile to S9 SE. |
| S19j Pro BBCtrl recovery 20210622, distributed by Luxor | AM335x profile; one Cortex-A8; DTB declares 256 MiB at `0x80000000`; 12 NAND partitions. | The physical PCB and memory-package markings are unknown. The partition span does not prove NAND capacity. |
| S19k Pro-labelled CV recovery 20231115, distributed by MSKMINER | Two CV1835 miner profiles declare two Cortex-A53 cores and 256 MiB at `0x100000000`. Separate development profiles declare 512 MiB and 1 GiB. | The bundle contains four selectable DTBs. Their presence does not prove that all are retail S19k Pro configurations. `linux,dummy-virt` does not establish QEMU compatibility. |
| Observed Amlogic A113D S400 board with S19j Pro hashboards | Maintainer reports 256 MiB RAM, Micron MT29F2G08-A 2-Gib NAND, U-Boot, Linux 4.9.241, and LuxOS. | This is one community-observed board. The catalogue did not reproduce the observation. |

Use `catalog/parts/open/amlogic-interface-contract.json` for the observed Amlogic PSU, fan, GPIO, sensor, EEPROM, and hashboard interfaces. Its source commit is `2439d6061c20d0294da66875f2fa6390c25b0ffb`. The encrypted S21 Amlogic recovery image is preserved. Only its public configuration and container members were inspected.

## Translation and OCR

The original documents remain unchanged. English companions state which pages or text layers they cover. A translated compliance appendix is not a translation of text embedded in a diagram. OCR output is machine transcription. It can confuse a board identifier, a unit, or a serial number. Check such values against the original page before they become simulator requirements.

The translation register contains 157 companion records. It includes 112 OEM text or OCR companions, 22 repair appendices, 10 publisher English repair counterparts, three Russian MSKMINER manuals, one residual Russian table label, eight document-media label or incidental path companions, and one Chinese configuration-comment companion. A publisher counterpart can have a different revision date. Those records state whether explicit dates match. All 117 selected Bitcoin/shared scanned OEM PDFs received an OCR attempt; the register contains 132 processed pages.

For the shared document-media collection, 342 static images received OCR and all 49 non-English language candidates received visual review. Four contain substantive Russian labels; their English companions are complete for the visible labels. Four other images contain the incidental Russian path word `тесты`, translated as `tests`. False language detections remain recorded. Four GIFs received first, middle, and last-frame checks. Those twelve sampled frames are English. The full animations were not reviewed frame by frame. The one missing media file is an unrelated site-template example, outside the main scope.

The language register distinguishes readable English text, mixed text, Korean text, Chinese text, and documents with no useful text layer. Separate OCR records preserve page numbers and confidence. A language label inferred from characters is a candidate label, not a human certification.

## Reproduce the area

Run `python work/bitmain/build_catalog.py` to rebuild the area JSONL records from the saved source data. It does not download or run firmware. Download collectors are separate. `work/bitmain/index_braiins_feed.py` rebuilds the feed index. `work/bitmain/index_platform_profiles.py` rebuilds controller profiles from the saved extracted members. Download logs are under `work/bitmain/`. Download jobs use `scripts/fetch_artifact.py` and preserve failure records. Do not restart a failed HTTP 429 queue while the supplier still applies a rate limit.

The collection is a dated snapshot of public sources. A complete list of every private build, removed release, repair schematic, or undocumented board revision is not available. Use `gaps.jsonl` to track each unresolved item. Add new evidence without removing its original source, version, or scope.

## Additional firmware distributors

QuantumHash has 63 model and controller rows in `quanthash-model-index.jsonl`. The installer and linked stock recovery images are in `breadth-download-index.jsonl`. The installer contains five BMU payloads for ALL, AML, BB, CV, and XLNX targets. They are extracted unchanged and indexed in `quanthash-bundle-index.jsonl`, at version `0.9.9.5.4224-4229_PncN6bGA_Q`. Its page does not identify an upstream code base. The catalogue leaves this unknown. Two recovery links are explicit cross-model shortcuts. They are not evidence that an OEM image supports another model. Current Amlogic stock builds from September 2025 onward are outside the vendor installation route.

EMCD explicitly distributes VNish firmware. The current web page links version `1.2.6-rc5`. The supplier parent directory also exposes `1.2.6` and `1.3.4`; these are indexed separately in `emcd-extra-download-index.jsonl`. The historical directory includes S9 Xilinx `3.9.0` and S17/T17 variants `2.0.4`. Each indexed package retains its model, controller, storage, and install or update role. `breadth-download-index.jsonl` links each file to the supplier directory.

EMCD collection stopped when a low-rate retry again returned HTTP 429. There are 68 saved records and 425 failed records, including reference pages in the totals. The index does not count a failed URL as an available binary.

ASICFW.io mentions the Bitmain 19 series, but its exposed model catalogue lists only L7 and D7. No exact Bitcoin image is established. CrMi / ASIC-Firmware direct pages and documents returned HTTP 403. These entries remain research gaps. They are not counted as verified available Bitcoin payloads.

The OEM TEST-pool repair article provides five special firmware attachments. Their purpose is to repair a fixed pool configuration, then permit the correct latest OEM image. The article lists S19k Pro recovery only through customer support. Do not use these special files as unrestricted upgrades.

## Payload integrity

`upstream-integrity.jsonl` compares archived files with exact supplier sizes and checksums. `oem-integrity-discrepancies.jsonl` records independent HTTP object lengths for OEM mismatches. All eleven discrepant OEM objects match the current HTTP length; the API metadata differs. This does not prove that the content is valid.

The primary `SD-S5-20150107.tar.gz` and a Git-verified mirror contain the same truncated gzip stream. They have 5,750,784 bytes; the OEM API expects 19,109,718 bytes and a different MD5. The archive preserves the damaged bytes and marks `payload_available=false`. A complete historical OEM copy was recovered from the Internet Archive capture dated 7 May 2016. Artifact `bitmain-s5-20150107-wayback` has the expected 19,109,718 bytes, matches OEM MD5 `0c30d32d68e64f528da06af19b23d859`, and passes a complete gzip stream read. This resolves file availability for that S5 release. It does not prove a successful boot. The S5+ image still has a published MD5 mismatch.

Ten MSKMINER files do not match their published MD5. Their actual SHA256 and the supplier mismatch remain visible. A downloaded file, a matching checksum, a valid archive, and a tested boot are separate states.

The Braiins checksum map also reads `Packages` and `Packages.gz`. It records 316 additional historical package references outside the directory listing. These are small metadata packages, not complete miner images. `braiins-package-reference-index.jsonl` preserves those URLs and expected hashes. The resume collector uses an existing payload only when its SHA256 matches the supplier hash. It uses checked byte ranges for large remaining files.

The final supplier audit covers 2,509 artifacts. It finds eleven OEM metadata discrepancies and 178 Braiins package-size discrepancies. Each affected Braiins package matches its exact published SHA256. Its stale `Size` field is preserved as a supplier metadata conflict, not a transfer-truncation claim. The available Braiins files have no publisher SHA256 mismatch.

`source-archive-identity.jsonl` pins saved GitHub tar snapshots from the archive global PAX commit marker. This identifies the collected bytes. It does not assume that a mutable branch still points to the same commit.

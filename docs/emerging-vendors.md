# Auradine, Bitdeer, Proto, ePIC, HashCore, and support software

## Auradine Teraflux

The hardware guide covers AT1500, AT2870, AT2880, AI2500, and AI3680.
AT models use air cooling.
The listed AI models use single-phase immersion cooling.
AT1500 and AI2500 require a MicroSD image.
The other three models use internal eMMC and can also boot from MicroSD.
The guide does not identify a complete ASIC register map or every internal board revision.
See the [manufacturer hardware reference](https://auradine.com/wp-content/uploads/2025/07/Teraflux-AT-AI-Series-Miner-Hardware-Reference.docx.pdf).

The public download application lists FluxOS update files and SD images.
The catalogue preserves its model labels, version, build date, Git hash claim, size, and checksum.
A broad `AT,AI,AH` label does not establish support for every board revision.
See the [supplier download service](https://update.auradine.com/).

Some supplier metadata conflicts with the saved files.
The FluxOS 2.3.1 ZIP has two checksum claims.
One matches the saved ZIP.
The listed ZIP size is also inconsistent.
The FluxOS 3.0.2 BIN checksum does not match the measured hash.
A second download returned the same measured hash.
The manifests preserve both observations and all supplier claims.
Resolve these conflicts before you use these images in a validated profile.

## Bitdeer SEALMINER

The public download API links each file to exact product categories.
The catalogue preserves the API response and the complete file metadata.
It separates update files, recovery images, manuals, and the API guide.
A2 power-input variants have separate firmware requirements.
Read the exact description before you select a payload.
See the [official firmware page](https://www.bitdeer.com/shop/support/firmware).

The roadmap associates A1, A2, A3, and A4 families with SEAL01, SEAL02, SEAL03, and SEAL04.
The catalogue labels roadmap dates as milestones or estimates.
An A4 roadmap date is not proof of shipment.
A family milestone is not a separate launch date for each Air, Hydro, or Pro variant.
See the [manufacturer roadmap](https://www.bitdeer.com/sealminer/roadmap).

The July 2026 manuals also name A4 Pro Air, A4 Pro Hydro, and A4 Ultra Hydro.
These model records preserve the manual evidence. Their exact shipment dates and firmware packages remain unknown.
The [English companion files](../translations/emerging/) contain the publisher's complete English manual sections.

Eleven update files contain a gzip stream followed by an installer script.
The [image layout index](../catalog/parts/emerging/bitdeer-image-layouts.jsonl) records each byte offset and checksum.
The scripts declare miner, controller, and PSU target values.
Controller labels include `taurus_air_v1.1`, `taurus_hydro_v1.1`, and `taurus_hydro_v1.2`.
These are firmware target labels. The collection has not verified matching PCB markings.
The readable installer scripts have separate artifact records. They are not complete firmware source releases.

## Block / Proto

The MDK assembly guide shows three controller-to-hashboard USB connections.
The catalogue separates the MDK, its control board, and its hashboard.
The public firmware guide gives version information.
The onboarding guide directs users to support for firmware access.
No public MDK firmware payload is claimed as downloaded.
See [MDK assembly](https://www.mining.build/manuals/hardware/miner-assembly/) and [firmware onboarding](https://www.mining.build/manuals/firmware/onboarding/).

Proto Fleet source is saved at a pinned commit.
It provides a separate reference for device management and API support.
Its support matrix has feature-specific limits.
It is not a firmware image.
See [Proto Fleet](https://github.com/block/proto-fleet).

## ePIC

The catalogue separates the ePIC UMC replacement controller from UMCOS for stock controllers.
It also separates Antminer, WhatsMiner, and BlockMiner release families.
Each public release asset has a version, publication date, URL, and artifact record.
See the [official download page](https://epicblockchain.io/support/firmware-download/).

The UMC guide shows a system on module and a carrier board.
S21 Pro, S21 XP, and S21+ need a different carrier board.
J1 is disabled by default.
These limits remain attached to the compatibility records.
See the [April 2025 UMC guide](https://epicblockchain.io/wp-content/uploads/2025/04/ePIC-UMC-Complete-User-Guide-for-Antminer_last-updated-April-2025.pdf).

A release filename can identify a controller family or a legacy S19 package.
The catalogue treats such names as target hints.
They do not prove that every device or revision in that family works.

## HashCore and ASIC.TO

ASIC.TO publishes firmware choices by model, controller, method, and version.
The Bitcoin subset excludes L7, L9, and other Scrypt targets.
The controller labels include AML, BB, CV, and XIL.
The catalogue preserves these labels with each file.
See [ASIC.TO](https://asic.to/).

HashCore Toolkit is a management tool.
HashCore Proxy routes Stratum connections.
The English documentation is archived separately from executable packages.
A management API or proxy implementation can support simulator tests.
It does not provide a model of the physical ASIC.
See [Toolkit](https://hashcore.com/toolkit), [Proxy](https://hashcore.com/proxy), and [the English documentation](https://docs.hashcore.com/en/).

The proxy archive includes the public deployment repository and OCI container data for amd64 and arm64.
Container manifests and layers have checked SHA-256 values.
The deployment configuration and the closed application binary have separate records.
No container was started.

## BiXBiT

The public archive contains stock, transition, and recovery firmware for Bitcoin Antminer and WhatsMiner models.
Do not identify these stock files as BiXBiT custom firmware from the download address alone.
The current custom Antminer page directs users to request access. A standalone custom Antminer payload was not obtained.

The archived WhatsMiner packages include versions 1.8.1 and 1.6.0.2.
Use the exact controller and update-path limits from the saved instructions.
The current M7X page links to a filename that names M3X, M5X, and M6X. This difference remains unresolved.
The P21, P21D, P221C, and P222C packages are PSU firmware.
They are separate from miner firmware.
See the [firmware catalogue](https://bixbit.io/en/firmwares) and [installation documentation](https://docs.bixbit.io/installing_and_updating_bixbit_firmware/whatsminer.html).

## LuxOS and D-Central data

The LuxOS collection includes versioned Python API clients, Commander installers, API documentation, and stock recovery files.
The proprietary LuxOS miner image was not obtained.
An API-client package version does not identify a firmware version.

D-Central datasets provide additional model, board, error-code, and maintenance leads.
They retain their source attribution and reuse terms.
Their compiled technical claims require primary-source confirmation before use in a validated device profile.
See the saved data-hub and methodology records in the catalogue.

## ePIC board revisions

The February 2026 guide gives separate records for UMC V4.1, V4.2, V4.3, and V4.5.
It also identifies the V7 module and the C4 and C10 carrier boards.
The V7 module needs a carrier board.
C10 supports the listed S21 Pro, S21 XP, and S21+ models.
It does not support the built-in display.
The catalogue preserves the release years from this retrospective table.
These are board release years. They are not miner release years.
[Official guide, pages 4 and 5](https://epicblockchain.io/wp-content/uploads/2026/03/ePIC-UMC-Complete-User-Guide-for-Antminer_last-updated-Februrary-2026.pdf)

Use the legacy S19 package only for the documented BM1398 S19/S19 Pro case.
The guide gives a separate update path for versions v0.56.3 and older.
The download page uses the broader label v0.x.x.
Keep this wording difference in an update-test profile.

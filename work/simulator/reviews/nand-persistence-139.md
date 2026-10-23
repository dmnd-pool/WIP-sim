# Native NAND persistence in the S9 reference environment

The three linked boots below passed their declared tests. This is a developer
review. The independent review and exact physical-board gates remain open.

The unchanged archived S9 firmware boots through SPL, U-Boot and Linux. Its
native `mtd`, `dd`, `cmp` and `sha256sum` tools write and read the private NAND.
The original SD image is read-only. Each run saves paired SD and NAND overlays
after an observed native restart and emulator exit code 0. The next run verifies the
parent result, process specification, actual snapshot records and restart bytes
before restoring both disks. A copied shutdown message alone is insufficient.

| Phase | Run | Native storage check | Independent mining |
| --- | --- | --- | --- |
| Write | [133](../qemu-s9/nand-boot-write-133/result.json) | Write 44 bytes to `miner_cfg`; read and compare | 36,771 computed hashes; 45 serial results; 2 fresh jobs; 2 Core blocks |
| Read and erase | [139](../qemu-s9/nand-boot-read-139/result.json) | Read the saved 44 bytes before erasure; erase; check 2048 erased bytes | 116,937 computed hashes; 112 serial results; 2 fresh jobs; 2 Core blocks |
| Read erased data | [139](../qemu-s9/nand-boot-erased-139/result.json) | Check 2048 erased bytes after another restart, before any NAND mutation in the test script | 109,025 computed hashes; 115 serial results; 2 fresh jobs; 2 Core blocks |

All six isolated Core nodes accepted the actual firmware-derived blocks,
changed from height 0 to 1, and retained the new tip after restart. Each boot
also passed an independent read-only SD filesystem check without journal replay.
The mining audits retain fresh jobs, clean-job transitions, difficulty changes,
actual hardware responses and reconstructed headers:
[write](../qemu-s9/nand-boot-write-133/regtest-audit-session-0.json),
[read/erase](../qemu-s9/nand-boot-read-139/regtest-audit-session-0.json),
[erased readback](../qemu-s9/nand-boot-erased-139/regtest-audit-session-0.json).

The tested backend is [130d](../qemu-s9/backend-recipe-130d/spec.json).
Its QEMU SHA-256 is
`8b287152de6a91085203129b590d864b3d026ea3074080d8c0c48103ca6b2e46`.
The original SD SHA-256 is
`97a257571af4fb9e62f709e2d6b8f36a191fdd07f1cd12ae780bd1c73af7eaef`.
The generated all-FF data/spare backing SHA-256 is
`fbee2e0f41769c042340dff3211f37f3933c097684bce976a69222767d10656b`.
It has 131072 pages, each with 2048 data bytes and 64 spare bytes.
This is generated reference media, not a physical factory dump.

The first boot uses source133 helpers; both later boots use source139 helpers.
Their complete process specifications retain every executable, library, script,
model, seed and budget. Each run has its own input identity. The actual parent
snapshot identities are in its `console.json` and `process-spec.json`.
Each keeps the explicit fan, temperature, power, console and low-rate filter
adaptations. The aggregate cap is 1000 H/s. These are adapted reference runs.

Earlier runs135 and137 failed snapshot export. Their native operations passed,
but those runs remain failed. Run137's independent mining auditor correctly
refused its failed execution result. No assertion was bypassed.

The old shared 32 MiB raw export bound was too small for this NAND layout.
Source139 uses a separate 64 MiB NAND bound and retains the 32 MiB SD bound.
The compressed export limit remains 24 MiB. Capture also compares every logical
byte after isolated qcow2 conversion and verifies that source and backing hashes
did not change. Final NAND exports grew from 19,922,944 to 39,649,280 and 57,933,824
bytes. Native UBI initialization writes headers across the NAND; the 64 KiB qcow2
allocation size amplifies small writes. This is a storage-format cost, not
evidence of a physical NAND timing defect.

Source141 passed the same complete sequence with compressed exports, 4 KiB
clusters and restore normalization. All three runs use the same frozen
[source141](../qemu-s9/core-checkpoint-141/manifest.json) and backend130d.

| Phase | Actual run | Independent mining | NAND raw export |
| --- | --- | --- | --- |
| Write | [write141](../qemu-s9/nand-cycle-write-141/result.json) | 66,330 hashes; 76 serial results; 69 submissions; 2 Core blocks | 155,648 bytes |
| Read and erase | [read141](../qemu-s9/nand-cycle-read-141/result.json) | 93,240 hashes; 65 serial results; 63 submissions; 2 Core blocks | 282,624 bytes |
| Read erased data | [erased141](../qemu-s9/nand-cycle-erased-141/result.json) | 112,008 hashes; 125 serial results; 122 submissions; 2 Core blocks | 405,504 bytes |

Every run has two fresh jobs, a clean-job transition, a difficulty change and
a clean read-only SD inspection. All six Core nodes accepted the actual blocks
from height 0 to 1 and retained them after restart. The audits are
[write](../qemu-s9/nand-cycle-write-141/regtest-audit-session-0.json),
[read/erase](../qemu-s9/nand-cycle-read-141/regtest-audit-session-0.json), and
[erased readback](../qemu-s9/nand-cycle-erased-141/regtest-audit-session-0.json).
Their separate Core records are in `../bitcoin-core/s9-nand-{write,read,erased}141-session0-{0,1}/`.

The [migration run](../qemu-s9/nand-boot-compact-141/result.json) also passed.
It restored the actual 57,933,824-byte source139 export. Normalization reduced
that file to 446,464 bytes. Isolated `qemu-img compare` checked every logical
byte, and both source and backing hashes remained unchanged. The subsequent
native erased-data readback, restart, SD inspection and two Core checks passed.
The final export was 565,248 bytes. Conversion is a recorded storage-layout
operation. It does not change the logical NAND contents.

Full-media writes, interrupted updates, power-loss recovery, bad-block fault
injection, exact physical geometry and wear calibration remain separate work.
The three successful boots do not close the complete NAND or release gates.

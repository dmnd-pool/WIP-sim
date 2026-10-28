# Verified source checkpoint511e

Status: S9 milestone2 remains incomplete. Continue S9 work. Stop only when
all milestone2 gates pass, then save the tested result. Other miners remain
checkpointed under the user's latest instruction.

Selected reconstructed S9 logic passes72 real AXI transactions and two resets
across three chain interfaces. The original CPU-absence notice is preserved.
See [the actual report](reviews/maxigp0-executed-511e.md) and
[independent audit](reviews/maxigp0-independent-execution-511e.json).
The failed511d result remains failed. No CPU, original-board, physical timing
or exact-target mining gate follows from the FPGA component test.

The current source passes formatting, strict workspace Clippy and1,171 tests
in125suites, with49 explicit ignored tests. All575 source inputs and source
set match before and after those checks. The compact
[source/check manifest](source-checkpoint-511e/manifest.json) and its four logs
are included in Git. The archive, firmware, executable payloads, VM disks,
cookies and keys remain excluded. Nine new include/exclude checks pass.
The README has the verified component commands and language reasons.

Earlier501–510 changes are preserved as tested component work. These include
strict UART configuration/reset tests, actual original Bitaxe trace retention,
reference computation samples and an independent ADC source-arithmetic oracle.
They do not count as S9 progress or as a completed miner. No further Bitaxe
implementation is authorized before S9 milestone2.

Continue from [the durable record](continuation-2026-10-28.md):

- 512: staged live Rust/Icarus/VPI interface and conservative CPU/PL timing.
- 513: staged exact FAT16 allocation/header analysis and original-ROM read oracle.
- Resolve exact S9 component identities and measured physical reference data.
- Complete original reset/boot, mining/Core, fault/update/persistence, native
  local/remote workflows and clean-install gates for the final exact identity.

The512/513 code remains staged and uncompiled. It is outside this verified
production source cohort. Original runtime inputs remain in the workspace
archive; transfer linked contents when moving the full execution environment.

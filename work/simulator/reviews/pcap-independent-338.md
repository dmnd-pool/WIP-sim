# Independent PCAP review 338

Date: 2026-10-23. Reviewer: `exact_target_gap`, separate from the author of
the reviewed PCAP changes. Scope: strict reference configuration semantics.
This is not a reconstruction of the FPGA or an exact physical target review.

The review found two correctness defects and one device-local contract gap. The saved successful runs do not
exercise these paths. They remain valid evidence for their stated paths.
The parent then assigned the reviewer to implement the fixes. New fixes need
a further independent review; the original review is not approval of those
later changes.

## Findings

### F1: Unsupported destinations bypass the strict contract (high)

`simulator/backends/qemu/s9-pcap.patch:129` only selects the strict transfer
function when the DMA destination is `0xffffffff`. Other destinations still
enter the upstream transfer loop. In
`work/simulator/qemu-s9/pcap-derived-320/hw/dma/xlnx-zynq-devcfg.c:165`, that
loop cannot finish a non-loopback transfer with source length zero and
destination length one. Each iteration selects zero bytes. The destination
length never changes. The queue entry never leaves the loop.

The qtest helper at
`crates/md-sim-firmware/examples/support/pcap_qtest.rs:89` always used the
configuration sentinel. Its existing positive and fault tests missed this
path. The new bounded test reproduced the problem on frozen backend320:
[`pcap-unsupported-old-339/assessment.json`](../qemu-s9/pcap-unsupported-old-339/assessment.json).
The result is `timed-out`, with complete retained traces and no pass. The last
unanswered transaction is `writel 0xf8007024 0x1`, after source length zero
and destination `0x500000`.

Required fix: route every DMA request through the strict contract when it is
enabled. Reject unsupported destinations before reading guest memory. Keep
the existing sticky failure and programming-reset policy. Leave the generic
upstream path unchanged when the strict model is absent. Test rejection,
failed retry after changing the destination, and recovery after reset.

### F2: Reset does not restore a disabled register region (medium)

The inherited bad-UNLOCK path disables the whole register region at derived
C line 274. The reset function at derived C line 148 resets the Rust port and
registers but never re-enables that region. The reset-time UNLOCK callback
returns immediately at line 261. Thus a system reset does not restore the
fresh realized reference state after a bad key. The existing reset tests
never submit a bad key.

Required check: bad key, inaccessible read/write, system reset, cold register
readback, then a complete checked configuration. A strict-reference fix may
restore the realized register region on system reset. Do not infer physical
power-on versus warm-reset security behavior from that test. The available
[AMD JTAG control reference](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/JTAG-Enable/Disable-Control)
specifically describes a power-on reset requirement for the debug controller
after an incorrect key. It is not a measured S9 reset trace.

### F3: Device-local migration rejection has no incoming hook (low)

`simulator/backends/qemu/s9-pcap.patch:142` installs only `pre_save`.
The device keeps the upstream migration name and version, and its field list
still loads the register array. The Rust port state is not serialized.
If this device is used in a board that permits migration, incoming generic
device state can load completion bits without the corresponding strict-port
state. This is not a demonstrated bypass of the current S9 board policy.
Its SWDT and other devices are marked `unmigratable`, and the pinned loader
rejects incoming state on that basis before loading device sections
(`migration/savevm.c:3133`). The shared DMA auditor also requires actual
transfer evidence. The missing hook is a device-local contract gap.

The pinned QEMU loader calls `vmstate_load_vmsd` on incoming device state
(`migration/savevm.c:1015` and `:2708`). That path calls `pre_load` before
loading fields (`migration/vmstate.c:350`). It does not call `pre_save`.
Required fix: reject incoming device state before field loading whenever the
strict contract is enabled. Retain generic upstream migration behavior.
Verify the hook in a compiled device-only test without removing the S9 board
guards. A malformed stream or an unrelated unmigratable-device failure does
not prove that the strict PCAP hook ran.

## Checks that held

- `crates/md-sim-firmware/src/bitstream/port.rs:94` checks the complete DMA
  chunk before committing its words. Its bounds follow the retained prefix.
  `reject` preserves that prefix, and `begin` cannot clear the failure latch.
- `crates/md-sim-qemu-core/src/pcap.rs:78` preserves the Rust result and
  exposes a null-terminated digest. The C/Rust progress structures agree.
  The documented pointer preconditions match the reviewed C callers.
- `simulator/backends/qemu/s9-pcap.patch:87` performs a new 32-bit read for
  each fixed-source word. It does not cache the first word. The saved test
  proves RAM-source behavior. It does not test peripheral read side effects.
- `crates/md-sim-firmware/examples/support/pcap_audit.rs:19` requires reset,
  enable, ordered byte counts and received-word hashes. It rejects premature
  completion and rejected transfers. It records legacy traces without a
  source-mode field separately.

No new build or original firmware run was used for the initial review.
The follow-up implementation uses new numbered output directories.

## Evidence inspected

The saved fixed-source and control-recovery tests each report eight actual
interrupt edges and two QMP resets. They pass 146 and 136 transactions.
The configuration audit for firmware321 records all 2,083,740 payload bytes
and digest `49d0faa400fbc8bed3f3e651da2c6da5f3184a08f942ec95db59c2526dcbea78`.
The separate mining audit records 1,642 computed hashes, two jobs, a clean-job
transition, a difficulty change, and 22 verified serial results. The two Core
reports record acceptance and restart retention at height one. Cycle322
records all four boots and eight different accepted blocks. It explicitly
leaves the original-board and full native-dashboard gates incomplete.

These are inspected report contents. This review did not replay the firmware
or rehash every large execution trace.

| Inspected report | SHA-256 |
| --- | --- |
| `../qemu-s9/native-pcap-321/configuration-audit.json` | `19dcaca1b03553854e2439fba722659f94ff9774520a86cd09c71338f3177a44` |
| `../qemu-s9/native-pcap-321/regtest-audit322.json` | `dfab5878ecfad9608ff8bca70aa2e70fbcfb159a301e99896701bfd37c6e74db` |
| `../qemu-s9/pcap-dashboard-cycle-322/assessment.json` | `5755827f25620f99b5aa9f286ad8f2cb03c87b6208eaf01b69998b2b20b86a90` |
| `../bitcoin-core/pcap321-candidate0-323/block-acceptance.json` | `b7635cb9df694c4328b13f5893fae88681957a29f554e940f9a2e60e0653d4c2` |
| `../bitcoin-core/pcap321-candidate1-323/block-acceptance.json` | `683ffee64821bd25e13e4560243869acfce57aafe71bf544541af61811c446b6` |

## Reviewed source identities, before follow-up fixes

| Source | SHA-256 |
| --- | --- |
| `crates/md-sim-firmware/src/bitstream/port.rs` | `117cb26f0bd4d80f3005192732017c426201d5cdbbd28e609bc88bd50c2c8a4f` |
| `crates/md-sim-qemu-core/src/pcap.rs` | `02536787ffe14c9858d1742ac786385757f870831e41fe351efa408155ea946c` |
| `simulator/backends/qemu/s9-pcap.patch` | `02564e0a8d707c8c4de553a620ab8f639ec6dfdb7e34e9b1147317227f9031e9` |
| `simulator/backends/qemu/md-sim-pcap.h` | `0d83c1452ef6682f46342e86ea4008d1a0e701fb68360376d5d035b17449d9bf` |
| `crates/md-sim-firmware/examples/support/pcap_audit.rs` | `8f5e33e01cb77b2006f5c38833e417f8a99932b33f772d580ae52889f69ef39c` |
| `crates/md-sim-firmware/examples/support/pcap_qtest.rs` | `eece3823d34eab6c125cb4a9e17bca402a8cfef8ac2148f6490f880630524faf` |
| `work/simulator/qemu-s9/pcap-derived-320/hw/dma/xlnx-zynq-devcfg.c` | `d8d3b29c27dec11c0409af89e7968e0a60f7089752ecd4674a04fce412d2cb42` |

## Follow-up checkpoint 345

The subsequent implementation uses derived sources 339, 341 and 343. Original
archives and prior outputs remain unchanged. Sanitized backend343 has SHA-256
`be5b22536955634b5c0e9609d000d0e0c982c5d1707dc39b3cc8b1b564c49274`.
Its build has no compiler warning or error diagnostic. The following compiled
tests pass on this backend:

- `pcap-unsupported-344`: 151 transactions; both unsupported destination
  forms fail, retries remain failed, and explicit reset permits completion.
- `pcap-unlock-344`: 131 transactions and three actual system resets;
  inaccessible state clears only after reset.
- `pcap-normal-345`, `pcap-fixed-345`, `pcap-control-345`: the prior 118,
  146 and 136 transaction suites pass.

The separate [source review](pcap-fixes-345-subreview.md) found no actionable
issue in these C changes or the two added fault cases. It does not independently
review its author's migration test module.

Migration test work remains open at this checkpoint. The first fixture lacked
the CPU address-space object required by QEMU qtest. UBSan caught that fault
in `pcap-migration-342`; the fixture now has one Cortex-A9 under the qtest
accelerator. Test344 then found that its generic status oracle omitted the
DMA completion flags created by upstream reset callbacks. That oracle was
corrected from the pinned source, without changing the generic model.
Test345 reaches actual generic state save, but the selected device-load QMP
path fails in upstream migration cleanup before strict loading. This is a
failed test, not evidence that the new guard passed. The next action is to
use the correct incoming migration lifecycle, with a successful generic
reload as the required positive control.

The local checks passed five configuration-port tests, two driver confinement
tests, affected-file formatting, and strict Clippy. A workspace format check
found changes in other agents' active files; this reviewer did not change
those files. Finished build-input cleanup journals 341, 343 and 345 each
record 698,349,720 reclaimed bytes and retained restore identities. No run
evidence or original payload was removed.

## Follow-up checkpoint 347

All six compiled suites now pass on the same final backend343 and frozen test
driver. The successful migration test uses standard QEMU file migration.
It starts separate generic sender/receiver and strict sender/receiver
processes. The generic receiver imports both SW_ID and interrupt-status
markers. Strict outgoing and incoming operations fail at the exact PCAP hooks.
The strict registers stay unchanged. All four QEMU processes exit cleanly.
The complete 12,067-byte migration stream has SHA-256
`4ce31a6dd4847f80d3cb0a71792c8513e8afd9225db4a04ce0c7caeba97ea580`.
No malformed-stream failure is counted as success.

| Suite | Report | Actual transactions |
| --- | --- | --- |
| Ordinary transfer/reset | `../qemu-s9/pcap-normal-347/assessment.json` | 118 |
| Fixed source | `../qemu-s9/pcap-fixed-347/assessment.json` | 146 |
| Control rejection/recovery | `../qemu-s9/pcap-control-347/assessment.json` | 136 |
| Unsupported destination | `../qemu-s9/pcap-unsupported-347/assessment.json` | 151 |
| Invalid UNLOCK/reset | `../qemu-s9/pcap-unlock-347/assessment.json` | 131 |
| Device migration | `../qemu-s9/pcap-migration-346/assessment.json` | 17 |

The driver SHA-256 is
`e19f287b7567e338723af3380cc99bfa3c050f1426e821f0f8380419da4610b7`.
The frozen helper and directly reviewed sources are in
[`pcap-target-347`](../pcap-target-347/manifest.json). Example rerun command:

```sh
work/simulator/pcap-target-347/release/examples/qemu_ttc_reset \
  work/simulator/qemu-s9/backend-build-343-ubsan \
  work/simulator/qemu-s9/pcap-migration-346 pcap-migration
```

That output path already exists. Select a new path for a rerun. The command
was executed with this exact helper before its byte-identical frozen copy
was made. The failed345 test source was recovered from its retained probe's
embedded source bytes. Its hash matches the recorded source hash; see
[`source-recovery.json`](../qemu-s9/pcap-migration-345/source-recovery.json).

These checks resolve the three review findings within the stated reference
contract. An independent review of the new migration test module is still
pending at this checkpoint. Fresh archived firmware execution on backend343
is running separately as `native-pcap-349`; it is not counted as passed here.

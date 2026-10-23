# Shared pool to Bitcoin Core handoff

Date: 2026-10-23. Author: `exact_target_gap`.

The Core checker now has an explicit shared-pool mode. This change prepares
the handoff from the selected shared execution. It is not evidence that
execution 391 passed or that Core accepted either candidate. Those commands
remain pending until the actual shared execution completes.

## What changed

`node_prerequisite_probe DAEMON NEW_OUTPUT shared REGTEST_AUDIT_JSON INDEX`
requires the shared-pool provenance field and invokes the adjacent frozen
`s9_asic_audit` with `regtest-session-shared`. The checker compares the full
fresh replay result with the selected audit before it imports a block.
An ordinary audit cannot enter shared mode. A shared audit cannot enter the
default mode. Existing source-fixture, original internal-pool, and offline
commands keep their previous selection rules.

The node record retains the exact shared bundle, receipt, support proof,
provider receipt, pool logs, and manifest. The bundle includes the original
session bytes. The fresh audit checks native worker receipts, carrier
identity, captured support inputs, successful pool outcome, successful outer
execution, and actual firmware hardware/submission correlation. The node
link preserves those exact bindings. It checks the outer result and process
hashes again. The large outer result stays at its recorded source path.
That source directory remains necessary for a complete replay.

Both shared and native evidence copies use one open regular source file.
They reject symbolic links, check the exact expected size, create a new
destination, copy at most the expected size plus one byte, and verify the
final size and digest. A changed or growing source cannot cause an
unbounded copy. A failed new output directory remains failure evidence.

Core validation is unchanged. The daemon must match the repository's
source-build registry for commit
`619334569b44b4278d0155bb8daf4c8535a47367`. It executes in private namespaces
with no external network. Each candidate uses separate run-owned storage.
Three sequential Core processes establish genesis, import the actual
candidate, and restart without the import argument. The checker requires
the exact candidate tip at height one and a clean stop in each phase.
It does not ask Core to generate a block.

## Checks completed

Run from the repository root:

```sh
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=work/simulator/check-target-085 TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 cargo test -p md-sim-firmware --example node_prerequisite_probe --offline --locked
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=work/simulator/check-target-085 TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 cargo clippy -p md-sim-firmware --example node_prerequisite_probe --offline --locked -- -D warnings
```

Ten tests pass. Strict Clippy passes. Tests include explicit mode selection,
changed pool and outer identities, bounded copy, symbolic-link rejection,
existing-output protection, and the previous three-phase Core log oracle.
These tests do not invent a shared firmware or node result.

| Input or result | SHA-256 |
| --- | --- |
| `node_prerequisite_probe.rs` | `b5e3d92cdc56f75d4ad394b6fd6ccd0b1de1589e34b093f9b2f2ee4224cf0780` |
| [Tests390](../checks-2026-10-21-001/shared-node390-tests.log) | `bfdebe09c1208c32a452ebd08d73aab3b7afa85c1c345442acc4e8d3df78819b` |
| [Clippy390](../checks-2026-10-21-001/shared-node390-clippy.log) | `227f519d02712774f38aad2258082acf27d69287a09db8ad3f74b1b905016c1a` |

Earlier compile387 and Clippy388 failures remain in the checks directory.
They exposed missing borrows in tests and unnecessary borrows in the new
copy path. Both were fixed before the successful checks above. Independent
review also found and prompted the bounded-copy fix.

## Exact selected recipe, not yet executed

The parent selected frozen helpers in `channel-target-390`, preparation
`native-shared-prepared-390`, and execution `native-worker-firmware-391`.
First finish and retain that exact execution. Do not run these commands
against another run as a substitute. The audit report must be in the native
execution directory because the checker derives its source directory from
that report's parent. Every output below must be new.

```sh
work/simulator/channel-target-390/release/examples/s9_asic_audit rates-shared work/simulator/network-vm/native-worker-firmware-391/execution work/simulator/network-vm/native-worker-firmware-391/execution/rate-audit392.json
work/simulator/channel-target-390/release/examples/s9_asic_audit regtest-shared work/simulator/network-vm/native-worker-firmware-391/execution work/simulator/network-vm/native-worker-firmware-391/execution/regtest-audit392.json
work/simulator/channel-target-390/release/examples/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/shared391-candidate0-392 shared work/simulator/network-vm/native-worker-firmware-391/execution/regtest-audit392.json 0
work/simulator/channel-target-390/release/examples/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/shared391-candidate1-392 shared work/simulator/network-vm/native-worker-firmware-391/execution/regtest-audit392.json 1
```

The independent audit must retain two candidates from distinct actual jobs.
Each Core output must have `accepted: true`, heights zero then one then one,
and `persisted_after_restart: true` in `block-acceptance.json`. Preserve
`original-submission-replay.json`, `source-run-link.json`, the block record,
candidate bytes, Core result, and Core logs. Record final helper and input
hashes in the execution report. A failed audit or Core process leaves that
gate open. No previous historical run result can replace it.

This path uses original archived firmware in the declared adapted S9
reference environment. It grants no exact physical board, calibration,
native-dashboard, external-host, or full-matrix gate by itself.

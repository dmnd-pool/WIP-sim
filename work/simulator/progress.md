# Simulator implementation checkpoint

Status: **incomplete**. Started 19 October 2026. Current checkpoint: v11.

Current priority, confirmed28 October: complete exact S9 milestone2 first.
Other miner development is checkpointed. Stop after the complete S9 milestone2 gates
pass and save a tested checkpoint. Read the current continuation and
[milestone2 acceptance](milestones.md).

23 October: [milestone 1.2 passed](milestone-1.2-handoff.md). The user committed
the checkpoint as `b276dd7a` and resumed work on 26 October. Read the
[current continuation](continuation-2026-10-26.md) before the older records.
Milestone 2 and the full matrix remain incomplete.

The user resumed implementation on28 October2026. Read the
[current continuation](continuation-2026-10-28.md). The preceding
[checkpoint500](checkpoint-500.md) was committed as`f5fbf74e` after the complete
CLI test passed. It records the tested commands, independent mining/Core/
browser evidence and the501/502 resume inputs. Its historical pause is over.

Latest28 October S9 result: the selected reconstructed FPGA logic passes72
real MAXIGP0 transactions and two resets through the live Rust/VPI link.
Independent review passes for the live run and a separate baseline with the
same frozen helper. Clock, four-state signal and event-order tests pass.
See the [execution report](reviews/live-pl-executed-512e.md). This does not
execute the CPU. The CPU/FPGA bridge and original SD reset path are next.
The unchanged public ROM's file-read routine also passes28 isolated cases;
its exact S9 silicon binding remains unknown. See the
[ROM routine report](reviews/zynq-rom-read-path-executed-513b.md).
The later original-ROM reset diagnostic reaches actual device writes. It
stops at missing SLCR register0x254 behavior. The
[independent audit](reviews/zynq-rom-reset-independent-execution-513g.json)
matches180 raw translated spans to the archived ROM. Full board boot remains
open. The shared transport's actual bus regression also passes; its
[throughput test](reviews/hdl-throughput-executed-514.md) fails at100us after
the unchanged10s deadline. A faster suitable HDL backend is implementation work.
Exact PSU/controller/hashboard identities and physical traces remain open.
At commit `e6fb96cf`, workspace checks passed1,171 tests in125 suites, with49
explicit ignored tests; formatting and strict Clippy passed. All575 source
inputs matched before and after those checks. Later changes need their own
full workspace checks. Other miner development stays checkpointed under the
user's S9-first instruction. See the [source checkpoint511e](checkpoint-511e.md).
The [later full workspace checks](reviews/workspace-check-after513g.json)
pass1,230 tests in128 suites, with55 explicit ignored tests. All595 source
inputs match before and after formatting, strict Clippy and tests. The new
CVC tool builder also passes its actual isolated build and smoke tests.
The [subsequent full checks](reviews/workspace-check-after516.json) pass1,237
tests in129 suites, with55 ignored tests and596 unchanged source inputs.
The faster backend still needs the selected-model and callback-order tests.

The earlier [adapted S9 run495 passes](reports/native-browser-mining-pass-495.md)
the selected SSH/TLS native browser checks, fresh mining, independent header
verification and two Core block acceptance/restart checks. The topology uses
one execution host with an isolated worker and KVM client VM. Original-board
and exact-target gates remain open. The PLL arithmetic oracle496 and FPGA
netlist conversion497 also pass their limited component checks. CLI500 then
passes the complete shared demonstration. Icarus tool502f passes, while its
selected HDL validation remains open. Read the current continuation for exact
source and test identities.

Implementation has continued after v11. The
[21 October record](continuation-2026-10-21.md) and
[20 October record](continuation-2026-10-20.md) retain the earlier experiments.
Use the 26 October continuation above for current work and next actions.
The [BM1397 component record](bitaxe/bm1397-progress.md) covers the new Rust
packet decoder and independent job-hash regressions. No exact physical target
has passed all required gates.

On 26 October, the user resumed all milestone work and said to continue.
The earlier milestone pause is no longer active. See
[the milestone record](milestones.md).
The adapted S9 start command passes its fresh demonstration in demo284. Its
four boots produce eight different Core-accepted blocks, all retained after
node restart. Native browser, signed update, settings restoration and restart
persistence also pass. See [the milestone report](reports/adapted-s9-milestone-284.md).
Demo280 failed its WebSocket sample deadline and remains a failed result.
The full matrix and exact FPGA work remain required. See the
[current author review](reviews/adapted-s9-milestone-283.md).

The user approved milestone 2, which is still in progress. The
[frame mapping report](reports/series7-frame-mapping-291.json) records both
archived S9 bitstreams, full address and padding checks, and every frame error
code. The selected FPGA logic and exact board identities remain unresolved.
The [ASIC clock separation](reports/asic-clock-295.json) passes compiled tests
with undefined-behavior checks. The archived firmware also produces two new
Core-accepted blocks at a 10 H/s cap. Both survive node restart. This remains
an adapted reference run. Follow the continuation record for active work.
Do not stop at a component pass.

The [checked FPGA configuration port](../../simulator/backends/qemu/PCAP.md)
also passes compiled reset/error tests and original firmware run302. Original
U-Boot sends the complete selected payload through actual DMA memory reads.
The run computes 1,325 hashes at 10 H/s and produces two Core-accepted blocks,
both retained after restart. The separate local IPC worker regression304 also
passes. It checks 1,652 computed hashes and two Core-accepted blocks, retained
after restart. Destination hashes, result retrieval and run cleanup pass.
See the [combined report](reports/checked-configuration-305.json) and
[author review](reviews/checked-configuration-305.md). The
[hardened-checker replay306](reports/configuration-checker-replay-308.json)
passes 1,316 independently checked hashes and two more Core acceptance and
restart checks. Cycle307 tests native dashboard and signed-update behavior
with strict configuration checks across all four boots. It passed, with eight
distinct Core-accepted blocks retained after restart. See the
[cycle report](reports/checked-configuration-dashboard-307.json). It uses the
frozen backend304 and does not replace tests of the later fixes.
These are not SSH or remote TLS dashboard passes.
This does not reconstruct the selected FPGA logic or resolve an exact board.

The expanded interrupt test309 found that backend304 left its interrupt
asserted after programming reset cleared completion status. The failure is
retained. The [reset review](reports/configuration-reset-review-316.json)
records three fixes: refresh the interrupt output after reset, suppress DMA
submission during register initialization, and reject unimplemented
fixed-source DMA. Backend315 passes both compiled 118-transaction tests,
including eight actual interrupt edges, with undefined-behavior checks.
Original firmware run314 on backend312 passes 1,661 independent hash checks
and two Core acceptance/restart checks. Backend315 firmware run316 and local
IPC worker318 also pass. Their independent audits check 1,310 and 1,318 hashes.
Each run has two Core acceptance/restart checks. Destination hashes, exact
result retrieval and worker cleanup pass. See
[their evidence report](reports/configuration-firmware-worker-319.json).
Full native cycle317 passes on release316. Its four boots, signed installation,
native browser, settings restoration and restart persistence pass. All eight
different blocks pass Core acceptance and restart checks. See its
[canonical assessment](qemu-s9/pcap-dashboard-cycle-317/assessment.json).

Further model work now implements fixed 32-bit source reads. The old guard
fails the new positive test, as expected. Review also found that rejected DMA
controls did not latch failure. Compiled test320 reproduces that defect on
backend315. The fix preserves the received prefix and requires programming
reset before more data. Sanitized backend320 and release321 pass the ordinary,
fixed-source and control-recovery compiled tests, with 118, 146 and 136
transactions respectively. All 612 Rust tests, formatting, strict Clippy and
release builds pass at checkpoint320. Original firmware run321 on backend320
passes 1,642 independent hash checks and two Core acceptance/restart checks.
See the [author review and evidence](reviews/pcap-fixed-source-and-recovery-321.md).
Full native cycle322 passes on release321, with four boots, the native browser,
signed update, settings restoration, persistence and eight different blocks
accepted by Core and retained after restart. See its
[assessment](qemu-s9/pcap-dashboard-cycle-322/assessment.json). These results do not close the
exact-board or remote dashboard gates. Follow the continuation record.

The new support topology runs a native worker beside an isolated client VM.
Transfer329 passes fresh 1 MiB upload, destination verification and download.
Process test330 found a denied nested user mapping. The explicit non-root
support identity fixes it; process test332 passes actual isolated execution,
verified result retrieval and cleanup. The original root-guest confinement
rule remains. This topology has no SSH or native-dashboard pass yet.

Firmware345 now passes archived S9 execution on this layout. Its independent
audit checks 1,661 actual hashes, two fresh jobs and 11 matched ASIC results.
Both selected blocks pass Core acceptance and restart checks350. These use the
adapted backend320 identity. The newer outer-host receipt path passes component
tests349,351 and frozen-helper repeat355; wrong-receipt353 refuses cleanup.
It saves exact result bytes outside the client VM before acknowledging cleanup.
Active-job reconnect and outer-worker loss remain separate open gates.
The [independent review](reviews/native-worker-independent-338.md) and
[parent review of the agent's fixes](reviews/coordinator-retention-parent-355.json)
record the boundaries. Checkpoint354 passes all659 Rust tests in105 binaries,
strict workspace Clippy and release helper builds. Its full source is retained
in [the checkpoint manifest](qemu-s9/core-checkpoint-354/manifest.json).
The fresh retained firmware354 also passes. Its independent audit checks1,322
hashes and14 matched ASIC results across two jobs. Both selected blocks pass
Core acceptance and restart checks357. The exact103,542,771-byte result is
retained outside the VM before cleanup. See the
[combined evidence](reports/native-worker-firmware-358.json).
This remains an adapted backend320 run. The subsequent typed-error transport
and SSH broker changes need their own affected checks.

Cleanup removed 12.2 GiB of inactive debug caches and 2.6 GiB of verified duplicate
build inputs. Original archives, frozen helpers and run evidence remain.
See the [cache record](reports/build-cache-cleanup-289.json) and
[restoration journal](reports/duplicate-build-input-cleanup-289.json).
New debug builds omit symbols and incremental output to control disk use.
Two more passes removed 2.6 GiB of verified duplicate inputs from finished
backend builds. See [journal299](reports/duplicate-build-input-cleanup-299.json)
and [journal302](reports/duplicate-build-input-cleanup-302.json).
Another 698 MB was reclaimed after release build304. See
[journal305](reports/duplicate-build-input-cleanup-305.json).
Cleanup318 reclaimed another 4.0 GiB of verified duplicates from completed
builds and cycle307. Restoration paths and hashes are in
[the build journal](reports/duplicate-build-input-cleanup-318.json),
[the cycle journal](reports/duplicate-cycle-input-cleanup-318.json), and
[the Core journal](reports/duplicate-core-input-cleanup-318.json).
Later cleanup reclaimed another 4.0 GiB from completed builds and cycle317.
The restoration paths and hashes are retained in journals
[321](reports/duplicate-build-input-cleanup-321.json),
[323 build](reports/duplicate-build-input-cleanup-323.json),
[323 cycle](reports/duplicate-cycle-input-cleanup-323.json), and
[325 Core](reports/duplicate-core-input-cleanup-325.json).

The [retained clean build check](reports/clean-build-210.json) passes535 Rust
tests in96 binaries. All58 release executables match the frozen209 build byte
for byte. Its [source manifest](reports/source-210.json) pins3611 source and
vendor inputs. This used an empty Cargo home and a new build directory.
It does not establish a clean firmware package installation or target pass.

The required specification is `docs/simulator-specification.md`, version 0.3.
Original archived S9 firmware now produces two Core-accepted regtest blocks
in an explicitly adapted reference environment. See
[run 030](qemu-s9/sd2022-private-pool-030/regtest-audit-001.json) and
[the node result](bitcoin-core/original-s9-030-candidate-0/block-acceptance.json).
This does not resolve its exact physical board or close the original-board gate.
All completion conditions in the implementation request remain required.
No complete exact physical target passes. Do not report this checkpoint as the
requested complete simulator. Original-board, per-target, native-dashboard,
remote-network and full-matrix gates remain open.

## Working execution path

Actual archived cgminer runs its own Stratum logic and S2 controller driver.
The Rust controller receives work packets, searches nonces under the selected
aggregate cap, and returns computed results through that interface.
A separate hash implementation reconstructs the complete headers.
Unchanged archived Bitcoin Core accepts the resulting regtest blocks.

The source artifact is `open-ckolivas-cgminer-source`, commit
`b8491c66e7e22f23a9edf095dd1337ee581e88bd`, archive SHA-256
`4b3515ec072904e81e91182f5f8d5f8b5b87361ef2a7e2131643d8207badb996`.
The fixture has eight chains with 32 chips each, from driver defaults.
This is not a resolved physical S2 board. Downstream FPGA and per-ASIC buses,
power, cooling, storage and fault behavior remain incomplete.
The original archive is unchanged. Source adaptations are separate and hashed.

The five current runs use the canonical workspace release binaries.
Each has two accepted submissions, a clean-job transition and a difficulty
change. All serial/header traces pass independent replay. There are no CRC
errors or truncated traces. The first four cases use fresh entropy. The fifth
is a fixed-seed regression, not fresh-work acceptance evidence.

| Run under `firmware/` | Cap H/s | Actual hashes | Returned ASIC results | Scope |
| --- | ---: | ---: | ---: | --- |
| `v11-default` | 10,000 | 6,005 | 2 | Fresh random-parent work |
| `v11-low-cap` | 1,000 | 171 | 3 | Fresh random-parent work |
| `v11-regtest` | 10,000 | 10,073 | 3 | Two fresh Core candidates |
| `v11-regtest-ubsan` | 10,000 | 14,071 | 2 | Two fresh Core candidates under UBSAN |
| `v11-zero-nonce` | 100 | 6 | 3 | Fixed nonce-zero regression under UBSAN |

ASIC filters and pool targets differ. Traces include real ASIC-valid results
that do not meet the pool target. The executing firmware withholds those results.
Hashes, counters, nonce results, guest logs and node acceptance are not synthetic.

## Frozen preparation and Core evidence

The cgminer toolchain is now captured and replayed from a closed input tree.
Snapshot `firmware/toolchain-snapshot-003/manifest.json` has SHA-256
`e2eabd7bd16c22397c37cfbcfd0115223406f7850b5ad8f0733de6ac79e4d12a`.
It contains 102 packages, 30,840 entries and 697,927,858 file bytes.
Capture uses the Arch Linux installed package database. Replay uses only the
saved tree and checks its files, links and modes before and after the build.

Both `prepared-closed-002` and `prepared-closed-ubsan-002` pass the exact C
helper regression and produce the same firmware bytes as the prior v10 builds.
The retained recipe runner hash is
`3a44a1066bc68239767e87d39fd3ddd6ba7e96b39965505a0f15d1cff20697d1`.
See `firmware/toolchain-closure-report.json` for input identities and the
explicit distinction between the earlier adaptation baseline and these closed
builds. This closes the installed-/usr gap for this exact cgminer recipe only.
Core preparation and other targets retain their own toolchain limits.

Bitcoin Core was rebuilt from unchanged archived commit
`619334569b44b4278d0155bb8daf4c8535a47367`. Its source archive hash is
`776f4a708eb1a461872a6f1dba28218f0ed09af04239c29115412eecd3de1bc8`.
Two isolated builds produced the same daemon hash:
`a22238f0a1ea0b0a67a17032d2df0039e9d3ac946f2e86e0634ea47448692fae`.
The corrected runnable build recipe is
`bitcoin-core/source-build-003/recipe.txt`. Build 002's quoted transcript defect
is retained as historical evidence.

All four v11 candidates passed the real node. Each case uses a fresh private
chainstate. Three unchanged Core processes initialize genesis, import the actual
firmware block, then restart without an import argument. The third process
loads the exact saved tip at height one. Each phase shuts down cleanly.
This tests process restart persistence, not host-crash durability.

Acceptance reports:
- `bitcoin-core/v11-regtest-candidate-0/block-acceptance.json`.
- `bitcoin-core/v11-regtest-candidate-1/block-acceptance.json`.
- `bitcoin-core/v11-regtest-ubsan-candidate-0/block-acceptance.json`.
- `bitcoin-core/v11-regtest-ubsan-candidate-1/block-acceptance.json`.

Reports link the original firmware submission, header, block, independent
verification, exact node inputs and raw logs. No node-generated block is used.
These are source-fixture tests. Every exact mining target still needs its own
fresh-work, Core and persistence tests.

## Original archived firmware diagnostic

The untouched S9 kernel and root image now execute original init, preinit,
procd, ubus and module loading on generic QEMU. See
`original-zynq/attempt-008/assessment.json` and
`original-board-progress.md`. The guest log has SHA-256
`57a5a4ea659fa37979559d96407abccaed99e53e6e3ad9dc909735faeca02f30`.

This diagnostic uses direct kernel entry, one CPU, explicit root/console
arguments, and a recorded bypass of `xiic_i2c_driver_init`.
The absent FPGA I²C interface caused an unbounded receive-FIFO wait.
The generic two-CPU loader fails with overlapping ROM regions.
Missing NAND/SD storage and board interfaces prevent a complete boot and miner
test. No native HTTP service, watchdog health, share or board gate is claimed.
Exit 137 is consistent with the configured CPU limit; it is not proof of cause.
All failed probes remain retained.

## Runtime and worker results

The Rust runtime has hashed input storage, isolated process execution, live
logs, cancellation, persistent results, chunked retrieval and acknowledged
cleanup. Tests use local inherited IPC. They are not SSH or external-host tests.

New v11 checks cover:
- Aggregate logical input bytes and object count, including partial transfers,
  repeated begin, retry, abort, restart and interrupted publication.
- Exclusive store writers, concurrent appends, persisted policy, atomic policy
  initialization and preservation of external files.
- Active-run evidence reservations and limits on retained run bytes and counts.
- Rejection before execution when a run exceeds the worker policy.
- Abrupt supervisor loss terminating the real isolated guest namespace.
- Explicit `lost` results after worker interruption, with exact partial logs
  and temporary JSON fragments retained for retrieval and acknowledged cleanup.
- Recovery between result and state commits, without changing result identity.

The default store policy is 8 GiB per object, 16 GiB total contents and 4,096
objects. Worker defaults are two active runs, 1,024 retained runs and 4 GiB of
retained evidence. Metadata uses additional space. These are not filesystem
block quotas. Guest CPU and RAM limits remain per process.

Independent review findings about initial policy writes, recovery after the
result commit and reused-PID temporary files were fixed and rerun.
See `store-progress.md`, `review.md` and
`reports/runtime-budget-tests-005.log`.

## Strict catalogue accounting

Frozen snapshot:
`bb994753c915ae63a63b41c568eed1273ac5db89cddd0970912dd3f653559cf5`.

Plan `plan-v11.json`:
`392efc9d21f0cbf4adcfb97aab95fd24b4ace16641be053390bd0a174bbdcc2f`.

The relevant-pair plan has 79,599 rows: 4,354 entities, 13,506 artifacts and
61,739 candidate relationships. Dispositions are 1,377 blocked, 71,437 unknown
and 6,785 not applicable. There are **zero executed exact-target rows and zero
passing exact-target rows**. Cartesian accounting separately adds 2,798,806
implicit unknown pairs. These are inventory totals, not firmware test coverage.

`matrix run --require-complete` returned exit code 3 with the v11 binary.
A forged empty plan with its own valid hash was rejected with exit code 2.
See `matrix-v11/result.json` and `reports/cli-v11.json`.
The current matrix command does not dispatch firmware suites.

## Current validation and source identity

The following current checks pass:
- Canonical workspace release build, offline and locked.
- Empty Cargo home and separate build directory; all three release executables
  match the normal workspace build byte for byte.
- Rust formatting and strict Clippy over all targets.
- 64 Rust tests: catalogue 15, firmware 7, freezer 4, Core oracle 2, mining 15,
  serial replay 1 and runtime 20. No failures or ignored tests.
- Five independent serial/header replays, two closed C rebuilds and C helper
  regressions, full UBSAN cases, and four real Core imports and restarts.

Logs are `reports/build-v11.log`, `clean-build-v11.log`,
`format-v11.log`, `lint-v11.log`, `tests-v11.log`, and
`v11-independent-replay.json`.
The unchanged Bitcoin header fixture passed in
`reports/bitcoin-fixture-release.log`.
The final guide check passes 309 links. Its log is
`reports/guide-links-v11.log`. All 3,246 source inputs are frozen in
`reports/source-v11.json`, identity
`206c316107d0b9825bc767340afe7dd87a32191c7874260bef337bd5d2cdb7bd`.
The independent verify command passes in `reports/source-verify-v11.json`.
`reports/checkpoint-v11.json` records commands and all ten completion gates.
The four Core evidence sets have a separate independent review in
`reports/v11-core-independent-review.json`. No scoped correctness finding remains.

Canonical release hashes:
- `md-sim`: `d96f30d54d099deb232526919e4d19662243675a3fb80fa96b2e4667b5442bfb`.
- `cgminer-lab`: `b3ab1091a3b8d2057c3ddb386e92e9a6ba6909d765ba02c2f17f76f08b847392`.
- `cgminer-fresh-work`: `90a5a39eb663994a1473385d1ccaad45a123b9c637189b001057f1a28721b1c3`.

Use the README's verified commands. Use new output paths for repeats.
Do not overwrite historical failures or reuse v10 results for changed identities.
Build canonical binaries with `cargo build --workspace --release --offline --locked`.
A per-package build can produce a different build identity.

## Exact blockers and remaining implementation

1. Required physical controller PCB, RAM, storage, hashboard, ASIC, PSU and cooling
   identities remain unresolved. Public/archive claims are not measured exact
   compatibility. Missing evidence is not incompatibility.
2. Original reset/bootloader/storage paths, most device models, complete target
   dispatch, native services, fault suites and update/recovery remain
   implementation work. These are not external blockers or valid exclusions.
3. AF_INET creation, Unix listeners and several socket operations are denied.
   No authorized external worker is configured. SSH, TCP pools, Core RPC, HTTP,
   separate network client tests and authenticated TLS need a permitted network
   environment. No alternative route was used to evade the restriction.
4. The accessible cgroup mount is read-only. Aggregate cgroup CPU/RAM enforcement
   is still implementation work and needs a delegated test environment.
   See `reports/host-limits-v11.json`.
5. Core and most other preparation tools do not have a complete frozen build
   toolchain. Do not extend the cgminer closure result to them.
6. `.git` is an empty protected mount. No commit is claimed. Source and input
   hashes provide checkpoint identities.
7. Original payloads are workspace links into
   `/home/user/mnt/small/md-sim-archive-20261019`. Check `catalog/storage.json`
   before large copies. Original files remain unchanged.

## Next action after this checkpoint

### Active continuation after v11

The user requested continued implementation with parallel agents. The v11
source freeze is historical. New source changes invalidate affected results.

Current milestones after the initial continuation:

- `native-s9/luci-007/service-report.json` passed independent checks of actual
  archived uhttpd/LuCI login, redirect, stylesheet identity, live uptime, config
  write/commit/readback, service restart and exact byte restoration. Run inside
  the diagnostic ARM guest only. Driver `native-s9/service-driver-007` is frozen
  because concurrent Cargo builds replace the shared example path. Failed
  luci-005/006 evidence remains: build mismatch and kernel-console interleaving.
  The final script retains the kernel ring after the test and records its
  console-level adaptation. No board, browser or remote dashboard pass claimed.
- The exact full SD image executed archived SPL and loaded archived U-Boot.
  NAND, FPGA and reset models remain open. See `s9-boot-progress.md`.
- `network-vm/boot-001` booted a real Linux kernel with Rust init and no network
  attachment. Real guest TCP loopback worked. This isolated VM can test SSH/TLS
  with separate guest client/worker environments; it is not an external host.
- Worker submission tokens now survive a lost start reply. Cleanup saves a
  bounded token receipt. The receipts count against the retained-run limit.
  Private process stdin is streamed through a pipe, never a host-file FD.
- Coordinator local/private-worker fixture suites are implemented. Run004
  exposed a stale-job verifier ordering defect, now under correction. Preserve
  failed results and rerun. No exact physical target passes yet.
- Root is adding OpenSSH control with pinned host keys and one-shot structured
  requests to the owner-only worker socket. Guest integration is in progress.
- `network-vm/ssh-005` now passes real SSH and worker transfer/execution tests
  inside one VM. Separate namespaces and TLS tests are next. The Rust gateway
  reuses Nginx and separates client-certificate auth from firmware auth.
  See `access-progress.md` for exact limits and commands.
- Coordinator006 passed the local/private-worker source suite and exact resume.
  All four fresh block candidates passed actual Core import and persisted tip
  checks. Further review found a missing full notify/difficulty trace binding;
  that correction and a new run are required before the next source freeze.
- Pinned QEMU build now has isolated compiled Samurai and explicitly recorded
  provider-derived keymap sources. The FPGA I²C model has behavior and C ABI
  tests. Actual patched-QEMU boot testing is still in progress.

- Root: add hash-locked process standard input through a supervisor-owned pipe.
  Never give the guest a descriptor for a host object-store file. Ten runtime
  isolation tests pass, including binary input and unread-input cancellation.
  Add native S9 service execution through the actual guest UART. The unchanged
  archived shell ran in `native-s9/shell-001`; service HTTP checks are in progress.
  Its one-CPU, direct-loader, init-shell and XIIC bypass adaptations are recorded.
  This is not an original-board or complete native-dashboard pass.
- Firmware agent: build pinned archived QEMU and implement FPGA XIIC behavior
  from the pinned kernel driver and protocol evidence. Resolve the missing
  Ninja build dependency. Remove boot adaptations only after real model tests.
- Catalogue agent: extracted the exact full SD image and validated its FAT boot
  files against the selected archive. `original-zynq/sd-bundle-004/boot/bundle.json`
  records SPL, U-Boot, FIT, FPGA and image identities. Actual SPL boot is under test.
- Mining agent: implement durable suite dispatch and the same local/worker
  contracts in a new Rust coordinator. Keep source fixture coverage distinct
  from exact physical targets and full-matrix completion.

Next root action: run the original uhttpd and boser services inside the guest,
test real guest-loopback requests, then replace diagnostic boot prerequisites
with the shared verified boot bundle and implemented board models.

Read this file and the component records first. Do not repeat resolved research.
Continue the original S9 path from attempt 008: resolve exact reset and persistent
storage inputs, then implement the FPGA I²C/NAND contracts from the pinned
archived driver and board evidence. Remove the diagnostic bypass only when the
actual interface works. It cannot become an original-board pass while the
one-CPU/direct-loader adaptations remain.

Extend exact profile resolution and executable bundles across the full matrix.
Implement the remaining target coordinator, board models and applicable suites.
Use a permitted isolated worker/client network environment for SSH and TLS.
Do not reduce the requested target set. Freeze new identities and rerun all
affected suites after each change. The final strict matrix must continue to
fail until all required cases have real passing evidence.

Detailed records: `firmware-progress.md`, `mining-progress.md`,
`bitcoin-core-progress.md`, `catalog-progress.md`,
`original-board-progress.md`, `store-progress.md`, `review.md` and
`catalog-review.md`. Scoped correctness findings are resolved. Full-release
completion findings remain open.

## Active continuation after the resource and network review

The full task is still active. No exact physical miner passes all required
gates. Do not return a successful final response or reduce the target matrix.

- Coordinator007 passes the adapted cgminer source fixture locally and through
  the private worker. Full notify/difficulty trace binding is now checked.
  All four fresh candidates also pass actual Core import and persisted-tip
  checks. `reports/coordinator-007-core-independent-review.json` records an
  independent hash and lineage check. This is not a physical S2 target pass.
- `aggregate-limits/run-005/limits-report.json` and its independent report pass
  actual CPU/memory faults, process bounds, cancellation after four children
  start, and SIGKILL worker recovery on the same socket. The report binds
  captured inputs, VM assessment, kernel counters and ownership receipts.
  `aggregate-limits/progress.md` records scope, commands and remaining limits.
- SSH child ownership, deadlines and output bounds were fixed and tested.
  Credentials use bounded OpenSSL execution. `network-vm/namespaces-009` passes
  separate client/worker namespaces, SSH transfer/reconnect/recovery, SSH
  tunnel, mTLS gateway, streams, SSE and WebSockets with a fixture service.
- Native S9 remote runs001–004 failed with retained evidence. Run004 records
  an actual HTTP timeout. Run005 tests a 30-second diagnostic-client deadline
  with process diagnostics. Native and fixture results remain separate.
- Pinned S9 QEMU backend012 passes live IIC and CPU-reset regression tests.
  `qemu-s9/spl-boot-003` executes archived SPL and U-Boot with both CPUs, then
  waits for the missing NAND controller. BootROM entry adaptation is explicit.
  The firmware agent is implementing configurable PL353 behavior on the
  existing QEMU NAND engine. Exact physical NAND identification remains open;
  `qemu-s9/nand-research-001.json` records public sources and failed downloads.
- Original ESP firmware now executes ROM, bootloader, PSRAM initialization
  and application startup. A held private stdin pipe fixed the real UART
  channel. Selecting existing octal PSRAM behavior fixed the prior mismatch.
  The mining agent traced the next wait to unsupported SAR ADC registers and
  is implementing the device from the archived ESP32-S3 technical reference.

Next root actions: finish native dashboard transport diagnostics, review new
device changes, and extend actual firmware execution. Runtime aggregate tests
and scoped strict Clippy pass. Documentation link check passes316 links.
Freeze new builds before integration runs; shared Cargo output can change
when another agent builds. Preserve failed attempts and their exact identities.

## Current continuation: native services, original boot and browser

- Native009 passes the actual archived S9 HTTP/RPC services through SSH and
  mTLS in separate client/worker namespaces. It checks login, assets, advancing
  uptime, configuration write, active-service restart, persistent readback,
  exact restoration and separate cookie contexts. It is a local VM, not an
  external physical host. Browser JavaScript, full reboot and mining are false.
  `network-vm/native-009/root-review-001.json` records independent review.
  Native010 repeats with the current worker/helper/verifier builds after the
  final integer-bound review fix. It is pending.
- S9 backend015 executed archived SPL, U-Boot, FIT kernel and ramdisk init with
  both CPUs and no IIC blacklist. It still uses a declared generic NAND
  fixture. Model command/OOB/reset tests and exact physical NAND identity
  remain under work. Independent review found and is fixing storage boundaries.
- ESP probe017 executes the actual original application's I2C NACK/retry paths,
  filesystem startup and degraded self-test failure. No success was invented.
  Voltage-controller and support-device implementation continues.
- Browser capture007 retains911478408 file bytes with exact hashes and recorded
  symlink normalization. Probe004 failed on the guest rootfs size before the
  browser started. Probe005 uses the same bounded2GiB guest with explicit
  ramfs root and is in progress. New large derived runs are under `/tmp`;
  compact evidence and location records remain in `work/simulator/browser/`.
  See that directory's progress record. No browser gate passes yet.

Verified duplicate historical QEMU build CAS objects were relocated to the
retained backend018 store. Restore maps are in
`qemu-s9/duplicate-cas-cleanup-001/`. Unique files, original archive bytes,
binaries and failure logs were preserved. Do not use hardlinks between CAS
stores: the storage policy rejects external hardlinks.

## Current continuation: original device paths and real browser diagnosis

Native010 passes the actual archived S9 HTTP/RPC service checks with the current
worker and verifier builds. Its selected run is
`d6ace6115e6a7bd4979bb72d1d4e5145b8caf7130e8c3a9b7a0c867f5ec5a41e`.
The report is `network-vm/native-010/remote-report.json`. This is an adapted
native-service path in separate client and worker namespaces inside a private
VM. It is not an external physical host or the original-board boot gate.

S9 NAND backend025 and UBSAN026 pass whole-storage program, erase, OOB,
boundary and restart checks. The independent review is
`reports/pl353-review-003.json`. The unchanged original kernel reads address
cycles from ONFI parameter byte101. The old non-ONFI fixture left those cycles
zero. Backend028 supplies a separately named, evidence-based ONFI development
profile. Its compiled protocol test passes; actual firmware detects256MiB,
scans NAND, writes the bad-block table and creates UBI. Boot and exact physical
NAND identification remain incomplete. ECC and later NAND warnings are under
investigation. No undocumented zero-cycle correction was added.

Original ESP probe021 passes the real fan-controller setup and reaches UART
initialization and detection of one BM1397. The model returns actual programmed
device values. INA260 power measurements and ASIC UART responses remain under
implementation. No synthetic boot or mining result is counted.

Browser probes009 and010 fail. The latter gives a specific internal XML
document failure and no screenshot despite process exit zero. Probe013 traces
failed memory mappings in the private VM. See `browser/progress.md` for exact
failed preparations, input changes and next actions. Actual isolated NSS
certificate import and browser cgroup adversarial tests pass separately. The
real native browser helper and independent verifier are implemented, but their
execution gate remains open.

All exact-target completion gates remain open. The full matrix must still run
with `--require-complete` after implementation and a new freeze. Source-adapted
cgminer mining results and component tests do not complete the physical matrix.

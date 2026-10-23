# Simulator milestones

The full specification remains the project goal. No exact physical target has
passed all gates. The frozen matrix still has 79,599 rows and zero full
exact-target passes.

The user asked on 22 October to stop after each completed milestone for review.
This replaces the earlier request to continue without a milestone handoff.
Do not start the next milestone until the user approves it.

## 1. Usable adapted S9 development run — passed; awaiting user review

Acceptance: one tested start command, hashed inputs, original archived firmware
execution, native dashboard browser/API checks, configuration restoration and
persistence, signed update, fresh work on each boot, independent verification,
and eight Core acceptance/restart checks across four boots. Label the reference
hardware and every adaptation. State the actual dashboard access boundary.

Command: `work/simulator/check-target-085/release/md-sim demo s9 --out NEW_DIRECTORY`.
Input check: the same executable with `demo s9 --check`.
Inputs: `simulator/demos/s9-adapted.json`. All 31 identities passed in check280.
Passed demonstration: `work/simulator/demos/s9-review-284/`.
Demo280 failed its WebSocket sample deadline. Its earlier four Core checks
passed. Demo284 passes with the same frozen inputs and assertions. Host
contention remains a possible cause; the failure stays in its own output.
Demo284 passed four firmware boots, eight different Core-accepted blocks with
restart retention, the native browser, signed installation, settings
restoration, restart persistence and storage checks. See the
[complete report](reports/adapted-s9-milestone-284.md).
It uses the unchanged tested263 helpers/backend. The newly changed FPGA backend
is separate and cannot silently alter this milestone's execution identity.

Dashboard boundary: the native page runs in Firefox in the disconnected support
VM. Login, assets, tables, WebSockets, telemetry, configuration and persistence
are tested there. This command supplies no host or remote browser URL. Host
network listeners are denied in this environment. The selected remote263/264
runs have failed HTTP readiness; they are not dashboard passes.

Estimate: about 34 minutes for the full demonstration, based on cycle263's
measured output timestamps. The earlier 20-minute estimate counted only the
four firmware processes and missed preparation and independent checks.
The estimate is not a timeout extension or a passing result. If it fails,
retain the failure and fix the specific defect before handoff.
Measured demo284 duration: 33.72 minutes. All 575 Rust tests, formatting,
strict Clippy, release builds and guide links pass after the last code fix.
Next action: stop for user review. Do not start milestone 2 before approval.

## 2. One complete exact target and its remote workflow — not complete

Acceptance: all applicable original-board, exact component, FPGA, timing,
mining, Core, storage, fault, native dashboard, isolation and remote gates pass
for one complete execution identity. Keep adapted passes separate.

Blockers: selected 2020 FPGA source/build identity or complete validated
reconstruction, remaining exact controller/hashboard/PSU identities and models,
physical timing/reference evidence, and independent review access. Current
agent usage limits prevent independent agent review until 25 October at 08:48.
Remote nested-emulation runs still have a large wall/guest time mismatch.

Completed component work: four Zynq fabric clock/reset outputs, system-reset
correction, legacy S9IO clock connection and mid-byte clock-gating tests pass
on backend280. The UART, fabric reset, watchdog and TTC reset tests also pass
with undefined-behavior sanitization. Raw fabric reset still needs the external
reset sequencer. Matching 2020 logic is not inferred
from the legacy RTL. See qemu-s9/fabric-derived-276/evidence.json.

Estimate: not defensible until the exact FPGA reconstruction and physical
reference inputs are available and validated. Do not promise an exact-target
completion date from adapted mining results. Next action after approval:
resolve the FPGA and exact-board dependencies, then re-run original-board and
per-target gates. Complete the actual selected-run remote workflow.

## 3. Full requested matrix — not complete

Acceptance: every requested applicable exact identity passes its required
suites; final matrix `--require-complete` succeeds. Only evidenced incompatible
or not-applicable cases may be excluded. Rebuilt, original, API-only and
synthetic results remain separate. Clean-install commands and independent
review must pass against the final frozen release inputs.

Blockers: required implementation and missing evidence across the full matrix.
Estimate: not defensible before exact target model and input gaps are resolved.
Next action after milestone 2 approval: extend real execution paths by exact
hardware/firmware identity, preserve all unresolved rows, and run every suite.

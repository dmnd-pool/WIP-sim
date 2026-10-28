# Simulator milestones

The full specification remains the project goal. No exact physical target has
passed all gates. The frozen matrix still has 79,599 rows and zero full
exact-target passes.

The user asked on 23 October for a smaller milestone 1.2 before milestone 2,
then a stop to save tokens. The checkpoint was committed as `b276dd7a`.
On 26 October the user asked to resume and finish the remaining milestones.
Work continues under that instruction. The historical 1.2 handoff remains
unchanged.

## 1. Usable adapted S9 development run — passed; next milestone approved

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
The user approved the next step on 22 October with “nextt step>”.
The milestone-285 version of this file is preserved in
[the checkpoint copy](reports/milestones-at-checkpoint-285.md).
Later edits do not change that historical result or its recorded hash.

## 1.2. Adapted S9 mining and selected SSH/TLS checkpoint — passed

Acceptance: retain one passed archived-firmware execution with fresh work,
independent header verification, two Core block acceptance/restart checks,
native HTTP through SSH and authenticated TLS for that same selected run,
artifact transfer, evidence retention and cleanup. Save the exact inputs,
tested commands, remaining defects and paused source work. Pass final source
checks and prepare Git exclusions. This is a development checkpoint, not a
reduction of milestone 2 or the complete specification.

Run441 and checks443 pass the execution, mining and access requirements. The
reference S9 has three chains of 63 BM1387 chips and a 10 H/s aggregate cap.
Seven real firmware submissions produced five accepted shares and two
LowDifficulty rejections. Two fresh jobs, a clean-job transition and a
difficulty change were independently checked. Both resulting blocks were
accepted by separate Core nodes and retained after restart. The selected
native service passed 15 HTTP and 15 TLS checks from an isolated client.
See the [full report and tested commands](reports/shared-access-pass-441.md).

The worker and isolated client VM used the same execution host. The test URLs
were private to the client environment and are now stopped. No external
physical host, host dashboard URL, exact physical board or measured physical
timing is claimed. Browser run453 failed and its gateway shutdown
acknowledgment remains incomplete. The separate Firefox startup test457b
passed, but does not close the native browser gate. All these gates remain
open at the1.2 checkpoint. The later adapted run495 passes the selected native
browser path, with separate evidence below. This does not change run453.

Final current-source checks pass: formatting, strict workspace Clippy and
919 tests in 109 suites, with zero failures and 38 explicitly ignored
integration helpers. The ignored helpers do not establish execution passes.
Git exclusions pass 33 include/exclude checks. The [handoff](milestone-1.2-handoff.md)
records the Git scope and next actions at that checkpoint. Work resumed
on 26 October after the user made the commit.

## 2. Complete exact S9 target and its remote workflow — in progress

Priority confirmed by the user on28 October: finish the complete S9 target
first. Checkpoint other miner development. Stop after all S9 milestone2 gates pass
and save a tested checkpoint. Do not count the adapted S9
demonstration, HDL compilation or a bus fixture as this milestone passing.

Acceptance: all applicable original-board, exact component, FPGA, timing,
mining, Core, storage, fault, native dashboard, isolation and remote gates pass
for one complete execution identity. Keep adapted passes separate.

Blockers: selected FPGA source/build identity or complete validated
reconstruction, remaining exact controller/hashboard/PSU identities and models,
and physical timing/reference evidence. Independent agents are available again
after the 23 October daemon restart. They are reviewing transport and FPGA
model behavior. The new native worker beside a private client VM passes
artifact transfer and process execution. Its original-firmware run354 also
passes fresh-work verification and two Core acceptance/restart checks. This
uses adapted reference hardware, not an exact physical target. The selected
SSH/TLS API path passes in adapted run441. The later
[adapted run495](reports/native-browser-mining-pass-495.md) also passes the real
native browser through both selected origins, separate cookie stores, clock
bounds, retention and shutdown. It has two independently verified Core blocks.
The topology avoids nested CPU emulation. Exact-target restart persistence,
client-loss survival and the complete exact-target remote workflow remain open.

Completed component work: four Zynq fabric clock/reset outputs, system-reset
correction, legacy S9IO clock connection and mid-byte clock-gating tests pass
on backend280. The UART, fabric reset, watchdog and TTC reset tests also pass
with undefined-behavior sanitization. Raw fabric reset still needs the external
reset sequencer. Matching 2020 logic is not inferred
from the legacy RTL. See qemu-s9/fabric-derived-276/evidence.json.

The [selected shared-access run441](reports/shared-access-pass-441.md) adds
15 native HTTP checks through SSH and 15 authenticated TLS checks from a
separate client namespace. Its independent audit verifies 1,687 computations,
two fresh jobs and five accepted shares from seven submissions. Both exact
candidate blocks pass Core acceptance and restart checks443. Native, pool
and access evidence was retained before cleanup. This closes that adapted
component path; it does not change this milestone's acceptance or estimate.

Estimate: not defensible until the exact FPGA reconstruction and physical
reference inputs are available and validated. Do not promise an exact-target
completion date from adapted mining results. Next action:
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
Stop after milestone 2 completion and save its tested checkpoint. Resume this
matrix only when the user asks. Then extend real execution paths by exact
hardware/firmware identity, preserve all unresolved rows, and run every suite.

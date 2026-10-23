# Milestone 1.2 handoff — 23 October 2026

Status: milestone 1.2 passed. Work is stopped at the user's request.
Milestone 2 and the full simulator
are incomplete. The frozen matrix has 79,599 rows and zero full exact-target
passes. Resume only when the user asks.

## What passed

[Run441 and checks443](reports/shared-access-pass-441.md) executed archived
Braiins S9 firmware `2022-04-29-0-5eb2c5fe-22.02.4-plus` in an adapted reference
environment. The original SD image SHA-256 is
`97a257571af4fb9e62f709e2d6b8f36a191fdd07f1cd12ae780bd1c73af7eaef`.
The hardware model has three chains of 63 BM1387 chips. Its aggregate cap is
10 H/s. Adaptations and all component identities are in the linked report.

The independent audit checked 1,687 actual computations and seven firmware
submissions: five accepted shares and two LowDifficulty rejections. It checked
two fresh jobs, a clean-job transition and a difficulty change. Two distinct
blocks passed isolated Bitcoin Core acceptance and restart checks. Core did
not generate these blocks. The archived firmware and device response path
produced them.

The same selected run passed 15 native HTTP checks through SSH and 15 native
TLS checks. These covered authentication, original assets, live telemetry,
configuration write/readback/restoration and session rejection. Artifact
transfer, destination hashes, retained evidence and owned cleanup passed.
The exact command and audit commands are in the report. Use a new output
directory when repeating them. Preserve the frozen440 helpers and input
paths. A newer auditor correctly rejects reuse when its verification identity
differs; do not overwrite the old report or weaken that check.

The native worker and disconnected client VM ran on the same host. During
the test, the client used `http://127.0.0.1:18090/` and
`https://192.0.2.1:9443/`. These were private test URLs. They are now stopped
and cannot be opened from the host browser. No external physical-host test
or exact original-board pass is claimed.

## Source checks

Final formatting, strict workspace Clippy and workspace tests passed.
There were 919 passed tests, zero failures and 38 explicitly ignored
integration helpers across 109 suites. Ignored helpers are not execution
passes. Logs are retained in [the checkpoint directory](milestone-1.2-checkpoint/).
The Git exclusions passed 33 positive and negative cases with actual Git
matching in a separate empty temporary repository, which was then removed.
These source checks cover the current source. The passed441 firmware run uses its separate frozen440
source and binaries; a current-source check does not replace that identity.

Run source checks from the repository root:

```sh
export CARGO_TARGET_DIR=work/simulator/check-target-085
export CARGO_INCREMENTAL=0
export TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline --locked -j 2 -- -D warnings
cargo test --workspace --all-targets --offline --locked -j 2
python3 scripts/check_guide_links.py
```

These commands need the existing workspace, vendored dependencies and declared
toolchain. Clean installation of the firmware bundle remains open.

## Git and retained storage

The user will create the Git repository and commit. No agent commit or
snapshot repository is needed. The existing root `.git` directory is empty
and read-only in this environment; an earlier `git init` failed for that
reason. No commit is claimed.

The root `.gitignore` excludes build products, node modules, Python caches,
local credentials and agent state. It also excludes original payloads,
derived source extracts, regenerated catalogue indexes and large run data.
It keeps implementation source, vendored Rust dependencies, catalogue source
records, acquisition manifests, compact reports, durable progress, frozen440
source, this handoff and the two paused source changes below.

Ignored files remain on disk. Git is not a backup of the firmware archive or
raw test evidence. Preserve `artifacts/files/`, its linked storage root in
`catalog/storage.json`, and `work/simulator/`. Raw run441/443, failed453,
startup457b, prepared images, backends and frozen helpers are needed for
review or repeat execution. Do not use `git clean -fdx` here. Do not remove
ignored files to make the first commit smaller. After Git setup, inspect
`git status --short` and the staged file list before committing.

## Remaining defects and next actions

1. The [selected browser run453](reviews/native-browser-retention-independent-453.md)
   failed Firefox startup and client gateway shutdown acknowledgment. Its
   native API results do not make the browser pass. Baseline457b subsequently
   passed two real browser startups in 202,007 and 202,503 ms. Their sum exceeds
   the native 305-second observation window. The cause is not established.
   The [diagnosis and prepared comparison command](reviews/browser-startup-diagnosis-456.md)
   specify the next isolated comparison458b. It has not run. Do that comparison
   before changing production cache policy or repeating the full run. Keep
   existing deadlines and assertions.
2. [Recovery459](shared-recovery-development-459/design.md) is staged source
   only. It has not been built, tested or integrated. Fix its whole-file clone
   operation to enforce the recorded copy bound under concurrent source
   growth. Then test its separate retained-output path and independent audit.
   Failed453 mining and Core checks remain pending.
3. [Cleanup455](cleanup-development-455/README.md) is staged source only. It
   has not been built, integrated or used to delete data. Validate its locked
   object-store checks before use. Preserve original downloads and the named
   canonical input stores.
4. Milestone 2 still needs an exact FPGA build identity or validated
   reconstruction, remaining board and PSU identities/models, physical
   timing evidence and all required original-board and remote tests. The
   [SPL extractor454](reviews/s9-spl-extractor-executed-454.md) recovered real
   firmware register and timing contracts. This is static firmware evidence,
   not a measurement of physical timing.

All simulator VMs and development commands are stopped after the final checks.
No458 comparison, new firmware run or staged cleanup is authorized by this
handoff alone. The user's next resume request starts the next work period.

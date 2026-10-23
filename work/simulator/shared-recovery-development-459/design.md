# Shared evidence recovery proposal 459

Status: staged source only. No production file was changed. No build, test,
firmware run, VM run or evidence copy has been performed for this tool.

Run 453 has complete acknowledged native and pool bytes. Its separate access
suite failed. The native process exited zero in 305,140 ms. The outer process
also exited zero. The existing strict shared verifier can check these process
outcomes without granting an access-suite pass. The missing derived
`execution/pool-result.json` prevents the existing strict submission and
regtest audit. The parent skipped observation retention after the access
failure. No missing original mining trace has been identified.

The [independent retention review](../reviews/native-browser-retention-independent-453.md)
pins the original failures and byte graph. The selected run is
`run-d7b13b55416780e0443c175955b8ec9dc008b888a58bcb8bedd330e6b5315cc4`.
The locked-run hash is
`460917fcc32ef1bf7829075124499366e0ba409ff32db4c6f1c690906412b930`.
The native result is `c18c239c1a3417a3f7573d75a443dce30763a7d38cc5562007fcbb05f87190c7`
(105,459,647 bytes). The pool bundle is
`a86232130a35647f6abe6bc593ed22fd34940fd50e47232301ab24ecb66891f7`
(102,679 bytes). The access bundle is
`b46e64375fc5df3760e2ca2482a54845d60b8567c2deb40b39ad03042b845ffb`
(11,206,012 bytes). The failed outer assessment remains an input, not a verdict
to replace.

## Small interface

The staged Rust example has two explicit commands:

```text
s9_retain_shared_evidence plan SOURCE_EXECUTION NEW_PLAN
s9_retain_shared_evidence retain SOURCE_EXECUTION NEW_ROOT
```

`plan` reads bounded metadata and lists required files and total logical copy
bytes. It grants no provenance or mining result. `retain` validates the source
with the existing `external_pool::verify(false)`, hashes the bounded plan,
and creates an owner-only new output root outside the original run tree.
The exact outer/execution relation is preserved. Every input gets a distinct
destination inode through Linux FICLONE, if supported, or a bounded stream
copy. Both source and destination hashes must match. Hard links and symlinks
are not used.

Before publishing a new pool report, source and destination joins must be
equal. All input hashes are checked again. The only new observation is
`NEW_ROOT/execution/pool-result.json`, from the unchanged verifier. Its entire
value must later satisfy the existing strict `pool == retained` comparison.
No verifier predicate or existing command is changed. The new `recovery.json`
pins all copied identities, methods, tool/source identities and failed access
verdicts. It says mining and Core acceptance have not run. An error after copy
starts leaves a failure report and the partial files for inspection.

The original failed access assessment, acknowledged access bundle, joined
receipt and failed client report are copied byte for byte. The access bundle
already contains all raw access file bytes. The recovery tool does not copy
the 251-file extracted access tree or invent the old legacy retainer's
`execution-identity.json`, `export.json` or dashboard metadata.

## Volume and storage policy

The metadata-only [file list](projected-files.txt) and
[size observations](projected-sizes.txt) contain 43 files. Their logical total
is **1,038,030,428 bytes (0.966741 GiB)**. The large items are the selected
combined support image, the original outer and native result envelopes, the
native trace and the original failed assessment. No firmware archive or
input-store tree is copied. The total counts both required native result
paths, even though their bytes are equal.

Preflight rejects more than 256 files or 2 GiB total. Each image is capped at
1 GiB. Process results use the existing runtime result bound. Metadata, logs,
pool bundles and sessions keep separate finite bounds. The complete plan is
checked before a destination is created. `create_new` prevents replacement.

Keep one complete derived root while strict audits and Core evidence refer
to it. Reflinks share physical extents without sharing writable inodes. If
reflink is unavailable, the recorded logical total is also the conservative
new-data budget. Do not delete the original 453 tree, its archive, or unique
results. Do not teach an existing cleanup utility to accept this new layout
without a separate reviewed restoration contract. Failed partial recovery
roots can be reviewed by their exact plan and failure record before any
targeted cleanup; the implementation does not delete files automatically.

## Proposed validation after the execution hold

1. Parent reviews and installs only the two new source files. Existing
   mining and Core examples remain unchanged.
2. Run focused tests, strict Clippy and a release build. Exercise the ignored
   exact 453 metadata-plan test with an absolute environment path. Check
   malformed paths, symlinks, bounds, changed identities, distinct output
   inodes and the preserved failed access verdict.
3. Run `plan` and inspect its actual projected volume before `retain`. Choose
   a new retained output directory. Keep all failures.
4. Run existing `rates-shared` and `regtest-shared` against the derived
   `execution` directory. Require real fresh-job replay and complete hardware
   correlation. Generate no nonce or header in this retainer.
5. Run the unchanged Core helper's explicit `shared` mode for both candidates.
   Each helper must rerun the strict session audit and compare exact results.
6. Independently review source and actual retained evidence. The access gate
   remains failed even if mining and both Core candidates pass.

Proposed commands are not verified yet. No new firmware execution, exact-board
acceptance, full-dashboard acceptance or complete target pass is claimed.

## Paused handoff

The user requested the milestone 1.2 checkpoint and a stop until next week.
All commands are idle. The proposal remains staged, unbuilt and untested.
Production source and original 453 evidence are unchanged. Independent source
review was requested but has not closed. Staged files are
`s9_retain_shared_evidence.rs` and `support/s9_shared_recovery.rs` in this
directory. Their intended production paths are the matching example and
support paths under `crates/md-sim-firmware/examples/`.

One author finding must be fixed before validation: whole-file `FICLONE` can
observe a concurrently grown source after the size check. Replace it with a
bounded `FICLONERANGE` request for the exact expected size, skip cloning empty
files, and retain the bounded copy fallback and both hash checks. The pinned
libc 0.2.183 exposes the required range structure and request. This source
finding is not a test result.

After the user resumes, finish independent review, resolve findings, then
coordinate production installation and Cargo ownership with the parent.
Proposed focused commands are `cargo test -p md-sim-firmware --example
s9_retain_shared_evidence --offline --locked` and `cargo clippy -p
md-sim-firmware --example s9_retain_shared_evidence --offline --locked -- -D
warnings`. The ignored metadata test needs
`MD_SIM_SHARED_RECOVERY_SOURCE=/home/user/src/md-sim/work/simulator/network-vm/native-browser-453/execution`.
Use the existing check target and temporary directory recorded in the parent
continuation. Run no copy until the actual plan confirms the volume above.
Strict mining replay and both Core checks are still pending for 453. Its
browser and gateway acknowledgment failures remain failures.

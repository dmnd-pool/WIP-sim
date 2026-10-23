# Finished T17 oracle duplicate plan 444

No files were removed. Space is sufficient for the bounded next oracle runs.
This cleanup is subsequent work, not an execution blocker.

Keep `work/simulator/bitaxe/t17-core-oracle-274/inputs` as the canonical
store. Compare the finished272 store against it. The captured manifests
declare43 common objects with a total size of541,161,169 bytes. One object
in272 is unique. The precise candidate and restore paths are in
[the planned record](t17-oracle-cleanup-plan-444.json). These are manifest
matches only. Stored bytes must be verified before removal.

The existing cleanup utility must retain its strict path checks. Do not
rename stores, use links or substitute another permitted path. A narrow
extension can accept only repository paths of this form:

`work/simulator/bitaxe/t17-{core,readback}-oracle-NNN/inputs`

For this branch, require real directories and bounded regular evidence files.
Decode the exact `ProcessSpec` and terminal `ProcessResult`; bind the hex
stdout and stderr to `evidence.tar` and `stderr.log`. The old successful
assessment schema is `md-sim-vendor-core-packing-oracle/1`; the new explicit
mode uses `md-sim-vendor-core-readback-oracle/1`. Validate the selected mode
against the captured argv and input paths. Verify extracted assessment and
result bytes against exact tar members, rather than trusting adjacent names.
The assessment pins the copied original miner, C wrapper, derived executable
and output. New mode also pins the link script and derived ABI header.

Acquire exclusive locks for canonical and target stores through the existing
Store API. Recheck the terminal evidence after taking the locks. For every
candidate, validate the complete content hash and declared size at both the
canonical restore path and target path. Record both paths and identities in
the durable journal before removal. Reuse the utility's same-open regular
file checks and device/inode checks. Keep all locks until removal completes.

Preserve the entire canonical store, all unique objects in each target, the
original archive, original extracted miner, fixtures, manifests, assessment,
executable, logs, result and evidence tar. Retain a usable restore source for
each removed byte. Extend the same procedure to the owned new oracle stores
only after their terminal results have been saved. A failed oracle remains a
failed result; duplicate removal must not change that result.

Tests for the extension should cover malformed or mismatched evidence,
active store locks, changed bytes, unique objects, restore equality and
refusal of paths outside this exact scope. Existing cleanup behavior stays
unchanged. Build and execute these checks after browser445 ends.

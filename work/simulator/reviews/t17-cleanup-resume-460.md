# T17 cleanup integration 460

The staged 455 Rust extension is integrated and its focused checks pass. The
actual locked plan verifies 128 duplicate objects and 1,623,481,881 bytes against
protected canonical 274. The actual cleanup then removed those exact duplicates.
The [plan](../cleanup-t17-460/plan.json) records removed_bytes 0 and plan-only.
The [before inventory](../cleanup-t17-460/before.json) hashes all 242 retained
regular files across 272, 274, 446 and 447 (2,174,433,532 bytes).

The first actual-evidence check failed. Staged 455 assumed that every tar
member also had an adjacent extracted file. Original 272 retained nine logs
only inside its complete evidence tar. Original 274 retained six empty logs
only in that tar. Both complete tar files are exactly bound to ProcessResult
stdout. No historical files were added or changed to resolve the failure.

The admission now permits those exact historical omissions only when the run
name, full tar hash and byte size match. Run 272 uses 97e3621a9c5d09da6240bcd52826bba2a88989064a91f0fc63033b1950d0d43c/208,384 bytes;
Run 274 uses 1de892e559bcf83b6a8fbc1e36a59f1b3064253ff5e736fd13a8b6bdc44bb96e/208,896 bytes.
Each decoded member still gets a hash. Every adjacent file that exists must
match. The journal lists archive-only member names. Other missing files fail.
Negative tests reject a changed archive, another run name and synthetic
records with the old names. Runs 446 and 447 require all their adjacent copies.

The new optional --plan mode uses the same store locks, exact input hashes,
protected canonical checks and final evidence recheck. It creates a new plan
journal with planned_bytes and removed_bytes 0, then returns before removal.
The normal deletion command still rechecks the original inputs independently.
A plan cannot authorize changed files at a later cleanup.

Checks: 16 ordinary tests passed; three explicit integration helpers remained
ignored in that ordinary invocation. The actual archived-evidence test then
ran once and passed for all four records. The real T17 CLI scratch test ran
once and passed active-owner refusal, changed-evidence refusal, protected
canonical refusal, plan preservation, duplicate-only deletion, failed-evidence
preservation and exact copy restoration. The prior component CLI regression
also ran once and passed. Strict Clippy, release build and scoped formatting
passed. The first failed actual-evidence log remains retained separately.

[The tested input manifest](../cleanup-t17-460/tested-inputs.json) pins source,
logs and the frozen executable. The utility SHA-256 is
46b3453ebbc7c28b53091b7bd10449e0b0642fe78cf8d353dfadfeba6eecfcbb,
1,554,360 bytes. Test logs are under checks-2026-10-26-001/t17-cleanup460b-*.
These are cleanup checks. They grant no firmware, mining or physical-target
acceptance.

The following command was executed successfully from the repository root:

```sh
work/simulator/cleanup-t17-460/reclaim_qemu_inputs --plan \
  work/simulator/bitaxe/t17-core-oracle-274/inputs \
  work/simulator/cleanup-t17-460/plan.json \
  work/simulator/bitaxe/t17-core-oracle-272/inputs \
  work/simulator/bitaxe/t17-readback-oracle-446/inputs \
  work/simulator/bitaxe/t17-core-oracle-447/inputs
```

The plan output already exists. A new plan needs a new output path. The
[independent review](t17-cleanup-independent-460.md) checked the final source,
plan and both byte copies before cleanup. The actual command then used the
same frozen utility and arguments without --plan, with a new output journal
`work/simulator/cleanup-t17-460/cleanup.json`. It exited zero.

The [completed journal](../cleanup-t17-460/cleanup.json) records 128 removed
objects and 1,623,481,881 logical bytes. The [postcheck](../cleanup-t17-460/postcheck.json)
passes: all 114 remaining files match the before inventory, and all 43 canonical
restore files match their journal identities. No unexpected file was added.
The protected canonical store, unique inputs, results, logs, source files and
adjacent archives remain unchanged. Original downloaded files were outside
the operation. This is logical byte accounting, not a physical filesystem
allocation measurement.

The explicit archived-evidence test passed before cleanup. Repeating that
test or an old oracle run now requires restoring its missing input objects
from the exact journal copy sources. Restore with regular copies, verify the
recorded SHA-256 and byte size, and retain one link. Never use hard links.
The ordinary tests and scratch CLI tests do not need those historical inputs.

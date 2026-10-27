# Browser duplicate cleanup481

The Rust reclaimer passes its focused tests. The locked read-only plan passes.
Actual removal is pending independent verification of the planned pairs.
This work does not change a firmware, dashboard, or exact-target verdict.

The helper is frozen at `work/simulator/cleanup-browser-tools-481/reclaim_qemu_inputs`.
Its SHA-256 is `d2e2c71c68de741cb5c3e227f2eba0864c17048475e6c181fd842e50e3a24c1d`.
The helper and source identities are in that directory's `inputs.json`.
The common source checkpoint486 is unchanged.

The focused example suite passed 23 tests. Four tests require explicit inputs
and remained ignored in that command. Strict Clippy, release build, and scoped
Rust formatting passed. The new real-CLI test then passed explicitly. It ran
the frozen helper against temporary labelled stores. It checked planning,
removal, preservation, restoration, and refusal to overwrite an old journal.
The ordinary tests also cover active locks, incomplete transfers, corrupt
restore bytes, symlinks, hard links, changed input identities, nonterminal
results, invalid and pending policies, bounded JSON parsing, and unexecuted
preparation records.

The logs are in `work/simulator/checks-2026-10-26-001/browser-cleanup481-*.log`.
These commands were tested from the repository root:

```sh
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 \
  CARGO_TARGET_DIR=work/simulator/check-target-085 \
  cargo test -p md-sim-firmware --example reclaim_qemu_inputs --offline --locked
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 \
  CARGO_TARGET_DIR=work/simulator/check-target-085 \
  cargo clippy -p md-sim-firmware --example reclaim_qemu_inputs --offline --locked -- -D warnings
CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 \
  CARGO_TARGET_DIR=work/simulator/check-target-085 \
  cargo build -p md-sim-firmware --example reclaim_qemu_inputs --release --offline --locked
MD_SIM_BROWSER_RECLAIM_TOOL=/home/user/src/md-sim/work/simulator/cleanup-browser-tools-481/reclaim_qemu_inputs \
  work/simulator/check-target-085/debug/examples/reclaim_qemu_inputs-8bed7308c8d4ecd3 \
  --ignored --exact browser::tests::real_cli_plan_reclaim_and_restore --nocapture
work/simulator/cleanup-browser-tools-481/reclaim_qemu_inputs --browser-plan \
  work/simulator/cleanup-browser-481/inventory.json \
  work/simulator/cleanup-browser-481/plan.json
```

The plan output already exists. Use a new journal path for another plan.
The original inventory480 remains unchanged. The derived inventory481 selects
3,653 duplicate copies with 1,447,428,871 logical bytes. It explicitly retains
two 489-byte copies from recovered inspection and gateway stores. Those stores
omit their empty `incoming` directories. The reclaimer keeps its strict store
validation. It does not create directories to make these copies eligible.
All seven objects in those two stores are separately hashed in
`cleanup-browser-481/retained-recovered-stores.json`.

The actual plan holds existing store locks and verifies every selected copy
and its restore source. It records 3,551 distinct restore paths, 27 unique
retained objects, and 14 original evidence and policy files. Run469's assessment
remains false. Preparation462 has no claimed execution result. The plan hash is
`a966c83557e0512795f1cf6f6565df1ba7813c45dc95f45fb12ec0890e8621fd`.
No archive file, unique object, failed log, result, or frozen helper is eligible
for removal.

Next: independently verify the actual pair list and current source pins. Then
run the same frozen helper with `--browser-reclaim` and a new journal. Verify
the retained stores and original evidence again before reporting completion.

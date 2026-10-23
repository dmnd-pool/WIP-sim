# Browser 451 inventory failure and fix 452

Run 451 failed before the native job or browser started. The outer emulator
exited 0 after 44,051 ms without truncation. Its actual guest log reports
`component input count exceeds bound`, then PID 1 exits and the kernel panics.
The final assessment correctly records failure and no native carrier receipt.
Kernel startup and emulator exit 0 are not firmware or browser passes.

The failed run remains at
[native-browser-451](../network-vm/native-browser-451/assessment.json).
Its result SHA256 is
`afb6854e78188d7584d07bd353637712f34286faf7bbe0b03db6037405e47a4b`;
its guest log SHA256 is
`fa174074a559679ebb50ae4dbe25964d1bf7ff0b6be6320bfae024798a0ef908`.
Frozen 450 and all failed run 451 files are unchanged.

## Exact cause and change

The initial inventory contains 4,619 regular files and 1,105 browser links:
5,724 unique records. This count is reconstructed from the exact retained
root manifest, payload manifest and initial capture paths. The complete root
manifest has 4,632 regular files.

The old function used one per-path `browser` flag for two different limits.
Capturing `/usr` set the total record limit to 8,192. The next shared input,
`/shared-selection.json`, used strict file rules and changed that total limit
to 1,024. It failed before reading that input, although the total was below the
declared browser inventory limit.

The fixed initializer chooses one total limit for the complete inventory:
8,192 for a requested browser run, otherwise 1,024. That value reaches every
root capture and recursive capture. The per-path browser flag still controls
symlinks and maximum file size. Strict shared and carrier inputs still reject
symlinks and files larger than 256 MiB. Browser paths retain their existing
512 MiB file rule. Aggregate byte limits and native execution limits did not
change.

Capacity is charged only for a new file or link record. A strict recheck of an
already captured tool at the exact limit remains valid. An empty directory
does not consume capacity. A new record above the limit fails before changing
the inventory. Strict link and size checks run even for a repeated path.

Only `remote_namespace_check.rs` and its new
`remote_namespace_check/input_inventory_tests.rs` module changed.

## Executed checks

The final affected helper suite passes 101 tests. Six entries are explicitly
ignored by that normal harness. The new ordinary tests use real files to
check a 1,200-entry browser inventory followed by a strict shared file, exact
1,024/8,192 capacities, repeated paths, empty directories, one extra record,
strict symlink rejection and a sparse file above 256 MiB. The sparse negative
is rejected by metadata before hashing; no large data payload is created.

The actual isolated regression is executed separately, not inferred from an
ignored test count. It captures the local test executable and its exact ELF
library closure. The runtime creates a closed root with no external network,
30 seconds wall time, 20 CPU seconds, 256 MiB address space and 64 MiB scratch.
The child performs the real mixed inventory and both exact-cap tests. It also
checks that a host-only sentinel path is absent. The final child result is
`exited`, exit 0, untruncated, 180 ms. The outer explicit test reports one pass
and zero ignored tests. This is input-inventory component evidence only.

The exact final command, from the repository root, was:

```sh
MD_SIM_INVENTORY_EVIDENCE=/home/user/src/md-sim/work/simulator/reviews/input-inventory-452b-execution \
CARGO_TARGET_DIR=work/simulator/check-target-085 CARGO_BUILD_JOBS=1 \
TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 \
cargo test -p md-sim-firmware --example remote_namespace_check --offline --locked \
  input_inventory_tests::actual_isolated_inventory_regression -- --exact --ignored --nocapture
```

Use a new evidence directory to repeat the command. It refuses an existing
directory. The initial 452 execution is also retained. The final 452b execution
adds the reviewer's explicit `status == exited` success assertion.

Final evidence:

- [Affected helper tests](../checks-2026-10-21-001/input-inventory452b-all-tests.log).
- [Actual isolated test](../checks-2026-10-21-001/input-inventory452b-isolated.log).
- [Strict Clippy](../checks-2026-10-21-001/input-inventory452b-clippy.log) and
  [scoped formatting](../checks-2026-10-21-001/input-inventory452b-fmt.log).
- [Isolated ProcessSpec](input-inventory-452b-execution/process-spec.json),
  [result](input-inventory-452b-execution/result.json) and
  [actual child output](input-inventory-452b-execution/guest.stdout).
- [Source, log and exact input hashes](input-inventory-fix-452-inputs.json).

The final result SHA256 is
`dcddce8c36487e754a4bd1d38f428b9db28b72a4d8e04f939c8a7a618aaf8fb9`.
The isolated ProcessSpec SHA256 is
`db9a3fa662cd09bb325ab2fdecfd72c348e7f90bf65318b55e47524aa36edba2`.
The [independent review](input-inventory-independent-452.md) checks the final
source, test design and actual compiled evidence. It has no unresolved finding.

The next step is a new source freeze and browser run. This fix does not turn
451 into a pass, establish native browser behavior or satisfy an exact-target
gate. Original archives and unique evidence were not deleted.

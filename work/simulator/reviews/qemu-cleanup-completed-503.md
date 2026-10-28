# Duplicate QEMU build inputs reclaimed 503

The cleanup removed 1,353 verified duplicate copies and reclaimed
14,807,692,817 logical bytes, about 13.79 GiB. Root filesystem free space rose
from 13 GiB to 27 GiB. No original archive, unique input, backend binary,
build result, log or frozen helper was removed.

The targets were 21 finished build directories from166 through280. They had
1,311 duplicate content-addressed input objects and42 duplicate derived
`overlay.tar.gz` or `keycodes.tar.gz` files. Every removed byte has an identical
retained copy in `work/simulator/qemu-s9/backend-build-280-release/`.
The four failed builds remain failed. Their evidence is unchanged.

The [independent plan review](qemu-cleanup-independent-plan-503.json) rehashed
all1,353 targets and77 distinct restore files. The frozen481 Rust reclaimer
then acquired all22 store locks, rechecked both copies and wrote a durable
journal before removal. Its completed object list exactly matches the
reviewed plan. The [author plan record](qemu-cleanup-plan-503.json) gives the
scope, source identities and guards.

The postcheck acquired the same22 locks. It rehashed all429 retained files
and all77 restore files, verified all1,353 removed paths were absent, and
checked the exact remaining object lists and empty incoming directories.
All retained file identities and contents matched the prior snapshot.

The current shared CLI input check also passed after cleanup:

```sh
work/simulator/cli-tools-500/md-sim demo s9-shared --check
```

It verified3,579 workspace inputs and43 host inputs. This check did not run
firmware and does not change any conformance result.

Records are under `work/simulator/cleanup-qemu-503/`:

- `request.json` has the exact tool arguments and terminal build states.
- `plan.json` is the reviewed plan; `before.json` pins every retained file.
- `reclaim.json` is the completed restoration journal. Do not rerun it.
- `postcheck.json` records the retained-file and exact-directory checks.
- `shared-cli-check.log` records the actual current-input verification.

To restore a removed input, select its entry in `reclaim.json`. Acquire the
affected store's exclusive `store.lock` while its owner is stopped. Verify
`restore_copy_from` against the recorded SHA-256 and size. Copy its bytes to
the absent `path`, using a new file rather than a hard link. Do not overwrite
different existing content. Check the destination hash and size before
releasing the lock. Restore a derived archive under the same build-store
lock. The earlier [real cleanup CLI test](browser-cleanup-tested-481.md)
verified this copy-and-rehash restoration procedure.

The original archive, canonical274, current Cargo store, browser inputs,
new501/502 work and all unique evidence remain outside the removal scope.

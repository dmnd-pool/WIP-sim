# Browser duplicate cleanup481 completed

The cleanup removed 3,653 verified duplicate copies. It reclaimed
1,447,428,871 logical bytes, about 1.35 GiB. It preserved the archive, unique
inputs, all failed-run evidence, and frozen helpers. Run469's assessment remains
false. This cleanup does not establish any firmware or target acceptance.

The [tested checkpoint](browser-cleanup-tested-481.md) records the focused
tests and frozen helper. The [independent pair review](browser-cleanup-plan-independent-481.json)
checked every candidate and restore copy before removal.

This command passed with the same frozen helper and a new durable journal:

```sh
work/simulator/cleanup-browser-tools-481/reclaim_qemu_inputs --browser-reclaim \
  work/simulator/cleanup-browser-481/inventory.json \
  work/simulator/cleanup-browser-481/reclaim.json
```

Do not repeat that command: the duplicate copies are now absent and the journal
already exists. `reclaim.json` records every original path, restore source,
content identity and pre-removal file identity. `reclaim.completed.json`
records the successful completion. Both files are in
`work/simulator/cleanup-browser-481/`.

An additional postcheck held all store locks and rehashed 3,599 retained files.
These include every restore copy, all unique objects in affected stores,
original result/log/assessment files, and store policies. It checked that every
removed path is absent and that each affected object directory contains exactly
its retained objects. The complete result is `postcheck.json` in that directory.

Both recovered inspection and gateway stores remain unchanged. All seven of
their objects, including the two 489-byte candidate copies, are retained and
rehashed. No exception was added for their missing `incoming` directories.

To restore a removed copy, use its journal entry. Copy `restore_copy_from` to
the absent original `path`. Verify the recorded size and SHA-256 after copying.
Do not hard-link the file or overwrite different existing content. The explicit
real-CLI test verified this restore procedure before actual removal.

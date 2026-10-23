# Verified resource cleanup 421

Removed 325 duplicate input objects: **559,650,689 bytes**. The cleanup used the
existing `reclaim_qemu_inputs` program. It took exclusive store locks, checked
the finished run records, verified both copies, and saved a journal before
removing each duplicate. No simulator source or new build was made.

The three completed journals and exact command arguments are in
[the cleanup record](resource-cleanup-421/commands-debug.json). The independent
[post-check](resource-cleanup-421/verification.json) verifies that all 325 removed
paths are absent and that all 24 distinct restore files still have the required
hash and size. It also hashes and syncs the completed journals.

The program used was the existing debug binary, SHA-256
`c58a759f101fca27d415996b374331391257e72cecee71f28f65efa1af4ad6bb`.
The older release binary, SHA-256
`d65c9ab9d37fb6778698598579dc0bfcd60ba2c630d68bfcbf30a97e6d187dc9`,
first rejected the SSH canonical path. It created no journal and removed no
object. That failed command and its log remain in the record. The later existing
debug binary admits the declared channel and SSH paths. It completed all three
groups. The current source whitelist tests also pass in workspace test 412;
this is not a claim that the old debug binary was rebuilt from source 412.

Canonical stores 342 and 375 remain intact. Prepared bundles 390 and 420,
active `native-access-421`, frozen helpers, results, logs, firmware inputs in
the archive, and all unique store objects remain intact. Restore a removed
object by copying the journal's `restore_copy_from` path to its `path`, then
verify the recorded hash and size. Do not use hard links.

Two finished support runs remain outside the program's explicit whitelist:

| Store | Verified matching objects | Matching bytes left intact |
| --- | ---: | ---: |
| `network-vm/byte-stream-399/input-store` | 33 | 39,501,688 |
| `network-vm/worker-timeout-406/input-store` | 27 | 36,570,856 |

These paths are relative to `work/simulator/`. The plan records every matching
object and its restore source. The post-check verifies those bytes without
deleting them. Their complete stores contain 116,702,583 and 105,361,270 bytes;
only the matching subset is a cleanup candidate.

The next safe action is a separate small change to the cleanup utility after the
active integration run. Add only these support-run path forms. Require their
actual `process_result`, `process_spec`, and `guest_log` assessment identities to
match the saved files. Require terminal execution and the existing exclusive
store locks. Add negative tests for active runs, changed identities, and names
outside the new forms. Then run the updated utility with new journals. Do not
rename directories to bypass the current whitelist.

After cleanup, the root filesystem reported 90,883,842,048 bytes available.
`/tmp` reported 14,192,439,296 bytes available. Those readings include other live
work; the verified reclaimed byte count above comes from the journals.

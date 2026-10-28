# Runtime storage relocation pilot517

This is a fixed two-run storage tool. It handles only completed backend513c and513d. It does not move original archive files, active runs or frozen helpers.

The tool is integrated. Seven ordinary scratch tests and one actual namespace regression pass. Strict example Clippy passes. It has not copied, published or deleted real run data. The complete external copy and post-publication state still need independent review before real retirement.

It preserves bytes, mode, owner, group and **modification times**. It does not preserve access times. Reading source files can change their access times. New copies have new inode numbers and change times. Extended attributes or unexpected ownership cause an error.

The five phases are separate:

- `copy` checks the fixed inventory under the source store lease. It creates a new external directory and copies every file. It writes durable intent and completion records.
- `verify` checks both trees. After retirement, it checks the retained destination and the retirement receipt.
- `publish` rechecks both copies under both leases. An atomic exchange replaces the workspace run directory with a link to its external copy. The complete original directory remains at the recorded local backup path.
- `rollback` exchanges the workspace link and intact backup directory. It also works if the exchange succeeded before the publication completion record was written. Both copies must still match the receipt.
- `retire` is a separate action after post-publication review. It rechecks both copies and removes only the exact local backup files and empty directories. It preserves the destination and the workspace link.

Every phase has a create-new receipt. A partial copy or retirement remains explicit. The tool does not guess how to resume an incomplete phase. After retirement, rollback requires a new separately verified copy transaction; the current rollback command needs the intact local backup.

The external topology is fixed to the reviewed visible ext4 device8:33. A changed mount is an error. Store roots, object directories and final objects remain real nodes. There are no CAS hardlinks. Only the top run path becomes a symbolic link. The old strict reclaimer rejects the resulting external canonical paths and must not be used through these links.

The namespace regression executed captured `cat` in the existing isolated runtime through the scratch parent-link topology. It is not a firmware test. No release executable was built for actual relocation.

These commands passed from the repository root:

```sh
cargo test -p md-sim-firmware --example relocate_runtime_run --offline --locked
cargo clippy -p md-sim-firmware --example relocate_runtime_run --offline --locked -- -D warnings
cargo test -p md-sim-firmware --example relocate_runtime_run --offline --locked tests::isolated_read_uses_real_store_beneath_published_run_symlink -- --exact --ignored --nocapture --test-threads=1
```

The complete small check outputs and exact source identities are in [the check receipt](runtime-relocation-checks-517.json). The production source is [relocate_runtime_run.rs](../../../crates/md-sim-firmware/examples/relocate_runtime_run.rs). It contains all required tests. The ignored development-stage copy is redundant and is not required to build from Git.

The coordinator must keep the two selected runs idle during each mutation. The tool does not defend against a concurrent malicious owner or root process that changes ancestor paths. All actual changes must use the frozen reviewed binary and preserve each phase receipt.

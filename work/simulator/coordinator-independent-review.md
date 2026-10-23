# Independent coordinator behavior review

This review covers the actual isolated process queue and private worker transport. It does not establish firmware, exact hardware, external host, SSH or TLS coverage.

The test file is `crates/md-sim-coordinator/tests/queue_behavior.rs`. It uses the installed shell and its exact library closure. The process runs under the shared namespace and resource-limit contract. It checks that host user files and `/etc/passwd` are absent. It does not use a service imitation.

Verified commands:

```sh
cargo test -p md-sim-coordinator --test queue_behavior --offline --locked -- --test-threads=1
cargo clippy -p md-sim-coordinator --test queue_behavior --offline --locked -- -D warnings
```

The latest run passed eight tests in 46.16 seconds. Strict lint checks passed.

The tests cover:

- Reuse of identical retained evidence. Changed evidence forces another actual execution and keeps the changed old file.
- Rejection of a passing gate that refers to a missing file. Bounded exports reject an excessive file before writing it.
- Recovery of a directory created before its initialization lock. The old attempt stays explicitly interrupted.
- Actual artifact transfer, worker execution, result retrieval, acknowledged cleanup and resume after worker restart.
- Seed, hash cap, clock, CPU budget and suite changes alter execution identity.
- Local and worker cancellation after real child output is observed. A cancelled row before process start cannot satisfy the test.
- Attempt-count and byte-budget admission before another attempt is created. Previous result hashes remain unchanged.
- Lost real worker replies after execution, cleanup and result transfer. Recovery uses the persisted token when the run-ID record is absent. It checks the cleanup receipt against the saved bytes. Two interrupted transfers keep distinct partial files.

The first cancellation test used a fixed delay. It failed when worker artifact transfer had not finished before the delay. The replacement waits for actual bounded child output. This strengthens the assertion; it does not turn an unstarted child into a passing cancellation test.

Source review confirmed the earlier fixes for gate references, bounded exports, interrupted initialization, durable submission tokens and repeated partial-file retention. One new finding required a directory sync after the retrieved result file was synced and before the worker received destructive cleanup. The author added `File::open(out)?.sync_all()?` at that boundary. Independent source reinspection confirms the fix. File sync alone did not make the new directory entry durable. No scoped correctness finding remains.

The worker token review found no new scoped correctness issue. A reserved token survives restart; another process specification cannot reuse it. An uncommitted specification remains lost and does not launch. Cleanup checks the exact acknowledged result and retains a bounded token receipt. The receipt is committed before removal, and restart completes only acknowledged cleanup. Token receipts consume the declared retained-run count.

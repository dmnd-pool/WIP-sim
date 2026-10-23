# Component input cleanup444

The narrow cleanup extension passes its focused checks and independent
review. The actual cleanup removed 94 duplicate CAS objects, or
114,348,280 bytes. All remaining run files and all restore copies were
hashed again. The postcheck passes.

| Finished run | Duplicate objects removed | Bytes removed |
| --- | ---: | ---: |
| byte-stream-399 | 33 | 39,501,688 |
| worker-timeout-406 | 27 | 36,570,856 |
| tls-readiness-438 | 34 | 38,275,736 |
| tls-readiness-435 | 0 | 0 |
| tls-readiness-437 | 0 | 0 |

Failed435 and437 do not have the required assessment identities. They remain
ineligible and unchanged. Their full file sets were hashed before and after
cleanup. All unique inputs, captured helpers, logs, results, manifests and
adjacent derived archives remain unchanged. Original downloaded archives
were outside the operation. Frozen440 and native441 were not changed.

The only source change is
`crates/md-sim-firmware/examples/reclaim_qemu_inputs.rs`, SHA-256
`fce6f9ad044d23a8754e4ea526d9e2b37b7cbe2204a9aa72ab072ffb002d7938`,
52,322 bytes. The executed utility is
`work/simulator/check-target-085/debug/examples/reclaim_qemu_inputs`, SHA-256
`123ce5eacd668bd3da5c181a02e6ac0c2ba391d1c62a3057b1132890870b01c6`,
2,400,576 bytes. The delta is separate from frozen440.

## Checks

The ordinary example suite passes nine tests. One test explicitly requires
a separately built CLI. That test then ran separately and passed one test
with zero ignored. It ran the real command in a new temporary repository.
It checked active Store lock refusal, exact duplicate deletion, retained
unique inputs and evidence, a complete restore journal, and restoration by
copy. Each child command had a five-second deadline. Strict example Clippy
and scoped formatting also pass.

These commands were executed from the repository root. Cargo used
`CARGO_TARGET_DIR=work/simulator/check-target-085`, `CARGO_BUILD_JOBS=1` and
`TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227`.

```bash
cargo test -p md-sim-firmware --example reclaim_qemu_inputs --offline --locked
cargo build -p md-sim-firmware --example reclaim_qemu_inputs --offline --locked
MD_SIM_RECLAIM_EXECUTABLE=/home/user/src/md-sim/work/simulator/check-target-085/debug/examples/reclaim_qemu_inputs \
  cargo test -p md-sim-firmware --example reclaim_qemu_inputs --offline --locked \
  tests::actual_cli_preserves_evidence_and_unique_inputs_and_restores_the_duplicate \
  -- --exact --ignored --nocapture
cargo clippy -p md-sim-firmware --example reclaim_qemu_inputs --offline --locked -- -D warnings
rustfmt --edition 2021 --check crates/md-sim-firmware/examples/reclaim_qemu_inputs.rs
```

Logs are in `work/simulator/checks-2026-10-21-001/`:

- `component-cleanup444-tests.log`
- `component-cleanup444-build.log`
- `component-cleanup444-cli-test.log`
- `component-cleanup444-clippy.log`
- `component-cleanup444-fmt.log`

The independent review is
`component-cleanup-source-independent-441.md`, with its444 addendum and
exact input manifest. No correctness finding remains in this scope.

## Actual cleanup and restoration evidence

The preflight held exclusive nonblocking locks on every target and
canonical Store. It saved all run-file hashes before any deletion. The
first read-only attempt stopped because Cargo's executable has a normal
build-output hard link. That diagnostic is retained. The second preflight
allowed this link only when hashing the executable; run-file checks stayed
strict. Neither preflight deleted a file.

The tested utility then ran twice. Each command held its own exclusive
Store locks, checked exact terminal evidence, verified both copies and
wrote a synced restore journal before deletion.

```bash
work/simulator/check-target-085/debug/examples/reclaim_qemu_inputs \
  work/simulator/network-vm/native-worker-channel-342/input-store \
  work/simulator/reviews/component-cleanup-444/reclaim-from-342.json \
  work/simulator/network-vm/byte-stream-399/input-store \
  work/simulator/network-vm/worker-timeout-406/input-store \
  work/simulator/network-vm/tls-readiness-438/input-store

work/simulator/check-target-085/debug/examples/reclaim_qemu_inputs \
  work/simulator/network-vm/native-ssh-channel-375/client-image/input-store \
  work/simulator/reviews/component-cleanup-444/reclaim-from-375-client.json \
  work/simulator/network-vm/byte-stream-399/input-store \
  work/simulator/network-vm/worker-timeout-406/input-store \
  work/simulator/network-vm/tls-readiness-438/input-store
```

These journal paths now exist and must not be reused. Their canonical
Stores remain protected. The first command removed43 objects and82,612,016
bytes. The second removed51 objects and31,736,264 bytes.

The durable records are in
`work/simulator/reviews/component-cleanup-444/`:

- `preflight.json`: exact before-state and duplicate plan.
- `reclaim-from-342.json`: first completed restore journal.
- `reclaim-from-375-client.json`: second completed restore journal.
- `postcheck.json`: all94 deletions match the plan; all94 restore copies
  retain the exact bytes; all remaining files in the five inspected runs
  match their before-state.

To replay the old component runs, first restore their removed input objects
from the journals. Hold the corresponding Store locks. Check each recorded
restore hash and size, copy `restore_copy_from` to the absent `path`, then
verify the copied hash and size. Do not use hard links. The scratch CLI
regression exercised this restoration contract. The actual removed copies
remain absent to retain the space saving.

This is cleanup evidence. It adds no firmware, dashboard, external-host or
full-matrix acceptance claim.

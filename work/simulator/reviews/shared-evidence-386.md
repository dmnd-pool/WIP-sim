# External pool evidence checkpoint 386

Date: 2026-10-23. The evidence join and explicit shared audit commands are
implemented. Affected tests, strict Clippy and release builds pass. The first
actual shared-pool firmware run remains an integration gate. This checkpoint
does not establish a new mining result, exact target or completed simulator.

## Implemented path

Prepared board freezing retains the exact `shared-pool.json` bytes. The file
must match its executed mount and carrier policy. The native worker result,
backend log and guest log keep their original bytes.

The separate pool join checks the native request token, selected run ID,
complete locked process identity, host and worker receipts, joined receipt,
pool bundle identity, exact manifest and pool input mounts, and support
component identities. It checks the actual outer process mounts and prepared
support image, scenario and root manifest. Pool stdout, stderr and session
files must match the saved byte envelopes and their hashes. Session result
traces must equal the separate raw session files.

The pool lease must identify the same run and helper. Its actual readiness
record must precede Execute within the frozen readiness budget. Strict audit
mode requires complete, uncancelled collection and successful native, pool
and outer processes. It checks the provider receipt against the one actual
host record in the outer result. The configuration, worker build, process,
support identity, carrier policies and bidirectional traffic must agree.

The existing SHA-256d verifier and ASIC serial correlation remain in use.
The shared regtest audit produces one candidate from each distinct fresh job.
Separate Bitcoin Core execution must still accept those candidates.

The explicit audit commands are `submissions-shared`, `regtest-shared`,
`regtest-session-shared` and `rates-shared`. They use the new external join.
Existing default commands retain their original runner-log path. These new
commands still need execution against the first actual shared run.

The parent importer owns pre-acknowledgement retention. It saves and verifies
both native and pool bytes before acknowledgement. The final firmware report
runs after the worker receipt, provider receipt and outer result are available.
Retention can preserve valid failed executions. It does not assert mining
success. No external pool line is inserted into the native guest log.

## Historical regression and checks

The final compiled audit ran this command against unchanged retained data:

```sh
work/simulator/check-target-085/release/examples/s9_asic_audit \
  regression-pool-split \
  work/simulator/network-vm/native-worker-firmware-354/execution \
  work/simulator/reports/historical354-pool-split-386.json
```

The [historical result](../reports/historical354-pool-split-386.json) verifies
1,322 computed hashes, 14 serial results, two original jobs, the clean-job
transition and difficulty change. The pool input is parsed separately from
the ASIC trace. The command requires the exact original result hash. This is
a historical parser and correlation regression. It is not fresh work at
regression time or execution of the new shared topology.

The new 23,884-byte fixture
`crates/md-sim-firmware/tests/fixtures/s9-pool-354-session0.json` retains actual
session0 data and parent hashes. Its SHA-256 is
`1083d5d33da610a07818da2a429491fe673ba41d129557d069deab8552ffa0e7`.
Tests reject changed pool bytes, digests, tokens, run IDs, process identities
and lock hashes. Other tests reject failed or truncated outer execution and
shared metadata that differs from its captured mount or policy.

Commands used `CARGO_INCREMENTAL=0`, the single active target directory
`work/simulator/check-target-085`, and
`TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227`.

```sh
cargo test -p md-sim-firmware --example s9_asic_audit --example worker_channel_probe --offline --locked
cargo clippy -p md-sim-firmware --example s9_asic_audit --example s9_boot_bundle --example s9_execute_prepared --example s9_retain_evidence --example worker_channel_probe --offline --locked -- -D warnings
cargo build -p md-sim-firmware --example s9_asic_audit --example s9_boot_bundle --example s9_execute_prepared --example s9_retain_evidence --release --offline --locked
```

The [final test log](../checks-2026-10-21-001/shared-evidence386-tests.log) has
50 passed audit tests and 48 passed worker probe tests. Their ignored descriptor
helper entries run as actual children. [Strict Clippy](../checks-2026-10-21-001/shared-evidence386-clippy.log)
and [release builds](../checks-2026-10-21-001/shared-evidence386-build.log) pass.
Affected formatting and guide links pass.

Earlier failures remain in `shared-evidence372-clippy.log` and
`shared-evidence385-clippy.log`. Duplicate module inclusion was removed by
using one shared root module per example. A displaced existing module attribute
was restored. The concurrent namespace helper compilation defects were sent
to its owner. No failed check was relabelled as a pass.

The [independent review](external-pool-independent-private-channel-384.md)
found missing strict outer-success and provider links. Both are fixed and the
affected tests were rerun. No correctness finding remains in that review's
scope. Its [final identity manifest](external-pool-independent-private-channel-386-inputs.json)
records the reviewed sources and checks.

## Current source identity and next action

| Source | SHA-256 |
| --- | --- |
| `support/s9_external_pool.rs` | `ddf6362c881515f4c6a5cf0e639c50bc938b4fb483e7667aaa31a246899fe48a` |
| `support/prepared_board.rs` | `29940270e3943330637ce779ae9e8fd6c1ab2806aab4dd1b6549d852634e547e` |
| `support/s9_execution_report.rs` | `0e9205fc151ec484e3913f4299024a49ec8f2fc060e75e635c03f075fcd07c2f` |
| `support/s9_submission_audit.rs` | `90737d07d45b5fba85ef1bcdb0c44427a54d4429824ee87de3e594e616ee046c` |
| `s9_asic_audit.rs` | `8d2ed2172845ee614cc2d906c80e3595fcb8514a083248db7213a4788c341eda` |

Paths in the table are relative to `crates/md-sim-firmware/examples/`.
Root module declarations also changed in `s9_boot_bundle.rs`,
`s9_execute_prepared.rs` and `s9_retain_evidence.rs` to reuse the common contract.

Next action: run the first frozen shared topology with the real archived
firmware. Preserve the selected run, complete pool bundle, provider and outer
receipts, scenario and prepared support inputs. Execute the explicit shared
audit and both Core candidate checks. Then test the same run's native dashboard
through the separate SSH/TLS client. All these integration gates remain open.

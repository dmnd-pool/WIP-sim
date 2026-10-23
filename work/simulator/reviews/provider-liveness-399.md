# Owning provider liveness checkpoint 399

The worker now answers `PrivateChannelStatus { token, challenge }` through its
existing control protocol. This is a read-only operation. Its response uses
`md-sim-carrier-provider-lease/1`. It includes the exact locked run, process,
dashboard policy, support input, provider configuration, execution token,
run ID, echoed challenge, increasing sequence, and observed provider state.

The token and challenge must each be 64 lowercase hexadecimal characters.
The token must name the already committed selected run and process. The
binding survives consumption of the one-use endpoint and result cleanup.
No process-state response is used to create provider liveness.

The actual provider thread owns the state guard. It starts in `waiting`, enters
`active` after the runtime hands it the real endpoint, and reports `closed` or
`failed` after its real relay ends. Unfinished exit and thread panic set
`failed`. A panic is also retained in the provider report before its root lock
is released. The root remains leased through worker service and provider join.

Provider configuration schema 1 accepts an optional field:

```text
lease: { locked_run: <complete LockedRun>, policy: <ObjectId> }
```

The full locked process must equal the configured process. The selected policy
must be present in the lock inputs and `policies.carrier_dashboard`. The
configuration remains outside that inner lock. Schema 1 without this optional
field keeps its old execution path. Its liveness query fails as unconfigured.

## Actual CLI tests

`channel398-provider-cli.log` retains the complete evidence from five real
worker CLI executions with isolated test guests and owned socket peers:

| Case | Actual provider result | Observed provider states |
| --- | --- | --- |
| Legacy completion | `guest-eof` | Lease query refused |
| Legacy cancellation | `guest-eof` | Lease query refused |
| Leased completion | `guest-eof` | waiting, active, closed |
| Peer closed while guest was alive | `peer-eof` | waiting, active, closed |
| Malformed peer frame while guest was alive | `failed` | waiting, active, failed |

Both failure-injection cases explicitly check that the worker still reports
the guest as active after the provider reaches its terminal state. They then
stop that same guest. Each case uses two fresh challenges and checks the real
guest's computed SHA-256 responses. These are transport tests, not mining.
The invalid frame produces a failed provider report and a nonzero CLI exit.

The tests compare every identity and echoed challenge, require sequence growth,
reject a wrong token and malformed challenges, retrieve exact result bytes,
check cleanup retention, and verify that unrelated host descriptors and files
do not enter the isolated guest. The same provider observation remains closed
or failed after cleanup.

The first run, `channel397-provider-cli.log`, passed the legacy and leased
completion cases but could not use `shutdown(2)` in the test environment. The
fixed test closes its sole owned peer file to create real EOF. It does not
replace EOF with a fabricated state or weaken the assertions.

## Checks

- `channel397-provider-unit.log`: 2 runtime state tests passed. One causes an
  actual provider-thread panic and checks its terminal state.
- `channel398-provider-cli.log`: the integration test passed all five cases;
  one ignored entry is its isolated guest executable.
- `channel398-provider-runtime-regression.log`: 6 passed, 3 ignored helper
  entries. This covers existing private-channel isolation and worker behavior.
- `channel398-provider-retry-tests.log`: 8 passed. A real interrupted pipe
  exchange retries the exact same token and challenge bytes only once.
- `channel399-provider-config-tests.log`: 6 passed, including full-lock
  validation, actual relay timeout, and retained thread-panic evidence.
- `channel399-provider-clippy.log`: strict coordinator Clippy passed for all
  targets, including the changed runtime dependency.

Commands use `CARGO_TARGET_DIR=work/simulator/check-target-085`,
`CARGO_BUILD_JOBS=1`, `--offline`, and `--locked`:

```sh
cargo test -p md-sim-runtime worker::channel --lib --offline --locked
cargo test -p md-sim-coordinator --test worker_carrier --offline --locked -- --nocapture
cargo test -p md-sim-runtime --test private_channel --offline --locked
cargo test -p md-sim-coordinator --lib ssh_transport::tests --offline --locked
cargo test -p md-sim-coordinator --bin md-sim-worker-ipc --offline --locked
cargo clippy -p md-sim-coordinator --all-targets --offline --locked -- -D warnings
```

This implements a liveness dependency for the selected native dashboard path.
It does not establish dashboard access, an external host test, firmware mining,
original-board boot, exact physical target coverage, or full completion.
The new path must be included in the next frozen integration run.

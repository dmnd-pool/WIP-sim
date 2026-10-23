# Completed worker errors

Implemented `CompletedWorkerError` in the runtime worker protocol. Its private
constructor is reached only after a complete newline frame has the exact
protocol, boolean `ok:false`, and a nonempty error string. The error string is
limited to 8,192 UTF-8 bytes and cannot contain NUL. Malformed, partial,
oversized and wrong-protocol replies do not receive this type.

`CompletedWorkerError::response()` returns the complete parsed JSON envelope.
A relay can forward all of its fields without adding error-context text. This
preserves the envelope's values; it does not promise the original JSON spacing.

The owned device and connected socket transports keep their descriptors after
this complete error. They still close after uncertain exchanges. The private
worker transport delegates to the connected implementation. No request is
automatically repeated.

Validation passed:

- [Runtime library tests](../checks-2026-10-21-001/completed-worker-error-runtime-all-tests.log): 26 tests.
- [Coordinator tests](../checks-2026-10-21-001/completed-worker-error-coordinator-tests.log): 18 library and 13 queue tests.
- [Real PTY tests](../checks-2026-10-21-001/completed-worker-error-pty-tests.log): 5 tests.
- [Runtime and coordinator strict Clippy](../checks-2026-10-21-001/completed-worker-error-clippy.log).
- [Firmware example strict Clippy](../checks-2026-10-21-001/completed-worker-error-pty-clippy.log).

The positive wire tests send a complete worker error, then verify a separate
Probe request and its reply on the same channel. The malformed-error tests and
the existing real five-second timeout/late-reply tests still close the channel.
The PTY and socket peer fixtures are protocol tests. They are not firmware or
mining acceptance results.

All commands used `CARGO_TARGET_DIR=work/simulator/check-target-085` and
`CARGO_BUILD_JOBS=1`:

```sh
cargo test -p md-sim-runtime --lib --offline --locked
cargo test -p md-sim-coordinator --all-targets --offline --locked
cargo test -p md-sim-firmware --example worker_channel_probe --offline --locked device_transport::tests -- --nocapture
cargo clippy -p md-sim-runtime -p md-sim-coordinator --all-targets --offline --locked -- -D warnings
cargo clippy -p md-sim-firmware --example worker_channel_probe --offline --locked -- -D warnings
```

Source SHA-256 at these checks:

| File | SHA-256 |
| --- | --- |
| `crates/md-sim-runtime/src/worker.rs` | `c3988db219f0552f2aae58ffcd0bef2583ab5a054d0a10a099390c020b316345` |
| `crates/md-sim-coordinator/src/executor.rs` | `7cfd10ed085dd6afc4df8a1584052b2f234a2273df695140b9de14714fefad7b` |
| `crates/md-sim-firmware/examples/support/worker_device_transport.rs` | `52506a0df6048482bdafd42e9d546bd90464ac8c1aabb8a2841e59f5cdb13a7d` |

The parent edited the relay forwarding in parallel. This author did not edit
those sections. The final format check still requested one formatting change
in the parent's relay-stream match arm. The final broker, full workspace and
firmware checks remain the parent's separate work.

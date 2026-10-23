# Shared S9 runner implementation checkpoint 366

Date: 2026-10-23. The explicit runner mode is implemented and its affected
checks pass. Actual shared-pool firmware, mining and dashboard integration is
still incomplete. This checkpoint does not establish an exact physical target.

## Change and identity

`s9_boot_bundle boot` now accepts `shared-pool=MANIFEST` with `local-pool`,
the real console script and `prepare-only`. It rejects an embedded pool VM,
nested TCG, private loopback and runner-owned dashboard tests in this mode.
The manifest and fresh pool input become exact captured ProcessSpec inputs.
The runtime's private carrier policy is set before preparation is frozen.
The strict manifest schema is in
[the integration contract](shared-pool-native-worker-366.md).

At execution entry, the runner owns socket descriptor 3 and protects it from
unrelated child programs. Only miner QEMU gets that descriptor. A small serial
API callback closes the runner copy immediately after successful spawn,
before any console read. Existing serial entry points use a no-op callback.
The shared mode starts neither existing embedded pool. It drains actual guest
output until the declared pool interval plus five seconds from attachment.
Its records describe attachment and observation only. The separate server must
prove pool readiness, execution, collection and mining results.

ASIC tracing, guest console execution and storage capture use the existing
paths. Original archive bytes are unchanged. The parent integration must retain
the external pool log and join it to the selected native run without changing
native result or guest log bytes. Its full support image belongs in the outer
lock to avoid a circular hash with the embedded miner lock.

| Checked source or build | SHA-256 |
| --- | --- |
| `crates/md-sim-firmware/examples/s9_boot_bundle.rs` | `45052da6414cd49012882ee7d9f6094195e84734452aed614522f8d11375c9a3` |
| `crates/md-sim-firmware/examples/support/s9_shared_pool.rs` | `acd9ffbe595b7ae8ca1a89e870dfd0b23ccb4a5fc90a3245223bca7e64a30809` |
| `crates/md-sim-firmware/src/serial_console.rs` | `beb442bbf8c193eb235af68b79345a7976d33efb4ad0bc93ac56ee9ed4c5a39b` |
| `work/simulator/check-target-085/release/examples/s9_boot_bundle` | `9ca950568572807dc1d9bb34ae1bf22ef3460263946c582195e5df576b25f019` |

The build directory is active and can change after this checkpoint. New
integration runs must capture their actual executable and source identities.

## Checks and fixes

All commands used `CARGO_INCREMENTAL=0`,
`CARGO_TARGET_DIR=work/simulator/check-target-085` and
`TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227`.

```sh
cargo test -p md-sim-firmware --example s9_boot_bundle --offline --locked shared_pool -- --nocapture
cargo test -p md-sim-firmware --lib --offline --locked serial_console::tests -- --nocapture
cargo clippy -p md-sim-firmware --lib --example s9_boot_bundle --offline --locked -- -D warnings
cargo build -p md-sim-firmware --example s9_boot_bundle --release --offline --locked
```

The shared mode has three passed outer tests. Two ignored helper entries run
as actual child executables. They prove that an unselected exec inherits
neither descriptor 3 nor the source descriptor. The selected child exchanges
real bytes and exposes EOF when it exits. The twelve existing serial tests,
including the new spawn-success/console-failure and spawn-failure callback
cases, pass. Affected Rust formatting, Clippy, release build and guide link
checks pass.

The first compile exceeded the existing large JSON macro's recursion bound.
The new adaptation assignment was separated instead of raising the limit.
The first descriptor test used a socket timeout call denied by the host. It
was changed to bounded POSIX polling and actual reads/writes. Both failures
remain in the checks directory as `shared-pool366-tests.log` and
`shared-pool366-tests-fixed.log`.

Independent review found two small issues. Parent loss now returns explicit
`ECANCELED`, because `getppid` does not set errno. The descriptor test now
executes an actual unselected child instead of relying only on the
close-on-exec flag. The reviewer confirmed both fixes and reported no open
finding in this runner and serial callback scope.

| Final evidence | SHA-256 |
| --- | --- |
| [Shared mode tests](../checks-2026-10-21-001/shared-pool366-review-fixed-tests.log) | `5e1ad7ae667a188480f9ae72e48664145ecc4880efd699b77cf253ee53980afe` |
| [Serial tests](../checks-2026-10-21-001/shared-pool366-serial-all.log) | `2b4fa0bfe909209a8d996d5dbdd1abca0c41b8174b278fc15f210d16e0543ded` |
| [Clippy](../checks-2026-10-21-001/shared-pool366-final-clippy.log) | `73dea57db0b25f56d61eea368d35a3a5a7e044b0188552b9799a74c628f4e147` |
| [Release build](../checks-2026-10-21-001/shared-pool366-final-build.log) | `dd8a01f3d7cfc812356fa2bb294774c1cdc0a0ff020fce009720ca72d631fe75` |

Next action: prepare the actual shared support manifest, freeze the miner and
outer support image, exercise the selected native worker through the isolated
SSH client, retain both evidence streams, run the independent mining audit
and Core tests, then test the selected native dashboard. None of those gates
is marked passed by this checkpoint.

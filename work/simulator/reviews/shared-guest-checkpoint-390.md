# Shared pool guest checkpoint 390

The shared pool namespace path is ready for its first full integration run.
This is a source and component checkpoint. It is not a firmware or mining pass.

The helper now prepares a selected locked process, moves the checked private
Ethernet device into the SSH server namespace, and starts the independent pool
only when that process reaches `ExecuteToken`. It forwards that request only
after it reads the actual pool readiness marker. The client uses the common
worker executor. Cleanup requires the joined native-result and pool-bundle
receipt from the outer host. The exact inner process time limit is unchanged.

The independent ownership review found two real defects before integration.
Both are fixed. The server work root stays root-owned. The broker writes only
in its own directory. Pool readiness uses a retained read descriptor for the
actual child's stdout. That descriptor has a separate file offset from the
writer. The bundle checks the original log device and inode. Cancellation is
staged in a root-only directory, changes ownership through the open descriptor,
and atomically replaces the destination entry. It does not follow a destination
symlink. Nonregular artifact files and symlinks are rejected without blocking.

Checks after the fixes:

- `channel390b-shared-clippy.log`: strict focused Clippy passed.
- `channel390b-shared-tests.log`: 79 passed, 2 ignored child helper entries.
  This includes real-file offset, inode, cancellation, symlink and FIFO checks,
  and real-socket joined receipt tests.
- Independent reread: `shared-guest-independent-private-channel-390.md`.

Commands used from the repository root:

```sh
CARGO_TARGET_DIR=work/simulator/check-target-085 CARGO_BUILD_JOBS=1 cargo clippy -p md-sim-firmware --example remote_namespace_check --offline --locked -- -D warnings
CARGO_TARGET_DIR=work/simulator/check-target-085 CARGO_BUILD_JOBS=1 cargo test -p md-sim-firmware --example remote_namespace_check --offline --locked
```

Source SHA-256 values at this checkpoint:

| File under `crates/md-sim-firmware/examples/` | SHA-256 |
| --- | --- |
| `remote_namespace_check.rs` | `49672957373656bfc7db106a5c8b91c5957697d6beb63ef3ac3d6395bb493154` |
| `remote_namespace_check/worker_broker.rs` | `08fdb87a92d0e47cfbff628e7c3b6b3c257d3672cd123136cf7c63b12e3b02ac` |
| `remote_namespace_check/shared_pool.rs` | `50a77a83472b75987d2d94c37deeee76185e8c95dda068db491e86b0d9dc2887` |
| `remote_namespace_check/pool_service.rs` | `e4d6887228e56e0d08af796eafc727aebea55e046119bbe74fdb69b043851520` |
| `remote_namespace_check/shared_run.rs` | `18831fd5588d1422998c1d38d929c1fd5d3057a8e559cdf2df537854c64c3c54` |
| `support/worker_shared_retention.rs` | `039e69479fdfc8a79c523abcbf50b45cb43cdadc06f52a55b4caa18dbf13cf5d` |
| `support/worker_retention_frame.rs` | `e636099ecd745d598eca038733f0f859f7b9d05906c518a47e85a5d1a9e58629` |

Next action: freeze these sources and run the real shared-pool firmware path.
Retain every failed run. Validate the joined artifacts with the independent
mining audit and Bitcoin Core. A source check cannot establish that path.
Full VM-loss recovery, the selected native dashboard through SSH and TLS,
original-board boot, exact physical targets, and the full matrix remain open.

# Carrier input and deadline checkpoint 406

`carrier_inputs::fetch` now obtains small captured inputs through the actual
worker `Get` operation. It has no worker-store path assumption. It checks the
object identity, offset, exact byte count, canonical hex, final EOF, complete
SHA-256 and size before publishing a new file. The maximum input is 4 MiB.
The destination directory must be private and owned by the current account.
Publication does not overwrite an existing file or link. Failed transfers
remove their owned staging file. The CLI token parser accepts only canonical
lowercase `SHA256:SIZE` values.

`worker::request_with_timeout` supplies one absolute deadline for an entire
request. The deadline covers bounded serialization, nonblocking Unix socket
connect, writes, response reads, and parsing. A full AF_UNIX backlog fails
immediately. A slow peer cannot restart the clock by sending partial bytes.
The existing worker envelope decoder is reused. Existing request functions are
unchanged. Each call owns and closes a new connection.

## Checks

- `channel403-carrier-input-tests.log`: 3 passed. This includes a real worker
  service over an owned socket pair, two chunks, an explicit EOF request, and
  exact retained bytes. Negative cases cover identity, offset, hex, length,
  surplus data, hash, extra fields, permissions, and existing files or links.
- `channel403-carrier-input-clippy.log`: strict access lint passed.
- `channel405-worker-timeout-tests.log`: the real socket-pair slow-reply test
  passed. Three listener tests are explicit VM helper entries.
- `channel405-worker-timeout-clippy.log`: strict runtime lint passed.
- `network-vm/worker-timeout-406/guest.log`: all three public-API listener tests
  passed in the captured private VM. The log records 3 passed and 0 failed,
  followed by the actual scenario exit code 0. These tests cover complete
  success/error envelopes, a slow partial reply, and a full Unix backlog.

The VM used the frozen `channel-target-390/release/examples/isolated_network_vm`
helper and the exact captured Rust test executable. It had only loopback and
no host network attachment. Its measured execution took 4,789 ms. This is a
local VM transport test. It does not establish an external-host, dashboard,
firmware, mining, or exact-target gate.

Source identities:

| Source | SHA-256 |
| --- | --- |
| `crates/md-sim-access/src/carrier_inputs.rs` | `5e6a7c64c3bffe730d92de859a97e5b674371fbb6c864d3d574bbf982b571c5b` |
| `crates/md-sim-runtime/src/worker_timeout.rs` | `83b14b1ec0bdd95534097c06992cf7965265a009295d7481ab89f77cee9bffb3` |

The captured binary and support-helper identities are in
`network-vm/worker-timeout-inputs-406/inputs.json`. The guest log is 1,057 bytes,
SHA-256 `14f70589505c75ab44fc3f934239d80befc1c1603fbe9260408d83a05c4c0d08`.

Commands, from the repository root:

```sh
export CARGO_TARGET_DIR=work/simulator/check-target-085
export CARGO_BUILD_JOBS=1
cargo test -p md-sim-access carrier_inputs::tests --lib --offline --locked
cargo clippy -p md-sim-access --lib --tests --offline --locked -- -D warnings
cargo test -p md-sim-runtime worker::timeout --lib --offline --locked
cargo clippy -p md-sim-runtime --lib --tests --offline --locked -- -D warnings
work/simulator/channel-target-390/release/examples/isolated_network_vm work/simulator/network-vm/worker-timeout-406 work/simulator/network-vm/worker-timeout-inputs-406/scenario.json
```

The VM output directory must be new when repeating that command. The retained
scenario selects the three ignored helper entries in the captured test binary.

Earlier failures remain explicit. Tests 401 and 402 exposed a fixture Store
policy mismatch; the real worker rejected that setup. The fixture now uses the
same persisted Store policy as that worker. Listener tests 404 received host
sandbox `EPERM`; they were then executed in VM 406 with their assertions and
deadlines unchanged. The passing socket-pair test does not replace those VM
listener results.

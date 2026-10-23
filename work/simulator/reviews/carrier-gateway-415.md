# Carrier SSH and TLS access checkpoint 415

The new access path is implemented. Its focused tests and strict lint pass.
The final independent review by `private_channel` has no open finding in this
component scope. The next gate is the actual selected
native firmware run with SSH, mutual TLS, and native HTTP checks from the
separate client. These component results do not establish that gate.

## Selection and transfer

`remote_carrier::RemoteCarrierGateway::inspect` sends the exact lock, policy,
and provider objects through the worker protocol. The server obtains those
objects with bounded Get requests and verifies their sizes and hashes. The
server has no host worker Store mount. Publication transfers the gateway
specification through the same path. Client and server retain exact input IDs.

Inspection requires actual LookupToken, Status, and owning-provider replies.
It waits at most ten seconds for the explicitly selected atomic ready record.
The ready record must bind the full identity, provider configuration, endpoint
path and exact endpoint bytes. An active provider is required for authorization.
A correctly bound waiting provider never creates authorization. An old runtime
dashboard Binding is neither created nor inserted into worker status.

The TLS gateway uses the existing gateway implementation and the exact owned
Unix socket of the carrier bridge. Startup and every poll check the same
endpoint bytes, socket inode, ready bytes, selected worker, and fresh increasing
provider lease. Control requests use an absolute deadline. Failed checks stop
the gateway. The bridge also enforces its own lease and byte limits.

The client verifies the returned run, full execution identity, loopback and
Unix upstreams, native source label, browser policy, client rules, and isolated
browser context. Listener readiness never becomes a firmware or dashboard pass.
Original native response bodies still pass through the existing transport.

## Lifecycle and resource checks

The new SSH service uses the existing `OwnedSshChild` guard. It observes exit
with `waitid(WNOWAIT)`. This keeps the leader PID reserved until its private
group is stopped and the leader is reaped. The guarded public constructor is
the only new entry point. No arbitrary Child can be adopted through that API.

Access selects 60 CPU seconds and 1 MiB per retained log. The shared guard also
limits address space to 512 MiB, descriptors to 256, and core files to zero.
Startup has a 30-second client deadline. Shutdown has a 15-second deadline.
The guard sets parent-death signaling and prevents privilege escalation.
The caller retains all failure logs within these limits.

The connected CLI monitors stdin before it retrieves metadata. It checks
connection loss before and after each bounded request, during readiness waits,
and before and after gateway startup. A disconnected startup is not published.

## Checks and fixes

`carrier-gateway415-tests.log` records 22 passed tests and one explicit VM-only
carrier fixture entry. That fixture passed in the earlier isolated VM 399.
It is not silently counted as a host test. Strict access Clippy and scoped
Rust formatting also pass after the last fix.

New regressions reject changed policy, process, ready record, endpoint,
upstream, browser context, client rules, and readiness claims. A cancelled
start does not contact control or create an output directory. A real child
exceeds its output allowance and stops at the 1 MiB file bound. Another real
child leaves a live descendant after the leader exits. Cleanup stops that
descendant while the leader PID is still reserved.

Independent review found missing SSH child limits, late connection-loss
handling, and incomplete cleanup after leader exit. Those findings caused the
fixes above. The initial CLI compile failure remains in log 409. The redundant
wrapper caught by Clippy remains in log 413. Neither failed check is a pass.
Actual SSH/TLS/native integration remains separate from these unit regressions.

Checks were run from the repository root with:

```sh
export CARGO_INCREMENTAL=0
export CARGO_TARGET_DIR=work/simulator/check-target-085
export TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227
cargo test -p md-sim-access --all-targets --offline --locked
cargo clippy -p md-sim-access --all-targets --offline --locked -- -D warnings
```

The new CLI forms are implemented. They still require the frozen integration
run before they can be listed as verified native-dashboard commands:

```text
md-sim-access inspect-carrier LOCK_ID POLICY_ID PROVIDER_ID TOKEN SOCKET READY ENDPOINT NEW_OUTPUT
md-sim-access serve-carrier-connected SPEC_ID LOCK_ID POLICY_ID PROVIDER_ID TOKEN SOCKET READY ENDPOINT NEW_OUTPUT
```

Each ID uses canonical `SHA256:SIZE`. Each output directory must be new.
The selected support topology uses `/worker/worker.sock`,
`/work/access/ready.json`, and `/work/access/bridge/endpoint.json`.
Inspection and publication use separate new output directories.

## Tested source and log identities

| File | SHA-256 |
| --- | --- |
| `crates/md-sim-access/src/carrier.rs` | `e7fe74f1c80f3cc6621be04d505967883c17e1a23c4ebca19f7f93794d51b6b6` |
| `crates/md-sim-access/src/carrier_tests.rs` | `c9c3d33228412eac166205631d9268f4fb06ba8d4ced1eca9a2f0773f46e39b1` |
| `crates/md-sim-access/src/carrier_gateway.rs` | `5cc2b1cb9ff2bb9540d0901e3f7b4ba96d5e70179961857d14a0c894a179bba5` |
| `crates/md-sim-access/src/carrier_gateway_tests.rs` | `cb4f6270738445f74ab04ef2e85c79758c577f08e43c57514c5aadbc2d8daafb` |
| `crates/md-sim-access/src/remote_carrier.rs` | `9d858072e69972c491fe00ab1517221f5f83420ebf5dafb8093e8d81ef4eba4c` |
| `crates/md-sim-access/src/main.rs` | `6ca33692ff4669310c723141fd74d0efbfb1f225acbfab12cb536b299a23c241` |
| `crates/md-sim-access/src/lib.rs` | `954c4f6feec57da15faae752e75da58fc8c8d9dd483f57d9594587cf2f3f1fba` |
| `crates/md-sim-coordinator/src/ssh_transport.rs` | `bd5823c8afa2ffd97d3443f6944a6b2ffd43c3f07e503a5dfeeb69413b703b2c` |
| `crates/md-sim-coordinator/src/lib.rs` | `22f83e7853d7acda05d436d3877b73b20d6b711d0763ba2a7e12e4ab536020c6` |
| `checks-2026-10-21-001/carrier-gateway415-tests.log` | `531d860a4645c6524a5d9e656c994e4cb1d6a4ffa2ef0a9a727313616b2ac190` |
| `checks-2026-10-21-001/carrier-gateway415-clippy.log` | `1b8415d3ebda0e48582e9f4f9da58f69ea78b187c4e61eaf6b6e55193ce09f52` |
| `checks-2026-10-21-001/carrier-gateway415-fmt.log` | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

Log paths in the table are relative to `work/simulator/`. The helper dependency
review is in `carrier-input-timeout-independent-410.md`. Original archives and
the frozen firmware 390/391 execution inputs were not changed by this work.

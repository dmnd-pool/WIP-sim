# Exact worker carrier binding checkpoint370

This extends the [private carrier primitive](private-channel-365.md). It does
not establish a firmware, mining, dashboard, SSH, TLS, or external host gate.

The trusted caller creates `worker::PrivateChannelBinding` from the exact
canonical `ProcessSpec` identity and a bounded `SyncSender<File>`. Use
`worker::process_identity` for this hash. `serve_connections_with_channel`
uses the same incoming stream iterator and worker limits as the default
worker. The request wire cannot install or replace a binding.

The worker rejects an unconfigured, changed, or consumed channel before it
creates a run. It consumes the matching binding after input verification and
budget admission, before run creation. A storage or launch failure does not
restore that binding. The lock records its exact identity and consumption.
Launch-error results also state that the binding was consumed. A matching
execution token still returns the accepted run after execution, cleanup, and
restart. It does not launch another process.

`isolation::execute_private_channel_recorded` preserves the worker's cgroup,
dashboard, trace, and cleanup receipt paths. `private_channel::relay_provider`
lets trusted host code relay two owned socket endpoints with the same packet
checks. It checks socket types, enables nonblocking I/O, and applies explicit
wall-time and cancellation limits. The caller must establish endpoint origin,
peer identity, and topology. These provider descriptors do not enter a guest.

Run from the repository root:

```sh
CARGO_TARGET_DIR=work/simulator/check-target-085 TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 cargo test -p md-sim-runtime --test private_channel --offline --locked -- --nocapture
CARGO_TARGET_DIR=work/simulator/check-target-085 TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 cargo test -p md-sim-runtime --all-targets --offline --locked
CARGO_TARGET_DIR=work/simulator/check-target-085 TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 cargo clippy -p md-sim-runtime --all-targets --offline --locked -- -D warnings
```

The [detailed evidence](../checks-2026-10-21-001/private-channel370-evidence.log)
passes five host integration tests. The three ignored harness entries are
explicitly invoked guest/provider programs. The worker tests use real connected
IPC streams and isolated processes. They check these paths:

- Default rejection leaves zero runs. Changed byte caps and input hashes also
  leave zero runs and preserve the unused provider.
- One accepted process exchanges real framed traffic in both directions. The
  lock and retrieved result contain the exact binding and channel evidence.
- Complete result bytes are retrieved and independently hashed before cleanup.
  Cleanup retains input objects and the execution token receipt.
- A repeated token returns the same run. A new execution after consumption is
  rejected. Token lookup still works after cleanup and an unconfigured restart.
- Stop cancels a live isolated process while the provider endpoint stays open.
  Cleanup removes only the acknowledged run evidence.
- A disconnected provider causes a recorded launch failure. The binding stays
  consumed. The failed run can be retrieved and cleaned up.
- The trusted provider relay transfers both directions, refuses regular files,
  enforces packet bounds, and stops on its own deadline or cancellation.

The relay result now includes optional kernel CPU accounting for that relay
thread. This avoids charging concurrent test work to the backpressure test.
The same 400,000 microsecond assertion remains. Run370 measured 9,134
microseconds while a closed source had one complete frame pending toward a
blocked destination. Full supervisor SIGKILL and host descriptor checks also
passed again.

The [runtime suite](../checks-2026-10-21-001/private-channel369-tests.log)
passes 53 tests. [Runtime Clippy](../checks-2026-10-21-001/private-channel369-clippy.log)
passes with warnings denied. The [workspace compile check](../checks-2026-10-21-001/private-channel370-workspace-check.log)
passes. It records an unrelated unused import in the concurrent shared-pool
work; the parent must rerun its workspace lint gate after that work settles.

The first worker test run correctly failed because the test retained the store
writer lease. The test now uses matching persisted limits and releases the
preparation lease before starting the worker. No runtime assertion was relaxed.
The retained failure is [run367](../checks-2026-10-21-001/private-channel367-worker.log).
Its abandoned test stores were removed after ownership and age checks. No
archive, release binary, shared build cache, or canonical input was removed.

The primitive has an [independent review](private-carrier-independent-365.md).
The [worker/provider delta review](private-carrier-worker-independent-368.md)
and its owned-root lease follow-up have no open correctness findings. Parent integration must still
exercise the actual coordinator IPC command, support VM peer, native firmware,
and selected remote dashboard path. The exact-target completion gates remain
open.

Follow-up371 adds `WorkerRootLease::acquire`, its canonical `path` accessor,
and `serve_connections_with_channel_owned`. The provider can retain one root
lease through startup, service, and final evidence publication. The owned
service borrows the lease. Its return does not release it. A real concurrent
normal worker is refused before and after service return. It can start only
after the caller drops the lease. The [detailed follow-up](../checks-2026-10-21-001/private-channel371-lease.log)
passes six host integration tests. The final [runtime suite](../checks-2026-10-21-001/private-channel372-tests.log)
passes 54 tests, and [strict Clippy](../checks-2026-10-21-001/private-channel372-clippy.log)
passes. Formatting checks pass for the changed runtime files. The
[final input manifest](private-channel-372-inputs.json) records the checked
module and log hashes. It is a component checkpoint, not a full release freeze.

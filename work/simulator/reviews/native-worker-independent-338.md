# Native worker channel independent review

Scope: the source and evidence for channel334, channel335 and disconnect336.
This review is independent of their author. It does not approve the simulator
release. The complete simulator, milestone 2 and all exact-target gates remain
incomplete. No exact physical target passes through this support channel.

## Findings

1. **P2: reuse after a failed exchange can consume an old response.**
   `crates/md-sim-runtime/src/worker.rs:134-154` clones the open channel for each
   request. A timeout closes that clone but leaves the original descriptor
   usable. The protocol has no request IDs. A later request can therefore read
   a delayed response to the first request. The prototype stops on its first
   error, so this does not invalidate successful channel334 or channel335.
   Production `WorkerExecutor` attempts Stop after an error at
   `crates/md-sim-coordinator/src/executor.rs:411-412`. The new character-device
   transport must own and close its descriptor after any failed exchange. Test
   an actual timeout, a late reply and refusal to send a subsequent request.
   This review agent added that type in
   `examples/support/worker_device_transport.rs`. Its four focused tests pass,
   including a real PTY timeout followed by a late response. The parent also
   assigned this reviewer to fix the equivalent `ConnectedTransport` and
   `PrivateTransport` paths. Both now use one owned socket implementation that
   closes after failure. All four real socket regressions pass. The actual
   worker recovery test now restarts the worker on its retained root and uses
   a fresh channel. It keeps all token, result and partial-history checks.

2. **P2: the prototype does not retain the exact result outside the VM.**
   `examples/support/worker_channel_execution.rs:132-164` under the firmware
   crate verifies the downloaded bytes inside the VM, then prints a parsed JSON
   value. The original byte representation named by `result_identity` is not
   retained outside that VM. The later codec integration fixes this byte-loss
   defect. This reviewer checked the restored result size and SHA-256 against
   the result identity in channel341, channel342 and firmware345. They match.
   This identity check is not an independent mining verification.

3. **P2: a timeout can discard the guest console.**
   The reviewed probe waited for QEMU to exit before it read
   `/work/client-console.log`. If the enclosing wall limit stopped QEMU, the
   private temporary filesystem disappeared with that console. The parent has
   added bounded live forwarding in `examples/worker_channel_probe.rs:256-284`.
   The saved stall342 assessment records the expected timeout, retained guest
   marker, `passed:false` and `fault_gate_passed:true`. The prior successful
   runs remain valid within their declared support-process scope.

4. **Open completion gate: client loss does not preserve a running worker job
   in this topology.** `md-sim-worker-ipc.rs:42-43` ends its incoming iterator on
   channel hangup. `md-sim-runtime/src/worker.rs:591-593` then cancels active
   jobs. The outer sandbox also owns the worker's temporary storage.
   Disconnect336 stops before Finish or Execute. It proves that an incomplete
   upload is not published and no job starts. It does not prove active-run
   survival, reconnect, retained results or a complete remote deployment.

5. **Open recovery boundary: cleanup precedes durable export from the VM.**
   Production `WorkerExecutor` synchronizes the exact result inside the client
   VM before it acknowledges worker cleanup. A client-VM loss before export
   can still remove both copies. A correct compressed export does not close
   this window. Keep this requirement incomplete until storage outside the
   disposable VM receives and verifies the result before cleanup, and the
   client-loss/recovery test proves that ordering.
   A common retention barrier has now been added. Its real-worker tests pass.
   The separate outer acknowledgment path passes component349. This reviewer
   checked its recorded acknowledgment, both saved receipts and exact result
   identity. This closes the cleanup-before-retention ordering defect for that
   component path. Active-job reconnect and outer-worker loss remain open.

## Isolation and evidence checks

No concrete isolation regression was found in the optional `namespace_uid`
field or the corresponding confinement check. The default zero field is
omitted from serialization. Nonzero values participate in the process
identity. The original root guest still rejects a parent-root mapping. The
native worker check requires effective UID and GID 1000, no permitted,
effective or bounding capabilities, and no-new-privileges. A mapping's parent
ID is relative to the parent user namespace. It is not a global host UID.

The saved `result.json` size and SHA-256 match the execution identity in each
assessment for channel334, channel335 and disconnect336. Channel334 and
channel335 record support-process passes. Disconnect336 records `passed:false`
and `fault_gate_passed:true`. These remain separate. The captured source hashes
match the checked isolation, worker client, confinement and prototype execution
module. The review did not run a new firmware test or a broad archive scan.

The new artifact codec also received a focused review. Its source opened files
in blocking mode before checking file type. A FIFO with no writer could block
that open. The artifact author added `O_NONBLOCK` and a bounded child-process
regression. The author reports 10 codec tests and focused strict Clippy pass at
channel340. No further codec defect was found for the declared caller-owned
directories and bounded input envelope. Codec success is not a durability test.

Review agents are available after the server restart. An attempt to delegate
another review was refused because all agent slots were occupied. It was not a
usage-quota failure.

The production execution module now uses the common WorkerExecutor and owned
DeviceTransport. Review also found that its post-run identity check could fail
before export. The parent moved artifact export before those fallible checks.
The separate active-job recovery boundary remains open. SSH, selected-run
tunnels, authenticated TLS, native dashboards and full exact-target coverage
remain outside these passes.

## Fix validation

The review agent became the author of the transport fixes after reporting the
findings. The changes themselves need a separate review before release.

The following focused command passed four tests in 5.05 seconds:

```sh
CARGO_TARGET_DIR=work/simulator/check-target-085 CARGO_BUILD_JOBS=1 \
  cargo test -p md-sim-firmware --example worker_channel_probe \
  --offline --locked device_transport::tests -- --nocapture
```

These tests use a real raw PTY and controlled protocol replies. They prove
channel behavior. They do not execute a worker, firmware or miner. The first
run had one unrelated unused-import warning in the shared prepared-board
module. The parent removed that import.

The first coordinator test run is retained in
`work/simulator/checks-2026-10-21-001/channel341-coordinator-tests.log`.
The 13 existing library tests passed. All four new socket tests failed because
their test peer used Rust UnixStream recv/send operations, which this host
denies with EPERM. The peer now uses plain File read/write operations on the
same real socket descriptor, as production IPC already does. The test failure
is retained. It is not a transport or firmware pass.

The next run, channel342, passed the new socket tests but found that the lost
reply test reused a closed channel after a worker error. The integration test
now restarts the real worker on the same root before recovery. Channel343 then
passed all 17 library tests and all 12 queue tests. Strict coordinator Clippy
and focused firmware-example Clippy passed.

The common retention barrier has two additional methods:
`run_with_retention` and `recover_with_retention`. They share the existing
production paths. Their callback receives the locked-run hash, submission
token, run ID, result ObjectId and local result path. Worker cleanup requires
an exact acknowledgment. The receipt is saved and synchronized first. The
original run/recover methods keep their local retention contract. Custom
recovery refuses records without a locked-run hash and applies its barrier
even when a cleanup receipt already exists.

The new actual-worker test withholds acknowledgment, changes each identity
field, and checks that the worker retains the exact result. It then restarts
the worker, copies and synchronizes the exact bytes in a separate run-owned
receiver directory, acknowledges them and checks cleanup. It also checks both
already-cleaned recovery paths. This is a host process test. It is not the
client-VM acknowledgment test.

After the barrier change, channel347 passes all 17 library tests and all 13
queue tests. A further review added a persisted requirement for external
retention. Default recovery now refuses an incomplete attempt that requires
this barrier. It cannot silently downgrade that attempt to local retention.
Channel350 passes the same 30 tests, including this new assertion, and strict
coordinator Clippy. Commands:

```sh
CARGO_TARGET_DIR=work/simulator/check-target-085 CARGO_BUILD_JOBS=1 \
  cargo test -p md-sim-coordinator --all-targets --offline --locked
CARGO_TARGET_DIR=work/simulator/check-target-085 CARGO_BUILD_JOBS=1 \
  cargo clippy -p md-sim-coordinator --all-targets --offline --locked -- -D warnings
```

Evidence: [final coordinator tests](../checks-2026-10-21-001/channel350-coordinator-tests.log),
[strict Clippy](../checks-2026-10-21-001/channel350-coordinator-clippy.log),
[focused retention test](../checks-2026-10-21-001/channel346-retention-tests.log),
[first failed test peer](../checks-2026-10-21-001/channel341-coordinator-tests.log),
[failed old recovery test](../checks-2026-10-21-001/channel342-coordinator-tests.log).

Source SHA-256 values after these checks:

| File | SHA-256 |
| --- | --- |
| `crates/md-sim-coordinator/src/executor.rs` | `e109f928497195a15f56b8505a59447ac4c71cf12e459ffa165e3ab64122c7d6` |
| `crates/md-sim-coordinator/src/lib.rs` | `3efca4be1b8412d0d749f74b4d6876e4b4951d700abdd84ffab2580f7a65b70e` |
| `crates/md-sim-coordinator/tests/queue_behavior.rs` | `25c6d43e165a76f341ce2f69ad7a5ebf6025ea40f7c27b19344fcc6b8bc4485a` |
| `crates/md-sim-firmware/examples/support/worker_device_transport.rs` | `82d91f78f468b487142e78ac6971e560f5022d7fe32b0b71b5ce02236751f527` |

The parent is changing the streamed-input runtime in parallel. Its changes
require their own checks and the final workspace run. The transport and
retention changes above also require a separate reviewer because this reviewer
became their author.

## Independent review of the outer receipt path

This reviewer did not author `worker_channel_retention.rs` or the streamed
input changes in `md-sim-runtime/src/isolation.rs`. No new concrete defect was
found in their component349 path. The runtime gives the isolated process a
pipe, with a declared byte limit and protocol identity. It captures the bytes
actually written. Ordinary execution refuses a streamed policy without its
matching supervisor channel. Namespace termination stops pipe readers before
the runtime joins a blocked writer.

The receiver restores exact result bytes in host-owned storage and verifies
the submission token, locked-run hash, run ID and result identity. It saves and
synchronizes its receipt before sending an acknowledgment. The guest receives
this acknowledgment on a separate private FIFO and virtual serial port. The
worker request stream remains separate. The receipt parser checks the exact
identity before the common coordinator path permits cleanup.

In [component349](../network-vm/native-worker-retention-349/assessment.json),
the runtime recorded 341 acknowledgment bytes under protocol
`md-sim-worker-retention/1`, with no input error. Those bytes decode to the same
identity as both saved receipts. Both receipt files have SHA-256
`cfe149bf44c7883ee66890a664f08a9455f8a6724c1e4d9e993465223c0294d3`.
The identified result is exactly 1,183 bytes with SHA-256
`dd0191e6d9819ffe6da7a144ab3bdbf6ad90627343af257a27826ad6c7188ebd`.
This is a support-process pass. Later wrong-ack fault changes and the final
workspace state still require their recorded checks.

This receipt flow does not preserve an active job when the current IPC client
disconnects, and it does not preserve the worker's temporary root after the
outer worker environment is lost. Neither gate is marked complete here.

## Independent receipt check for firmware354

This reviewer read the two saved receipts and the runtime's recorded input in
`native-worker-firmware-354`. The 346 input bytes decode to the exact same
receipt. The input protocol is `md-sim-worker-retention/1` and the input error
is null. The lock hash is
`4c336d4354acb1ab6cc7d1d356c468d71b43b20175efacad4ffcc8f613a1398e`.
The submission token is
`7678818ef57c2aa628021bacb5efde1b0fc8491ba2a5b8fe4df565dd7fdf8eaa`.

An independent fresh read of the complete retained `worker-result.json`
confirms 103,542,771 bytes and SHA-256
`0d8a5f376caca3cf1fdaeb4f09f85f96472be41a91f083c45c66c38f007f65b9`.
These values match both receipts and the runtime's recorded acknowledgment.
See the [host receipt](../network-vm/native-worker-firmware-354/execution/host-retention.json)
and [worker receipt](../network-vm/native-worker-firmware-354/execution/worker-retention.json).
This checks the exact large artifact's retention boundary. It does not by
itself establish firmware correctness, mining acceptance or whole-VM recovery.

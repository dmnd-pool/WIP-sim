# Native worker SSH design checkpoint

This is a design checkpoint. No SSH or dashboard pass is claimed here. Keep
the full target matrix and original/adapted execution labels unchanged.

Use one disconnected x86 Linux VM for the SSH client and server namespaces.
Keep the native worker and its ARM miner backend outside that VM. The worker
still starts the same captured process through the common runtime. This avoids
running the ARM emulator inside another emulated CPU.

```text
client namespace -> real OpenSSH -> server namespace
  WorkerExecutor                  sshd -> existing worker relay
                                          |
                                owner-only Unix socket broker
                                          |
                                private worker virtio port
                                          |
                              native worker outside the VM
                                          |
                             isolated captured miner process
```

| Reuse | Required change |
| --- | --- |
| `remote_namespace_check::orchestrate` and `role_command`: private veth, distinct network/mount/PID/IPC/UTS namespaces, split credentials, no default route | Add an explicit native-worker mode. Give only the server the checked virtual worker device. Preserve client-only firmware inputs until transfer. |
| `SshTransport::new_streaming`, pinned known_hosts, `md-sim-worker-relay --stream` | Keep their protocol and SSH behavior. Point the relay at the server's private broker socket. |
| `WorkerExecutor` transfer, tokens, live logs, result download and retention barrier | Use these directly for the frozen native process. Do not retain the old board client's separate collection implementation. |
| `worker_channel_probe` outside-VM execution and checked virtual-port identity | Replace its single direct client with the namespace scenario. Keep the native worker's identity and root independent of SSH sessions. |

The new server broker must be a small Rust transport adapter. Its socket lives
in an owner-only directory and accepts only the expected effective UID. It
reads one bounded complete worker request per socket connection. It forwards
requests serially through an owned DeviceTransport. The bridge returns the
existing worker response envelope. It must not invent operation results.

An SSH relay can disappear after forwarding a mutation. The broker must still
finish reading the corresponding native reply before accepting another
request. Failure to write that reply to the departed client must not close the
native channel or stop the worker. If the native exchange itself becomes
uncertain, close that channel and fail explicitly. Never replay a mutation.
The next SSH client uses the saved submission token and selected run ID.

For the client-loss regression, retain the server/broker process and native
worker while killing the SSH client or its entire role namespace. Restart a
client on the retained client root. Confirm the original run ID, continued
live output and unchanged worker build before result retrieval and cleanup.
The existing orchestrator currently returns on the first client failure. Its
fault scenario must instead launch the recovery client and retain failure
evidence. One-shot read reconnect tests alone do not prove run survival.

This closes SSH/client-namespace EOF only. A whole network-VM loss closes the
virtual device and the current `md-sim-worker-ipc` iterator; that still cancels
active runs. To support that larger boundary, keep the native service/root
alive independently and supply fresh request connections to the existing
`worker::serve_connections` loop. A native supervisor can own an mpsc-fed
connection iterator and create socket pairs per complete request. VM channels
then attach to that service instead of owning it. Preserve and drain in-flight
replies before reattachment. Outer-worker storage loss remains a separate gate.

## Selected native dashboard

The existing native S9 service is reachable from its pool-support VM at
`10.0.2.15:80`. Keep that genuine Ethernet/service path. The current runtime
dashboard server creates host Unix and TCP listeners, which this host denies.
It cannot be enabled unchanged on the new native worker.

The smallest additional data path is:

```text
SSH-server loopback endpoint -> separate dashboard virtio channel
  -> run-owned native channel -> pool-support VM virtio channel
  -> bounded TCP connector -> original miner HTTP service at 10.0.2.15:80
```

This requires a declared, bounded, run-owned full-duplex descriptor through
the process sandbox into `pool_vm::start_with_browser`. ProcessSpec currently
offers stdin/stdout/stderr and a mounted dashboard socket, not this descriptor.
Bind the channel to the exact selected run and endpoint identity. Do not put
HTTP data into worker JSON frames, firmware console output or mining traffic.
Preserve byte streams, half-close behavior and flow control for uploads,
streaming and WebSockets. Stop only this run's channel during cleanup.

Reuse `SshTunnel`, `board_dashboard::Probe`, `board_browser`, the native S9
probe and Nginx-based `Gateway` after that endpoint works. Keep separate
browser profiles and cookie stores. TLS must bind the selected VM-server IP
and require the existing client certificate and address policy. The tests
must use the same selected run, not the fixture service at port 18080.

`RemoteGateway::start` currently uploads configuration objects to the worker
and then assumes the SSH server can read them under its local worker store.
That assumption fails when the worker is outside the VM. Add a small verified
object fetch/cache on the SSH server, or an object-ID startup mode. Reuse the
existing gateway implementation after this metadata-location issue is fixed.

Execute gates in order: broker bytes and owner rejection; actual worker run
survival across separate-client loss; artifact and token recovery over SSH;
captured miner execution/mining; selected native endpoint; selected SSH tunnel;
selected authenticated TLS/browser suite. Record the local VM topology. None
of these tests establishes an external physical-host pass.

## Smaller shared-pool alternative

After the first review, the parent proposed hosting the independent pool
process in the same support VM as the SSH namespaces. This is preferable to
the separate HTTP channel if its lifecycle change is explicit. Pass one
declared run-owned Ethernet descriptor from the native miner process to the
support VM. Reuse the existing private-stream QEMU NIC contract. Place that
NIC in the server/pool namespace. Then a selected VM-local proxy can reach the
real miner at `10.0.2.15:80`, and existing SSH/TLS clients need no new HTTP
multiplexing protocol. Keep IP forwarding and default routes disabled.

This moves the pool out of `MinerPool`'s current job-owned VM lifecycle. Preserve
the independent pool verifier and Core node as real processes. Lock their
inputs, budgets, clocks, startup/stop ownership and run ID explicitly. Retain
their service and verifier logs as part of the same execution evidence; do not
lose them because they are no longer merged into the worker's stdout. Record
the support-environment adaptation and rerun fresh-work, Core and dashboard
tests. The new private full-duplex descriptor primitive is still required.

## Implementation checkpoint after broker361

The persistent broker now passes the [real-worker client-loss test](worker-broker-361.md).
The first SSH integration is implemented in
`remote_namespace_check/worker_broker.rs`, with explicit mode hooks in
`remote_namespace_check.rs`. Its preparation command is:

```sh
remote_namespace_check prepare-worker-broker SCENARIO_JSON LOCKED_RUN_JSON INPUT_OBJECTS_DIR RELAY_BINARY VETH_MODULE
```

The helper writes the minimal program and file maps. It verifies each locked
input hash. The parent adds the exact `/worker-identity.json` and provides the
native worker through the named `md-sim.worker` virtual port. Only the server
role receives that checked device. Descriptor3 carries the device. All other
descriptors become close-on-exec. A deliberate inheritable input descriptor
tests that boundary. The client requires descriptor3 to be absent. Installed
Bubblewrap0.13.0 does not support `--preserve-fds`, so no such option is used.

The server runs the owner-only broker as UID1000. The client uses the production
streaming `SshTransport` and `WorkerExecutor`. The orchestrator observes the
real SSH process, terminates its complete client PID namespace, checks that
all old client processes disappear, and starts a new client namespace. Recovery
requires the same native run to remain active and produce more real output.
It then stops that run, verifies the complete result, requests cleanup, and
exports exact bytes through the bounded artifact codec. The module never starts
the separate HTTP protocol fixture.

The marker `MD_SIM_NATIVE_SSH_BROKER` contains the same artifact layout used by
the native channel receiver. This first mode claims VM-local durable retrieval
before cleanup only. Its final stdout export does not establish an outer
durable acknowledgment. SSH integration has not passed at this checkpoint.
The parent is running its first native outside-VM worker test. Keep this gate
open until its exact run, loss, result and cleanup evidence passes.

The first real outside-VM SSH test, cycle372, reached authenticated SSH,
artifact transfer, ExecuteToken and live output. It failed the strict check
for remaining processes after client namespace termination. SSHD records the
connection closing. The saved evidence lacks the remaining process state, so
that failure is not classified as a harmless zombie. Cycle373 makes the VM
orchestrator a child subreaper, records actual PID, parent PID, state and nested
PID values, and reaps only observed descendants of the killed client namespace.
It still requires zero remaining processes. A fresh client must also observe
output growth between two post-loss Status calls, not only between an earlier
client sample and recovery. These changes require a new real integration run.

Cycle373 confirms the remaining process behavior: the owned Bubblewrap child
changes from sleeping to zombie, then the orchestrator reaps it. The SSH
process disappears. The replacement client retrieves and cleans the same run,
but a later namespace comparison fails. Both clients' recorded namespace
inode numbers are identical after the old namespaces have no references.
The Linux kernel can reuse those numbers. The test now holds checked namespace
file descriptors across replacement, so the strict inequality has a stable
basis. It does not waive the inequality or the no-process-left requirement.

The late cycle373 failure also exposed an evidence defect: result export came
after the failed observation. The new wrapper emits the exact available
artifacts with `passed:false` before propagating a late error. The outer
receiver must import them before checking that flag. Complete retained bytes
still do not turn a failed observation into a pass. Helper375 builds with these
changes; the next actual SSH run remains required.

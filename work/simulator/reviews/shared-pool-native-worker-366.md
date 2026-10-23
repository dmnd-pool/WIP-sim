# Shared pool for the native S9 worker: integration review 366

Date: 2026-10-23. Reviewer: `exact_target_gap`. This is a source review and
an integration contract. It is not an executed remote-miner result. The
archived S9 firmware still uses an adapted reference environment. The exact
physical controller, FPGA, hashboard and PSU gates remain open.

## Selected path

Use one native S9 QEMU process in the existing native worker. Connect its
Cadence GEM Ethernet device to the existing support/SSH VM. Use the new
runtime-created private carrier. Run the existing independent `s9_pool_guest`
verifier in the support VM's server namespace. Do not start another pool VM
or a pool inside the miner runner.

The miner's existing NIC contract is
`socket,model=cadence_gem,fd=3,x-md-sim-private-stream=on`. The support VM uses
the matching private stream backend and an e1000 device. The original console
script already sets the miner to 10.0.2.15/24 and the pool to 10.0.2.100:3333.
Do not change that firmware script only to join the carriers.

The support VM must be alive before its SSH client can request execution.
The runtime offers its peer only during execution. Thus the outer supervisor
must create a run-owned support-NIC socket pair before VM startup. A bounded
provider bridge then joins that endpoint to the runtime-created peer for the
selected process. Neither the request protocol nor ProcessSpec accepts an
arbitrary host descriptor. Keep the worker control character device separate.

Move the support VM's actual `eth0` into `md-worker` before the server drops
privileges. It has 10.0.2.100/24. Keep the existing `worker0`/`client0` veth pair
at 192.0.2.1/29 and 192.0.2.2/29, with the existing client alias. Record network
namespace IDs, interface inventories, addresses and routes. Verify no default
route, forwarding or bridge between the client and miner networks. The client
must use the selected SSH/TLS service path. It must not gain the miner NIC.

## Smallest explicit preparation contract

Add `shared-pool=MANIFEST` to `s9_boot_bundle boot`, with `local-pool`, a real
console script and `prepare-only`. Keep the old CLI path unchanged. For this
first shared mode, reject embedded `pool-vm`, nested TCG, private loopback,
runner-owned browser/dashboard actions and multiple runner restarts. Those
remain integration work; these rejections do not prove target incompatibility.

The strict manifest must pin the pool verifier executable, support runner,
support QEMU binary and support kernel as content objects. It must also pin
the carrier protocol, its frame/byte/packet limits, fixed network addresses,
pool readiness budget and observation margin. Capture the manifest bytes at
`/shared-pool.json` and retain the same bytes as `shared-pool.json`. Include
the new optional runtime carrier policy and execution flag before freeze.
Generate `pool-input.json` once through the existing fresh-seed code and mount
it at `/private-pool.json` as before. Preserve its seed, time, two difficulties,
regtest anchor, connection limit and completion policy.

Avoid a hash cycle. The final support VM image contains the selected miner
lock. Therefore the inner shared manifest cannot hash that final image. Pin
the exact support components in the inner manifest, then pin the complete
assembled support image and its root manifest in the outer execution lock.
The outer preparation must verify the captured support components against
the inner manifest. A path or a descriptive build name is insufficient.

The runner must take ownership of inherited socket descriptor 3 at entry,
check its type and mark it close-on-exec. Explicitly pass it only to miner
QEMU. Close the runner copy immediately after successful QEMU spawn. Keep
ASIC and storage tracing unchanged. A fixed observation end at pool duration
plus the declared margin, measured from attachment, can hold the real guest
alive. It is only an observation window. It does not assert pool readiness,
accepted work, or successful mining.

## Exact implementation sites

| Site | Required change |
| --- | --- |
| `s9_boot_bundle.rs::boot` (currently near line 950) | Validate the explicit mode; capture manifest; choose private-stream backend checks and NIC; set carrier policy before writing ProcessSpec; record adaptation and observation budget. |
| `s9_boot_bundle.rs::__boot-console` (near 537) | Preserve the actual serial guest path. Suppress `LocalPool::start` and `MinerPool::start`; own/pass/close descriptor 3; drain actual guest output for the declared observation window. |
| New small `support/s9_shared_pool.rs` | Strict manifest parsing and validation, descriptor ownership, child attachment and observation records. Keep target-specific policy out of generic runtime. |
| `support/prepared_board.rs::freeze` | Capture shared metadata along with ProcessSpec and pool input. Reject mutation after freeze. |
| `support/worker_channel_execution.rs::stage` (111) | Admit only the explicit selected shared mode. It currently rejects every private carrier. Preserve all input, runtime, memory and trace bounds. |
| `worker_channel_probe.rs::inside` (190) | Add the support VM NIC, run-owned endpoint and bounded provider bridge. The current VM has `-nic none`. Retain actual carrier and peer receipts. |
| `md-sim-runtime/src/worker.rs` | Use the new trusted exact-ProcessSpec binding and recorded carrier execution path. Current implementation work already preserves cgroup/dashboard receipt arguments. Repeated token lookup must not consume a second provider. |
| `remote_namespace_check.rs` role setup | Move/configure the miner NIC in the server namespace; capture only the server's pool helper/input/manifest; retain separate client keys and mounts. |
| `remote_namespace_check/worker_broker.rs::service` (177) | Wrap the persistent native transport with a selected pool lease. Start it before forwarding the first matching ExecuteToken; keep it across SSH reconnect. |
| `s9_pool_guest.rs::main`, `configure_network`, `drop_privileges`, `server` | Add a checked preconfigured-server entry. Verify the existing server namespace/address and unprivileged UID. Reuse the same actual verifier and session allocation. Do not reload/configure a second NIC or call root-only credential operations after privilege drop. |
| `support/worker_channel_retention.rs` and execution artifact export | Bind and retain external pool artifacts before cleanup acknowledgement. Existing retention acknowledges only native worker evidence. |
| `support/s9_execution_report.rs::retain` and `support/s9_submission_audit.rs::analyze_session` | Add explicit external-pool provenance. The current audit requires actual pool markers in the native runner log. Never append external markers to that log or waive the identity check. |

## Pool lifecycle and evidence

`WorkerExecutor::run_internal` transfers and verifies all input objects before
it sends ExecuteToken. The persistent server broker can intercept that first
selected request. Validate the immutable shared manifest and exact process,
start the pinned `s9_pool_guest`, wait for its real ready record, and then
forward ExecuteToken. Do not start the pool window at VM boot: artifact
transfer would consume the mining budget. Record the measured readiness and
forward times. Existing pool deadlines remain unchanged once it starts.

Keep one lease with the request token, exact process identity, pool input hash,
support component hashes, helper PID/start time, actual ready record and the
returned worker run ID. Retry of the same token reuses this lease. LookupToken
and SSH reconnect must not regenerate seeds or restart the pool. Reject a
different token or identity while the singleton carrier is reserved. A failed
or uncertain Execute exchange retains the lease and its evidence until actual
worker state is known; it must not launch a second miner.

The pool already limits clients and session allocation. Retain every completed
and failed session, collection marker, raw bounded stdout/stderr, process exit
state and packet capture. Bind these to the selected native worker result and
carrier receipts. Select the first authorized V1 session in allocation order,
as the current report does; never select by success. Keep SHA-256d, job seed,
serial response, nonce, difficulty and clean-job checks unchanged. Each Core
test must use a candidate from this joined evidence.

On cancellation, stop the selected native run and close its carrier. Reap the
owned pool process under a bounded deadline and retain incomplete sessions as
incomplete. Client namespace loss must leave the server broker, pool and miner
running. Support VM loss is a pool failure and needs separate evidence; it is
not a remote survival pass. Whole-run cleanup must reap only owned processes.

Extend the retention receipt so the outer supervisor verifies and saves both
native result bytes and external pool artifacts before acknowledging cleanup.
Record hashes for the final result, process, adaptation, pool input, shared
manifest, pool binary, pool log, collection and support execution. Preserve the
native `result.json` and guest log bytes exactly. A missing or mismatched pool
artifact must block the joined mining result and cleanup acknowledgement.

## Budgets and checks before a claim

The current SSH broker/server and role orchestrator have 240-second limits.
The shared case needs one explicit outer budget that includes preparation,
input transfer, the unchanged native miner wall/CPU limits, pool collection,
result transfer and bounded retention. Do not inherit the short support-process
fixture deadline. The existing native firmware process limits remain at most
360 wall seconds and 330 CPU seconds unless a new recorded profile is justified.

First test wrong manifests, unavailable/unbound carriers, duplicate tokens,
readiness failure, cancellation, peer EOF and retained evidence mismatch. Then
run actual archived firmware, independent submission audit and two Core checks
for the new execution identity. Test client loss/reconnect with that same run.
Finally test selected-run SSH and authenticated TLS dashboard paths from the
separate client namespace. A local VM/namespace test must state that topology;
it is not an external physical host result.

The independent primitive review is in
[private-carrier-independent-365.md](private-carrier-independent-365.md).
Its passing framing and process-loss tests do not substitute for the tests
above. No integration gate is marked passed by this design note.

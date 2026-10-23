# Native remote fault tests after run 431

The shared native path still needs an explicit client-loss and cancellation test.
The passed support-process test 375 does not close this firmware gate. Normal
access run 431 failed at TLS listener startup after the native HTTP checks. Its
result remains failed. This note is a source review and a next action, not a test
result. The TLS startup fix and its actual test take priority.

## Current behavior

- `shared_run.rs:160–186` starts one selected firmware run with
  `run_with_retention`. Its cancellation flag stays false. The shared supervisor
  at line 245 starts only one client. It does not inject SSH or client loss.
- `worker_broker.rs:643–790` already tests an actual OpenSSH client namespace
  loss, keeps the broker and worker alive, pins namespace descriptors, reaps
  owned descendants, connects a fresh client, checks the same run and log prefix,
  then stops and cleans it. Shared mode dispatches to `shared_run`; it does not
  run this fault scenario. Test 375 used a support process.
- `executor.rs:459–629` has `recover_with_retention`. It validates the saved
  worker identity and token, finds the same run, then sends **Stop** before result
  retrieval. It cannot resume a running job to natural completion. It preserves
  the exact result barrier and performs no second ExecuteToken.
- `shared_pool.rs:443–483` forwards the selected Stop and writes a cancellation
  request with the same token and run. `pool_service.rs:243–307` checks that
  binding, requests actual pool cancellation, retains its outcome and applies a
  bounded child-stop fallback.
- `shared_run.rs:367–379` correctly requires a normal exited result and a complete,
  non-cancelled pool. A cancellation test needs its own explicit expected outcome.
  It must not weaken those normal-run predicates.

## Smallest useful actual extension

Add one explicit, captured **native client-loss then stop** scenario. Reuse the
current prepared archived firmware, exact LockedRun and normal retention format.
Keep this fault result separate from natural-completion mining acceptance.

1. Reuse the owned kill/reap and namespace-pin code from test 375. Start the
   existing native worker, broker, private pool and one production SSH client.
   Wait for the selected token/run to be committed, the same run to be booting,
   and actual device or guest output to be available. Save bounded prefixes and
   offsets from both stdout and stderr. Use an explicit bounded trigger. Do not
   extend firmware, bus or protection deadlines to make the trigger pass.
2. Kill only that client namespace and its observed descendants. Preserve the
   server, pool, provider channel, native worker and exact attempt directory.
   Retain the first client PID, observed real SSH PID, namespace identities,
   status/log samples and kill/reap result. Hold namespace descriptors until the
   replacement identity comparison is complete.
3. Start a new authenticated SSH client. Read the existing worker request and
   LookupToken. Require the original ProcessSpec, worker identity, token and run
   ID. Require booting at reconnect. Check unchanged log prefixes and real
   post-loss progress within a declared bound, with the same run still active.
   Do not issue ExecuteToken again. Do not copy test 375's fixed 500 ms stdout
   growth assertion: real native stdout can pause while stderr or hardware/pool
   transactions advance. Final retained traces must corroborate the observations.
4. Call existing `recover_with_retention` to stop this same run deliberately.
   Require a cancelled native result and the corresponding pool cancellation
   outcome. Preserve partial pool sessions and errors. Reuse the exact
   native/pool/access export, host durable receipt and cleanup barrier. Verify
   exact acknowledged result bytes, terminal token lookup, cleanup and no owned
   process or namespace leftovers. A wrong receipt must still prevent cleanup.
5. Add a separate fault assessment that requires every step above. It must fail
   on early natural completion, absent progress, wrong token/run, lost evidence,
   missing cancellation or cleanup failure. A cancelled run is not a substitute
   for the required fresh-work/Core natural-completion test.

No new generic worker execution implementation is needed for that combined
survival-and-stop case. The shared harness needs an explicit first/recovery
client phase, a reused owned-loss helper and a scenario-specific outcome check.
A later natural-completion-after-full-client-loss test needs a small common
executor resume API that reuses the existing polling and collection loop while
preserving the original deadline. Calling today's recover API would stop it.

## Access and selected URL

`carrier_dashboard_client.rs:113–226` binds access to the persisted token, selected
run, complete lock, provider/policy identities, endpoint and TLS publication. Its
native probes and optional browser use that selected run. The normal observer
stops its gateway and SSH tunnel before reporting success. These bindings are
sound for the tested session; they are not evidence that a publication survives
client loss.

The smallest combined test can wait for the actual successful access report
while the selected run is still active, then inject loss. A recovery client must
validate and reuse that exact retained report; it must not start another Observer
in the existing output directory or claim a new publication tested the old URL.
This closes access-before-loss plus native control survival, not live-publication
survival. It may be simpler to run the first fault case without a dashboard policy
and then add the combined case. Either selection must be explicit in its frozen
identity and report.

To claim published dashboard survival, inject loss while the selected publication
is still open. Use a separately owned authorized client to request the **same**
URL and run before and after the loss. For an intentionally lost local SSH tunnel,
record closure and an explicit replacement tunnel; do not call it an uninterrupted
URL. Keep that work separate from the minimal control fault test.

`SshTransport` already retries one interrupted read-only exchange; it does not
replay mutations. A second monitoring SSH connection or the existing one-shot
Probe fixture does not prove loss of the active execution controller. The real
controller/client must be the subject of the fault.

This topology is a native worker outside a support VM with separate OpenSSH
client/server namespaces inside that VM. It does not prove survival of support
VM loss, native worker loss or an external physical-host deployment. Exact-board,
full fault-matrix and full-matrix completion gates remain open.

No source, build or VM was changed for this review. The adjacent manifest records
the source state inspected.

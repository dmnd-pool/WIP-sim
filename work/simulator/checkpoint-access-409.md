# Access integration checkpoint

23 October 2026, 10:09 UTC. Work is incomplete. Milestone 2 and the full
matrix remain open. This checkpoint does not grant a dashboard pass.

The tested native shared run391 and Core392 evidence remains unchanged.
See `reports/shared-native-worker-394.md` and `continuation-2026-10-21.md`.
New access code has not yet run against that native path.

Current source owners:

- Parent: carrier_dashboard_client.rs, carrier_artifacts.rs, shared_run.rs,
  support/worker_shared_retention.rs, worker_shared_host.rs, new
  worker_shared_access.rs, and worker_channel_ssh.rs.
- private_channel: carrier_dashboard.rs service, shared_pool preparation,
  credential/tool/mount wiring in main, optional carrier browser capture,
  s9_browser_target.rs, s9_browser_selected.rs and board_browser.rs.
- exact_target_gap: access/carrier_gateway.rs, remote_carrier.rs, main CLI,
  identity/readiness tests. Public API is remote_carrier::{CarrierInputs,
  CarrierRemotePaths, RemoteCarrierGateway}. Paths include explicit ready.
- channel_review: runtime worker absolute-deadline request API and access
  carrier_inputs.rs. Independent client/service/retention review continues.

The new client observer runs beside WorkerExecutor. It waits for the actual
saved worker run, inspects its authenticated carrier endpoint, starts an SSH
tunnel, checks the original HTTP index hash and the native API suite, starts
selected mutual TLS, checks missing-client-certificate rejection, and runs the
same native suite. Optional real browser checks run while TLS remains active.
Native360s/305s observation limits remain unchanged. Browser failure cannot
become an HTTP/TLS pass. The callback joins the observer before requesting
native retention. Failed observations remain explicit.

Shared retention v1 stays unchanged without access. Access uses explicit
schema2 and an added access-bundle identity. Native, independent pool and
access bytes must be durable before the joined ACK permits cleanup. Limits:
32MiB access JSON,17MiB compressed,15MiB raw files,8MiB/file,512 traversed
entries and bounded depth. TLS private keys and browser profiles are excluded.
Receipt-bound browser request and helper identity require the corresponding
browser result. Host checks rehash extracted files before reading pass fields.

Review fixes made:

- Bound directory entries before collecting them, including empty fanout.
- Keep native control broker alive after access failure so evidence can be saved.
- Provider EOF with a still-running worker is not normal completion. Require
  an actual selected terminal worker observation; preserve raw failure receipts.
- Use one absolute worker request deadline, including connect/write/read.
- Rehash extracted access evidence against the retained bundle on resume.
- Capture provider/policy/full-lock/access helper hashes in the VM support proof.

Checks completed:

- worker_channel_probe405 and407:52 pass,2 descriptor helper entries ignored;
  these entries have separate actual descriptor regression evidence.
- remote_namespace_check408 failed because the API moved to remote_carrier.
  Parent import fixed.409:89 pass,2 descriptor helper entries ignored.
- Input fetch403:3 pass, including actual Worker Get over socket pair.
- Absolute deadline host405 and isolated VM406 tests pass. See agent report.
- Earlier carrier and byte-stream VM399 and provider lifecycle398 remain valid
  component evidence; they do not exercise the new full access integration.
- Browser helper407:9 pass. Optional final BROWSER_PAYLOAD preparation argument
  captures the supplied browser payload and current sibling s9_browser_check.

Next actions: finish new gateway tests and independent review; run formatting,
strict workspace Clippy and affected/full tests; freeze all helpers and source;
prepare a new shared-pool manifest plus carrier policy and a new native bundle;
run actual selected SSH/TLS access with joined retention; independently audit
mining and submit both fresh candidates to Core. Add browser capture to that
actual run when its integration is ready. Never rewrite390/391/392 evidence.

One shared Cargo target remains `work/simulator/check-target-085`. Incremental
builds are off. TMPDIR is `work/simulator/test-tmp-227`. About87GiB repo and14GiB
tmp free at09:55. Preserve all protected stores listed in continuation09:33.

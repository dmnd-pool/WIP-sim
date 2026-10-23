# Selected carrier access implementation

This is current implementation work. It is not a native dashboard or full
simulator completion claim. Frozen390 and actual391 are unchanged.

The support server now starts a UID1000 carrier byte service only after the
selected pool gate has committed the actual execution token and run ID. Its
own mount, PID, IPC and UTS namespaces retain the server network. Its writable
directory is `/work/access` at the same absolute path as the SSH service.
It cannot see pool scratch, client credentials or the server TLS private key.
It saves actual namespace checks, bounded control exchanges, atomic readiness,
the endpoint, byte receipts, raw refresh failures and final outcome.

Root owns launch, stdout, stderr and process outcome outside that directory.
The helper has memory, CPU, descriptor, file-size and process limits. It uses
the actual worker provider monitor with a single absolute deadline per request.
The listener has its own owner-lease expiry. A carrier EOF closes it at once.
EOF is normal job completion only after exact selected worker observations
show stopped or cleaned within a separate ten-second bound. Failed or still
active workers are retained failures. Service failures preserve the control
broker so the parent can retain the native result before applying the access
gate.

Preparation captures exact lock/policy/provider metadata and the access CLI,
curl, OpenSSL and nginx. Provider configuration comes from the parent outer
scenario. Credentials use a fresh outer context. The server receives only
server credentials; the client receives only client credentials.

The browser has an explicit carrier selection schema. It validates exact
policy/provider bytes, the complete lock, selected process, private endpoint,
actual TLS publication and recorded live provider observation. It does not
create a runtime dashboard Binding. It reuses the existing two-origin native
UI, asset, fresh-profile, HTTP and WebSocket suite. Optional final
`BROWSER_PAYLOAD` on `prepare-shared-pool` captures the browser inputs. The
native process and mining observation budgets are unchanged.

Checks are in `checks-2026-10-21-001/`:

- `carrier-service402-check.log`: helper compiles before the later browser path.
- `carrier-service403-tests.log` and `carrier-service404-tests.log`: concurrent
  parent client API work did not compile yet. These are not passes.
- `carrier-browser405-tests.log`: nine browser metadata/regression tests pass.
- `carrier-browser406-clippy.log`: large selection enum found; both variants
  were boxed without changing the serialized selection.
- `carrier-service-browser407-tests.log`: 89 helper and nine browser tests pass.
  Two ignored entries are existing child entry helpers used by real tests.
- `carrier-service-browser408-clippy.log`: unnecessary borrow and oversized
  preparation argument list found. Both were fixed.
- `carrier-service-browser409-clippy.log`: strict focused lint passes.
- `carrier-service-browser411-tests.log`: final affected test rerun follows the
  preparation interface and bounded terminal-wait fixes.
- `carrier-browser-retention418-tests.log`: after shared oracle extraction and
  the final retained process-exit checks, 90 namespace, ten browser and 58 host
  probe tests pass. The four ignored entries are existing child helpers.
- `carrier-browser-retention419-clippy.log`: strict lint passes for all three
  affected examples. Owned-source formatting also passes.

Independent reviewer channel_review found and reviewed fixes for per-read
timeouts, false normal completion from peer EOF, preservation of the broker
after access failure, and precision of the shared terminal-wait deadline.
The parent owns client orchestration, joined artifact retention and the next
real native run. That run, native browser completion, authenticated TLS and
SSH endpoint checks are still required. Metadata tests alone cannot pass them.

The separate access gateway review found unbounded SSH child logs and late
connection-loss handling. The access agent fixed both and added focused tests.
The remaining cleanup finding was fixed by reusing the existing coordinator
owned-child implementation. It keeps the SSH leader unreaped until its group
is stopped. The real descendant regression and final access tests415 pass.
The independent gateway review is `reviews/carrier-gateway-independent-private-channel-415.md`.

The browser report oracle now lives in `support/s9_browser_target.rs`. Both
the live parent and retained-result checker call it. Host verification also
joins exact raw stdout/stderr, one raw result marker and the process record.
It rejects nonzero exit, process error and report-count contradictions.
The parent records the original browser request and exact payload/helper
identities before preparation, then compares the actual scenario/root capture.
This closes silent downgrade and shallow passed-flag acceptance findings.

Own source is final after checks418/419. Independent review is closed. Parent
will freeze and run the basic native HTTP/TLS path first. Browser capture is
optional and must have a separate real run before it can be claimed.

## Actual startup fix after native access421

Native421 retained its native/pool/access failure evidence. Carrier startup
failed on bwrap --uid without a user namespace. The real-account fix uses a
minimal privileged entry; actual424 then found and preserved the root chdir
permission failure. Actual426 passes the production Supervisor/service
launch with real broker UID1000 credentials, zero capabilities, isolated
mount/PID/IPC/UTS, exact endpoint readiness and clean stop. Broker responses
are scripted component data; no HTTP/TLS or firmware pass is inferred.
Helper tests426 pass90; production/test-target strict Clippy427 and format
checks pass. See reviews/carrier-startup-426.md and its exact manifest.
Only a verified formatter line wrap separates the captured test source from
current source. Production source is unchanged. Next action is the parent's
fresh frozen native access run.

## TLS readiness after native431

Native431 passed its separate native HTTP-over-SSH check, then failed the
one-shot client TCP reachability test after publication. It did not run
the TLS suite. The old check lost the socket error; the exact OS cause
cannot be reconstructed. The client now retains each actual connect error
and uses the existing single30-second deadline across publication and
listener readiness, with owned-child checks and unchanged cleanup.

Small actual fixture435 failed on early nginx exit. Diagnostic437 retained
getpwnam("http")failed, a root-only fixture difference. The realUID1000
fixture438 passes delayed nginx startup, authenticated component TLS,
remaining-budget refusal, dead-child rejection and Drop cleanup. Failed
variants are retained. Host438 tests22pass; strict lib/test lint439 and
format pass. Source is final and independent438 review is closed. See
reviews/carrier-tls-readiness-438.md and its exact input manifest.
Next action: parent freeze440 and fresh selected native HTTP/TLS run.
No native TLS, browser or full simulator completion is claimed here.

# Private channel component progress

Completed: runtime-created framed Ethernet carrier; explicit provider API;
per-direction framing, byte and packet limits; hashes, counters and closure
receipts; full-duplex isolated process tests; host descriptor exclusion; EOF,
backpressure, timeout, cancellation, and real supervisor-loss tests; bounded
trusted-provider relay; exact one-use worker binding; token-safe replay and
cleanup; owned worker-root lease.

Latest checked state: [checkpoint370 with follow-up371](reviews/private-channel-370.md).
Detailed371 passes six host integration tests. Runtime372 passes 54 tests and
strict Clippy. Changed runtime formatting checks pass. Three ignored harness
entry points are executed by the integration tests. Source and log hashes are
in [the final input manifest](reviews/private-channel-372-inputs.json).

Independent reviews cover the primitive, worker binding, provider wrapper, and
root lease. No correctness finding remains in those component scopes. The
shared runner review is [recorded separately](reviews/shared-pool-independent-private-channel-366.md).

The [actual worker CLI test](reviews/actual-worker-carrier-379.md) now executes
the built worker with a real isolated challenge guest. It checks uploaded
hashes, two fresh full-duplex exchanges, descriptor isolation, exact process
binding, saved results, cancellation, cleanup, provider reports, and shutdown.
The full coordinator suite passed 36 tests in checkpoint380. Independent
review found a test cancellation race; the saved-log barrier fixes it.
Checkpoint382 passes the revised actual CLI test and strict focused Clippy.
This peer is a local test process, not the support VM or an external host.
The [final independent CLI review](reviews/worker-cli-carrier-independent-382.md)
has no open finding in that test scope.

Follow-up387 exposes the existing `worker::PROTOCOL_VERSION` constant for the
shared pool gate's strict error envelope. Its value stays `md-sim-worker/2`.
The runtime build check, worker formatting check, and 14 affected worker tests
pass. Logs are `checks-2026-10-21-001/private-channel387-public-protocol-check.log`
and `checks-2026-10-21-001/private-channel387-worker-tests.log`.

Retained failures: unsupported Bubblewrap `--preserve-fds` was removed after
checking its actual source. The extra `unshare --fork` monitor concealed guest
EOF, so channel runs now let unshare execute Bubblewrap directly. A closed
socket with no requested poll events caused a host CPU spin; the relay now
omits that descriptor while blocked. The first worker test retained the store
writer lease; the corrected test releases preparation ownership before service.
Failed367 evidence remains. Its four abandoned fixture stores and the separate
accidental default debug cache were removed. Original evidence, release
binaries, canonical firmware stores, and the shared build cache were retained.

Next action belongs to the parent integration: connect the trusted worker IPC
provider to the captured support VM NIC, freeze exact combined inputs, and run
the actual selected native miner and service path. The runtime primitive does
not prove firmware, mining, SSH, TLS, native dashboard, or full target coverage.
Those completion gates remain open.

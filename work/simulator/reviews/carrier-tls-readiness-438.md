# TLS listener readiness fix

Actual native431 retained a failed selected TLS result. The client accepted
an exact selected publication, then its single TCP connect failed. It had
not yet sent the unauthenticated TLS probe or run the native TLS suite.
The native HTTP suite over SSH passed separately. The carrier service
closed normally. Publication and an empty startup log do not prove nginx
had reached listen. The old client discarded the socket error, so the
retained431 record cannot prove the exact operating-system refusal cause.

The client now waits for the selected listener inside the same original
30-second startup deadline as publication. It checks its owned SSH child
before and after each bounded connect. It retains each actual socket error,
error number, duration and attempt time. A deadline or child exit still
fails. The existing owned-child cleanup runs on every failed start. This
changes no firmware, mining, TLS authentication or native timing policy.
A TCP-ready record is explicitly separate from TLS/native acceptance.

Actual fixture435 failed on early nginx exit. Its first test version did
not export the child error before temporary-directory cleanup. That raw
failure remains. Diagnostic437 preserved the exact stderr:
`getpwnam("http") failed`. This was a root-only test setup difference;
production nginx runs as worker1000. The fixture now uses real UID/GID1000
and only changes ownership of its own temporary paths. Its fixed log
allowlist is retained on every failure. Child PID publication is atomic.
No production gateway configuration or assertion was weakened.

Actual private VM438 passes with the captured nginx and test binary:

- Seven real ECONNREFUSED results precede a successful connect at370 ms.
  The complete readiness operation records374 ms.
- Curl checks the generated CA and presents its client certificate. The
  actual nginx TLS response contains the explicit component fixture body.
- A refused listener receives only250 ms left from the original30-second
  startup budget. It fails at30002 ms on that original clock.
- An already exited owned child fails before any connect, even when an
  unrelated listener is reachable at the address.
- Failed-start Drop closes stdin, obtains the child acknowledgment and
  reaps it. The owned nginx process and listener are also removed.

The VM exited0 without truncation in14605 ms. Its guest kernel was
7.2.9-arch1-1. This is a real nginx and process-lifecycle component test.
It is not a firmware, native dashboard, browser or external-host pass.
Native431 remains a failed TLS run. A fresh full native run is required.

Host access tests438 pass22. The three ignored outer entries are the
existing carrier stream VM test and the new readiness test plus its child
entry. VM438 executes the readiness test and its actual child cases.
Strict lib/test Clippy439 and scoped formatting checks pass. Production
readiness source is unchanged since the initial435 capture; only the
fixture was corrected after its measured failures.

Commands run:

```sh
CARGO_TARGET_DIR=work/simulator/check-target-085 CARGO_BUILD_JOBS=1 cargo test -p md-sim-access --lib --offline --locked
work/simulator/check-target-085/debug/examples/isolated_network_vm work/simulator/network-vm/tls-readiness-438 work/simulator/tls-readiness-vm-438-inputs/scenario.json
CARGO_TARGET_DIR=work/simulator/check-target-085 CARGO_BUILD_JOBS=1 cargo clippy -p md-sim-access --lib --tests --offline --locked -- -D warnings
```

Use a new output directory when repeating the VM command. Exact source,
helper, root inputs, results, old failures and check logs are pinned in
[the manifest](carrier-tls-readiness-438-inputs.json).
The [independent review](carrier-tls-readiness-independent-438.md) has no
open finding in this scope. The parent owns the next release freeze and
full selected native HTTP/TLS rerun.

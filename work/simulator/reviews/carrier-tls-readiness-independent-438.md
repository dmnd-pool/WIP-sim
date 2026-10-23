# Independent TLS readiness and progress review 438

No open correctness finding remains in the reviewed change. The current
production and fixture source bytes match the captured 438 inputs. The actual
VM regression passed one test with zero ignored tests. This is a component
readiness result. The full selected native SSH/TLS run must still be repeated.

`RemoteCarrierGateway::start` now keeps publication parsing and TCP readiness
inside one 30-second startup deadline. Each connection uses the lesser of three
seconds and the remaining budget. It records the operating-system error,
error kind, elapsed time and connection time. It checks the owned SSH child
before and after each connection and checks the deadline before accepting
success. It retries only readiness probes. It does not replay service mutations
or change the firmware and pool budgets. Existing owned-child cleanup remains
active on every startup error. The receipt claims TCP reachability only.

The [independently parsed result](carrier-tls-readiness-independent-438.json)
and [exact inputs](carrier-tls-readiness-independent-438-inputs.json) confirm:

- The captured test helper and actual nginx match the VM root manifest and
  their input objects. The helper hash is
  `0d86bd5bc6a38bb46df725edf2b0350e391906c1f6973c42151eb91196d95816`.
- An actual delayed nginx process returned seven connection refusals, then a
  successful connection at 370 ms. The loop completed at 374 ms. A real curl
  request with the generated client certificate received the expected component
  body over authenticated TLS.
- A refused listener with only 250 ms left from the original 30-second budget
  failed at 30,002 ms. It did not receive a new listener startup budget.
- An already-exited owned child failed before any connection, even though an
  unrelated listener occupied the selected address.
- Failed-start Drop closed the owned stdin pipe and reaped the child. The nginx
  process and listener were also absent after the successful case was stopped.
- The scenario test exited zero. The outer process exited zero with untruncated
  traces. The author also ran 22 host tests with zero failures.

The initial attempts 435 and 437 remain failed. Attempt 435 lost its temporary
child logs; this was an evidence defect in the test fixture. The follow-up 437
retained the actual nginx error: its compiled default `http` account was absent.
The final fixture runs the child as real UID/GID 1000, as production does, and
checks those IDs at child entry. It changes ownership only on its fresh fixture
paths. It does not add a system account or alter the nginx configuration or
production assertions. A failure guard now exports bounded allowed logs before
its temporary directory is removed. The fixture account error does not establish
the cause of run 431's separate TLS failure.

The parent `shared_progress.rs` change also has no open finding. It reads regular
files through the existing nonblocking, no-follow, bounded reader. It adds file
metadata and a passed/error summary, limited to 512 Unicode characters. It does
not copy HTTP observations, response bodies or publication fields. The existing
8 KiB output cap and diagnostic-only label remain. Its 91-test helper run passed,
including FIFO, oversized-report and omitted-response-data cases. Focused strict
Clippy and formatting checks 439 also passed. Diagnostics do
not grant acceptance and a missing or oversized report does not become a pass.

This review changes no source or result. It does not grant selected native TLS,
browser, mining/Core, client-loss, exact-board or full simulator completion.
Workspace checks and a frozen full native rerun remain the parent's next steps.

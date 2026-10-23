# Actual worker CLI carrier checkpoint379, reviewed382

This test runs the actual `md-sim-worker-ipc` executable and a real isolated
test guest. Its independent peer is a local socket endpoint in the test
process. It does not establish a support VM, archived firmware, Bitcoin mining,
SSH, TLS, native dashboard, or external physical host result.

The command passed from the repository root:

```sh
CARGO_TARGET_DIR=work/simulator/check-target-085 TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 cargo test -p md-sim-coordinator --test worker_carrier --offline --locked -- --nocapture
```

The [reviewed detailed log](../checks-2026-10-21-001/actual-worker-carrier382-reviewed-tests.log)
contains both exact process specifications, uploaded input identities, fresh
request and response bytes, complete process results, and provider reports.
The tested worker binary is 4,828,504 bytes, with SHA-256
`730be4b42cf5696fb198fa95dc0476327b6a399a77de29de2981399a23f67455`.

The test launches that executable with separate inherited control descriptor 3
and support descriptor 4. It also supplies an unrelated host file on
descriptor 77. Each isolated guest checks that descriptor 3 is a newly created
socket, with neither inherited endpoint's inode. Descriptors 4, 64, 65, 77 and the
host marker path are absent. The support endpoint never enters the guest.

Each case uploads the trusted test executable, its exact ELF libraries, and a
captured support-contract file through the worker protocol. Destination hash
verification completes before execution. A changed packet limit is rejected
without creating a run. One exact token is then accepted. Two random
challenges are generated at test time. The executing guest computes SHA-256
responses and sends them through its real private descriptor. Both directions
carry two Ethernet frames and 100 bytes. Independent test-side values match
the runtime relay and provider relay hashes.

| Case | Canonical process SHA-256 | Result | Provider closure |
| --- | --- | --- | --- |
| Complete | `1347dc4841ef65e46c828642c6dfbe7ecce03a1a79da09c8c3a4f2d0beb2c85c` | Exited with code 0 | Guest EOF |
| Stop | `fbf162f085fb7392a6b23f071e20717cd46826bdd1d7a6379b658488a572f706` | Cancelled after both responses and saved completion marker | Guest EOF |

Complete result bytes are downloaded and hashed before cleanup. The test
checks rejection of binding reuse, token lookup after cleanup, retained input
objects, worker shutdown, and the final provider report's configuration,
process, support, and traffic hashes. Root ownership remains exclusive until
the worker exits. After shutdown, the test can acquire the root lease and the
provider report remains unchanged.

The existing trusted ELF capture helper was extracted to a shared test module.
The queue test's source paths, hashes, library selection, and store import
behavior stay the same. The [full coordinator suite](../checks-2026-10-21-001/actual-worker-carrier380-coordinator-tests.log)
passes 36 tests, including 13 queue tests and this CLI test. The one ignored
harness entry is explicitly invoked twice as the isolated guest.
[Strict coordinator Clippy](../checks-2026-10-21-001/actual-worker-carrier380-clippy.log)
and formatting checks pass. Temporary stores and worker roots are removed by
owned guards. Compilation failure378 remains recorded; it required only a
parenthesized Rust cast in a comparison.

Independent review of the parent IPC fixes confirms that the owned root lease
spans provider startup, worker service, and final receipt publication. Handoff
and relay wall budgets are independent. A relay timeout causes failure.
Create-new pending files, hard-link publication, and directory fsync preserve
existing reports. The [parent unit log](../checks-2026-10-21-001/worker-carrier-cli-test-377.log)
also tests a real timeout and no-overwrite publication.

Independent review found a cancellation race in the new test. The test could
stop the guest after its second response but before its completion message.
The revised test waits at most five seconds for that message through the
worker's real saved-log API before it sends Stop. Both cases now require
untruncated traces. The affected CLI test passed again in checkpoint382, in
8.22 seconds. [Focused strict Clippy](../checks-2026-10-21-001/actual-worker-carrier382-clippy.log)
also passed. The [final independent review](worker-cli-carrier-independent-382.md)
has no unresolved finding in this test scope.

The [source and evidence manifest](actual-worker-carrier-379-inputs.json)
identifies this component checkpoint. Parent integration must still exercise
the captured support VM and the selected native firmware/service path.

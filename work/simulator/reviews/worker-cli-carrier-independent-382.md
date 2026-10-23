# Independent worker CLI carrier test review 382

Date: 2026-10-23. Reviewer: `exact_target_gap`. No open correctness finding
remains in the reviewed test and shared ELF capture helper. This tests a real
worker CLI and isolated support process. It does not test miner firmware,
mining, SSH, a dashboard or an external physical host.

The test uploads the exact locally built executable and library objects.
It supplies two fresh random challenges through the real private carrier and
checks the guest's SHA-256 responses. It verifies descriptor exclusion,
host-path isolation, wrong-process rejection before run creation, idempotent
tokens, result bytes, both relay hashes, cancellation, result cleanup, provider
receipt retention, root lease lifetime and preservation of published inputs.
Owned child cleanup also runs when a test assertion fails. The ELF helper
retains the former trusted-file capture behavior and never inspects acquired
firmware with `ldd`.

The review found a cancellation race: response delivery could occur before
the guest printed its required completion marker. Immediate Stop could kill
that print. The fixed test waits for the actual saved worker log marker before
Stop. It also requires an untruncated result. The compiled run passes both the
natural completion and cancellation cases. The ignored guest entry executes
twice inside the outer test. The focused strict Clippy check passes.

| Reviewed input | SHA-256 |
| --- | --- |
| `crates/md-sim-coordinator/tests/worker_carrier.rs` | `9ea8227f883c2cfed340cbcd1a8cd57342cca8c2655fd1aa51e6cb4939d9be6a` |
| `crates/md-sim-coordinator/tests/support/elf.rs` | `f4c4ab3f054c1747e735276f6836765cfc4a956b92f7e2c35ea56364cdc9c33c` |
| [Compiled tests](../checks-2026-10-21-001/actual-worker-carrier382-reviewed-tests.log) | `0ab0f4f4a6e0964f83293e80e89203ffb7e1a7a587040a6b02103632a512b192` |
| [Clippy](../checks-2026-10-21-001/actual-worker-carrier382-clippy.log) | `62bd76fe3d77c85921368dcae1b3cfdf0c23316b8d3957fba66e9f53deb3ce38` |

# Carrier policy preparation review400

Date: 2026-10-23. Reviewer: `exact_target_gap`. Read-only review.

The fixed S9 endpoint, adapted-native source, support identity, bounded policy
reader, and explicit wall limit are sound. `s9_boot_bundle` keeps the exact
single-read policy bytes and captures them as a new process input. The policy
does not contain its provider configuration hash, so the later optional
provider lease does not create an identity cycle. Default preparation omits
the new policy and lease.

Two findings were sent to the author before further access work:

1. `support/prepared_board.rs:193–197` validates one read from the source path,
   then `retain_metadata` opens that path again. The bytes checked as a valid
   policy can differ from the bytes captured and compared with the executed
   mount. Retain the exact bytes returned by the strict reader and compare
   their object identity with the mount before import. Do not rely on a
   pathname staying unchanged across separate reads.
2. `support/worker_channel_ssh.rs:79–90` validates the lock, but does not
   explicitly compare its process with the already loaded process-specification
   before creating the optional provider lease. The provider's later strict
   validator can reject a mismatch. Preparation should reject it at this
   boundary and retain a clear failure instead of creating a mixed selection.

Both findings were fixed and rechecked at402. No correctness finding remains
in this preparation delta. No source was edited by the reviewer. Native HTTP
readiness, lifetime coverage, provider wiring, gateway publication and browser
outcomes remain separate tests.

`dashboard_metadata` now hashes the exact bytes from the strict reader,
compares them with the executed mount, writes them with create_new, syncs,
and imports that object. It never rereads the source path for the policy.
`worker_channel_ssh::prepare` now compares the full locked process with the
loaded specification before writing the provider configuration. The affected
[tests402](../checks-2026-10-21-001/carrier-policy-tests-402.log) and
[strict Clippy402](../checks-2026-10-21-001/carrier-policy-clippy-402.log) pass.

Final reviewed hashes:

- `prepared_board.rs`: `56833b42a6d4b2ca3eca3e5afb04e68fbc4438f4c0fb6e257769a49fe58e7317`.
- `worker_channel_ssh.rs`: `6a2ec5472d81a0f7ba4733097df1c1581938c651fe37ef4f430698301b115894`.

| Initial reviewed source | SHA-256 |
| --- | --- |
| `support/s9_carrier_dashboard.rs` | `ac05c6a789a1b2c3b89d4a029e5a61af9f06b896cfebc4cc8b7ce1fe2ce70a2a` |
| `support/prepared_board.rs` | `386d67240cd8692cc757af108f8f80af969b4d83a91aebf08353efcb678cc96e` |
| `s9_boot_bundle.rs` | `8588e5e99bafccf7add85d8d38b69c2dc1dae2f8191212ff5b59459fbbd1662a` |
| `support/worker_channel_ssh.rs` | `228ac49bd43ceefaa3cb1cbf2e01b52a473a6ce8e2b3865813d23232aa0cfbc1` |

# Independent worker carrier review 368

Date: 2026-10-23. Reviewer: `exact_target_gap`. This review covers the worker
binding and trusted provider relay added after review 365. The reviewer did
not edit these runtime files. No correctness finding remains open within this
delta. The shared-pool and selected dashboard integrations need separate tests.

| Reviewed input | SHA-256 |
| --- | --- |
| `crates/md-sim-runtime/src/worker.rs` | `01834b84edcde0a62163817c00ee7d338e092a2245accfdc6dcdc23f08cba3e1` |
| `crates/md-sim-runtime/src/worker_channel.rs` | `7a6c4146450a21f7a040ac3aca5d4f28220c900354fcc8ebb9b311bb043483fa` |
| `crates/md-sim-runtime/src/private_channel.rs` | `7a3e0b08cf4d06d3cd34ecc2495489b2eaae16b589827f77dbd9128e00b8fc8f` |
| `crates/md-sim-runtime/tests/private_channel/worker.rs` | `1faf9b08542aa724ee4e08d41634014c974ab6293d647383f40a9b6d54a577bf` |
| `crates/md-sim-runtime/tests/private_channel/provider.rs` | `8680f20bb2d4a677708be21ae663babfe5f41ffb45fdf95919035100636c2332` |
| [Compiled tests](../checks-2026-10-21-001/private-channel368-worker.log) | `dce0175b2940f7890adef8986aeb09890b401a5abfc2871e35338f4da992a5c3` |

`worker.rs:855` checks the durable token before channel admission. A matching
token must have the same canonical process identity. It can return its
existing result after channel consumption, cleanup or an unconfigured worker
restart. It does not allocate a second carrier. New execution checks policy,
binding, every input object and resource admission before taking the provider
(`worker.rs:865-889`). The binding is configuration, not a wire request field.
Its identity includes carrier limits and all ProcessSpec fields.

`worker_channel.rs:37` consumes the only sender before creating run storage.
It cannot be reused after launch failure. Failure during later storage setup
also consumes the in-memory binding. That behavior is explicit in the API;
the caller must not infer a successful launch or retry with a second provider.
The run lock records consumption when run storage is available. A runtime
launch failure records it in the result. `worker.rs:924` preserves the existing
cgroup and dashboard receipt paths in the carrier execution branch. Ordinary
processes retain their prior execution branch.

`private_channel.rs:332` checks both owned provider endpoints as sockets and
sets nonblocking I/O. It uses the reviewed bounded frame relay, with an
additional explicit wall limit and cancellation flag. Its returned scope does
not claim endpoint identity, topology, application delivery or private origin.
Those properties belong to the provider's selected-run setup and evidence.
The wrapper never starts or joins arbitrary provider processes.

The compiled log reports five passed tests. Its three ignored entries are
child entry points exercised by the outer tests. New checks cover wrong input
and cap identities before run creation, unconfigured admission, one successful
carrier, token reuse, exact retained result bytes, cleanup, restart lookup,
cancellation and unavailable-provider launch failure. Provider checks cover
two-way frame hashes, peer EOF, invalid endpoint type, invalid/expired wall
limits, cancellation and oversized-frame rejection. Earlier primitive tests
remain part of this run.

These are process and carrier checks. They do not prove actual firmware work,
an external physical host, pool readiness, SSH/TLS access or a native dashboard.

## Owned-root lease follow-up 371

The reviewer also checked `WorkerRootLease` and
`serve_connections_with_channel_owned`. This is a small follow-up delta.
It uses the existing exclusive acquisition path and borrows the caller's
lease. The service does not release that lease on return. The final provider
receipt can therefore be saved before another worker acquires the root.
The ordinary convenience entry retains its scoped lifetime. No new public
wire field or source of arbitrary descriptors is added.

The compiled regression attempts ordinary acquisition before and after owned
service return. Both attempts fail while the lease is held. Acquisition works
after the caller drops it, and the provider evidence remains. The full carrier
log reports six passed tests, with three exercised child entry points.
No new correctness finding remains open for this delta.

| Follow-up input | SHA-256 |
| --- | --- |
| `crates/md-sim-runtime/src/worker.rs` | `d7b21557946624af06b3f0b17e7538d914f2d6e3b8644262d48dde0dfdb1cc5a` |
| `crates/md-sim-runtime/tests/private_channel/worker.rs` | `638e91983ed44751beb04ecf64cd6965d7ba9a49385b1439bde1eb38138ab5af` |
| [Compiled lease tests](../checks-2026-10-21-001/private-channel371-lease.log) | `78efa99fe031fec710d75d71d54e17803bd530d6aa08384c757c9d8af610184f` |

# Selected carrier access component399

Date: 2026-10-23. Author: `exact_target_gap`.

The access crate now has a separate carrier endpoint contract and an owned
loopback/Unix byte forwarder. It does not create a runtime dashboard binding
or change a worker reply. This component is ready for gateway integration.
Actual selected native HTTP, SSH/TLS publication, and browser tests remain
required. No firmware gate is granted by the component fixture.

## Contract and public interface

`Selection::new(run, policy_object, exact_policy_bytes, token)` checks the
full lock, captured policy identity, `run.policies.carrier_dashboard`, the
native process's private carrier, and the captured support identity. The
native process must have no runtime dashboard binding. Policy bytes are
bounded and hashed before parsing. The policy pins the native destination,
source label, loopback port and all connection, byte and time limits.

`ProviderBinding::from_config(selection, configuration_object, exact_bytes)`
checks the exact provider configuration, complete process, support identity,
and its optional lease's complete locked run and selected policy. That
configuration is formed after the process and lock. The policy itself points
only to the static support identity, so no recursive identity is required.
The outer caller must also verify that this exact configuration was captured
and executed by its provider.

`WorkerProviderTransport::new(transport)` sends the actual read-only
`PrivateChannelStatus { token, challenge }` request. `ProviderBinding::authorize`
also queries actual `LookupToken` and `Status`. It checks the complete process
identity, exact run/token/lock, null runtime dashboard, and active worker.
Provider replies carry a fresh random challenge, exact configuration and
identity, increasing sequence and waiting/active/closed/failed state. Only
active replies authorize access. Typed transport implementations must use
the owned authenticated control channel; they must not infer provider health
from the miner's address.

`CarrierService::start(binding, worker, provider, new_output)` checks control
state before and after creating its listeners. `refresh(worker, provider)`
renews access only with a fresh checked reply. A failed refresh revokes the
listeners. Its own thread enforces lease expiry and wall limits even when
the caller or control transport stalls. `endpoint()` returns a separate
`CarrierEndpoint`. `stop()` closes listeners, joins only owned bounded
connection threads and returns a final receipt. The new output retains
`endpoint.json` and `receipt.json`; Drop also stops a live owned service.

The forwarder uses the reviewed runtime `byte_stream` relay. It preserves
full duplex and half-close behavior. It has no HTTP parser and does not alter
native response bodies, authentication, cookies, uploads or WebSocket bytes.
Only the fixed private IPv4 destination in the captured policy is reachable.
The public TCP listener is IPv4 loopback. The private Unix socket has mode0600
inside a new mode0700 directory. Cleanup checks the owned socket identity.
The connection-attempt limit drains its last admitted connection before
stopping. A connection attempt can remain in its bounded connect call for
at most two seconds; cancellation prevents forwarding after that call.

Maximum policy limits are eight concurrent connections,4096 total attempts,
64MiB per connection,256MiB aggregate admitted reads,2s connect time,60s idle,
3600s service wall time and10s owner lease. Lower limits are accepted. Actual
stream receipts distinguish observed, admitted and written bytes. Endpoint
and receipt records always leave firmware readiness false.

## Checks and actual isolated byte execution

These commands passed with the standard target085 and TMPDIR227 environment:

```sh
cargo test -p md-sim-access --lib --offline --locked
cargo clippy -p md-sim-access --all-targets --offline --locked -- -D warnings
```

The [host tests399](../checks-2026-10-21-001/carrier-access399-tests.log)
have13 passes and one explicit VM-only ignored test. They cover changed
policy/lock/process/support/configuration, unknown fields, wrong worker
state, stale or wrong provider challenges, non-active owners, typed monitor
requests and endpoint identity. [Strict Clippy399](../checks-2026-10-21-001/carrier-access399-clippy.log)
passes. Only the existing workspace `sha2` dependency and its local lock edge
were added. No downloaded dependency or version changed.

The ignored test then ran in the actual disconnected
[VM399](../network-vm/byte-stream-399/):

```sh
/access-tests carrier::tests::vm_carrier_streams_and_revocation --exact --ignored --nocapture --test-threads=1
```

It passed two real TCP/Unix exchanges with a fresh131072-byte random payload,
partial writes and responses after half-close. It checked524288 admitted
bytes, natural draining at the two-connection attempt limit, active stream
revocation on owner closure, independent100ms lease expiry, replay rejection,
and occupied-listener cleanup. The provider and worker replies in this test
are explicitly scripted contract fixtures. Actual provider-monitor and
native-service integration are not claimed by these results.

| Source or evidence | SHA-256 |
| --- | --- |
| `access/src/carrier.rs` | `9e071fc246674b32bb3d8e677b5d16b548c6ba67c1c11c680b1c5f772904f262` |
| `access/src/carrier_tests.rs` | `4ab37fd46b29c00a72c4ed2271956d555a18ec77c81c15cc63e8b9eaccae8db6` |
| Captured access test binary | `4faa41f64737f0958e22d8c7aefb541dc04c8663f94337f80ebe360527019397` |
| Host tests399 | `657c943a449f28cadf3053a22986b68f00f4f3442a9db737ebd347d959942df9` |
| Strict Clippy399 | `70b2f26a61defc1c68fdf78cd2226e0495e3a07d94f775c84e379fe3a2eb53c6` |
| VM process specification | `6cee5a945f0fcf8c292d4442d1aebcb522cd1f655e2d9f732d8c086ffeae80a1` |
| VM process result | `952dcd2313cd8dbfec5eb9b7755c41730200b190bb926b264aadc2b1a93e2696` |
| [VM guest log](../network-vm/byte-stream-399/guest.log) | `22a1c3eae297cd0cd951e0e4af72205ddd4c74aeaf1266111d51daa8b91ac1e9` |

`private_channel` independently reviewed this component and reported no
open correctness finding after actual399 execution. Its report is separate.
The same VM independently exercises the extracted runtime stream relay;
see [the runtime review](byte-stream-independent-399.md).

Next: add explicit selected/remote/CLI carrier publication. The native worker
store is on the host, outside the support VM. Fetch small configuration and
lock objects through actual hash-checked Worker Get into server-owned storage;
do not assume their host store paths are mounted. Then test the exact new
native run through SSH, authenticated TLS and isolated browser contexts.

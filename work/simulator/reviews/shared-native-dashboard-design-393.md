# Native dashboard through the selected private carrier

Date: 2026-10-23. Author: `exact_target_gap`. Read-only design review.

Use one bounded byte forwarder in the existing support server namespace.
Bind it to server loopback and to a private Unix socket. Its only destination
is the selected miner's `10.0.2.15:80`. Keep the current SSH tunnel and TLS
gateway. Add an explicit carrier endpoint contract to the access layer.
Do not insert a synthetic `dashboard` field in native worker status.

No source was changed for this review. No new dashboard result is claimed.

## Why the existing binding cannot be reused

`runtime/src/dashboard.rs:85` defines a bridge to an isolated process's
loopback port. `Binding::verify` at line128 requires that policy and the
complete process identity. `Server::start` at line231 creates the worker's
incoming listeners. The selected carrier process has `dashboard: None` and
no host listener. Its real HTTP server is inside QEMU on the private miner
Ethernet network. A fabricated runtime binding would describe a different
path and would require a hidden change to the locked process.

`access/src/selected.rs::checked_binding` correctly requires the actual
worker's active status and runtime binding. `RemoteGateway::start` also
verifies that same binding. Both need an explicit second endpoint variant;
their current checks must remain strict for the existing runtime path.

The old local263/SSH264 runs were not successful dashboard tests. The
continuation at lines3874–3884 records HTTP status000/curl28, slow nested
execution, no browser result, and no TLS publication. Later fresh-work and
Core acceptance do not change those failed access results. The current
carrier topology avoids running the miner CPU emulator inside the support
VM. It does not, by itself, prove native HTTP readiness.

## Smallest explicit implementation

| Location | Change |
| --- | --- |
| `crates/md-sim-access/src/carrier.rs` (new) | Define `CarrierEndpoint` and a run-owned loopback/Unix byte forwarder. Verify an exact endpoint policy, the existing shared lease, actual token lookup, and actual worker status. Own listener and connection shutdown. |
| `crates/md-sim-access/src/selected.rs` | Factor endpoint authorization from TLS process ownership. Add an explicit carrier selection branch. Preserve the current runtime `Binding` branch and manifest schema. Use a new manifest schema for carrier publication. |
| `crates/md-sim-access/src/remote.rs`, `main.rs` | Add explicit carrier-selected deployment/CLI. Transfer and destination-hash-verify the lock and endpoint policy. Validate the returned carrier publication record. Keep `serve-selected-connected` unchanged. |
| `crates/md-sim-firmware/examples/remote_namespace_check/shared_pool.rs` | Reuse the existing exact selection and lease. Do not create a second device catalogue. Add only the selected dashboard policy reference and explicit lifecycle signal needed by the support service. |
| `remote_namespace_check/worker_broker.rs` | Start the owned bridge service in the server namespace after the selected `ExecuteToken` produced the actual run ID. Stop it with the selected lease and broker. Keep its records separate from worker protocol replies. |
| `remote_namespace_check/shared_run.rs` | Exercise the dashboard while `WorkerExecutor` runs. Use a separate client-side observer with its own bounded SSH control connection. Do not block artifact transfer or the retention callback. Join its outcome before final assessment. |
| `remote_namespace_check/board_dashboard.rs`, `board_browser.rs` | Reuse native readiness, HTTP, TLS and browser checks with an explicit carrier endpoint selection. Do not bypass the current `spec.dashboard` guard by inventing a runtime binding. |
| `examples/support/s9_browser_target.rs` | Add a new strict selection schema/endpoint enum for carrier publications. Validate the exact lock, run, carrier endpoint and publication, then produce the existing HTTP and TLS targets. Keep old schema1 behavior. |
| `worker_channel_probe` preparation/retention modules | Capture access, TLS and browser tools plus the endpoint policy in the support image. Retain exact bridge, access and client evidence beside the unchanged native/pool result. Include its hashes in the final outer evidence join. |

The core `GatewaySpec` already accepts a private Unix upstream and a
loopback `upstream`. It already provides mutual TLS, selected-IP certificate
checks, client address restrictions, WebSocket forwarding and streaming.
`SshTunnel::start` already enforces explicit loopback endpoints and owns its
SSH child. Neither needs a non-loopback exception for this topology.

For byte transport, reuse the bounded full-duplex stream pump in
`runtime/src/dashboard.rs:678–839` through a small extracted module or a
small explicit API. Preserve its half-close semantics, per-connection and
aggregate byte caps, connection cap, cancellation, and idle timeout. Do
not reuse the Ethernet relay: it parses four-byte frame lengths and treats
either EOF as closure of the entire carrier. That is wrong for HTTP upload
half-closes and WebSockets. Do not duplicate the whole runtime bridge.

Suggested small interfaces are:

```text
CarrierPolicy::verify(locked_run) -> checked fixed destination and limits
CarrierEndpoint::verify(locked_run, policy, shared_lease, lookup, status)
CarrierService::start(checked_selection, worker_socket, output) -> owned service
CarrierService::endpoint() -> &CarrierEndpoint
CarrierService::poll(actual_carrier_state) -> Result<bool>
CarrierService::stop() -> final byte/lifecycle receipt
SelectedGateway::start_carrier(worker_socket, run, spec, endpoint, output)
RemoteGateway::start_carrier(transport, run, spec, endpoint_policy, paths, output)
```

These are proposed signatures, not existing functions. `checked_selection`
holds the existing lock, token, canonical process and shared manifest
identities. `actual_carrier_state` comes from the checked ownership signal
described below. It is not a caller-supplied success boolean. Use typed
active/closed/fault states with their originating lease identity and time.
The service owns the forwarder and revokes it before publishing a closed
receipt. The gateway owns only its TLS process and fails closed if the
endpoint lease is no longer live. Keep selection verification in one place
so local and SSH callers apply the same rules.

The reusable transport pieces are `dashboard.rs::Direction`, `io_retry`
and `pump`, plus the cancellation/aggregate-budget portion of `State`.
`pump` is currently private and takes owned descriptors and `&State`.
Extract only those pieces behind a bounded stream-budget type. Keep
`Server::start`, token authentication, `Agent::start_from_environment`,
`Binding` and their loopback policy intact. The new carrier listener can
reuse the pump with a connected native TCP stream and an accepted local
TCP/Unix stream. Each stream pair remains owned by one bounded handler.
This avoids adding firmware-specific addresses to the generic runtime.

## Selection and endpoint contract

The new record should contain a schema, the complete `LockedRun` object,
the canonical process object, token, actual run ID, shared manifest object,
and endpoint-policy object. Add the actual loopback address, private Unix
socket, fixed native destination, limits, creation/closure times, and
`firmware_readiness_established: false`. These are endpoint facts, not a
firmware success record. Reference the existing shared lease and selection.

Before listener creation and after startup, validate all of the following:

1. The shared selection, manifest, lock and captured support identities match.
2. The lease contains the same token and actual successful ExecuteToken run ID.
3. Actual `LookupToken` returns that token, `run_id`, and canonical
   `token_process_identity`. Actual `Status` names the same active run.
   Native `dashboard: null` remains unchanged.
4. The server has exactly `eth0`, `lo`, and `worker0`, no default route, and
   disabled IPv4/IPv6 forwarding. The client has only `client0` and `lo`.
5. The selected endpoint policy permits only miner `10.0.2.15:80` on this
   carrier. A caller cannot supply another destination or a host descriptor.
6. The Unix directory is unique, owned by the service account and mode0700.
   The socket is mode0600. The loopback port is held by the owned listener;
   a pre-existing listener is never accepted as proof of startup.

The endpoint policy belongs in the complete locked run and captured support
inputs. All byte, connection, clock and wall budgets must therefore change
the resolved identity. Do not modify a prepared390 or completed391 identity
to add dashboard tests. Prepare a new exact run for this extension.

Use server `127.0.0.1:<owned-port>` as the SSH destination. Reuse client
`127.0.0.1:18090` for the selected tunnel. Publish TLS on the explicit server
address `192.0.2.1:9443` for this isolated test topology. TLS uses the private
Unix upstream to avoid port reuse. Keep the TLS run header, native cookies,
authentication and response bodies. The existing `upgrade-insecure-requests`
header adaptation is required by the archived Boser application, which
uses `ws://`; record that adaptation separately. Never edit its asset bytes.

## Lifetime is a required dependency

The service must revoke the bridge, TLS listener, open streams and SSH tunnel
on selected-run completion, cancellation, identity mismatch, broker loss,
provider loss, or its own deadline. Preserve final connection counts, byte
counts, failed attempts and termination reasons. Do not call a listener or
an empty TCP connection native readiness.

There is a concrete carrier-liveness gap to handle. In
`runtime/src/private_channel.rs:293–301`, ordinary `peer-eof` is a completed
relay result, not an error. `isolation.rs:736–751` immediately stops the guest
only for `Session::failed()`. Thus a clean provider EOF can leave the process
active until its normal wall limit. Actual worker Status alone is not a live
carrier oracle. Do not change this behavior merely to make access pass.

Keep a separate bounded live lease from the owning carrier/provider
supervisor to the support endpoint service. Reuse the existing owned control
transport where possible; report actual closure, not inferred success.
Alternatively, first prove that this pinned QEMU backend reports peer EOF
as guest NIC link-down and use that checked event. A filename or assumption
about QEMU is insufficient. Until one path is tested, provider-loss revocation
remains an open access gate. The service must also have an absolute control
request deadline; repeated partial reads must not postpone revocation.

The current shared manifest permits pool20–300s, native wall at most360s,
and runner observation of pool duration plus5s (`s9_shared_pool.rs:74–85`).
The old dashboard sequence can spend120s on HTTP,120s on TLS, then360s on
the browser. Running it unchanged can outlive this selected miner. Start
client readiness and browser preparation concurrently with mining. Measure
the native path. If the full suite needs more time, add an explicit bounded
dashboard observation policy before freezing a new run. Keep the pool's
fresh-work deadline separate. Do not silently extend the original pool or
native wall time. Configuration restart tests need new linked executions
and storage snapshots, not a stale publication surviving another run.

## Evidence and tests before a pass

- Component tests must reject changed lock, token, process, manifest, endpoint,
  stale lease, unknown endpoint schema, altered worker identity, and a missing
  active run. Verify both before/after startup checks. Keep original binding
  and gateway regression tests passing.
- Test the real bridge with randomized bytes, partial writes, backpressure,
  half-close, active streaming, bounded connection/byte limits and shutdown.
  Test an occupied port, substituted socket, failed destination, broker loss,
  provider EOF while the worker stays active, and server/client process loss.
  Check that only owned processes and sockets are removed.
- From the separate client namespace, test the SSH URL and the published TLS
  URL of the same actual run. Reject no certificate, wrong CA/certificate,
  denied client address, wrong Host/Origin, changed run metadata and stale
  publication. Record listener closure and inability to reconnect after stop.
- Reuse the actual native service tests: archived index/assets, login and
  rejection, live telemetry, supported operations, configuration write/read
  and restoration, WebSockets, streaming, redirects and applicable uploads.
  Retain native response evidence and the selected URL in each result.
- Reuse two fresh browser profiles and independent cookie stores. The current
  selected browser prepares profiles separately and has its own UID/cgroup
  checks. Two ports alone do not isolate cookies. Assert no cookies carry
  between runs or between isolated contexts.
- Preserve the native worker result, pool bundle and receipts byte-for-byte.
  Join separate dashboard records to the complete lock, live lease, bridge,
  gateway configuration, credentials identities, browser binary/profile,
  client namespaces and actual outer result. A bridge receipt, static page,
  pool acceptance or a different run's browser result grants no dashboard gate.

This is one physical host with an isolated support VM, separate server/client
network namespaces, and an independently isolated native worker. It is not
an external physical host test. External publication remains separately
incomplete if no authorized external host is exercised.

## Source identities inspected

| Source | SHA-256 |
| --- | --- |
| `access/src/remote.rs` | `fe869f58951290d25ff312c4a5ba842011162fb440d2fc8693b5a243479f959f` |
| `access/src/selected.rs` | `5d8c394e02a6a28ebef19763abd22a3aa65440cfe067e5263afac8cf754e370e` |
| `access/src/gateway.rs` | `9f0eed14d98800c6cd1c28d320cbdb95204b68682a22bbbf1405232b04112f6d` |
| `runtime/src/dashboard.rs` | `0fe0c4a66117610d899034714ae91535574cc8a2eaed3015ded7b322614fb9bf` |
| `coordinator/src/ssh_transport.rs` | `00245ae737e75936ea2e4dfd587bb76007f7997eeab1faf54feeb1c8e42f666a` |
| `remote_namespace_check/board_dashboard.rs` | `f464c5fa7329ef0216af70ab338b6fbcbf43499865de54f5b00ac244d645fd6a` |
| `remote_namespace_check/shared_pool.rs` | `50a77a83472b75987d2d94c37deeee76185e8c95dda068db491e86b0d9dc2887` |
| `remote_namespace_check/shared_run.rs` | `5f98e7ab676056b745242e4c7d0497dd85efcec07a7e810286023492bcc3e8b8` |
| `support/s9_browser_target.rs` | `223504a1ab46c6fdbbcb70b7a7fef4990e8e46a1126f6a82e4ee2c6d96a536f1` |
| `support/s9_shared_pool.rs` | `acd9ffbe595b7ae8ca1a89e870dfd0b23ccb4a5fc90a3245223bca7e64a30809` |

`private_channel` independently agreed with the explicit carrier endpoint
direction. Its review note requires owned bridge/gateway shutdown on run or
provider loss and cautions that a static IP does not establish run identity.
This design still requires implementation and independent code review.

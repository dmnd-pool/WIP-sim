# Carrier access independent review399

The private-channel agent reviewed `carrier.rs` and `carrier_tests.rs`.
No correctness finding remains in this API and service scope. This does not
establish the real provider-control, gateway, browser, or native service path.

Selection binds exact policy bytes, the complete lock, process, support input,
token, and run ID. Provider configuration binds the same process, lock, and
policy. The service does not create a runtime dashboard receipt. Its explicit
carrier endpoint still requires the separate native service tests.

Authorization checks actual selected worker lookup and status through the
caller-owned transport. It separately requires a fresh challenge and an active
reply from the owning provider. Strictly increasing sequence numbers prevent
replay. Transport calls run on the caller. The listener's own expiry timer
closes independently if the caller stalls. Failed refresh revokes access.

The service bounds active connections, attempts, byte budgets, connect time,
idle time, and wall time. It lets the last admitted connection drain when the
attempt limit is reached. Shutdown closes listeners, cancels bounded relay
threads, joins only owned threads, and keeps their error receipts. The Unix
socket path is private and checked against its original device and inode
before removal. Failed startup does not delete an unrelated listener.

The [actual VM399 test](../network-vm/byte-stream-399/guest.log) passes real TCP
and Unix transfers, fresh bytes, half-close, revocation, expiry, replay,
attempt draining, and occupied-port cleanup. The control responses in that
test are labelled scripted fixtures. The [host checks](../checks-2026-10-21-001/byte-stream400-host-tests.log)
and [strict lint](../checks-2026-10-21-001/byte-stream400-clippy.log) also pass.
The [input manifest](carrier-access-independent-private-channel-399-inputs.json)
records the exact source and evidence reviewed.

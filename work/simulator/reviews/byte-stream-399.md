# Bounded stream relay checkpoint399

The runtime now provides `byte_stream::ByteBudget` and `byte_stream::relay`.
They reuse the existing dashboard pump. The dashboard keeps its original
64 MiB connection limit, 256 MiB shared limit, 60 second idle limit, and saved
receipt fields. The new API accepts lower limits and shared cancellation.
It creates no listener and sends no caller descriptor into a guest.

Each started relay keeps actual per-direction read, write, EOF, half-close,
and pending-byte counts, including error cases. A caller must check
`require_completed()`. Charged bytes are admitted reads. Bytes rejected by a
limit are recorded as reads and are never forwarded. The owning access layer
remains responsible for endpoint identity, connection count, and wall time.

Independent review found a cancellation interval during poll. The fixed pump
checks cancellation again after poll, before it reads or writes ready bytes.
The host regression checks that no newly ready bytes pass in this case.

The final [private VM399 log](../network-vm/byte-stream-399/guest.log) executes
both explicitly selected ignored test entries. The runtime test verifies:

- Simultaneous fresh random traffic: 2 MiB forward and 1 MiB reverse. Both
  readers initially wait, which forces backpressure. All bytes match.
- A fresh response sent after the peer's real write half-close. Both streams
  finish with EOF and propagated half-close.
- Connection and shared limits after two bytes were forwarded. Each receipt
  records four bytes read, two forwarded, and the expected limit error.
- Cancellation with a blocked reader. The final receipt retains 69,312 bytes
  read, 52,928 bytes written, and 16,384 bytes pending. It reports cancellation.

The access test checks actual TCP and Unix sockets, a fresh 128 KiB challenge,
two half-close exchanges, live revocation, independent expiry, replay
rejection, occupied-listener cleanup, and draining of the last admitted
connection. Its control replies are explicitly scripted fixtures. Neither
test proves a native dashboard or actual provider-control integration.

The VM exits with code 0 and an untruncated result. Its peak resident memory
is 348,664 KiB and its measured VM wall time is 6,001 ms. It has no external
host network. The [captured input manifest](../byte-stream-vm-399-inputs/manifest.json)
identifies both test executables. The scenario uses the installed shell only
to run the two explicit test commands; all new relay and test logic is Rust.

After the last source fix, [host tests400](../checks-2026-10-21-001/byte-stream400-host-tests.log)
pass 48 tests across runtime and access. Their two ignored VM entry points
are executed above. [Strict focused Clippy](../checks-2026-10-21-001/byte-stream400-clippy.log)
and owned-source formatting checks pass. The
[independent review](byte-stream-independent-399.md) has no open finding.

The [source and evidence manifest](byte-stream-399-inputs.json) records this
component state. VM398 remains the passing pre-cancellation-fix record.
Build failures392 and395 remain explicit. They are not acceptance evidence.

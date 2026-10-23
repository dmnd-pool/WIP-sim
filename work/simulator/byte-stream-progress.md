# Bounded byte-stream extraction progress

The parent authorized a small reusable runtime byte pump for the explicit
carrier access service. The public module is `md_sim_runtime::byte_stream`.
`ByteBudget::new` takes connection bytes, shared bytes, idle seconds, and a
shared cancellation flag. Upper bounds match the existing dashboard policy:
64 MiB per connection, 256 MiB shared, and 60 seconds idle. Lower limits are
allowed. `relay` takes two owned socket descriptors and returns a receipt.
After I/O starts, errors remain in that receipt with actual read/write counts.
Callers must check `require_completed`. `consumed` means admitted reads.

The dashboard now delegates to this pump. Its default policies and serialized
receipts remain unchanged. The Ethernet carrier remains separate; this module
implements ordinary stream half-close, not QEMU Ethernet framing.

Host runtime tests394 passed 32 tests. The first actual private VM398 passed
the runtime and access ignored test entries. Its captured runtime test binary
is `c6f1c0f97c18914ec15b64edbaf7f60ab6d2467ffdb861e492a342ab37e089a7`.
The captured access test binary is
`e86bbfce5eb8013205d0e1cb9fb6a8ea96a439f2f67288b0024768eaa78a3f6c`.
The scenario and exact captured files are in `byte-stream-vm-398-inputs/`.
The actual log is `network-vm/byte-stream-398/guest.log`.

VM398 checked simultaneous random duplex traffic (2 MiB plus 1 MiB), delayed
reads that cause backpressure, a response after a real half-close, partial
connection/shared limit failures, and active cancellation. Access checks used
actual TCP and Unix sockets with a labelled scripted control fixture. They
checked fresh bytes, half-close, provider-close revocation, expiry, replay,
and occupied-port cleanup. These are component tests, not native dashboard or
firmware results.

Independent review found that cancellation during poll could allow the next
ready batch before the next loop check. The pump now checks cancellation again
after poll and before I/O. A new host regression checks zero new bytes in that
case. Runtime399 initially passed 33 tests with one explicit VM-only entry.
The test was then made robust to an already closed peer. Both runtime and
current access binaries are being rebuilt for a final VM399 rerun. Access's
final test also checks that the last admitted connection drains after its
attempt budget is exhausted.

Final399 rerun passes both captured ignored entries. Current runtime and
access host suites400 pass 48 tests; strict focused all-target lint and
owned-source formatting checks pass. Independent extraction review is closed.
The full checkpoint is `reviews/byte-stream-399.md`. The separate access
source review is `reviews/carrier-access-independent-private-channel-399.md`.
The next integration belongs to the parent: real provider liveness, native
service bytes, selected SSH tunnel, gateway, and browser tests. Component
success is not a native dashboard or full simulator completion claim.

Retained failures: check392 refused a stale Cargo lock while another agent
added access's existing sha2 dependency. Build395 found two scoped-thread
ownership errors in the new tests; they were fixed before VM398. Neither
failure was counted as a pass.

Other completed review records: actual shared native run391 and its two
Core392 checks are in `reviews/shared-execution-independent-private-channel-391.md`.
The later broker-control FIFO fix is independently reviewed in
`reviews/shared-control-independent-private-channel-393.md`. Frozen391 does
not prove that changed helper's adversarial case.

# Independent provider lifecycle review 400

No open correctness finding remains in this reviewed change. This review is
read-only. It does not establish native dashboard access or full simulator
completion.

The implementation checkpoint is `provider-liveness-399.md`. All 11 source
and log identities in its input manifest matched the files during this review.
The adjacent review input manifest also pins the unchanged IPC entry point.

The worker accepts the lease only as trusted local configuration. Its wire
request cannot install a provider, pass a descriptor, or change the selected
process. The canonical process identity includes the full inputs and limits.
The optional lease configuration checks the full locked process, captured
policy membership, and selected policy field. Existing configurations without
the optional lease retain their prior path and refuse a lease query.

The actual relay thread owns the lifecycle guard. It starts in waiting state,
becomes active after endpoint handoff, and becomes closed or failed when the
real relay returns. An unfinished guard, including thread panic, becomes
failed. This state does not come from worker process Status. The worker root
lease remains held through the provider join and receipt publication.

Lease observations require a strict selected execution token and challenge.
They check the committed token lookup, selected run ID, and full canonical
process identity. The reply contains the complete configuration and input
binding. Its sequence strictly increases. A cleanup receipt preserves the
selected identity and terminal provider observation. Read-only reconnect can
repeat the same token and challenge once after a transport interruption. It
does not retry mutations or complete worker errors.

The actual CLI398 evidence contains five real worker executions. This review
parsed every receipt, extracted the two fresh 32-byte payloads from each
Ethernet frame pair, recomputed all ten SHA-256 responses, and checked both
complete wire hashes against runtime and provider receipts. Each result is
untruncated. The observed paths are:

| Case | Worker result | Provider result | Lease observation |
| --- | --- | --- | --- |
| Legacy completion | exited | guest-eof | refused |
| Legacy cancellation | cancelled | guest-eof | refused |
| Leased completion | exited | guest-eof | waiting, active, closed |
| Peer endpoint closed | cancelled | peer-eof | waiting, active, closed |
| Oversized peer frame | cancelled | failed | waiting, active, failed |

The last two tests require the selected guest to remain booting after the
provider terminal state is observed. The oversized frame also causes a
nonzero worker CLI exit and retains its error. The test source checks result
retrieval, cleanup, preserved inputs, release of the root lease, strict query
validation, and exclusion of the actual control/support descriptors and host
paths from the guest.

The retained unit and regression results cover actual provider thread panic,
timeout failure, receipt no-overwrite behavior, strict lease configuration,
and interrupted read-only transport. Their exact logs are in the adjacent
manifest. They were inspected, not rerun by this review. The prior unsupported
shutdown syscall test remains a recorded failure; final CLI398 creates actual
EOF by closing its sole owned file endpoint.

Limits: trusted callers still own support endpoint provenance and topology.
An active relay observation alone does not prove a native HTTP service is
ready or a TCP exchange succeeded. The access service must retain its own
lease deadline and byte receipts. The five CLI cases use a local test peer,
not a support VM or external physical host. Mining, original-board boot,
native dashboard, and full target-matrix gates remain separate.

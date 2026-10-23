# Independent bounded stream review399

Date: 2026-10-23. Reviewer: `exact_target_gap`. Author: `private_channel`.

No correctness finding remains in the reviewed byte-stream extraction and
dashboard delegation. This is a component review. It grants no firmware,
native dashboard, SSH, TLS, or exact-board gate.

The extracted `ByteBudget` preserves the existing dashboard limits:
64MiB per connection,256MiB shared admission,60s idle. Lower limits are
explicit. The relay owns both descriptors, checks socket file types, sets
nonblocking/CLOEXEC, and uses bounded buffers. The caller must establish
stream type, peer identity, connection count, and overall lifetime.

The review checked independent directions, delayed half-close propagation,
backpressure, ignored descriptors when no event is needed, shared atomic
admission, idle expiry, and cancellation. Receipt counters distinguish bytes
read, admitted, written, and pending. A read rejected by a byte limit is
retained in observed counts without being admitted or forwarded. A started
relay retains an error receipt instead of turning a partial exchange into
success. Existing dashboard accounting and bridge policy remain separate.

Review found one cancellation interval. Cancellation could arrive during
`poll`, followed by ready data. The loop checked cancellation only before
polling and could forward that ready batch. The author added a check after
poll and before I/O. A regression now sets cancellation while the relay
waits and verifies that newly ready bytes are not consumed or forwarded.

The exact ignored runtime test ran in a captured disconnected Linux VM at
[byte-stream399](../network-vm/byte-stream-399/). It passed simultaneous
randomized 2MiB and1MiB duplex traffic with delayed reads, a response sent
after the client half-close, partial byte-cap failure receipts, and active
blocked-writer cancellation. This test uses real socket send/shutdown
operations. Host-only tests are not counted as half-close evidence.

The same VM also ran the access agent's real TCP/Unix component fixture.
That result is separate from this independent runtime review. Its scripted
control replies do not prove actual worker/provider integration.

| Reviewed source or evidence | SHA-256 |
| --- | --- |
| `runtime/src/byte_stream.rs` | `a1bd7efd475ad19fce8c093024dbda5b987da3080a87e4b5423263780632b424` |
| `runtime/src/dashboard.rs` | `d2063543dfe760e9beeac17350e1203a17517df87e15a50895d83cf95c6e613f` |
| Captured runtime test binary | `6dcecaf14795cfd5bd220603c4e2bc380a12d99bcbdb51b7d49d673cb629ef7d` |
| VM process specification | `6cee5a945f0fcf8c292d4442d1aebcb522cd1f655e2d9f732d8c086ffeae80a1` |
| VM root manifest | `b215d84719d557ca6088ab95701e274e829c6c474535e60327455a83dc17880c` |
| Actual VM process result | `952dcd2313cd8dbfec5eb9b7755c41730200b190bb926b264aadc2b1a93e2696` |
| [Actual guest test log](../network-vm/byte-stream-399/guest.log) | `22a1c3eae297cd0cd951e0e4af72205ddd4c74aeaf1266111d51daa8b91ac1e9` |

The exact invocation inside the captured VM was:

```sh
/runtime-tests byte_stream::tests::vm_full_duplex_half_close_and_backpressure --exact --ignored --nocapture --test-threads=1
```

The [captured input manifest](../byte-stream-vm-399-inputs/manifest.json)
and [scenario](../byte-stream-vm-399-inputs/scenario.json) pin the executable
and command. The assessment explicitly leaves firmware, external-host,
worker-transport, and SSH/TLS gates false. Earlier398 evidence used the
pre-fix relay and remains historical;399 is the reviewed rerun.

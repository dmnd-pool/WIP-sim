# Private Ethernet stream

This development adapter connects two QEMU network devices through a connected
Unix stream socket pair. The launcher creates the pair. It does not create a
host listener or attach a host network interface.

The QEMU source is archive SHA-256
`2a40e6705e59fd8807012631bc6b2c0877f10481b12768f78e4f9d7b88376cce`,
commit `11661d857254b27fca5568c6deacf4f6b6f3460f`.
The baseline files are:

| File | SHA-256 |
| --- | --- |
| `net/socket.c` | `182194a1b5c0ffb8f94ff5dab41647ac0775215fcb6d58c0aa6d7fd1bbc5d87d` |
| `qapi/net.json` | `01f716c110d0b5f4cc21468cf76b32c4da2bf3658b0bc405e5c65dfea8727f08` |

[The patch](private-stream.patch) adds the explicit option
`socket,fd=3,x-md-sim-private-stream=on`. The option requires a socket
descriptor. The launcher must supply a connected `AF_UNIX/SOCK_STREAM` pair.
The adapter uses `read` and `writev`. It retains QEMU's length prefix, packet
parser, partial writes, queues, backpressure, and disconnect handling.
The normal socket modes remain available. They keep their normal type checks.

The adapter does not query or invent socket addresses. The run specification
and launcher bind the descriptor to the private pair. A descriptor cannot
select a listening address through this option. No CPU or guest code changes
are part of this adapter.

The restricted host rejects socket inspection calls. An unchanged QEMU stream
backend started but did not exchange packets in private VM link tests 001
and 002. The retained packet captures show requests with no replies.
The source records are in
[the captured network sources](../../../work/simulator/qemu-fd-source-002/references.json).
This is the reason for the explicit local I/O adapter.

The UBSan x86 build and [private VM link test 004](../../../work/simulator/qemu-s9/private-vm-link-004/assessment.json)
pass. Two actual Linux guests exchange TCP through this adapter. The test
checks two separate Stratum sessions, reconnect, a separate binary probe,
and rejection of invalid submissions. It retains both Ethernet captures.
It does not perform mining or use an external physical host.

The test command was:

```sh
target/release/examples/s9_pool_link /tmp/md-sim-private-vm-link-004 /tmp/md-sim-qemu-pool-003-ubsan
```

Use a new output directory to repeat it. The backend binary is SHA-256
`a6b79923e6e111041a7197bf22f9080c4ec125bd64edf8398b47d98ed4505ad2`.
The patch is SHA-256
`e075755129de7c70a19a19aaf1807eb952981c233f28c65bee289aebebe1d0aa`.
The test input store captures the binary, its libraries, BIOS, kernel,
module, pool executable, and frozen pool inputs.

The adapter is C because it extends QEMU's existing C network interface.
Pool services, launchers, evidence processing, and device models remain Rust.
The [miner integration audit](../../../work/simulator/qemu-s9/sd2022-private-pool-028/submission-audit-002.json)
links two pool-accepted shares to actual archived firmware and ASIC traffic.
Exact physical target and full-suite gates remain open.

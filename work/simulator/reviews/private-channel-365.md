# Private Ethernet carrier checkpoint365

This is transport primitive evidence. It is not a firmware, mining, SSH,
dashboard, external host, or exact device conformance result.

The Rust runtime creates two socket pairs. One endpoint enters the isolated
process as descriptor3. A trusted provider receives the other endpoint through
a bounded `SyncSender<File>`. The runtime accepts no caller descriptor or host
address in the process specification. Normal execution rejects a declared
channel without an explicit provider. The optional field is absent from the
default serialized specification and result.

The carrier uses the existing QEMU four-byte big-endian Ethernet length prefix.
Each direction has independent frame, wire byte, and packet limits. A single
nonblocking relay keeps at most one pending frame per direction. The result
records observed and forwarded bytes, SHA-256 hashes, packet counts, pending
bytes, and the reason for closure. EOF disconnects the carrier. TCP half-close
is carried inside Ethernet packets. This is not a general stream half-close
implementation. Queued kernel bytes that the relay has not read are outside
its receipt. A successful process exit does not prove delivery to the peer.

The installed Bubblewrap 0.13.0 has no `--preserve-fds` option. Its
[source](https://raw.githubusercontent.com/containers/bubblewrap/v0.13.0/bubblewrap.c)
passes inherited descriptors to the guest, and closes extras in its monitors.
The runtime marks all descriptors from3 upward with
`close_range(CLOSE_RANGE_CLOEXEC)`, then duplicates only the new private socket
to3. A denied call fails the launch. A channel run lets `unshare` execute
Bubblewrap directly. The optional `unshare --fork` monitor would retain the
socket and delay guest EOF. Default runs keep their previous monitor path.

Run from the repository root:

```sh
CARGO_TARGET_DIR=work/simulator/check-target-085 TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 cargo test -p md-sim-runtime --test private_channel --offline --locked -- --nocapture
CARGO_TARGET_DIR=work/simulator/check-target-085 TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 cargo test -p md-sim-runtime --all-targets --offline --locked
CARGO_TARGET_DIR=work/simulator/check-target-085 TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 cargo clippy -p md-sim-runtime --all-targets --offline --locked -- -D warnings
```

The detailed [run365 log](../checks-2026-10-21-001/private-channel365-evidence.log)
contains nine carrier run specifications, component hashes, guest logs, and
typed receipts. It passed two host integration tests. Three ignored harness
entries are guest/provider entry points. The host tests explicitly execute
them as separate processes. They are not excluded requirements.

The tests cover fragmented traffic in both directions, exact wire hashes,
provider EOF, an oversized packet, a truncated packet, packet and byte limits,
timeout, cancellation, blocked readers, and host-path exclusion. An unrelated
host descriptor without CLOEXEC is excluded from both default and carrier
runs. The closed-peer backpressure test holds one complete pending frame. It
used 145,197 microseconds of host CPU during a 1,012 millisecond timeout. Its CPU
limit assertion is 400,000 microseconds.

The supervisor-loss test kills the actual Rust supervisor with SIGKILL while
the guest and a forked descendant are live. It observes four guest-side
processes stop. A separate host provider process first receives a real frame,
then observes EOF after the supervisor dies. The frame SHA-256 is
`d1389923d0ab7d1eb71556733e6adb5276ad110c78bf54b6b1bd7eabd0797c25`.

Independent review found a host CPU spin when `poll` observed HUP on a socket
with no requested events. The fix omits that descriptor until I/O can progress.
The compiled backpressure test covers it. Review also required the actual
supervisor-loss test and corrected provider-topology wording. The reviewer
reported no remaining correctness findings in this primitive scope.

The subsequent [runtime test log](../checks-2026-10-21-001/private-channel366-tests.log)
passes 50 tests. The [Clippy log](../checks-2026-10-21-001/private-channel366-clippy.log)
records the applicable lint check. Existing constructors received only a
default `private_channel: None` field. Formatting was applied to the workspace.
An accidental separate 40 MiB debug cache created during the first check was
removed. The shared check cache, release binaries, original archive, and
canonical firmware inputs were retained.

The [next checkpoint](private-channel-370.md) adds a provider bound to one exact
canonical process identity, with admission, reuse, token, result, cancellation,
and cleanup tests.
Parent integration must provide independent evidence for the support VM peer,
its topology, and the native miner service path.

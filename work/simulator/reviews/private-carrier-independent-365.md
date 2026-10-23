# Independent private Ethernet carrier review 365

Date: 2026-10-23. Reviewer: `exact_target_gap`. The reviewer did not edit
the implementation or tests. No correctness finding remains open within
the reviewed primitive. Firmware and remote integration need separate tests.

## Reviewed identity

| File | SHA-256 |
| --- | --- |
| `crates/md-sim-runtime/src/private_channel.rs` | `b00e7e4cc1ac3eb7fcd1221892acd7f311b9a2189aeae99d3d2742e064ddb52b` |
| `crates/md-sim-runtime/src/isolation.rs` | `bcab9c459928aa21801fe62e30fcdb5d7f594a3eb1b85a31153ab179e5b2df40` |
| `crates/md-sim-runtime/tests/private_channel.rs` | `2e68b6ffb80552f1ff9e0f30f204715aaab0637b7e3aab39c872abe1e87d1d3a` |
| [Compiled test log](../checks-2026-10-21-001/private-channel365-evidence.log) | `7f2db5643bf1d5ef434c26fcb4ee419ff9ffdb124e21b2fe94dd8b9f7bb0bbf5` |

## Findings and resolution

1. **Host CPU spin under backpressure.** The original `bridge` poll array
   kept a descriptor active when its requested event mask was zero. Linux
   can still return `POLLHUP`. A closed source with a complete frame waiting
   for a blocked destination could spin the relay. The reviewer confirmed
   this kernel behavior with a bounded socket-pair check. The author now
   sets an uninterested poll descriptor to -1. The compiled regression
   checks a complete pending frame, not only partial buffered bytes. It
   measured 145,197 microseconds of host CPU across a 1,012 ms timeout,
   below its 400,000 microsecond bound. The receipt records 50 complete
   received packets, 49 forwarded packets, and 2,052 pending bytes.

2. **Supervisor-loss coverage for the changed launch path.** Carrier mode
   removes the `unshare --fork` monitor to prevent a retained descriptor
   from hiding EOF. Existing cancellation tests did not prove abrupt
   supervisor-loss cleanup. The new compiled test kills the actual Rust
   supervisor with SIGKILL after a real frame crosses the carrier. Four
   observed guest processes, including a forked descendant, become dead.
   A separate host provider process holds the runtime-created endpoint and
   observes EOF. Its received frame hash is
   `d1389923d0ab7d1eb71556733e6adb5276ad110c78bf54b6b1bd7eabd0797c25`.
   Cleanup guards restrict signals to owned PIDs with matching start times.

3. **Isolation wording.** The previous unconditional result text claimed
   no egress. The runtime cannot prove the topology of trusted provider
   code. Carrier results now state the narrower proven boundary: no
   inherited host network interface or route, a runtime-created private
   carrier, and separate evidence required for the provider's topology
   and lifetime. The default result text remains unchanged.

## Other checks and limits

The two-direction framing state, independent byte and packet budgets,
bounded frame storage, and receipt arithmetic are consistent. The policy
does not accept a caller-selected host descriptor or address. An absent
policy is omitted during serialization. The launcher marks non-stdio
descriptors close-on-exec and explicitly preserves only the private
descriptor 3. Tests cover an unrelated inheritable descriptor and the
ordinary no-carrier path.

The reviewed log has nine actual carrier runs. Duplex transfer and peer
EOF complete successfully. Oversized frames, partial EOF, packet limits,
and byte limits fail explicitly. Backpressure times out or cancels within
the declared bounds. Forced termination closes and joins the owned
nonblocking relay before killing the guest. It never joins provider code.
The suite reports two passed tests and three ignored helper entries.
Those helpers run as actual child executables inside the two outer tests.

Relay receipts describe observed reads and writes. They exclude unread
kernel queues and do not prove application delivery. An exited process
with an error-free carrier receipt must not replace independent peer
receipts, pool verification, native dashboard tests, or exact-target
acceptance. No QEMU integration, external physical host, SSH/TLS path,
aggregate cgroup resource limit, or original-board gate follows from this
primitive review.

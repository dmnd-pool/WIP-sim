# Native worker channel author review

Status: component passed. This is an author review. Independent agent review
is unavailable because of the recorded usage limit. Milestone 2 is incomplete.

The native worker and its client VM run beside each other in a closed support
sandbox. The worker receives structured requests through an inherited private
stream and QEMU's upstream virtual serial device. The VM checks the port name
and device identity. No host listener or host directory is shared with the VM.
The pinned minimal backend does not have optional libseccomp support. The
runtime provides separate namespaces, read-only captured files, dropped
capabilities, no-new-privileges, scratch storage and resource limits.

Test334 passes a fresh 1 MiB upload, destination hash verification and complete
download. The client then transfers an executable process specification and
its inputs. The actual worker runs that process in another private sandbox.
The process hashes the uploaded challenge. The client retrieves the complete
result, verifies its hash and checks the computed challenge hash. Cleanup must
acknowledge that exact result. Worker shutdown must also acknowledge the request.
No nonce, miner service or dashboard is synthesized by this test.

The first process test330 failed before execution. Linux denied its nested
user mapping. The failure and exact retrieved result remain in the logs. The
outer worker now uses an explicit non-root identity, UID/GID 1000. Its jobs keep
their normal private root mapping. The new optional `namespace_uid` field is
part of the process identity. Default zero is omitted from serialization, so
old process specifications keep their wire representation. The original
root-guest confinement restriction remains.

The non-root check also requires zero permitted, effective and bounding
capabilities and no-new-privileges. Bubblewrap can create an intermediate user
namespace. A mapping's parent number is relative to that namespace, not a
global host UID. Test331 first assumed otherwise and failed. The final test
checks the actual UID/GID, capability sets, one mapped identity, locked root
and writable scratch. The failure was an incorrect mapping expectation, not
a permission granted to the worker.

Review found that `Command` kept the VM's configured stream descriptors after
spawn. A failed VM could leave the worker waiting for EOF. The parent now drops
that command after spawning. The complete error path still needs an explicit
disconnect regression. Test330's outer timeout remains a failure.

Formatting, strict workspace Clippy and all 616 Rust tests in 105 test binaries
pass at checkpoint334. The support backend build uses undefined-behavior
checks. Its smoke test realizes the actual upstream virtual serial device.
No new C device implementation was added.

Remaining work: an explicit disconnect regression; original miner execution
on this topology; actual SSH transport; selected-run tunnel and authenticated
TLS dashboard tests. The support worker's current 110-second CPU limit and
64 MiB file limit are not sufficient for the complete prepared miner bundle.
Any larger support profile must be declared and tested. It must preserve the
miner's own limits and clocks. No external physical host was tested.

Evidence:

- [Support backend build](../qemu-s9/backend-build-328-pool/build-report.json).
- [Transfer-only run329](../network-vm/native-worker-channel-329/assessment.json).
- [Failed process330](../network-vm/native-worker-channel-330/assessment.json).
- [Successful process332](../network-vm/native-worker-channel-332/assessment.json).
- [Final component334](../network-vm/native-worker-channel-334/assessment.json).
- [Frozen-helper repeat335](../network-vm/native-worker-channel-335/assessment.json).
- [Source checkpoint](../qemu-s9/core-checkpoint-334/manifest.json).
- [Helper checksums](../channel-target-334/manifest.sha256).
- [Full Rust tests](../checks-2026-10-21-001/channel334-tests.log).

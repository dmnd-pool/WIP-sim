# Independent namespace process-limit review 466

No blocking finding remains in the reviewed runtime and coordinator change.
The earlier Core attempts correctly remain failed before node execution.
The static inspector fix alone did not fix the generic runtime's same
pre-bootstrap NPROC ordering defect.

The runtime now creates its user namespace before trusted prlimit lowers
NPROC to the unchanged ProcessSpec value. prlimit then execs Bubblewrap.
No acquired program executes before the declared bound is active. The
remaining CPU, address-space, file-size, descriptor and core limits, parent
loss handling, process-group ownership, no_new_privs, private namespaces,
locked mounts and wall/output limits remain in their existing paths.

The carrier path still omits unshare's extra fork monitor. prlimit execs
without adding a monitor which could retain private FD3 and conceal EOF.
The real private-channel suite checks socket identity, fragmented duplex
traffic, leaked-descriptor rejection, close behavior and supervisor loss.
The real worker-carrier suite checks complete, cancel and leased cases.
Those tests passed after this change.

The new prlimit identity appears in runtime ProcessResult tools, doctor
output, local executor identity and remote worker identity. Both local and
worker execution require exact equality with the locked tool map. A prior
two-tool lock therefore cannot silently reuse the current three-tool
executor. Historical result schemas remain readable; their frozen binaries
and prior evidence do not acquire new execution claims.

The new actual regression creates 24 owned host sleepers and runs the real
isolated process with NPROC 16 under both namespace UID 0 and UID 1000. It checks
host-file exclusion, one successful guest child, exact soft/hard 16 readback,
complete output and the recorded prlimit hash. The owned sleepers are killed
and reaped. This passed. The isolated-worker suite passed 15 tests. The private
channel suite passed 6 tests with 3 guest-entry helpers ignored in the ordinary
invocation. The worker-carrier suite passed 1 test with 1 guest-entry helper
ignored. Those helper entries execute inside the successful parent tests;
their ignored status alone is not execution evidence. Strict runtime and
coordinator Clippy and the new Core-helper release build also passed.

This review covers the runtime ordering and identity change, not subsequent
Core acceptance. Fresh node import/restart results are still required. The
adjacent input manifest pins the reviewed files and retained check logs.

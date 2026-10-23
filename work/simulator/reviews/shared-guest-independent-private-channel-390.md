# Shared guest wiring review390

The private-channel agent reviewed the support VM guest wiring, selected pool
lifecycle, and joined retention exchange. The review covers source behavior.
Actual native firmware and pool execution require the separate integration
run.

The support supervisor moves the declared e1000 NIC into the server network
namespace. The server also has the private SSH link. The client sees only its
SSH link and loopback. Interface, address, route, module, and forwarding checks
bind the declared topology. The pool runs in its own mount, PID, IPC, and UTS
namespaces inside the server network namespace. It drops to UID 1000 before
accepting mining work. It has private scratch space and finite resource limits.
The IPv6 checks distinguish an absent kernel subsystem from enabled IPv6 with
a forbidden default route or forwarding policy.

Only the exact selected process and token can acquire the pool lease. The
broker waits for the actual pool's endpoint, input, and executable readiness
record before it sends ExecuteToken. Stop applies to the selected run and
signals the independent pool. Pool sessions check cancellation with finite
socket timeouts. The supervisor has a bounded graceful stop and then kills
and reaps its owned child.

The client downloads the native result before requesting retention. The
supervisor waits for both the native request and the pool outcome. It sends
the exact combined native and pool receipt to the host. Cleanup occurs only
after the host returns that same receipt. A failure keeps its own outcome
and available artifacts. The implementation does not turn a pool or transport
failure into a mining pass.

`isolation::save_json` uses a random create-new temporary file, complete JSON,
file synchronization, atomic rename, and directory synchronization. The
lease, ready record, pending result, expected receipt, acknowledgment, and
error records all use it. Existence polling does not observe a partial JSON
publication.

Two ownership findings were fixed before the source freeze:

- Root used path-based ownership changes for a cancellation file staged in
  pool-owned scratch. Replacing that entry could redirect the ownership
  operation. Cancellation now stages in root-only `/work/pool-control`, sets
  ownership through the open descriptor, and replaces the destination entry
  with rename. It does not follow a destination symlink.
- Root reopened the pool stdout path below a broker-owned directory. A
  replacement could supply a false readiness line. `/work` now stays owned
  by root. The broker owns only `/work/broker` and `/worker`. Readiness uses
  a retained, independently opened descriptor that matches the child's
  original stdout inode. The final pool bundle checks both log identities.

The readiness reader's same-open-file checks already reject symlinks,
nonregular files, non-root owners, multiple links, writable group/other bits,
and oversized files. These checks complement the protected source log; they
do not replace it.

The [final helper tests](../channel390b-shared-tests.log) pass 79 tests, with
two child entry points invoked by their outer descriptor tests. The new
real-file checks verify independent writer and reader offsets, retention of
the original inode after path replacement, cancellation without changing a
symlink target, and rejection of symlinks and writerless FIFOs.
[Strict focused Clippy](../channel390b-shared-clippy.log) passes.

The parent pool-service reader's [17 focused tests](../checks-2026-10-21-001/shared-pool-cancel-tests-390.log)
and [strict Clippy](../checks-2026-10-21-001/shared-pool-cancel-clippy-390.log)
also pass. Those tests check the exact cancellation content, owner, mode,
link count, size, symlink rejection, and nonblocking FIFO rejection. They
preserve the production requirement for UID 1000.

The independent review has no open correctness finding in this scope. The
source is ready to freeze for the actual shared native execution. All seven
helper source hashes match the implementation checkpoint390. The
[review input manifest](shared-guest-independent-private-channel-390-inputs.json)
also records the pool reader, pool executable source, atomic JSON helper, and
test logs. This approval does not establish a firmware, mining, dashboard,
external host, or full target pass.

# Independent runtime process-limit review 466

The root-authored runtime correction has no unresolved finding in this review.
It creates the user namespace before applying the declared process bound.
Trusted `prlimit` then executes Bubblewrap without an added monitor. The
untrusted guest still starts after the bound applies. CPU, address-space,
file-size, file-descriptor and core limits remain in the earlier protected
setup. No-new-privileges, capability removal, network separation, closed
mounts and owned cleanup remain enabled.

The default path retains its existing unshare fork monitor. The private
carrier path still avoids that monitor, which would retain descriptor3 and
hide guest EOF. The inserted `prlimit -- COMMAND` executes the next program;
it does not add that monitor. Runtime results, local coordinator identity and
worker doctor/probe identity now pin the prlimit executable. The actual live
worker test adds this third tool to its new locked run. Historical locked
runs and captured results were not rewritten.

The reviewer read the source assertions and independently counted the retained
logs:15 isolated-worker tests,6 private-channel tests and1 actual worker-CLI
test passed, with no failures. Four ignored entries are explicit guest/provider
entry points exercised by their parent tests. An additional focused busy-user
run passed the new test again; it is not another distinct coverage case.

The busy-user test keeps24 owned host sleeper processes active. For both
namespace UID0 and UID1000, an actual isolated shell starts an external true
child, finds no host home or passwd file, and reads both process limits as16.
Its exact output is asserted. It also verifies the prlimit hash recorded in
ProcessResult. The other executed tests check dropped capabilities, preserved
no-new-privileges, hidden host descriptors, bounded traces, timeout and
cancellation cleanup, inherited carrier descriptor3, exact framed bytes,
real guest and peer EOF, and descendant teardown. The actual worker-CLI test
checks provider identity, selected process, trace retention and acknowledged
cleanup. Strict runtime/coordinator Clippy passed.

The [input manifest](runtime-process-limit-independent-466.json) pins the
reviewed source and logs. This reviewer also added the required prlimit
capture entries to six firmware examples. Those additions are listed for
provenance, but this report does not count the author as an independent
reviewer of them. A common refreshed build and actual selected-run execution
must still check the resulting closed input sets.

This is a process-runtime component result. It grants no firmware, native
browser, Bitcoin Core, original-board or exact-target acceptance. The root's
separate source-tool launcher corrections remain under their own review.

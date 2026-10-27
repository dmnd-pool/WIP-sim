# Independent ELF inspector review 464

No blocking source finding remains in the reviewed lab.rs change. It fixes
namespace setup failing before the parser starts when the real host UID
already has more than 16 processes. This does not change a firmware verdict.

The change removes only RLIMIT_NPROC from the trusted bootstrap's pre_exec
limits. The fixed command inside bwrap is now trusted prlimit, followed by
readelf and the fixed read-only /input path. prlimit sets the soft and hard
NPROC values to 16 before readelf can parse the selected input. The reviewer
independently checked the installed prlimit option semantics: both limits
report 16. This remains a per-real-UID limit, not an isolated process quota;
unrelated host activity can leave fewer available parser children.

CPU 5 seconds, address space 256 MiB, file/core size 0, no_new_privs, the private
user/network/mount/PID setup, capability removal, read-only mounts, 10-second
wall deadline and 1 MiB stdout bound remain in place. The parser remains in the private preparation
filesystem with no network access. The bootstrap executes trusted
system programs only before the NPROC limit is set.

The new stderr reader retains at most 64 KiB plus one overflow byte. A failure
shows only the first 2048 input bytes as a lossy UTF-8 diagnostic. Output and
exit checks remain required. Both readers drain concurrently, so a small
stderr pipe cannot deadlock a parser which also writes stdout. The existing
owned process-group guard still handles timeout and error cleanup.

The explicit regression ran one actual test, with zero ignored. It creates
24 owned trusted sleepers, checks the real isolated readelf path, then checks
that malformed input is rejected with a readelf diagnostic. Its guard kills
and reaps every owned sleeper on return or error. The test log passed in 0.06 s.
Strict Clippy and the new Core-helper release build also passed. This review
inspected those logs; it does not count a zero-test filter as execution.

The earlier Core attempts failed before node execution. They remain failures.
Fresh Core acceptance/restart runs are still required. Source and check-log
hashes are in the adjacent input manifest.

A separate runtime path still sets NPROC in pre_exec before namespace setup
(`md-sim-runtime/src/isolation.rs`). Core declares 32 there. The reviewer flagged
that path to the parent because the same host-UID pressure can affect it.
This source review closes only the static inspector fix. Actual Core execution
must resolve or disprove the separate runtime concern with retained evidence.

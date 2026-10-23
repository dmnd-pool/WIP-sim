# Process namespace correction

Status: local, worker and SSH component checks pass after correction128.
Firmware validation remains in progress. This is not a release pass.

The SSH122 firmware failure showed an `nginx` process in the diagnostic for the
emulator PID. That process belonged to the enclosing test VM. The old process
launcher created its PID namespace with `unshare`, then asked Bubblewrap to
provide `/proc`. Bubblewrap could use the existing parent proc filesystem.

A new real-process test reproduced the mismatch: the shell reported PID2,
but `/proc/self/stat` reported PID67. See
[the failed regression](../checks-2026-10-21-001/proc-namespace-regression-before-001.log).

Adding `unshare --mount-proc` corrected those numbers. A stronger test then
found a file outside the sandbox through a process root in `/proc`. That
intermediate change was rejected. See
[the second failure](../checks-2026-10-21-001/proc-namespace-regression-after-002.log).

The current launcher lets Bubblewrap create the PID namespace and proc mount
together with `--unshare-pid`. The enclosing `unshare` still creates the user,
network and mount namespaces and retains its child termination policy. The
regression checks matching process IDs and denies access to a test file outside
the sandbox through every visible process root. Both local and private-worker
executions pass. See
[the corrected regression](../checks-2026-10-21-001/proc-namespace-regression-after-003.log).

The firmware guard now checks the process ID before acquired code starts.
The serial supervisor also stops reading a child PID after it has reaped that
child. Old `owned_emulator_proc` records must not be attributed to the emulator
without this correction.

Earlier firmware, mining and transport evidence is preserved. It does not
establish the required isolation gate. The ongoing SSH126 and local125 runs use
the old launcher and cannot close that gate. No exact target was complete before
this finding. The complete Rust suites, real SSH test, and affected firmware
paths must run with the corrected launcher before isolation can be accepted.

Correction128 applies the same PID fix to the ELF inspector and both source
preparation launchers. The frozen cgminer build replay passed and produced the
same executable hash. The installed-toolchain build and C undefined-behavior
checks also passed. See [the replay](../checks-2026-10-21-001/frozen-preparation128.log)
and [the installed build manifest](../firmware/prepared-installed-proc-128/input-manifest.json).

The first SSH retest failed before the test program ran. The VM kernel refused
a nested proc mount below Bubblewrap's protected parent proc submounts. The
trusted VM management roles now mount a fresh proc filesystem after Bubblewrap
creates their PID namespaces and separate roots. Only the trusted management
program uses that environment. The worker drops to UID1000. Each firmware
process still gets the strict inner runtime and its protected proc mount.
Both management roles check their PID view and deny access to orchestrator
paths through process roots. This is a local disconnected VM test, not an
external physical host test. The [actual SSH128 report](../network-vm/ssh-isolation-128/remote-report.json)
passed artifact, process, live-log, reconnect, cancellation and cleanup checks.

The original firmware in local127 passed its process guard and started its
native miner. It completed no pool session. The host denied libslirp's internal
TCP socket creation. See [the retained process result](../qemu-s9/local-pipe-127/result.json).
This gives no mining gate. The two old nested firmware attempts also failed:
local125 timed out, and SSH126 exited after its emulator was killed. Their
original output remains unchanged.

Corrected source131 now runs archived S9 firmware through the original SPL,
U-Boot and kernel in its declared adapted environment. Native HTTP, damaged
update rejection and miner stop/start pass. Independent verification checks
fresh work before and after restart. Four separate Core nodes accept the
resulting blocks and retain the height change after restart. See
[the process result](../qemu-s9/native-batch-131/result.json),
[first session](../qemu-s9/native-batch-131/regtest-audit-session-0.json) and
[restart session](../qemu-s9/native-batch-131/regtest-audit-session-4.json).
These runs use the corrected guard and launcher. Their reference hardware,
filter, clock, monitor-script and console adaptations remain explicit.
They do not establish an exact physical board, remote miner or release gate.

The regression command is:

```sh
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=work/simulator/check-target-085 cargo test -p md-sim-coordinator --test queue_behavior proc_view_uses --offline --locked
```

# Adapted S9 milestone review

This is an author review. Independent review is still open. All three review
agents report a usage limit until 25 October 2026 at 08:48. This review cannot
close the full specification's independent review gate.

## Command and execution boundary

The new Rust command is `md-sim demo s9`. Its embedded manifest names the
frozen cycle263 helper and 31 primary input objects. It checks their SHA-256
hashes and lengths before execution and again before a passing assessment.
The helper records and checks its transitive inputs. The command requires a
new output directory. It retains the manifest, invocation, CLI hash, helper
logs and cycle assessment hash. It rejects an existing output directory and
missing prerequisites. These cases were executed in checks280.

The helper launches firmware and preparation tools through the existing
isolated worker path. The wrapper does not execute downloaded firmware on the
host. It passes arguments directly to the trusted Rust helper. It does not
build a shell command from user input. Output-path creation errors are command
errors; they cannot become passes.

The frozen helper requires complete process traces, successful process exit,
the three original boot-stage messages, native restart observations and linked
disk identities. The fresh-work audit reconstructs headers independently.
Each boot must produce two blocks accepted by separate Core nodes. Each node
must retain height one after restart. A boot message alone cannot pass.

The native browser uses the firmware's services and application. Browser and
API records must match the captured guest output and process identity. The
cycle checks configuration write, restart readback, restoration and another
restart readback. It also requires the signed installation result. Cookies
and browser profiles are private to the run.

This remains the declared adapted S9 reference environment. The command
provides no host or remote browser URL. Exact FPGA, component, remote and full
matrix gates remain open. The new backend280 is separate from the unchanged
backend263 used by this demonstration.

## Fabric clock and reset changes

Backend280 retains fractional clock periods and propagates FCLK1 to the
legacy S9IO model. Mid-byte gating preserves the UART and FIFO state. Elapsed
gated time does not become extra clock edges. The independent serial oracle
checks all three links, byte values, exact fractional timing and FCLK0
independence. Backend280 passes; the fixed-clock backend276 fails that oracle.

The four raw reset outputs respect the SLCR lock. System reset clears the
throttle control and gate registers. The reset regression passes two actual
QMP system resets; backend263 fails because it retains a programmed gate.
The fabric UART, fabric reset, watchdog and TTC reset tests also pass on the
undefined-behavior-sanitized build. Legacy protocol and ASIC tests pass.

The reset sequencer is not connected. Counted throttling stops with an
explicit model error. Physical PLL transition timing is not calibrated.
The selected 2020 bitstream is not treated as equivalent to the legacy RTL.
The exact FPGA gate remains open. See
[the fabric contract](../../../simulator/backends/qemu/FABRIC.md).

## Remote evidence defect and correction

Selected local263 and SSH264 completed their inner processes but failed native
HTTP readiness. Neither tested its selected native browser or TLS publication.
The strict outer reports fail. They remain failed remote results.

Review of SSH264 found that the exporter parsed and re-serialized the worker
process result. The resulting JSON had the same values but different bytes.
Its SHA-256 hash differed from the transported object. The retention tool
correctly rejected it with `exported process envelope differs`.

The exporter now writes the original verified bytes to both result files.
It parses those bytes only to extract logs. A regression uses reordered JSON
with spaces and checks byte equality, the exported hash and decoded log for
both local and SSH records. The original failed extraction remains unchanged.
The corrected extraction must use a separate output directory. This fix does
not change any original transport, dashboard or mining verdict.

The corrected extraction then exposed the same rewrite in the shared
observation reporter. It too must preserve an existing result file. The
reporter now checks that the parsed saved result matches the supplied process
result and leaves the bytes unchanged. It rejects changed or invalid records.
A regression checks byte preservation and both rejection cases. The first
failed re-retention remains in `board-ssh-clean-264-export-283/retained/`.
The corrected retention284 passes. Its `result.json` hash matches the original
transport object. All 575 Rust tests in 101 test binaries pass after this fix.
Formatting, strict Clippy, release builds and guide links also pass.

Both selected executions have separate successful mining audits for session 0.
Each audit checks two fresh jobs, a clean-job transition, a difficulty change,
the firmware submissions and their complete hardware responses. Separate Core
nodes accept both block candidates and retain them after restart. The other
sessions and the failed native dashboard do not acquire a pass from these
results. See the local
[audit](../network-vm/board-local-clean-263/retained-283/regtest-audit.json)
and SSH
[audit](../network-vm/board-ssh-clean-264-export-283/retained-284/regtest-audit.json).
The four Core reports are under their respective parent directories as
`core-session0-candidate0-284/` and `core-session0-candidate1-284/`.

## Fresh demonstration failure280

Demo280 failed during the native WebSocket probe in the restore boot. The
socket opened and received `connection_ack`, but no subscription sample
arrived within the existing 20-second transaction deadline. The trace is
complete and the browser failure is retained. The earlier four Core checks
passed. This is not a completed milestone run.

The private browser's HTTP preflight took 64,806ms in this run. Full workspace
tests and builds ran during browser VM startup. Host contention is a possible
cause, not an established explanation. Demo284 repeats the unchanged frozen
inputs. Complete the remaining checks before its browser phase, then leave
that phase free of other build and test loads. Do not increase the deadline,
change firmware timing, remove assertions, or count the earlier run as a pass.

Demo284's unchanged browser probe passed. Its socket-open to acknowledgement
interval was 397ms. It then received two samples with different timestamps.
The native page showed three boards, 63 chips per board, the configured
frequency and accepted work on the private pool. Same-origin cookie isolation
also passed. These are observed differences; the repeat alone does not prove
the cause of demo280's timeout. Keep that failure and require a complete fresh
cycle before declaring the adapted milestone passed.

The complete demo284 cycle now passes: four firmware boots, all storage and
settings phases, and eight different blocks accepted and retained by separate
Core nodes. The current CLI matches the tested invocation hash. Final source
freeze285 and its verification pass. This closes the defined adapted milestone
only. The full specification and independent review gate remain open.

# TTC mode and decrement review

This is an author review. The independent review agents remain quota blocked.
No independent review or release completion is claimed.

The AMD UG585 counter-control definition assigns bit3 to matching and bit6 to
waveform polarity. The vendor TTC driver also defines clock bit6. The old
QEMU code stores neither bit6 and ignores match-enable. The new184-transaction
baseline records24 differences across six channels. The separate mode patch
gates both match scheduling and match status. It retains documented readback.
Latched status remains until read acknowledgement. Masking the interrupt
output does not discard status. All184 expectations pass on223 and226.

The vendor reset API specifies that down-counting starts at65535 or the
programmed interval top. The old code resets all counters to0. Its250-command
baseline records30 differences. The separate decrement patch restores the
proper starting value and uses a complementary fractional count. The visible
integer then changes only on a completed input tick. Match comparisons use
the same edge. Changing direction complements only the fractional count;
the visible count and smaller elapsed-time remainder are retained. Unsigned
32-bit arithmetic bounds the65535 reset value before shifting it16bits.

The expanded322-command test uses independent arithmetic at111111110Hz.
It checks pre-tick reads at8ns and9ns, the first tick at10ns,111ticks at1000ns,
both reset modes, direction changes, three distinct matches, interval reload,
acknowledgement, and reset while stopped. All six channels pass on226.
The328-command clock regression and184-command mode regression also pass.

The strengthened actual QMP reset test enables matching explicitly. It checks
385 transactions and24 GIC input transitions over two resets. Each of the six
real input wires must rise before reset and fall at reset. No extra wire may
change. It passes on218,223 and226 with its own retained executions.

Backend226 is built with UndefinedBehaviorSanitizer. All13 startup checks,
the compiled PIC bus and BM1387 regressions pass. Its binary hash is
14c33c658e0bd1286bb69982b500b9468d0e2ed7c8f3f33d1aea4111050ccbf8.
The exact patch parents and outputs are in ttc-mode-derived-223 and
ttc-decrement-derived-226. The complete model source is in core-checkpoint-226.
The signed update and browser firmware cycle227 is running on this backend.
Earlier cycle218 is separate evidence and does not grant226 a firmware pass.

Rust Clippy and the release build pass. Earlier223 and224 full tests pass.
The226 full test failed twice at a fixture write with EDQUOT in shared/tmp.
The unchanged test passes alone. The same complete11-test binary passes with
parallel execution in a private repository temporary directory. A complete
workspace rerun with that directory is running. The failure is not suppressed,
and no assertion, fixture size, test selection or isolation limit changed.
Only disposable intermediate libraries from completed clean build210 were
removed; reports and original evidence remain.

The complete rerun227 now passes553 tests across98 test binaries. It uses
TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227. All tests retain their
normal parallel execution. Rust formatting and489 local documentation links
also pass. The earlier shared/tmp failures remain in their original logs.

External clock input, output waveforms, full migration, physical phase
calibration and exact board correspondence remain open. The tests do not
cover all timer transitions or establish whole-board recovery. Failed baseline
and environment attempts remain failed and separate from successful runs.

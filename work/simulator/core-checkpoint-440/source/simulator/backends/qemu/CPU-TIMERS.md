# Zynq CPU peripheral timer clocks

The C patch connects the existing QEMU timer models to the Zynq clock
controls. C is required for these QEMU device interfaces. The test driver
and build coordinator use Rust. No new CPU emulator is added.

The parent is QEMU commit `11661d857254b27fca5568c6deacf4f6b6f3460f`.
Its archived source SHA-256 is
`2a40e6705e59fd8807012631bc6b2c0877f10481b12768f78e4f9d7b88376cce`.
The [source member record](../../../work/simulator/qemu-s9/cpu-timer-derived-001/parents.json)
pins the eight original files. Derived files and the patch are separate.

The selected SD image declares a 33,333,333 Hz PS clock. Its kernel
clock driver selects the ARM, DDR or IO phase-locked loop (PLL), applies
the ARM divider, and divides by two for `cpu_3or2x`. Bit 25 gates this
branch. The global timer, private timers and watchdog timer counters use
this branch. Other QEMU boards without a connection retain 100 MHz.

The patch uses QEMU's clock values with fractional nanosecond periods.
It accounts for elapsed ticks before a clock change. A gated or disabled
timer holds its count. A clock or prescaler change starts a new fractional
cycle phase. Register reads do not restart the phase. Global comparator
deadlines round up to a whole nanosecond. Auto-increment accepts values
that are not powers of two. A stopped, expired private one-shot timer
does not restart after a clock change.

The recorded 047 backend fails the
[baseline timer test](../../../work/simulator/qemu-s9/cpu-timer-qtest-001-baseline/assessment.json).
It counts 100,000,000 ticks in one second after the guest programs
333,333,330 Hz. It also advances the disabled global counter. This agrees
with the slow firmware heartbeat observed in the original firmware run.
The regression uses arithmetic counts and exact QEMU virtual intervals.
The [048 build](../../../work/simulator/qemu-s9/backend-build-048-ubsan/build-report.json)
uses the undefined-behavior sanitizer. All build and startup checks pass.
Its binary SHA-256 is
`930260cdaa548504a69f7763e95b38f82eb662151fe0853804293541388aff12`.
The [compiled regression](../../../work/simulator/qemu-s9/cpu-timer-qtest-002-ubsan/assessment.json)
passes all 100 transactions. It checks the configured rate, stopped
counters, prescaling, rate changes, private timer expiry, non-power-of-two
comparator increments, fractional deadlines, and the watchdog counter.
The [PIC bus regression](../../../work/simulator/qemu-s9/pic16-qtest-004-ubsan/assessment.json)
also passes.

The [original firmware rerun](../../../work/simulator/qemu-s9/sd2022-pic-console-006/assessment.json)
verifies 37 input identities. It records heartbeat intervals of
1.015 to 1.057 seconds, including guest scheduling and bus overhead.
The prior 047 run recorded 3.378 to 3.404 seconds. No firmware bytes or
watchdog deadlines were changed. The executing miner reaches chip
enumeration and reports no chips on all three chains.
This remains a failed mining path. The new timer behavior is not a miner
acceptance result.

Two shorter diagnostic attempts ended before miner startup. Their failure
reports are retained. The second captures first-boot key generation and
service setup. The completed diagnostic used a longer observation window;
it does not claim a boot deadline pass.

This change does not model PLL lock delay or the complete fabric clock
tree. Clock-transition phase has no physical calibration. Migration has
not been tested; new timer state formats reject the old timer versions.
The S9 development devices already do not support whole-machine migration.
Timer tests do not establish board boot or mining acceptance.

The optional boot diagnostic `icount-shift=0..6` uses QEMU's fixed
instruction-count clock. Each guest instruction advances time by
`2^shift` nanoseconds. The run manifest records this policy. It is not
measured CPU timing or a whole-board deterministic replay claim.
Device timer periods remain in virtual nanoseconds.

Two firmware diagnostics with shifts 1 and 6 hit their CPU limits at
the failsafe prompt. A separate TTC regression then reproduced a
zero-time callback loop. TTC means Triple Timer Counter. The upstream
TTC model rounded a fractional event deadline down. A remaining fraction
shorter than one nanosecond scheduled another callback at the same time.
The [failing test](../../../work/simulator/qemu-s9/ttc-qtest-001-baseline/assessment.json)
stops after 106 of 112 responses on backend 050.

The separate `s9-ttc-deadline.patch` rounds deadlines up and cancels
scheduled callbacks when disabled. Its
[parent record](../../../work/simulator/qemu-s9/ttc-derived-002/parents.json)
pins the original file. It retains the upstream fixed 133 MHz input and
counter precision. Full TTC clock-tree integration remains open.
The first fix on backend 051 returns all 112 expected responses, but
still spins with a disabled callback scheduled and reaches the CPU limit.
That run remains a failure. The second fix adds explicit cancellation.
The [054 compiled test](../../../work/simulator/qemu-s9/ttc-qtest-003-ubsan/assessment.json)
passes all 112 responses and reaches the expected idle wall deadline.
It does not hit the CPU limit. The UBSan backend binary SHA-256 is
`ebf631d5e1363a5703ccb15355fa47a1ee96502f4f6e5368aa7086ed7ac58b10`.
The archived firmware rerun is still pending. Passing this component
test does not establish that the firmware boot or mining path passes.

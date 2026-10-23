# Zynq Triple Timer Counter clock connection

This derived QEMU component is under test. It is not a complete board model.
C is required for the existing QEMU clock and timer interfaces. The build and
transaction test drivers use Rust. No new CPU or independent timer emulator
is introduced.

The original DTB identifies clock6 for both Triple Timer Counter (TTC) blocks
at0xf8001000 and0xf8002000. See the retained
[decoded board tree](../../../work/simulator/qemu-s9/dtb-inspection-001/board.dts)
and its provenance. The pinned kernel driver defines clock6 as cpu_1x.
Its source is in [the isolated inspection](../../../work/simulator/kernel-clock-source-211/references.json).
That archive has SHA-256
5aa5c0915c93c6078984b13367edd12670e8ad25d922d352654f768d7005db8f.
The clock driver member has SHA-256
e0b7a555053c21908d29fe2d065172dcb5f8c381fc3f7c06ad60ecdef5ee2fc2.
This reference source does not establish the build identity of the guest kernel.

The clock uses the ARM PLL, multiplexer and divider. ARM_CLK_CTRL bit27 gates
the branch. CLK_621_TRUE selects a final division by6 or4. The selected board's
PS clock is33333333Hz. The compiled transaction oracle programs PLL40 and an
ARM divider of2. With division by6, the expected TTC rate is111111110Hz.
Backend201 instead counts at its upstream fixed133MHz rate. The retained
[baseline test](../../../work/simulator/qemu-s9/ttc-clock-baseline-211/assessment.json)
fails the new expectations. It remains a failure.

The patch connects both TTC blocks to the existing QEMU clock tree. Other
boards without that connection retain the upstream133MHz default. Clock
periods retain QEMU's32 fractional nanosecond bits. The counter keeps its
existing16 fractional bits. A separate division remainder prevents register
reads from discarding smaller fractions. Event deadlines round up. Disabled
or gated counters hold their values. Selecting the unconnected external clock
source produces no clock edges.

A clock or prescaler change retains the16.16 counter and starts a new phase
below that precision. This is an explicit transition policy. It is not a
physical calibration. Counter reset clears both the count and its remainder.
The changed saved state rejects the prior timer state version. Full machine
migration remains untested and unsupported by other development devices.

The [parent and derived file hashes](../../../work/simulator/qemu-s9/ttc-clock-derived-212/parents.json)
separate this patch from the original QEMU archive and earlier timer patches.
The source commit is11661d857254b27fca5568c6deacf4f6b6f3460f. Its archive
SHA-256 is2a40e6705e59fd8807012631bc6b2c0877f10481b12768f78e4f9d7b88376cce.
Original downloaded files remain unchanged.

The separate s9-ttc-reset.patch connects QEMU system reset to each TTC block.
It is under test. It restores the original model's cold register defaults,
clears both counter fractions, cancels scheduled callbacks and lowers IRQs.
The [parent record](../../../work/simulator/qemu-s9/ttc-reset-derived-218/manifest.json)
pins its exact input and output. The private FIFO and QMP regression checks
all six channels before and after two actual system resets. Backend213 fails
the [retained baseline](../../../work/simulator/qemu-s9/ttc-reset-baseline-217/assessment.json):
the counter-control register remains enabled after system reset. The earlier
215 and216 attempts failed at socket setup and do not establish device behavior.
The test does not expose emulator control ports or execute guest firmware.

The separate s9-ttc-modes.patch is under test. The AMD
[counter-control contract](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/Register-Counter_Control_2-Details)
requires bit3 to enable register matching. Backend218 instead latches match
events while that bit is clear. It also drops documented bit6 in the clock
and counter controls. The new184-transaction regression has24 differences
against that backend. It tests all six channels. The
[baseline result](../../../work/simulator/qemu-s9/ttc-modes-baseline-222/assessment.json)
remains failed. The new patch gates match events and their deadlines, retains
both documented control bits, and preserves the existing read acknowledgement.
The [source manifest](../../../work/simulator/qemu-s9/ttc-mode-derived-223/manifest.json)
pins parent, derived file, patch and the retained reference-provider response.

External clock pins and waveform outputs remain unconnected. Their settings
have readback, but no output behavior is claimed. Review also found that the
upstream decrement reset and fractional readback need separate conformance
work. The increment tests do not establish decrement correctness.

The s9-ttc-decrement.patch now addresses that defect and is under test. The
vendor [reset API contract](https://xilinx.github.io/embeddedsw.github.io/ttcps/doc/html/api/group__ttcps.html)
states that a decrementing counter resets to65535 or the interval value.
Backend223 instead resets to0. The retained
[250-transaction baseline](../../../work/simulator/qemu-s9/ttc-down-baseline-225/assessment.json)
has30 differences. The derived model retains a complementary fractional phase
for decrementing counters. Readback and matches then change on completed ticks.
Changing direction preserves both the integer count and elapsed clock phase.
The expanded regression also checks those direction changes. Its arithmetic
expectations are separate from the C model. The
[patch manifest](../../../work/simulator/qemu-s9/ttc-decrement-derived-226/manifest.json)
pins both source files and the patch. Physical clock phase calibration remains
open. Increment, mode, reset and firmware tests must still pass on the new build.

Backend226 now passes the322-command decrement regression,184-command mode
regression,328-command clock regression and the stronger system-reset test.
The latter checks385 transactions and24 actual GIC input transitions. The
[author review](../../../work/simulator/reviews/ttc-modes-decrement-226.md)
records the arithmetic, patch identities, validation and remaining limits.
The firmware cycle on this backend is still running. Independent review and
exact-board acceptance remain incomplete.

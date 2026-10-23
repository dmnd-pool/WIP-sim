# Zynq fabric signals

The System Level Control Registers (SLCR) supply four fabric clocks and four
active-low reset outputs. The QEMU patch uses its existing phase-locked loop
(PLL) periods. The two dividers and source multiplexer follow the archived
Linux `drivers/clk/zynq/clkc.c` in
`work/simulator/kernel-clock-source-001/members/`.
The clock period retains QEMU's 32 fractional bits per nanosecond.
A zero divider bypasses that divider. Source selections 0 and 1 use the I/O
PLL; 2 uses the ARM PLL; 3 uses the DDR PLL.

Bit 0 of `FPGAx_THR_CNT` disables the continuous output, as in that Linux
clock driver. Nonzero `FPGAx_THR_CTRL` selects an unimplemented throttle mode.
It stops the run with a model error and gates that output. It cannot pass a
firmware gate. All throttle controls and counts clear on system reset.
Clock and reset changes respect the SLCR write lock.

`FPGA_RST_CTRL` bits 0 through 3 control the active-low reset outputs.
Reset asserts all four. These are raw processor outputs. The external
`proc_sys_reset` block and its release timing are separate required models.
The archived legacy board script connects fabric clock 1 to S9IO and reset
output 0 to this reset block. The selected 2020 bitstream's corresponding
connections are not established by that script.

The legacy S9IO consumer now uses fabric clock 1. It retains the exact QEMU
clock period, including fractional nanoseconds. Clock changes first complete
all elapsed old-clock edges. A frequency change starts a new edge phase.
This phase rule is a declared digital policy; physical PLL transition timing
is not calibrated. Clock gating preserves UART and FIFO state. Gated time
cannot become extra edges on resume. Register polling cannot add edges.

The selected 2020 reference profiles retain their declared fixed clocks.
Their exact wiring must come from the selected bitstream or matching source.
The new ASIC scheduler advances from physical virtual time. Its UART and hash
search continue when the fabric clock stops. Controller samples occur only on
fabric edges. Both peers sample the old pin levels when their times coincide.
Fractional core edges round up to the next nanosecond for the ASIC interface.
Periods below one nanosecond fail explicitly. The clock-change callback uses
one captured time for the old clock and the new phase.

`fabric-clock-reference=on` connects FCLK1 to the BM1387 development machine.
This is a separate explicit reference configuration. It is off by default and
does not establish the selected 2020 bitstream's wiring. Rust tests require
16 additional computed hashes after the fabric stops. They also check a
complete 441-byte ASIC reply while the controller takes no samples.
The compiled UBSan backend294 passes 1,257 transactions with this connection,
including full replies from three chains before and after a clock change.
Its 859-transaction fixed-clock regression and the UART gating, fabric reset,
watchdog and timer reset checks also pass. The archived firmware regression
also passes with the default fixed clock. Independent verification checks
1,323 computed hashes at a 10 H/s cap, two fresh jobs, a clean-job transition
and a difficulty change. Two isolated Bitcoin Core nodes accept its different
block candidates and retain them after restart. See
[the clock evidence](../../../work/simulator/reports/asic-clock-295.json).
These results do not establish an exact board pass.
The raw processor reset outputs still need the external reset sequencer
before connection to S9IO.

The C patch extends the existing QEMU clock interface. Rust owns the test
transactions and independent expected results. No new CPU emulator is used.

The compiled output test passes 242 transactions, 111 clock states and seven
reset states in `work/simulator/qemu-s9/fabric-signals-280/assessment.json`.
The reset test passes 128 transactions and two actual system resets in
`work/simulator/qemu-s9/fabric-reset-282/assessment.json`. Backend263 fails
that same reset check because a programmed clock gate survives reset.

The UART test uses private QMP and qtest pipes. It requests a clean QEMU exit
after 45 transactions. Each of three UARTs stops part way through a byte,
preserves state for 1ms, and resumes at half frequency. It checks exact byte
values, fractional timestamps and independence from fabric clock 0. Backend280
passes; the earlier fixed-clock backend276 fails the same oracle.

```sh
work/simulator/check-target-085/release/examples/qemu_ttc_reset \
  work/simulator/qemu-s9/backend-build-280-release \
  work/simulator/qemu-s9/fabric-uart-qmp-282 fabric-uart
```

That output path already exists. Use a new path for another run. The initial
stdin-only probe280 stopped before the first byte and later hit its CPU limit
after EOF. Its failed evidence remains. Probe282 waits until transmission has
started and uses explicit QMP shutdown; it requires exit code 0 and complete traces.

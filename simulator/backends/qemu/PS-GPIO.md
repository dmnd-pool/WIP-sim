# Zynq7000 PS GPIO reference

This is the processor GPIO controller at `0xe000a000`. It is separate from
the PL AXI GPIO devices. The model is enabled only in the public-ROM reference
machine. It does not establish the selected S9 pin wiring or silicon identity.

The primary contract is the archived UG585 v1.12.2, Chapter14 and AppendixB.
The PDF SHA-256 is
`e24b2574c52110f351e6aee0375b97f62556d2decd957e006646367ca43b7a66`.
Four banks contain 32, 22, 32 and 32 pins. Their shared interrupt is GIC ID52,
SPI input20. The first original ROM write is `0xffffffff` to INT_DIS_0 at
`0xe000a214`. Its instruction bytes and retained failed execution are recorded
in `work/simulator/reviews/zynq-lvttl-independent-execution-513i.json`.

Rust owns register and interrupt state. A small C binding connects the existing
QEMU CPU, bus, clocks and interrupt wires. The exact Xilinx C candidate remains
archived unchanged. Its six-bank profile can be changed, but its input/latch,
interrupt and reset behavior also needs replacement. The bounded assessment
is `work/simulator/reviews/ps-gpio-reuse-independent-513j.json`. There is one
PS GPIO state implementation.

DATA is an output latch. DATA_RO reads separately supplied input samples.
Masked writes preserve unselected bits. Bank1 ignores reserved upper bits.
Reads of the write-only interrupt enable/disable registers fail because the
manual calls their values unpredictable. Undefined offsets, extra banks,
partial accesses and unaligned accesses fail before state changes.

MIO latch reset values are unknown. EMIO latches reset to zero. An external
input is unknown until an explicit settled sample supplies it. A value and
known-bit mask preserve this distinction. This sampled-state type is separate
from the live HDL transport's four-state wire encoding. Unknown guest reads
and unknown interrupt levels stop the reference machine. They do not return
a successful zero value. The guest-error trace and stop are required because
the inherited board ignores failed bus transactions.

Interrupt behavior covers rising, falling, either-edge, high-level and
low-level modes. Edge status latches; write-one-to-clear removes an edge.
An active level remains visible until its source changes. All enabled status
bits contribute to the shared interrupt. Output writes never create input
events. The supplied external sample is the only input event source.

The existing CPU_1x clock passes through APER_CLK_CTRL bit22. Access while the
clock is gated fails explicitly. GPIO_RST_CTRL bit0 holds the bus interface in
reset; its reserved bits read zero. Controller state persists across that bus
reset and QMP warm resets. Cold state is created once. Input samples supplied
while the clock is gated fail; this increment does not guess lost edges or
synchronizer delay. Samples supplied while only the bus is reset can still
reach the controller logic. No subcycle timing claim is made.

The output interface exposes latch data, whether each latch bit is known, and
the DIRM&OEN enable intent. Consumers must honor all three. It does not resolve
MIO mux, TRI_ENABLE, pulls, voltage straps, pad contention or external feedback.
Bank0 bits7/8 are not general inputs. Migration is explicitly unsupported.

The staged component suite includes the original four INT_DIS writes, the
manual's `0xbdef4210` masked-write example, bank1 width, independent input/output
data, five interrupt modes, multi-bank interrupt reduction, pending-mask
changes, clear behavior, clock/bus reset guards and two actual warm resets.
A separate rejection suite requires unknown reads, extra-bank writes, partial, doubleword
and unaligned writes, and accesses while reset or clock gated to stop. No
firmware, original-board, physical timing or mining pass follows from these
component results.

Build, test and actual execution results must be added after the reviewed
stage is integrated. No staged source is counted as a passing result.

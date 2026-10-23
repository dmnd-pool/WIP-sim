# TTC system reset review

This is an author review. Independent review is still unavailable because
the three review agents reached the service usage limit.

The old model initializes its count-control register to0x21 but has no QEMU
device reset handler. The new Rust regression programs all six channels,
advances the actual QEMU virtual clock and issues QMP system_reset. QMP is
the QEMU Machine Protocol. Baseline217 records the real RESET event, then
reads0 from count control instead of0x21. It remains a failed device test.

The earlier215 and216 attempts fail at socket setup. The host policy rejects
socket binding and QEMU's inherited-socket validation. The final test uses
two FIFOs in the private scratch mount and QMP over standard input/output.
The outer process imposes memory, CPU, process, trace and wall limits.
The test checks the confinement contract before starting QEMU. It exposes
no emulator port and has no guest network interface. Error cleanup owns and
reaps only its direct child. A completed child is not killed again.

The separate C patch adds a device reset handler. The common timer reset
routine cancels the timer, clears all register and fractional count state,
restores the original cold default, and lowers its IRQ. The constructor
creates the QEMU timer before calling this routine. IRQ delivery is skipped
only before that line is connected. The existing clock pointer, timer and
IRQ objects remain owned by QEMU; reset does not replace them.

Backend218 uses UndefinedBehaviorSanitizer. All13 startup checks pass.
The reset regression passes384 transactions, two real RESET events and a
clean QEMU exit. The328-transaction clock regression, PIC bus and BM1387
regressions also pass. Rust formatting, strict Clippy and537 tests across97
test binaries pass. The full release build passes. Frozen helper binaries
are in ssh-target-218; binaries.json records all59 hashes.

The compiled backend SHA-256 is
b530f76bbdf5ba9a3250b87683ccf8cd3322a01579dda9f5e1a8a94fc469d425.
The derived timer SHA-256 is
d0542840eaec50b1731041686d298abe2169d3988cba019a20447e42be8ff0cd.
core-checkpoint-218 retains the source and model library. The patch parent,
derived file and patch identities are in ttc-reset-derived-218/manifest.json.

This test validates reference reset state, not physical reset timing or
whole-machine persistence. Full migration, external TTC clocks, PLL lock
timing and physical calibration remain open. The signed firmware update and
native dashboard cycle218 is running and is not yet reported as passed.
The firmware interruption cycle213 passed separately on its exact213backend;
that result is not reused for218. No exact-board or full-release pass follows
from these component checks.

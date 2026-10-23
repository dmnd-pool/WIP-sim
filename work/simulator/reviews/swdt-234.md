# Watchdog implementation review

This is an author review. The independent agents cannot run because of their
recorded quota limit. Independent review remains a release requirement.

The pinned QEMU board had an unimplemented system watchdog. The new model
uses QEMU PTIMER and Clock. It does not replace a CPU emulator. Rust handles
keyed registers and expiry output selection. The small C adapter connects
the model to the real GIC input and QEMU reset request.

The archived kernel driver member and AMD register references are recorded
in [the contract](../../../simulator/backends/qemu/SWDT.md). Original files
are unchanged. Each isolated build captures the source, patch, static model
library, toolchain and derived output hashes.

The baseline probe failed against backend 226: 118 actual responses instead
of 126 expected responses. It observed no IRQ transitions. Build 231 then
rejected an incorrect clock-source parent hash. The source derivation had
omitted the project's CPU reset changes. Build 232 uses the exact prior
SLCR source identity and passes the strict parent and derived checks.

Validation:

- Three Rust tests check all mode, control and restart key values, register
  masks, read-only status, reload vectors and all output-enable combinations.
  The initial vector used a wrong decimal literal for `0x123fff`; its failure
  is retained. The corrected value follows the published bit concatenation.
- Compiled qtest 234 passes 146 ordered responses. It checks all four divisors
  and IRQ pulse lengths, invalid restart behavior, status, stop at zero,
  CPU clock gating, and an unconnected external clock. IRQ transitions are
  checked in order between time advances, not only counted at the end.
- Reset 234 passes 60 transactions. A real pulse is lowered by QMP reset.
  An expired watchdog causes the second system reset, without a QMP reset
  command. A third reset cancels an armed counter. Old callbacks do not fire.
- Existing 328-command CPU/TTC clock, 385-command TTC reset with 24 IRQ events,
  881-response PIC and859-response ASIC tests pass on the same backend.
- Backend 232 builds and executes with undefined-behavior sanitization.
  All 13 existing startup checks pass. Full Rust tests 232 pass 556 tests.
  The final 234 checks also pass 556 tests, strict Clippy, formatting and 496 guide links.

Reviewed limits remain open. The hidden counter reset value uses an explicit
default-reload assumption. Missing-clock APB hang timing, external reset
waves, BootROM-written reset reasons, migration and physical calibration are
not claimed. The firmware cycle 234 is running. Component success does not
close original-board, exact-target mining, remote or full-matrix acceptance.

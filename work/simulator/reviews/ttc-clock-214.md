# TTC clock review

This is an author review. The independent review gate is open. All three
review agents remain unavailable because of the service usage limit.

The original board tree selects clkc6 for both TTC blocks. The pinned Linux
clock driver identifies that clock as cpu_1x. The previous fixed133MHz input
therefore did not follow the guest's clock programming. The new328-response
oracle fails68 expectations on backend201. It passes unchanged on backend213
with UndefinedBehaviorSanitizer. The six channels use arithmetic counts from
PLL40, ARMdiv2 and a33,333,333Hz source. Gating, changed dividers, prescaling,
stopped counters and an unconnected external source have explicit checks.

The first build212 failed before compilation because the BM1387 board-source
hash no longer included the preceding TTC wiring. Composition213 preserves
the exact old parents, records the addition and replaces both whole-file
hash assertions with the composed identities. It does not remove either
assertion. Backend213 builds and passes all13 startup checks. The PIC bus and
ASIC compiled regressions also pass.

The clock calculation uses QEMU's existing fractional-period clock API.
Products fit unsigned128-bit arithmetic for the selected clock and prescaler
ranges. Deadlines round up. A retained division remainder prevents repeated
reads from losing elapsed time. Disabled and gated counters keep their values.
The external pins have no source, so selecting them supplies no ticks.
Clock changes synchronize the old count before applying the new period.
They retain the16.16 count and restart the smaller phase. This is a declared
reference transition policy without physical calibration.

Review found stale scope text in probe213 that still named a fixed133MHz TTC
input. Probe214 selects its scope from the same explicit clock-tree policy
as its oracle. Its actual rerun passes328 responses. The old report remains
unchanged. The implementation code and the backend binary are unchanged.

Open work includes whole-device reset integration, external clock pins,
saved-state validation and migration tests, full fabric-clock connections,
PLL lock timing and exact physical timing evidence. The counter-reset write
is tested; it does not prove a complete board reset. The full archived
firmware recovery/mining cycle213 is active and must finish before reporting
that cycle as passed. The separate native network dashboard208 remains
unready. No exact physical target or release completion claim is made.

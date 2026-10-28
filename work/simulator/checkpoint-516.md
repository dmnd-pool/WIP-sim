# Verified S9 component checkpoint516

S9 milestone2 is incomplete. Continue S9 only. Stop after complete S9 acceptance;
do not start the other miner targets.

The live selected FPGA path passes72 bus operations and two resets after the
shared child-transport change. Its throughput test fails at100us;10us takes
2.472s. The source-available CVC tool builds and passes smoke/restore tests.
Its scheduler and selected-model equivalence remain open. Verilator5.052
source cannot preserve this unchanged four-state interface.

The unchanged public ROM now reaches real device writes from reset. Its
first unsupported SLCR write isF8000254=1. Independent audit matches180 raw
translated spans/2736bytes to the original ROM. Full firmware boot, exact
silicon/component identities, mining through this board path and physical
calibration remain open. Historical adapted S9 mining/dashboard results stay
separate.

Full Rust checks pass1237 tests in129 suites, with55 explicit ignored tests.
Formatting and strict all-target Clippy pass. All596 source inputs match
before and after. See [compact evidence](source-checkpoint-516/manifest.json)
and [workspace report](reviews/workspace-check-after516.json).

Next: run the independently reviewed actual CVC/Icarus callback-order oracle.
Retain the expected CVC failure before a recorded kernel correction. Integrate
and test the reviewed513h reset-reason patch, then execute the original ROM
again. Keep all old failures. The CPU/PL bridge remains separate staged work.

Read [the current continuation](continuation-2026-10-28.md),
[the progress record](progress.md) and [milestone2 acceptance](milestones.md).
Original archives, emulator disks, executables, credentials and large runtime
traces stay outside Git. Transfer linked archive contents with the repository
when those original inputs are needed.

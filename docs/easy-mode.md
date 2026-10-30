# Easy mode: real firmware for pool testing

Easy mode runs the real mining program of each miner with simulated hardware.
The purpose is to test a mining pool with the Stratum clients that real
miners use. See [the track rules](tracks.md) and
[the easy-mode milestones](../work/easy/milestones.md).

## Scope

- Adapted environments are allowed. Examples are process-level runs, direct
  boot and a lowered chip filter. Label each adaptation in the results.
- The internal test pool and the regtest Bitcoin Core nodes check every share
  and block. Connect to an external pool only when the user asks.
- Coverage is by Stratum client family, not by miner model: Bitmain
  bmminer and cgminer, Braiins bosminer, LuxOS, VNish, WhatsMiner btminer,
  Canaan cgminer and ESP-Miner.
- Do not use hardware access.

## Current state

The adapted S9 runs the original Braiins OS+ 22.02.4 and mines on the
internal test pool. See the
[adapted S9 report](../work/simulator/reports/adapted-s9-milestone-284.md).
Other S9 firmware is not run yet. Bitmain stock S9 firmware needs a model of
Bitmain's own FPGA, which also builds the mining work.

## Method

- Use discovery mode for unknown registers: reads return 0, writes are
  ignored, and each access is logged. One run lists every gap.
- Prefer a process-level run for Linux miners: the mining program runs in a
  generic ARM Linux virtual machine, and the simulated hardware connects
  through device files.
- Run long jobs as scripts outside the agent session, with one small JSON
  summary each.

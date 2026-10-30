# Easy-mode milestones

Track rules: [docs/tracks.md](../../docs/tracks.md). Guide:
[docs/easy-mode.md](../../docs/easy-mode.md). The goal is to run each
miner's real mining program with simulated hardware, to test a mining pool
later. The internal test pool is enough until the user asks to connect their
own pool. No hardware access is used.

## E1. Adapted S9 with Braiins OS+ — passed

Milestones 1 and 1.2 in [the old milestones](../simulator/milestones.md).

## E2. More Braiins S9 versions — open

Run the other Braiins OS S9 versions from the catalogue on the adapted S9.
Use discovery mode for unknown registers. Record which versions need a change.

## E3. Bitmain stock S9 firmware — open

Stock firmware uses Bitmain's own FPGA, which also builds the mining work.
The hard track owns that model (hard-mode H9). A functional model from the
bmminer-mix source can come first. Write the request in the hard-mode handoff.

## E4. Process-level runner — open

Run a Linux miner's mining program in a generic ARM Linux virtual machine.
Connect the simulated hardware through device files: a pseudo-terminal for
each chain UART, user-space character devices for the FPGA and I2C, and Linux
`gpio-sim` for GPIO. Use it for miners without a controller-board model.

## E5. One firmware per Stratum client family — open

Bitmain bmminer and cgminer, Braiins bosminer, LuxOS, VNish, WhatsMiner
btminer, Canaan cgminer and ESP-Miner. The user sets the order.

## E6. Connect to the user's pool — later, only when the user asks

A checking proxy between firmware and pool, network access to one address,
and hashing on many CPU cores.

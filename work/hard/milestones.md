# Hard-mode milestones

Track rules: [docs/tracks.md](../../docs/tracks.md). Guide:
[docs/hard-mode.md](../../docs/hard-mode.md). On 30 October 2026 this file
replaced the exact-S9 milestone 2 in
[the old milestones](../simulator/milestones.md). No hardware access is used.

## H1. Overnight FPGA comparison — ready

The `md-sim-hard` tool compares the rebuilt S9 FPGA circuit with the Rust
S9IO model. Calibration and an 8-sequence trial pass. The trial found the
serial clocking difference and three leads. Next: the user starts the
overnight run.

## H2. S9IO model matches the rebuilt circuit — open

Acceptance: one overnight run of at least 2,000 random sequences has zero
`read-mismatch`, `circuit-unknown`, `bus-response`, `tx-bytes` and `tx-level`
findings. Report busy-read timing differences with their range.

Work: confirm and fix each lead and each later finding. Add a regression test
from the replayed case for each fix. Then compare the interrupt lines and add
divisor changes inside a sequence. Decide the model clock: one 100 MHz fabric
clock with 8 samples per bit is the current evidence.

## H3. Fast two-state circuit simulation — open

Build a Verilator version with simple two-state primitive models. A circuit
rebuilt from a bitstream starts with defined values, so two-state is closer to
the real FPGA. Check it against Icarus with the same sequences. Measure the
speed, then use it for the overnight comparison.

## H4. CPU and circuit co-simulation — open, depends on H3

Run short windows of the original firmware with the circuit in place of the
Rust model. Compare each window with the same window on the Rust model.

## H5. BootROM in discovery mode — open

Run the public BootROM with unknown registers that read 0 and ignore writes,
and log each access. One run lists the registers that the ROM uses before it
loads the SPL. Implement them in one batch, checked against the Zynq manual.
Braiins OS boots its own unencrypted SPL on real S9 units, so the S9 eFuses
allow unencrypted boot.

## H6. PIC program in an emulator — open

Run the original PIC16F1704 program (`work/simulator/pic16-static-001`)
instead of the protocol-only model.

## H7. Strict BM1387 model — open

Flag command orders and PLL or baud settings that no known driver uses. The
real failure behavior is not in any file. Report it; do not invent it.

## H8. G7 and G8 from file-based ground truth — open

For each timing and physical value: the source, the confidence and the
comparison result.

## H9. Bitmain stock S9 FPGA — open

Rebuild the stock bitstream with the existing netlist tools. Write its Rust
model from the bmminer-mix `driver-btm-c5` source. Compare both with the same
tool. Easy mode then runs stock firmware on this model.

## Done for the S9

H2 to H9 pass, and Braiins OS and Bitmain stock firmware run on the faithful
models.

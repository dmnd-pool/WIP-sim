# Hard-mode handoff

Updated 30 October 2026. Keep this file under two pages.

## State

- The S9 FPGA circuit, rebuilt from the Braiins OS+ bitstream, runs in the
  archived Icarus 12 build with Xilinx's own primitive models.
- `md-sim-hard s9-fabric` compares it with the Rust S9IO model (reference
  1.0.2). Calibration and an 8-sequence trial pass. Results and leads are in
  [the hard-mode guide](../../docs/hard-mode.md#first-results).
- Older exact-S9 work is paused. See [checkpoint 518](../simulator/checkpoint-518.md):
  the prepared CVC run, the BootROM at SLCR `0x500` and the CPU-to-FPGA
  bridge. They are optional references now. The methods in
  [the track rules](../../docs/tracks.md) replace the one-register-per-step
  loop.

## Next actions

1. Read the newest `work/hard/runs/*/summary.json`. Open a case folder only
   for a finding that you work on.
2. For each finding key: replay one case, confirm the cause, fix the S9IO
   model, and add a regression test.
3. Run the overnight comparison again. Repeat until H2 passes.
4. H5 (BootROM discovery) and H6 (PIC emulator) do not depend on H2. They can
   run in parallel with it.

## Commands

```bash
CARGO_TARGET_DIR=work/hard/target cargo build --release -p md-sim-hard --offline --locked
nice -n 10 work/hard/target/release/md-sim-hard s9-fabric overnight \
  --workspace . --out work/hard/runs/s9-fabric-$(date +%Y%m%d-%H%M) --hours 8
work/hard/target/release/md-sim-hard s9-fabric replay \
  --run work/hard/runs/<run> --case work/hard/runs/<run>/cases/<seed>
```

## Requests from the easy track

None yet.

# Hard mode: faithful hardware simulation

Hard mode builds a simulation that behaves like the real hardware, from files
only. The first target is the Antminer S9. The uses are study and writing new
miner firmware. See [the track rules](tracks.md) and
[the hard-mode milestones](../work/hard/milestones.md).

## Ground truth without hardware

| Part | Source in the archive | Use |
| --- | --- | --- |
| FPGA | Circuit rebuilt from the original Braiins OS+ bitstream | Reference for the Rust S9IO model, by automatic comparison |
| Zynq start-up | Public BootROM dump and the Zynq technical manual | Original boot path, in discovery mode |
| Hashboard PIC | Original PIC16F1704 program image | Run in an emulator instead of the protocol-only model |
| BM1387 chips | Firmware drivers and vendor test code | Strict model that flags command orders no known driver uses |
| Sensors, EEPROM | Datasheets | Device models |
| Power, heat, fans | Public specifications | Approximate models, labelled |

## Overnight FPGA comparison

The `md-sim-hard` tool compares two things with the same inputs:

- the S9 FPGA circuit, rebuilt from the bitstream
  (`work/simulator/qemu-s9/netlist-selected-502k/selected.v`), simulated by the
  archived Icarus Verilog 12 build with Xilinx's own primitive models;
- the Rust S9IO model that the S9 simulator uses (reference contract 1.0.2).

The circuit, the vendor models and the Rust model are not changed.

### Start a run

Run from the repository root. Choose a new output folder.

```bash
CARGO_TARGET_DIR=work/hard/target cargo build --release -p md-sim-hard --offline --locked
nice -n 10 work/hard/target/release/md-sim-hard s9-fabric overnight \
  --workspace . --out work/hard/runs/s9-fabric-$(date +%Y%m%d-%H%M) --hours 8
```

The default number of parallel simulations fits the free memory (about 0.5 GB
each) and leaves four cores free. Set it with `--jobs`. Each run starts from a
new seed (the Unix time). Use `--seed` to repeat a run. The subcommands
`prepare`, `calibrate` and `run` do the same steps one at a time.

### Steps

1. `prepare` hashes every input, unpacks Icarus, writes the testbench and
   compiles it. This takes about 10 seconds.
2. `calibrate` reads the build ID and version from the circuit. It finds the
   TX and RX pins of each chain from their behavior, because netlist pin names
   are physical sites. It measures the serial bit length, sets the clock ratio
   between model and circuit, and searches for the bus alignment. This takes
   about 3 minutes.
3. `run` generates random sequences and runs them in parallel until the
   deadline. A sequence writes and reads registers, fills the command and work
   FIFOs, and sends chip responses on the RX pins: valid frames, frames with a
   bad CRC and short frames. A planner runs its own copy of the model and
   places most reads at quiet points.
4. Each sequence is compared: quiet reads and FIFO reads exactly, busy reads
   within 32 cycles, bus responses, decoded TX bytes, and the TX idle level at
   quiet points.

### Read the results

- `summary.json` is rewritten after every sequence. `sequences_by_class` counts
  the sequences that show each finding. `examples` gives up to five cases for
  each finding, with the case folder.
- `calibration.json` holds the pin map, the alignment and the evidence for it.
- `cases/<seed>/` is kept only for a sequence with a finding or an error. It
  holds `sequence.json`, `sequence.txt`, the circuit trace `circuit.txt`,
  `vvp.log` and `report.json`.
- Compare one case again:
  `md-sim-hard s9-fabric replay --run <run> --case <run>/cases/<seed>`.
  Add `--simulate` to run the circuit again.

Finding classes, strongest first: `read-mismatch` (quiet or FIFO read),
`read-mismatch-busy`, `circuit-unknown`, `bus-response`, `tx-bytes`,
`tx-level`, `tx-unknown` and `circuit-timeout`. The key also names the
register, for example `read-mismatch 0x300c`.

### First results

Trial on 30 October 2026, 8 random sequences:

- Calibration found build ID `0x5FCA47E9` and version `0x00901002`. TX pins
  are output pins 5, 3 and 1, and RX pins are input pins 6, 4 and 2, for
  chains 0, 1 and 2. All calibration sequences match with zero findings.
- The circuit's serial bit is 8 x (divisor + 1) fabric cycles, with one more
  sample period at each stop bit. The model uses 16 samples per bit at twice
  the clock, because the S9 simulator clocks it at 200 MHz. The bosminer
  driver passes a 100 MHz baud clock. The comparison uses two model cycles
  per circuit cycle.

Leads to confirm with `replay` and a focused sequence before a model change:

1. A disabled chain: the circuit keeps TX high. The model holds TX low.
2. A 4-midstate job (CONTROL midstate field 3): the model sent it. The
   circuit kept words in the work FIFO and sent nothing.
3. Work response word 2: the circuit gives extended work ID `0x0004`. The
   model gives `0xFF84`, a wrap below zero.

### Limits

- Speed is about 150 to 300 circuit cycles per second for each simulator
  process. One night with 24 processes gives about 100 million cycles.
- Only the three S9IO cores are compared. The fan, GPIO and I2C cores are not.
  Interrupt lines are recorded but not compared.
- The serial divisor stays constant inside one sequence.
- After the model reports an unsupported case, that chain is not compared for
  the rest of the sequence. The summary counts these cases.

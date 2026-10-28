# Selected S9 programmable-logic bus execution

The recovered S9 programmable logic now passes 72 actual MAXIGP0 bus
transactions and two reset sequences with the original vendor cell models.
An independent execution audit passes. This is a component test. The PS7
processor is still a port shell; no ARM firmware instruction runs in this test.

The run uses the exact [502k netlist](../qemu-s9/netlist-selected-502k/selected.v),
SHA-256 `3ea22dc51d32539e0a52ff83d7152d82009f4e50883bc20e6643e6672d1a2c9b`.
Its [preservation audit](netlist-structure-independent-execution-502k.json)
binds all 11,122 cells and original routing constraints to the selected FPGA
configuration. The existing Icarus 12.0 tool build and vendor models are pinned
in the [input profile](../../../simulator/firmware/s9-hdl-inputs-511d.json),
SHA-256 `f137f1e433e80074ecb539b8173bd993ce825ca5f1ccfe7f892f90eb0512f25b`.

The expected version and build checks come from the exact archived 2022
Bosminer ELF. Its [instruction evidence](bosminer-static-executed-511c.md)
and [machine-readable contract](../../../simulator/firmware/s9-bosminer-mmio-511.json)
remain separate from HDL execution. No expected read value is supplied to the
hardware model.

The Rust-generated fixture drives only declared PS7 output ports. It checks
all 42 selected port widths and directions against both pinned interface
sources. External input drivers stay at high impedance. Real fabric logic
produces every ready signal, response, read value and reset result.

The test covers the three controller blocks at `0x43c00000`, `0x43c10000`
and `0x43c20000`. Each returns version `0x00901002` and build timestamp
`0x5fca47e9` (2020-12-04 14:30:01 UTC). It checks control bit 3, full-word and
byte-strobe baud writes, a zero-strobe write, response backpressure, register
restoration, and reset after a deliberately changed register value. All data,
IDs, responses and sequence positions are also checked by Rust from the
complete retained trace. Unknown values, unexpected diagnostics and timeouts
remain failures.

The fixture supplies a 100 MHz digital clock at FCLKCLK1 and reset at
FCLKRESETN0. It observes propagation through the original clock buffer and
state recovery in the actual logic. This is not physical timing calibration.

This command was tested from the repository root. Use a new output directory:

```sh
work/simulator/qemu-s9/axi-driver-511e/s9_hdl_validate \
  /home/user/src/md-sim simulator/firmware/s9-hdl-inputs-511d.json \
  work/simulator/qemu-s9/axi-selected-511e
```

The run completed in 26,663 ms, exited zero and retained complete output. Both
selected compilers returned zero with empty diagnostic streams. The original
582 primitive assertions and both 510-case RAM comparisons also passed.

| Retained output | Bytes | SHA-256 |
| --- | ---: | --- |
| Generated HDL fixture | 14,250 | `487ca147fdb023acde4c03d779ee1df15b1e730f73a90a9b63379fc68fd02331` |
| Actual AXI trace | 5,072 | `bfd77d63ca7494012be93708b853e462e8aa26a97c266620e566799ea0c44061` |
| Compiled VVP program | 212,935,686 | `290f215b47da63aa2cb42772e15e93d35f877fbda16b927499abff2491415110` |
| Compiled program archive | 25,491,671 | `a2a2e4bdbebeb79ae521b7564e281ec8d99f478a59fd98227ef9d04718e79fbe` |

The [failed 511d run](maxigp0-failures-511.json) remains failed. Its HDL
assertions completed, but its result checker rejected the original PS7
startup notice that no CPU model is present. Run 511e checks that literal
against the original source and requires its exact expansion once at output
line zero. It retains the notice and explicit false CPU/board flags. Other
warnings remain failures. The HDL source and complete AXI trace are identical
in both runs. Each compiled VVP program has its own identity; compiler byte
reproducibility is not claimed.

The [freeze](../qemu-s9/axi-driver-511e/manifest.json) retains 408 source entries,
matching before/after maps, 11 focused tests, strict Clippy, formatting and the
exact release helper. The [HDL report](../qemu-s9/axi-selected-511e/hdl-report.json),
[process result](../qemu-s9/axi-selected-511e/result.json),
[fixture](../qemu-s9/axi-selected-511e/axi-probe.v),
[trace](../qemu-s9/axi-selected-511e/axi-trace.txt) and
[compiled archive](../qemu-s9/axi-selected-511e/selected-vvp.tar.gz) retain the
actual run. Both source reviews pass, including the narrow notice change.
The [independent execution audit](maxigp0-independent-execution-511e.json)
checks all source/input identities, 46 mounts, complete output and all 72
transactions.

The next step is to connect the existing QEMU processor to these real bus
transactions and execute unchanged archived firmware. Original reset/boot,
ASIC work, independent mining verification, exact board identities and
physical timing remain separate gates. This component pass does not complete
S9 milestone 2.

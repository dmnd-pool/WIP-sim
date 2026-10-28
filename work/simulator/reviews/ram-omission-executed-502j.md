# RAM input representation check 502j

The isolated vendor-model comparison passes. It covers all 15 RAMB36E1
instances in the original 497 netlist. It does not pass the selected structural
gate, which still reports the same unresolved declarations and open ports.

The Rust helper copies every parameter from each selected RAM instance. It
requires TDP mode, equal 18-bit or 36-bit port widths, disabled error correction
and no cascade extension. It compiles the same test twice. One form omits the
four original input ports. The other form connects those ports to explicit Z
(high impedance). It does not set them to zero or one.

Both executions pass 510 observations. These cover writes and reads through
both ports, data and parity, unknown data, and reset. The complete output traces
match byte for byte. Each observation also checks that the four optional input
ports read Z. The omitted form must produce exactly its 60 known diagnostics.
The explicit form must compile without a diagnostic. This separate diagnostic
oracle does not relax the selected-netlist rule.

The [independent review](hdl-selected-independent-execution-502j.json) checks
all 301 frozen source entries, 34 mounted inputs, 20 output streams and all 15
parameter blocks against the prior netlist. It independently checks all 510
data observations and 306 fully printed parity values. The 18-bit models print
mixed unknown upper parity bits in hexadecimal. Their lower two parity bits
are checked by the actual HDL assertion, but are not separately recoverable
from that text trace.

The [run report](../qemu-s9/hdl-selected-502j/hdl-report.json) includes the full
generated test source, parameter identities and both traces. The
[frozen helper](../qemu-s9/hdl-driver-502j/manifest.json) passes six focused Rust
tests, strict Clippy and its release build. This executed command took 6,556 ms:

```sh
work/simulator/qemu-s9/hdl-driver-502j/s9_hdl_validate /home/user/src/md-sim simulator/firmware/s9-hdl-inputs-502i.json work/simulator/qemu-s9/hdl-selected-502j
```

The command returns failure because its final selected-netlist structural
check remains incomplete. Use a new output path to repeat it. All 582 earlier
primitive assertions also pass in this run. No selected VVP program, processor
execution, physical timing or original-board gate is accepted here.

The next converter output has a separate
[adaptation manifest](../../../simulator/tools/fasm2bels/structure-v2/adaptation.json).
Its checker requires all original cells, parameters, connections, assignments
and routing constraints to remain unchanged. Only the recorded declarations,
top timebase and explicit-Z port representation may differ.

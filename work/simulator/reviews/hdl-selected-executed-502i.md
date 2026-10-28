# Selected HDL check 502i

The isolated vendor primitive checks pass. The selected netlist structural gate fails.
This is a component result. It is not an original-board boot or an exact mining target pass.

This command was executed from the repository root:

```sh
work/simulator/qemu-s9/hdl-driver-502i/s9_hdl_validate /home/user/src/md-sim simulator/firmware/s9-hdl-inputs-502i.json work/simulator/qemu-s9/hdl-selected-502i
```

Use a new output path for a repeat. The command exits with failure because the
selected structural gate is incomplete. The frozen helper and inputs stay fixed.

The run took 5,735 ms. The actual Icarus 12.0 compiler and VVP runtime used a
closed library set. They did not use the compiler build tree. The unchanged
vendor LUT6_2, CARRY4 and FDRE models passed all 582 driven assertions:
64 LUT inputs, 512 carry inputs and six flip-flop cases. The LUT fixture starts
at X and then drives its first input. This corrects the failed 502h stimulus.
It does not prove that the selected netlist settles from static initialization.

The selected Verilog is the complete 497 converter output. Icarus returned zero,
but emitted 1,453 implicit-wire warnings, 60 dangling-input warnings and a top
timebase warning. The strict diagnostic rule rejected the result. No selected
VVP artifact was produced. The implicit wires are alias-map sinks. The RAM
warnings refer to four ports on each of 15 RAMB36E1 instances. Those ports need
a checked representation before the structural gate can pass.

The next iteration changes the converter output in a separate recorded
derivative. It must preserve all existing assignments and parameters. It will
test omitted RAM inputs against explicit Z through the original vendor model.
It will not assign an invented zero or one to these inputs.

The [independent execution review](hdl-selected-independent-execution-502i.json)
rehashes all 283 frozen source entries, 34 mounted inputs and 12 output streams.
The [input profile](../../../simulator/firmware/s9-hdl-inputs-502i.json),
[frozen build](../qemu-s9/hdl-driver-502i/manifest.json),
[raw result](../qemu-s9/hdl-selected-502i/result.json) and
[HDL report](../qemu-s9/hdl-selected-502i/hdl-report.json) retain the exact identities.
Earlier failed attempts remain in the [failure record](hdl-selected-failures-502.json).

The vendor PS7 module is a port shell. It does not execute a processor or AXI
transactions. The missing monitor feature table, 33 electrical ambiguities,
static startup behavior and physical timing remain open.

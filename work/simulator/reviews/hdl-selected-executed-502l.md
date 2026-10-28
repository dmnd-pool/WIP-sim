# Selected FPGA structural check 502k–502l

The complete converter preservation check and strict HDL compilation pass.
The selected logic has not yet executed a processor bus transaction. This
result does not pass the original-board, physical timing or mining gates.

These commands were executed from the repository root:

```sh
work/simulator/qemu-s9/netlist-driver-502k/s9_netlist_reconstruct simulator/firmware/s9-netlist-tool-inputs-502k.json work/simulator/qemu-s9/netlist-selected-502k
work/simulator/qemu-s9/hdl-driver-502l/s9_hdl_validate /home/user/src/md-sim simulator/firmware/s9-hdl-inputs-502l.json work/simulator/qemu-s9/hdl-selected-502l
```

Use new output directories to repeat these commands. The HDL profile selects
the exact retained 502k output by hash. A different conversion output needs
its own reviewed input profile.

The converter ran for 85,653 ms. Its
[preservation check](../qemu-s9/netlist-selected-502k/structure-comparison.json)
retains all 11,122 cells, attributes, parameter texts, existing port connections
and assignments. All existing nonblank lines remain. The top header stays in
order, and its end marker remains last. The XDC routing constraints are
byte-for-byte identical to the original 497 output.

The separately recorded [tool adaptation](../../../simulator/tools/fasm2bels/structure-v2/adaptation.json)
adds exactly 1,453 scalar declarations for existing alias sinks, one top
timebase, and 60 explicit high-impedance connections for the four already
omitted input pins on 15 RAMs. It does not add a known zero or one, change a
route, change a primitive parameter or replace firmware. Its exact source
derivation and input profile have an
[independent source review](netlist-structure-independent-source-502k.json).

The strict HDL run took 14,863 ms. Both selected elaboration and VVP compilation
returned zero with empty standard output and standard error. The run also
passed all 582 primitive assertions. Both forms of the RAM test passed all
510 observations and produced identical complete traces. This run prints
parity in binary, including the unknown upper bits of the 18-bit modes.
The deliberate malformed HDL probes remain separate negative tests.

| Artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| Selected Verilog | 12,607,262 | `3ea22dc51d32539e0a52ff83d7152d82009f4e50883bc20e6643e6672d1a2c9b` |
| Unchanged XDC | 8,065,912 | `96abe81a11f36f77f3ebcb2e236e829a8d8ae96ddacd6308362354afc3146b27` |
| Compiled VVP program | 211,790,335 | `27f1fc48f488b1606dc57b19f31b2b768a542427387b2ff420d23790c38187b6` |
| Retained VVP archive | 25,452,295 | `75284c7047319c6675fcec9bb2c4bfc54133477b8aeea37a03e3e39f54c7d315` |

The [converter freeze](../qemu-s9/netlist-driver-502k/manifest.json) records
303 source/input entries, 11 focused Rust tests, strict Clippy and a release
build. The [HDL freeze](../qemu-s9/hdl-driver-502l/manifest.json) records 302
entries, six focused tests, strict Clippy and a release build. Each source
capture matches the bytes before and after its build. Both isolated runs
exited zero without truncated evidence, under the existing resource limits.

The [HDL report](../qemu-s9/hdl-selected-502l/hdl-report.json),
[process result](../qemu-s9/hdl-selected-502l/result.json),
[input profile](../../../simulator/firmware/s9-hdl-inputs-502l.json) and
[compiled archive](../qemu-s9/hdl-selected-502l/selected-vvp.tar.gz) retain the
actual evidence. The [earlier failures](hdl-selected-failures-502.json) remain
unchanged. Independent execution audits now pass for both the
[converter](netlist-structure-independent-execution-502k.json) and the
[HDL run](hdl-selected-independent-execution-502l.json). The HDL audit also
checks all 510 data and parity results from the binary traces.

The vendor PS7 is still a port shell. The next test must drive and observe the
recovered MAXIGP0 processor bus and clock path. It must not invent a read value,
ready signal or reset success. The missing monitor feature table, 33 electrical
ambiguities, FPGA build identity and physical timing remain unresolved.

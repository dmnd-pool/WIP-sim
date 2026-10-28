# Selected HDL throughput514

The actual10us fabric interval completed in2.472s of host wall time. The100us
case hit the unchanged10s command limit and failed. The1ms case was not run.
This is a practical open gate for full firmware execution with the current
Icarus interpreter. The clock was not rescaled or stopped between accesses.

The separate actual bus regression passed after the shared process transport
was extracted. It checked all72 operations, two resets and all event/clock
oracles in31,669ms. This pass does not turn the throughput failure into a pass.

The [exact input and assessment receipt](hdl-throughput-executed-514-inputs.json)
pins the [423-file helper freeze](../qemu-s9/live-driver-514/manifest.json),
[throughput report](../qemu-s9/live-throughput-514/report.json), failed result
and [bus regression report](../qemu-s9/live-selected-514/hdl-report.json).
Both runs retained complete output and owned process cleanup.

The benchmark uses the exact compiled program and C plugin from512e. It
measures startup separately from each real command. Startup took6.489s and
6.538s in the two cases. The completed10us interval contains1000 actual fabric
cycles at the explicit100MHz clock. Its next clock boundary was checked
against the unchanged1ps origin and5000ps half-period. The failed100us command
has no completion or inferred cycle count. No CPU-time or physical calibration
measurement was made. No linear boot-time estimate is asserted.

These commands were executed from the repository root. The first command
returns failure for the retained100us timeout. Use new output paths to repeat.

```sh
work/simulator/qemu-s9/live-driver-514/s9_live_benchmark \
  /home/user/src/md-sim work/simulator/qemu-s9/live-throughput-514

work/simulator/qemu-s9/live-driver-514/s9_hdl_validate \
  /home/user/src/md-sim \
  work/simulator/qemu-s9/live-driver-514/source/simulator/firmware/s9-hdl-inputs-512.json \
  work/simulator/qemu-s9/live-selected-514
```

The shared transport passed27 tests. These include the five preserved real
process tests and four new tests for child-initiated frames, queued replies,
partial EOF and bounded deadlines. Strict Clippy, formatting and release
builds passed. The actual72-operation regression uses the unchanged selected
HDL and assertions through the extracted transport.

Next, assess a faster existing four-state simulator or an optimization with
independent equivalence evidence. Do not freeze idle clocks, replace unknown
values with zero, enlarge deadlines to hide this failure, or claim a timed CPU
stall from the atomic reference stage. The CPU bridge, firmware, mining,
original-board and physical timing gates remain open.

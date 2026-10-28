# Live S9 programmable-logic component512e

The selected FPGA logic passed72 actual AXI register operations and two reset
sequences through the new live interface. The original benchmark also passed
in a separate invocation of the same frozen helper. This is a programmable
logic and transport result. It does not execute the S9 CPU or firmware.

The [input and result receipt](live-pl-executed-512e-inputs.json) pins both runs,
the [421-file source freeze](../qemu-s9/live-driver-512e/manifest.json), the
compiled models and the retained reports. The helper SHA-256 is
`1b386c34d3e0a300a283543c042806e4e47a8767359d537a2b83fef71fc13025`.
The [live report](../qemu-s9/live-selected-512e/hdl-report.json) and
[baseline report](../qemu-s9/axi-baseline-512e/hdl-report.json) both have exit0
and complete output. They took31,218ms and27,328ms respectively.

These commands ran from the repository root. Use new output paths for a repeat.
The prepared workspace must contain the pinned source, compiler and Icarus
archives. The helper checks their hashes before isolated execution.

```sh
work/simulator/qemu-s9/live-driver-512e/s9_hdl_validate \
  /home/user/src/md-sim \
  work/simulator/qemu-s9/live-driver-512e/source/simulator/firmware/s9-hdl-inputs-512.json \
  work/simulator/qemu-s9/live-selected-512e

work/simulator/qemu-s9/live-driver-512e/s9_hdl_validate \
  /home/user/src/md-sim \
  work/simulator/qemu-s9/live-driver-512e/source/simulator/firmware/s9-hdl-inputs-511d.json \
  work/simulator/qemu-s9/axi-baseline-512e
```

Rust owns the128-byte protocol, sequence and time checks, child process,
deadlines, cancellation, output limits and evidence. A small C module uses the
existing Icarus Verilog Procedural Interface (VPI). This C interface is required to enter
the emulator's read-write and read-only event phases. Verilog supplies the
actual four-state signal and AXI behavior. No CPU or HDL emulator was added.

The private response descriptor is separate from native stdout and stderr.
The live run retained81 requests,81 completions,53 raw output changes,10 raw IRQ
changes and one transport-ready message. IRQ means interrupt request. It joined all72 wire observations to
the unchanged AXI task's text output. The three real controller blocks each
returned version`00901002` and build`5fca47e9`. Stop closed the owned child with
exit0, all streams at EOF and no truncation or cleanup error.

Commands apply only at a future timestamp. Existing clock edges and their
sequential updates settle before the input batch. New input work then settles
before the read-only receipt. The actual event oracle checked two nonblocking
assignment (NBA) waves,
ordered X/Z transitions and completion at the same1000ps timestamp. Duplicate
completion was rejected. Split and unsplit clock commands produced identical
raw event timelines. A separate unknown-to-zero clock case produced no early
edge. The C plugin compiled with strict warnings and UBSan traps; execution
produced no sanitizer diagnostic.

The selected test explicitly supplies100MHz and two1us reset intervals. The
interface has no default clock. Its clock conversion and phase rules are
declared digital policies. A reported next owned clock edge is not the next
HDL kernel event and does not permit CPU run-ahead. CPU synchronization,
physical timing, original-board boot, firmware and mining gates remain open.

Both invocations also passed582 primitive assertions and two510-case RAM
comparisons. Rust checks passed23 ordinary tests and the explicitly selected
retained511e generator comparison. Strict Clippy, formatting and release
build passed. The actual supervisor tests covered fragmented and malformed
frames, partial EOF, timeout, cancellation, output limits and descendant cleanup.

Per-invocation limits stayed unchanged:900s wall,600s CPU per process,4GiB
address space,32 processes,1GiB scratch and96MiB trace. There is no aggregate
cgroup resource claim. The compiler's temporary tree is removed after a
successful plugin build and hash check. The pinned compiler input, build logs
and plugin remain retained. Separate baseline and live runs retain both actual
compiled programs within the32MiB archive bound. VVP output contains heap
addresses, so no cross-build byte identity or cached baseline was assumed.

Failed512 through512d runs remain failed and retained. They exposed a reserved
fixture identifier, a declaration initializer that did not start an event,
implicit array-sensitivity diagnostics, and an unsupported procedural force
on a bit-select. The fixes preserve the assertions and time bounds. The final
wrapper uses normal connections for14 external inputs and resolved drivers
for two inouts. No internal FPGA output is forced by those input connections.

The next step is a separate bridge to the existing QEMU CPU. It must bind
FCLK1, reset0 and actual IRQ signals, and distinguish an atomic MMIO reference
from a fully timed CPU stall. MMIO means memory-mapped input/output.
This component does not establish either mode.

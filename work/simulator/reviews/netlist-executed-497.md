# Selected FPGA conversion 497

The pinned existing fasm2bels converter produced Verilog and XDC constraints
from the checked original FPGA features. The isolated process exited with code 0
in 89,218 ms. No output was truncated. This is a conversion result. Independent
HDL parsing, primitive behavior and execution are still required.

The input is the exact 4,719,842-byte FASM file from the passed 482d round trip.
It has SHA-256 `3654907a9bb4de393c8d0a40fab86d2d04555461f2e2b80d1ce5a38c7b3283e4`.
The Rust launcher binds the complete converter profile, all 40 source files,
the parser/database profile, and the actual 482d assessment, output and input
record. It checks the prior configuration-bit comparison before execution.
The converter consumed the saved file once, through the checked bounded parser.
All 219,702 canonical feature names match the earlier result. The whole/chunk
parser fixture, exact consumed byte count and SHA-256 also pass.

The existing graph constructor processed 13,440 tiles and 90 tile types. Its
271,196,160-byte SQLite database had 3,607,498 wires and 1,200,074 nodes.
SQLite `quick_check` returned `ok`. Explicit queries found no unassigned wire,
missing node reference or missing physical-tile reference. The graph remained
in private temporary storage and was removed on process exit. Its hash and
query results are retained. This is not an independent retained-graph audit.

The retained outputs are:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `selected.v` |12,538,532|`31bef35aaa1e18af42fdd0c01d1045a5d56ae031c569adf9440eab5fa416e848`|
| `selected.xdc` |8,065,912|`96abe81a11f36f77f3ebcb2e236e829a8d8ae96ddacd6308362354afc3146b27`|

A lexical declaration count found 11,122 instances of 17 primitive types. It
includes one PS7 processor boundary, one BUFGCTRL clock buffer, 15 RAMB36E1 blocks,
6,712 LUT6_2 cells and 3,986 flip-flops. This count helps select the existing
primitive library. It is not an HDL syntax, connectivity or behavior check.
No orphan-sink or clock-route relaxation flag was passed.
The upstream log retains 33 electrical-setting ambiguity messages: 14 input
standards and 19 output/bidirectional standard, drive and slew settings. We
supplied no guessed defaults. These settings remain unresolved; see the
[I/O ambiguity record](netlist-io-ambiguities-497.json).

The converter revision is `bafbcd8727e0807d6a620d1cf5dd151111c4a322`.
The separate [tool adaptation](../../../simulator/tools/fasm2bels/adaptation.json)
moves four optional VPR/interchange imports to their existing use sites.
Original source files remain unchanged in the archive. Rust reapplies every
recorded text change and checks the resulting bytes. Independent source review
also checked all non-import syntax nodes and the unchanged import multiset.
The existing parser API uses the separately recorded 482d grammar correction.
The thin Python adapter supplies the original parsed tuples to the existing
converter. It creates no feature, primitive, connection or successful result.

The preparation budget was 4 GiB virtual address space, 600 CPU seconds, 900 wall
seconds,16 process slots, 1 GiB per file and scratch tmpfs, and 128 MiB combined
trace. The graph has 3.61 million possible wire instances before the upstream
processor-system mask, which led to this bounded tool budget. It is separate
from firmware limits. The process used private namespaces, read-only captured
inputs and no egress. These are per-process and per-tmpfs limits.
A [host process sample](netlist-resource-sample-497.json) recorded 2,005,356 KiB
resident high-water usage. This sample is not a final aggregate memory measure.

All 1,017 input mounts were rehashed after execution. They refer to 1,003 unique
objects with 109,585,825 bytes. All saved output text matches both the declared
hashes and the retained process output. Seven focused Rust tests, strict scoped
Clippy, formatting and the release build pass. The frozen helper has SHA-256
`913c85d28559f26b89308d180f416ee4f8188c4195cec8a71c4e972e6b4f7d2d`.
Its 271 relevant source entries were unchanged across the checks and build.
Ten deltas are retained against source snapshot 494. Base source copies were
rehashed. Unrelated binaries and examples are outside this source closure.

These commands were executed from the repository root:

```sh
export CARGO_TARGET_DIR=work/simulator/check-target-085
export CARGO_INCREMENTAL=0
export CARGO_BUILD_JOBS=1
export TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227
cargo test -p md-sim-firmware --example s9_netlist_reconstruct --offline --locked
cargo clippy -p md-sim-firmware --example s9_netlist_reconstruct --offline --locked -- -D warnings
cargo build -p md-sim-firmware --example s9_netlist_reconstruct --release --offline --locked
work/simulator/qemu-s9/netlist-driver-497/s9_netlist_reconstruct \
  simulator/firmware/s9-netlist-tool-inputs.json \
  work/simulator/qemu-s9/netlist-selected-497
```

Use a new output directory for a repeat. The profile requires the recorded
archive files, passed 482d evidence and captured Python 3.14 runtime. No acquired
source executes on the host. Raw output byte reproducibility is not yet a
cache contract: Python set iteration can affect graph and layout order.

The [input record](netlist-executed-497-inputs.json) pins the full evidence.
The [source review](netlist-conversion-independent-source-497.json) has no open
finding. The [independent actual review](netlist-conversion-independent-execution-497.json)
also passes for conversion. It rehashed the source and input closure, joined
the raw process streams, checked the emitted files and independently rebuilt
the expected complete feature-name set. The unresolved monitor table
remains in the assessment. The original FPGA build identity, exact board
completion and physical timing gates remain open. The next step uses an
existing Verilog parser/simulator and pinned primitive models to check the
retained netlist. Missing behavior must remain explicit.

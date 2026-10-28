# HDL validation resume point 502

Update28 October: the user resumed work. The complete signed gperf input and
[isolated Icarus tool build502f](hdl-tool-executed-502f.md) now pass. Continue
from [the current checkpoint](hdl-acquisition-checkpoint-502b.json). The
remaining text records the earlier paused state; do not repeat closed work.

The user asked to finish the fresh CLI test, then save a checkpoint and pause.
Do not start an HDL build or simulator while that pause is active. The full
simulator and exact-board gates remain incomplete.

## Completed work to retain

- Export482d passed the full non-ECC configuration round trip. All327,095 set
  bits match original bytes, regenerated frames and the independent Rust union.
  The one-byte grammar correction and bounded parser adaptation are explicit.
- Conversion497 passed in89,218ms. Its frozen Rust helper invoked the existing
  pinned fasm2bels converter in isolation. It retained12,538,532-byte Verilog
  and8,065,912-byte XDC files. All219,702 feature names match482d. Independent
  source and actual conversion reviews pass. The missing monitor table and33
  ambiguous electrical settings remain open. See [the report](netlist-executed-497.md).
- The original Icarus12.0 archive is pinned to commit
  `4fd5291632232fbe1ba49b2c26bb6b2bf1c6c9cf`. Its2,997,829 bytes have SHA-256
  `690f02e6fd0e0c05e4768bcb65a2c7f1b64d20289344d1ad3ccfb69bae8bd55b`.
  All5,050 source blobs match the exact recursive Git tree. There are no missing
  blobs or symlinks. Do not repeat that audit without an input change.
- Nineteen original vendor files are saved under
  `artifacts/files/source/source-unisim-1c8e05f/`. They match Git blobs at
  `1c8e05fd1e9a79ceb8b996a0996674122eed086f`. They cover the17 emitted primitive
  names, `glbl.v` and README. The licence reuses the existing byte-identical
  Apache2.0 licence after matching its publisher Git blob.
- The25 initial HDL records passed artifact inspection, incremental archive
  verification, coverage/catalogue rebuild and verification, archive/document
  indexes and guide links. The archive check read5,835,312 bytes. The catalogue
  then held13,982 records. Later gperf failure records are a separate addition.

## Exact next path after resume

1. Read [the current acquisition checkpoint](hdl-acquisition-checkpoint-502.json)
   and the parent's fresh-CLI checkpoint. Keep495/500 and static FPGA results
   separate. Use new output directories for new executions.
2. Complete the required gperf source input. The first official download
   timed out after40seconds with847,474 of1,215,925 expected bytes. The partial
   original and headers are preserved separately. It is not a build input.
   The543-byte detached signature is saved; signature verification is not
   claimed. [The transfer record](gperf-original-preserved-502.json) gives the
   exact paths and identities. After resume, fetch a fresh complete archive
   from `https://ftp.gnu.org/gnu/gperf/gperf-3.1.tar.gz` into a new path, with a
   bounded transfer. Check the archive and source identity before any build.
   Do not overwrite or concatenate onto the retained failed original.
3. Reuse the existing isolated preparation framework and compiler snapshot.
   The snapshot is `work/simulator/firmware/toolchain-snapshot-003/manifest.json`,
   SHA-256 `e2eabd7bd16c22397c37cfbcfd0115223406f7850b5ad8f0733de6ac79e4d12a`.
   It has697,927,858 source bytes and no gperf. Host `autoconf`, `bison`, `flex`,
   `make`, `gcc` and `g++` exist; bison/flex are not declared snapshot roots.
   Capture their exact runtime/data closure explicitly. Do not mount an
   unrecorded host tool tree. Existing code patterns are
   `examples/build_qemu_tool.rs` and `examples/freeze_cgminer_toolchain.rs`.
4. Add a small Rust preparation driver for the existing gperf and Icarus build
   paths. Icarus's archived `autoconf.sh` invokes autoconf and gperf. Its README
   lists GCC/C++, make, bison, flex and readline/ncurses dependencies. Build only
   inside the declared namespaces with run-owned storage and measured limits.
   Retain source, toolchain, build logs and output hashes. This driver is not
   yet implemented. No proposed build command has been reported as tested.
5. Inspect transitive primitive modules/includes before elaboration. RAMB36E1.v
   also defines `RB36_INTERNAL_VLOG`. Do not add placeholder primitive modules.
   Vendor PS7.v explicitly states at line5167 that it has no behavioral Zynq
   model. It can provide a structural port shell only. Actual CPU/AXI behavior
   must use the existing QEMU implementation or an explicit component-test
   boundary. Neither a shell nor forced success may satisfy firmware execution.
6. Use existing Icarus parsing/elaboration on the exact497 Verilog plus the
   required vendor model closure. Preserve all diagnostics and unresolved
   properties. Verify module/port references, structural connections, constants,
   clocks and reset propagation. Unknown PS7/monitor behavior and ambiguous
   electrical settings remain separate from functional programmable-logic
   tests. Add independent primitive vectors and explicit boundary transactions.
   Only then connect the model to the real firmware hardware path.

The relevant files are [conversion inputs](netlist-executed-497-inputs.json),
[conversion review](netlist-conversion-independent-execution-497.json),
[I/O ambiguities](netlist-io-ambiguities-497.json),
[Icarus tree audit](iverilog-original-tree-audit-502.json),
[vendor acquisition](unisim-selected-results-502.json), and
[metadata preservation](hdl-primary-metadata-preserved-502.json).
Raw netlist byte reproducibility has no cache/resume claim yet; Python set
iteration can affect layout order. There is no physical timing result, exact
FPGA build-source identity, original-board boot or complete target matrix pass.

The first metadata-copy attempt assumed `.json.headers`. It failed before any
copy. The actual `.headers` files were then copied and checked. Four small
metadata responses now have two preserved identical archive copies. Remove a
duplicate only after pair verification, reference updates and a cleanup record.
The original acquisition `/tmp` copies also remain pending a bounded pair audit.

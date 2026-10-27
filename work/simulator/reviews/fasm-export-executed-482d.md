# Selected FPGA assembly round trip 482d

The static export and full non-ECC configuration round trip pass. The isolated
process exited with code 0 in 29,469 ms. Its output was not truncated. This is
not a netlist, physical timing result, firmware boot or exact-board acceptance.

The selected original S9 FPGA configuration is unchanged. Its compressed input
has SHA-256 `bdf2b3bff4f5f06169b302ccfac8424e46bb2089b6313e363457bf2ae1d4bd40`.
The configuration payload has SHA-256
`49d0faa400fbc8bed3f3e651da2c6da5f3184a08f942ec95db59c2526dcbea78`.
The existing pinned frame map supplies 5,144 frame addresses and byte offsets.
Every original frame is rehashed. This path does not add a second packet parser.

The unchanged Project X-Ray disassembler produced 221,975 raw features. The
existing FASM formatter retained 219,702 features and removed 2,273 zero-only
defaults. Every positive predicate from the separately checked Rust result is
present. No non-ECC set bit is unknown. The saved text retains 11,859 ECC-bit
annotations. The unresolved `MONITOR_BOT_PELE1_X67Y79` table remains explicit.
A zero-only match does not prove that a resource is used.

The 4,719,842-byte FASM file has SHA-256
`3654907a9bb4de393c8d0a40fab86d2d04555461f2e2b80d1ce5a38c7b3283e4`.
It is byte-identical to the text saved by run 482b. The parser consumed all
115,981 lines in 19 chunks of at most 262,144 bytes. It checks the consumed
file hash and size. Formatter items with embedded line breaks are rejected
before chunking. Each parsed item enters the existing assembler API.

All 327,095 original non-ECC set bits were regenerated. The original frame set,
regenerated frame set and independent Rust union of positive asserted bits
have the same SHA-256:
`d375439df482fbe33e496cc00e2892dd00bf9fd11b32292544673193653258b2`.
There are no missing or extra bits. Parsed formatted feature names also have
the expected complete count and digest.

The tool adaptations are explicit. The unchanged whole-parser fixture exposed
an escaped-quote defect in the archived textX grammar. A separate one-byte
correction excludes backslash from the non-escape character class. Rust checks
the complete adapted tool-profile identity, all source hashes, the original
and derived grammar, the patch and the saved original failure. It verifies
the exact one-byte derivation. The same escaped-quote fixture now passes both
a whole parse and bounded parses. Full parsed tuples include comments,
annotations, a bit range and an explicit zero feature. No fixture assertion
was removed. The original grammar, profile, helpers and failed runs remain.
See the [grammar adaptation](../../../simulator/tools/fasm/grammar-adaptation.json)
and [independent source review](fasm-grammar-independent-source-482d.json).

The limits remained 1 GiB virtual memory, 120 CPU seconds, 180 wall seconds,
16 process slots, 32 MiB file size, 64 MiB trace and 64 MiB per scratch tmpfs.
The process used private namespaces, read-only captured inputs and no egress.
These are per-process and per-tmpfs limits, not an aggregate cgroup claim.
All 997 mounts were checked again after execution. They refer to 959 unique
objects with 89,740,217 bytes. No downloaded source executed on the host.

Nine focused Rust tests, strict scoped Clippy and the release build pass.
The frozen helper has SHA-256
`2a9cbdc6e827866ee06c7dc3c2535381b3c76f6db2fee60fb8eafa34c3a94e99`.
Its 288 relevant source entries were unchanged across the checks and build.
Eight source deltas are retained against common snapshot 491; unchanged base
source copies were rehashed. Unlinked examples are outside this helper's
source closure. Full workspace checks remain required after later edits.

These commands were executed from the repository root:

```sh
export CARGO_TARGET_DIR=work/simulator/check-target-085
export CARGO_INCREMENTAL=0
export CARGO_BUILD_JOBS=1
export TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227
cargo test -p md-sim-firmware --example s9_fasm_export --offline --locked
cargo clippy -p md-sim-firmware --example s9_fasm_export --offline --locked -- -D warnings
cargo build -p md-sim-firmware --example s9_fasm_export --release --offline --locked
work/simulator/qemu-s9/fasm-export-driver-482d/s9_fasm_export \
  simulator/firmware/s9-fpga-feature-inputs-475.json \
  simulator/firmware/s9-fasm-tool-inputs-escaped-quotes.json \
  work/simulator/qemu-s9/features-selected-475/features.json \
  work/simulator/qemu-s9/fasm-export-482d
```

Use a new output directory for a repeat run. Required original inputs and
prepared frame/feature records are listed in the two manifests. The capture
also requires the recorded Python 3.14 runtime, pure PyYAML sources and the
explicit native extensions. It does not capture unrelated site packages.

The [input record](fasm-export-executed-482d-inputs.json) pins the process
specification, helper, output, source freeze and complete input identities.
The [assessment](../qemu-s9/fasm-export-482d/assessment.json) and
[saved FASM](../qemu-s9/fasm-export-482d/selected.fasm) retain the result.
The [independent execution review](fasm-export-independent-execution-482d.json)
also passes. It rehashed every captured input and recomputed the raw frame-bit
set directly from the checked original frame bytes. The next implementation step is
isolated conversion to primitive cells and connections with the pinned
fasm2bels tool. Dispatch coverage alone does not prove that conversion.

Earlier failures remain at `fasm-export-482`, `fasm-export-482b` and
`fasm-export-482c`. The first failed while encoding retained output. The second
retained output but hit a memory limit inside the parser/assembler block; its
exact allocation site was not recorded. The third preserved the original
whole-parser escaped-quote failure. The final run uses bounded parsing,
releases completed intermediate objects and records bounded failure locations.

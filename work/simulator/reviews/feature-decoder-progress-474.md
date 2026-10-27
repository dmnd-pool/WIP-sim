# FPGA feature decoder progress 474

The selected archived FPGA configuration now passes the isolated Rust feature
probe and the unchanged upstream matcher comparison. This is a static analysis
result. No firmware or exact-board gate passes here.

Run `features-selected-474c` exited with code 0 in 951 ms. It used the unchanged
512 MiB memory limit. Its 42,184,411-byte report was not truncated. The first
run, `features-selected-474`, failed with an allocation error and remains saved.
The fix streams a borrowed report through a bounded writer. It removes the
two extra full report copies and keeps the 64 MiB output bound.

Review found that duplicate alias destination names were rejected only after
a feature matched. The final parser rejects them before matching. A negative
test checks this case. Run474c repeats the actual inspection after this fix;
its full decoding object equals the earlier passing run474b. The parent review
found no further source defects. The [independent upstream run476](../qemu-s9/feature-oracle-476/assessment.json)
passes every segment digest, result field and count. It exited with code 0 in
6,065 ms. It used the exact final474c report and did not truncate output.

The probe found 223,128 matching feature predicates across 5,400 segments.
There are 13,038 zero-only defaults; these do not establish resource use.
Matching positive predicates account for 323,417 of 327,095 set data bits.
The report keeps all 3,678 remaining set bits and 346 segments with missing
feature tables explicit. These counts do not establish a complete netlist.

New source: `crates/md-sim-firmware/src/bitstream/series7/features.rs`,
`tests/series7_features.rs`, `examples/s9_feature_probe.rs`, and
`examples/support/fpga_feature_inputs.rs`. The Series-7 module exports features;
the tile-grid module exposes checked metadata only to sibling modules.

The verified command, run from the repository root, is:

```sh
work/simulator/check-target-085/release/examples/s9_feature_probe \
  simulator/firmware/s9-fpga-feature-inputs.json \
  work/simulator/qemu-s9/features-selected-474c
```

Use a new output path for a new run. The report keeps matched positive
conditions, zero-only defaults, aliases, missing tables and every unmatched
non-ECC set bit separate. It grants no reconstructed-netlist or exact-board
acceptance result. Four new feature behavior tests, the existing Series-7
tests, the report bound test, strict scoped Clippy, and the release build pass.
[The checkpoint](feature-decoder-checkpoint-474.json) pins the inputs, helper,
results and check logs. Tile mapping470 already passed a separate byte audit.

Agent `cleanup_resume` owns the independent oracle example and thin Python
API harness. The original seven upstream files are archived under
`artifacts/files/source/source-prjxray-oracle-c9f02d/`, with exact source
commit `c9f02d8576042325425824647ab5555b1bc77833` and acquisition manifests.
The oracle invokes the unchanged Python matcher only in isolation.

Eight feature/connectivity/API inputs and eight oracle-source/API inputs have
been archived and recorded since catalogue check470. Artifact inspection,
incremental archive verification, coverage rebuild, catalogue build and
verification, and guide link checks pass. The archive check read 16 new paths
with 10,718,213 bytes and reused 13,179 unchanged prior hashes. Catalogue
verification counted 13,585 artifact records. It reports no errors. Original
files and response headers remain unchanged. Logs use the `features474-`
prefix in `work/simulator/checks-2026-10-26-001/`.

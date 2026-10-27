# Independent FPGA feature oracle 476

Both actual comparisons pass. The unchanged Project X-Ray matcher and alias
reader ran in the isolated process runtime. The new Rust launcher compared
every complete feature record, every segment identity, every unresolved
segment, and every unexplained bit. No firmware executes in this check.
This result does not establish a logic netlist, physical timing, or an exact
board acceptance gate.

| Input | Tables | Matches | Decoded segments | Real aliases | Missing segments | Unexplained non-ECC bits | Process time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Final probe 474c | 6 | 223,128 | 5,400 | 0 | 346 | 3,678 | 6,065 ms |
| Expanded probe 475 | 28 | 231,546 | 5,745 | 8 | 1 | 0 | 6,700 ms |

Both processes exited with code zero and untruncated output. The expanded
comparison covers `BLOCK_RAM` predicates and all eight actual I/O alias
segments. Its 18,220 zero-only matches remain defaults; they do not by
themselves identify used logic. All 327,095 non-ECC set bits have a matching
predicate in the expanded tables. The remaining monitor segment has no pinned
table and remains unresolved. Complete bit coverage does not establish routing
or logic behavior.

The upstream source is commit `c9f02d8576042325425824647ab5555b1bc77833`.
Seven archived Python files were mounted unchanged with fixed size and SHA-256
checks. `TileSegbits.match_bitdata` evaluates the original predicates.
`TileSegbitsAlias` applies the original offset, site-name and range rules.
The thin Python adapter calls those APIs. Rewriting them in Rust or C would
remove the independent upstream comparison. Capture, bounds, command handling,
result comparison and result reports use Rust.

The frame coordinates come from the earlier pinned 291 frame-map report,
SHA-256 `a5e5a889a42bc929ff5c67cce88d5d60d2ef05c5682a6f1339cf1ec47128acbe`.
The adapter checks the compressed configuration, bounded decompressed bytes,
part identity, and all 5,144 complete 404-byte frame hashes before calling the
upstream bit-data loader. This is an independent feature matcher over that
prior frame map. It is not a second packet or frame-address parser.

Each segment digest includes every full physical feature name, original
database feature name, zero-only flag, and complete list of asserted frame/bit
positions. Records and positions have canonical sort order. Strings and lists
have explicit little-endian lengths. Duplicate feature names and inconsistent
zero-only flags fail. All segment metadata and all residual records are also
compared directly. The Rust reader processes one segment at a time and avoids
the previous full-report DOM allocation failure in probe 474.

Four fixed cases executed the original alias reader with positive, negative,
zero-only, excluded-range and renamed-site conditions. The complete result of
the matching Rust alias fixture has the same digest:
`06f3efa18714aa727b81f7bde836099533cf2788ec5e9fa3fb4f229479109642`.
Future multi-bus alias tiles fail explicitly in this adapter. Each alias in
the selected grid has exactly one `CLB_IO_CLK` segment.

The process limits were 60 CPU seconds, 90 wall seconds, 512 MiB virtual memory,
16 process slots, 1 MiB file size, 1 MiB per scratch tmpfs, and 8 MiB shared trace
output. The network is private and disabled. Only captured files are mounted.
The first run captured 716 unique objects (38,902,536 bytes); the second captured
738 (42,345,504 bytes). All captured object hashes were checked again after the
runs. No acquired Python file ran on the host.

Three focused Rust tests, scoped Clippy with warnings denied, and the release
build pass. The helper freeze records 552 source files that were unchanged
before and after the build. It retains 53 source deltas from common freeze 478.
The frozen executable is 2,377,912 bytes, SHA-256
`32f77496b22835e86992220fe060259dbce3c18d98d3ab2e597f2951ad83ce06`.

These commands were executed from the repository root. Use new output names
when repeating a run:

```bash
export CARGO_TARGET_DIR=work/simulator/check-target-085
export CARGO_INCREMENTAL=0
export CARGO_BUILD_JOBS=1
export TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227
cargo test -p md-sim-firmware --example s9_feature_oracle --offline --locked
cargo clippy -p md-sim-firmware --example s9_feature_oracle --offline --locked -- -D warnings
cargo build -p md-sim-firmware --example s9_feature_oracle --release --offline --locked
work/simulator/qemu-s9/feature-oracle-driver-476/s9_feature_oracle \
  simulator/firmware/s9-fpga-feature-inputs.json \
  work/simulator/qemu-s9/features-selected-474c/features.json \
  work/simulator/qemu-s9/feature-oracle-476
work/simulator/qemu-s9/feature-oracle-driver-476/s9_feature_oracle \
  simulator/firmware/s9-fpga-feature-inputs-475.json \
  work/simulator/qemu-s9/features-selected-475/features.json \
  work/simulator/qemu-s9/feature-oracle-476b
```

The [input record](feature-oracle-executed-476-inputs.json) pins the source,
checks, manifests, original probe reports, process specifications, actual
outputs, and assessments. The independent source review has no open finding.
Independent execution review is pending. Full workspace checks remain required
after subsequent implementation changes.

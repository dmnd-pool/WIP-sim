# Selected S9 FPGA tile mapping 470

The missing Project X-Ray tile grid was acquired on 26 October. Its exact
SHA-256 matches the publisher's retained value:
`db16874f2827fc05248ad4a7ef5769deaa8e70158a60c8dd40194c48713479ee`.
The original 4,351,137-byte response is preserved under
`artifacts/files/metadata/prjxray-xc7z010-tilegrid-20261026/tilegrid.json`.
Its acquisition record is in `artifacts/manifests/` with that artifact ID.
The moving download URL is not treated as a pinned Git commit.

The new Rust module checks the tile grid against the separately pinned part
database. A tile is a physical FPGA resource region. It checks exact hashes,
JSON duplicate keys, names, unique physical coordinates/sites, bus fields,
frame membership, word bounds and aliases. It repeats packet, CRC and frame
error-code checks before using any configuration bytes. A tile alias points
to another feature layout; this stage retains the alias without interpreting
it. Whole-word ranges can overlap because feature masks can select different
bits. The report preserves this overlap instead of assigning one tile as the
exclusive owner of a word.

The isolated probe ran against the unchanged selected S9 configuration in
`original-zynq/sd-bundle-291/boot/system.bit.gz`. It exited zero in 180 ms with
no truncated output. The decoded configuration hash remains
`49d0faa400fbc8bed3f3e651da2c6da5f3184a08f942ec95db59c2526dcbea78`.

| Result | Count |
| --- | ---: |
| Database tiles | 13,440 |
| Tiles with configuration ranges | 5,686 |
| Declared configuration segments | 5,746 |
| Explicit alias segments | 8 |
| Unique associated frame words | 296,576 |
| Frame words without a tile association | 222,968 |
| Additional whole-word associations | 177,632 |

The [independent byte audit](tilegrid-byte-audit-470.json) recomputes every
segment hash and set-bit count from the original container, exact tile grid
and previously checked 291 frame offsets. It also rehashes all 5,144 prior
frames and each new isolated input object. No new Python component was added;
the independent check used the analysis environment's standard library.

The actual output is `work/simulator/qemu-s9/tiles-selected-470/header.json`.
Its process specification and result retain the inspector and input identities.
This command ran from the repository root. Use a new output path to repeat it.

```sh
work/simulator/check-target-085/release/examples/s9_bitstream_probe \
  work/simulator/original-zynq/sd-bundle-291/boot/system.bit.gz \
  work/simulator/qemu-s9/tiles-selected-470 --series7-idcode 03722093 \
  --part-json fixtures/fpga/xc7z010clg400-1/part.json \
  --part-sha256 1754ec1c7a8e0447a116984505cc422819d3be06389527de96bf192c5175d095 \
  --tilegrid-json artifacts/files/metadata/prjxray-xc7z010-tilegrid-20261026/tilegrid.json \
  --tilegrid-sha256 db16874f2827fc05248ad4a7ef5769deaa8e70158a60c8dd40194c48713479ee
```

Three parser tests and two fixed-frame tests pass. The fixed-frame tests use
the independent `bitcoin_hashes` SHA-256 implementation and literal frame
bytes. Existing Series-7 and frame tests also pass, as does strict Clippy and
the release build. Logs are under `checks-2026-10-26-001/tilegrid470-*`.

This is physical configuration-range mapping. It does not decode LUT truth
tables, routing, instantiated reset logic or measured timing. Unmapped words
and unresolved aliases remain explicit. No firmware execution, exact-board
acceptance or milestone 2 completion follows from this component check.

## Public source identity progress

The public path-history API returned eight changes to `am1-s9/system.bit`.
The public branch list contains `bos-release` at
`9250b1e63102fcb824bf52c2d9ef914df5ef6dfc` and `master` at
`af957922a58bc6f8d609aa2c65266f1713e50593`. The full, untruncated tree for the
archived branch now identifies the S9 source Git link as
`e80262a72c92f1e5a5918b136ed83270d131437e`. This replaces the earlier abbreviated
`e80262a` search clue with an exact Git object identity for that archived
binary revision. The tree also records its bitstream blob as
`f193220aa3a41407bc3fa1d565fe61b965da3ff7`.

The source URL in the archived `.gitmodules` remains
`git@gitlab.ii.zone:x/braiins.git`. The exact source bytes were not obtained.
The archived 2019 bitstream was already shown to differ from the selected
2020 configuration in review422. The new Git link does not prove an association
with that selected configuration. The other historical binary contents have
not all been fetched and compared. No filename or date establishes a match.

Original API responses and acquisition hashes are recorded as
`braiins-s9-fpga-path-history-20261026`, `braiins-fpga-branches-20261026` and
`braiins-fpga-tree-9250b1e` in `artifacts/manifests/`.
Incremental archive verification checked four new objects, 4,399,623 bytes,
and reused 13,175 unchanged prior hashes. It passed. Catalogue coverage,
build and verification were regenerated successfully. The historical frozen
79,599-row execution plan is unchanged; new research records are not test
passes.

## Next reconstruction step

Acquire matching, pinned Project X-Ray feature-bit and connectivity data.
Use the checked tile ranges to decode features, including negated bit tests
and aliases. Keep unused or unknown bits visible. Then recover and validate
the instantiated reset/clock/FIFO logic against an independent oracle.
Exact board, PSU identity and physical traces remain separate requirements.

Primary format references: [Project X-Ray tile-grid format](https://f4pga.readthedocs.io/projects/prjxray/en/latest/dev_database/part_specific/tilegrid.html),
[maintainer grid reader](https://raw.githubusercontent.com/f4pga/prjxray/master/prjxray/grid.py),
and [published database hashes](https://github.com/f4pga/prjxray-db/blob/master/Info.md).
The word-range interpretation follows these sources. The executable input is
the archived, hash-pinned database, not a live URL.

# Selected S9 reconstruction checkpoint 475

Probe474c passes after the alias validation fix. The full unchanged-upstream
comparisons 476 and 476b also pass. No exact-board gate passes here.

The database revision is now pinned to
`0a0addedd73e7e4139d52a6d8db4258763e0f1f3`. Its original API response and
publisher hash index are archived. The index SHA-256 is
`3bd7c78cd80e32ded5ebc859c58eeec0df57e37eeb4240ffeb0c69ac821bb901`.
[Nine existing input checks](prjxray-publisher-index-check-475.json) all match
that exact index. This replaces a mutable-branch assumption with byte evidence.

[The next acquisition plan](fpga-remaining-feature-acquisition-475.json) selects
only 22 tables needed by unresolved tile and bus pairs in the selected image.
[The transfer record](fpga-remaining-feature-results-475.json) retains each
result. All expected hashes match, for 3,435,241 bytes. The originals and
response headers are archived. Artifact inspection, incremental archive
verification, coverage rebuild and catalogue build/verification pass. The
archive check read 24 new paths, including the head and index, for 3,740,859
bytes. Catalogue verification counted 13,609 artifact records with no errors.
The
[format audit](fpga-remaining-feature-format-475.json) found no unsupported
tokens or current parser-bound violations. Do not execute acquired code on
the host. These files are static data.

The separate expanded manifest is
`simulator/firmware/s9-fpga-feature-inputs-475.json`. It selects 28 tables. It
passed the isolated probe after native browser479 stopped. The run exited with
code 0 in 1,111 ms at the unchanged 512 MiB memory limit. Its 43,704,960-byte
report was not truncated. The six-table manifest and its prior results remain
unchanged.

```sh
work/simulator/check-target-085/release/examples/s9_feature_probe \
  simulator/firmware/s9-fpga-feature-inputs-475.json \
  work/simulator/qemu-s9/features-selected-475
```

Use a new output path for another run. The result contains 231,546 matching
predicates across 5,745 segments, including all eight real aliases. It reports
18,220 zero-only defaults. All 327,095 set non-ECC data bits have matching
positive predicates. No set data bit remains unexplained. This does not prove
complete primitive interpretation, connectivity or behavior. The
[unchanged upstream comparison476b](../qemu-s9/feature-oracle-476b/assessment.json)
passes all segment digests, result fields, unexplained-bit lists and counts.
It checks the actual BLOCK_RAM tables and all eight real I/O aliases. It exited
with code 0 in 6,700 ms without truncated output. Netlist reconstruction remains
pending.

`MONITOR_BOT_PELE1` has no table entry in this pinned index. Its three remaining
set-bit associations also belong to `HCLK_L_X70Y78`: frame/bit pairs are
`0x1100/1614`, `0x1102/1620`, and `0x1104/1622`. The expanded run resolves them
through HCLK predicates. The monitor segment remains explicitly unresolved.
A whole-word range is not a unique feature assignment. Zero set bits do not
prove that a primitive is unused.

## Existing converter candidate

The upstream [fasm2bels project](https://github.com/chipsalliance/f4pga-xc-fasm2bels)
converts FPGA assembly features into connected primitive Verilog. This is a
candidate for reuse after the selected features pass the independent oracle.
It needs a device connection database, tile and site definitions, package pins,
fixed connections and pseudo-PIP records. The current six tables are not enough.

Its README lists MMCM conversion as future work, but its
[clock model source](https://raw.githubusercontent.com/chipsalliance/f4pga-xc-fasm2bels/master/fasm2bels/models/cmt_models.py)
implements both PLL and MMCM conversion. Its
[connection importer](https://raw.githubusercontent.com/chipsalliance/f4pga-xc-fasm2bels/master/fasm2bels/database/create_channels.py)
handles Zynq processor-system tiles. The current
[dispatch table](https://raw.githubusercontent.com/chipsalliance/f4pga-xc-fasm2bels/master/fasm2bels/fasm2bels.py)
does not include DSP or monitor processing. Check selected resource use and
actual conversion behavior. Do not infer compatibility from the README.

These links are discovery references, not pinned execution inputs. Preserve a
full source revision and its dependency identities before any isolated run.
Reusing this converter would avoid a duplicate new netlist implementation.
It would not supply missing external clock constraints or measured timing.

# Next selected FPGA reconstruction contract

Status update: [probe474](feature-decoder-progress-474.md) now implements and
executes this feature decoder. The original data and upstream source files are
archived; their incremental archive and catalogue checks pass. The unchanged
upstream comparison remains pending. The text below records the design basis
before implementation.

The exact tile-grid mapping470 passes. The next data set has also been fetched
and hash-checked. It contains four CLB feature tables, two INT feature tables
and the XC7Z010 tile-connection table. CLB means configurable logic block; INT
is an interconnect tile. The [acquisition plan](fpga-feature-acquisition-471.json)
and [transfer results](fpga-feature-acquisition-results-471.json) retain each
URL, byte count and identity. The [format observations](fpga-feature-format-471.json)
record 10,038 feature rules and 99,248 wire pairs. Original archival and index
updates wait for the current browser capture hold to end.

This is acquired metadata, not decoded selected logic. It adds no execution
or exact-target pass. No Rust source or build was changed for this next step.

## Small decoder interface

Add one pinned feature-table reader beside the existing frame/tile modules.
Pass the exact configuration, part, tile grid and named table identities.
Reuse the checked frame map. Reject a wrong table type, duplicate feature,
contradictory condition, out-of-range bit or unsupported rule token.
Keep bus and feature type explicit. Do not infer them from a local filename.

Each rule is a conjunction. A plain `frame_bit` token requires one; a token
with `!` requires zero. Calculate the absolute frame as tile base plus the
relative frame. Calculate the absolute bit as 32 times the tile word offset
plus the relative bit. Retain the source rule and the exact observed positions
for every emitted feature. The output must distinguish decoded features,
unknown set bits and tiles/buses without a supplied table.

Twenty-two rules in these six files contain only zero conditions. Such a rule
can match unused space. A matched default is not evidence that a resource is
used or a FIFO/reset block exists. LUT initialization bits describe a truth
table, but they do not name its netlist inputs or function.

The eight current tile-grid aliases affect other tile types. Keep them marked
unresolved until their tables are supplied. The upstream alias reader remaps
type and site names, subtracts the alias start offset, and rejects a feature
if any of its relative words lies outside the alias range. Do not apply an
alias as a simple unsigned addition or silently decode the parent table over
neighboring words.

## Connectivity boundary

The connection table joins neighboring physical wires. Its entries do not
give signal direction. Upstream `connections.py` calculates the second tile
coordinate by adding the stored grid delta to the first tile coordinate,
then requires the second tile's exact type. This resolves the ambiguous
direction wording in the prose format page.

Programmable interconnect features use destination/source wire names. Complete
routing still needs the tile wire lists, fixed connections and pseudo-PIP
tables, including the distinction between `always`, `default` and `hint`.
It also needs primitive/site pin maps and the remaining active resource types.
Do not label the six feature files as a complete reconstructed FPGA.

## Required next checks

Use fixed independent configuration vectors for positive and negated bits,
word/bit order, conjunction failure and all-zero defaults. Verify feature
counts and bounds against the real six tables. Compare actual selected-image
decoding with the unmodified upstream Project X-Ray matching routine inside
the isolated tool environment. Preserve its source/dependency identities.
The selected firmware and bitstream remain unchanged.

Primary references: [feature format](https://f4pga.readthedocs.io/projects/prjxray/en/latest/dev_database/common/segbits.html),
[upstream feature matcher](https://raw.githubusercontent.com/f4pga/prjxray/master/prjxray/tile_segbits.py),
[alias matcher](https://raw.githubusercontent.com/f4pga/prjxray/master/prjxray/tile_segbits_alias.py),
and [physical connection iterator](https://raw.githubusercontent.com/f4pga/prjxray/master/prjxray/connections.py).
These live pages describe the method. They are not executable release inputs
until their exact source bytes and dependencies are acquired and pinned.

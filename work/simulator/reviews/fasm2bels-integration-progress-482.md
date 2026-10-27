# Existing netlist converter integration checkpoint 482

The selected FPGA predicate checks475 and the unchanged upstream comparison476b
pass. They account for all set data bits. Connectivity and primitive behavior
are still unproved. Reuse an existing converter for the next stage.

The original fasm2bels source is pinned to
`bafbcd8727e0807d6a620d1cf5dd151111c4a322`. The untruncated tree is
`898381e80e6a37199d62a02121eaeaf67f7c21b2`. Forty core source, licence and
metadata files total 460,241 bytes. Every file matches its exact Git blob ID
and size. Originals and response headers are preserved at the configured
external archive link, `artifacts/files/source/source-fasm2bels-bafbcd8/`.
The selected converter executed in isolation in497 with two separately
recorded source files that relocate optional imports. The pinned Project
X-Ray export dependency also executed in isolation. No downloaded source
ran on the host.

The [acquisition plan](fasm2bels-source-acquisition-482.json),
[transfer results](fasm2bels-source-results-482.json), and
[destination verification](fasm2bels-source-preserved-482.json) retain the
identities. Forty source records and two source-identity API records have
passed artifact inspection, incremental archive verification, coverage rebuild
and catalogue build/verification. The archive check read 507,015 new bytes.
Catalogue verification reports 13,651 artifact records and no errors. Logs
use the `fasm482-` prefix in `work/simulator/checks-2026-10-26-001/`.

The first metadata copy assumed the API headers used a `.json.headers` suffix.
Those two API responses use `.headers` instead. The metadata copy failed before
publication, then resumed with the actual files. It checked the already copied
body byte for byte. Source files and their 40 manifests were not rewritten.

## Actual dependency path

The source records Project X-Ray revision
`8146f1f2adc5aa2de8fe41886e18a73b57082086` and database revision
`8372b58fe62c67f8f5896d78cd1611d01621dfae` as submodules. All 28 selected feature
tables and the selected part, tile grid and connection table have identical
publisher hashes at that database revision. The current database adds four
I/O pseudo-PIP files and changes two origin-info files. Keep that distinction
in a converter input manifest; do not infer compatibility from path names.

[The Python-library acquisition plan](fasm-prjxray-library-acquisition-482.json)
selects 25 new files, with 137,270 bytes. It reuses seven archived oracle files
only after their Git blob IDs match the older tree. These files are now archived
unchanged, with hashes and HTTP response headers. The original source and
database API listings are archived too.

The Verilog conversion path calls the existing FASM parser, Project X-Ray
database/routing code, and progressbar package. Imports also load VPR and FPGA
interchange modules. Their packages are absent from the current Python
installation. Trace these imports before selecting dependencies. If a minimal
lazy-import change is needed, preserve the original source and record that tool
adaptation separately. Do not replace an upstream algorithm with a stub.

The selected tile grid contains 90 exact tile types. Their definitions all
appear in the pinned publisher index. The index also supplies processor-system
ports, site definitions and fixed-connection records. The exact database tree
bounded the selected acquisition by file size. No broad database mirror was
needed.

The [selected graph acquisition](fasm-selected-graph-acquisition-482.json) has
174 files and 36,529,394 bytes. Each file matches its Git blob, publisher SHA-256,
and expected size. The [reference audit](fasm-graph-reference-audit-482.json)
checks 90 tile types, 41 site types, 96,965 wire names, 35,208 programmable
connections, and 99,248 pairs in fixed connections. All references resolve.
This checks database closure. It does not reconstruct the selected FPGA logic.

The unchanged FASM parser is pinned to
`ffafe821bae68637fe46e36bcfd2a01b97cdf6f2`. Its 14 files and eight pure Python
dependency wheels are archived. The [final input record](fasm-parser-wheels-preserved-482.json)
lists the 33 parser, wheel, and release-metadata records. They passed artifact
inspection, incremental archive verification, catalogue build and verification,
and guide-link checks. The last archive check read 2,968,695 new bytes. The
preceding library/graph batch read 37,045,563 new bytes. Catalogue verification
reports 13,888 artifact records and no errors. Logs use the
`fasm482-final-` prefix in `work/simulator/checks-2026-10-26-001/`.

## Export execution and resolved failures

The unchanged disassembler produced a 4,719,842-byte FASM file with SHA-256
`3654907a9bb4de393c8d0a40fab86d2d04555461f2e2b80d1ce5a38c7b3283e4`.
The [482b assessment](../qemu-s9/fasm-export-482b/assessment.json) verifies all
positive Rust predicates. It records 221,975 raw features and 219,702 formatted
features. Formatting removed 2,273 zero-only defaults. No non-ECC set bit is
unknown. The output retains 11,859 unknown ECC-bit annotations. The missing
monitor table stays explicit in the assessment.

The first full round-trip attempts failed. The first run exceeded its 1 GiB address-space
limit while retaining output. The second run fixed that allocation but reached
the limit within the parser/assembler stage. A whole-file textX syntax tree is
the leading cause to test; the retained exception does not identify its exact
allocation. Both failures are saved.
Neither receives a round-trip pass.

A bounded invocation of the existing string-parser API is now implemented.
It rejects formatter items with embedded line breaks, parses complete lines
in bounded chunks, and passes each parsed item to the unchanged assembler.
It checks the exact consumed file hash and size. Its small isolated fixture
compares complete parsed tuples with a whole-file parse. The original and
regenerated non-ECC bit sets must still match the independent Rust bit-set
digest. The 1 GiB limit and all acceptance assertions remain in force.
The unchanged whole-parser fixture then exposed an escaped-quote defect in
the original grammar. A separately recorded one-byte grammar correction
passes the same fixture. Run482d now passes the complete selected export and
assembly round trip in29,469ms under the same limits. All327,095 original
non-ECC set bits match both the regenerated frames and the independent Rust
asserted-bit union. The FASM bytes are unchanged from482b. The
[execution report](fasm-export-executed-482d.md) and
[independent actual review](fasm-export-independent-execution-482d.json) retain
the evidence. All earlier failures and original tool inputs remain.

The [implementation checkpoint](fasm-export-checkpoint-482.json) records the
next step. The optional converter imports have a small, separately reviewed
lazy-import adaptation staged under `work/simulator/netlist-import-dev-482e/`.
The [selected conversion497](netlist-executed-497.md) now passes and retains
Verilog/XDC with complete input identities. Independent HDL validation,
physical timing and exact-board acceptance remain open.

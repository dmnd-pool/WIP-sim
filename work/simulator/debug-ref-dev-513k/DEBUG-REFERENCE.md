# Historical SLCR storage reference

Status: staged. No build or execution pass is claimed.

The original public ROM diagnostic513j now reaches a write to SLCR `0x500`.
The older UG585v1.0 address table calls this a CoreSight SoC debug control
register. It does not define its fields. This stage uses the exact prior
QEMU implementation as a storage reference. It does not infer silicon fields
or physical debug behavior.

The source basis is QEMU commit `6619bc5c55c1322a89009e2edd530113da95d551`,
file `hw/misc/zynq_slcr.c`, SHA-256
`9f907c7832926765f0d867f69c0e941a21ebec67f8484e8068c8f467ba18f7d8`.
The file stores all32 bits, applies the ordinary SLCR lock and retains this
word across its reset callback. Same-commit QOM allocation clears the instance
before initialization. Both files and their Git blob identities are archived.
See the parent, construction and reset reviews in the stage input manifest.

The new word exists only in the explicit `bootrom-reference` mode. It starts
at zero once at construction. Ordinary warm resets retain it and relock SLCR.
It uses the existing C register array. A second Rust implementation of one
word would duplicate that state and require another foreign-function boundary.
The surrounding existing QEMU C code remains the implementation.

The reference has an explicit full-word bus contract for SLCR. Byte, halfword,
doubleword and unaligned operations stop before state changes. Reference bus
operations permit these request forms to reach the rejection callback. This
is needed because QEMU otherwise splits some requests before a device callback
can see their original size. The default machine keeps its original operations
table and access policy. Both tables are selected at realization, after the
reference property is set and before the board maps the bus region.

The literal test plan checks zero at construction, all32 storage bits, correct
and incorrect lock keys, the existing low16-bit key rule, locked-write rejection,
and two warm-reset retention cases. The separate rejection plan checks nine
invalid accesses and reads back the unchanged sentinel after each. These
include a doubleword that starts before the word and overlaps it. Every
reference read/write has its own trace event. Locked writes carry
`committed=0`; the test does not need to enable unrelated guest-error logging.
The test must reject
missing, extra, reordered or truncated result records.

These tests do not establish debug signals, security permissions, eFuse state,
CPU halt, trace output, physical reset domains or exact S9 silicon identity.
Those remain unknown. No host debug port is exposed. The public ROM and original
firmware remain unchanged. This reference cannot close original-board,
physical-timing or exact-target mining gates.

Before integration, review the staged C patch and Rust test plan. Then verify
the exact patch parents and outputs in the isolated builder. Run the new
positive and rejection suites, all affected ROM/GPIO/clock regressions and the
default-machine regression. Only then run a fresh original-ROM diagnostic.
A successful backend build or a later translated code address is not a full
firmware boot.

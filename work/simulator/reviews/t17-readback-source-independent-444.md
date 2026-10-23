# Independent T17 readback source review444

No blocking source finding was found in the new explicit `--readback` path.
This is a source review. The original readback execution, isolated C build,
UBSan checks and unchanged default packing output still need their actual
runs. No miner, ASIC, original-board or mining pass is implied.

The wrapper copies the pinned collector and division bytes. The verifier
requires their exact hashes, unique occurrences and executable mappings at
their original virtual addresses. The fixed linker sections keep the
callback branch targets and required data addresses separate. The queue,
global and configuration accesses fit their declared mapped regions.

The eight controlled cases cover matching and mismatching queue metadata,
later matches, an empty queue and index wrap. The expected software FOUND
flag remains separate from the returned data. Every emitted word is checked.
The sentinel and callback checks cover unexpected writes and lock depth.
The delay callbacks record calls and do not claim physical timing.

The default packing fixture is unchanged. Its syscall ABI is extracted only
after checking the original fixture hash. The new mode has separate source,
linker, ABI, assessment and adaptation identities. It uses the existing
isolated compiler and ARM emulator path. Focused Rust validator tests are
useful negative checks; they do not execute vendor instructions.

Reviewed source hashes:

| File | SHA-256 |
| --- | --- |
| `crates/md-sim-firmware/examples/t17_core_oracle.rs` | `59425b324a0d245bda381d5d8e3734985aa79a99b903b75bd8afd132fbb3387d` |
| `crates/md-sim-firmware/examples/support/t17_readback_oracle.rs` | `f5564ca927cc9e4051a5b2e5e19ff71d69941568901633ad22ca421b99107d45` |
| `simulator/firmware/fixtures/bm1397-readback-oracle.c` | `f4424255106d8f0a3c67015a5e49654a38ea441cdf5a257f046119f1366b90dd` |
| `simulator/firmware/fixtures/bm1397-readback-oracle.ld` | `e30bf35a73fea6efe5971eec693401c7161fa98545478232053b76be730dda01` |

The next review must inspect the compiled ELF, actual callback/output bytes,
retained compiler diagnostics and exact original function copies. Keep any
mismatch as a failure. Run default packing separately and compare its full
output with the retained fixture. These executions are on hold during the
parent's native-browser445 run. The reviewer ran no build, test or emulator.

## Actual execution446 and default regression447

The later retained executions pass independent review. Run446 exited0 in
138ms with an untruncated trace. The readback output contains196 words and
matches all eight static442 collector predictions. Wrong-chain and
wrong-ASIC cases set the software FOUND byte while preserving the initial
data word. This is confirmed driver behavior in the declared controlled
queue fixture. It is not ASIC readiness evidence.

The exact collector and division bytes are in unique executable mappings at
the original virtual addresses. Their file slices also match the pinned
vendor input. The actual compiler arguments include strict warnings and
undefined-behavior traps. Compiler and execution stderr are empty. These
traps instrument the C wrapper; they do not instrument the copied vendor
instructions.

The default447 run exited0 in123ms with an untruncated trace. Its49,152-word
output and ARM executable are byte-identical to retained274. The output
SHA-256 is
`7527abdd4015bbafa0dc1121733a9ff20c37799e142e11b6d2c69f57b45745d1`.
No default packing regression was found.

For both runs, the exact retained tar equals the process stdout bytes.
Every extracted member was compared to that tar without extracting or
executing a file. Assessments, wrapper/output hashes and actual ELF mappings
were checked independently. Exact result/specification/assessment/archive/
executable/output identities are saved in
`t17-readback-execution-independent-446.json`.
No unresolved correctness finding remains in this oracle scope. The
reviewer ran no compiler, firmware or emulator.

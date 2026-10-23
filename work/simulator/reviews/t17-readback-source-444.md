# T17 collector oracle source checkpoint 444

The explicit `--readback` mode extends `t17_core_oracle`. It uses the same
isolated compiler and ARM QEMU path as the packing oracle. The C fixture
copies the exact collector and unsigned-division instructions from the pinned
T17 executable. A linker script retains their original virtual addresses.
Rust checks both the unchanged bytes and their executable load mappings.

The eight controlled queue cases cover a matching response, wrong register,
wrong chain, wrong ASIC, later matching entries, an empty queue and index wrap.
The software `FOUND` output remains separate from the returned word. The
expectations come from the disassembly and vectors saved in analysis442.
They are not ASIC status or timing claims. The C fixture contains inputs;
the Rust verifier checks every emitted field and bounded callback count.

The test replaces only mutex and delay entry points with owned callbacks.
Mutex callbacks check single-thread lock depth and arguments. Delay callbacks
record requests without sleeping. The divisor is explicitly4. The original
division helper's zero-divisor path is outside this test. No clock-write,
ASIC readiness, calibration, nonce, miner or original-board behavior is added.

The new wrapper reuses the old fixture's Linux syscall ABI as a derived,
captured header. The old fixture is hash-checked before extracting that text.
The ARM compiler uses the existing strict flags `-Wall -Wextra -Werror` and
`-fsanitize=undefined -fsanitize-trap=undefined`. This instruments the new C
wrapper; the original copied vendor instructions are not instrumented.
The actual isolated compiler arguments and tool versions will be retained.

Source files:

- `crates/md-sim-firmware/examples/t17_core_oracle.rs`
- `crates/md-sim-firmware/examples/support/t17_readback_oracle.rs`
- `simulator/firmware/fixtures/bm1397-readback-oracle.c`
- `simulator/firmware/fixtures/bm1397-readback-oracle.ld`

Focused Rust tests passed:5 total, including3 new validator tests.
The log is `work/simulator/checks-2026-10-21-001/t17-readback444-tests.log`.
These tests check hash rejection, executable mapping constraints, every
changed result field and output length. They do not execute vendor code.
Clippy, the release build, isolated C compilation and original-code execution
are pending. The parent requested no concurrent build or oracle during the
next browser integration run. No oracle or build remains active here.

The old retained fixtures have the same hashes before and after this edit:

| File | SHA-256 |
| --- | --- |
| `bm1397-core-oracle.c` | `c1c2a3fd7fd693797fe54eb87faaf4d7ad560710ea35f58fbc147fec12209b71` |
| `t17-core-packing-results.bin` | `7527abdd4015bbafa0dc1121733a9ff20c37799e142e11b6d2c69f57b45745d1` |
| `t17-core-packing-results.json` | `a7e6f0543139502d3128b90ee3c52599ca133ea44271dbb34a2ec095491e5921` |

Next: after the browser window, run focused Clippy and the release build.
Execute both explicit readback and default packing in new output directories.
Save any mismatch without weakening the expectations. Confirm that default
packing output remains identical to the retained49152-word fixture.
Review the new path independently, then reclaim only exact copied inputs
whose preserved canonical copies have been verified.

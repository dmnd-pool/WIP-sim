# Original T17 core0 formatter execution 472

The unchanged vendor formatter passed5,192 controlled cases. The new Rust
readback view matched5,144 emitted header calls and2,575 complete field sets.
The complete callback verifier checked77,574 actual calls. This closes the
field-extraction oracle for this exact routine. It does not implement ASIC
core-write effects or advance the strict original probe035 boundary.

The [assessment](../bitaxe/t17-core0-oracle-472/assessment.json) reports pass.
The isolated process exited0 in446ms with no trace truncation. Compiler and
execution stderr are empty. The new C wrapper compiled with strict warnings
and UBSan traps, then executed. The original vendor bytes are not instrumented.

## Exact execution identity

The553,247-byte original miner has SHA-256
`ad090e7abbf525a7a8a23c7153529e45af94b864e1170e677e1f85b4c4091367`.
Its1,024-byte formatter remains at original virtual address `0x238f0`, with
SHA-256 `8d3b1b4e80ef91084c23018fc6055b6bd5249b0e0f91119919f2fb20ef0f8245`.
Four original literal ranges also retain their bytes and original addresses.
The verifier checks unique copies and non-writable executable load mappings
before QEMU starts.

The30,436-byte adapted test ELF has SHA-256
`7ef1317cd44962abc9d6a35a58458d77b841ae4c58c08d8946f051b27cb888ad`.
Its2,069,472-byte output has SHA-256
`20fd344da2d2fc0fb4aeb95e252ccc41964d44bff286314dba9817dd0f02c8f9`.
The captured Rust coordinator has SHA-256
`159e2b7954559add2f881f292a2d53306ed5be3df1684905ea23b4324e9eb415`.
Clang23.1.1, LLD23.1.1 from the pinned Rust1.99 toolchain, and QEMU ARM11.1.2
are captured in the [process specification](../bitaxe/t17-core0-oracle-472/spec.json).
The process result also pins `unshare`, `prlimit` and Bubblewrap.

The [source snapshot](../bitaxe/t17-core0-driver-472/manifest.json) retains
the current source identities and changes from checkpoint468. All seven
changed oracle and model source files still match their frozen bytes after
the run. Unrelated workspace sources in that snapshot are source records;
this oracle does not claim that their features executed.

## Tests and limits

The5,120 main inputs come from the plan recorded before this implementation
in [analysis442](t17-readback-analysis-442.md). They cover every data byte,
five reported-core fields, two middle bytes, and both software status values.
Eight explicit vectors and64 extra cases cover debug thresholds, a second
log, independent open failures, and nonzero software status255.

The original formatter calls `fclose(NULL)` after an open failure. The
fixture records that call. It does not call a host stdio implementation or
repair the original routine. Caller slots, padding, surrounding guards and
configured globals remain unchanged.

The Rust `Core0Readback` view requires a valid pending core0 read, the expected
ASIC address, and a register0x40 response. Tests reject writes, other register
contexts, invalid selectors, wrong ASIC addresses and nonce replies. The
view preserves the complete raw word and reserved bit. It keeps the requested
selector separate from the full reported-core field. It cannot establish
physical core geometry, successful clock enable, completion or readiness.
The live `IdentificationChain` remains unchanged and rejects the core write.

The existing namespace isolation and limits apply: no network, read-only
captured inputs, private storage,90s wall,60 CPU seconds,16 processes,
64MiB files and4MiB trace. The6GiB virtual-address limit permits QEMU's ARM
address reservation. It is not an aggregate memory claim. Only this mode's
captured stdout limit increases from1MiB to3MiB for the exhaustive output;
the outer trace limit and old mode limits are unchanged.

The C fixture is needed for the existing ARM32 binary ABI and callbacks.
The installed Rust targets lack an ARM32 core library. The fixture reuses the
existing hash-checked syscall header. It does not rewrite the vendor routine.
The CLI, input capture, independent output checks and semantic view use Rust.

Nineteen BM1397 library tests and seven oracle-example tests passed. Strict
scoped Clippy, release build and Rust formatting passed. The first Clippy run
found a new API above the declared Rust1.85 minimum. The length check now uses
the compatible remainder operator. Its failed log is retained; strict Clippy
and all seven affected example tests passed after the fix.

No firmware, source archive, original evidence or canonical restore input was
changed or deleted. The evidence tar equals the process result's stdout bytes.
Every expected regular member was extracted with a strict name and size bound.
An initial analysis command treated the hexadecimal stdout field as a byte
array. The corrected reader used `bytes.fromhex`; no evidence was changed.

## Verified commands

Run these from the repository root. Choose a new output directory for another
execution. This command uses the frozen helper that produced the result above.

```bash
export CARGO_TARGET_DIR=work/simulator/check-target-085
export CARGO_INCREMENTAL=0
export CARGO_BUILD_JOBS=1
export TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227
cargo test -p md-sim-firmware --lib bm1397 --offline --locked
cargo test -p md-sim-firmware --example t17_core_oracle --offline --locked
cargo clippy -p md-sim-firmware --lib --example t17_core_oracle --offline --locked -- -D warnings
cargo build -p md-sim-firmware --example t17_core_oracle --release --offline --locked
work/simulator/bitaxe/t17-core0-driver-472/t17_core_oracle \
  work/simulator/bitaxe/t17-static-262/bmminer \
  /home/user/.rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld \
  work/simulator/bitaxe/t17-core0-oracle-472 --core0
```

The complete source, check-log and result identities are in the companion
`t17-core0-executed-472-inputs.json`. Independent source review found and
closed two wrong callback counts in a parser regression before execution.
The actual original-code output passed without changing its expected calls.
The [independent execution review](t17-core0-independent-472.md) checked
the original ELF bytes and all 5,192 callback records. It found no unresolved
correctness issue. Its 5,152 header calls and 15,474 field calls include the
secondary output destination; the model counts above use the primary one.
The [state-transition contract](bm1397-core0-contract-472.md) keeps the next
ASIC implementation boundary explicit. This is an adapted function-level
oracle. No board, mining, physical timing or full-matrix gate passes here.

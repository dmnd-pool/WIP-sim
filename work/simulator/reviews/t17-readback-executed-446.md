# T17 collector oracle execution 446

The controlled original-code collector test passed all eight cases. The
original T17 collector can set its software `FOUND` flag for the wrong chain
or ASIC while leaving the caller's previous data word unchanged. This confirms
the static prediction from [analysis442](t17-readback-analysis-442.md).
The flag alone is not valid ASIC readback or readiness evidence.

This is a function-level protocol oracle in an adapted ARM test executable.
It is not a full miner, an original-board boot or a physical ASIC model.
The original probe035 core-write boundary remains unresolved. No clock-write,
reset, fan-out, readiness, calibration or mining behavior was added.

## Exact execution and result

The extracted original miner is553,247 bytes with SHA-256
`ad090e7abbf525a7a8a23c7153529e45af94b864e1170e677e1f85b4c4091367`.
It comes from the retained T17 artifact pinned in analysis442. The collector
is916 unchanged bytes at original address`0x1c0c4`; its SHA-256 is
`a6fa7c53118e2e3ca140a56f086044085e57d750b7e6a98873c024409abd3445`.
The original168-byte unsigned-division helper stays at`0x75be8`, with SHA-256
`81c5ee0515ea5584984332c432a85952fa793bc7be049e82b188da86226d613f`.
Rust checks each unique byte copy and its exact executable, non-writable
load mapping before QEMU starts.

The new C fixture supplies controlled queue inputs and bounded mutex/delay
callbacks. It records actual output words and callback counts. Rust checks
all196 emitted words against expectations saved from the independent
disassembly. No expected output is supplied to the original collector.
The collector itself supplies the returned data and software flag.

| Case | Returned data | Driver `FOUND` | Remaining entries |
| --- | --- | ---: | ---: |
| Exact match | `01230074` | 1 | 0 |
| Wrong register | Initial `deadbeef` | 0 | 0 |
| Same register, wrong chain | Initial `deadbeef` | 1 | 0 |
| Same register, wrong ASIC | Initial `deadbeef` | 1 | 0 |
| Wrong register, then match | `01230074` | 1 | 0 |
| Wrong chain, then match | Initial `deadbeef` | 1 | 1 |
| Empty queue | Initial `deadbeef` | 0 | 0 |
| Read-index wrap | `01230074` | 1 | 0 |

The last wrong-chain case returns before the valid queued entry is consumed.
That is observed driver behavior. It must not be copied into an ASIC model
or repaired inside this original-code oracle.

The isolated process exited0 after138ms with no trace truncation. The
91,548-byte test ELF hash is
`ee5667dde6661fc776298d8a48211e00964d60ce225fec9dcdd3c9993a833ca4`.
The784-byte result hash is
`2db7274bed8e9a8a90ac28af2c86b52613a7341c74357a6514a9417869d6b4bd`.
The complete input closure is in [spec446](../bitaxe/t17-readback-oracle-446/spec.json).
The [assessment](../bitaxe/t17-readback-oracle-446/assessment.json),
[process result](../bitaxe/t17-readback-oracle-446/result.json) and
[evidence tar](../bitaxe/t17-readback-oracle-446/evidence.tar) are retained.

## Compiler and adaptation scope

The actual tools were Clang23.1.1, the captured Rust1.99 LLD and QEMU11.1.2.
The captured Rust coordinator executable SHA-256 is
`19b4be620c5a73de772a3d94beae3df7f498d6164974d44bdfe718a530ee38fe`.
The input manifest records its size and all compiler, linker, emulator and
library identities. The exact [compiler arguments](../bitaxe/t17-readback-oracle-446/compile-arguments.json)
include `-Wall -Wextra -Werror`, `-fsanitize=undefined` and
`-fsanitize-trap=undefined`. Compiler stderr and execution stderr are empty.

UBSan trap instrumentation checks the new C wrapper. The embedded original
vendor instructions remain unchanged and are not sanitized. The wrapper
checks queue guards, output guards, padding, mutex arguments/depth and delay
request bounds. Its callbacks are explicit adaptations: they do not emulate
physical locking or delay. The address divisor is4. The division helper's
zero-divisor path is outside this test. The debug level is0.

The C fixture supplies the existing ARM32 binary calling convention and Linux
syscalls. The installed Rust targets have no ARM32 core library. It reuses
the old fixture's hash-checked syscall text as a separately captured header;
rewriting the vendor routines would remove the independent oracle. The CLI,
capture, result verification and reports remain Rust.

The existing namespace confinement and limits apply: no network, read-only
captured inputs, private scratch storage,90s wall,60 CPU seconds,16 processes,
64MiB files and4MiB trace. The6GiB virtual-address limit permits QEMU's ARM
address reservation; it is not an aggregate memory measurement or claim.

## Checks and reproduction

Focused tests passed5/5, including three new validator tests. Each changed
output field is rejected. Hash, length and executable-mapping negatives pass.
Strict focused Clippy, the release build and focused Rust formatting passed.
Logs are listed in [the hash manifest](t17-readback-executed-446.sha256).
That manifest was checked with `sha256sum -c` after both executions.

The following commands were run from the repository root. Use new output
directories when repeating the two execution commands.

```bash
export CARGO_INCREMENTAL=0
export CARGO_TARGET_DIR=work/simulator/check-target-085
export TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227
cargo test -p md-sim-firmware --example t17_core_oracle --offline --locked -j 1
cargo clippy -p md-sim-firmware --example t17_core_oracle --offline --locked -j 1 -- -D warnings
cargo build -p md-sim-firmware --example t17_core_oracle --release --offline --locked -j 1
work/simulator/check-target-085/release/examples/t17_core_oracle \
  work/simulator/bitaxe/t17-static-262/bmminer \
  /home/user/.rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld \
  work/simulator/bitaxe/t17-readback-oracle-446 --readback
work/simulator/check-target-085/release/examples/t17_core_oracle \
  work/simulator/bitaxe/t17-static-262/bmminer \
  /home/user/.rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld \
  work/simulator/bitaxe/t17-core-oracle-447
```

The default packing regression447 passed49,152 words. Its output and generated
ARM executable are byte-identical to retained274. The old C fixture and
retained packing binary/JSON fixtures are unchanged. See
[assessment447](../bitaxe/t17-core-oracle-447/assessment.json).
The source and executable reviews are closed with no unresolved finding in
this scope. See [independent review444](t17-readback-source-independent-444.md)
and its [execution evidence pins](t17-readback-execution-independent-446.json).
The reviewer checked exact tar/result equality, all extracted members,
original-code mappings, all196 output words, strict compiler flags and the
byte-identical default packing regression. These are oracle results only.

No extra background process remains. Duplicate captured inputs are eligible
for the separate [locked cleanup plan](t17-oracle-cleanup-plan-444.md) after
the narrow utility extension is reviewed. All original evidence and unique
inputs must remain. This result does not reduce the full target matrix.

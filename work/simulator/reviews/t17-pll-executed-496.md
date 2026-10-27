# Original T17 PLL arithmetic execution 496

The unchanged T17 routine passed all 22,134 input cases in isolated QEMU ARM.
The process exited 0 in 158 ms. Its output is complete. Compiler, emulator and
process error streams are empty. The C wrapper used strict warnings and
undefined-behavior traps. The original vendor instructions are not instrumented.

The 108 original bytes remain at virtual address `0x28b2c`. Their SHA-256 is
`650ae97e0c4c042e4e89b91d2bdf995ce29433b0e3f7f8bd47cacd986155f4d7`.
They came from the unchanged T17 miner with SHA-256
`ad090e7abbf525a7a8a23c7153529e45af94b864e1170e677e1f85b4c4091367`.
The checker verifies the exact copy and its executable, non-writable load
mapping before execution. The original firmware archive remains unchanged.

The function extracts the feedback, reference and two post-divider fields.
It uses the low byte of the caller's software-divider argument. It calculates
`25 / reference * feedback / (post1 * post2 * software_divider)` with three
single-precision operations, then truncates to an integer. These post-divider
fields are direct divisors. The separate output-divider register uses its own
encoding; that register is not an input to this oracle.

The cases cover every nonzero feedback field, reference divider and software
divider, all nonzero post-divider values, and ignored status or upper argument
bits. Rust checks each exact original result and a separate rational error
bound. Four cases show why exact rational arithmetic alone is insufficient:
the original results are 124, 24, 24 and 4 where the exact rational values are
125, 25, 25 and 5. All input tuples and results are retained in
[execution.stdout](../bitaxe/t17-pll-oracle-496/execution.stdout).
The [input manifest](t17-pll-executed-496-inputs.json) lists those four cases.

The 9,024-byte test ELF has SHA-256
`e96f38a6df508fbbb612343ac2de61fe6cc4fea8862763d354ed6ff10beaf072`.
Its 354,160-byte output has SHA-256
`9fd7c05d9f83cdab32094688708aac380fd9a5997a2493be0a2515e6af17b13c`.
The frozen Rust helper has SHA-256
`710e0f9ff4fe75fbe004e85c52e4abf1cedb52fe42ac0ac46afc97fb4a97be09`.
Its [source receipt](../bitaxe/t17-pll-driver-496/manifest.json) retains 137
source inputs. Each matched before and after the build and execution.
All 47 captured mounts match the [process specification](../bitaxe/t17-pll-oracle-496/spec.json).

Nine focused Rust tests, strict Clippy, formatting and the release build pass.
The parser tests reject changed headers, input identities, results and lengths.
Invalid zero divisors are rejected before oracle comparison. No invalid-divisor
behavior or valid physical voltage-controlled oscillator range is claimed.
The unchanged isolation limits are 90 seconds wall time, 60 CPU seconds,
16 processes, 64 MiB files, 32 MiB scratch storage and 4 MiB trace.
The 6 GiB virtual-address bound permits QEMU's ARM address reservation.
These are per-process and per-mount limits, not aggregate cgroup claims.

These commands were tested from the repository root:

```bash
CARGO_TARGET_DIR=work/simulator/check-target-085 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 \
  cargo test -p md-sim-firmware --example t17_core_oracle --offline --locked
CARGO_TARGET_DIR=work/simulator/check-target-085 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 \
  cargo clippy -p md-sim-firmware --lib --example t17_core_oracle --offline --locked -- -D warnings
CARGO_TARGET_DIR=work/simulator/check-target-085 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=1 \
  cargo build -p md-sim-firmware --example t17_core_oracle --release --offline --locked
work/simulator/bitaxe/t17-pll-driver-496/t17_core_oracle \
  work/simulator/bitaxe/t17-static-262/bmminer \
  /home/user/.rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld \
  work/simulator/bitaxe/t17-pll-oracle-496 --pll-frequency
```

Choose a new output directory for another execution. The helper pins the linker
and its LLVM library. Captured Clang and QEMU identities are in the run spec.
The existing C language exception applies: the fixture supplies the ARM32 ABI
because the installed Rust targets lack that core library. The runtime,
input capture and result checks remain Rust.

The [assessment](../bitaxe/t17-pll-oracle-496/assessment.json) is an arithmetic
oracle pass. It does not create PLL lock, readiness, output routing, physical
timing, ASIC register state or mining results. The strict device model still
rejects the PLL3 command seen in original factory probe036. The
[independent execution review](t17-pll-independent-execution-496.json)
rechecked every source, mount, original byte mapping and input/result tuple.
It found no unresolved issue.

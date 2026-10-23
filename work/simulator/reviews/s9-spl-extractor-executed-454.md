# Exact S9 SPL contract extraction 454

The new Rust inspector passed an actual isolated run on the selected archived
S9 boot image and its exact secondary program loader (SPL) suffix. It decoded
all 15 initialization tables, in the call order for all three revision branches.
This is a static inspection result. The inspector does not execute ARM
instructions, change a model, select a physical silicon revision, or establish
a boot, mining, FPGA equivalence or physical calibration pass.

## Inputs and result

| Item | SHA-256 | Bytes |
| --- | --- | ---: |
| Original selected `boot.bin` | `950d19c6e5bfd9d8107def130b19fc9a5f64d824b26c0c493d5fdfe8503edac4` | 79,263 |
| Exact SPL suffix | `765efb23844156e976ba785c24a6a753c664504a499c64581d1c356a3f4c02df` | 77,023 |
| Rust example source | `66cbd15df5e1413a16b328455650c86a0b539ef89dca5e82a16bc1f636291285` | 4,442 |
| Rust contract module | `c409be69bf3a0c568f2f437aefe27ba8406d6f5068a18d757696a8b37dc89a03` | 17,531 |
| Release inspector | `ab5d274f85e9bcab47ac5da69978df87e5a9c837c617ea8b86feb113c5d11506` | 1,793,176 |

The [process specification](../original-zynq/spl-contract-454/spec.json) pins
the executable, its library closure and both firmware inputs. The
[actual process result](../original-zynq/spl-contract-454/result.json) reports
exit code 0, 31 ms elapsed, no trace truncation and empty stderr. The process
uses the existing isolated preparation runtime. It has no network interface
or dashboard. Limits are 15 s wall time, 10 s CPU time, 256 MiB address space,
16 processes, 2 MiB per output file, 1 MiB trace and 1 MiB scratch space.
These are the runtime's recorded process limits; no aggregate cgroup claim
is added.

The [contract JSON](../original-zynq/spl-contract-454/contract.json) is the
exact decoded process stdout. It retains source and executable identities,
parent offsets, code range hashes, all raw record arguments and their decoded
operations. The SPL equals the parent suffix at offset 2,240 byte for byte.
The header checksum and entry fields are checked. The unresolved difference
between the header's declared length and the available suffix stays explicit.
This run does not establish BootROM copy behavior.

The [independent comparison](../original-zynq/spl-contract-454/independent-comparison.json)
matches every table's encoded hash, size, record count, poll arguments and
delay arguments against the separate
[original-byte oracle](spl-table-groups-independent-454.json). The counts
include the terminating stop record:

| Revision field | Table record counts in call order |
| --- | --- |
| 0 | 73, 26, 14, 82, 22 |
| 1 | 73, 26, 14, 84, 22 |
| Neither 0 nor 1 | 73, 26, 14, 83, 22 |

The source-derived poll condition accepts any selected bit. Its budget is
100,000,001 reads, with return value 2 on exhaustion. The delay calculation
preserves the ARM multiplication's low 32 bits before the full 64-bit product
and right shift. Argument 1 gives 333,333 timer ticks. A read budget and a
requested tick count are not measured hardware latency. Timeout execution
and physical timer calibration remain unverified. The first and fourth table
groups retain neutral names until their register roles are independently
established. See the [timing analysis](firmware-timing-recovery-450.md) and
[independent instruction review](firmware-timing-independent-450.json).

## Tested commands

Run from the repository root. The output directory must be new. These exact
commands used the retained paths below; choose a new output path for another
inspection.

```sh
MD_SIM_SPL_BUNDLE=/home/user/src/md-sim/work/simulator/original-zynq/sd-bundle-291/boot CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=work/simulator/check-target-085 TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 cargo test -p md-sim-firmware --example inspect_s9_spl --offline --locked -j 1 -- --include-ignored
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=work/simulator/check-target-085 TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 cargo clippy -p md-sim-firmware --example inspect_s9_spl --offline --locked -j 1 -- -D warnings
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=work/simulator/check-target-085 TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 cargo build -p md-sim-firmware --example inspect_s9_spl --release --offline --locked -j 1
TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 work/simulator/check-target-085/release/examples/inspect_s9_spl work/simulator/original-zynq/sd-bundle-291/boot/boot.bin work/simulator/original-zynq/sd-bundle-291/boot/spl-payload.bin work/simulator/original-zynq/spl-contract-454
```

The [final focused tests](../checks-2026-10-21-001/spl-contract454-tests-final.log)
pass all eight cases. They check the independent clock words, any-bit polling,
wrapping arithmetic, unknown operations, argument counts, every truncated
prefix, missing termination, record bounds, changed sources, and the exact
retained inputs. The test count includes two existing confinement tests.
[Formatting](../checks-2026-10-21-001/spl-contract454-fmt.log),
[strict Clippy](../checks-2026-10-21-001/spl-contract454-clippy-fixed.log),
[release build](../checks-2026-10-21-001/spl-contract454-build.log) and
[isolated execution](../checks-2026-10-21-001/spl-contract454-execution.log)
all passed.

Two initial failures remain recorded. The
[first retained-input invocation](../checks-2026-10-21-001/spl-contract454-retained-tests.log)
used a relative environment path, which Cargo resolved from the crate
directory. The absolute path above passed. The
[first Clippy invocation](../checks-2026-10-21-001/spl-contract454-clippy.log)
found that `is_multiple_of` needs a Rust version above this project's minimum.
The equivalent alignment check now uses `start & 3 == 0`. All affected tests
and strict Clippy passed after that fix. No original evidence was changed.

The [independent final review](spl-inspector-independent-454.md) found no open
correctness or scope issue. It checked all 654 emitted records against the
original words, including offsets, headers, arguments and operations. It also
verified the actual stdout, all six captured objects and the final source
and executable identities. Its [exact review record](spl-inspector-independent-454.json)
retains these checks. The README's
[guide check](../checks-2026-10-21-001/spl-contract454-guide-links.log) passes
all 546 checked links after the new command was added.

## Next action and limits

Use these pinned records to check the existing clock and reset models against
the selected firmware's requested ordering. An independently controlled
timeout execution test would be a separate next step. Do not infer a physical
lock delay from the software loop or return success merely because the guest
polls. Exact FPGA logic, board and PSU identity, BootROM handoff, physical
timing evidence and the complete target matrix remain open.

The adjacent [hash manifest](s9-spl-extractor-executed-454.sha256) pins the
sources, binary, inputs, outputs and check logs for this component result.

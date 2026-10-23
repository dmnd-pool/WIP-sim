# Selected S9 SPL extractor source checkpoint 454

The new Rust example is `inspect_s9_spl`. Its support module is
`support/s9_spl_contract.rs`. No existing runtime, emulator model, README or
frozen450 file changed. Source formatting is complete. Builds, tests and
execution remain on hold until the parent coordinates the next check window.

The proposed invocation is:

```text
inspect_s9_spl BOOT_BIN SPL_PAYLOAD NEW_OUTPUT
```

The example captures both exact files and its own executable closure in the
existing bounded preparation namespace. It emits a JSON contract and retains
the actual ProcessSpec, process result and stderr. It does not execute ARM
instructions. Parent and suffix hashes, their 2,240-byte relationship, header
checksum, selected instruction ranges and source/build identities are explicit.

The parser follows all three byte-confirmed revision-table groups. It records
each pointer, source/parent offset, raw header/arguments and decoded operation.
Argument counts, unknown operations, input bounds, record counts and table
terminators are checked. Polls retain the any-bit test and the 100,000,001-read
budget. Delays preserve the original low32 multiply before the 64-bit product
and right shift. The first and fourth register groups have neutral labels
until their MIO/DDR role names receive an independent register-level check.

Five unit tests cover the independently pinned 204-byte clock table, any-bit
polling, exact wrapping arithmetic, malformed/truncated/unterminated tables,
and changed source/instruction identities. An ignored, read-only test uses
`MD_SIM_SPL_BUNDLE` to verify the exact retained pair and three known table
record counts. No test or inspector result is claimed yet.

Source-ready hashes:

- `inspect_s9_spl.rs`:
  `66cbd15df5e1413a16b328455650c86a0b539ef89dca5e82a16bc1f636291285`.
- `support/s9_spl_contract.rs`:
  `4e64f091727dc0e19f89a667cb42195d0db3289114c54842c8ba72dd349cb0bf`.

The independent source review found no correctness blocker. The requested
spacing cleanup in messages and comments is included in the hash above.

The underlying byte facts are in `firmware-timing-recovery-450.md` and the
independent range review `firmware-timing-independent-450.json`. Physical
revision, PLL lock latency, FPGA equivalence, BootROM handoff and calibration
remain unproven. The existing276/282/295 clock models are reused, not replaced.

Next: after the parent releases the build hold, run focused Rust tests, strict
Clippy and the release build. Run the ignored retained-input test and the
isolated inspector in a new454 directory. Retain any failure. Compare all
decoded tables with independent raw-word inspection. Request final independent
source/result review before reporting this extractor as verified.

Execution follow-up: the [454 execution report](s9-spl-extractor-executed-454.md)
records the actual isolated pass, final source hashes, eight passing tests and
strict Clippy. The module hash above is the earlier source checkpoint. The
final module replaces `is_multiple_of` with the equivalent bit alignment
check to support the project's minimum Rust version. Both initial check
failures remain retained. All 15 tables match the independent byte oracle.

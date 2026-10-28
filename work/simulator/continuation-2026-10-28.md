# Simulator continuation:28 October2026

Status: implementation resumed at the user's request. The full specification
and milestone2 remain incomplete. Commit`f5fbf74e` preserves checkpoint500.
Keep its frozen binaries, manifests and successful results unchanged.

The working tree was clean at resume. Root storage had13GiB free and `/tmp`
had12GiB free. Cleanup must preserve original archive files, unique evidence,
locked inputs, frozen tools and unrelated user files.

## Active work

- BM1397 controls501: root verified the three production base identities,
  seven staged files, recorded patch and original496 arithmetic output.
  The reviewed patch is now integrated. All seven resulting hashes match.
  Its bank records posted configuration, with no invented acknowledgement,
  lock, physical timing, guest readback or mining success. Focused tests are
  running in `checks-2026-10-28-001/bm1397-501-lib.log` (session58041).
  Next: affected example checks, strict Clippy, release/source freeze, then
  unchanged factory firmware probe037 and independent trace review.
- Exact FPGA/HDL502: the existing exact-target agent resumes the isolated
  gperf/Icarus preparation and real validation of netlist497. Reuse the prior
  complete source/tree audits. The failed gperf partial remains preserved.
  No placeholder PS7 behavior, guessed electrical settings or relaxed sink/
  clock checks may grant a pass.
- Storage: the cleanup agent checks task-owned duplicate objects and temporary
  downloads with the existing guarded cleanup path. A concrete hash-verified
  plan and review are required before deletion. No compiler runs beside the
  root501 Cargo build.
-501 checks now pass:25 focused library tests, five example tests, scoped
  strict Clippy, formatting and release. Helper/source capture is
  `bm1397-probe-tools-501/inputs.json` (132 sources). Original probe037 exits1
  after2,879ms without truncation. The recorded controls receive no reply;
  the next actual unsupported write is`0x28=0x0600000f`. Independent review
  verifies all mounts and command CRCs. See
  `reviews/bm1397-programmed-executed-501.md` and
  `reviews/bm1397-programmed-independent-execution-037.json`.
- Root independently verified cleanup503:1,353 duplicate files totaling
 14,807,692,817 bytes,77 retained restoration files,21 finished build stores,
  no original archive or exact demo-input path. Apply/post-check is assigned
  to the cleanup agent. Failed build verdicts and unique evidence stay intact.
-504 UART extension passes29 library and five probe tests, scoped Clippy,
  format and release. Original038 exits1 after2,913ms without truncation.
  FastUART/Misc words are recorded silently; the next actual unsupported
  write is`0x70=0x0f0f0f00`. Root independently rechecks133 sources,17 mounts,
  12 command CRCs and the sole identification reply. Report:
  `reviews/bm1397-uart-executed-504.md`.
- Cleanup503 completed exactly as planned. Root independently verifies all
  1,353 removed paths absent and all429 retained files plus77 restoration
  objects unchanged. Free root storage rose to about27GiB. Restore records
  and the four failed build verdicts remain intact.
- HDL502d passes the full upstream gperf tests after serializing its check
  target. Icarus configure then exposes the separate build-compiler variables.
  Failed502b/c/d/e results remain.502e's first edit did not actually populate
  those variables; its config log confirms the same missing-sysroot failure.
  The next502f driver adds explicit fields and an environment regression
  before its new isolated attempt. No test, source archive or limit is relaxed.
- Divider505 is source-staged with exact504 bases. It stores raw`0x70` and
  maps only the proven low-byte encoding to the original software divisor.
  Encoded255 is an explicit unrepresentable error. Root review/build/039
  follow after the measured HDL tool run. No PLL3 or physical range inference.
- Independent review: the browser agent reviews501 and the next original
  Fast UART/baud command evidence. Source work stays separate until reviewed.

## Current verified state

-505 is integrated and tested. All31 focused library tests and five probe
  tests pass, as do scoped strict Clippy, formatting and release. Actual
  original039 reaches the unsupported hash-rate read`0x04` after the firmware
  frequency ramp and factory self-test start. No complete board, mining or
  dashboard gate passes. See`reviews/bm1397-divider-executed-505.md`.
-502f builds the original Icarus12 tool and passes upstream gperf checks and
  export/restore execution. Independent review passes. Selected HDL502g fails
  on runtime-library closure;502h fixes the closure and diagnostic checks but
  fails a LUT startup assertion. Both failures remain.502i corrects the
  fixture stimulus to make its first input transition explicit. Vendor code
  and all582 assertions stay unchanged. It is not a physical timing test.
-506 reproduces the S3 UART register errors against unchanged backend017.
  The old address`0x78` returns an ESP32 version constant instead of S3 clock
  configuration. Its divider mask is also wrong. A staged S3-only patch has
  documented register defaults and bounded write observations. Effective
  baud timing, clock gating and reset-pin effects remain separate work.
- The second cleanup plan passes the root's full byte/path/terminal review:
  151 Core duplicate files,56 restore files,2,013,780,471 bytes. The cleanup
  agent applies the frozen481 helper and checks retained evidence. The staged
  HDL cleanup source also passes root review; it still needs tests and a
  separate actual-plan review before deletion.
- Root free space is about6.8GiB before this second cleanup. The cleanup
  agent measures only about109MB of new task data while overall root free
  space falls by more than2GB. Do not delete unrelated data to compensate.

## Later tests and fixes

- Core cleanup503 is complete. Root independently checks the completed
  journal,151 absent copies and234 retained files. HDL cleanup503 is also
  complete:333 copies,1,322,525,463 bytes. Root independently rechecks their
  absence and160 unique retained/evidence files. No original archive, unique
  helper or failed result was removed. The larger later rise to67GiB free
  space exceeds this cleanup and must not be attributed to it.
- HDL502j passes510 real RAM observations for all15 selected parameter sets.
  Omitted-input and explicit-Z traces match exactly. All582 primitive vectors
  pass. Strict unchanged497 compilation still fails. The separately derived
  converter502k declares1453 existing aliases and makes60 already floating
  inputs explicitZ. Its reviewer also requires unchanged module framing and
  exact original XDC bytes. New conversion execution remains pending.
- UART018 first failed before execution because the historical017 overlay
  path had been reclaimed. The exact128,003,553-byte overlay is restored from
  retained008 bytes. The restoration receipt records locks and provenance;
  no historical direct-path cleanup journal was found or invented.
- UART018b builds successfully in195,015ms with all required UBSan trap flags
  checked in the actual S3 compile command. Its actual register test fails
  three DATE reads. The inherited region ends before`0x7c`, so the new DATE
  callback is unreachable. Reviewed506c extends only the S3 region to128bytes.
  Root's new018c build is session2711, with the same limits and overlay.
- The actual018b reset test also fails the fixture's clock advance. Exact
  upstream qtest/timer code stops when no timer is pending after reset.
  The staged correction schedules the existing real I2C NACK transaction,
  keeps the100001 UART timestamps, and checks the actual NACK at200000. It
  must first reproduce only the DATE failure on018b, then pass on018c.
- Root507 adds a strict programmed-UART observation mode. Independent review,
  eight tests, scoped Clippy, formatting and release pass. The frozen helper
  is`bm1397-probe-tools-507/esp32_original_probe`, SHA-256
  `7bd0564d1b853676ce420d7896b5f690a8618d98149069178fa83f4f783ab72f`.
  All134 sources match before/after compilation. Actual original040 must wait
  for018c's register/reset tests. Overflow or truncation fails capture; no
  effective baud or board/mining pass is inferred.
- ADC509 pins ten exact manufacturer source files at IDF5.5.3 commit2c211b2.
  Incremental archive, inspection, coverage and catalogue checks pass.
  Original039's calibration failure follows blank eFuse data and rejected
  calibration version0. Factory trims and analog initial-code behavior remain
  unresolved. No version bit, trim or successful reading was invented. See
  `reviews/esp32-adc-calibration-contract-509.json`.
- Next508 will stage an additive engine computation receipt and constrained
  nonce search, then a bounded one-chip reference BM1397 work/measurement
  path. Only actual computed hashes may advance its ledger. Physical counter
  semantics, multi-chip forwarding and core maps remain open. The strict
  default must still reject unsupported behavior.

## Current execution checkpoint

-018c and both corrected506d UART suites pass. Independent review verifies
  the sanitizer build,46 basic responses,332 reset/timer responses, two real
  resets and the trace cap. The only changed build input is the reviewed S3
  mapping patch. See`reviews/esp32-uart-executed-506c.md` for tested commands.
- Original040 executes unchanged factory firmware and exits1 at the same
  unsupported ASIC read`0x04` after12,534ms. Its complete26-write UART trace
  and253 ASIC commands pass independent audit. No job, nonce or mining pass.
- HDL502k conversion and502l strict compilation pass their component gates.
  All11,122 cells and original constraints stay intact. The211,790,335-byte
  VVP program is compiled, not executed. Independent reviews retain physical,
  PS7/AXI, monitor-table and33 electrical ambiguities as open work.
-511 reuses isolated listing/extraction. Exact Bosminer extraction511b passes
  at192ms with no firmware execution. ELF SHA-256 is
  `4997a1c10a09f6881d6f5e0d4967eb31072ea9f1da7a80dac192ae1299d45960`,
  12,699,280bytes. The next actual logic test must use FCLKCLK1 and exact
  selected MAXIGP0/register evidence, then connect an existing CPU emulator.
-510 is implemented and tested. Its pure requested-clock decoder uses
  explicit caller-supplied source frequencies. A shared bounded regular-file
  reader fixes consumed-byte identity races and rejects FIFOs/symlinks.
  Three library, five result-tool and13 affected netlist helper tests pass,
  with Clippy/format/release. The frozen138-source helper processes actual040.
  Independent audit confirms16 conditional word pairs and10 unpaired writes.
  With declared80MHz APB/40MHz XTAL, final UART1 ratio is1280000000/409 bits/s.
  Reset continuity, effective rate and physical calibration remain false.
-508 is now integrated after independent review. The engine adds actual
  computation receipts and live-filter changes without changing its original
  nonce enumeration or work schema. Masked search is deferred because the
  exact one-chip consumer's int interval256 accepts the whole nonce range.
  The reference supports only proven aligned job IDs0..124 and32 live slots.
  The immutable drop policy counts actual post-computation events; native
  semantics remain unsupported. Focused mining/BM1397/probe tests pass.
  The browser agent finishes its release/source freeze and then runs original041
  at unchanged limits. Hold shared library/probe edits until that freeze ends.
- Full-workspace Rust checks must run after these current source integrations
  settle. Full specification and milestone2 are still incomplete. No exact
  physical target has passed the required full matrix.

The user authorized continued implementation and agents in the original task.
The latest resume instruction ends the checkpoint pause. Do not stop at the
next component pass or count the adapted S9 result as an exact-target pass.

## Workspace check after508

Formatting, strict Clippy and all workspace/all-target tests pass. The run
contains 1140 passed tests and 49 explicit ignored tests in 122 suites.
See `reviews/workspace-check-after508.json`. Production was held during the
checks, but no before/after source snapshot was captured. This is not a
release input freeze. Later source changes need affected checks.

Original041 sends24 real jobs. The reference records at least455,680 real
computations. The factory test fails on the declared open temperature sensor.
The60-second run times out. Native ASIC filter2^36 is preserved. No nonce,
share or fresh-work mining gate passes. Timeout loses the private QEMU log
prefix; reviewed508b will retain complete lines incrementally and preserve
the primary timeout failure. Next042 uses unchanged budgets.

##042 and ADC follow-up

Reviewed508b retains QEMU stderr incrementally. Original042 times out at
60,016ms with unchanged limits. The6,059,408-byte stderr prefix contains all
26 UART writes and24 first-computation receipts with real digests and distinct
admission/completion timestamps. No final receipt survives outer termination;
retention and UART completeness remain false. The thermal test is unchanged.
No solution, share or fresh mining pass. The next independent verifier uses
bitcoin_hashes, separate from the engine SHA implementation.

Root stages an S3 eFuse calibration decoder and original-C arithmetic oracle
under `adc-contract-509/`. No production ADC model is altered yet. Review found
two real bugs before integration: ADC2 attenuation2 begins at bit248 rather
than249; IDF unsigned64-bit polynomial multiplication wraps before division.
Both are fixed, with boundary regressions. The staged decoder never creates
factory trims. Controlled reference images cannot establish physical accuracy.
The isolated C oracle must compare all sample outputs before this arithmetic
is accepted or connected to an explicit reference ADC transfer policy.

## User priority correction: complete S9 milestone2 first

The user asked why Bitaxe work continued and explicitly directed complete S9
first. Treat this as active priority. Other miner development is checkpointed;
finish only already running review/checkpoint closure. Do not continue508c or
ADC feature work before the complete exact S9 path. Root acknowledged that
parallel Bitaxe work diluted the requested order. Agents were redirected.

ADC509 isolated C run passes264 images,2112 points and131072 raw conversions.
Its570-source frozen helpers and original-C archive are retained. This is a
component arithmetic pass only. No physical ADC or board gate. Independent
execution audit is checkpoint closure only; adding a regression fixture is
deferred until after S9.508c remains staged pending final review, unintegrated.

S9 next: final independent source review of511d,72 selected MAXIGP0 bus
transactions through actual recovered HDL, then existing CPU emulator bridge,
original archived S9 firmware boot, hardware/mining/Core/fault/persistence
suites and the same exact selected run's native and remote workflows. Keep
all milestone2 acceptance requirements. No component pass completes it.

The user then set the stop boundary: when S9 milestone2 is complete, stop and
save the tested checkpoint. Do not proceed to other miners or milestone3.
Do not call the full simulator specification or matrix complete.

## S9 selected logic511d/e

Both source reviews close511d. Root independently checked all42 PS7 ABI widths
and directions against the unchanged vendor source. Review fixed truncated
response-width risks and an ineffective excluded-byte test. The final fixture
uses72 real transactions, including60 before a second reset.

Original511d exits failed after25,891ms. The actual VVP run executes all72 bus
operations and both resets, but its exact original PS7 startup notice was not
classified. Preserve that failed run.511e checks the exact literal in pinned
PS7.v and requires its expanded notice once at stdout line0. Every other
notice/diagnostic remains rejected. Compiler policy and generated HDL remain
unchanged. The report explicitly keeps CPU model absent and board gate false.

Actual511e exits0 after26,663ms, untruncated. All72 transactions, both resets,
582 primitive checks and both510-case RAM comparisons pass. All3 cores return
version00901002 and build5fca47e9; control and baud reset values are0.
The trace hash isbfd77d63ca7494012be93708b853e462e8aa26a97c266620e566799ea0c44061,
identical to the retained511d trace. Independent execution audit is pending.
This is actual recovered digital logic, not CPU execution or original-board
boot. Root/source reviews and failed-parent record are in `reviews/`.

Next: browser agent stages a live Rust/Icarus inherited-pipe interface using
upstream fscanf/fgets support. Exact upstream remote-port sources are pinned
as reference data. Existing remote-port has loose-time and unbounded-packet
limits that cannot be inherited silently. Need explicit virtual-time/FCLK1/
reset0/IRQ/UART synchronization before connecting the existing QEMU CPU.
No new CPU emulator. No other miner development.

Root also acquired missing original APW7 report ACRT1901202 through the normal
archive workflow. PDF SHA65b6188f0c0f43567af73c2b758189529c3a0803005c09655d97b88343b169bf,
2,936,196bytes. Material-test sample photos may narrow PSU PCB identity; they
are not power or thermal calibration. Initial isolated render retains font
errors; a separate explicit captured-font render exits cleanly. Original PDF
is unchanged. Work and raw specifications are in `s9-psu-512/`.

## Verified source checkpoint after511e

Full workspace formatting, strict Clippy and all-target tests pass:1171passed,
0failed,49explicitignored,125suites. All575 source inputs and source set match
before/after. See `reviews/workspace-check-after511e.json`. This is a checked
source checkpoint, not S9 milestone2 or full-matrix completion. The same
selected recovered logic result has independent execution audit at
`reviews/maxigp0-independent-execution-511e.json`.

512 live PL work remains staged. Original Icarus blocks same-time writes made
in a read-only callback. Browser and cleanup have a source-reviewed future-
only VPI handoff design; CPU conservative synchronization is still required.
513 exact boot/storage work remains staged. Actual BOOT.BIN is79263bytes;
source offset2240 plus declared length requires2240bytes beyond logical EOF.
Its FAT16 allocation has only609zero slack bytes. Adjacent unallocated sectors
must not silently become file data. A primary public Zynq ROM candidate at
`trebisky/Zynq` commit3f64d2fd828e706a8023d308a8370fad1c76a34a has been acquired
only into temporary staging so far. No selectedS9 silicon compatibility is
proved. Its actual read instructions appear to clamp at EOF; a confined
unchanged-code oracle and independent revision binding are next. Do not infer
this behavior from generic producer code or a filename.

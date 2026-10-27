# Simulator continuation — 26 October 2026

The user resumed work after milestone 1.2 and asked to finish the remaining
milestones. Git HEAD at resume was `b276dd7a`; the working tree was clean.
The historical [1.2 handoff](milestone-1.2-handoff.md) is preserved. Its passed
441/443 results remain adapted S9 evidence. Milestone 2 and the full simulator
are incomplete: 79,599 frozen matrix rows and zero full exact-target passes.

## Active work

- Browser agent: run the prepared458b cache comparison after checking all21
  frozen manifest files and20 stored objects. It uses the exact same image,
  two virtual CPUs,4GiB memory and deadlines as457b; only the declared TCG
  cache size differs. It does not grant native browser acceptance. Hold heavy
  builds until this isolated comparison ends. Earlier453 and457 failures stay
  unchanged.
- Root: integrate the staged459 evidence retainer in Rust. Replace whole-file
  cloning with a bounded range. Add growth and empty-file checks. Use a new
  output root, preserve failed access verdicts, then run the unchanged mining
  verifier and Core tests on recovered453 evidence. No new firmware execution
  is claimed by recovery.
- Cleanup agent: integrate staged455 with a read-only plan command using the
  same file locks and hash checks as removal. Review the real272/274/446/447
  evidence and execute scratch-store tests before any duplicate deletion.
  Protect original downloads, unique evidence and canonical274.
- Exact-target agent: resolve selected FPGA source/build identity or advance
  validated reconstruction using pinned source data. Static firmware timing
  contracts are not measured physical timing.

At resume the root filesystem had30GiB free and `/tmp` had11GiB free. Keep
outputs bounded, reuse frozen tools and avoid duplicate prepared images.
The environment now permits filesystem and network access. Use it within the
task scope; downloaded firmware still executes only in the isolated runtime.

## Current commands and results

Final1.2 checks remain historical:919 passed,0 failed,38 ignored,109 suites.
New source changes need their own checks before acceptance. New check logs go
in `checks-2026-10-26-001/`. No milestone gate is changed by a source edit.

## Completed and active checks

- Comparison458b passed actual two-profile startup:49,563ms and48,876ms;
  outer159,583ms,exit0,no truncation. Both cookies/about:blank and owned cgroup
  cleanup checks passed. Same frozen image and2vCPU; TCG cache256MiB. Baseline
  ran on a different day, so cache is not claimed as the sole cause of453.
- Recovery461 source tests:37 passed,4 ignored. The qualified actual453 plan
  test ran once and passed. The first short exact test filter ran zero tests;
  it is retained but does not count as execution. Strict Clippy and release
  build passed. Frozen three tools are in `recovery-tools-461/`.
- `s9_retain_shared_evidence retain` completed against original453. New root
  is `network-vm/native-browser-453-recovered-461/`. All43 files totaling
  1,038,030,428bytes used bounded stream copies. Independent review rehashed
  both copies, checked distinct inodes and preserved failed access verdicts.
  Source453 remains unchanged. Rates audit461 passed; strict regtest audit is
  active in root session5796. No Core acceptance claim yet for this recovery.
- Cleanup460 passed16 ordinary tests, actual four-record evidence test, real
  T17 CLI removal/restoration test, old component CLI test, strict Clippy and
  release build. The first actual-evidence failure led to exact272/274 tar
  omission handling; no archived file was fabricated. Root independently
  hashed all128 duplicate pairs. Actual locked cleanup removed1,623,481,881
  logical bytes. Postcheck verified all114 remaining files and43 restore
  copies. See `cleanup-t17-460/{cleanup,postcheck}.json` and independent460
  reviews. Original archive and unique evidence are outside removal scope.
- Browser agent is freezing462 and preparing actual native run463. The change
  starts two verified fresh Firefox sessions before worker dispatch and uses
  the measured256MiB support cache only in browser mode. Native305/360-second
  budgets and enclosing1200-second support budget remain unchanged. The
  readiness receipt and dispatch ordering use the same VM monotonic clock.
  No new native dashboard pass exists yet.

## Runtime startup fixes and recovered Core checks

- Strict recovered453 audit461 passed1,349 computations and12 matched firmware
  submissions/serial results:6 accepted,6 LowDifficulty;2 fresh jobs, clean
  transition and difficulty change. Both independently reconstructed block
  candidates pass their targets. See `reports/shared-recovery-pass-461.md`.
- Core461 failed before ELF inspection. Exact bounded diagnostic reproduced
  `unshare: fork failed: Resource temporarily unavailable`. The parser's
  NPROC16 limit was applied before user-namespace setup while hostUID had more
  processes. `lab.rs` now applies NPROC16 inside the isolated preparation
  environment immediately before readelf and retains bounded error output.
  CPU/AS/FSIZE/CORE/NNP/isolation unchanged. An explicit real test with24 owned
  host sleepers and malformed input passed. Old failure logs remain.
- Core464 then failed at the generic runtime bootstrap for the same reason:
  its guest process limit32 was applied before namespace creation. Runtime466
  now creates the user namespace, then trusted prlimit sets the unchanged
  declared limit before Bubblewrap executes the closed guest. The carrier
  path still execs without an extra FD3-holding fork. Tool hashes include
  prlimit in result, doctor, local and worker identities. Old locks do not
  silently match the new runtime tool set. No old frozen helper is replaced.
- Runtime466 real checks passed15 isolated-worker cases and6 private-channel
  cases, plus the actual worker CLI carrier case. The new regression forces24
  host sleepers and verifies UID0/1000 guests can run a child while retaining
  soft/hard process limits16. Strict Clippy passed. Current live fixture tool
  lists were updated to include prlimit; full workspace checks remain pending.
- Core466 BOTH passed, in `bitcoin-core/recovered453-candidate{0,1}-466/`.
  Exact block hashes are1c5d5a21017e0d57efd5a80fde5a88f965b65dcd1d97352361538927bdf0aded
  and06024c0b03f2cb77c1040fcdb38fc05c15013f9f48980aad7f7503ab917403e3.
  Each node moved0->1 and retained1 after restart. Neither block was generated
  by Core. Both root sessions66933/37311 ended0. No Core process remains.
- The two remaining host source-build launchers had the same earlyNPROC
  ordering. Cleanup agent is fixing/testing prepare.rs and
  freeze_cgminer_toolchain.rs with the original256-process limit and recorded
  supervisor hashes. Root reviewed the narrow changes. Common browser freeze
  waits for these and final checks; run463/465 was never launched. Historical
  source/helper checkpoints462/464 are preparation records, not new passes.
- Root public downloads succeeded while the exact-target child context still
  failed DNS. New Project X-Ray xc7z010 tilegrid bytes match
  db16874f2827fc05248ad4a7ef5769deaa8e70158a60c8dd40194c48713479ee.
  Exact-target agent owns archival provenance and a strict tilegrid/frame
  association module, preserving overlaps and unresolved features. Public
  zynq-io-bin path history, branches and pinned tree responses are saved for
  source-identity investigation; a branch or filename is not compatibility.

## Common validation and next native browser run

- Both real source preparations 467 passed with the unchanged 256-process
  bound. The installed-tool build and frozen snapshot 003 replay produce the
  same 515,224-byte cgminer binary and pass the existing C regression under
  UBSan. All compiler processes stopped. The report is
  `reviews/source-preparation-process-limit-467.md`.
- Independent recovery review 466 is complete. It verifies all 43 copied
  files, all 12 actual submitted headers and both Core acceptance/restart
  chains. The original browser and gateway shutdown failures remain failed.
- Tile-grid mapping 470 ran in isolation and passed an independent byte audit.
  Root reviewed the parser, checked frame association and fixed-vector tests.
  The map does not establish logic, wiring, timing or original-board behavior.
- The browser agent is running the full workspace checks before common
  freeze 468 and selected native browser run 469. Earlier proposed run
  numbers 463 and 465 were not executed. No new browser pass exists yet.
- The cleanup agent starts read-only research for BM1397 core-register
  command 0x3c=0x80008074, the real boundary in original factory probe 035.
  New edits wait for freeze 468. Heavy builds wait for the browser VM to stop.

## Selected browser run 469 and work after freeze 468

- Common checks 468 passed formatting, strict workspace Clippy and 972 tests
  across 111 suites, with zero failures and 45 declared ignored entries.
  The source checkpoint contains 491 files. Its manifest hash is
  `3ed3a942f004ab872cc36f2c5d3e2b3dba1d65805f1160871bd7ba2cb2417b67`.
  Twelve copied helpers match their release builds. These checks apply to
  freeze 468, not to subsequent source edits.
- Actual browser run 469 passed the preparation barrier and dispatched
  `run-dd9a2c143d7b21d21b989795732901bbf964cfc3478970247f0b2f8b804da1af`.
  It later failed with `selected native browser process failed`. At this
  update, the support VM is retaining evidence. No browser pass is granted.
  The browser agent will inspect the original report and fix the cause.
- Root started CLI work 473 in `crates/md-sim/src/demo_shared.rs`. It reuses
  the frozen native, mining-audit and Core helpers. Its input manifest is
  intentionally incomplete and fails closed. It is not a tested command yet.
  Finish the input inventory, dependency lock update, checks and actual run
  before adding a working command to the README.
- The cleanup agent added the original core0 formatter oracle 472 and a
  narrow raw readback view. The model still rejects the unsupported mailbox
  transaction. Root review corrected two parser-test callback counts before
  execution. Real oracle execution and C checks are still pending.
- Feature data acquisition 471 passed all seven publisher hash checks.
  Exact-target work 474 will decode pinned feature bits. Seven unchanged
  Project X-Ray source files at commit
  `c9f02d8576042325425824647ab5555b1bc77833` are archived for an independent
  isolated oracle. No acquired source was executed on the host. The cleanup
  agent will implement that oracle after completing 472.

## Reviews and fixes before freeze 478

- Native 469 is a retained failure. The real dashboard loaded and login and
  session extension passed. The subscription-schema request then timed out
  while the second profile loaded the application. Both profiles started in
  about 54 seconds. Browser readiness preceded dispatch. The separate native
  HTTP and TLS checks passed 15 exchanges each, and gateway/tunnel stop and
  browser cleanup passed. See `reviews/native-browser-failure-469.md`.
- The next browser change moves the second profile's navigation after the
  first profile's telemetry and WebSocket checks. It preserves the active
  first session, fresh second profile, rejection probe, final session check
  and all deadlines. Root reviewed the source. No new browser pass exists.
- Firmware wall time differs from the client by five days in 469. This is a
  separate clock-consistency finding. The archived chart retains at most 150
  samples; no evidence ties that clock difference to the failed schema call.
  Do not enable the older pool-VM clock adaptation without handling the shared
  worker's delayed dispatch.
- Core0 oracle 472 passed all 5,192 cases and 77,574 callbacks, with empty
  compiler/execution stderr and UBSan traps. Root independently checked the
  original ELF ranges, every callback record and tar contents. See
  `reviews/t17-core0-independent-472.md`. The strict live core-write boundary
  remains unchanged.
- Feature decoder 474 first failed its 512 MiB address limit. Streaming the
  report removed a duplicate JSON allocation. Fresh 474b passed under the same
  limit. It reports 223,128 matches, 346 unresolved segments and 3,678 unmatched
  data bits. The failed output remains. A later alias validation fix and
  negative test require focused checks and new actual probe 474c.
- CLI 473 now has a complete input inventory: 3,574 repository files and 43
  installed host files. Its input check passed. The cohort remains the known
  failed browser cohort 468, explicitly marked in its limits. This is not yet
  a working full-suite command and is not advertised as such in the README.
- Independent CLI review found descendant cleanup, per-stage identity and
  report-reading gaps. Fix 473b keeps the group leader unreaped until group
  cleanup, checks the expected helper hash at each stage, and bounds regular
  report reads without following symlinks. Four tests, strict Clippy and the
  release build passed. Tests include a failed real helper with a child,
  timeout/reaping, changed input identity, FIFO/symlink/size rejection and
  binding Core results to candidates. A mistyped log directory prevented the
  first Clippy command from starting; the corrected command passed.
- Full baseline checks 478 passed formatting, strict Clippy and 985 tests in
  113 suites, with zero failures and 45 declared ignored entries. They preceded
  the final CLI and alias fixes. Record their focused reruns before freezing.
  Browser agent owns freeze 478 and native run 479. Root plans one-command CLI
  run 480 after the browser path passes. Independent upstream feature oracle
  476 remains on a source hold until the freeze, then can be implemented while
  the VM runs. Avoid heavy builds during the native browser test.

## Selected native run 479 and independent FPGA checker

- Freeze 478 is complete. Native 479 passed the two-browser preparation
  barrier, verified all 49 destination objects and dispatched
  `run-2360e47fa0d091b32eb802104a8f52b77de94e1359e05c117937ea7d267218fc`.
  Browser and mining verdicts remain pending. Heavy builds and large hash
  scans stay stopped while the support VM runs.
- Root reviewed the new 476 checker against unchanged Project X-Ray APIs.
  There is no open source finding. It independently checks feature predicates
  over the prior pinned frame map; it does not independently decode packets.
  The README records why this small Python adapter is required. No acquired
  Python has run on the host. Actual isolated execution is next. See
  `reviews/feature-oracle-independent-source-476.json`.
- The 22 additional feature tables are archived with exact commit and
  publisher hashes. The separate 475 manifest has 28 tables. Its execution
  and independent comparison wait for 479 to stop.
- Read-only converter research 482 pins existing fasm2bels source before
  assessing a logic reconstruction path. Metadata downloads are bounded;
  downloaded code does not execute on the host.

- Native 479 reported a failed browser observer at 600 seconds. Native
  execution ended; result transfer is still active. Its detailed failure
  report is required before choosing a browser fix.
- Root added source change 483 for the separate wall-clock finding. Explicit
  `adapt-clock-host-realtime` samples the execution host after dispatch, then
  uses monotonic elapsed time before the original guest date/readback. It is
  restricted to the shared console path and records the selected epoch and
  policy. Device clocks and deadlines stay unchanged. It is not built or
  executed yet and does not establish remote clock synchronization.
- Converter 482 preserves 40 unchanged source and license files (460,241
  bytes) under the configured external archive link. All exact Git blob IDs
  and sizes match. Downloaded code has not executed.

## FPGA comparison passes and support virtualization work

- Expanded probe 475 passed at the unchanged 512 MiB limit. All 28 tables
  produce 231,546 matches over 5,745 segments, including all eight aliases.
  All 327,095 set non-ECC bits have matching predicates. One monitor segment
  still lacks a table. This does not establish a netlist or monitor behavior.
- Unchanged upstream comparisons 476 and 476b passed in 6.065 and 6.700
  seconds. Root independently checked all 32 pinned files, all 1,470 mount
  identities, both complete outputs, every segment digest and residual record.
  No finding remains for this static comparison. The first audit command
  assumed a serialized false loopback field; the corrected command used the
  declared default. See `reviews/feature-oracle-independent-execution-476.json`.
  README now contains the tested expanded commands.
- Native 479 remains failed. Its first HTTP browser passed login, exact asset
  identity, two real WebSocket samples, advancing telemetry and native board
  and pool DOM checks at 140.145 seconds. The second empty profile timed out
  during navigation after 90.866 seconds. TLS browser was not reached. The
  separate HTTP/TLS API probes passed 15 requests each. Gateway stopped at
  the server, but the client did not receive its required acknowledgment.
  Both browser groups were empty. See `reviews/native-browser-failure-479.md`.
- Clock source 483 passed three focused tests, strict Clippy and a release
  build. Independent source review found no issue. Browser source now derives
  its clock policy from the locked native ProcessSpec and enforces the same
  five-second request-interval bound in live and retained results. Those
  browser checks and actual clock-adapted execution still need to run.
- Root confirmed `/dev/kvm` is available and actual KVM API query returns 12.
  Runtime 484 work adds an explicit, narrowly granted support-VM capability.
  Ordinary firmware execution must reject it. Root wires a basic isolated
  boot/TCP prerequisite and selected shared-browser support mode. No KVM VM
  has executed yet. The existing custom support backend 328 lacks KVM; future
  build 485 enables it for x86 only. ARM firmware/model budgets stay unchanged.
- New source and helper freezes must follow runtime 484, browser clock checks,
  support prerequisite execution and backend 485. Do not reuse old results
  for these changed identities. Planned native 486 will exercise the whole
  path after review. CLI 480 remains pending; its old failed cohort is not
  promoted or advertised as working. Cleanup 481 stays deferred; root had
  44 GiB free at the latest check.

## KVM prerequisite 484: fixed ABI call and actual host test

- First boot attempt `network-vm/kvm-prerequisite-484` failed before VM launch.
  `KVM_GET_API_VERSION` omitted the zero third ioctl argument. Linux rejects
  a nonzero argument. The production call and test now pass explicit zero
  and retain OS errors. The failed helper and log remain unchanged.
- Ordinary runtime tests passed 37 library, 1 default integration, 15 worker
  and 6 private-channel cases. Strict runtime Clippy passed.
- The child agent could not see `/dev/kvm`; its ENOENT result remains saved.
  Root ran the real grant/isolation test twice on this exposed host. Both
  passed. The repeated output is `checks-2026-10-26-001/kvm484e-root-actual.log`.
  This checks the support capability only; a Linux boot is still pending.
- New support build 485 will explicitly compile KVM. The prior 328 binary
  has no KVM support and will not be relabelled. The captured toolchain 003
  and overlay 212 hashes match their prior exact identities. Core checkpoint
  478 has verified library `fc59fadd61b39049f10f538f458a5efcc4acd0194d3e73859da5014061d5385b`.
- Parser source (14 files, 66,507 bytes) and three transitive wheels
  (148,000 bytes) were hash-checked and preserved for FPGA export 482.
  No acquired Python code or installer ran on the host.

## KVM Linux boot and support build passed

- Attempt484b failed before Linux: `x86-64-v2` is not a QEMU model name.
  The pinned source defines `Nehalem-v1`, with all required v2 additions.
  The corrected explicit model uses `enforce` and has no fallback.
- New attempt484c passed actual Linux boot and fresh private TCP exchange
  in971ms. Its fresh KVM receipt has API12, device10:232 and no cache reuse.
- Isolated support build485 passed in174,519ms. Configure, compile, strip
  and machine smoke checks all exited0; UBSan was enabled. The new x86
  binary is20,082,048bytes with hash
  `06f21bb63d603b323f36f772c8b288cb044df68e8db88bac46b1f86ae1ff66d6`.
  Inputs and results are in `reviews/kvm-boot-and-support-build-485.json`.
- These are support prerequisites. No native browser or exact target gate
  changes. Freeze486 and native487 are next.
- CLI reader fix488 streams the real49MiB native assessment instead of
  applying the small-report16MiB limit. Its output joins support process
  and result hashes with the native lock. Old failed reports remain failed.

## Review closure before source freeze486

- Independent review verified all71 Linux-boot input mounts and83 support
  build input mounts, plus11 result hashes. No source finding remains.
  See `reviews/kvm-boot-build-independent-485.json`.
- CLI488 passed7 ordinary regressions, the explicit retained479 report
  reader check, and strict Clippy. Independent review confirms the old
  failure remains failed. The full shared CLI still has no passing run.
- Shared Python capture extraction passed its actual expanded oracle replay.
  Root rehashed all746 captured mounts and checked745 non-guard mappings
  against frozen476b. Output bytes match exactly. The first missing built-in
  module attempt is retained. See `reviews/python-capture-independent-482.json`.
- Full486 checks found two missed compact ProcessSpec constructors in tests.
  Both now explicitly declare no support virtualization. Their assertions
  are unchanged. Failed compilation logs remain. Full checks are repeating.

## Frozen native run487 started

- Final baseline passed1009 tests,0 failures,48 declared ignored entries
  across115 suites. Final formatting, strict workspace Clippy and release
  bins/helpers passed. Explicit KVM integration remains separately passed.
- Root independently rehashed509 captured sources,12 frozen helpers and
  all46 prepared input mounts. Native argv and resource limits match478.
  Prepared spec is `a90bda97da7a98174502a756a5c682941e9dceb46aca7f9f60d0ac014c00ffbf`.
- Actual487 runs from `channel-target-486` with custom support backend485,
  explicit KVM and the execution-host wall-clock adaptation. Root session
  is50780. No browser/mining/Core pass is inferred before evidence checks.
- New FASM export source is integrated afterfreeze486. It has no executed
  checks yet. Cleanup481 remains staged and has removed no files. Both
  hold heavy builds while the selected native browser run is active.

## Native487 export failure and next transport fix

- Native487 is FAILED. Its exact run ID is
  `run-03b6619efb06d698a225caed41566320a4382e248a4fac2aca1644b2c6864e4e`.
  All49 destination objects were checked and actual guest logs advanced.
- Outer execution was cancelled after397,456ms. The primary retention error
  is a control character inside JSON at column5,033,150. A Linux IRQ4
  soft-lockup message interrupted the large export on the UART console.
  The later receipt-frame error was an empty ACK after cancellation, not
  proof of an oversized ACK. Its4KiB limit is unchanged.
- Only the declared retention identity and a partial native artifact prefix
  survived. Pool and access payloads were later in the truncated stream.
  Browser/clock/mining acceptance remains unknown for487; no pass is granted.
  See `reviews/native-browser-failure-487.json`.
- Trace489 adds a dedicated virtio port for support userspace output. Kernel
  UART output is retained separately. Guest fd1/fd2 are blocking named-port
  descriptors; worker protocol and receipt ports stay distinct. An8MiB JSON
  stress test generates concurrent labelled kernel noise and independently
  checks complete bytes/digest and both channels. Actual KVM test is starting.
- Cleanup481 removed3,653 verified duplicate copies (1,447,428,871 logical
  bytes). Root independently rechecked all restore/unique/evidence files
  after removal. Two recovered stores without incoming directories remain
  untouched. Original archive and failed verdicts are preserved.
- FASM482 and482b remain failed under the same1GiB cap. First output encoding
  retained too much memory; second saved a complete export but whole-file
  textX parsing ran out of memory. The next harness uses unchanged parser
  APIs on bounded formatter lines, with full independent bit/feature checks.

## Trace transport prerequisites passed

- First stress489 exited0 but failed the eight kernel-record check. The test
  writer used formatted I/O, which could split one `/dev/kmsg` record into
  several writes. It now preformats each bounded line and requires one exact
  write. No noise assertion was relaxed; failed489 remains.
- New489b passes actual KVM Linux boot, private TCP, complete8MiB user JSON,
  fresh challenge and SHA checks, and all eight simultaneous kernel records
  on the separate UART. It finished in1,208ms with untruncated evidence.
  Two independent agents checked the retained bytes and all71 input mounts.
- Ordinary worker component490 also passes with the dedicated trace port.
  It transferred1MiB in each direction, verified destination bytes, retained
  the actual process result, exchanged matching durable receipts, and cleaned
  the run. It finished in9,519ms. Two affected fault regressions are running.
- Planned next source freeze491 and native492 retain all native limits,
 10H/s cap, filter adaptation and execution-host clock policy. They require
  final source checks after the FASM and trace changes. No native gate changed.

## Worker failure paths and next frozen run

- Both actual transport490 fault regressions pass their expected-failure gates.
  The wrong-receipt case rejects a different locked-run identity and retains
  the result without cleanup. The disconnect case stops after65,536 bytes;
  it publishes no object and starts no run. Ordinary success remains false
  for both injected faults. See `reviews/worker-trace-faults-490.json`.
- Full491 source checks, helper capture and prepared native bundle are running.
  The next measured run is492, after independent input checks. Native limits,
  browser deadlines and all assertions stay unchanged.
- FASM482c failed an escaped-quote grammar fixture in the original parser.
  The failed result remains. A separate, recorded grammar correction is staged;
  it must pass the same full/chunk parser fixture before selected export.
- Research493 acquired67 exact BM13xx source files (266,796 bytes) at commit
  `d00c086bc66e6d4d4fa47fa6191ae40e7e566c0d`. Every file matches its Git blob
  identity and SHA-256. The source may help explain the T17 command boundary.
  It is not compatibility or physical timing evidence. Catalogue registration
  and comparison against the archived original firmware are in progress.

## Frozen491 and measured run492

- Full workspace format, strict Clippy, release binaries/helpers and1,029 tests
  pass. There are zero failures and49 explicitly ignored integration entries
  across116 suites. Ignored entries do not establish execution passes.
- Source checkpoint491 pins517 files (6,695,904 bytes). Root independently
  checked all captured bytes,12 frozen helpers and46 prepared native mounts.
  Native argv, environment and limits are exactly486. Only the fresh private
  pool seed/time changed. See `reviews/native-freeze-independent-491.json`
  and `reviews/native-browser-freeze-491.json`.
- Root started native492 with the frozen491 driver, prepared491 and support
  backend485, with explicit `support-accel=kvm`. It retains all native limits
  and browser assertions. No heavy builds run alongside it. Acceptance is
  pending the terminal assessment, complete retained evidence and independent
  mining/Core/browser reviews for this exact run.
- FASM482d source edits are separate from frozen491. The original parser
  profile remains. The adapted grammar, patch and provenance are now under
  `simulator/tools/fasm/`. Focused builds and actual export wait for492 to stop.

## Run492 failure retained; next readiness fix

- Selected run492 is `run-219a3defb1256e5ca2570d24ce61ca3e4f3a80a7ecd1ea0b61ed7d5be5f56ad3`.
  It remains FAILED. Outer support execution exited0 after374,947ms with
  no trace truncation. All274 retained access files and the573-byte shared
  acknowledgment join the exact native, pool and access identities. Native
  result retention and acknowledged cleanup worked. HTTP15/TLS15 API probes
  and gateway/tunnel stop acknowledgments passed. Overall
  `complete_result_exported` remains false because the requested client failed.
- The fast KVM browser reached two WebSocket samples before accepted work.
  Both valid samples had zero accepted solutions; the strict test failed.
  Later native telemetry shows the first accepted solution25 seconds later.
  The fix moves the existing60-second native DOM readiness gate before the
  unchanged20-second WebSocket sampling check. Both positive-share assertions
  and all deadlines remain. Clock and fresh-profile checks were not reached.
  See `reviews/native-browser-failure-492.json`.
- The independent rate audit already checks1,660 computations across all
  three chains. Minimum admission interval100,009,933ns obeys the10H/s cap.
  Direct regtest audit failed because failed access left the derived
  `pool-result.json` absent. The existing strict recovery tool now creates a
  separate43-file evidence copy (1,030,844,658 bytes), preserves all failed
  verdicts, and reconstructs the pool join. No firmware was rerun or altered.
  Submission/Core checks are pending on that recovered492 evidence.
- Full source checks/freeze494 and next run495 are in preparation. Root
  reviewed the minimal readiness-order change against frozen491.

## FPGA482d and BM1397 probe493

- Actual isolated FASM482d passes under the same1GiB/120CPU/180wall limits.
  It regenerates all327,095 original non-ECC set bits with no missing or
  extra bits. Root rehashed997 captured mounts,5,144 frames and the exact
  saved FASM/output/result. The independent raw-frame bit digest matches
  both upstream bit sets and the Rust predicate union. The one-byte parser
  grammar adaptation is separate and pinned. The missing monitor table
  remains unresolved; no netlist, physical timing or original-board pass.
  See `reviews/fasm-export-independent-execution-482d.json`.
- Probe036 runs unchanged ESP-Miner v2.14.2 with the explicit one-shot core0
  write observer. It sends no core reply. The real firmware then writes
  ticket0x14=0xf0 and PLL3 0x68=0xc0700111. The strict model stops with
  UnsupportedCommand, exit1 and a complete trace. Root independently checks
  all17 mounts,127 sources,9 command CRCs and the sole earlier identification
  reply. All board/mining gates stay incomplete. See
  `reviews/bm1397-observer-independent-execution-493.json`.
- Catalogue493 evidence checks pass for67 new exact source files and two
  retained API responses. The next original T17 PLL arithmetic oracle is
  staged as496; it will not invent physical lock or timing.
- Verified task-owned temporary download cleanup removed522 duplicate files
  totaling37,882,049 bytes. Original archive bodies, headers and runtime
  evidence remain. Root filesystem has about29GiB free at this checkpoint.

## Recovered492 checks passed; native495 is active

- Recovered492 strict submission audit passes:12 actual firmware submissions
  correlate with complete hardware replies and1,660 computed hashes. There
  are six accepted shares and six low-difficulty rejections, two fresh jobs,
  a clean-job transition and a difficulty change. Root independently rebuilt
  all12 Bitcoin headers from the raw Stratum messages and verified SHA-256d
  against the per-job pool target.
- Both original Core nodes accept their selected492 blocks and retain the
  exact tip after restart. Heights are0,1,1. Root separately checked raw Core
  UpdateTip/Loaded best chain logs, complete process output, exact daemon and
  block bytes and all captured inputs. Reports:
  `reviews/shared-recovery-mining-core-independent-492.json`,
  `reviews/shared-recovery-core-raw-independent-492.json`,
  `reports/shared-recovery-pass-492.md`. The492 browser failure stays false.
- Full494 format/Clippy/release/guide checks and1,033 tests pass (0 failures;
 49 declared ignored entries;116 suites). Root rehashed523 frozen sources,
 12 helpers,46 prepared mappings and all freeze receipt references. Native
  argv/limits/environment are unchanged from491. Only fresh pool seed/time
  and new support-helper/carrier identities differ.
- Native495 started from frozen494 with support backend485/KVM. Root session
 54312 and log `checks-2026-10-26-001/native495.log` track it. Selected run ID:
  `run-2d806d4f216edd3a517e68f5c4de9df129aef9a63219b994fa14a0bd166d9b56`.
  All49 transferred objects are verified. Acceptance is pending. No heavy
  compiler or oracle work runs alongside it.
- After494 capture, reviewed PLL oracle496 and netlist tool497 source changes
  were integrated. Neither has yet run its focused checks or actual oracle.
  Their work waits for495 to stop.496 executes an exact original ARM leaf
  and establishes arithmetic only.497 uses the pinned existing converter
  with recorded optional-import/parser adaptations and a distinct bounded
  preparation profile. No clocks, sinks or physical effects are invented.
- Next: assess495 fully, fix any actual failure, run independent mining/Core
  checks for that exact execution, then complete496/497 measured checks and
  the next source freeze. Shared CLI promotion still requires a passed full
  browser cohort. It remains pinned to the older failed development cohort.

## Selected browser495 passed; next CLI and model work

- Native495 exits0 after373,176ms with no truncation. The assessment is true
  and complete result export is true. Independent review joins278 retained
  access files, four complete Firefox logs and the exact573-byte receipt.
  The selected SSH and authenticated TLS origins each pass15 API checks,
  actual native DOM and WebSocket work checks, distinct cookie stores and
  the unchanged5,000ms clock bound. Both shutdown acknowledgments pass.
  See `reviews/native-browser-pass-495.json`. This remains an adapted S9
  reference run on one host with isolated worker and client environments.
- Independent rate/submission checks pass for1,664 computations and14 actual
  firmware submissions. Seven shares pass and seven fail for low difficulty.
  Root reconstructed all14 headers from raw messages. Two jobs, a clean-job
  transition and changed difficulty are verified. See
  `reviews/native-mining-independent-495.json`. Core candidate0 has finished;
  candidate1 and the independent Core review are pending the497 resource slot.
- Original T17 PLL oracle496 passes22,134 arithmetic cases in158ms. Root
  rehashed137 frozen sources and47 mounts, joined13 retained tar files and
  checked the unchanged108-byte leaf at its original address. Independent
  single-precision arithmetic agrees with all outputs. Four truncations below
  the exact rational floor remain explicit. No physical lock or timing claim.
  See `reviews/t17-pll-independent-execution-496.json`.
- Netlist497 focused tests, strict Clippy, format and release pass. Actual
  isolated conversion is running. One root process sample shows about2GiB
  resident high-water memory under the declared4GiB address-space limit.
  It is a process sample, not an aggregate or final peak measurement.
- CLI review found that replaying the prepared pool would reuse its seed.
  Fresh preparation498 will clone the verified494 inputs into a new owned
  directory and change only pool entropy/time. A strict parent/child receipt
  must join into the next worker lock. The CLI, input freeze499 and actual
  one-command run500 are pending. Existing failed cohorts stay unchanged.

- Core495 candidate1 also passes. Root independently checks both nodes' raw
  chain logs, all nine input mappings per node, exact block bytes and seven
  source evidence files per node. Heights are0,1,1 and Core generated neither
  block. The joined report is `reports/native-browser-mining-pass-495.md`.
- Netlist497 completed in89,218ms, exit0 without truncation. It outputs exact
  Verilog and XDC files from all219,702 FASM features. Independent retained
  output review and HDL/primitive validation502 follow. Missing monitor
  evidence and original-board gates remain open.

## User-directed pause after CLI500

The user now has few tokens left. Their latest instruction is:
"finish the test than save checpoint and pause". Finish and review the full
fresh-work CLI test, fix any actual failure, save the checkpoint, then pause.
Do not continue501 or502 after that checkpoint without a new resume request.

- Preparation498 passes its real494 derivation and independent review.
  It changes only the pool seed/time and the corresponding process mount.
  All51 baseline files and46 child mounts were rehashed. Parent provenance
  uses strict bounded file reads and is part of the next worker lock.
- Helper cohort499 has13 helpers. Eleven retain exact tested494 bytes; only
  the fresh preparation helper and worker-channel driver are new. This keeps
  the baseline shared-pool runner/verifier identities consistent. Independent
  review is in `reviews/shared-cli-cohort-independent-499.json`.
- The shared CLI input manifest now pins3,579 workspace and43 host files.
  It uses baseline494, backend485/KVM, and helper cohort499. Each run creates
  its own fresh preparation. Formatting and full strict Clippy pass; the full
  workspace tests and final CLI build are running before CLI500.
- BM1397 programmed controls501 are source-reviewed but not integrated or
  executed. Its Git-eligible patch and compact restore record are
  `reviews/bm1397-programmed-501.patch` and
  `reviews/bm1397-programmed-checkpoint-501.json`. Restore/apply was tested in
  a separate temporary copy. Production bases remain unchanged.
- HDL502 is checkpointed at `reviews/hdl-validation-resume-502.md` and
  `reviews/hdl-acquisition-checkpoint-502.json`. Icarus and selected primitive
  sources are preserved and pinned. The gperf download timed out after40s at
 847,474 of1,215,925 bytes; its failed partial and unverified signature stay
  explicit. No HDL build or execution started. Resume with the complete
  support-tool acquisition and isolated build, then actual HDL validation.

- Final CLI source checks pass:1,069 tests, zero failures,49 declared ignored
  entries across118 suites; formatting, strict full Clippy and release pass.
  Input check passes3,579 workspace and43 host identities. Frozen CLI:
  `cli-tools-500/md-sim`, SHA
  `7c39149aa98bb74dc697f1e7ba64e3e627e276cd93ddf7293f2209c110639ad8`.
  Final source checkpoint500 pins538 source files. Build and check evidence
  is in `cli-tools-500/build.json` and `checks-2026-10-26-001/common500-*`.
- Root started the actual full CLI500 in session62570:
  `work/simulator/cli-tools-500/md-sim demo s9-shared --out work/simulator/demos/s9-shared-500`.
  Main log: `checks-2026-10-26-001/cli500.log`. Fresh preparation has passed.
  New pool SHA`38a78aeb2d808cba2d7ac901e6380881393c38dd3c85c98b900d4a5733bd05f1`;
  new process SHA`bb7cc3c11b8324aa02950958dbf6bf145b8eaf6f84c4fe1f348cd89a35024a64`.
  Full execution, browser, mining, Core and final independent joins are pending.
  No heavy work runs beside the native browser. After the full test and review,
  save the final checkpoint and pause as the user requested.
- CLI500 selected run:
  `run-ef37c532ac0c1cc8ab3b207299b928eb0ae9878f8da8941a3b6ef994acfd5661`.
  All54 destination objects are verified. The support image preparation took
  longer than the earlier run but completed. Root confirmed the exact KVM
  process and live worker exchange. A browser report exists, but its bounded
  diagnostic exceeds the small status limit; no verdict is inferred before
  full retention. The native observation window remains unchanged.

## CLI500 passed; paused checkpoint saved

Full CLI500 exits0. Every stage passes. It independently verifies1,346 actual
computations and15 firmware submissions, with nine accepted shares and six
low-difficulty rejections. Seventeen complete hardware responses are verified;
15 match actual pool submissions. Two fresh jobs, clean transition, changed
difficulty and the exact fresh seed derivation pass the independent review.
Both Core candidates change fresh chain height0 to1 and retain the exact tip
after restart. Core generated neither block.

Independent access review checks275 exported files,46 runtime inputs, the
exact573-byte joined acknowledgment and cleanup. Both native browser origins
pass real DOM/WebSocket telemetry, cookie isolation and the unchanged clock
bound. Both15-operation API suites and gateway/tunnel shutdown pass.
Reports are `reports/shared-cli-pass-500.md` and
`reviews/{native-browser-pass,shared-cli-mining-independent,shared-cli-core-independent}-500.json`.

The user's requested checkpoint is `checkpoint-500.md`. Implementation is
paused after the successful test. Staged501 and HDL502 remain held. No new Git
commit was made. No successful full-specification or exact-target claim is made.

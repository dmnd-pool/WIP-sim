# Independent implementation review

Date: 19 October 2026. Reviewer: mining-engine agent. Status: release incomplete.

The review covers the Rust mining crate, catalogue contracts, process isolation, worker storage and control, the CLI, and the source-adapted cgminer path.
The catalogue agent independently reviewed the mining crate.
A component test pass is not a firmware, board, dashboard, remote-network or regtest pass.

## Findings and disposition

| ID | Finding | Disposition |
| --- | --- | --- |
| R1 | The isolation root tmpfs was writable. A guest could write outside the sized `/tmp` and `/work` mounts. | Fixed. The runtime remounts its root read-only. An executed isolation test rejects writes to `/escape` and `/usr/unbounded`. Scratch writes still pass. |
| R2 | Worker root, runs and lock paths could follow symbolic links. | Fixed. Real directory checks and `O_NOFOLLOW` protect the lock. A regression test preserves an external sentinel. |
| R3 | Completed worker threads remained in the active map until shutdown. | Fixed. The worker joins and removes completed threads. A panicked run retains a failed state. |
| R4 | An infrastructure error after process start lacked automatic child cleanup. | Fixed. A process guard kills and reaps the owned namespace. It does not signal a reaped PID. A test confirms descendant pipe descriptors do not delay namespace cleanup. |
| R5 | The source firmware process inherited openpty controller descriptors. | Fixed by the firmware agent. Both PTY descriptors now have close-on-exec. The v6 source runs pass after this fix. |
| R6 | Required hardware fields accepted the wrong JSON types. Hardware evidence IDs could be invented. Component IDs could collide. | Fixed by the catalogue agent. Type, range, chip-address, evidence and duplicate-ID checks are present. |
| R7 | A full snapshot hash embedded in a profile would have a circular identity. | Fixed by the catalogue agent. Targets pin catalogue-data identity. The outer snapshot also freezes profiles and registries. |
| R8 | Duplicate or unknown pool submissions could grow result and trace storage without a bound. | Fixed. Explicit message, submission and trace limits are retained in each pool report. Budget failure retains an incomplete report. |
| R9 | Poll readiness on a blocking inherited descriptor did not bound a large write. | Fixed. Inherited IPC is nonblocking. Poll checks error and hangup flags. A stalled-peer test confirms a 4 MiB write fails within one second. |
| R10 | Matrix run trusted only the plan's self-hash. A rehashed empty plan could claim complete coverage without the frozen catalogue. | Fixed by root. The runner loads and verifies the named snapshot, regenerates its plan, and checks an exact identity match. It also requires a new output directory. The actual forged-empty-plan command is rejected; see `reports/cli-checks.json`. |
| R11 | The source manifest omitted the Bitcoin fixture compiled into Rust tests. | Fixed by root. The source identity includes fixture and backend input directories. |
| R12 | A source-adapter target could declare no boot entry. | Fixed by the catalogue agent. Executing source targets now require a real entry and an explicit changes array with a reason. |
| R13 | CLI process execution did not persist an infrastructure error result. | Fixed by root. The CLI saves an infrastructure-failure result and returns exit code 4. |
| R14 | The new preparation runner repeated the writable-root and error-cleanup defects. | Fixed by the firmware agent. It remounts root read-only, drops capabilities, sets no-new-privileges and guards the owned child. Optimized and UBSAN v6 preparations pass. |
| R15 | Equal ASIC and pool thresholds did not test the distinction between the two filters. | Fixed. The pool supports the exact rational difficulty factor 3/2. Its target is integer floor(2*T/3). The firmware model keeps the programmed power-of-two filter. A boundary test and the live IPC test pass. Final source-firmware test uses this factor. |
| R16 | An untrusted ELF interpreter could name an unrelated host file. | Fixed by the firmware agent after root review. The loader path must match the system-loader allowlist. Canonical paths must remain in system library roots and have ELF magic. A regression rejects unrelated paths before reading. |
| R17 | UBSAN found unaligned upstream nonce2 writes and hash-display reads. | Fixed by the firmware agent with byte-preserving `memcpy` and an aligned target. Failed runs `reviewed-ubsan-002` and `final-ubsan` remain saved. Both v6 UBSAN source runs pass, including the nonce-zero regression. |
| R18 | Early nonce responses exposed stale queue-credit readback in the upstream sender. | Fixed by the firmware agent with recorded source adaptation. Status work count and generation are tracked. Stale replies do not restore spent credits. The failed zero-nonce run is retained and v6 zero-nonce mining passes. Missing-status recovery remains unimplemented. |
| R19 | The protocol-only coinbase did not contain the canonical height-one script required by active regtest BIP34. | Fixed for explicit regtest mode. The job uses `OP_1`, an exact genesis parent and a distinct coinbase per job. Random-parent protocol evidence remains separate. Actual Core acceptance is still required. |
| R20 | The first node import probe checked block bytes but did not link them to the saved firmware submission. Its generic daemon argument also did not establish a trusted Core source identity. | Fixed by the catalogue agent. The probe requires the exact source-build binary registry. It verifies source evidence checksums, reads the saved verified copies, reconstructs the recorded accepted share, and finds its exact `mining.submit` in the firmware trace. Build, run, candidate and log identities are retained. All four final v10 source block imports pass. |
| R21 | The pool allowed only 120 seconds of rolled time even when the low-rate mining budget was longer. Valid work could become out of range before the declared deadline. | Fixed. The time window covers the rounded-up work budget plus 120 seconds of clock margin. Budgets must be positive and at most one day. Checked addition rejects epoch overflow. A computed share at 300 seconds passes under a 600-second budget; work beyond the allowed window is rejected. Final v10 source runs pass with this harness. |
| R22 | A per-object limit did not bound the total host input store. Repeated transfers could exhaust storage. | Fixed. Aggregate logical bytes and object slots include incomplete transfers. Exclusive process and thread locks serialize admission and writes. Six behavioral quota tests pass. The catalogue agent independently reviewed the implementation and policy recovery with no remaining scoped finding. |
| R23 | Concurrent runs could consume host evidence storage without an aggregate reservation. | Fixed by root. Run admission reserves raw traces, encoded result traces and bounded metadata. Actual retained bytes and active reservations are charged across restart. An integration test rejects a second active run before its first peer writes its possible full output. Cleanup releases only acknowledged run evidence. |
| R24 | Abrupt supervisor loss could bypass the Rust child cleanup guard. | Fixed by root. The child requests a parent-death signal and checks the parent identity before exec. A real subprocess test kills the supervisor and confirms that its namespace descendants cannot continue execution. |
| R25 | An interrupted run had no completed result to acknowledge, so retained storage could not be released through cleanup. | Fixed by root. Restart preserves partial files and their hashes, creates an explicit lost result, and permits identity-acknowledged cleanup. It never creates a firmware pass. |
| R26 | Loss between the recovery result and state commits left Status without a state file. PID-only temporary names could also collide with retained fragments. | Fixed by root after review. Missing state gets an explicit lost status without changing the saved result. Atomic writes use a random 96-bit suffix with exclusive creation. The regression preserves the old same-PID fragment and exact result identity across repeat recovery. |
| R27 | First store policy serialization could leave a partial authoritative policy after interruption. | Fixed after root review. A bounded pending file is published without overwrite after synchronization. Restart recognizes incomplete serialization and the same-inode publication window. Committed corruption and conflicting valid policies remain visible errors. |
| R28 | The frozen toolchain verifier could accept a symlink at its `/usr` root, and permission comparison omitted special mode bits. | Fixed by the firmware agent after review. Snapshot roots must be real directories. Modes include all permission bits. Regression tests reject root links and permission changes. |
| R29 | Toolchain output overflow discarded diagnostics. Directory traversal also queued entries before applying its count limit. | Fixed by the firmware agent after review. Bounded partial output and error status are saved. Capture and traversal check limits before growth. The custom supervisor also applies the parent-death guard. Four example tests and strict lint checks pass. |

The catalogue agent re-reviewed R8 and R9 and ran all 11 mining tests then present. No further correctness finding remained in the reviewed target, midstate, filter, rate, independent pool or inherited IPC code.
Root and the catalogue agent also independently reviewed the new regtest anchor and block export. They found no defect in supported serialization, byte order, BIP34 prefix or proof-of-work revalidation. Core acceptance remains the external consensus oracle.

## Executed checks

- `cargo test -p md-sim-mining --all-targets --locked --offline`: 15 library/integration tests and one evidence replay test pass.
- `cargo clippy -p md-sim-mining --all-targets --offline -- -D warnings`: pass.
- `cargo test -p md-sim-runtime --locked --offline`: 20 tests pass in the independent review run: six unit, eight integration and six store-quota tests.
- `cargo clippy -p md-sim-runtime --all-targets --offline -- -D warnings`: pass.
- `cargo fmt -p md-sim-mining` and `cargo fmt -p md-sim-runtime`: pass.
- `python3 scripts/verify_bitcoin_fixture.py`: pass.

The pool test uses an actual private inherited connection. It tests two fresh jobs, a clean transition, a difficulty change, and invalid, duplicate, stale and out-of-range submissions.
The engine and verifier use separate SHA-256 implementations.
The midstate tests compare 256 varied complete headers and an archived BM1397 state vector.

## Firmware evidence reviewed

The preserved v10 source runs use their recorded canonical workspace release build and retain real serial packets, submitted shares and independently reconstructed headers. Later runtime quota and recovery changes require a new canonical run. The old reports are not evidence for the changed harness.
The tested harness SHA-256 is `b94cb250323b30fc49204d979cdca11953a4e0c2d451ecf06e6de01e5856d3e8`.
`v10-default` has 34,337 actual hashes and two accepted jobs at a 10,000 H/s cap.
`v10-low-cap` has 2,287 actual hashes and two accepted jobs at a 1,000 H/s cap.
`v10-regtest-ubsan` has 50,038 actual hashes, five ASIC results and two accepted anchored jobs without sanitizer errors.
`v10-zero-nonce` has six actual hashes and two accepted jobs. Actual nonce zero succeeds for distinct jobs; it is not a manufactured response.
The replay tool checks raw packet CRCs with a separate bitwise implementation. It matches work midstates to complete pool headers and checks them with the independent full-header hash oracle.
For example, `reviewed-002/independent-asic-replay.json` confirms a real ASIC-valid result at sequence 12 which fails the stricter pool target and is not submitted by firmware.
The firmware is rebuilt cgminer source. The low-rate and private transport changes are adaptations.
The hardware is an 8-chain, 32-chip controller-interface fixture derived from source. It is not an exact physical board profile.
`v10-regtest` has 25,718 actual hashes, three ASIC results and two accepted anchored jobs. Earlier random-parent runs cannot satisfy regtest acceptance.

The independent reviewer checked both candidate indices in `bitcoin-core/v10-regtest-candidate-{0,1}` and `bitcoin-core/v10-regtest-ubsan-candidate-{0,1}`. This includes each acceptance report, full Core log, locked process inputs and source-run link. All recorded evidence hashes match the saved files.
Untouched archived Core source `619334569b44b4278d0155bb8daf4c8535a47367` produced daemon SHA-256 `a22238f0a1ea0b0a67a17032d2df0039e9d3ac946f2e86e0634ea47448692fae`.
Core accepted these actual submitted blocks, each in its own fresh chainstate:

- Optimized job 0: `000144c1560e2b36999009a239c0a38ce7a0a85cbfea3eaec1996dfa7d65d8d5`.
- Optimized job 1: `00034d074b16d166d483021f3ff069a3c42adc604b4def4f633199f01cc5f811`.
- UBSAN job 0: `0002a601f31eec6000d13f9454bdfdaa1c0486b9def99576a7d4865bf6c39a46`.
- UBSAN job 1: `00005a45c2cfc8fe681384cc78441b166181c6baa6c0d3c88ab6cbbc131c6509`.

Core's own `UpdateTip` logs record regtest genesis at height zero and each exact block at height one.
Two Core processes share the same private temporary chainstate. The first initializes genesis; the second imports the exact firmware candidate. Both stop cleanly, and the wrapper exits zero. This startup order retains all assertions. The first direct-import attempt failed a Core block-index assertion and remains saved.
The example does not alter Core consensus rules, disable the assertion, search another nonce or request a node-generated block. This establishes the source fixture's block-acceptance capability. It does not establish original-board, exact-target, RPC, native-dashboard or remote-network gates.

## Worker and original-kernel follow-up review

The reviewer checked `Store::abort`, worker `Logs`, `ResultData` and `Cleanup`, and the doctor's disk and essential-tool checks. No correctness finding remains in these additions.
Abort removes only a validated incoming regular file and retains published objects. Live logs are bounded host-owned files. Completed results can be retrieved in bounded chunks with exact identity checks. Cleanup requires the completed result identity and rejects unknown files or links before deleting run-owned evidence. Behavioral tests cover reconnect, worker restart, live logs, results larger than one control message, and preservation of CAS inputs and an external sentinel.
These checks use private inherited process connections. They do not prove network listeners, SSH or remote dashboards.

The later quota and recovery review covers `StoreLimits`, active-run evidence reservations, restart, interrupted transfers, abort, publication, supervisor death and cleanup. All 20 runtime tests pass. Store accounting counts logical bytes, with metadata bounded by object slots. It does not claim filesystem block quotas or aggregate CPU and memory control groups. The default store policy allows 16 GiB and 4,096 object slots. The worker allows two active runs, 1,024 retained runs and 4 GiB of retained evidence. The exact policy is recorded. See `store-progress.md` for the store commands and recovery behavior.

The reviewer also checked the original Zynq probe's archive identities, bounded extraction, isolated input closure and claims. Early one-CPU probes execute archived Linux instructions and stop before init. Attempt 008 reaches actual archived init, preinit, procd, ubus initialization and module loading after an explicit FPGA I2C initialization bypass and RAM-disk root selection. All nine saved evidence hashes and the probe source hash match. Missing NAND and SD storage remain actual errors. Miner service health, persistence, watchdog behavior and dashboards are not established. Two-CPU probes fail before guest entry. The historical `original-bootargs` label explicitly states that QEMU supplies the DTB text through `-append`. These are adapted loader paths, not an original full-board boot.

## Frozen toolchain review

The new Rust example captures bounded installed package files and checks all recorded file, link and directory identities. The build mounts only the captured `/usr` tree. The package graph chooses a conservative set; the isolated replay establishes that this exact build recipe has its required inputs. It is not a minimal-input or package-source reproducibility claim.
The reviewer checked the final optimized and UBSAN preparations in `firmware/prepared-closed-002` and `firmware/prepared-closed-ubsan-002`. Both report exit zero, complete bounded evidence and no stderr. Their C regression tests pass. The saved closure manifests both hash to `e2eabd7bd16c22397c37cfbcfd0115223406f7850b5ad8f0733de6ac79e4d12a`.
The independently hashed firmware outputs remain `88602d125e46310d0e61f64622d50be25df285085c2fe09c63070d852c64a90a` and `0b18f7f5aa273df38e204690883cf948b6d23be0e294085719b87b9ff6358326`. These match the earlier firmware bytes. Matching firmware bytes alone do not reuse execution evidence after the harness changes.
The reviewer also ran the final snapshot verifier against `firmware/toolchain-snapshot-003`. It verified all captured entries and returned the recorded manifest identity.
The static adaptation manifest still describes the earlier installed-toolchain baseline. `firmware/toolchain-closure-report.json` identifies that exact preserved input and states that each later preparation's closure and recipe manifests define its actual environment. The reviewer checked this distinction against the saved hashes. No prior input was rewritten.

## Current canonical source replay

All five v11 source runs use harness `90a5a39eb663994a1473385d1ccaad45a123b9c637189b001057f1a28721b1c3` and the frozen preparation outputs. Independent replay of their raw serial packets and complete headers passes. The reviewer checked every prior root-file identity before adding each replay report to its evidence index.

| Case | Actual computed hashes | Returned ASIC results | Independently valid firmware pool submissions |
| --- | ---: | ---: | ---: |
| v11-default | 6,005 | 2 | 2 |
| v11-low-cap | 171 | 3 | 2 |
| v11-regtest | 10,073 | 3 | 2 |
| v11-regtest-ubsan | 14,071 | 2 | 2 |
| v11-zero-nonce | 6 | 3 | 2 |

The low-cap, optimized anchored and nonce-zero cases each return one actual ASIC-valid result outside the stricter pool target. Firmware correctly withholds these results from the pool. See `reports/v11-independent-replay.json` for the replay executable identity and check scope. The earlier v10 acceptance evidence remains separate.

## Current Core acceptance and persistence review

The final three-phase probe source has SHA-256 `c328e68a042bc3a66621f50b4603c836f31968127ef31fa5d803eddfe72b97bf`. Two oracle regression tests and strict lint checks pass. Its wrapper starts the same original Core daemon three times in one private temporary data directory. Only the middle process receives `-loadblock`. The third process loads the saved chain without that argument.
The reviewer checked all four current runs. Each raw transcript has three distinct ordered phase markers. The first process records genesis at height zero. The second loads that genesis, then accepts the exact firmware candidate at height one. The third loads that same candidate at height one. Each phase has its own single clean stop and shutdown. All wrappers exit zero with empty stderr and complete traces.

| Candidate | Exact accepted and reloaded block hash |
| --- | --- |
| v11-regtest job 0 | `000a2820d8400d95c5d26744cda1315a611ebcf27ae3cf082e80bb9de6f68086` |
| v11-regtest job 1 | `000378e50038ece131dea055d18a4499f36b21866e7f432df3cd5d3b3d4b842a` |
| v11-regtest-ubsan job 0 | `00039ab0149a9caf66c02e26208aaa5f44a8a9b7e0ffe957d69a40dc30cbedd9` |
| v11-regtest-ubsan job 1 | `0002995074f31c2caa9c9eb32b08f6a8182190c9e612531682f6d9fa5bd1ce2d` |

Each candidate exactly matches its actual source firmware export. The reviewer checked all 11 copied source files and their original index identities, five report references, the exact daemon mount, and all 11 top-level evidence files for each Core run. No remaining correctness finding applies to this component. See `reports/v11-core-independent-review.json`.
This proves source-fixture block acceptance and process-restart persistence in private temporary storage. It does not prove host-crash durability, RPC, external networking, original-board boot or acceptance for each exact hardware target.

## Required completion findings still open

- The full requested hardware and firmware matrix does not run. Classification is not execution.
- There is no accepted original board boot result.
- There is no Bitcoin Core regtest block-acceptance result for each exact target.
- The source fixture does not establish a complete downstream ASIC, MCU, PSU, cooling, storage or recovery model.
- Native dashboard service tests, SSH transport and an authenticated TLS gateway remain incomplete.
- The host denies network socket operations and listener binding. Inherited local IPC does not pass TCP or remote-network gates.
- Whole-run cgroup limits are not implemented. Current limits are per process and per scratch mount. Reports state this limit.

These findings prevent a successful release claim even if every current Rust test passes.

## Locked coordinator and wire-verifier follow-up

The new coordinator now dispatches a real source-interface suite. It does not change the full physical matrix result. Independent review covered complete plan and resume identities, bounded evidence, cancellation, initialization loss, transfer, request-token recovery, repeated result-retrieval loss and cleanup ordering. Eight behavioral tests use actual isolated processes and a separate private IPC worker. All pass. Their component results do not establish a remote network gate.

Review found and fixed missing evidence-reference checks, unrestricted verifier exports, interrupted initial locks, an untracked worker-dispatch window, repeated partial-result overwrite, and missing destination directory sync before cleanup. Root also found an invalid recipe panic, missing serial work/header replay, an unsafe closure output path, and missing trace sequence/time checks. These are fixed and tested.

The actual plan 004 run exposed a verifier error with a legitimate stale submission after a clean job. Replay now processes the trace in order and preserves stale and duplicate decisions. Later root review found that the replay did not bind complete wire notifications and difficulty to the saved job records. The fix checks the correlated subscription and extra nonces, every notification field, exact supported wire difficulty, locked job policy and the wall-clock budget. Tamper regressions pass. The catalogue reviewer checked the ordered transition and serial trace additions; root checked the full wire-binding fix.

Plan 007 passed locally and through the separate private worker using the same locked ProcessSpec. Both resume checks reused their original sealed result. The verified commands, exact plan/run/build hashes and reports are in `coordinator-progress.md`. This establishes dispatch, transfer and verifier behavior for the rebuilt cgminer 8-by-32 source interface fixture. It does not declare that fixture to be an exact physical board. Earlier plan and firmware evidence remains separate.

The reviewer also checked `examples/support/s9_service_report.rs`. It binds the script, firmware components and driver/verifier image, and checks retained console bytes against ProcessResult. Its HTTP/RPC checks cover login, redirects, the exact native stylesheet, live uptime, configuration write/readback, service restart and exact restoration. Its report correctly leaves browser JavaScript, full guest restart, mining telemetry and remote access incomplete. No correctness finding remains in this scoped report review.

The ESP32 original-firmware probe received a separate source review. It preserves the original factory image, records trailing erased flash fill, verifies backend and ROM identities, and runs in bounded private storage. It leaves complete board, mining and dashboard gates false. The diagnostic ESP QEMU build omits generation of unavailable upstream floating-point test executables through a recorded option. Those test dependencies and tests remain an open gate; this adaptation is not a release exclusion.

The coordinator 007 Core reports received independent raw-evidence review. All four imports match their exact retained source exports and all 11 copied source files. SHA-256d of each header and coinbase was recomputed independently. Import framing and complete block bytes match. Each raw transcript records genesis, the exact height-one update, and the same tip after a third process starts without loadblock. See `reports/coordinator-007-core-independent-review.json`. This closes the scoped candidate acceptance check, not exact hardware coverage.

SSH process review found an unbounded child-wait and pipe-collector path. The transport now uses nonblocking bounded pipes, per-request time and resource limits, private process groups, and parent-death cleanup. It observes child exit without reaping, then stops the group before releasing the PID. Five library tests pass, including blocked stdin, excessive stderr and a descendant that keeps a pipe open. Strict Clippy passes. The catalogue agent independently reviewed the fix with no remaining scoped finding. Actual SSH connection and selected dashboard tests remain separate integration evidence. The same reviewer checked the corrected certificate helper cleanup; it also stops the owned group before reap and avoids signalling a reused PID.

The AXI IIC C ABI review checked the new fatal unwind path. Its personality signature matches the Linux host ABI and aborts if invoked. Panic also aborts. The C regression checks SIGABRT with core files disabled. No model fault is converted into a successful device response. Backend compilation and actual firmware behavior remain separate checks.

Worker listener restart now holds the exclusive root lock before endpoint recovery. A separate endpoint lease and bounded receipt bind the canonical path, device number, inode and owner. Recovery and cleanup remove only that exact socket. Foreign regular files, symlinks, unrecorded sockets and inode replacements are retained and rejected. Root independently reviewed the implementation. Eleven library tests and eleven private-IPC integration tests pass, with strict Clippy. These tests do not prove listener restart: the endpoint unit tests create only filesystem socket inodes. The separate actual VM test now passes: `aggregate-limits/run-004/guest.log` records SIGKILL of the worker, restart at the same socket, an explicit lost result, removal of the exact recorded run cgroup, and preservation of its unrelated parent. This proves the scoped listener restart path in that disconnected VM. Abrupt loss after bind but before receipt publication leaves an unrecorded socket and fails closed; no request is accepted before publication.

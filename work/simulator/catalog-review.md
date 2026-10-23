# Independent implementation review

Date: 19 October 2026. Reviewer: catalogue agent.

The requested release is incomplete. This review does not approve the complete simulator.

## Mining library

Reviewed `crates/md-sim-mining`.
The review covered compact targets, SHA-256 compression state, byte order, version ranges, nonce ranges, ticket-mask probabilities, rate limits, pool verification, and adapted Stratum input/output.

Two findings were corrected:

1. Repeated duplicate or stale submissions could increase the result and trace without a fixed bound. The pool now limits messages, submissions, and trace bytes. A budget failure retains an incomplete report.
2. The inherited descriptor could block during a large write after `poll` reported some free space. The descriptor now uses nonblocking input/output. The code checks error and hang-up flags. A stalled peer fails within the configured bounds.

The corrected code was reviewed again.
`cargo test -p md-sim-mining --locked --offline` passed all 12 tests.
The later exact rational difficulty conversion was also reviewed. It computes the pool target for a factor of 1.5 without changing the programmed ASIC filter. The integer carry, division, and supported exponent limits are correct.
The later consensus block export uses the pinned Core regtest genesis, a height-one coinbase and a reconstructed accepted source submission. Its block framing and hash calculation were reviewed. The low-rate time-window fix was also reviewed: positive budgets through one day are rounded up, and checked arithmetic adds the fixed 120-second clock margin. The change does not alter bus or network deadlines. No correctness finding remains in these additions.
No unresolved correctness finding remains in these reviewed library paths.
These tests do not prove firmware execution, Bitcoin Core acceptance, TCP transport, Stratum V2, or full target conformance.

## Source firmware path

Reviewed `crates/md-sim-firmware`, `simulator/firmware/cgminer-low-rate.patch`, and the retained `work/simulator/firmware/attempt-004` evidence.

The source code rebuilds archived cgminer commit `b8491c66e7e22f23a9edf095dd1337ee581e88bd`.
The archive SHA-256 is `4b3515ec072904e81e91182f5f8b5b87361ef2a7e2131643d8207badb996`.
Attempt 004 contains guest logs, real task packets, real returned nonces, and two independently reconstructed accepted headers.
The programmed difficulty changes between the jobs. The second job clears the controller queue.
The source and output identify the path as rebuilt source with an adapted controller interface.
They explicitly exclude original board boot, exact board identity, downstream FPGA execution, native dashboard tests, and regtest acceptance.
No nonce fabrication was found in the reviewed path.

Initial findings sent to the firmware agent and corrected in code:

1. The public session entry checked only the user namespace. A user namespace alone does not isolate the filesystem and network. The public session binary was removed. The library now checks the private root, mounts, network interfaces and resource limits before acquired code runs. The host wrapper uses the strict worker and locked ELF inputs.
2. Attempt 004 did not bind the executed binary, model, verifier, adaptation, and complete run specification in one input lock. The wrapper now saves these identities, output hashes and failed-run records. Seed and epoch are resolved before the process specification is hashed. Attempt 004 remains preliminary evidence.
3. The low-rate `test_nonce` adaptation checked the pool target. A valid ASIC result can be above the pool target. The patch now constructs the device target from `work->device_diff`. The later pool check stays separate. The Rust boundary test and fresh-work runs use different ASIC and pool targets.
4. Original cgminer rejects equal successive nonces across different work IDs. Its initial previous nonce is zero. The recorded source variant now includes work identity and a first-result flag in both device and Stratum duplicate checks. The source adaptation retains actual nonce bytes.

The `v5-default` run was superseded by v6 after the later source fixes. Both remain source-derived controller fixtures, not exact boards.

Later review findings sent on 19 October:

1. UBSAN found original unaligned nonce2 stores and hash-display loads. The patch uses `memcpy` to preserve bytes. Review also found that the hash-display scan could end at offset 29 and read past a 32-byte array. The exact C helper now stops at offset 28. Its UBSAN regression tests all 32 nonzero-byte positions and an all-zero hash.
2. An early nonce exposed stale FIFO credits. The source now preserves a pending clean replacement until a packet is sent. It restores replacement credit under the send lock. Status requests record sent-work counts and the clean generation. Replies subtract later writes and cannot restore credit from an old generation. At most one status request is pending. The exact C helper regression tests old responses after clean transitions and writes after status requests. The model continues to report its actual queue space.

The v6 patch and helper hooks were reviewed again. The patch SHA-256 is `1cc3f80bbd223e9c7fa2c2313a306ce388f9c3c076de8c81360fea4308594f4b`.
The regular firmware binary hash is `88602d125e46310d0e61f64622d50be25df285085c2fe09c63070d852c64a90a`.
The UBSAN firmware binary hash is `0b18f7f5aa273df38e204690883cf948b6d23be0e294085719b87b9ff6358326`.
Both preparation manifests record the successful UBSAN C regression. The regression source hash is `4ecfadf525ef102327cc8633ce5de6b134cb90326ad9832f9a1b36ad5cc7acdf`.

The retained v6 runs contain:

| Run | Hash cap | Actual hashes | ASIC results | Accepted fresh jobs |
| --- | ---: | ---: | ---: | ---: |
| `v6-default` | 10000 H/s | 6725 | 2 | 2 |
| `v6-ubsan` | 10000 H/s | 8793 | 3 | 2 |
| `v6-low-cap` | 1000 H/s | 971 | 2 | 2 |
| `v6-zero-nonce` | 100 H/s | 6 | 3 | 2 |

The final row returns nonce zero on two distinct jobs through the real source process. It uses fresh work and the UBSAN binary. All four runs retain an independent full-header replay report. A returned ASIC result above the distinct pool target is retained as a real device result and is not counted as an accepted share.
`cargo test -p md-sim-firmware --locked --offline` passed six tests. `cargo clippy -p md-sim-firmware --examples --locked --offline -- -D warnings` passed.
No unresolved correctness finding remains in this reviewed source fixture path. Missing-response recovery, complete fault behavior, per-chip buses, exact physical profiles, original firmware execution and the release gates remain outside this limited result. This is not approval of the complete simulator.

## Original archived kernel probe

The separate probe is recorded in `work/simulator/original-board-progress.md`.
It verified the original Braiins S9 archive, FIT and selected kernel, root image and DTB hashes. Preparation and QEMU ran in strict isolation.
The two-CPU generic direct loader failed before guest entry. An explicitly adapted single-CPU and console probe executed the unchanged archived kernel and printed actual driver logs. It did not reach an observed init or miner service.
Neither probe passes the original board gate. Exact board identity, reset path, NAND state, FPGA peripherals, mining and native dashboards remain incomplete.
The later diagnostic sequence in that progress file locates the stall at `xiic_i2c_driver_init`. A separately recorded initcall blacklist, RAM-root argument and one-CPU direct loader then reach actual archived init, preinit, procd and ubus in attempt 008. The evidence shows missing SD partitions and NAND identification failure. No mining or dashboard success is claimed. The mining agent reviewed all attempt-008 evidence hashes and the probe source. Its only wording finding was corrected: exit 137 is consistent with the configured CPU limit, but the exact signal cause was not measured.

## Independent Bitcoin Core path

The archived Core source built without changes inside the preparation environment. Its source archive, commit, exported daemon, recipe and build log are pinned in `simulator/backends/bitcoin-core-619334569b44.json`. The read-only build toolchain is not fully frozen.
The mining agent reviewed `node_prerequisite_probe.rs` and actual v9 candidate-zero acceptance. The initial standalone block input was insufficient to prove source provenance. The corrected path uses verified copied source reports and traces, requires the indexed accepted submission, reproduces its entire block record, and checks the matching firmware-to-pool submission. It also requires the source-verified Core binary before making a consensus claim.
The first fresh-directory import exposed an original Core block-index assertion. The correction initializes genesis and imports in two successive original Core processes within the same private directory. It keeps all assertions enabled. Actual logs show genesis height zero, the exact candidate height one, and both clean shutdowns. No remaining correctness finding was reported in this limited node import path. RPC, post-import persistence, exact-board execution and complete release approval remain separate.
The mining agent repeated its evidence checks for all four final v10 candidates. Each optimized or UBSAN source candidate passed an independent fresh-chain import with exact source linkage, ordered genesis and candidate tips, both clean shutdowns, exit zero and empty stderr. The final preparation command was also repeated after a quote-escaping defect was found in the saved build-002 command transcript. Build 003 used the corrected saved command and produced exactly the same pinned daemon bytes. Both transcripts and all failed earlier probes remain available.
The current v11 result adds a third unchanged Core process in the same private chain directory. It starts without a block-import argument and loads the exact selected candidate at height one. The mining agent independently checked all four three-phase runs, source links, input records, evidence hashes and clean shutdowns. Its report is `work/simulator/reports/v11-core-independent-review.json`. There is no unresolved scoped finding. This establishes process-restart persistence; RPC, host-crash durability and complete target gates remain open.

## Catalogue and CLI review coordination

The mining agent independently reviewed the catalogue crate.
The root agent reviewed the snapshot reference design and matrix trust boundary.
Corrections cover typed hardware fields, evidence IDs, duplicate component IDs, parent directory links, explicit source execution entry, adaptation records, and the catalogue/registry hash cycle.
Fifteen catalogue tests and strict lint checks pass.

The root agent owns CLI follow-up checks. These include comparing a loaded snapshot hash with the requested frozen ID and regenerating matrix classification from the frozen source before trusting plan exclusions.

No inventory classification in this work is counted as an executed firmware test.

## Frozen cgminer preparation review

The review covers `crates/md-sim-firmware/examples/freeze_cgminer_toolchain.rs`, source SHA-256 `082071058c476408e2645f25f4436fe7d6c71f05fefc17ef82857f3f26ca115a`, and the final `prepared-closed-002` and `prepared-closed-ubsan-002` records.
The capture includes 102 installed packages, 30,840 entries and 697,927,858 regular-file bytes. Its manifest records exact bytes, modes and link targets. Verification checks the complete captured tree and rejects missing, extra, changed and linked root entries.
The final manifest is `e2eabd7bd16c22397c37cfbcfd0115223406f7850b5ad8f0733de6ac79e4d12a`, with 7,144,215 bytes. An independent `verify work/simulator/firmware/toolchain-snapshot-003` run matched it. All four example tests passed.

The reviewed build mounts that captured tree as read-only `/usr`. It does not mount the installed host `/usr`. It verifies the capture before and after each bounded, isolated build. The final optimized and UBSAN preparations exited zero without truncated output or capture errors. Both output hashes match the earlier recorded binaries. The preparation reports retain source, recipe executable, namespace supervisor and captured-manifest identities.
This proves replay from the captured installed package bytes for the selected recipe. It does not prove package-source reproduction, a minimal dependency set, or upstream package authenticity.
The static adaptation manifest retains the older preparation baseline description. The per-preparation input record explicitly identifies the captured tree and no installed `/usr` mount. The firmware agent records this distinction separately, without changing earlier evidence bytes.
No unresolved correctness finding remains in this limited captured-input preparation review.

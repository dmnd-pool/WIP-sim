# Mining engine progress

Date: 19 October 2026. Status: component tests pass. Full simulator gates do not pass.

## Implemented

- `crates/md-sim-mining` contains a Rust SHA-256d engine. It uses `sha2` 0.10.9.
- The engine restores a SHA-256 compression state. It does not hash midstate bytes as a new message.
- The cgminer adapter converts little-endian state words and word-swapped tail bytes.
- A single rate limiter caps actual hash calls. The default engine cap is 10,000 H/s. Idle time does not create burst credit.
- Work has an inclusive nonce range, a version mask, a bounded queue and an explicit programmed ASIC filter.
- Numeric target and zero-bit mask filters have separate tests. Share interval estimates use the intersection with the pool target.
- The pool uses the independent `bitcoin_hashes` 0.14.1 implementation. It reconstructs the coinbase, Merkle root and header from pool data.
- The verifier rejects duplicate, stale, unknown, low-difficulty, invalid-version and out-of-range submissions.
- The pool also supports the exact factor 3/2. Integer target division keeps the pool target distinct from the ASIC power-of-two filter.
- The Stratum V1 service changes difficulty and sends a clean job after a valid submission. It retains the wire messages and independent verdicts.
- Private inherited stream transport uses POSIX read/write. This environment denies socket bind, send and recv calls. No TCP, external worker or remote dashboard pass is claimed.

## Commands and results

- `cargo test -p md-sim-mining --all-targets --locked --offline`: 15 library/integration tests and one replay example test pass.
- `cargo fmt -p md-sim-mining`: pass.
- `cargo clippy -p md-sim-mining --all-targets --offline -- -D warnings`: pass.
- `python3 scripts/verify_bitcoin_fixture.py`: pass.

The tests include 256 varied complete headers with independent hashes, nonce boundary and sparse version searches, a real private stream client/server exchange with two fresh jobs, a clean transition, difficulty changes and rejection cases.
These are engine and protocol tests. They do not prove firmware execution or Bitcoin Core block acceptance.

## Checked archive evidence

The source artifact `open-bitaxeorg-esp-miner-source` has SHA-256 `b93d5fd28110e6dd7082346caccf8799340194391016ead9a179d18e91bfcd88`.
Its commit is `cd470d4df1944fb71b6c07d101a5fddade9ac02d`.
The member `components/asic/test/test_bm_job_packet_building.c` has SHA-256 `ed25932cb89359d4e9cdbf4067399ff7e803bb7872d3fb7eec9c6d5863e04800`.
Both the original archive and the extracted member were hashed. The member matches the saved extraction.
The BM1397 compression-state vector is now a Rust regression test. Its expected state words are little-endian in the published vector.
The Bitcoin Core genesis fixture remains a separate regression test.

## Fixes during review

- The archived BM1397 vector uses little-endian state words. The test now converts compression words to that declared representation.
- JSON parsing without round-trip float support changed a power-of-two fractional difficulty by two units in the last place. The crate now enables the parser's `float_roundtrip` feature. The wire test checks the exact difficulty.
- A lint finding for a needless reference was removed.
- Explicit message, submission and trace budgets now bound rejected traffic. Incomplete evidence is retained on failure.
- Inherited IPC now uses nonblocking I/O and checks poll error flags. A stalled-peer regression test prevents an unbounded write.
- The job's allowed rolled time now covers the configured mining budget plus a 120-second margin. The old fixed window could reject valid low-rate work. Budgets are bounded to one day and epoch overflow is rejected. A computed share at 300 seconds passes with a 600-second budget; work past that window fails.

## Current source and consensus work

The v10 source fixture uses the canonical workspace release build. It passed two fresh jobs at 10,000 and 1,000 H/s caps after the final time-window fix. Its anchored UBSAN run also passed. See `firmware-progress.md` for exact binaries and all preserved failures.
The reusable `replay_s2_trace` Rust example checks bounded trace files, packet CRCs and full headers independently. It confirms ASIC-valid results which the stricter pool target rejects. CRC reference and corruption checks pass.

The explicit regtest mode is implemented in `src/regtest.rs` and `src/stratum.rs`. The anchor records the exact genesis hash, height, bits, time and network magic. Constants match the archived Core chainparams and an independent hash of the regtest genesis header.
Anchored coinbases use canonical BIP34 `OP_1` at height one. Their entropy changes between jobs so the Merkle roots differ. The independent verifier revalidates each accepted share before block export. Export retains the exact header, coinbase, full block, import framing, source submission and hashes. It does not search for another nonce or claim node acceptance.
The default random-parent protocol suite remains separate. `firmware/v10-regtest` produced 25,718 actual hashes and two accepted anchored jobs. Its UBSAN counterpart produced 50,038 hashes and two accepted jobs. Independent replay of all five final source traces passes. Updated checksums include each replay report.
The frozen-source Core daemon accepted both actual firmware blocks from each anchored run. Each import changed a fresh chain from height zero to one. Evidence is in `bitcoin-core/v10-regtest-candidate-{0,1}/` and `bitcoin-core/v10-regtest-ubsan-candidate-{0,1}/`. The review checks exact input, source share, serial and pool traces, Core identity, artifact hashes and Core's own ordered tip logs. Exact block hashes are in `review.md`.
Core first initializes genesis, then imports the candidate in the same private temporary storage. The first direct-import assertion failure is retained. Assertions and consensus rules remain enabled. This is a source-fixture capability pass, not an exact-board or complete-release pass.

## Open gates and next action

- The source-derived 8-chain, 32-chip interface fixture is not an exact physical board or original firmware boot. All exact-target gates remain incomplete.
- Descriptor inheritance, runtime root writes, worker path checks, thread retention and child cleanup findings are fixed. Worker live logs, bounded result retrieval, explicit cleanup and transfer abort were also independently reviewed.
- Actual firmware block import passes for the source fixture through an isolated socket-free Core path. The later v11 check also proves process-restart persistence. No RPC, host-crash durability, external network or exact-board result is implied. The wrapper at `/usr/bin/bitcoin` did not prove a real daemon.
- Complete board boot, physical profile coverage, dashboard tests, remote transport and all-target mining acceptance remain open.
- The catalogue agent independently reviewed the mining crate. It found unbounded submission/trace growth and a blocking inherited write. Both are fixed. The reviewer re-ran all 11 tests and reported no remaining correctness finding in this component. See `work/simulator/review.md`.

## Canonical runtime quota follow-up

The runtime now bounds aggregate input bytes, input slots and retained run
evidence. Store recovery and lost-run status defects found in review are fixed.
All 20 runtime tests pass. See `store-progress.md` and `review.md`.

The new v11 firmware runs use canonical harness SHA-256
`90a5a39eb663994a1473385d1ccaad45a123b9c637189b001057f1a28721b1c3`.
Both C variants were rebuilt using only a captured, fully checked toolchain
tree. The resulting firmware bytes match the earlier variants. The changed
harness was still rerun for all five source fixture cases.

Independent serial replay passes for all five cases. The default case computed
6,005 hashes. The 1,000 H/s case computed 171. The anchored optimized case
computed 10,073; its UBSAN variant computed 14,071. The nonce-zero case computed
six. Each case has two independently valid actual firmware submissions.
The low-cap, anchored optimized and nonce-zero traces also contain ASIC-valid
results outside the stricter pool target which firmware correctly does not submit.
All old root-file hashes were checked before adding each replay result to its
evidence index. See `reports/v11-independent-replay.json`.

All four v11 Core imports and process restarts pass independent review. Each
exact firmware candidate changes a fresh chain from height zero to one, then a
third original Core process loads that same candidate from the private data
directory without a loadblock argument. Raw phase logs, all source-copy hashes,
report references and top-level evidence indexes were checked. See
`reports/v11-core-independent-review.json` for exact blocks and scope.
The v10 Core results remain preserved historical evidence. Original-board and
exact-target gates remain incomplete.

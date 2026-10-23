# Firmware execution checkpoint

The complete simulator is not finished. The working execution path is a source-adapted cgminer support-tool fixture. It uses the archived cgminer process and its S2 serial driver. The declared topology is eight chains with 32 chips per chain, from the source defaults. This is not an exact physical S2 board. It has no downstream FPGA model, individual ASIC bus model, calibrated power or thermal model, or native dashboard.

The current source fixture passes fresh work, independent header verification, a clean-job transition, a difficulty change, lower aggregate caps and a nonce-zero regression. Separate regtest mode produces full blocks from actual accepted firmware submissions. An unchanged Bitcoin Core source build accepted all four final blocks and preserved their exact tips across clean process restarts in isolated tests. Each test advanced a fresh regtest chain from genesis height zero to the submitted block at height one. The four blocks compete for height one; they are not a four-block chain. No original-board or complete catalogue gate is passed by this fixture.

## Current identities

| Input or build | SHA-256 |
| --- | --- |
| Archived `open-ckolivas-cgminer-source`, commit `b8491c66e7e22f23a9edf095dd1337ee581e88bd` | `4b3515ec072904e81e91182f5f8d5f8b5b87361ef2a7e2131643d8207badb996` |
| Recorded source patch | `1cc3f80bbd223e9c7fa2c2313a306ce388f9c3c076de8c81360fea4308594f4b` |
| Static adaptation manifest | `a15914f53d7e6beaca1d362258f83bea8beb157087249c88c5b2eaa35f08ad68` |
| Optimized cgminer binary | `88602d125e46310d0e61f64622d50be25df285085c2fe09c63070d852c64a90a` |
| UBSAN cgminer binary | `0b18f7f5aa273df38e204690883cf948b6d23be0e294085719b87b9ff6358326` |
| Tested release harness | `90a5a39eb663994a1473385d1ccaad45a123b9c637189b001057f1a28721b1c3` |

The original archive remains unchanged at `artifacts/files/source/open-ckolivas-cgminer-source/cgminer-b8491c66e7e2.tar.gz`. Preparation verifies its full hash, each declared original member hash, absence of each new member, and each patched member hash. The static input description is `simulator/firmware/cgminer-source-adaptation.json`. The patch is `simulator/firmware/cgminer-low-rate.patch`. Results are separate from these input identities.

The captured toolchain manifest is `e2eabd7bd16c22397c37cfbcfd0115223406f7850b5ad8f0733de6ac79e4d12a`. The frozen-preparation runner is `3a44a1066bc68239767e87d39fd3ddd6ba7e96b39965505a0f15d1cff20697d1`. The [closed-input report](firmware/toolchain-closure-report.json) gives the complete input set and provenance.

Current prepared bundles are `firmware/prepared-closed-002` and `firmware/prepared-closed-ubsan-002`. They contain the binary, build log, patch, copied adaptation manifest and input manifest. Each current run has `run-spec.json`, `execution-identity.json`, `input-manifest.json`, `adaptation-manifest.json`, locked executable/library inputs, guest logs, complete serial and pool traces, reconstructed headers and checksums. The specification hash covers the seed, epoch, cap, difficulty, mode, limits and all mounted input hashes. The harness build identity covers the linked model and independent verifier.

## Current test results

These are the final `v11` source runs. They use the current pool time-window fix and the canonical workspace release build. The workspace build identity is recorded in each run. The five executions used those canonical installed command bytes. Their preparation uses the separately pinned frozen-preparation runner. Every row has two independently accepted jobs and zero serial CRC errors.

| Run under `work/simulator/firmware/` | Cap H/s | Actual hashes | ASIC results | Scope |
| --- | ---: | ---: | ---: | --- |
| `v11-default` | 10000 | 6005 | 2 | Fresh random-parent jobs |
| `v11-low-cap` | 1000 | 171 | 3 | Fresh jobs; first accepted nonce is zero |
| `v11-regtest` | 10000 | 10073 | 3 | Two fresh blocks accepted by Core |
| `v11-regtest-ubsan` | 10000 | 14071 | 2 | Two fresh blocks accepted by Core; no UBSAN error |
| `v11-zero-nonce` | 100 | 6 | 3 | Fixed-seed regression; actual nonce zero accepted on two distinct jobs under UBSAN |

The first four runs obtain fresh random seeds. The low-cap, regtest and zero-nonce traces include computed results that pass the ASIC filter but fail the stricter pool target. The executing firmware does not submit those results. The last repeats the seed from a failed development case. `independent-asic-replay.json` in each run reconstructs every returned ASIC result with a complete-header oracle. It distinguishes the guest-programmed filter from the stricter pool target. All five replay checks pass. Their files are included in `evidence-checksums.json`.

Seven firmware library tests and four frozen-preparation tests pass. Formatting, the offline release build and strict Clippy pass. UBSAN is the C undefined behavior sanitizer. Every preparation compiles and executes `simulator/firmware/cgminer-regression.c` under UBSAN. That test uses the exact patched C helper header. It checks all 32 hash-prefix positions, an all-zero prefix, an old status reply after a clean job, and work sent after a status request. The entire UBSAN firmware also completes the source mining path. This C regression is needed to test the existing C interface and its alignment and bounds behavior.

## Bitcoin Core acceptance

The oracle is built from archived, unchanged Bitcoin Core commit `619334569b44b4278d0155bb8daf4c8535a47367`. Its binary SHA-256 is `a22238f0a1ea0b0a67a17032d2df0039e9d3ac946f2e86e0634ea47448692fae`. Each report links the source execution identity, accepted pool submission, complete header, block bytes and exact node input. All four tests use fresh private chain storage. Core first initializes genesis, then a second original Core process imports the submitted block. A third original Core process starts without the import option and reads the same height-one tip from saved chain state. All three processes run in the same isolated environment and private storage. Core assertions remain enabled. The logs show the exact expected hash as the active tip at height one, a clean stop, and the same tip after restart.

| Firmware run and submission | Accepted block hash | Node evidence |
| --- | --- | --- |
| `v11-regtest`, 0 | `000a2820d8400d95c5d26744cda1315a611ebcf27ae3cf082e80bb9de6f68086` | [Acceptance report](bitcoin-core/v11-regtest-candidate-0/block-acceptance.json) |
| `v11-regtest`, 1 | `000378e50038ece131dea055d18a4499f36b21866e7f432df3cd5d3b3d4b842a` | [Acceptance report](bitcoin-core/v11-regtest-candidate-1/block-acceptance.json) |
| `v11-regtest-ubsan`, 0 | `00039ab0149a9caf66c02e26208aaa5f44a8a9b7e0ffe957d69a40dc30cbedd9` | [Acceptance report](bitcoin-core/v11-regtest-ubsan-candidate-0/block-acceptance.json) |
| `v11-regtest-ubsan`, 1 | `0002995074f31c2caa9c9eb32b08f6a8182190c9e612531682f6d9fa5bd1ce2d` | [Acceptance report](bitcoin-core/v11-regtest-ubsan-candidate-1/block-acceptance.json) |

This is offline block import through Core's actual consensus path. It does not prove an RPC connection, network transport, crash or power-loss recovery, an original miner board boot, or complete exact-target coverage. The Core preparation also has an unfrozen installed toolchain dependency closure. Its pinned source and binary identities are in each `core-backend-identity.json`.

## Verified commands

Run from the repository root. Output directories must be new. The names below refer to retained tests; choose new names to repeat them.

```sh
cargo build --workspace --release --offline --locked
cargo test -p md-sim-firmware --offline --locked
cargo clippy -p md-sim-firmware --all-targets --offline --locked -- -D warnings
cargo build -p md-sim-firmware --example freeze_cgminer_toolchain --offline --locked
cargo test -p md-sim-firmware --example freeze_cgminer_toolchain --offline --locked
target/debug/examples/freeze_cgminer_toolchain capture work/simulator/firmware/toolchain-snapshot-003
target/debug/examples/freeze_cgminer_toolchain build . work/simulator/firmware/toolchain-snapshot-003 work/simulator/firmware/prepared-closed-002
target/debug/examples/freeze_cgminer_toolchain build . work/simulator/firmware/toolchain-snapshot-003 work/simulator/firmware/prepared-closed-ubsan-002 --ubsan
target/release/cgminer-lab run work/simulator/firmware/prepared-closed-002 work/simulator/firmware/v11-default
target/release/cgminer-lab run work/simulator/firmware/prepared-closed-002 work/simulator/firmware/v11-low-cap 1000
target/release/cgminer-lab run-regtest work/simulator/firmware/prepared-closed-002 work/simulator/firmware/v11-regtest
target/release/cgminer-lab run-regtest work/simulator/firmware/prepared-closed-ubsan-002 work/simulator/firmware/v11-regtest-ubsan
target/release/cgminer-lab run work/simulator/firmware/prepared-closed-ubsan-002 work/simulator/firmware/v11-zero-nonce 100 d6540ef1163e19f4d89b19a81f7493f0c71cdc02dbd6e12a432d96a59a573da1 1792425800 -31
cargo build -p md-sim-mining --example replay_s2_trace --offline --locked
target/debug/examples/replay_s2_trace work/simulator/firmware/v11-regtest
cargo build -p md-sim-firmware --example node_prerequisite_probe --offline --locked
target/debug/examples/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/v11-regtest-candidate-0 work/simulator/firmware/v11-regtest/regtest-blocks.json 0
target/debug/examples/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/v11-regtest-candidate-1 work/simulator/firmware/v11-regtest/regtest-blocks.json 1
target/debug/examples/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/v11-regtest-ubsan-candidate-0 work/simulator/firmware/v11-regtest-ubsan/regtest-blocks.json 0
target/debug/examples/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/v11-regtest-ubsan-candidate-1 work/simulator/firmware/v11-regtest-ubsan/regtest-blocks.json 1
```

Optional arguments after the output directory are `CAP_HPS SEED_HEX EPOCH DIFFICULTY_EXPONENT`. The default cap is 10000 H/s. The default ASIC exponent is -20 at that cap. It falls to -24 at 1000 H/s and stops at -31 for smaller caps. The second job uses exponent plus one. The pool difficulty is exactly 1.5 times each power of two.

`run-inputs.json` records expected device and accepted-share intervals from both exact targets. The mining timeout is five seconds plus twelve times the summed expected accepted-share intervals, with a 30 second minimum and a one-day process limit. A selected cap and difficulty that exceed the limit fail before execution. The allowed header time window follows that budget with bounded clock margin. Ordinary bus and boot timing stay separate.

## Isolation and adaptations

The baseline `cgminer-lab prepare` command uses the installed read-only `/usr` toolchain. The new closed-input preparation command uses only the captured, fully hashed `/usr` tree. Both use private namespaces and bounded temporary storage. The closed rebuilds include headers, libraries, tools and tool data, and produce the exact earlier firmware bytes. This resolves the missing toolchain closure for the declared cgminer recipe only. The separate Bitcoin Core preparation and other firmware still need their own frozen toolchain inputs.

The execution worker receives only hashed executable and library files and private temporary storage. It has process and resource limits. ELF inspection runs in a separate bounded namespace. An ELF interpreter must match a supported system loader path. Libraries must resolve within approved system library directories. Direct host preparation and execution entries reject an absent isolation contract.

The environment denies AF_INET and AF_NETLINK sockets, socket bind, and socket send/receive operations. Private inherited socketpair POSIX reads and writes work. The recorded source adaptation uses that private IPC path for unchanged Stratum messages. This does not test TCP, SSH, TLS, a native dashboard or a remote physical host.

The source adaptations also permit negative ASIC filter exponents, validate the programmed device target separately from the pool target, handle PTY backpressure, preserve a pending clean replacement, track status reply generations and spent queue credits, key duplicate checks by work identity, and fix original unaligned C accesses. Model readback describes actual state. No nonce, hash count, accepted share or boot message is generated without its corresponding computation or guest execution. The upstream guest's integer hash-rate display can round fractional difficulty to zero; it is separate from the simulator's actual computed-hash count.

## Retained failures and reviews

| Evidence | Failure and fix |
| --- | --- |
| `attempt-001` | Socket bind denied before firmware execution; explicit inherited private IPC adaptation added. |
| `attempt-002` | One accepted share, then partial PTY write corruption; bounded write retry added. |
| `attempt-003` | Old work filled the queue after a clean job; explicit clean replacement added. |
| `prepared-v1` | Global PAX metadata rejected; bounded metadata handling added. |
| `reviewed-001`, `reviewed-ubsan` | System loader and library path aliases rejected; only validated canonical system aliases are now accepted. |
| `reviewed-ubsan-002`, `final-ubsan` | UBSAN caught original unaligned nonce serialization and hash-display accesses; byte-preserving `memcpy` fixes added. |
| `final-zero-nonce` | A real early result exposed stale queue credits; sent-work and clean-generation tracking added. |

Independent reviews also found and resolved descriptor inheritance, broad direct-entry isolation, missing epoch and input identities, missing failed-run checksums, arbitrary ELF interpreter imports, pool resource limits, device-target versus pool-target checking, duplicate nonce handling, a hash-display boundary read, and a fixed time window at very low caps. Earlier successful runs are historical evidence. They are not reused for a different final execution identity.

## Remaining work and next action

The fixture does not resolve a physical board or provide complete device fault, persistence, update, recovery, cooling or power behavior. Missing-status timeout/retry recovery and serial wire timing remain implementation gaps. Complete per-target execution and mining, original firmware boot, remote transport, dashboards and the complete required matrix remain incomplete.

`firmware/original-board-prerequisites.json` retains freshly checked archive and member hashes for VirtualAxe, Espressif QEMU and ESP-Miner. VirtualAxe targets a different ESP-Miner commit from the collected current source. Its source adaptation does not satisfy the original-board gate. The installed QEMU lacks an ESP32/S3 machine; ESP-IDF, Xtensa ESP compilers, Meson and Ninja are absent. A pinned backend and dependency closure still require work. The separate original Zynq feasibility probe must retain its own limited scope.

Next action: prepare a fully pinned emulator and toolchain closure for an evidenced exact physical target. Resolve its complete controller, board, ASIC, storage, PSU and firmware identity before execution. Implement the missing board interfaces and original boot path, then repeat the fresh-work and Core acceptance tests for that exact target. The completed source fixture remains a separate support-tool capability.

## Toolchain closure work after v10

After v10, the preparation path gained a frozen build-input set. The standalone Rust example `crates/md-sim-firmware/examples/freeze_cgminer_toolchain.rs` captures installed package files under `/usr`, with a 1 GiB byte limit and a 60,000 entry limit. It expands the installed dependency providers, retains package metadata identities, hashes each regular file, and records links and directories. It excludes paths outside `/usr` and system installation defaults under `/usr/share/factory`. Excluded files are unavailable to the replay.

An isolated `strace` probe failed with `PTRACE_TRACEME` and `PTRACE_SEIZE` denied. This restriction is not bypassed. Package metadata therefore selects a conservative input set; it does not prove that the set is complete. A closed-input rebuild must supply that proof for the tested recipe.

Capture `toolchain-snapshot-001` stopped at the protected installation default `/usr/share/factory/etc/crypttab`. The failed attempt is recorded in `firmware/toolchain-snapshot-001/failure.json`. Its incomplete copied tools were removed; no archived input was changed. Capture `toolchain-snapshot-002` passed with the explicit factory exclusion. It contains 102 packages, 30,840 entries and 697,927,858 bytes. Its manifest SHA-256 is `e2eabd7bd16c22397c37cfbcfd0115223406f7850b5ad8f0733de6ac79e4d12a`. The optimized and UBSAN builds in `firmware/prepared-closed-001` and `firmware/prepared-closed-ubsan-001` both passed. Each had only the captured `/usr` tree available, produced the exact v10 firmware bytes, and passed the C regression. The snapshot hash verified again after each build.

Independent review found and fixed a root-symlink escape, unchecked special permission bits, an unbounded directory queue, and loss of partial export diagnostics on failure. The supervisor now kills its namespace on parent loss and checks the fork race. Four regressions and strict Clippy pass. Bounded hashing and copying now stop if an input grows beyond its declared size or the per-file 128 MiB limit. The final bounded capture `toolchain-snapshot-003` produced the identical manifest hash. Final reviewed replays `firmware/prepared-closed-002` and `firmware/prepared-closed-ubsan-002` both passed with complete diagnostics and unchanged firmware bytes. Five fresh `v11` source runs also passed with those prepared outputs and the new canonical workspace harness. All five independent trace replays passed. All four v11 blocks passed Core acceptance and clean-restart persistence checks. The current evidence tables above link the exact reports.

The [closed-input report](firmware/toolchain-closure-report.json) pins the snapshot, source and retained runner. It also clarifies the static adaptation manifest: `build_environment.isolation` describes the earlier baseline preparation. The actual later preparation environment is recorded in each closed output’s `preparation-inputs.json` and `input-manifest.json`. No earlier hashed input or result was edited. The scoped closure work is complete for this recipe. The next implementation step remains the exact physical target and original boot path described above. Original board, per-target, dashboard and remote gates remain open.

# Bitcoin Core execution checkpoint

Date: 19 October 2026. No per-target regtest mining gate passes at this checkpoint.

## Local and archived inputs

The installed `bitcoin-tx` split package is version `31.1-3`.
Its `/usr/bin/bitcoin` program is a command launcher. The package has transaction, wallet, utility and kernel tools, but no node daemon or RPC client.
The package file list was checked at `/var/lib/pacman/local/bitcoin-tx-31.1-3/files`.
The launcher hash is `64737f2618f54c9df6909a62df933c8a876eb9a3c759a142940bcf70707590f5`.
`/usr/lib/libbitcoinkernel.so` has hash `c4f3acff8a042c0d1eb20f275ee9b91027020fe29a1dba9537ea8ee88cf1de23`.
These tools are not evidence that a node is installed in PATH.

The audit checked `/usr/bin`, `/usr/lib`, `/usr/libexec`, `/usr/local/bin`, `/opt`, `/home/user/.local/bin`, `/home/user/.shared_bin`, the package cache, and the known `/home/user/src/bitcoin` workspace.
The last location contains real daemon candidates. Its current source identifies as SV2 Template Provider, so the directory name does not establish the binaries' source identities.

| Candidate | SHA-256 | Observed result |
| --- | --- | --- |
| `/home/user/src/bitcoin/bitcoind` | `e91640d84f5dbaaa0934831dc21e9e72434cec57359cb83cb75b877a81bf2794` | Reports Bitcoin Core v29.0.0 in strict isolation. Loads regtest genesis with network and RPC disabled. Exact source-build provenance is unresolved. |
| `/home/user/src/bitcoin/bin/bitcoind` | `2acd6778440eddff9f31095c18c8caf1173f20ae55490102bafb247e283d28cf` | Not executed. Static strings include an SV2 Template Provider. |
| `/home/user/src/bitcoin/build/bin/bitcoind` | `a960a3925526360c520d3f1c505b1b76b2b92e1bff4f0ceebdc990ee227ab1a4` | Not executed. |
| `/home/user/src/bitcoin/build/bin/bitcoin-node` | `e4098242b42aca2b74ac483f4ed97afd754508699500cbfb4878ff83c00ad0c3` | Not executed. |
| Guix output under `guix-build-aacfcca89b57/.../installed/bitcoin-aacfcca89b57/bin/bitcoind` | `98c378921aca3e5ca9c43832ba2284f2b849f5c812acbb1fe484e1e6201a4782` | Both isolated probes return only `Ok` and `Err`. No daemon identity or startup was established. A filename and exit zero are not a node test. |

The frozen catalogue also contains the complete source snapshot `source-bitcoin-core`.
It is `artifacts/files/source/source-bitcoin-core/bitcoin-619334569b44.tar.gz`.
Its 15,309,632 bytes match SHA-256 `776f4a708eb1a461872a6f1dba28218f0ed09af04239c29115412eecd3de1bc8`.
The source commit is `619334569b44b4278d0155bb8daf4c8535a47367`.
No new archive download or broad archive rescan was needed.

The isolated extraction of `src/kernel/chainparams.cpp` is saved at `bitcoin-core/source-evidence/chainparams.cpp`.
Its hash is `cfadf1478377c32625280520655b598674e6d487f08d7f7a80ac69252104b5e1`.
Lines 620–633 define regtest message magic `fa bf b5 da`, bits `0x207fffff`, and genesis hash `0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206`.

## Isolated execution and corrections

`crates/md-sim-firmware/examples/node_prerequisite_probe.rs` runs a closed ELF input set with the strict worker.
Each run retains the source path, file hash, dependency hashes, process specification, raw stdout and stderr, and result.
No acquired executable ran on the host. No host data directory or credential was mounted.

`local-daemon-probe-001` exposed two probe errors: the version call lacked a private data directory, and the runtime's 128-descriptor limit was below Core's requirement of 160. The runtime now has a bounded 256-descriptor limit. The version call uses `-version` and the private data directory.

`local-daemon-probe-002` established version v29.0.0. RPC startup failed because its HTTP server could not start in this environment. The socket-free import path reached genesis loading, but the 32 MiB scratch filesystem was below Core's free-space requirement.

`local-daemon-probe-003` increased scratch storage to 256 MiB. The import path loaded the block index and chainstate. It then aborted with an out-of-memory message under the 1 GiB virtual address limit and 15 script threads.

`local-daemon-probe-004` uses a 2 GiB address limit, 256 MiB scratch filesystem, 64 MiB file limit, and `-par=1`. It logged the exact regtest genesis `UpdateTip` at height zero, then `Stopping after block import` and `Shutdown: done`. It returned exit one because shutdown was requested during initialization. No error was logged. This is useful node startup evidence. It is not mining evidence.

The tested command was:

```sh
cargo run -p md-sim-firmware --example node_prerequisite_probe --locked --offline -- /home/user/src/bitcoin/bitcoind work/simulator/bitcoin-core/local-daemon-probe-004 offline
```

For a repeat run, use a new output directory. The source file also accepts a `regtest-blocks.json` path instead of `offline`. That mode checks imported block framing and hashes, links the exact accepted source submission, loads the block through Core, and requires the exact candidate hash at height one after the exact genesis tip.

## Pinned source build

The archived source configured successfully in private user, mount, PID and network namespaces. The toolchain directory was read-only. Only bounded temporary filesystems were writable. No missing dependency was reported for the selected node build with wallet, GUI, tests and IPC disabled.
The configure log is `bitcoin-core/source-configure-001/configure.log`.

The first build compiled 98 percent of the node target. The static library exceeded the 64 MiB per-file limit. `SIGXFSZ` stopped the link step. The exact failure is retained in `bitcoin-core/source-build-001/result.json` and `build.log`. No daemon was exported.

The second build used the same untouched source and `-O0 -g0`, with a 256 MiB per-file limit, 2 GiB build scratch filesystem, 4 GiB address limit, two compiler jobs and a 600-second wall limit. It completed with exit zero.
Its exact recipe and tool executable hashes are in `bitcoin-core/source-build-002/`.
The derived daemon is `bitcoin-core/source-build-002/bitcoind`, with 33,824,920 bytes and SHA-256 `a22238f0a1ea0b0a67a17032d2df0039e9d3ac946f2e86e0634ea47448692fae`.
The hash computed inside preparation matches the exported binary.
It reports Bitcoin Core `v32.99.0-g619334569b44b4278d0155bb8daf4c8535a47367`.
The backend registry is `simulator/backends/bitcoin-core-619334569b44.json`.
The preparation recipe, tool executable hashes and build log are separately hashed inputs. They are outside the repository source manifest. The read-only installed toolchain includes headers and libraries that are not all frozen. This establishes a source-derived daemon identity, not a fully reproducible toolchain release.
Final review found that `source-build-002/recipe.txt` contains escaped display quotes around the compiler flags. The actual build log records the correct `-O0 -g0` flags. The transcript is preserved. The corrected command in `source-build-003/recipe.txt` was executed in a separate bounded preparation environment. It completed with exit zero and produced exactly the same 33,824,920 daemon bytes and SHA-256 as build 002.
The tested repeat command was `bash work/simulator/bitcoin-core/source-build-003/recipe.txt`. The output directory was new. For another build, use another output directory and save its command separately.
The corrected recipe hash is `25baef31f568ab3f5428dbeb1b44afec5a43568b65d14d6e3f2f36e82203ac0a`.
The repeat build log hash is `be07c3248d945bb6b91bfb49d66087dacceabece294c82fb22ec4e9be41b909a`.
The tool checksum file hash is `3081782407fc9a9fdb22db5e6139401c9d2a95f13a54e35c3ecc5435733dc9b5`. All its checks passed.
The result is [source-build-003/result.json](bitcoin-core/source-build-003/result.json). This extra preparation evidence is outside the repository source manifest. It does not change the registry's accepted binary identity.

## Actual firmware-produced block acceptance

The first import into an uninitialized directory exposed an upstream assertion in `CheckBlockIndex`: `m_blockman.m_block_index.size() <= 1`.
The result is retained at `bitcoin-core/v8-regtest-candidate-0/`.
The probe now starts the unchanged Core binary twice in one isolated environment and one run-owned directory. The first process initializes and saves genesis. The second imports the selected firmware-produced block. All original assertions stay enabled.
The earlier `v8-two-phase-candidate-0` probe actually imported the block, but its result was false because the log check expected the older `Shutdown: done` spelling. That failed result is preserved.

The `v9-regtest-candidate-0` run passed. Core reported the exact genesis tip at height zero and then `0006b60838101c2e47c08fb3eb7852fdbdf61d8c752d07a71d709b09bc9660a4` at height one.
Both processes logged their stop after import and clean shutdown. The wrapper exited zero. No output was truncated.

The tested command was:

```sh
cargo run -p md-sim-firmware --example node_prerequisite_probe --locked --offline -- work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/v9-regtest-candidate-0 work/simulator/firmware/v9-regtest/regtest-blocks.json 0
```

The input check requires the registry's source-verified Core binary hash. It also checks the source run's saved checksums and copies those exact bytes before decisions. The independent pool must have accepted the indexed submission. Re-exporting that submission must reproduce the complete block record. The firmware-to-pool trace must contain the same job, extra nonce, timestamp and nonce. The source run must have configured the adapted device, computed actual hashes and exited successfully. The report retains source and node execution identities, header, coinbase, block, trace hashes and actual node logs.

The mining agent independently reviewed this path and its v9 evidence. No unresolved correctness finding remains in this limited import path. This uses no RPC. The chainstate is private temporary storage and is removed after the run. No post-import restart is claimed. This source fixture does not establish an exact physical board or the full target gate.

## Earlier canonical workspace run

All four v10 candidates passed against the pinned Core daemon. The source runs use the canonical workspace release harness `b94cb250323b30fc49204d979cdca11953a4e0c2d451ecf06e6de01e5856d3e8`.
Each candidate was imported into a separate fresh private chainstate. These are four height-one acceptance tests, not one four-block chain. Both jobs in each source run share the genesis parent and have distinct coinbases.

| Source run and candidate | Actual Core height-one block hash | Acceptance report |
| --- | --- | --- |
| Optimized v10, 0 | `000144c1560e2b36999009a239c0a38ce7a0a85cbfea3eaec1996dfa7d65d8d5` | [Report](bitcoin-core/v10-regtest-candidate-0/block-acceptance.json) |
| Optimized v10, 1 | `00034d074b16d166d483021f3ff069a3c42adc604b4def4f633199f01cc5f811` | [Report](bitcoin-core/v10-regtest-candidate-1/block-acceptance.json) |
| UBSAN v10, 0 | `0002a601f31eec6000d13f9454bdfdaa1c0486b9def99576a7d4865bf6c39a46` | [Report](bitcoin-core/v10-regtest-ubsan-candidate-0/block-acceptance.json) |
| UBSAN v10, 1 | `00005a45c2cfc8fe681384cc78441b166181c6baa6c0d3c88ab6cbbc131c6509` | [Report](bitcoin-core/v10-regtest-ubsan-candidate-1/block-acceptance.json) |

The direct command was tested for each source run and index. One exact invocation is:

```sh
target/debug/examples/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/v10-regtest-candidate-0 work/simulator/firmware/v10-regtest/regtest-blocks.json 0
```

The example was built with `cargo run -p md-sim-firmware --example node_prerequisite_probe --locked --offline -- ...` for the preceding v9 run. For another run, select a new output path. To test the other records, change the source directory, output directory and final index together.
All four wrappers exited zero. Core printed the exact genesis tip, exact selected height-one tip and two clean shutdowns. Source evidence is copied beside each result. The independent source replay is retained in each original v10 source run.
The mining agent checked all four final reports independently. It verified the source paths and submission indices, daemon hash in the process specification, ordered genesis and candidate tips, empty stderr, both stops and shutdowns, exit zero, and the hashes of the block input, stdout, source link, backend identity and process specification. No unresolved correctness finding remains in this limited import result.

This bounded Core preparation and import task is complete. RPC, remote-network, post-import restart and complete exact-target gates remain separate. The complete simulator remains incomplete. Next actions for that larger task are the unresolved original board input closure and models, full target execution, and remote and native dashboard gates recorded in the main progress file.

## Current closed-input firmware run and restart check

The v11 source runs use the canonical workspace harness `90a5a39eb663994a1473385d1ccaad45a123b9c637189b001057f1a28721b1c3` and the captured-toolchain firmware preparations. Each selected candidate passed a fresh three-process Core test. The unchanged pinned daemon first saves genesis, then imports the actual firmware-produced candidate, then starts again without `-loadblock` in the same private chain directory.
The third process must report `Loaded best chain: hashBestChain=<exact candidate> height=1`. This log contract was observed in the same pinned Core build when it loaded persisted genesis. The first two phases must report genesis and the selected candidate in the proper order. Each phase must report its own stop after import and clean shutdown. Two new regression tests reject missing or reordered phases, a different hash, an extended hash, a different height and missing or repeated shutdowns.

| Source run and candidate | Actual Core height-one block hash | Acceptance and restart report |
| --- | --- | --- |
| Optimized v11, 0 | `000a2820d8400d95c5d26744cda1315a611ebcf27ae3cf082e80bb9de6f68086` | [Report](bitcoin-core/v11-regtest-candidate-0/block-acceptance.json) |
| Optimized v11, 1 | `000378e50038ece131dea055d18a4499f36b21866e7f432df3cd5d3b3d4b842a` | [Report](bitcoin-core/v11-regtest-candidate-1/block-acceptance.json) |
| UBSAN v11, 0 | `00039ab0149a9caf66c02e26208aaa5f44a8a9b7e0ffe957d69a40dc30cbedd9` | [Report](bitcoin-core/v11-regtest-ubsan-candidate-0/block-acceptance.json) |
| UBSAN v11, 1 | `0002995074f31c2caa9c9eb32b08f6a8182190c9e612531682f6d9fa5bd1ce2d` | [Report](bitcoin-core/v11-regtest-ubsan-candidate-1/block-acceptance.json) |

The exact candidate-zero invocation was:

```sh
cargo run -p md-sim-firmware --example node_prerequisite_probe --locked --offline -- work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/v11-regtest-candidate-0 work/simulator/firmware/v11-regtest/regtest-blocks.json 0
```

The other three runs used the built `target/debug/examples/node_prerequisite_probe` with their respective source directory, new output directory and index. All four wrappers exited zero. All reports have `accepted: true` and `persisted_after_restart: true`. Raw logs show three distinct phases and three clean shutdowns. There is no output truncation.
The reviewed probe source hash is `c328e68a042bc3a66621f50b4603c836f31968127ef31fa5d803eddfe72b97bf`. Its two oracle tests and strict Clippy check pass. The mining agent independently reviewed the phase checks and all four final results with no unresolved correctness finding. It checked phase-local genesis, candidate and restarted candidate tips; each clean stop and shutdown; wrapper exit zero and empty stderr; exact source candidate records; all 11 linked source files; all acceptance references; and each output's evidence checksum index. The independent report is [v11-core-independent-review.json](reports/v11-core-independent-review.json).

This establishes process-restart persistence of each accepted candidate in run-owned temporary storage. It does not establish host-crash durability or retained storage after namespace cleanup. These are four independent height-one tests, not a four-block chain. The node used no RPC and did not generate these blocks. The fixture remains source-adapted, with no exact original-board or complete simulator gate claim.

# Fresh shared CLI test500

The complete shared CLI test passes. All six stages exit0: fresh preparation,
native browser, rate verification, mining verification and two Core nodes.
The [terminal assessment](../demos/s9-shared-500/assessment.json) records
`passed: true` and `full_specification_complete: false`.

This runs the original archived Braiins S9 firmware
`2022-04-29-0-5eb2c5fe-22.02.4-plus` in an adapted reference environment.
The reference topology has three chains of63 BM1387 chips and a10H/s
aggregate cap. It is not an exact physical target or an original-board pass.

The selected run is
`run-ef37c532ac0c1cc8ab3b207299b928eb0ae9878f8da8941a3b6ef994acfd5661`.
The CLI generates a new pool seed and job time on each invocation. Its
[fresh-work receipt](../demos/s9-shared-500/prepared/fresh-work.json) binds
the unchanged parent, derived inputs, helper and executed worker lock.
The parent archive and prepared494 inputs stay unchanged. The
[preparation review](../reviews/fresh-preparation-independent-498.json)
checks that only pool entropy/time and its process mount can change.

The [rate audit](../demos/s9-shared-500/run/execution/demo-rate-audit.json)
checks1,346 actual computations. The minimum admission interval is149,550,652ns,
above the100,000,000ns interval required by the10H/s cap. The
[mining audit](../demos/s9-shared-500/run/execution/demo-regtest-audit.json)
checks17 complete hardware replies and matches15 firmware submissions.
Nine shares pass; six fail for low difficulty. Two fresh jobs, a clean-job
transition and a difficulty change are verified. The
[independent raw-message review](../reviews/shared-cli-mining-independent-500.json)
rebuilds all15 headers, checks SHA-256d and pool targets, and joins the
executed job seed to the new preparation.

Both resulting blocks pass isolated Bitcoin Core acceptance. Each fresh chain
moves from height0 to1, then retains the exact tip after restart. Core generated
neither block. The [independent Core review](../reviews/shared-cli-core-independent-500.json)
checks raw node logs, exact daemon/block bytes and all source/input joins.

| Candidate | Block hash | Evidence |
| --- | --- | --- |
| 0 | `4a000d510095095c17ff97588abb679fcc614f43d69edbbe34de8f4cbe6f1b45` | [Core result](../demos/s9-shared-500/core-0/block-acceptance.json) |
| 1 | `2a2268ff02fd7801a3d7fba0f36700fe9b21a4035218bfbb94609664acf5c5cb` | [Core result](../demos/s9-shared-500/core-1/block-acceptance.json) |

The [native assessment](../demos/s9-shared-500/run/assessment.json) passes
complete result export and selected-run browser access. HTTP through SSH and
authenticated TLS use the real native service in the isolated client VM.
The worker and client VM share one execution host. The test endpoints are
`http://127.0.0.1:18090` and `https://192.0.2.1:9443` inside that client.
Both stop with the suite. This command does not provide a persistent host URL
or establish deployment on an external physical host.

The [independent access review](../reviews/native-browser-pass-500.json)
checks275 retained access files,46 runtime inputs, the exact573-byte shared
receipt and acknowledged cleanup. Both browser origins show real native DOM
and WebSocket work. Separate cookie stores reject the unauthenticated client
while retaining the primary session. Clock skew meets the unchanged5s bound.
Both15-operation API suites check configuration write/read/restoration. Both
browser groups are empty after cleanup; gateway and tunnel shutdown pass.

These commands ran from the repository root:

```sh
CARGO_TARGET_DIR=work/simulator/check-target-085 CARGO_INCREMENTAL=0 TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 cargo build -p md-sim --release --offline --locked -j2
work/simulator/cli-tools-500/md-sim demo s9-shared --check
work/simulator/cli-tools-500/md-sim demo s9-shared --out work/simulator/demos/s9-shared-500
```

The output path exists. Use a new output directory for another run. The
input check verifies3,579 workspace files and43 host files. This requires the
prepared workspace, archive links, pinned host tools, KVM, captured browser,
firmware, emulator and Core files. Building the CLI alone does not install
these dependencies. Clean-install packaging remains incomplete.

The [tested CLI build](../cli-tools-500/build.json),
[independent source review](../reviews/shared-cli-build-independent-500.json),
[input manifest](../../../simulator/demos/s9-shared.json) and
[helper provenance](../channel-target-499/cohort.json) pin the tested state.
Thirteen helpers comprise eleven unchanged tested494 builds and two reviewed
preparation/worker builds. This preserves the baseline support identities.
Formatting, strict workspace Clippy and1,069 tests pass, with zero failures
and49 declared ignored entries across118 suites. Ignored tests are not
execution passes. No simulator code changed after those checks.

Full exact-target coverage, original-board execution, physical timing,
clean installation and persistent host access remain open. The frozen matrix
still has79,599 rows and zero complete exact-target passes. Earlier failed
runs retain their failed verdicts.

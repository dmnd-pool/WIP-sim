# Adapted S9 development milestone

**Passed on 22 October 2026. The full simulator is incomplete.**

The complete demonstration took 33.72 minutes. The CLI and cycle both returned
exit code 0. All four firmware processes exited normally with complete traces.
The run used one adapted reference profile: three chains of 63 BM1387 chips.
It does not establish an exact physical S9 target. The frozen full matrix has
79,599 rows and zero full exact-target passes.

## Run and inputs

The [README](../../../README.md#adapted-s9-development-milestone) gives the
tested build, input-check and start commands. The output directory is
`work/simulator/demos/s9-review-284/`. Use a new directory to repeat the run.

- [CLI assessment](../demos/s9-review-284/assessment.json)
- [Complete cycle assessment](../demos/s9-review-284/run/assessment.json)
- [31 primary input identities](../demos/s9-review-284/input-manifest.json)
- [Invocation and CLI identity](../demos/s9-review-284/invocation.json)
- [Driver, firmware preparation and transitive input identities](../demos/s9-review-284/run/driver-input.json)
- [Saved tested CLI](../demos/s9-review-284/cli-binary)

The archived S9 SD baseline is Braiins OS+ 22.02.4, dated 29 April 2022.
The executing miner identifies itself as `bosminer-plus-am1-s9 0.9.0-5eb2c5fe`.
Original SPL, U-Boot and Linux execute before the native services and miner.
Their log messages are checked together with actual service, device, storage
and mining results. A log message alone cannot pass the cycle.

| Input | SHA-256 |
| --- | --- |
| Original SD image | `97a257571af4fb9e62f709e2d6b8f36a191fdd07f1cd12ae780bd1c73af7eaef` |
| SPL payload | `765efb23844156e976ba785c24a6a753c664504a499c64581d1c356a3f4c02df` |
| FIT image | `8b2a32020281aa67490c01633adf99c1a2ca6c9897e4c210bb6d0d001c15caca` |
| Selected expanded FPGA image | `980532de5eb666508dafb111466f30169a70b8b6ea662bdda61e17dbd55a4421` |
| Signed update archive | `469743ebcd5faf3445e4f4209ad8af58cc7adeff37c7df28af3c53a5b255b56e` |
| Executed backend263 | `a139b1c6afc70e993bd88d009e84d05af3a6d65c96dcd014ef6f6ea23f1a3db1` |
| Independent Core binary | `a22238f0a1ea0b0a67a17032d2df0039e9d3ac946f2e86e0634ea47448692fae` |
| Tested CLI | `0f36a5779b450ca838ed8b2c5f23835fc9bb2e02db15c6229f75fbdf2c624bd6` |

The selected FPGA image's identity does not establish model equivalence.
The separately tested backend280 is not substituted into this frozen run.

## Firmware mining and persistence

Each boot uses two fresh jobs, a clean-job transition and a difficulty change.
The firmware sends work through the simulated hardware interface. The engine
computes nonces and returns complete hardware responses. A separate verifier
reconstructs Bitcoin headers and checks SHA-256d, targets, work identity and
the recorded serial responses. Each audit retains the headers and blocks.

| Boot | Execution time | Computed hashes | Independently checked hardware results | Core acceptance and restart |
| --- | --- | --- | --- | --- |
| [Provision](../demos/s9-review-284/run/provision/regtest-audit.json) | 245,604ms | 112,737 | 94 | [Block 0](../demos/s9-review-284/run/core-provision-0/block-acceptance.json), [block 1](../demos/s9-review-284/run/core-provision-1/block-acceptance.json) |
| [Write and signed update](../demos/s9-review-284/run/run-0/regtest-audit.json) | 308,030ms | 5,119 | 3 | [Block 0](../demos/s9-review-284/run/core-0-0/block-acceptance.json), [block 1](../demos/s9-review-284/run/core-0-1/block-acceptance.json) |
| [Readback and restoration](../demos/s9-review-284/run/run-1/regtest-audit.json) | 370,555ms | 114,988 | 117 | [Block 0](../demos/s9-review-284/run/core-1-0/block-acceptance.json), [block 1](../demos/s9-review-284/run/core-1-1/block-acceptance.json) |
| [Restored settings after restart](../demos/s9-review-284/run/run-2/regtest-audit.json) | 247,579ms | 117,759 | 105 | [Block 0](../demos/s9-review-284/run/core-2-0/block-acceptance.json), [block 1](../demos/s9-review-284/run/core-2-1/block-acceptance.json) |

All eight block hashes differ. Eight separate Core nodes accept them. Each
node changes height from 0 to 1 and retains height 1 after restart. The blocks
come from firmware submissions; Core does not generate them for this test.

The cap is 1,000 hashes per second. The fixed four-bit ASIC filter override,
private pool difficulty, reference sensors, power interfaces, clock policy,
native boot configuration and monitor adaptation remain explicit. See each
boot's `adaptation.json`, `console.json`, `pool-input.json` and `rate-audit.json`.
Original archived files remain unchanged. The
[phase record](../demos/s9-review-284/run/progress.json) links saved storage,
filesystem inspection, configuration changes, restoration and restart checks.

## Native dashboard and access boundary

The [browser evidence](../demos/s9-review-284/run/run-1/browser-result.json)
comes from Firefox inside the disconnected support VM. It uses the native
service at `http://10.0.2.15`. Login, archived assets, live page tables,
WebSockets and same-origin cookie isolation pass. The page shows three boards,
63 chips per board, the configured 100MHz frequency and accepted private-pool
work. The subscription returns two different sample timestamps. The native
API checks write, readback, restoration and persistence across both restarts.

The command provides no host or remote URL. Host network listeners are denied
in this environment. The selected local263 and SSH264 native dashboard tests
failed HTTP readiness. Their separate mining results do not close the remote
dashboard gate. This topology is not an external physical host test.

## Checks, review and remaining work

Formatting, strict Clippy, all 575 Rust tests in 101 test binaries, release
builds and guide links pass after the last code fix. Logs are in
`work/simulator/checks-2026-10-21-001/` with suffix284. The
[author review](../reviews/adapted-s9-milestone-283.md) records the tested fabric
clock/reset changes, failed baselines, sanitizer checks and the corrected
transport-result retention bug.
The [final source manifest](source-285.json) verifies 3,644 source and vendor
files. The [source checkpoint](../qemu-s9/core-checkpoint-285/manifest.json)
retains 412 project source files and the built Rust model library. Both were
verified after the final code and README changes.

[Demo280](../demos/s9-review-280/assessment.json) remains failed. Its native
WebSocket deadline expired while other builds and tests ran. Demo284 used
unchanged inputs, deadlines and assertions, with no other test or build load
during the browser phase. Resource contention is a possible cause, not a
proven diagnosis. Run the cycle after other heavy checks finish.

Exact FPGA source/build equivalence or validated reconstruction, exact physical
component models, selected remote dashboard access, clean firmware package
installation and full-matrix conformance remain open. Independent review is
blocked by the agent usage limit. The author review does not replace it.

The user requires a stop at this milestone for review and approval before the
next milestone. See [the milestone record](../milestones.md) for acceptance
criteria, blockers and next actions. No full-specification pass is granted.

# Archived S9 mining and selected SSH/TLS access pass in run 441

Run 441 passes native HTTP and authenticated TLS checks for the selected
archived S9 firmware process. Check 443 independently verifies fresh mining
and two Bitcoin Core block acceptance/restart results from that same run.
This is an adapted S9 reference result. Milestone 2 and the full simulator
remain incomplete. No browser was requested.

## Tested command and prerequisites

This exact command ran from the repository root and exited zero. It requires
the prepared workspace, archived inputs, pinned kernel, backend 328, prepared
440 input store and frozen 440 helpers. The output directory now exists. Use
a new output directory to repeat the run; preserve the other frozen paths.
This is not a clean-install command or a persistent dashboard server.

```sh
work/simulator/channel-target-440/release/examples/worker_channel_probe \
  work/simulator/channel-target-440/release/md-sim-worker-ipc \
  work/simulator/qemu-s9/backend-build-328-pool \
  work/simulator/network-vm/native-access-441 \
  prepared=work/simulator/qemu-s9/native-access-prepared-440 \
  ssh-helper=work/simulator/channel-target-440/release/examples/remote_namespace_check
```

The native process ran for 305.154 seconds. The complete outer run took
767.202 seconds, about 12 minutes 47 seconds. Both exited zero without trace
truncation. The [final assessment](../network-vm/native-access-441/assessment.json)
passes. Firmware limits remained 360 seconds wall time, 330 CPU seconds and
3 GiB address space. The functional hash cap was 10 H/s across all three
chains. The independent pool ran for 300 seconds, with a five-second
observation margin. Ordinary device, boot and network timing was not scaled
with the hash cap.

## Topology and actual access

The native worker and ARM miner emulator ran beside an isolated support
VM on the same execution host. OpenSSH connected separate client and server
network namespaces inside that VM. The server hosted the independent pool
and the dashboard bridge. A private Ethernet carrier connected it to the
miner's real virtual NIC. The client had no direct miner-network or external
network route. This was not an external physical-host deployment.

During the test, the separate client used `http://127.0.0.1:18090/` through
its SSH tunnel and `https://192.0.2.1:9443/` through the authenticated TLS
gateway. These addresses belonged to the isolated test environment. They
are not host-browser URLs and are no longer live after cleanup. The
[publication](../network-vm/native-access-441/execution/shared-access/client/publication.json)
binds both access paths to this actual selected run:

`run-1c963695c1adf0903873ff423ff2f3c4be65537d03241ecd445b4fcef88c3268`

The canonical complete lock hash is
`09e19cc31d326fbee7f9d88c8cbaeca636f390d0fc03106850b011fbeea4f5a9`.
The selected native process identity is
`555e6e020940c890b8542dce5ef92b639809cbdc8704e3a791b3b688fddef97b`
(8,303 canonical bytes). Provider leases, worker lookup, endpoint records
and all TLS response headers name that same run. A listener or the worker's
generic `booting` state was not treated as firmware readiness.

The [HTTP report](../network-vm/native-access-441/execution/shared-access/client/http/report.json)
and [TLS report](../network-vm/native-access-441/execution/shared-access/client/tls/report.json)
each contain 15 successful checked exchanges. They establish native login,
invalid-credential rejection, session extension, original assets, live API
telemetry, configuration write/readback/restoration, logout and rejection of
the expired session. A client without a TLS certificate was also rejected.
HTTP and TLS used separate fresh cookie stores and distinct native sessions.

The native service reported version `2022-04-29-0-5eb2c5fe-22.02.4-plus`.
Its original application asset was 425,409 bytes, SHA-256
`1c318483df06a03e460e35f7419d8562c007f5dc4965a20d412c96392ad36423`,
through both paths. Uptime advanced from 70 to 74 seconds over HTTP and from
88 to 91 over TLS. Each test changed the web theme from null to DARK, read
it back, and restored null. These are one-boot API checks, not browser,
WebSocket or restart-persistence results.

The actual [readiness record](../network-vm/native-access-441/execution/shared-access/client/gateway/listener-readiness.json)
contains four refused connections before success at 2,737 ms. Readiness
finished at 2,744 ms within the existing 30-second startup budget. The native
HTTP/TLS checks then ran. The earlier [failed run 431](shared-access-failure-431.md)
remains a failure with its original evidence.

## Exact firmware, models and retained results

The [original bundle](../original-zynq/sd-bundle-291/boot/bundle.json) identifies
archive artifact `braiins-feed-4366569e66ab54bc0d15`. Actual logs contain
archived SPL, U-Boot, Linux and Boser service execution. The reference model
has three chains of 63 BM1387 chips. It does not establish exact physical
controller, hashboard, PSU, cooling or storage compatibility.

| Input | SHA-256 | Bytes |
| --- | --- | ---: |
| Original SD image | `97a257571af4fb9e62f709e2d6b8f36a191fdd07f1cd12ae780bd1c73af7eaef` | 112,197,632 |
| Native backend 343 | `be5b22536955634b5c0e9609d000d0e0c982c5d1707dc39b3cc8b1b564c49274` | 20,189,720 |
| Support backend 328 | `2cebd1b0f94f98bd1bc1a4a9465dd9744f20201f4cadf8816ec7b2a79fe0812f` | 19,570,240 |
| Native worker | `aa82dd61cd9e4d4b92351539f4a437f3f084175075a6844956cc4e6db5bf4f29` | 2,483,944 |
| Support helper | `f489c0669d00cad5f3eaec4e2e44881e5fcf784b7c0d2e5d95c590a5291593b9` | 6,119,664 |
| Independent pool helper | `85c4476e8bee02947e1e9fba49970f56fc5f50ee6f4882a29a47395d484aa1ef` | 2,156,784 |
| Independent mining auditor | `94863054eba47f15f7abb548d412b6f7627591102acd6b9850700358c7e5b601` | 3,084,752 |
| Bitcoin Core daemon | `a22238f0a1ea0b0a67a17032d2df0039e9d3ac946f2e86e0634ea47448692fae` | 33,824,920 |

The [frozen executable hashes](../channel-target-440/SHA256SUMS),
[475-file source checkpoint](../core-checkpoint-440/manifest.json),
[complete locked run](../network-vm/native-access-441/execution/locked-run.json),
[transferred inputs](../network-vm/native-access-441/ssh-inputs.json), and
[support proof](../network-vm/native-access-441/execution/shared-pool/support-proof.json)
pin the remaining firmware, model, kernel, policy and helper inputs.

The [adaptation manifest](../network-vm/native-access-441/execution/adaptation.json)
records the direct SPL loader and secondary-CPU park, unvalidated legacy FPGA
logic correspondence, explicit sensor/fan/PIC reference fixtures, and the
changed native monitor script. The fixed ASIC leading-zero condition is
reduced from 32 bits to one bit for low-rate work. Guest ticket-mask writes,
readback and filtering remain applied. Nonces and shares remain actual
computed results. The downloaded SD image is unchanged; adapted code and
environment identities are separate. Boot ROM execution, calibrated physical
timing and original-board equivalence remain incomplete.

Before cleanup, the host retained these exact objects:

| Result | SHA-256 | Bytes |
| --- | --- | ---: |
| Native process result | `c3faec35da051fb3e14f86eecdae4507f68ec62bc4e6046fc9904de16f872b7d` | 105,932,004 |
| Independent pool bundle | `aa66b980a4ace342f683918caeaa00bf29bcdf1c29870981c44bf226d86f53eb` | 93,953 |
| Access bundle | `38260f925109ec16f08718921283e34fb235f85017afde601ea28c59a06a55ef` | 15,921,402 |

The [joined receipt](../network-vm/native-access-441/execution/shared-pool/retention-receipt.json),
runtime stdin receipt, worker acknowledgment and cleanup record agree.
Independent review rehashed both native copies, all 260 access files and ten
pool-session files. Gateway, tunnel and carrier shutdown were observed.

## Fresh mining and Bitcoin Core checks

The [rate audit](../network-vm/native-access-441/execution/rate-audit443.json)
checks 1,687 computations against the shared 10 H/s admission budget. The
[mining audit](../network-vm/native-access-441/execution/regtest-audit443.json)
correlates seven firmware submissions with actual computed hardware results.
An independent replay reconstructs all seven Bitcoin headers from raw
Stratum work and recomputes SHA-256d. It checks coinbase, merkle root, nonce,
version mask and targets. Two fresh jobs, a clean-job transition and a
difficulty change from 2^-31 to 2^-30 are present. Session 0 has five accepted
shares and two LowDifficulty rejections.

Two distinct blocks from those jobs were imported into separate isolated
Core nodes. Each fresh chain moved from height 0 to 1. A clean restart without
the import argument loaded the same height-1 block. Core generated neither
candidate. These are offline import/restart checks; RPC and host-crash
durability are not claimed.

| Candidate | Accepted and restarted block hash | Evidence |
| --- | --- | --- |
| 0 | `756bad500232f33432d1fb307b29420adf3c0177f6ba1b2443c3b3869b467a63` | [Core 0](../bitcoin-core/shared441-candidate0-443/block-acceptance.json) |
| 1 | `21c7823c03e5097c15eb5d705793371216fc3d4dc109ea3fc0679965d000d916` | [Core 1](../bitcoin-core/shared441-candidate1-443/block-acceptance.json) |

These four commands ran successfully after run 441. Their outputs now exist.
Use new report/output paths for another audit, and keep the selected source
run consistent throughout.

```sh
work/simulator/channel-target-440/release/examples/s9_asic_audit rates-shared \
  work/simulator/network-vm/native-access-441/execution \
  work/simulator/network-vm/native-access-441/execution/rate-audit443.json
work/simulator/channel-target-440/release/examples/s9_asic_audit regtest-shared \
  work/simulator/network-vm/native-access-441/execution \
  work/simulator/network-vm/native-access-441/execution/regtest-audit443.json
work/simulator/channel-target-440/release/examples/node_prerequisite_probe \
  work/simulator/bitcoin-core/source-build-002/bitcoind \
  work/simulator/bitcoin-core/shared441-candidate0-443 shared \
  work/simulator/network-vm/native-access-441/execution/regtest-audit443.json 0
work/simulator/channel-target-440/release/examples/node_prerequisite_probe \
  work/simulator/bitcoin-core/source-build-002/bitcoind \
  work/simulator/bitcoin-core/shared441-candidate1-443 shared \
  work/simulator/network-vm/native-access-441/execution/regtest-audit443.json 1
```

## Review and remaining gates

The [independent access/retention review](../reviews/native-access-retention-independent-441.md),
[access input manifest](../reviews/native-access-retention-independent-441-inputs.json),
[independent mining/Core review](../reviews/native-mining-core-links-independent-443.json)
and [mining/Core input manifest](../reviews/native-mining-core-links-independent-443-inputs.json)
bind these results to the same execution. Frozen 440 passed formatting,
strict workspace Clippy, release builds and 863 test executions. Those checks
are component evidence, not full-target acceptance.

Browser rendering, WebSockets, uploads, restart persistence and client-loss
survival remain untested on this execution identity. Session 0 does not
establish duplicate, stale, invalid or out-of-range share rejection. Separate
component negatives cannot replace those target gates. External physical-host
deployment, clean installation, exact hardware/FPGA identities and calibration,
the original-board gate and complete matrix remain open. The frozen matrix
still has 79,599 rows and zero full exact-target passes.

The later browser attempt 445 failed before VM launch because the probe
selected the base image instead of the declared combined browser image.
Its [failure review](../reviews/native-browser-preparation-failure-445.md)
retains the exact cause and hashes. It grants no browser result and does not
replace this successful no-browser run.

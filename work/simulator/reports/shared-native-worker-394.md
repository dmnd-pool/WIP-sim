# Archived S9 execution through SSH and a separate pool

Run 391 passes its execution, artifact retention, independent mining audit and
two Bitcoin Core acceptance checks. This is an adapted S9 reference result.
Milestone 2 and the full simulator remain incomplete.

The native worker ran archived Braiins S9 firmware beside a disconnected
support VM. OpenSSH connected distinct client and server namespaces inside
that VM. The server owned the independent pool and its private Ethernet
connection to the miner. This used one physical host. It was not a test on
an external physical host. No native dashboard URL was published.

## Exact inputs and results

- [Frozen executable hashes](../channel-target-390/SHA256SUMS) and
  [459-file source checkpoint](../core-checkpoint-390/manifest.json).
- [Original bundle identities](../original-zynq/sd-bundle-291/boot/bundle.json),
  [prepared process](../qemu-s9/native-shared-prepared-390/process-spec.json),
  [complete locked run](../network-vm/native-worker-firmware-391/execution/locked-run.json),
  and [adaptations](../network-vm/native-worker-firmware-391/execution/adaptation.json).
- [Support image and component proof](../network-vm/native-worker-firmware-391/execution/shared-pool/support-proof.json),
  [provider receipt](../network-vm/native-worker-firmware-391/execution/shared-pool/provider-receipt.json),
  and [overall assessment](../network-vm/native-worker-firmware-391/assessment.json).

The reference hardware has three chains of 63 BM1387 chips. Backend 343 uses
the checked configuration transfer model. The selected FPGA logic and exact
physical components remain unresolved. The original SD input is
`97a257571af4fb9e62f709e2d6b8f36a191fdd07f1cd12ae780bd1c73af7eaef`
(112,197,632 bytes). The archive remains unchanged.

The native process kept its wall limit of 360 seconds and cap of 10 H/s.
Its actual duration was 305.143 seconds. The complete outer run took 748.209
seconds. The client transferred 48 objects containing 168,074,593 bytes before
execution. The actual pool readiness preceded the selected ExecuteToken.

The exact native result is 105,424,186 bytes, SHA-256
`152ff586fe592184fb801a051fa89d0033a966f01cc0f763d24af84d754f08c2`.
The pool bundle is 105,365 bytes, SHA-256
`0c5dcd05f8bf9fad864e69b2419264a0f555548a292d669805d3e025f2410d4c`.
The host saved both before it acknowledged cleanup. Host, worker and joined
receipts agree. The provider and runtime recorded identical bytes in both
directions, with no pending bytes. Ten real pool sessions completed without
cancellation. These facts are checked in the
[independent execution review](../reviews/shared-execution-independent-private-channel-391.md).

## Mining evidence

The [independent audit](../network-vm/native-worker-firmware-391/execution/regtest-audit392.json)
recomputed 1,351 hashes. It checked two fresh jobs, a clean-job transition,
a difficulty change, 14 serial results and 13 matching computations for the
firmware submissions. The [rate audit](../network-vm/native-worker-firmware-391/execution/rate-audit392.json)
checks the actual monotonic grants against the 10 H/s cap. Bus time remains
separate. The filter adaptation is recorded in the run manifest.

Separate isolated Core nodes accepted these candidates and retained them
after restart. Each chain changed from height 0 to 1 and remained at 1. Core
did not generate either block.

| Candidate | Block hash | Evidence |
| --- | --- | --- |
| 0 | `04217364004c16bcc760830065147f2cc4adc7596be7a75424d68cc0270a356e` | [Core0](../bitcoin-core/shared391-candidate0-392/block-acceptance.json) |
| 1 | `24e3fdba0328dc7387b162eb916fa28d0ca679ef0e8de0646768ee53331de4d7` | [Core1](../bitcoin-core/shared391-candidate1-392/block-acceptance.json) |

## Tested commands

Run from the repository root. Use a new output directory for another run.
Keep the prepared input store and frozen binaries from checkpoint 390 available.

```sh
work/simulator/channel-target-390/release/examples/worker_channel_probe \
  work/simulator/channel-target-390/release/md-sim-worker-ipc \
  work/simulator/qemu-s9/backend-build-328-pool \
  work/simulator/network-vm/native-worker-firmware-391 \
  prepared=work/simulator/qemu-s9/native-shared-prepared-390 \
  ssh-helper=work/simulator/channel-target-390/release/examples/remote_namespace_check
```

The separate audit and Core commands are in the
[executed recipe](../reviews/shared-core-handoff-390.md). They now have
results at the exact paths listed there. The recipe's earlier pending label
records its state before execution; this report records the later result.

## Limits and subsequent fixes

Frozen checkpoint 390 passed formatting, workspace strict Clippy and release builds.
The earlier workspace test 388 passed 792 test executions. Final changed
helper tests passed 79, pool tests 17, and node tests 10. Ignored child helper
entries remain explicit in the test logs. These are component checks, not
full target coverage.

Review after freeze found that root could block while opening a substituted
broker control FIFO. Current source 393 fixes those reads and symbolic-link
handling, and adds bounded diagnostic progress. Its 82 helper tests and
strict Clippy pass. The [follow-up review](../reviews/shared-control-independent-private-channel-393.md)
is closed. Run 391 does not test that later binary; a fresh integration is
required. The frozen command above retains this known limitation.

Selected native dashboard SSH/TLS access, exact FPGA and board identities,
physical timing, full per-target suites, clean installation and the complete
matrix remain open. No exact physical target passes all gates. The frozen
matrix still contains 79,599 rows with zero full exact-target passes.

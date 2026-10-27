# Recovered S9 mining evidence461 and Core acceptance466

The strict mining audit and two isolated Core acceptance/restart checks pass
for the retained original run453. This is recovery of existing evidence.
No firmware was run again. Its browser startup and gateway shutdown
acknowledgment failures remain failures. Milestone2 is incomplete.

## Evidence and results

The [original run review](../reviews/native-browser-retention-independent-453.md)
pins the archived firmware, adapted reference hardware, actual execution and
original failed access verdicts. The selected run is
`run-d7b13b55416780e0443c175955b8ec9dc008b888a58bcb8bedd330e6b5315cc4`.

The [recovery manifest](../network-vm/native-browser-453-recovered-461/recovery.json)
records43 copied files, totaling1,038,030,428 bytes. The source remains
unchanged. Every copy has a separate inode. The filesystem used bounded
stream copies. The retainer added only derived observations in a new root.
The acknowledged original assessment, access bundle and failed client report
were copied without changes. The existing strict verifier was not weakened.

The [rate audit](../network-vm/native-browser-453-recovered-461/execution/rate-audit461.json)
checks1,349 actual computations across all three chains at an aggregate cap
of10H/s. The minimum observed grant interval is131,057,633ns, above the
required100,000,000ns. This measures the software admission limit. It does not
calibrate physical timing or production hash rate.

The [submission audit](../network-vm/native-browser-453-recovered-461/execution/regtest-audit461.json)
matches12 actual firmware submissions to12 computed hardware results and
complete serial replies. It reconstructs the Bitcoin headers and checks the
programmed/adapted ASIC filters and pool targets. There are two fresh jobs,
a clean-job transition and a difficulty change. Independent replay finds
six accepted shares and six LowDifficulty rejections.

Two firmware-produced blocks were imported into separate fresh Core nodes.
Each chain changed from height0 to1 and retained the same height1 tip after
restart without a block-import argument. Core generated neither block.

| Candidate | Block hash | Core evidence |
| --- | --- | --- |
| 0 | `1c5d5a21017e0d57efd5a80fde5a88f965b65dcd1d97352361538927bdf0aded` | [acceptance and restart](../bitcoin-core/recovered453-candidate0-466/block-acceptance.json) |
| 1 | `06024c0b03f2cb77c1040fcdb38fc05c15013f9f48980aad7f7503ab917403e3` | [acceptance and restart](../bitcoin-core/recovered453-candidate1-466/block-acceptance.json) |

The [independent copy check](../reviews/shared-recovery-copy-independent-461.json)
rehashes all source and destination files. The
[independent mining review](../reviews/shared-recovery-mining-independent-461.json)
reconstructs all12 headers from the raw Stratum stream and checks both block
candidates. The [source review](../reviews/shared-recovery-independent-461.md)
covers bounded copying and preservation of the failed access verdict.
The [complete independent review](../reviews/shared-recovery-complete-independent-466.md)
also checks both isolated Core acceptance and restart runs against the
retained candidates and exact daemon identity.

## Commands that ran

These commands ran from the repository root. Their output paths now exist.
Use new output paths to repeat them and keep each audit bound to its selected
recovery directory. The prepared workspace and original453 evidence are
required. This is not a clean-install procedure.

```sh
work/simulator/recovery-tools-461/s9_retain_shared_evidence plan work/simulator/network-vm/native-browser-453/execution work/simulator/reports/shared-recovery-plan461.json
work/simulator/recovery-tools-461/s9_retain_shared_evidence retain work/simulator/network-vm/native-browser-453/execution work/simulator/network-vm/native-browser-453-recovered-461
work/simulator/recovery-tools-461/s9_asic_audit rates-shared work/simulator/network-vm/native-browser-453-recovered-461/execution work/simulator/network-vm/native-browser-453-recovered-461/execution/rate-audit461.json
work/simulator/recovery-tools-461/s9_asic_audit regtest-shared work/simulator/network-vm/native-browser-453-recovered-461/execution work/simulator/network-vm/native-browser-453-recovered-461/execution/regtest-audit461.json
work/simulator/core-tools-466/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/recovered453-candidate0-466 shared work/simulator/network-vm/native-browser-453-recovered-461/execution/regtest-audit461.json 0
work/simulator/core-tools-466/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/recovered453-candidate1-466 shared work/simulator/network-vm/native-browser-453-recovered-461/execution/regtest-audit461.json 1
```

[Recovery/auditor identities](../recovery-tools-461/SHA256SUMS) and
[Core-helper identities](../core-tools-466/SHA256SUMS) pin these tools. The
Core helper retained the same frozen461 auditor. It independently reran the
strict session audit before each block import.

## Failures and limits

Core attempts461 failed before executable inspection. Attempts464 passed
inspection but failed before node startup. Both failures came from setting
the small process limit before creating the user namespace. The host UID
already had more processes than that limit. Their outputs remain retained.
The fixes apply the same parser or guest limit after trusted namespace setup.
CPU, memory, network, filesystem and other isolation controls stay in place.
The current runtime also records the `prlimit` tool identity. Real root and
nonroot guest tests check the declared limit and successful child execution.

Historical frozen helpers keep their original bytes and old launch behavior.
They can encounter the old host-process-count defect if run directly in the
current environment. Use the explicit refreshed Core cohort above. A new
selected browser run needs its own refreshed cohort and complete acceptance.

Recovery461 grants no new browser, original-board, physical calibration,
external-host or exact-target result. Core checks use private temporary
storage and offline import. They do not establish RPC or host-crash durability.

# Recovered492 mining and Core checks

The mining audit and both Bitcoin Core acceptance/restart checks pass for
original run492. Its browser test failed and remains failed. These checks use
retained execution evidence. They do not run the firmware again or establish
an exact physical S9 target.

The selected run is
`run-219a3defb1256e5ca2570d24ce61ca3e4f3a80a7ecd1ea0b61ed7d5be5f56ad3`.
It executed the original Braiins S9 SD image in the declared adapted reference
model, with three chains of63 BM1387 chips and an aggregate10H/s cap. The
[execution input freeze](../reviews/native-browser-freeze-491.json) pins its
complete source, firmware, helper and prepared-input identities.

The [rate audit](../network-vm/native-browser-492/execution/rate-audit492.json)
checks1,660 real computations. The minimum grant interval is100,009,933ns,
above the100,000,000ns limit. The
[submission audit](../network-vm/native-browser-492-recovered-492/execution/regtest-audit492.json)
correlates12 firmware submissions with actual work packets, computations and
complete serial responses. Six pass the pool target; six are rejected for
low difficulty. Two fresh jobs, a clean-job transition and a difficulty
change are verified.

The [independent header review](../reviews/shared-recovery-mining-core-independent-492.json)
reconstructs all12 headers from raw Stratum messages and checks SHA-256d.
Both candidate blocks changed a separate isolated Core chain from height0
to1, then retained the same tip after restart. Core generated neither block.
The [raw Core review](../reviews/shared-recovery-core-raw-independent-492.json)
checks exact daemon and block bytes, all input mappings and actual Core logs.

| Candidate | Block hash | Core evidence |
| --- | --- | --- |
| 0 | `773aa66b02faa19d37ce6b10f99f17b603b8ed208cfff1e906c81f0248c70c18` | [acceptance/restart](../bitcoin-core/recovered492-candidate0-492/block-acceptance.json) |
| 1 | `2cc76361005c7562e83603b76de887ceec2fe3fb59245a796e294f0af7d64c95` | [acceptance/restart](../bitcoin-core/recovered492-candidate1-492/block-acceptance.json) |

The direct submission audit first failed because failed access prevented
publication of the derived pool-result file. The existing strict recovery
tool copied43 evidence files to a new root and reconstructed that pool join.
It changed no original bytes or verdict. The
[independent copy check](../reviews/shared-recovery-copy-independent-492.json)
checks all1,030,844,658 copied bytes against their originals.

These commands ran from the repository root. Output paths now exist; use new
paths for another recovery or audit. The prepared workspace is required.

```sh
work/simulator/channel-target-491/release/examples/s9_asic_audit rates-shared work/simulator/network-vm/native-browser-492/execution work/simulator/network-vm/native-browser-492/execution/rate-audit492.json
work/simulator/recovery-tools-492/s9_retain_shared_evidence plan work/simulator/network-vm/native-browser-492/execution work/simulator/reports/shared-recovery-plan492.json
work/simulator/recovery-tools-492/s9_retain_shared_evidence retain work/simulator/network-vm/native-browser-492/execution work/simulator/network-vm/native-browser-492-recovered-492
work/simulator/channel-target-491/release/examples/s9_asic_audit regtest-shared work/simulator/network-vm/native-browser-492-recovered-492/execution work/simulator/network-vm/native-browser-492-recovered-492/execution/regtest-audit492.json
work/simulator/channel-target-491/release/examples/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/recovered492-candidate0-492 shared work/simulator/network-vm/native-browser-492-recovered-492/execution/regtest-audit492.json 0
work/simulator/channel-target-491/release/examples/node_prerequisite_probe work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/recovered492-candidate1-492 shared work/simulator/network-vm/native-browser-492-recovered-492/execution/regtest-audit492.json 1
```

The [failed access review](../reviews/native-browser-failure-492.json) preserves
the browser failure, the passed HTTP/TLS API checks, exact retention receipt
and acknowledged cleanup. The browser sampled zero accepted work before the
first actual accepted share. Clock and fresh-profile checks were not reached.
The next run uses the existing real DOM readiness gate before sampling. It
keeps the same deadlines and assertions. No browser pass is inferred here.

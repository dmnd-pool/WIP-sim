# Independent PCAP migration review

Reviewed `support/pcap_migration_qtest.rs`, its `qemu_ttc_reset.rs` integration,
and saved run `qemu-s9/pcap-migration-346`. No concrete correctness defect was
found in this device-test path. This review did not author either module and
did not rerun the backend.

The positive control uses separate QEMU sender and receiver processes with
normal `migrate` and `migrate-incoming` operations. The receiver starts in
`inmigrate`. It imports the distinct SW_ID value and the changed INT_STS value
from the actual saved stream. It does not load a hand-written state fixture.

Strict outgoing migration must fail with the pre-save hook error. Strict
incoming migration must fail with the pre-load hook error. The test uses
`exit-on-error:false` so it can inspect the failed receiver. Both strict cases
check unchanged SW_ID and INT_STS after rejection. Generic input state remains
unchanged. The test retains QMP commands, responses, state bytes, state hashes
and all 17 MMIO transactions.

The outer driver requires the exact backend build hash, successful isolated
process exit, untruncated traces, one result marker, 17 transactions and two
strict-mode diagnostic messages. The run has a 20-second wall limit and
separate process, memory, file and trace limits. Blocking QMP/qtest reads are
therefore bounded by the enclosing run, rather than individual read deadlines.

Independent checks matched the saved spec/result hashes and decoded migration
state hash. The observed migration outcomes are completed/completed for the
generic pair and failed/failed for the strict pair. Both checked source hashes
match the source recorded in the assessment:

| Source | SHA-256 |
| --- | --- |
| `support/pcap_migration_qtest.rs` | `8647ecd184fddaaa74f185e3e08a0ac3d9af639f92b201c851f15ce89b61c1f6` |
| `qemu_ttc_reset.rs` | `d30e9496bcb51c820cf2d218a9df4c8bd0c8cc4a5274d54429cbd3efb31a9d63` |

[Assessment](../qemu-s9/pcap-migration-346/assessment.json),
[execution result](../qemu-s9/pcap-migration-346/result.json),
[transactions](../qemu-s9/pcap-migration-346/transactions.log).

Scope remains DevC device-state migration in a qtest-only machine. This is not
a firmware, complete S9 board migration, physical FPGA, security-equivalence,
mining or exact-target pass. The C implementation received a separate review;
this report does not expand that review's scope.

# PCAP follow-up source review 345

Date: 2026-10-23. Reviewer: `exact_target_gap/pcap_migration`.
The reviewer did not author the C changes or the unsupported-DMA and UNLOCK
tests. The reviewer authored the separate migration test module. That module
is excluded from this independent review.

No actionable finding was reported for the reviewed scope:

- The strict transfer path rejects unsupported destinations before memory
  reads. All strict DMA requests reach that path. The generic path is unchanged.
- System reset restores register access only for strict PCAP. It clears the
  port state and updates the interrupt. This is a stated reference policy.
- The strict migration guard runs on save and before loading device fields.
  Existing whole-board migration restrictions remain separate.
- The unsupported-DMA test checks empty and valid sources with the wrong
  destination, sticky failure, reset and complete transfer: 151 transactions.
- The UNLOCK test checks inaccessible registers, ignored writes, an actual
  system reset, restored state and complete transfer: 131 transactions.

The test machine has one Cortex-A9 object for qtest address lookup. Its
`qtest_enabled()` check tests the selected accelerator's flag. The qtest
server alone does not set that flag. The pinned qtest accelerator uses a
dummy CPU thread that handles events and waits. It does not execute guest
instructions. Thus this fixture does not provide a firmware execution path.

Reviewed patch SHA-256:
`e90cd612c70536b8c054a2aae91cef1c9fcaa87e195bdc89cfc5b145489a0cdc`.

Derived C SHA-256:
`864a58b1070e35ab93b9bf31bfda6df326254fa9e58ed61adaee72f135112505`.

All following members are from archived QEMU revision
`11661d857254b27fca5568c6deacf4f6b6f3460f`, archive SHA-256
`2a40e6705e59fd8807012631bc6b2c0877f10481b12768f78e4f9d7b88376cce`.

| Member | SHA-256 |
| --- | --- |
| `migration/vmstate.c` | `2ab692d07e65c186382803acb3e02ce5310eee7ddef5386fec134f9dc1a868b2` |
| `migration/savevm.c` | `5b81e57252e6df32cf303f5ba3cbb06caa2d7c1020a10f95789c0c53fe85cd97` |
| `include/system/qtest.h` | `f78604bd4b96814cdb2d2be205ad8d50fcec08d36d3a197577851dfee905962b` |
| `accel/qtest/qtest.c` | `9166ffb57ab61b6ccdf9766f5952c072be67f08825a0e9cdf9ebc5d5994257f0` |
| `accel/accel-system.c` | `cfb7a73338a014e3c0f5c6dbd636db5855de0e2098f3a69a8d758662f90fd57e` |
| `accel/dummy-cpus.c` | `084d2a7fa954ee7934555376e3e4b07c46b6d037f508bf993dc548b062bce522` |

No build or runtime test was performed by the reviewer during this source
review. Compiled results belong to the separate numbered test reports.

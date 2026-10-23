# Original boot SD persistence

The boot runner can retain the SD card's changed sectors in a separate
qcow2 image. Qcow2 is QEMU's disk image format. The archived SD image stays
read-only. The changed-sector image has a virtual capacity of 128 MiB.
Its snapshot records the exact backing image, compressed bytes, decoded
bytes and selected process specification.

The runner uses `cache=writethrough`. The pinned QEMU SD frontend calls
`blk_pwrite` without a flush flag. With the default write-back cache,
stopping QEMU can lose cached image writes even after the guest runs `sync`.
Disabling write caching makes the block backend add forced-unit-access
writes. The relevant captured sources are in
[the source inspection](../../../work/simulator/qemu-sd-source-001/references.json).

| Source member | SHA-256 |
| --- | --- |
| `hw/sd/sd.c` | `057427f10ea5f6ff9fb48f769230cf19b21e9d5f97ad3bf78afbc7f8fa78f657` |
| `block/block-backend.c` | `8bae44d4cad82762558afc79fb5a478d6460b158caac3ff985161f3638e245f5` |
| `block/qcow2-cache.c` | `2307ee169a1b00780bb0af86f92487977a1de4ddd20b898c62670dced5f5020e` |

[Cycle 001](../../../work/simulator/qemu-s9/original-sd-cycle-001/assessment.json)
fails with the original write-back setting. The first guest writes a fresh
marker, but the next guest cannot read it. An
[independent disk inspection](../../../work/simulator/qemu-s9/sd-inspection-003/inspection.json)
also finds that the first saved image lacks the marker and changed setting.
The failure and its images remain available.

[Cycle 002](../../../work/simulator/qemu-s9/original-sd-cycle-002/assessment.json)
passes with write-through mode. Three new emulator processes execute the
original SPL, U-Boot and kernel. The native UCI tool changes a setting from
100 to a fresh value of 953. The second boot reads 953 and restores the
original file. The third boot reads 100 and verifies the original file hash.
Both the restored and original files have SHA-256
`302128daf55df98b268cc7296326a8fd33786747da9b3ca58762730aa675b077`.

The independent reader uses captured `qemu-img` and 7zip programs in a
separate preparation namespace. It checks the actual partition and file
bytes. It confirms the
[changed state](../../../work/simulator/qemu-s9/sd-inspection-001/inspection.json)
and the [restored state](../../../work/simulator/qemu-s9/sd-inspection-002/inspection.json).
It does not execute firmware. QEMU `img_dd` applies `count` before `skip`;
the reader checks the resulting partition length explicitly.

These commands were tested. Use a new output directory to repeat them.

```sh
target/release/examples/s9_boot_storage_cycle \
  work/simulator/original-zynq/sd-bundle-006 \
  /tmp/md-sim-qemu-backend-066-ubsan \
  work/simulator/qemu-s9/original-sd-cycle-002
target/release/examples/s9_sd_snapshot_inspect \
  work/simulator/qemu-s9/original-sd-cycle-002/run/sd-snapshot-0.json \
  work/simulator/qemu-s9/sd-inspection-001
```

The `save-sd` boot option exports a snapshot after emulator process
termination. On a successful run, native guest `sync` completes first.
Failed runs can retain an unsynced image for diagnosis or recovery.
Snapshot schema 2 records this observation as `guest_sync_completed`.
Schema 1 did not record it. In particular, the scope text in failed run
031 incorrectly says that sync completed; its failed execution does not
support that statement. Its original evidence is retained unchanged.
The `sd-state=SNAPSHOT_JSON` option checks
and loads a saved snapshot into a new private run. Up to two
`restart-console=SCRIPT` options select additional boot phases. Each phase
uses a new emulator with the same private SD overlay. The current restart
cycle does not accept a local-pool selection.

The following separate run loads the changed state, reads its fresh value
through native UCI, then restores the original file. Its
[execution result](../../../work/simulator/qemu-s9/original-sd-restore-003/result.json)
passes. A [second separate run](../../../work/simulator/qemu-s9/original-sd-restore-004/result.json)
loads the restored state and verifies 100.

```sh
target/release/examples/s9_boot_bundle boot \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/original-sd-restore-003 spl sd2022-v1 \
  backend=/tmp/md-sim-qemu-backend-066-ubsan budget=180 save-sd \
  sd-state=work/simulator/qemu-s9/original-sd-cycle-002/run/sd-snapshot-0.json \
  console=work/simulator/qemu-s9/original-sd-cycle-002/console-1.sh
```

This tests SD persistence in the recorded reference environment. NAND and
volatile devices reset. It does not establish full-board state retention,
host-crash durability, original BootROM execution, update/recovery behavior,
or a physical storage revision. The native management and full target suites
still require separate evidence.

The combined mining and SD run 033 completes in 237.272 seconds without
trace truncation. Its
[audit](../../../work/simulator/qemu-s9/sd2022-private-pool-033/regtest-audit-001.json)
checks two fresh firmware submissions and 267,118 actual hashes. Both
candidates pass the separate Core node tests. The command below was tested;
use a new output directory to repeat it. The independent audit and Core
commands are described in the README.

```sh
target/release/examples/s9_boot_bundle boot \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/sd2022-private-pool-033 spl sd2022-v1 \
  backend=/tmp/md-sim-qemu-backend-066-ubsan budget=360 \
  presence-pin-fixture=e0 fan-fixture pic-contract asic-contract asic-packets \
  local-pool pool-vm=/tmp/md-sim-qemu-pool-003-ubsan temperature-fixture \
  adapt-fixed-filter-bits=4 pool-difficulty=-28,-27 regtest-genesis save-sd \
  console=work/simulator/qemu-s9/console-private-pool-014.sh
```

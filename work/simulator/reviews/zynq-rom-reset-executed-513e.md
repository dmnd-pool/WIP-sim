# Public Zynq ROM reset components 513e

The ROM mapping components pass. The CPU reset path remains incomplete.

The exact UBSan backend 513d passes three isolated cases with the frozen 513e helper:

| Case | Actual transactions | Actual resets | Result |
| --- | ---: | ---: | --- |
| Complete 128 KiB ROM protection and all 16 OCM RAM_HI settings |340|2|Pass |
| Same checks plus rejected private SWAP and unchanged ROM/OCM state |383|2|Pass |
| Existing fabric clock/reset regression |128|2|Pass |

The tests write distinct words at both ends of each 64 KiB bank. They also write distinct values at the old low addresses after moving banks high. The high bank values stay unchanged. Moving banks back preserves the original values. Thus a stale duplicate alias cannot pass. ROM checks read all original bytes and reject writes at four locations. The negative SWAP test observes the actual QMP internal-error stop and unchanged state.

[The independent audit](zynq-rom-map-independent-execution-513e.json) checks the complete transactions against original ROM bytes and an independent mapping oracle. [The input manifest](zynq-rom-reset-executed-513e-inputs.json) binds the helpers, backend, raw streams and results. The frozen source maps match before and after builds. The backend was built with undefined-behavior sanitization. No sanitizer diagnostic occurred in these component runs.

The actual CPU diagnostic creates a private 128 MiB qcow2 overlay over the original read-only 112197632-byte SD image. Zero extension occurs only beyond the complete original image. It does not change BOOT.BIN, FAT allocation or file EOF behavior. The overlay capacity is a reference storage assumption. The actual creation record is labelled `MD_SIM_ROM_STORAGE_CREATED`; it is tool evidence, not a guest boot message. QEMU uses an explicit 64 MiB translation cache within the unchanged 1 GiB process limit.

That run stopped after 185 ms at the startup UART chardev event. No instruction translation line was retained. Two SDHCI configured-capability messages and two disconnected GEM notices also appear before the ROM-map trace. They remain raw evidence. The next classifier must bind each message to its exact source and require QEMU's explicit running-state boundary before permitting instruction records. Unsupported guest accesses must still stop the run.

Earlier attempts stay failed.513c exposed the ROM owner-type integration error.513d completed the inner mapping assertions but the outer checker applied the TTC interrupt count; it also exposed QEMU's default 1 GiB cache allocation.513e corrects those defects without changing the hardware assertions or increasing limits.

The public ROM is not bound to the selected S9 silicon revision. Private SWAP, ROM_SHADOW, complete reset-to-firmware, physical timing and mining acceptance remain open. These component results do not close the original-board gate.

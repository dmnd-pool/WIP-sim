# Original ROM reset reason check

The reset-reason component tests pass. The original public ROM now reaches
the next missing register, GPIOB_CFG_LVTTL at0xf8000b10. It has not booted the
selected S9 firmware. The exact S9 silicon and public ROM match is unknown.

The unchanged original ROM is131,072bytes, SHA-256
`c58e2bc1d837654457ed47064accb26c4fb48307737e60d6c2d05253e7ee3aea`.
The reset-state model uses the archived 2012 manufacturer register table and
AMD Answer52030. The latter has conflicting summary bit labels. The original
sources and this conflict remain recorded.

The actual isolated QEMU build passed with undefined-behavior checks in
186,101ms. Its limits stayed900s wall time,120s CPU time per process and3GiB
address space. The final component runs check:

| Test | Bus transactions | Actual resets | Result |
| --- | ---: | ---: | --- |
| Reset reason, clear, read-only fields and status persistence |44|4|Pass|
| Original ROM protection and all OCM bank mappings |340|2|Pass|
| Unsupported private SWAP rejection |383|2|Pass|
| Existing fabric clock and reset behavior |128|2|Pass|

Two reset-reason resets come from actual PSS software-reset writes. Two come
from QMP, the emulator control interface. QMP resets add no invented physical
cause. The component checks POR at construction, accumulated SLC cause,
clear-zero/one, reserved-bit behavior and retained REBOOT_STATUS.

The first reason run remains failed. Its inner assertions passed, but an old
result field still reported two resets. The corrected helper counts the four
retained RESET events. It keeps the required count and every bus assertion.
Both helper freezes retain604 source files with equal before/after hashes.
Focused tests, strict Clippy, formatting and release builds pass.

These commands were tested from the repository root. They require the saved
backend and frozen helper. Use a new output path for another run.

```sh
work/simulator/qemu-s9/rom-reset-driver-513h2/qemu_ttc_reset work/simulator/qemu-s9/backend-build-513h-ubsan work/simulator/qemu-s9/rom-reason-513h2 rom-reason
work/simulator/qemu-s9/rom-reset-driver-513h2/qemu_ttc_reset work/simulator/qemu-s9/backend-build-513h-ubsan work/simulator/qemu-s9/rom-map-513h2 rom-map
work/simulator/qemu-s9/rom-reset-driver-513h2/qemu_ttc_reset work/simulator/qemu-s9/backend-build-513h-ubsan work/simulator/qemu-s9/rom-swap-reject-513h2 rom-swap-reject
work/simulator/qemu-s9/rom-reset-driver-513h2/qemu_ttc_reset work/simulator/qemu-s9/backend-build-513h-ubsan work/simulator/qemu-s9/rom-fabric-513h2 fabric
```

The separate CPU diagnostic stopped at its first unsupported access after
207ms. It retained169 translated-block address headers and a complete trace.
These headers are not an instruction execution count. The trace has no prior
reset-clear access error. Boot, original-board and mining results remain false.

See the [exact input and result identities](zynq-reset-reason-executed-513h2-inputs.json),
[primary-source archive checks](zynq-reset-reference-checks-513h.json),
[root source review](zynq-reset-reason-independent-source-513h-root.json),
[second source review](zynq-reset-reason-independent-source-513h-cleanup.json)
and [metadata correction review](zynq-reset-count-independent-source-513h2.json).
The [independent execution audit](zynq-reset-reason-independent-execution-513h2.json)
passes. It checks both604-source freezes, the actual component observations
and all180 original-ROM raw spans (2,736bytes).

Partial and unaligned rejection branches remain untested. Watchdog, external
and debug reset causes remain unimplemented. No migration, physical timing,
electrical behavior or complete S9 boot acceptance is claimed. Private SWAP,
ROM_SHADOW and later boot interfaces remain open.

# Original ROM LVTTL register check

The LVTTL register component tests pass. The unchanged public ROM now reaches
the next missing interface: PS GPIO INT_DIS_0 at `0xe000a214`. It has not booted
the selected S9 firmware. The public ROM's exact S9 silicon match is unknown.

The model stores the documented lower 28 register bits. It preserves the SLCR
unlock rule. The test checks the original ROM value `0x0c301166`, ignored upper
bits, clear, all lower bits, and two actual resets. It does not model electrical
thresholds or propagation delays. Default non-ROM mode remains unchanged.

The isolated QEMU build with undefined-behavior checks passed in 186,484 ms.
Its limits stayed at 900 seconds wall time, 120 seconds CPU time per process,
and 3 GiB address space. The final component runs all passed:

| Test | Bus transactions | Actual resets |
| --- | ---: | ---: |
| LVTTL storage, masks and reset | 20 | 2 |
| Reset reason and persistence | 44 | 4 |
| ROM protection and OCM bank maps | 340 | 2 |
| Unsupported private SWAP rejection | 383 | 2 |
| Existing fabric clock and reset | 128 | 2 |

These commands were tested from the repository root. They require the retained
backend and frozen helper. Use a new output directory for another run.

```sh
work/simulator/qemu-s9/rom-reset-driver-513i/qemu_ttc_reset work/simulator/qemu-s9/backend-build-513i-ubsan work/simulator/qemu-s9/rom-lvttl-513i rom-lvttl
work/simulator/qemu-s9/rom-reset-driver-513i/s9_rom_reset_probe work/simulator/qemu-s9/backend-build-513i-ubsan artifacts/files/source/source-zynq-bootrom-3f64d2f/bootrom/zynq_bootrom.bin artifacts/files/firmware/braiins-feed-4366569e66ab54bc0d15/braiins-os_am1-s9_sd_2022-04-29-0-5eb2c5fe-22.02.4-plus.img work/simulator/qemu-s9/rom-reset-513i
```

The CPU diagnostic correctly returns failure. It stopped on the unsupported
GPIO write after 207 ms. It retained 168 translated-block address headers.
These are not instruction execution counts. The trace shows the original ROM
forming address `0xe000a214`, value `0xffffffff`, and calling its store routine.

Fifteen focused tests, strict Clippy, formatting and release builds passed.
The helper freeze records 607 equal before/after source identities. The
[independent execution audit](zynq-lvttl-independent-execution-513i.json) checks
1,473 retained identities and all 179 original-ROM spans, totaling 2,728 bytes.
See the [exact inputs and results](zynq-lvttl-executed-513i-inputs.json),
[root source review](zynq-lvttl-independent-source-513i-root.json), and
[all-bits vector review](zynq-lvttl-allbits-independent-source-513i.json).

Partial and unaligned rejection branches remain untested. Private SWAP,
ROM_SHADOW, GPIO and later boot interfaces remain open. No original-board,
physical timing, electrical, firmware boot or mining gate passes here.

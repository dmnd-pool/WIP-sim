# Original ROM PS GPIO check

The PS GPIO component tests pass. The unchanged public ROM now completes its
four GPIO interrupt-disable writes and stops at the next unsupported SLCR
register, `0xf8000500`. The selected S9 firmware has not booted from reset.
The public ROM's exact S9 silicon match remains unknown.

The new Rust core implements four banks with widths 32, 22, 32 and 32.
It keeps output latches separate from external input samples. It preserves
unknown state, masked writes, five interrupt modes, shared interrupt reduction,
pending masks and write-one-to-clear behavior. A small C binding connects this
state to the existing QEMU CPU, bus, clock and interrupt interfaces.

The existing Xilinx C model was checked first. A bank profile alone could not
correct its input/latch, interrupt and reset behavior. Its exact original files
remain archived. See the [independent reuse assessment](ps-gpio-reuse-independent-513j.json).

The actual isolated QEMU build passed with undefined-behavior checks in
186,108 ms. Compiler stderr was empty and output was complete. Limits stayed
at 900 seconds wall time, 120 seconds CPU time per process and 3 GiB address
space. The binary is 20,208,792 bytes, SHA-256
`cc311dd5fab5233f696110e4eb1d22c4efb4fd18121e705513016edf5ea854ba`.

| Actual component run | Transactions | Resets | Result |
| --- | ---: | ---: | --- |
| PS GPIO data, interrupts and retention | 144 | 2 | Pass |
| PS GPIO rejection checks | 19 | 0 | Pass |
| LVTTL register | 20 | 2 | Pass |
| Reset reason | 44 | 4 | Pass |
| ROM and OCM mapping | 340 | 2 | Pass |
| Unsupported private SWAP | 383 | 2 | Pass |
| Fabric clocks and reset | 128 | 2 | Pass |

The positive GPIO run observes 14 real transitions at GIC SPI input20. It
uses the manual's `0xbdef4210` masked-write vector. It checks independent input
and output data, all interrupt modes, shared interrupt behavior, bus reset,
clock gate restoration and warm-reset retention.

The rejection run retains nine exact faults. These cover unknown reads, an
absent bank, partial, unaligned and doubleword accesses, held bus reset and
clock gating. DIRM and OEN remain unchanged after rejected writes. The first
fault stops the VM. Later faults are tested while it remains stopped; this is
not nine separate running-to-stopped transitions. No error read counts as a
successful zero-valued device read.

These commands were tested from the repository root. They need the saved
backend and frozen helper. Use a new output directory for another run.

```sh
work/simulator/qemu-s9/rom-reset-driver-513j/qemu_ttc_reset work/simulator/qemu-s9/backend-build-513j-ubsan work/simulator/qemu-s9/rom-gpio-513j rom-gpio
work/simulator/qemu-s9/rom-reset-driver-513j/qemu_ttc_reset work/simulator/qemu-s9/backend-build-513j-ubsan work/simulator/qemu-s9/rom-gpio-reject-513j rom-gpio-reject
work/simulator/qemu-s9/rom-reset-driver-513j/s9_rom_reset_probe work/simulator/qemu-s9/backend-build-513j-ubsan artifacts/files/source/source-zynq-bootrom-3f64d2f/bootrom/zynq_bootrom.bin artifacts/files/firmware/braiins-feed-4366569e66ab54bc0d15/braiins-os_am1-s9_sd_2022-04-29-0-5eb2c5fe-22.02.4-plus.img work/simulator/qemu-s9/rom-reset-513j
```

The last command correctly returns failure. The original ROM run stopped
after 205 ms at the first unsupported SLCR write, trace byte offset8604.
It retained 169 translated-block address headers without truncation. These
headers are not an instruction execution count.

Seven Rust core tests, one ABI test, 16 helper tests, strict Clippy, formatting
and release builds pass. The helper freeze retains 617 equal before/after
source files. The fresh model checkpoint retains 619 source files. The
[independent execution audit](ps-gpio-independent-execution-513j.json) passes.
It checks the actual per-pin state and IRQ positions, all rejection records,
build inputs, frozen sources and 2,736 original ROM bytes. The CPU probe did
not enable the GPIO bus trace; its evidence remains distinct from the component
bus trace. The exact [inputs and results](ps-gpio-executed-513j-inputs.json)
and [source review](ps-gpio-binding-independent-source-513j-cleanup.json) are saved.

Review found that the new build stage also ran for the x86 pool support target.
That target would fail the ARM source checks. The frozen ARM build above is
unchanged. A separately reviewed current-source fix skips the complete ARM
stage before accessing any ARM file or patch tool. Its regression checks an
absent ARM tree for x86, and missing or changed parent files for ARM. Three
affected tests, strict Clippy, formatting and release pass. The corrected
builder freeze retains620 equal before/after source files. This
does not claim a new complete x86 backend build.

MIO mux, straps, pulls, exact S9 wiring and physical timing remain unresolved.
No pad level is inferred from an output latch. Synchronizer timing and
migration remain unsupported. Private SWAP, ROM_SHADOW and later boot
interfaces remain open. No firmware boot, original-board or mining gate passes
from these component results.

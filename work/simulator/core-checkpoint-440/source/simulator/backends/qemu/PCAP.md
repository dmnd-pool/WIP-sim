# Checked FPGA configuration transfer

The Zynq processor configuration access port (PCAP) moves configuration words
from guest memory to the FPGA. The pinned upstream QEMU source reports success
for destination `0xffffffff` without reading that memory. It also sets the
configuration-done flag after interrupt-register writes. That behavior cannot
establish a valid FPGA programming result.

The optional `md-bitstream` and `md-part` device properties select a strict
reference transfer contract. Both files must be captured, hashed run inputs.
The Rust state validates the container, configuration packets, silicon ID,
every frame address, padding word and frame error code before execution.
It then checks each word that QEMU reads from guest DMA memory. DMA words use
little-endian byte order. The full payload or its exact suffix from the sync
word is accepted. The archived U-Boot and Linux drivers use these two forms.
No arbitrary prefix or trailing data is ignored.

The programming reset clears all received words and completion state. The
enable transition arms a new transfer. Each bounded DMA read must match the
selected image. The configuration-done interrupt is set only after the whole
image arrives. Interrupt mask and status writes cannot create completion.
The trace records source addresses, byte counts, omitted preamble size and a
SHA-256 digest computed from the actual received words. Invalid memory,
unsupported controls, changed words or extra data stop execution with a model
error. They cannot pass a programming check. A new programming reset is needed
after a rejected transfer.

This retains upstream synchronous DMA timing. Physical PCAP timing and status
for every hardware fault remain open. The strict model rejects migration.
It does not decode FPGA logic, connect the missing reset sequencer or prove an
exact physical board. The generic upstream path remains distinct when neither
property is set. Existing frozen demonstrations retain that earlier identity.

The completion-status policy is a reference adaptation. The
[AMD initialization procedure](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/Configure-the-PL-via-PCAP-Bridge-Example)
also describes completion status during initialization before configuration
data is sent. This strict path restricts that flag to checked image delivery.
It does not yet reproduce every physical initialization event or status bit.
The independent transfer report must check all actual words; a hardware status
bit alone is not proof. Generic QEMU reset values are reference-model values.
They are not measured physical cold-reset values. See
[the reference observations](../../../work/simulator/qemu-s9/pcap-reference-313.json).

Reference source and finding:
`work/simulator/qemu-s9/pcap-source-297/provenance.json`.
The archived driver is
`work/simulator/kernel-gpio-source-001/members/drivers/char/xilinx_devcfg.c`.
Backend298 passes the compiled 107-transaction test with undefined-behavior
checks. It exercises split transfers, both payload forms, programming reset,
two system resets, interrupt clear/mask behavior and corrupted guest memory.
All three completed stream digests match the selected input. Probe300 caught
an ignored QEMU option and remains failed. Device names that contain a dot
require the explicit `driver=...,property=...,value=...` option form.

The expanded test313 passes 118 transactions and eight actual GIC interrupt
edges on sanitized backend312. Test309 found a stale interrupt after programming
reset. Test311 found DMA completion during register reset without a command.
Both failures remain retained. The corrected strict path refreshes its output
after reset and suppresses DMA submission from reset callbacks. The test checks
masking, unmasking, selective status clearing and system reset with IRQ asserted.

Backend315 also rejects the unimplemented fixed-source DMA control. Earlier
versions silently used increasing addresses and could accept an image when
the guest had prohibited that behavior. The `pcap-fixed-source` test retains
the failure on backend312 and passes the explicit guard on backend315. Actual
keyhole reads and their FIFO behavior remain implementation work. This guard
does not establish that hardware capability.

Backend320 implements fixed 32-bit source reads. It reads the source address
again for each word. It does not cache that word or advance the address.
This follows the documented
[source-increment control](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/Register-XDCFG_CFG_OFFSET-Details)
and [32-bit DevC interface](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/PCAP-Bridge-to-PL?contentId=6KE9sGlmfy05i32B6bS8Vw).
The trace records the selected source mode. The independent auditor also
reports older traces that lack that field as legacy increment-only records.

The compiled fixed-source test321 passes 146 transactions with sanitization.
It reads eight copies of one RAM word, changes to incrementing reads and
completes the exact image. It then rejects a repeated stream at its first
different word. The same test fails the old guard315. This checks RAM reads;
peripheral FIFO sources and physical transfer timing need separate evidence.

Control-recovery test320 found another defect on backend315. Restoring valid
controls after a rejected transfer allowed configuration without programming
reset. Backend320 now latches every rejected DMA operation. It preserves the
received prefix and requires reset before further data. The 136-transaction
control-recovery test321 passes, including completion after explicit reset.
This is the strict reference failure policy. It does not establish physical
hardware error flags or recovery timing. The ordinary 118-transaction test
also passes. See the retained
[source change and failure](../../../work/simulator/qemu-s9/pcap-derived-320/provenance.json).

Archived firmware run302 also passes. Original U-Boot reads the SD file and
transfers 2,083,740 payload bytes from guest address `0x02100000`. The observed
digest is `49d0faa400fbc8bed3f3e651da2c6da5f3184a08f942ec95db59c2526dcbea78`.
Independent mining checks verify 1,325 computed hashes at a 10 H/s cap, two
fresh jobs and two Core-accepted blocks. Both blocks survive node restart.
The run remains an adapted S9 reference result.

The [combined report](../../../work/simulator/reports/checked-configuration-305.json)
also records release-backend test305 and worker run304. The worker uses the
same captured process and job seeds. It transfers the inputs to a separate
local process, verifies destination hashes, retrieves the result and confirms
run cleanup. Its 1,652 computed hashes pass independent checks. Both block
candidates pass separate Core acceptance and restart tests. This is an IPC
transport test. It does not establish SSH or TLS dashboard support.

Select `configuration-contract` in `s9_boot_bundle boot` to require this path.
The shared local and worker result code requires canonical device options,
matching captured input hashes and ordered complete DMA evidence. It retains
`configuration-reference.bit` and `configuration-audit.json`. Missing traces,
changed data, false counts and premature completion fail the check.
The checker also rejects a missing initial reset, a DMA range outside 32-bit
memory, competing device options and changed or linked retained references.

```sh
work/simulator/check-target-085/release/examples/qemu_ttc_reset \
  work/simulator/qemu-s9/backend-build-315-ubsan \
  work/simulator/qemu-s9/pcap-irq-316 pcap
```

That output exists. Use a new path to repeat the test. It needs the pinned
selected image in `work/simulator/original-zynq/sd-bundle-291/boot/system.bit`.
The test exposes only private pipes in its isolated environment.

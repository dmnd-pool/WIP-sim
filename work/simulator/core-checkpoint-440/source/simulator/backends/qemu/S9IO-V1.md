# S9IO 1.0.2 reference contract

This is a development model of the BM1387 interface. It is separate from the
legacy 0.2 model. It is not a validated model of the selected 2022 FPGA image.
Do not use it to exclude or pass an exact target.

The reference archive is DCENT_OS, SHA-256
`5501e53fc05648ce4a03da2b47ba799c0fe1e0c6dd4e358db5a18504d419f25c`.
The isolated extraction is in `work/simulator/dcent-s9io-source-003/`.
It contains these exact source members:

| Member under `DCENT_OS_Antminer/dcentrald/` | SHA-256 |
| --- | --- |
| `dcentrald-hal/src/fpga_chain.rs` | `9e08e9959e5647298a41b5e2d7d5db7ac5aeddec83c139398e42a8327674131d` |
| `dcentrald-asic/src/drivers/bm1387.rs` | `5ff740dda4cafa5935ec910173bdc3b6110a2b30c64a4fee684ea343659d2247` |
| `dcentrald-asic/src/protocol.rs` | `7b86185f19b757dab0c46538c4664597ddeb4a154391765ecc461dc77efa5c43` |
| `dcentrald-asic/src/work_tx.rs` | `8054a41599a6d5c875a03fc7ca14046503ebab83a4e1bf3bd1042236a879bf70` |

The primary public project is
[DCENT_OS](https://github.com/DCentralTech/DCENT_OS).
Its claims of hardware measurements are reference statements. This archive
inspection does not independently establish those measurements. The catalogue
module `src/bm1387.rs` contains conflicting chip-family and work-ID statements.
The model uses the driver code and the known BM1387 wire format.

The selected profile reports version `00901002`. It uses the reference S9
serializer frequency of 200 MHz. UART division remains
`frequency / (16 * (divisor + 1))`. ENABLE-low drives TXD low, as specified by
the reference driver's ASIC BREAK reset sequence. This behavior differs from
the legacy model. No ASIC reset success follows from driving that pin.

The common register addresses, command words, 12/20/36-word work format,
midstate order, FIFO reset controls, interrupt masks, and 2048-word work queue
match the common interfaces in the two source revisions. The implementation
reuses the legacy FIFO, checksum, receive and UART state machines.
This reuse is an inference. Exact internal timing, undocumented FIFO corner
cases, power-on register values and reset behavior of version 1.0.2 remain
unvalidated. In particular, the reset BAUD value remains the legacy value zero.
The reference project's simulated register initialization is not a hardware
power-on trace and is not used as proof of a different reset value.

CONTROL bit 4 selects BM139X mode in the reference contract. The model stores
that bit but stops when the guest enables that unimplemented dataplane.
It never sends BM1387 packets for BM139X mode. The reference also describes a
destructive disable operation that clears the midstate field after UART
traffic. Its precise internal failure and recovery are unknown. The model
rejects that transition and the backend stops with a model error. It does not
invent successful recovery. Global device reset clears the model failure.

The build ID remains the explicit `74657374` development fixture value.
Clock and reset connections to the Zynq fabric are not implemented. The
selected FPGA image is not executed or decoded into logic. Its correspondence
with this model remains open.

Select this profile only with the explicit `sd2022-v1` boot diagnostic option.
The run manifest records the revision, clock, contract hash and inference.
The backend must pass its separate profile startup check before selection.
Startup alone does not establish firmware boot or mining.

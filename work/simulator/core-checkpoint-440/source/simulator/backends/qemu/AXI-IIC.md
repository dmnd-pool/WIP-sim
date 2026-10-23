# AXI IIC development model

The Rust engine models dynamic master transfers. QEMU supplies virtual
byte timers and I2C slave callbacks. Address and data transfers each take
nine bus clocks. The receive-count FIFO word is control data. It does
not add a wire byte. Bus timing is not physically calibrated.

The contract basis is AMD PG090, version 2.0, October 5, 2016. Its
[interrupt description](https://docs.amd.com/api/khub/documents/qDTQFhZJdUT~2BwH2AuUjA/content)
defines the bus-idle interrupt as a persistent condition. The
[version 2.1 description](https://docs.amd.com/r/en-US/pg090-axi-iic/Interrupt-4-IIC-Bus-is-Not-Busy)
retains that rule. These online references were checked on 2026-10-21.
Their complete source files are not captured in the archive.

Original SD2022 run017 exposed a defect. The guest read the interrupt
register, then its one-byte transfer finished before the guest toggled
the status bits. The model lost the idle interrupt. The guest timed out
and sent an acknowledged `55` byte again. The PIC then received
`55,55,aa,17`, which does not form a valid version command.

The corrected model keeps the idle interrupt set while its bus is idle.
An enabled START at the FIFO head reserves the bus during the address
interval. The slave callback still occurs at the address ACK boundary.
This is a byte-clock abstraction. It does not claim sub-cycle START,
signal filtering or electrical timing.

The regression exercises each placement of address and data completion
around the driver's interrupt read and clear. It checks one actual
peripheral write, no retry, and the next transfer's busy state. A disabled
controller with a queued START remains idle.

Run the component checks from the repository root:

```bash
CARGO_INCREMENTAL=0 cargo test --offline --locked --manifest-path simulator/backends/qemu/axi-iic/Cargo.toml --features host-std
```

The standalone crate needs `host-std` for these host tests. Its normal
library build does not use the Rust standard library.

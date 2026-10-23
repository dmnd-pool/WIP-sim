# S9 GPIO development model

The Rust model implements one fixed-direction AXI GPIO channel.
The QEMU binding uses word accesses and settled input levels.
It has real DATA and TRI state, width masks and reset values.
It does not yet model input synchronization, AXI handshakes or fabric clock
and reset domains. It does not establish a complete physical GPIO model.

The [archived board script](../../../work/simulator/braiins-fpga-source-005/references.json)
declares a nine-bit input block at `0x41200000` and a thirteen-bit output
block at `0x41210000`. Each address window is 64 KiB.
The [original guest console](../../../work/simulator/qemu-s9/legacy-s9io-console-003/guest.log)
confirms those device-tree declarations. Both DATA reset values are zero.
Both TRI defaults are `ffffffff`. Each block has one channel and no interrupts.
These are firmware declarations, not measurements of a physical board.
The earlier console probe 002 used an absent `od` command. It did not
extract the properties successfully. Probe 003 uses the archived `hexdump`
and stops on command failure.

The register behavior follows AMD PG144, version 2.0, dated 30 January 2026.
Input DATA reads the pin values. Output DATA reads zero and writes drive
the output port. The fixed-direction wiring remains separate from TRI.
See the [DATA register](https://docs.amd.com/r/en-US/pg144-axi-gpio/AXI-GPIO-Data-Register-GPIOx_DATA),
[fixed inputs](https://docs.amd.com/r/en-US/pg144-axi-gpio/All-Inputs)
and [fixed outputs](https://docs.amd.com/r/en-US/pg144-axi-gpio/All-Outputs).

Writes use the entire data word. The byte strobe does not mask it.
Removed channel and interrupt registers read zero and ignore writes.
See the [register contract](https://docs.amd.com/r/en-US/pg144-axi-gpio/Register-Space).
The binding accepts aligned 32-bit transfers. Narrow writes remain unsupported
because a CPU byte transfer does not specify the unused AXI data lanes.

The public vendor reference has a nine-bit address port. The binding decodes
that port within the board's 64 KiB window. See the
[vendor AXI wrapper](https://github.com/Xilinx/revCtrl/blob/master/bd/zynq_bd/ip/zynq_bd_axi_gpio_0_0/axi_gpio_v2_0/hdl/src/vhdl/axi_gpio.vhd).
This web reference is not a pinned source input. The document download failed
because this host could not resolve the source domain. Exact source capture
and correspondence to the selected bitstream remain open.
Do not use this model as evidence that those gates pass.

The [archived kernel driver](../../../work/simulator/kernel-gpio-source-001/references.json)
has source archive SHA-256
`5aa5c0915c93c6078984b13367edd12670e8ad25d922d352654f768d7005db8f`.
Its `drivers/gpio/gpio-xilinx.c` member has SHA-256
`d50659f2f29149d0b7ddc6fab303d61c3c1dfd38ba6ec51a572e87ce5f0cb867`.
The driver reads DATA for pin values and keeps its own output and direction
shadows. Matching that source does not establish the original kernel build.

The new QEMU machine is `md-sim-s9-gpio-development`.
Its inputs default to explicit low levels. It has no attached ASIC chain.
Test pin transitions do not count as hashboard detection or mining results.
It inherits the legacy S9IO and NAND development fixtures, with their limits.
Its QEMU object paths are `/machine/gpio-input` and `/machine/gpio-output`.
Migration is refused until the full state has a versioned format.

The following component checks passed. Use new output paths for another run.

```sh
CARGO_INCREMENTAL=0 cargo test -p md-sim-firmware --test axi_gpio --offline --locked
cc -std=c11 -Wall -Wextra -Werror -fsanitize=address,undefined -fno-sanitize-recover=all simulator/backends/qemu/tests/gpio-ffi.c target/release/libmd_sim_qemu_core.a -lm -ldl -lpthread -o /tmp/md-sim-gpio-ffi
ASAN_OPTIONS=detect_leaks=0 /tmp/md-sim-gpio-ffi
target/release/examples/s9_backend_qtest /tmp/md-sim-qemu-backend-039-ubsan work/simulator/qemu-s9/gpio-qtest-002-ubsan gpio
target/release/examples/s9_backend_qtest /tmp/md-sim-qemu-backend-039-ubsan work/simulator/qemu-s9/sd2022-s9io-qtest-001-ubsan sd2022-s9io
```

The [GPIO result](../../../work/simulator/qemu-s9/gpio-qtest-002-ubsan/assessment.json)
checks 25 protocol responses and six actual output-pin transitions.
The [layout result](../../../work/simulator/qemu-s9/sd2022-s9io-qtest-001-ubsan/assessment.json)
checks 1,363 responses, serial packets and 18 interrupt transitions.
The [backend identity](../../../work/simulator/qemu-s9/backend-build-039-ubsan/build-report.json)
records the UndefinedBehaviorSanitizer build and all source inputs.
LeakSanitizer remains unavailable under this host's tracing setup.

The separate `md-sim-s9-sd2022-io-development` machine uses addresses and
interrupts declared by the selected 2022 image. It reuses the legacy S9IO core.
That reuse is an explicit development hypothesis. The tests establish the
mapping and component behavior; they do not establish correspondence to the
archived bitstream or a complete original-board gate.

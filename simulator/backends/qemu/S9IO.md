# Legacy S9IO controller checks

This development machine implements three S9IO0.2 controller blocks.
It uses the archived Braiins hardware source. It does not establish
compatibility with the selected 2022 FPGA bitstream.
It has no ASIC chain. Injected serial replies in these tests are test inputs.
They are not mining results.
The separate [1.0.2 reference profile](S9IO-V1.md) has its own clock, reset
signal contract and explicit limits. It does not change this legacy profile.

The Rust state machines provide the registers, bounded queues, serial checksums,
UART, interrupts and reset behavior. The C file connects them to QEMU.
C is required for QEMU's device and memory interfaces. It does not duplicate
the Rust state machines. The combined Rust library also retains the existing
I2C and NAND controller models.

The source archive SHA-256 is
`dc5f58721f596d2fc36b50d5049461264986ac339c4b4ff4d383749b98f3a363`.
The board script SHA-256 is
`650d3c550e8240218493b85c848182406347249661e785c87ea58cff2695d44d`.
The retained [source record](../../../work/simulator/braiins-fpga-source-005/references.json)
includes the exact text and hashes. The script connects the first controller
to FPGA interrupt bits 6, 7 and 8. Zynq maps these to GIC numbers 67, 68 and 84.
This explains the gap that the source README's general `base + 2` rule misses.

| Controller | Address | GIC interrupt numbers | Object path |
| --- | --- | --- | --- |
| S9IO 5 | `0x43c50000` | 67, 68, 84 | `/machine/s9io5` |
| S9IO 6 | `0x43c60000` | 85, 86, 87 | `/machine/s9io6` |
| S9IO 7 | `0x43c70000` | 88, 89, 90 | `/machine/s9io7` |

Each block has a 64 KiB address window. The fixture uses version `00900200`
from the hardware source and an explicit test build ID, `74657374`.
The conflicting version in the source SVD is not substituted.
The machine name is `md-sim-s9-legacy-io-development`.
Its inherited NAND fixture still has unresolved physical board identity.

The legacy board now connects the core to SLCR fabric clock 1. The archived
board script selects a nominal 50 MHz. The component's unconnected clock
fixture remains 50 MHz. The hash cap does not change either clock.
Serial pins and internal state advance on that clock. Idle advancement retains
the clock phase. An enabled controller can repeat queue checks without
an external event. This includes a work queue that lacks its final words.
The model calculates these repetitions directly. It retains the timer
expiration, timer acknowledgement and state phase. A command that waits for
another word also keeps its state while the free timer advances. Pending
resets, serial events and complete queued packets require ordinary evaluation.
This avoids a callback on every clock when the guest enables the default timer.
The equivalence tests compare the next complete edge with ordinary execution.
They check partial packets, later FIFO writes, mode changes and queue resets.
The compiled [partial-packet regression](../../../work/simulator/qemu-s9/partial-fifo-after-077-002/assessment.json)
also checks all serial bytes, their timing and the interrupt transitions.
The [same pause on the previous model](../../../work/simulator/qemu-s9/partial-fifo-before-076/assessment.json)
exhausts its CPU limit. Both results remain in the evidence.
QEMU memory accesses are atomic transactions.
They do not implement AXI channel stalls. At one timestamp, elapsed core edges
precede a memory access or serial input change. Register polling does not advance
device time. The separate Rust AXI model tests channel handshakes.
Neither model has a physical timing calibration result.
The [fabric clock test](FABRIC.md) now checks guest divider changes and a
mid-byte clock stop. Raw processor reset outputs also work. The external
reset sequencer and its connection to the peripherals remain required work
before a complete board claim.

Run these commands from the repository root:

```sh
CARGO_INCREMENTAL=0 cargo build -p md-sim-qemu-core --release --offline --locked
CARGO_INCREMENTAL=0 cargo test -p md-sim-firmware --test s9io_axi --test s9io_idle --offline --locked
cc -std=c11 -Wall -Wextra -Werror -fsanitize=address,undefined -fno-sanitize-recover=all simulator/backends/qemu/tests/s9io-ffi.c target/release/libmd_sim_qemu_core.a -lm -ldl -lpthread -o /tmp/md-sim-s9io-ffi
ASAN_OPTIONS=detect_leaks=0 /tmp/md-sim-s9io-ffi
```

The C interface test checks byte lanes, bounds, computed command checksums,
serial reply input, interrupts, reset and unsupported-state failure.
AddressSanitizer and UndefinedBehaviorSanitizer pass.
LeakSanitizer cannot run under this host's tracing setup. Its failed check is
retained. This is not a leak-check pass.

The following build uses the previously captured toolchain and Samurai build.
It runs the archived QEMU build tools in the isolated preparation environment.
The output directory must be new. QEMU source commit
`11661d857254b27fca5568c6deacf4f6b6f3460f` has archive SHA-256
`2a40e6705e59fd8807012631bc6b2c0877f10481b12768f78e4f9d7b88376cce`.
The build records the patches, library, compiler inputs and output identity.

The current build command is in the [README](../../../README.md).
It requires an explicit source checkpoint. The following test commands name
historical build artifacts; their reports retain the original build inputs.

```sh
target/release/examples/s9_backend_qtest /tmp/md-sim-qemu-backend-034 work/simulator/qemu-s9/legacy-s9io-qtest-003 legacy-s9io
```

These exact run paths now contain retained results. Use new output paths for
a new run. The build can reuse the captured overlay in backend 031 instead.
The [build report](../../../work/simulator/qemu-s9/backend-build-034/build-report.json)
identifies the tested executable. The
[interface test](../../../work/simulator/qemu-s9/legacy-s9io-qtest-003/assessment.json)
passes 1,363 protocol responses and 18 interrupt transitions.
It checks all nine GIC input routes. It also checks actual UART transmission at
three divisors against the archived packet and exact byte-start intervals.

The test observes GIC input signals because QEMU's test memory accesses are
non-secure. They cannot read the reset GIC's Group 0 pending bits.
The test does not change the GIC security model.
CPU exception delivery requires a separate guest execution test.
The worker stops the finite test stream at its deadline. A crash, missing
response, wrong interrupt, truncated trace or wrong serial timing fails the test.

The UndefinedBehaviorSanitizer build and its tests also pass:

```sh
target/release/examples/s9_backend_qtest /tmp/md-sim-qemu-backend-037-ubsan work/simulator/qemu-s9/legacy-s9io-qtest-004-ubsan legacy-s9io
target/release/examples/s9_backend_qtest /tmp/md-sim-qemu-backend-037-ubsan work/simulator/qemu-s9/combined-iic-qtest-002-ubsan
target/release/examples/s9_backend_qtest /tmp/md-sim-qemu-backend-037-ubsan work/simulator/qemu-s9/combined-onfi-qtest-001-ubsan nand-onfi
```

See the [sanitizer build](../../../work/simulator/qemu-s9/backend-build-037-ubsan/build-report.json),
[controller test](../../../work/simulator/qemu-s9/legacy-s9io-qtest-004-ubsan/assessment.json)
and [source review](../../../work/simulator/s9io-qemu-review-001.json).
The build permits a 128 MiB intermediate file because the unstripped sanitizer
link exceeded 64 MiB. The final executable remains limited to 28 MiB.
Firmware-run limits are unchanged.

The default-timer regression now also checks one simulated second after
enabling each controller, before changing its timer or baud registers:

```sh
target/release/examples/s9_backend_qtest /tmp/md-sim-qemu-backend-040-ubsan work/simulator/qemu-s9/legacy-s9io-qtest-007-ubsan legacy-s9io
target/release/examples/s9_backend_qtest /tmp/md-sim-qemu-backend-040-ubsan work/simulator/qemu-s9/sd2022-s9io-qtest-002-ubsan sd2022-s9io
```

Both layouts pass with the [new build](../../../work/simulator/qemu-s9/backend-build-040-ubsan/build-report.json).
The [legacy test](../../../work/simulator/qemu-s9/legacy-s9io-qtest-007-ubsan/assessment.json)
and [2022-layout test](../../../work/simulator/qemu-s9/sd2022-s9io-qtest-002-ubsan/assessment.json)
retain their complete responses and serial traces.
The [same test on build 039](../../../work/simulator/qemu-s9/sd2022-s9io-empty-before-001/assessment.json)
failed after the controller enable. This failure remains in the evidence.

The [archived firmware diagnostic](../../../work/simulator/s9io-102-evidence-001.json)
now reaches these registers through the separate 2022 address-map fixture.
It rejects the legacy version. The separate reference profile now executes
archived firmware and produces verified mining results, as the
[README](../../../README.md) describes. Exact board conformance and the complete
hardware matrix remain open.
The device refuses migration until its entire state has a versioned format.

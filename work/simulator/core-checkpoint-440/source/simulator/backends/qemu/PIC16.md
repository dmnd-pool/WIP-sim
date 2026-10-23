# S9 PIC command model

This Rust component models the commands used by the archived S9 driver.
It does not execute PIC instructions. The C binding attaches three separate
instances to the QEMU I2C bus in `md-sim-s9-pic-development`.
This is a development profile. No board power or mining gate passes from
component tests.

The exact application is retained under `simulator/firmware/pic16/`, with
parent hashes in its provenance manifest. The 2022 ramdisk member
`lib/antminer/hash_s8_app.txt` is identical to the archived Braiins source
package. Its text SHA-256 is
`ec84da07ef98665c6258cc531dc2d74aa8be7018925c041325ca233aec52b46a`.
It contains 3200 fourteen-bit words for word addresses `0300..0f7f`.
The big-endian word byte stream has SHA-256
`d138aefed416e18a7c5f2a374ef99148c745f7d576b3821e615225fffad1d35b`.

Protocol evidence comes from the archived Braiins `power.rs` and
`power/firmware.rs`. The latter has SHA-256
`54145acc99d975db9b1a4652462e59fb474ac7008f13a91b090d70a611933e3c`.
Source archive identity is
`dc5f58721f596d2fc36b50d5049461264986ac339c4b4ff4d383749b98f3a363`.
The separate DCENT `pic/braiinsos.rs`,
SHA-256 `38f388512623db7932e3550c5bf9601611bdff4ade714d95722bed83a55ef1fe`,
documents the same runtime grammar. These are driver contracts, not captured
PIC instruction execution. The unrelated `pic1704` interface is not selected.

Commands use the `55 aa` prefix. Parser state survives I2C STOP, because the
driver sends individual bytes in separate transactions. A mismatched second
prefix byte returns to the initial state without rematching the byte.
The documented zero flush can therefore restore an unfinished argument
parser before reset. The exact application's
[interrupted-prefix path](../../firmware/pic16/parser-analysis.md) consumes
one data byte after invalid command00 or55 and clears its parser. ItsEE
diagnostic goes to an unconnected UART. It is not an I2C reply.
That path leaves voltage, power and heartbeat state unchanged. Other
unsupported commands remain errors. The loader retains its separate contract.

The modeled loader has a word-addressed pointer, a 16-byte transfer buffer,
32-word sector erase, eight-word program operations and flash readback.
Only committed operations change storage. Reset before completion aborts the
pending operation; reset at its deadline first commits it. Program operations
clear bits. Erase restores fourteen one bits. Out-of-range or unaligned
operations are rejected. The precise result of interrupted physical writes,
loader write protection and malformed upper word bits are not calibrated.

The explicitly named development constructor fills storage outside the
application with `3fff`. This is not a factory dump, a bootloader image, or
valid calibration data. No good-board identity or bad-core table is invented.

Jump checks the complete current application hash. Only the exact known
image selects the version3 runtime contract. A damaged or unknown program
fails; it does not receive a successful version reply. The supported runtime
stores voltage commands, controls the enable state, reads stored voltage,
and accepts heartbeat commands. Power-on voltage is unknown until written.
Enabling an unknown voltage fails.
The exact application enables power for every nonzero enable payload. A new
host's55 prefix can therefore enable power if the earlier host stopped during
that command. The parser regression retains this transient before reset.

The current timing parameters use the driver's 500ms reset wait, 100ms
operation waits and approximate 10s heartbeat timeout. These are explicit
uncalibrated parameters, not measured operation durations. Static analysis of
the [exact application](../../firmware/pic16/heartbeat-analysis.md) establishes
that disabled power pauses its software heartbeat counters. The model keeps
the remaining interval when power is disabled. Repeated enable commands do
not reset this interval. A received heartbeat resets it without changing power.
Expiry cuts power and clears the elapsed counter. The application remains
available for commands. Only an explicit enable command restores power.
The hardware watchdog is a separate mechanism; this is not a successful
hardware-watchdog test or PIC CPU execution. An explicit external reset starts
the loader contract again. Timer0 phase and exact cutoff timing remain
uncalibrated.

Only the first byte of each read transaction is defined by the driver.
Additional bytes repeat the output latch and are marked undefined in the
component response. The driver discards them. This is an explicit assumption
pending an exact wire trace.

The binding requires an explicit, bounded application text file. Its decoded
program must match the pinned hash. Busy devices and empty reply queues give
an I2C NACK. A model error stops the emulator. Every byte and bus boundary can
be traced. Virtual timers commit pending writes and enforce the heartbeat
deadline even without a subsequent bus access. Power enable has an output
pin. The separate PIC development machine leaves it unconnected. The BM1387
reference machine connects it to its corresponding ASIC chain.
Committed flash survives device reset but remains private process storage.
Migration and storage across process restart are not implemented.

The compiled bus test uses the pinned application word `3183`, separate
nonzero flash pointers, runtime voltage readback and actual power output
transitions. It also tests initial busy NACK, undefined extra read bytes,
a20-second disabled interval, scheduled cutoff without a bus access, and
version readback after expiry. It requires seven exact power-state timestamps
and three enable/disable pairs. The [current report](../../../work/simulator/qemu-s9/pic16-heartbeat-qtest-078/assessment.json)
passes579 responses under UBSAN. The same test rejects the former077 backend.
These are protocol tests, not PIC instruction or whole-board acceptance.

The C binding regression also passed address and undefined-behavior sanitizers:

```sh
cc -std=c11 -Wall -Wextra -Werror -fsanitize=address,undefined -fno-sanitize-recover=all simulator/backends/qemu/tests/pic16-ffi.c target/release/libmd_sim_qemu_core.a -lm -ldl -lpthread -o work/simulator/checks-2026-10-21-001/pic16-heartbeat-ffi-001
ASAN_OPTIONS=detect_leaks=0 work/simulator/checks-2026-10-21-001/pic16-heartbeat-ffi-001 simulator/firmware/pic16/braiins-s9-app.txt
```

LeakSanitizer is disabled because of this environment's tracing restriction.
This does not establish the absence of memory leaks.

Remaining work includes exact factory storage, controller
and application instruction execution where required, undocumented commands,
temperature-offset storage, voltage ramp and load response, board connections,
hardware-watchdog execution, timing calibration and firmware-driven acceptance
tests. The former fail-on-heartbeat-expiry behavior is retained in historical
backend evidence; it does not describe the current model.

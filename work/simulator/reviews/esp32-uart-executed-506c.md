# ESP32-S3 UART register and capture checks

Backend018c and the corrected506d register/reset tests pass. This is a
peripheral component result. The complete board and mining gates remain open.

The S3 subclass now masks CLKDIV correctly, stores CLK_CONF at`0x78`, and
stores DATE at`0x7c`. Its region extends to128bytes so DATE is reachable.
Device reset calls the inherited reset and restores the documented defaults.
Selected writes record the device, virtual time, sequence, address, size, raw
word and stored word. The lifetime trace budget survives reset.

The first018 preparation failed on a missing derived overlay. Its exact
bytes were restored from retained008 evidence, with a
[separate receipt](esp32-uart-overlay-restore-506.json). Build018b then passed,
but its actual UART tests failed. The new DATE callback lay outside the old
region. The reset fixture also lacked a timer for its clock step, and early
stdin closure caused a GLib shutdown assertion. All failed runs stay intact.

The fixes extend only the S3 region, schedule a real I2C test deadline and
hold the qtest input open through shutdown. No UART assertion or deadline was
removed. Unexpected runtime diagnostics now prevent a component pass even
when the process exits0. The corrected helper still fails only DATE reads on
unchanged018b.

The [independent018c review](esp32-uart-independent-execution-506c.json) checks
the169 builder and170 test source identities,42 build mounts and24 build
streams. The actual C command includes all three undefined-behavior trap
flags. Build018c exits0 after194,739ms without truncation. Backend SHA-256 is
`b15458cea0c5a54d85f6f524f19c9caff9cdced48e2cc9c6b8e51b9868e30342`.
The ROM stays unchanged.

All46 basic register responses pass. The reset suite passes332 responses,
including all315 earlier assertions, two actual QMP resets and real I2C NACK
deadlines. It retains257 writes and one deliberate overflow. That overflow
is a passed negative test, not complete firmware capture. Both runs have clean
runtime diagnostics. No firmware executes in these register tests.

These commands ran from the repository root:

```sh
work/simulator/esp32-uart-tools-506d/esp32_backend_qtest work/simulator/bitaxe/esp-qemu-build-018c work/simulator/bitaxe/uart-config-qtest-506-new018c uart-config
work/simulator/esp32-uart-tools-506d/esp32_backend_qtest work/simulator/bitaxe/esp-qemu-build-018c work/simulator/bitaxe/uart-reset-trace-qtest-506-new018c uart-reset-trace
work/simulator/bm1397-probe-tools-507/esp32_original_probe work/simulator/bitaxe/esp-qemu-build-018c artifacts/files/firmware/open-gh-asset-475665897/esp-miner-factory-102-v2.14.2.bin work/simulator/bitaxe/original-probe-040 bm1397-control-storage-adc-uart-config
```

Use new output directories to repeat them. Frozen helpers and their inputs are
required; the source build alone does not install the archive or backends.

Original040 executes unchanged factory102 v2.14.2 firmware, SHA-256
`3fd36298681d7797429dd72610041d731558c5253430902a3c7feeced232557c`.
It exits1 after12,534ms at the real unsupported ASIC read`0x04`. Its
[independent audit](esp32-uart-original-independent-507.json) checks134 frozen
sources,17 mounts,253 CRC-valid ASIC commands and26 UART configuration writes.
Nine writes belong to the ASIC UART. The only response is identification;
there is no job, nonce or write acknowledgement. The complete configuration
capture does not establish effective baud or physical clock calibration.

The [UART scope](../../../simulator/backends/esp32/UART.md) lists remaining
line timing, clock-gate, reset-pin and migration work. ADC calibration errors
remain visible. The [exact ADC source analysis](esp32-adc-calibration-contract-509.json)
explains the missing factory data without inserting a successful reading.

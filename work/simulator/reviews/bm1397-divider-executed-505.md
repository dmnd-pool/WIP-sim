# BM1397 posted output divider505

The register bank now stores the original PLL output-divider word at`0x70`.
Only its low-byte software encoding is decoded. Encoded255 cannot be
represented by the original eight-bit software divisor and returns an error.
The other fields stay as raw bits. This does not establish a physical clock.

The [independent source review](bm1397-divider-independent-source-505.json)
checks the archived original T17 instructions and the retained496 arithmetic
results. The focused checks pass:31 library tests, five probe tests, strict
scoped Clippy, formatting and release. All256 raw low-byte encodings and
all255 representable software divisors are checked.

The [frozen helper inputs](../bm1397-probe-tools-505/inputs.json) retain133
source files checked before and after compilation. The helper SHA-256 is
`dfa59ac3778b936e53cc3ae144562cc774572bce8bb31c5976d039af63ee926c`.

This command ran from the repository root:

```sh
work/simulator/bm1397-probe-tools-505/esp32_original_probe work/simulator/bitaxe/esp-qemu-build-017 artifacts/files/firmware/open-gh-asset-475665897/esp-miner-factory-102-v2.14.2.bin work/simulator/bitaxe/original-probe-039 bm1397-control-storage-adc
```

Use a new output directory to repeat the probe. The original factory firmware
SHA-256 remains
`3fd36298681d7797429dd72610041d731558c5253430902a3c7feeced232557c`.
Probe039 exits1 after11,898ms without truncation. The firmware completes its
requested frequency ramp, logs ASIC initialization, and starts factory
self-test. Its next read, register`0x04`, is unsupported. These guest log
messages do not establish effective clocks or complete board boot.

The [retained-byte audit](bm1397-divider-execution-audit-039.json) checks133
source files and17 mounts. It reconstructs253 command frames and replays245
posted state updates. There are120 divider writes and120 PLL writes, with
zero acknowledgements. The only response is the earlier identification frame.
The auditor wrote the model; the parent supplied the independent source
review. The [assessment](../bitaxe/original-probe-039/assessment.json) keeps
complete board boot, functional mining and native dashboard gates false.

Next, correct the ESP32-S3 UART register observations and trace actual guest
clock writes. Connect hash-rate replies only to computed work with a recorded
measurement contract. Factory self-test uses fixed work and production-rate
thresholds; it cannot replace fresh-work mining acceptance.

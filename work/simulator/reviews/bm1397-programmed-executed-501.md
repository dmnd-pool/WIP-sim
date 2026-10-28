# BM1397 posted-control implementation501

The new explicit capability stores repeated core0, PLL0 and PLL3 writes for
each existing chip. Addressed and broadcast writes use the existing decoder
and chain. Invalid controls and ambiguous addresses fail before mutation.
Power loss and reset remove recorded knowledge. Unknown silicon reset values
remain absent. Guest writes produce no acknowledgement or invented lock,
effective clock, completion or guest readback.

The normal chain constructor retains its prior strict capability. The
explicit`RegisterStorage` capability advances the development profile. It is
implementation work toward the real device, with its current limits visible.

The reviewed501 source was integrated after checking all three original base
hashes, seven staged identities and the recorded patch. The
[integration record](bm1397-programmed-integration-501.json) pins the result.
The added arithmetic regression uses354,160 bytes of actual unchanged T17
instruction output from oracle496. All22,134 cases agree, including four
single-precision truncations below the exact rational floor. No new model
generated these expected values.

Focused checks pass:25 BM1397 library tests, five probe tests, formatting,
strict scoped Clippy and release. The
[helper/source capture](../bm1397-probe-tools-501/inputs.json) pins132 files,
the exact build logs and helper. The source closure equals commit`f5fbf74e`
except the seven reviewed501 outputs. Current source was rehashed after the
build. The frozen helper SHA-256 is
`5837e5cf26a96a536104a2073a36c7d1f3e7393408d02866657c6f8f6fca4945`.

Actual probe037 runs the unchanged original ESP-Miner v2.14.2 factory image,
SHA-256`3fd36298681d7797429dd72610041d731558c5253430902a3c7feeced232557c`.
It exits1 after2,879ms without truncation. This remains an incomplete boot,
mining and dashboard result. The host probe finishing is not a firmware pass.
The existing one-chip, power/reset, ADC and UART-pipe fixture remains explicit.
ADC calibration errors are preserved in the original guest output.

The actual firmware posts core0 data`0x74`, ticket`0xf0` and
PLL3 word`0xc0700111`. The new bank records those supported control writes.
The sole reply is the earlier identification response. The next command is
FastUART register`0x28=0x0600000f`, wire bytes`55aa510900280600000f18`.
The strict model rejects it. A later guest frequency log is programming
intent, not proof that a clock changed or locked.

The [independent execution review](bm1397-programmed-independent-execution-037.json)
rechecks132 sources,17 executed mounts, exact process streams and all ten
command CRCs plus the sole identification response. The one-shot observer is
absent. No write acknowledgement was sent. The
[assessment](../bitaxe/original-probe-037/assessment.json) keeps complete-board,
functional-mining and native-dashboard gates false.

These commands ran from the repository root, with
`CARGO_TARGET_DIR=work/simulator/check-target-085`, `CARGO_INCREMENTAL=0`,
`CARGO_BUILD_JOBS=1` and
`TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227` for Cargo:

```sh
cargo test -p md-sim-firmware --lib bm1397 --offline --locked
cargo test -p md-sim-firmware --example esp32_original_probe --offline --locked
cargo clippy -p md-sim-firmware --lib --example esp32_original_probe --offline --locked -- -D warnings
cargo build -p md-sim-firmware --example esp32_original_probe --release --offline --locked
cargo fmt --all -- --check
work/simulator/bm1397-probe-tools-501/esp32_original_probe work/simulator/bitaxe/esp-qemu-build-017 artifacts/files/firmware/open-gh-asset-475665897/esp-miner-factory-102-v2.14.2.bin work/simulator/bitaxe/original-probe-037 bm1397-control-storage-adc
```

Use a new output path for another probe. Next implement the evidenced FastUART
and baud-programming state, then run the original image again. Physical UART
timing, effective PLL clocks, remaining board devices, network, mining and
the complete target suite remain open.

# Original firmware probe036

The unchanged ESP-Miner v2.14.2 factory image executed with the explicit
`bm1397-observe-core0-adc` reference adaptation. It emitted the exact core0
write once. The observer recorded it and sent no response. The firmware then
programmed ticket register0x14 to0x000000f0 and sent PLL3 register0x68 with
0xc0700111. The strict model stopped at that next unknown command.

The isolated process exited1 after2,901ms. Its trace is complete. The real
ADC calibration errors remain. The only ASIC reply was the existing chip
identification reply. No core state, clock lock, timing or mining success was
created. ASIC initialization, original-board, mining and dashboard gates
remain incomplete.

The probe example passed five tests, strict Clippy and a release build.
All17 captured input mounts and127 frozen source files passed byte checks.
The author audit is in `../bitaxe/original-probe-036/stage-audit-author-493.json`.
The frozen helper and source receipt are in `../bm1397-probe-tools-493/inputs.json`.
The independent source review is `bm1397-observer-source-independent-493.json`.
Independent execution review is pending.

The command below was executed. The retained process result is an explicit
unsupported boundary, not a successful miner startup. Use a new output path
for a fresh probe.

```sh
work/simulator/bm1397-probe-tools-493/esp32_original_probe \
  work/simulator/bitaxe/esp-qemu-build-017 \
  artifacts/files/firmware/open-gh-asset-475665897/esp-miner-factory-102-v2.14.2.bin \
  work/simulator/bitaxe/original-probe-036 bm1397-observe-core0-adc
```

Next: execute the original T17 PLL integer-frequency leaf routine as an
arithmetic oracle. Its location and evidence limits are recorded in
`bm1397-pll-followup-493.json`. Physical lock and timing remain separate.

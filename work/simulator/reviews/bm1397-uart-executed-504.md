# BM1397 UART programming504

The explicit programmed-control bank now records FastUART register`0x28`
and MiscControl register`0x18` for each existing chip. Repeated, addressed and
broadcast writes retain the received word. Invalid routing still fails before
mutation. Reset and power loss remove recorded knowledge. Reads and jobs stay
unsupported. The model sends no write acknowledgement.

The separate UART view computes an exact requested CLKI baud ratio from a
caller-supplied clock and the original split divider fields. It does not
change the transport or establish an effective baud rate. PLL3 clock selection
returns an explicit error, including after a PLL3 word has been recorded.

[Source review](bm1397-uart-independent-source-504.json) checked the exact
archived ESP source and six original T17 instruction ranges. Two misleading
ESP comments remain separate from the wire behavior: the low divider is in
bits8–12, and25MHz divided by216 is115740.7407 bits/s. The original firmware
word and its own log/return values stay unchanged. Conflicting reference
implementations and the newer UART layout are not imported.

Focused checks pass:29 library tests, five probe tests, strict scoped Clippy,
formatting and release. The helper/source capture
[`bm1397-probe-tools-504/inputs.json`](../bm1397-probe-tools-504/inputs.json)
pins133 sources checked before and after compilation. The frozen helper hash
is`88d05953e9c9408f4665bbf8e6d8e2c412aa49d8dc83f9638124a1d7e6f26df3`.

Actual original factory probe038 exits1 after2,913ms without truncation. It
records the real FastUART word`0x0600000f` and MiscControl word`0x00007a31`.
The next unsupported write is PLL output register`0x70=0x0f0f0f00`. The
[assessment](../bitaxe/original-probe-038/assessment.json) keeps complete board
boot, functional mining and native dashboard gates false.

The [independent retained-byte review](bm1397-uart-independent-execution-038.json)
rehashes133 sources and17 executed mounts, checks both exact process streams,
and reconstructs all12 transmitted command frames with polynomial CRC checks.
Four state updates produce zero acknowledgement bytes. The only response is
the earlier identification reply. The one-shot observer is absent.

This original-firmware command ran from the repository root:

```sh
work/simulator/bm1397-probe-tools-504/esp32_original_probe work/simulator/bitaxe/esp-qemu-build-017 artifacts/files/firmware/open-gh-asset-475665897/esp-miner-factory-102-v2.14.2.bin work/simulator/bitaxe/original-probe-038 bm1397-control-storage-adc
```

Use a new output directory to repeat it. The firmware, backend, budgets,
environment and arguments retain the037 profile. Only the reviewed probe and
model build changes. Host-pipe transport still does not enforce the physical
UART line rate. The upstream UART clock/timing limitations and the next
original divider contract are recorded in
[the follow-up](bm1397-uart-divider-followup-20261028.json).

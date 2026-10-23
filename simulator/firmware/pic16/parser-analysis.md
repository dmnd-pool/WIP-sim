# Archived S9 PIC interrupted command path

This is static analysis of the exact application in [provenance.json](provenance.json).
It is not PIC instruction execution. Addresses below are program word addresses.
The [listing](../../../work/simulator/pic16-static-001/listing.txt) and its
[manifest](../../../work/simulator/pic16-static-001/manifest.json) identify the
decoded words. The text hash is
`ec84da07ef98665c6258cc531dc2d74aa8be7018925c041325ca233aec52b46a`.
The word-byte hash is
`d138aefed416e18a7c5f2a374ef99148c745f7d576b3821e615225fffad1d35b`.
Instruction decoding uses the same data sheet as the
[heartbeat analysis](heartbeat-analysis.md).

The native fan power-loss run190 stopped at QEMU time221946507570ns.
PIC address57 received55 after an unfinished55 AA prefix. The old miner had
sent that prefix before it stopped. Its replacement then began the normal
parser flush. The model rejected the resulting command55 and stopped QEMU.
See the [unchanged failing trace](../../../work/simulator/qemu-s9/fan-power-manual-recovery-190/backend.log).
More host time cannot fix this error.

The application has these paths:

| Words | Effect |
| --- | --- |
| 037b..038d | Recognize55 thenAA. Store parser states4 then5 in file016b. A mismatchedAA clears state. |
| 038e..039f | Save the command in file00c6. Clear its data count. Call0a99. |
| 0a99..0ad4 | Handle commands04,05,07,16 immediately. For00 or55, no comparison matches. File0077 remains zero and the routine returns zero. |
| 03a0..03a3,07a8..07ab | A zero return stores parser state1. This waits for one data byte. It does not rematch55 as a new prefix. |
| 0510..0517,03a5..03a6 | The next data byte in state1 clears state, then dispatches on the saved command at04f4. |
| 04f4..050b | Commands10,11,12,14,15,20,22 have data handlers. Neither00 nor55 matches. The default branch goes to04f0. |
| 04f0..04f3,07dd..07de | Clear parser state. WriteEE to file019a, which is UART TX1REG. This is not the I2C buffer. No voltage, enable or heartbeat counter changes occur on this path. |

The contract now models this one-byte invalid-command path for00 and55 in the
exact application mode. It retains unknown-command errors for other unsupported
commands and for55 in loader mode. It does not infer loader behavior from the
application. The diagnostic UART is not connected or modeled by this contract.
No I2C response is supplied for this path. Microchip's
[register map, pages25 and33–34](https://ww1.microchip.com/downloads/en/DeviceDoc/40001715D.pdf)
defines TX1REG at019a and SSP1BUF at0211. The distinction prevents a diagnostic
byte from being mistaken for a reply to the controller.

Regression tests send the interrupted prefix, check that no I2C reply appears,
and check unchanged power, heartbeat deadline and
flash. They then send the native zero flush and reset. The compiled bus test
also checks an empty reply queue and subsequent version readback through AXI IIC.
Full malformed-command and PIC CPU
coverage remain open. Independent disassembler comparison remains open.

Review also found that the model rejected enable payloads above1. The exact
branch at0478..0481 tests only whether the payload is zero. Every nonzero
value enables power. Source192 removes that false range check. Tests cover
all255 nonzero values and an interrupted command15 followed by the new host's
55 AA zero flush. That55 can briefly enable power until reset. The model
retains this behavior; the host's intent to flush does not change the byte's
meaning. Unknown initial voltage remains an explicit model limitation.

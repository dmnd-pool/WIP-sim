# Archived S9 PIC heartbeat path

This is a static analysis of the exact archived application. It is not PIC
instruction execution or a physical timing measurement. The model uses these
control paths with its existing, uncalibrated 10-second interval.

The input and parent hashes are in [provenance.json](provenance.json).
The text SHA-256 is
`ec84da07ef98665c6258cc531dc2d74aa8be7018925c041325ca233aec52b46a`.
The decoded big-endian word SHA-256 is
`d138aefed416e18a7c5f2a374ef99148c745f7d576b3821e615225fffad1d35b`.
Addresses below are program word addresses, starting at `0300`.

The [Rust listing tool](../../../crates/md-sim-firmware/examples/pic16_static_listing.rs)
decodes words linearly. It does not execute instructions or infer control flow.
Data words can decode as instructions. Its first output is retained in
[pic16-static-001](../../../work/simulator/pic16-static-001/manifest.json).
The opcode and register reference is the
[Microchip PIC16(L)F1704/8 data sheet, DS40001715D](https://ww1.microchip.com/downloads/en/DeviceDoc/40001715D.pdf).
Table31-3 gives the instruction encodings. Register maps and the instruction
descriptions define bank selection, page selection and conditional skips.

The following paths were traced manually. CALL and GOTO use `PCLATH` page
bits. Their low eleven bits alone are not complete destinations. File-register
addresses use the bank set by `MOVLB`. Common registers retain their fixed
meaning across banks. `STATUS.Z` is bit2; `STATUS.C` is bit0.

| Path | Exact words and effect |
| --- | --- |
| Application entry | `0300..0342` initializes storage and branches to `0e82`. The clear loop at `0811..0816` clears the range starting at `01a0`, including counters `01a4/01a5`. The divider at `016e/016f` starts at61 through `0320..0328`. |
| Power initialization | `0e82..0e84` calls `08dc`. Words `08eb=0020, 08ec=150c` select bank0 and set PORTA bit2. The logical power output starts disabled. |
| Power command | At `04f4`, the command byte is read. Successive XOR literals `10,01,03,06,01` select command `15` and branch at `0504` to `0478`. Payload zero follows `047c=0020, 047d=150c` and sets PORTA bit2. Nonzero follows `047f=0020, 0480=110c` and clears it. Neither path changes the heartbeat counters. |
| Heartbeat command | `038e..039f` saves the command and calls `0a99` with it in W. Successive XOR literals `04,01,02,11` select command `16`. `0ad1` branches to `0abc`. Words `0023,01a4,01a5,0022,01ee,01ef` clear both the elapsed counter and divider. The handler does not write PORTA. |
| Disabled power | At `0f0a..0f0e`, the loop checks Timer0's flag, then PORTA bit2. If bit2 is set, it skips the divider and elapsed counter and branches to `0f3a`. It does not accumulate elapsed heartbeat counts while disabled. |
| Enabled power | `0f0f..0f1e` clears the Timer0 flag, decrements the divider and reloads61 on underflow. `0f22..0f34` saves the old elapsed counter, increments it, and compares the old value with61. |
| Heartbeat expiry | `0f35=0020, 0f36=150c` disable power. `0f37=0023, 0f38=01a4, 0f39=01a5` clear the elapsed counter. The next instruction, `0f3a=0064`, clears the hardware watchdog. Execution continues in the application loop. This path has no RESET instruction. |
| Separate reset command | Command `07` selects `0ab8..0aba`, which sets a separate flag at `016c`. The main loop checks this flag at `0f4b..0f50` and reaches `0f66=0001`, the RESET instruction. Heartbeat expiry does not set this flag. |

These paths support pausing the model's remaining heartbeat interval while
power is disabled. Repeating ENABLE must not reset elapsed time. A received
heartbeat resets the interval and leaves power unchanged. Expiry cuts power,
clears the elapsed count and leaves application commands available. Only an
explicit enable command restores power. This is not automatic recovery.

The contract still approximates time continuously. It does not reproduce
Timer0 phase, the pending overflow flag, instruction latency, the divider's
initial phase, or the oscillator and configuration words outside this image.
The exact first interval and interval after cutoff remain uncalibrated. This
change does not extend the declared10-second interval. A complete PIC CPU
and factory-storage path remains required for claims that need it.

Run the static listing command from the repository root with a new output:

```sh
cargo build --release --offline --locked -p md-sim-firmware --example pic16_static_listing
target/release/examples/pic16_static_listing simulator/firmware/pic16/braiins-s9-app.txt work/simulator/pic16-static-001
```

The command above was tested. Its output directory now exists. Select another
new directory for a repeat. The output records the tool, source, input and
listing hashes. Decoder tests use literal data-sheet opcode encodings. A
separate author review and an independent disassembler comparison remain open.

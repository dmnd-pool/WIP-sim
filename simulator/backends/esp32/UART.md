# ESP32-S3 UART register observations

A UART sends serial data. This development change corrects three S3 register
observations in the archived Espressif QEMU backend. It is not a complete
serial line model or an accepted physical board.

The inherited ESP32 model reports its DATE constant at address`0x78`. On S3,
that address is CLK_CONF. The S3 DATE register is at`0x7c`. S3 CLKDIV also
has a12-bit integer field, rather than the inherited20-bit field.

The separate S3 patch provides these register contracts:

| Register | Offset | Stored-bit mask | Device-reset value |
| --- | --- | --- | --- |
| CLKDIV | `0x14` | `0x00f00fff` | `0x000002b6` |
| CLK_CONF | `0x78` | `0x0fffffff` | `0x03701000` |
| DATE | `0x7c` | `0xffffffff` | `0x02008270` |

The exact manufacturer reference is ESP-IDF5.5.3 commit
`2c211b236707889e8400c4dc5644dd5c4ee071e0`. Its S3 uart_reg.h SHA-256 is
`eef72592d7949e641faf7c1d153a8bb0eb0e851619a5f579b4a3c3c663d67e07`.
The archived backend source commit is
`febae182e132e4055529be423a818225ebddaa3a`; its source archive SHA-256 is
`5456034eb8a9f348b1f4c8820dc79f55677bddfe0d1c66566b6c42ceb9df66f0`.
The [source review](../../../work/simulator/reviews/esp32-uart-independent-source-506.json)
checks the original archive members, patch and register definitions.

The C change uses the existing QEMU UART class, memory callbacks and reset
phases. It leaves the ESP32 parent source unchanged. The S3 reset calls the
inherited reset before it restores CLK_CONF and DATE. C is used at the
existing emulator interface. The probe and result checks use Rust.

Selected writes emit the canonical device path, QEMU virtual nanoseconds,
sequence, address, access size, raw value and stored value. Each device can
emit256 write records and one overflow record during its lifetime. Reset
does not renew this budget. Overflow means incomplete capture. It cannot
support a complete firmware capture claim.

The [old-backend regression audit](../../../work/simulator/reviews/esp32-uart-independent-baseline-506.json)
checks the failed test on unchanged017. All26 earlier controller transactions
match;12 of the20 new UART responses differ. This proves the before state.
The [corrected018c tests pass](../../../work/simulator/reviews/esp32-uart-executed-506c.md).
All46 register responses and332 reset/timer responses pass, with clean
runtime diagnostics. The deliberate overflow remains an incomplete capture.
Original040 also captures26 actual clock/divider writes before its unsupported
ASIC read. It supplies no job or nonce. The report links the failed before
states, exact inputs, independent reviews and verified commands.

The inherited baud calculation still uses a fixed40MHz approximation. TX
drains immediately, and RX throttling does not model each character. Clock
gates, reset-pin effects, framing errors, migration, physical clock calibration
and communication with the ASIC still need implementation and tests. Stored
clock requests do not establish effective line timing. The full board and
mining gates remain open.

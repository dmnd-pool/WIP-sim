# T17 readback analysis 442

The retained T17 driver separates the ASIC register value from a software
`FOUND` flag. The flag is **not** an ASIC ready or lock bit. The existing
BM1397 reply codec already carries register `0x40` as a raw 32-bit value.
No second UART reply codec is needed.

This analysis uses only the retained executable, disassembly and symbol table.
No build, VM, vendor-code execution or simulator source change occurred.
The proposed vectors below are static predictions, not oracle results.
The original probe 035 core-write boundary and the full milestone gaps remain.

## Pinned ranges

The executable SHA-256 is
`ad090e7abbf525a7a8a23c7153529e45af94b864e1170e677e1f85b4c4091367`.
Its first load segment maps file offset 0 to address `0x8000`; it contains
all selected code and strings. The ELF header and segment fields were read
from the saved bytes. [The range record](t17-readback-ranges-442.json) retains
the exact bytes, hashes, original offsets and disassembly for these ranges:

| Range | Virtual address | File offset | Bytes | SHA-256 |
| --- | --- | ---: | ---: | --- |
| Response collector | `0x1c0c4` | 82,116 | 916 | `a6fa7c53118e2e3ca140a56f086044085e57d750b7e6a98873c024409abd3445` |
| Core-0 formatter | `0x238f0` | 112,880 | 1,024 | `8d3b1b4e80ef91084c23018fc6055b6bd5249b0e0f91119919f2fb20ef0f8245` |
| Core-register dispatcher | `0x24f64` | 118,628 | 152 | `1da1e6d3fe663c20344fd7841ebd4a8ba25117e90a9e6ad55ccd98b22758a403` |
| Core-read sequence | `0x251c4` | 119,236 | 548 | `0319ab765f8335d070234c1cf97524138fa6c7e1702c86331a1105f43218c7db` |
| Unsigned division helper | `0x75be8` | 449,512 | 168 | `81c5ee0515ea5584984332c432a85952fa793bc7be049e82b188da86226d613f` |

These ranges include required literal pools. For example, collector bytes at
`0x1c438..0x1c457` and formatter bytes at `0x23cb8..0x23cef` are data.
The prior linear disassembly labels some of them as instructions; this report
does not treat those labels as executable control flow. The exact format and
status strings are separately retained in the range record.

## Driver layout facts

The read sequence clears 64 bytes and uses eight slots, each eight bytes long.
For each core-register number 0 through 7, it sends the existing mailbox read,
collects register `0x40`, then calls the corresponding formatter.

The collector writes the returned 32-bit value at slot offset 0. It writes
the byte value 1 at slot offset 4 on its completion path. The calling sequence
initially clears that byte. The formatter uses it to select `FOUND` or
`NOT FOUND`. Neither byte is obtained from a hardware ready register.

The core-0 formatter reads the raw value, then prints:

- `raw >> 16` as the reported core field, without a 9-bit mask or doubling.
- `raw & 0xff` as the core-register byte. Bits 8 through 15 do not contribute.
- The six fields at `[7:6]`, `[5:4]`, `[3]`, `[2]`, `[1]` and `[0]`.
  It prints the reserved bit too; it does not reject it.

These are direct operations at `0x23908`, `0x23938`, `0x23a4c`, `0x23ab8`,
`0x23b24`, `0x23b90`, `0x23bfc` and `0x23c68`. The literal strings identify
the field names. They do not establish the effects of changing those fields.
The reported core field must remain separate from physical core geometry.

The collector reads an already processed queue, not a nine-byte UART frame.
The queue base is `0xb52588`; its read index is at offset 4 and count at 8.
Entries start at offset 12 with an eight-byte stride. This collector consumes
the word at entry offset 0, logical address at 5, register at 6 and chain at 7.
It divides the logical address by a configured nonzero byte before comparing
the requested ASIC index. The exact division helper is retained.
The queue producer and its wire-to-queue conversion were not traced here.
Thus this layout must not replace the existing independently checked UART
endianness or CRC rules.

## Apparent vendor selection defect

The static control flow predicts a useful negative test. With a matching
register but a different chain or normalized ASIC index, the collector reaches
`0x1c228`, sets its software completion value to 1, consumes the queue entry,
and then writes `FOUND=1` at `0x1c3fc`. It does not reach the value copy at
`0x1c41c`. A caller's cleared or stale value can therefore remain while the
software flag says `FOUND`.

This is an **unexecuted static prediction**, not a confirmed runtime defect.
It shows why the driver flag alone cannot be used as evidence of valid ASIC
readback or readiness. A future oracle must retain the original behavior,
including this case, rather than repair the vendor routine inside the test.
It does not justify copying the apparent software defect into an ASIC model.

## Small next implementation and oracle

[The planned vectors](t17-readback-vectors-442.json) contain eight formatter
cases and eight queue-selection cases. They explicitly mark their outputs as
static predictions. The formatter plan also covers 5,120 combinations of
data byte, reported core field, ignored middle byte and software status.
The queue cases include wrong register, wrong chain, wrong ASIC, a later valid
entry, empty queue and index wrap. They use no computed or fabricated ASIC work.

The smallest useful implementation path after the active frozen run is an
additional **vendor driver oracle**, using the existing ARM QEMU and small
C ABI wrapper approach from 274. Keep the retained routines at their original
virtual addresses, verify their unchanged bytes and capture their literal
pools and strings. Bounded test callbacks can replace only the listed external
library/driver boundaries:

- Formatter: `fopen` at `0xa2fc`, `fprintf` at `0xa638`, `fclose` at `0xa248`
  and `fputc` at `0xa668`. Record call arguments; do not claim real log services.
- Collector: mutex calls at `0xa458`/`0xa404` and the delay wrapper at `0x6269c`.
  Record delay requests without claiming physical elapsed time. Execute the
  original unsigned division helper with an explicit nonzero divisor.

This is an adapted function-level test, not a substituted miner. It requires
no new CPU emulator. Compare results with the prepared expectations, but save
any mismatch as a failure. The test should check that the caller slot and
queue state change only as observed in the unchanged routine.

If that oracle confirms the formatter rules, a small explicit semantic view
of a core-0 readback word is supportable. It must take the pending requested
core-register context; a generic `0x40` response does not identify register 0
by itself. Keep the original 32-bit word, reported core field, data byte and
decoded field values. Reuse `Reply::Register`; do not create another frame
codec or invent a valid hardware status bit.

This view would help later model validation. It would **not** implement the
`0x80008074` write effect or advance original probe 035. Core fan-out, reset
values, mailbox completion, clock effects and independent physical timing
remain unresolved. The next hardware acceptance input is still a pinned
chip contract or independent transaction trace for the declared stepping.

The principal inputs and new records are pinned in
[the audit manifest](t17-readback-analysis-442-inputs.json).

# Timing contracts recoverable from retained S9 firmware

Yes. Reverse engineering can recover much of the required digital behavior.
The exact archived firmware contains register addresses, write order, masks,
poll conditions, retry bounds, clock divisors and requested delays. We should
extract and test these contracts. The missing FPGA source is not a reason to
stop that work. Firmware requests alone do not measure the hardware response.

This review read retained files, existing instruction traces and small binary
ranges. It did not build, run firmware, start a VM, download files, retry the
inaccessible source server or change model behavior. No calibration or exact
physical target pass follows from it.

## Concrete next extraction: the selected SPL initialization interpreter

SPL is the first downloaded boot stage, after the internal boot ROM. The
selected SD release is artifact `braiins-feed-4366569e66ab54bc0d15`. Its retained
bundle records the 112,197,632-byte SD image as
`97a257571af4fb9e62f709e2d6b8f36a191fdd07f1cd12ae780bd1c73af7eaef`.
This review rehashed the extracted `boot.bin` and its unchanged SPL suffix:

| Input | Bytes | SHA-256 |
| --- | ---: | --- |
| `original-zynq/sd-bundle-291/boot/boot.bin` | 79,263 | `950d19c6e5bfd9d8107def130b19fc9a5f64d824b26c0c493d5fdfe8503edac4` |
| `original-zynq/sd-bundle-291/boot/spl-payload.bin` | 77,023 | `765efb23844156e976ba785c24a6a753c664504a499c64581d1c356a3f4c02df` |
| `original-zynq/sd-bundle-291/boot/bundle.json` | 7,553 | `a7f940cb1ee4e690e5027d518768d8be6d98cc3c14e4493f9fbc21d17b4fb129` |
| `original-zynq/spl-boot-006/backend.log` | retained instruction trace | `8c663e5be47359da8a8be4fb8aee83fa1a42e0ac923275a43195b5739d422f0a` |

Paths in the tables are relative to `work/simulator/`. SPL file offsets below
are also virtual addresses in the retained direct-entry diagnostic. Add 2,240
to obtain offsets in `boot.bin`. The unresolved boot-ROM header length and
handoff remain separate; this mapping does not close the original reset gate.

The existing trace already executes the table interpreter at `0x4ac`. Its
instructions read the low four bits as the argument count, and the remaining
bits as the operation. The branch table at `0x4f0` selects stop, clear, write,
masked write, masked poll and timer delay. Examples present in the exact bytes
are `0x22` for a write with two arguments, `0x33` for a masked write, `0x42` for a
masked poll and `0x52` for a timer delay. This is a bounded extraction target;
we do not need to reconstruct the whole FPGA to read it.

| Range in SPL | Length | SHA-256 |
| --- | ---: | --- |
| Interpreter and constants, `0x4ac..0x5c0` | 276 | `e5dac9552205e2a27d8da3fa113f5e83c99945cb35afa3507ab02a4a90a5da1b` |
| Revision dispatch and table pointers, `0x5f4..0x734` | 320 | `4b47742567604ce254bda655676cd7e0400442039f535496a2425c43d8db9666` |
| Clock table, `0x1071c..0x107e8` | 204 | `741e08636cf9195f7c27a543c7b6ac76def7211df2e0aec3925f6a413f173084` |
| PLL table, `0x10e34..0x10fb4` | 384 | `aa3ee1ee22142fcee73f5dc86dc2be15c20a8b1ad9219092d3a8a7c4a657de35` |
| Peripheral table, `0xee4c..0xef78` | 300 | `f5b4c1a9a0e55793b1f1bf00b25d0263c43596e6424e7d0e25b87092894e5543` |

The revision reader at `0x460` reads the high four bits of `0xf8007080`.
The retained adapted trace takes the branch for a value other than 0 or 1.
That branch selects the PLL, clock and peripheral tables above. This is a
property of that run; it does not identify the real controller's silicon
revision. A complete extractor should retain all three dispatch alternatives.

### Exact decoded timing facts

- The PLL table unlocks SLCR at `0xf8000008` with `0xdf0d`. SLCR is the system
  control register block. It programs the ARM, DDR and IO PLLs at `0xf8000100`,
  `0xf8000104` and `0xf8000108`. Their feedback fields are 40, 32 and 30. Each
  sequence sets bypass bit 4, asserts then releases reset bit 0, polls the
  corresponding bit at `0xf800010c`, then clears bypass. Poll masks are 1, 2, 4.
  ARM clock control at `0xf8000120` receives masked value `0x1f000200` with mask
  `0x1f003f30`. These are the firmware's writes and ordering.
- The poll body at `0x544..0x568` uses `TST` and succeeds when **any** selected
  bit is nonzero. Its counter literal at `0x5bc` is `0x05f5e101`, or 100,000,001.
  Each failed read decrements the counter. Exhaustion returns 2. The failure
  instructions at `0x55c..0x568` are `e2533001 1afffffa e3a00002 ea00000f`.
  The existing trace shows the successful branch; timeout behavior has not
  been independently executed in this review. An iteration limit is not a
  measured number of nanoseconds: CPU speed and MMIO latency affect it.
- Clock-table writes at `0x10798` and `0x107a8` select FCLK0/1 controls
  `0xf8000170` and `0xf8000180`, each with mask `0x03f03f30` and value `0x00200500`.
  The retained Linux clock driver interprets these as mux selection 0 and
  divisors 5 and 2. The SPL literal at `0x26c` is 33,333,333 (`0x01fca055`).
  Together with that nominal input and IO feedback 30, this
  requests 99,999,999 Hz at this boot stage. It does not prove the clock after
  later software writes or inside the selected FPGA's clock network.
- Timer delay code at `0x56c..0x58c` uses literals `0x27bc86a4` (666,666,660)
  and `0x10624dd3`. It multiplies the first by the requested argument with
  32-bit wrap, multiplies that result by the second into 64 bits, then shifts
  right 39. Argument 1 therefore gives 333,333 timer ticks. Do not replace this
  exact integer rule with an unbounded mathematical formula for other inputs.
- The helper at `0x474..0x4a4` writes 0 to timer control `0xf8f00208`, clears
  low/high counter words at `0xf8f00200/204`, then writes 9 to control. The delay
  loop reads the low counter until it reaches the computed threshold. The
  peripheral table contains six successive `[0x52,0xf8f00200,1]` records at
  `0xef2c..0xef74`. These are six requested delays of 333,333 ticks. Calling
  them approximately 1 ms each assumes the intended timer clock; it is not a
  measured duration or a physical PLL/rail settling time.

These values were read with `od -tx4` from the exact suffix and checked against
the retained translated instructions. The next bounded implementation is a
Rust inspector which pins these code ranges, follows the three table choices,
checks operation/argument bounds and terminators, and emits exact register
operations and tick/iteration contracts with source offsets. Reject unknown
operations and malformed tables. Reuse the existing isolated inspection and
clock-test paths. Add independent negative tests for changed masks, truncated
records, absent terminators and the distinction between any-bit polling and
all-bit polling. A later original-code test should exercise delayed-ready and
timeout exits without changing the original routine or inventing ready status.

This creates a selected-firmware regression input. It does **not** require a
new CPU emulator or a second clock implementation. It can compare selected
SPL operations with retained MMIO/clock traces and expose a model that reports
ready too early. It cannot alone determine the correct physical lock delay.

## Work already implemented: do not repeat it

Fabric-signals276 already checks 242 transactions, 111 clock states and 7 raw
reset states. The four SLCR outputs implement source/divider/static gating and
active-low raw reset. The reset work also clears throttle state. UART282 checks
FCLK1 gating mid-byte, preserved FIFO/UART state, half-frequency resume and
independent FCLK0 behavior. The later ASIC clock295 work separates ASIC time
from fabric edges and keeps functional hashing and rate caps separate.

| Existing evidence | SHA-256 rechecked here |
| --- | --- |
| `qemu-s9/fabric-signals-276/assessment.json` | `501625f59e81c7b1c795cdd84c909fef3868bb703f04aa8d7a3e6451dcb890c3` |
| `qemu-s9/fabric-uart-ubsan-282/assessment.json` | `a9c01e5ea20d4916b450fe1b4590e1fa786140fb4a99a526f7d2d6984760fb6e` |
| `reports/asic-clock-295.json` | `b51294eb49925f4eb5e1ab7b96d2e7ffa40346ef99a47fd73afeda49614ba189` |
| Repository `simulator/backends/qemu/s9-fabric.patch` | `db0c90eb80d0efe986fd66843ef14f21f8fe8ca45bbbda6631461638ce5fb6f8` |

The new gap is the exact SPL table/timeout contract and its binding to those
existing mechanisms, not implementing FCLK or reset again. Raw SLCR reset
is still different from the FPGA's downstream synchronizer and reset sequence.

## What the other retained sources can and cannot establish

The archived legacy Braiins source has concrete software delays. In
`braiins-fpga-source-005/members/open/bosminer/bosminer-am1-s9/src/lib.rs`,
lines 80–84 define 100 ms inactivation, 1 s initialization and 2 s retry delays.
Lines 442–452 hold reset, disable voltage, wait 1 s, enable voltage, wait 2 s,
release reset and wait 1 s. `power.rs` defines 500 ms after the PIC reset command,
100 ms for several other commands and a 1 s heartbeat period. `io.rs` polls
command state with a 1 ms software delay. These are recoverable caller contracts.
The source archive hash is
`dc5f58721f596d2fc36b50d5049461264986ac339c4b4ff4d383749b98f3a363`.
Rechecked member hashes are:

- `lib.rs`: `1eaa099131265a837c3e3ebbe03674fe22193206573a69fa87e08c035d522a6e`.
- `power.rs`: `43b9deb299f657bcd7fb42c4ef2668c62af643e81d2b0aba1b78d37350479cd7`.
- `io.rs`: `0b59792f50117f01f8784c9eebdba451d24f2dbe6955fa61116ce6a74ca00481`.

This legacy source also assumes a 50 MHz FPGA core, 25 MHz chip oscillator and
specific baud rates. It is **not** proven to build the selected 2020 FPGA or
the selected archived miner. Do not transfer those constants by family name.
Match selected executable instructions and observed transactions before using
them as that exact guest's software contract. Even a matching 500 ms sleep would
show when the driver retries; it would not prove that a PIC is busy for 500 ms.

The retained Linux `clkc.c` explains clock mux/divider fields; its hash is
`e0b7a555053c21908d29fe2d065172dcb5f8c381fc3f7c06ad60ecdef5ee2fc2`.
Its companion `pll.c`, hash
`1218ce234c896d7122edecd33acd54f7021fe3c8fb0665ad3c9b16b957f9a75c`,
clears reset/powerdown and waits for the lock bit. That wait is unbounded in
the source. It specifies the condition, not the analog lock time. Both member
texts were rehashed from `kernel-clock-source-211/references.json` (record
hash `3b9d396d70b05d50e51574fcc4206bc0d717982c8c7523b51d6213c785ddd33f`,
archive hash `5aa5c0915c93c6078984b13367edd12670e8ad25d922d352654f768d7005db8f`).

## FPGA and physical evidence: remaining distinctions

The selected bitstream is
`980532de5eb666508dafb111466f30169a70b8b6ea662bdda61e17dbd55a4421`;
its complete configuration stream is
`49d0faa400fbc8bed3f3e651da2c6da5f3184a08f942ec95db59c2526dcbea78`.
Frame addressing, CRC and ECC have been checked. A bitstream contains actual
configuration, so validated feature decoding and logic reconstruction are a
real alternative to obtaining the lost source. This could recover counters,
state machines, reset pipeline depth, clock resources and routing. It requires
the complete matching feature/connectivity databases and independent checks.
Part geometry and valid frame checksums do not provide those meanings.

The already identified Project X-Ray route remains useful beyond `e80262a`:
the missing pinned tilegrid has expected publisher hash
`db16874f2827fc05248ad4a7ef5769deaa8e70158a60c8dd40194c48713479ee`.
Matching tile, feature and connectivity data are also needed. Public binary
history or public forks can identify a different source/build association if
their **complete configuration bytes** match. The archived 2019 binary differs
in 1,964 selected frame ranges; resolving its private source link alone will
not establish selected 2020 equivalence. No failed remote query was retried.

The raw reset connection in legacy `system.tcl` and generic PG164 reset
documentation do not identify selected generated IP parameters. The retained
G19 hardware metadata belongs to other boards. No G19 width or default value
should be silently assigned to S9. See [reset reference302](../qemu-s9/reset-reference-302.json),
hash `5f11510659399ad5f4ce2a105e86f7ce4646e648ac4494e593cb63a2723a2f62`,
and [exact dependency422](milestone2-next-dependency-422.md).

Actual oscillator tolerance, PLL lock latency, rail ramp/settling, reset pulse
quality, propagation delays, fan/sensor inertia and behavior across voltage,
temperature and part variation need independently identified device evidence.
Datasheet bounds and validated generated logic can establish some digital
limits; measured traces are needed for calibration claims. Firmware may give
safe upper waits or operating assumptions, but many different physical boards
can satisfy the same waits. It cannot uniquely reveal an installed PSU suffix,
PCB revision or an analog waveform that it never measured.

The specification explicitly separates source-derived expectations from
measured calibration. Reference traces must identify the device, board,
firmware, capture method and clock uncertainty. Thus we can continue concrete
reverse engineering and functional model work now while keeping physical
calibration and unresolved exact identities open. The SPL extractor above is
the next small step with complete local inputs and no dependence on access to
the private FPGA repository.

# Selected S9 Bosminer MMIO evidence

The exact archived 2022 Bosminer ELF was extracted without execution. The
isolated Capstone decoder then checked 2,604 bytes from three function ranges.
Each range matches consecutive entries in the original ARM unwind table.
The decode completed in 31 ms, with exit code zero and no truncated output.

The selected ELF is 12,699,280 bytes, SHA-256
`4997a1c10a09f6881d6f5e0d4967eb31072ea9f1da7a80dac192ae1299d45960`.
Its [member record](../qemu-s9/axi-bosminer-extract-511b/member.json) binds it
to the exact root filesystem. The [extraction audit](bosminer-extraction-independent-execution-511b.json)
passes. The [static input record](maxigp0-inputs-511.json) also pins the selected
DTB, reconstructed netlist and PS7 port table.

These commands were tested from the repository root. Use new output paths:

```sh
work/simulator/qemu-s9/axi-inspection-driver-511b/s9_native_service \
  extract-bosminer work/simulator/original-zynq/attempt-003/components \
  work/simulator/qemu-s9/axi-bosminer-extract-511b
work/simulator/qemu-s9/axi-static-driver-511c/s9_bosminer_inspect \
  work/simulator/qemu-s9/axi-bosminer-static-511c
```

The second helper checks the retained extraction by its exact hash. It decodes
only the three reviewed ranges. The original T17 decoder API and the new range
API also passed a literal instruction regression inside the same isolation.
The host unit tests do not load the foreign decoder.

The [contract record](../../../simulator/firmware/s9-bosminer-mmio-511.json)
retains each cited instruction address, file offset and byte sequence:

| Operation | Original instruction evidence |
| --- | --- |
| Read version at common offset `0x00` | `0x47b630` |
| Read build timestamp at common offset `0x04` | `0x47b670` |
| Clear then set control bit 3 at offset `0x08` | `0x47b60c` through `0x47b624` |
| Write S9 baud divisor at offset `0x10` | `0x3c0828` through `0x3c0830` |
| Write common baud divisor at offset `0x10` | `0x47b264` through `0x47b268` |

The version branches require `(value & 0xfff0ffff) == 0x00901002`.
Bits 19 through 16 are not checked by this branch. The build timestamp must be
at least 1,546,300,801 and at most `0x7fffffff`. This evidence supplies a check
for a real FPGA response. It does not supply a response value to the device model.

Both baud functions pass the literal 100,000,000 to the calculation at
`0x474180`. That callee is outside this decode. No divisor formula is inferred.
The selected netlist connects FCLKCLK1 through its sole global clock buffer
to MAXIGP0ACLK. It consumes FCLKRESETN0 in the reset logic. These are separate
static observations. They do not prove a measured clock rate or reset success.

The [freeze](../qemu-s9/axi-static-driver-511c/manifest.json) contains 401 source
entries, 11 affected Rust tests, strict Clippy and the exact release helper.
The [process result](../qemu-s9/axi-bosminer-static-511c/result.json),
[decode report](../qemu-s9/axi-bosminer-static-511c/inspection.json) and
[instruction text](../qemu-s9/axi-bosminer-static-511c/a32.txt) retain the actual
output. Source reviews pass for the [range API](bosminer-static-independent-source-511c.json)
and [old API regression](bosminer-static-legacy-independent-511c.json).
The [independent execution audit](bosminer-static-independent-execution-511c.json)
also passes. It checks every decoded position, all cited instruction bytes
and literals, and the original ELF range boundaries.

This is static evidence. Embedded data still appears in the linear decode.
No ARM firmware instruction or selected FPGA transaction ran in this step.
The next gate is the bounded MAXIGP0 signal test with real clock, reset and
response behavior. Original-board and full-matrix gates remain open.

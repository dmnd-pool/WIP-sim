# BM1397 continuation audit 437

The first unsupported command in original probe 035 is still the broadcast
write to ASIC register `0x3c`, with word `0x80008074`. The newer T17 evidence
resolves its encoding. That codec is already implemented. The retained
vectors do **not** establish the core-register effects needed to make the
live model accept this command.

This was a bounded read-only audit. No source, firmware, catalogue or prior
result changed. No build, firmware run or VM was started. This is additional
matrix work; it does not change milestone 2 or establish an exact target.

## Actual failure and source association

The retained 035 result is SHA-256
`bed8b61a73821458acb3f6b5ced8c1fcc0170a804dc88a917b105321d094b376`.
Its input manifest, process specification and assessment still match the
hashes in [the stage review](../bitaxe/original-probe-035/stage-audit-root.json).
It exited 1 without trace truncation. UART captures 16 and 17 contain:

```text
55aa5109
003c8000807410
```

The complete frame is `55aa5109003c8000807410`. Its decoded event identifies
broadcast, address 0, register 60 and value 2147516532. The next event reports
`UnsupportedCommand`. This is an observed boundary, not a predicted one.

The original factory image hash was rechecked:
`3fd36298681d7797429dd72610041d731558c5253430902a3c7feeced232557c`,
15,802,368 bytes. The separate ESP-Miner source archive was also rechecked:
`ccba9a695c3661a7d536cd649c0dcd6935cb0f9d59d24ff667391d65398ce069`,
900,415 bytes, commit `64680f8a4da0b9a3b532051f0aa18429fcf04e82`.
Its extracted `components/asic/bm1397.c` text hash is
`9bd28dff43cb3c07a63c4843dba2704f7e071b1bfc7fb310cfd925cb35be2f56`.
The saved text was rehashed without adding a newline.

That source's lines 214–215 send precisely the same register and value.
This connects the observed command to the source sequence. It does not prove
that the complete factory image is a reproducible build of that source.
The catalogue associates both with release v2.14.2; matching release names
alone would not prove such a build relationship.

Probe 035 retains the original ADC calibration errors. Its 0.55 V SAR input,
power/reset assumptions and support-chip values are explicit diagnostic
inputs. This audit does not reinterpret them as calibration, regulator or
physical-board results.

## What the independent T17 evidence proves

The T17 archive hash was rechecked as
`89677e26b8fb4b25cd8b3157b0a73b4df88f10f9994f0e7f8102af9d11f99772`.
The retained extracted ARM executable was rechecked as
`ad090e7abbf525a7a8a23c7153529e45af94b864e1170e677e1f85b4c4091367`.
The [262 inspection](../bitaxe/t17-static-262/inspection.json) connects the
archive, nested firmware images and executable. It is a static inspection.

The read-packing routine at executable offset 117,936 and write-packing
routine at 118,000 are each 64 bytes. Their rechecked hashes are:

- Read: `57a96a1cf41038322a9ce46cf615bf161fdc528f5089a4c0e50bfeff4266b715`.
- Write: `6914a2d19ac20c011eb801a32b04fc859604490ed2b83451968038414a8ecc89`.

The copies in the retained oracle 274 executable at offsets 296 and 360 have
those same hashes. Both oracle 272 and 274 retain the identical 196,608-byte
output, SHA-256
`7527abdd4015bbafa0dc1121733a9ff20c37799e142e11b6d2c69f57b45745d1`.
The checked-in fixture has the same bytes. Its manifest pins **274**, not 272;
274 adds the output identity and input-order fields to the assessment. The
274 assessment hash matches that manifest:
`8bab86b7d1c5f8f42ec3cfe301a79383069c761d0222b3ca50fbb63e31e97d71`.

The 49,152 outputs cover 32,768 read and 16,384 write inputs. They establish:

```text
read  = ((core_argument >> 1) & 0x1ff) << 16
        | (register & 15) << 8 | 255
write = 0x80008000 | (register & 15) << 8 | (value & 255)
```

The exact failing word occurs at output byte offset 131,536. It decodes as
`Write { register: 0, value: 0x74 }`. The selector is an encoded driver field;
the 512 possible values do not establish physical core count or pairing.

This work is already present in
`crates/md-sim-firmware/src/bm1397/core_control.rs:24`, with exhaustive fixture
comparison starting at line 66 and an explicit `0x80008074` test at line 103.
Its rechecked source hash is
`0f25fcaf58c0e3bf3df7f3080bda65c450439283a83da0283f2276a8d47b4a63`.
Do not repeat the packing implementation or rerun the oracle merely because
the older progress file ends before this work.

The live path still excludes `0x3c` in
`crates/md-sim-firmware/src/bm1397/identify.rs:123`.
Its current source hash is
`6ef618637a2aedd49788b190ec5aec22f97c5395e229048d8b08cba72725a397`.
It differs from the model-source hash retained by probe 035. No historical
execution result is silently assigned to the current source.

## Precise missing semantics

The T17 strings label core register 0 fields as `CCDLY_SEL[7:6]`,
`PWTH_SEL[5:4]`, `HASH_CLKEN[3]`, `MMEN[2]`, reserved bit 1 and `SWPF_MODE[0]`.
The value `0x74` selects bit values 1, 3, 0, 1, 0 and 0 respectively.
These labels and packing outputs do not establish all effects of those bits.

The live model still needs evidence for:

1. Which internal cores receive the write. The outer packet broadcast selects
   ASIC chips. It does not alone resolve the internal write fan-out.
2. The core-register reset value, valid/reserved bits and write effects.
   Retaining `0x74` in a new array would not implement its clock and mode effects.
3. Mailbox completion, busy/error behavior, overwrite rules and response
   behavior when another command arrives before completion.
4. The link from a core read command to register `0x40`, including response
   value, selected core and whether/when a spontaneous response occurs.
5. Power, reset and clock-stop effects on pending operations. The hash-rate
   cap must not determine these interface timings.

The retained community view reports `0x3c` reset `0x00004000`, `0x40` reset
zero and possible unsolicited register-value replies. It also says that
reverse engineering of this register is incomplete. That view has no pinned
upstream commit or silicon stepping. It cannot fill the missing transitions
by itself. Its retained SHA-256 remains
`501c855b43ecf49a37575766454d1e4e439d0911f1b0c2f5bd2bef287642fe67`.

The independent oracle contains only packing functions. It has no ASIC, no
mailbox register bank, no clock input and no measured readback. A no-op write
or an unconditional success reply would bypass the actual missing behavior.
It must not be used to advance the unchanged firmware past this boundary.

## Concrete next bounded step

Continue from the already retained T17 executable and disassembly. Trace the
core-read path at virtual address `0x251c4`. The retained instructions call
the read packer through `0x24e9c`, then pass register `0x40` to `0x1c0c4` and
dispatch the returned data through `0x24f64`. That is a direct lead for the
driver's response selection and byte layout. The `0x24f64` dispatcher sends
core register 0 data to `0x238f0`. Pin each complete function and required data
range before using a new isolated oracle. Do not treat linear disassembly
alone as an executed behavioral oracle.

This can resolve more of the driver-side readback contract from existing
bytes. It still cannot measure the ASIC's reset state or completion delay.
For that, the concrete missing input is a pinned chip-level contract or an
independent trace of reset, the captured `0x80008074` write, selected-core
readback and clock effects for the declared BM1397 stepping.

Once those effects are supported, a small separate `core_mailbox` module can
own per-chip state and pending transactions. `IdentificationChain::receive`
can call the existing decoder and apply the addressed transaction. Use the
resolved model clock, preserve reset/power transitions, and reject unknown
forms. Add behavior tests against those independent traces before enabling
the command in a new original-firmware probe. No lock or readiness response
is justified by the present vectors.

The archived ESP sequence sends the already supported ticket write `0x14`
next, then `0x68=0xc0700111` for PLL3. The PLL 450 MHz source vector recorded
in 241 concerns the separate frequency calculation used for PLL0 register
`0x08`; it does not establish PLL3 behavior or physical lock time. That is
the next visible source-level boundary after the mailbox, not a mining pass.

The principal evidence and implementation inputs are pinned in
[the audit input manifest](bm1397-core-continuation-437-inputs.json).
No new ASIC test or firmware execution result was created in this audit.

# M69A development NAND profile

This is a separate development profile. It does not resolve the NAND part on
the selected physical S9 board. The EC/DA fixture remains unchanged.

The protocol reference is Micron M69A, PDF09005aef83b83f42,
`m69a_2gb_nand.pdf`, revision K01/11. Its public copy is at
<https://community.nxp.com/pwmxy87654/attachments/pwmxy87654/imx-processors/12466/1/352-MT29F2G08ABAEAWP.pdf>.
The provider can read its text. Original PDF bytes are not yet available to
hash. This reference is separate from the older AACWP part and the newer
automotive datasheet. The source observation record is under
`work/simulator/qemu-s9/`.

The profile supplies the five ID bytes `2c da 90 95 06`, the ONFI signature,
three 256-byte parameter copies, and feature addresses01/90 in their default
mode. Geometry is2048 data bytes plus64 spare bytes,64 pages per block,
2048 blocks and one logical unit. Parameter byte101 is23h: two column and
three row address cycles. The CRC is calculated over bytes0–253 with ONFI's
4f4e initial value and8005 polynomial.

ID bytes after the published fields read asff in this development profile.
This is an explicit unassigned-output policy, not additional physical ID
evidence. Such reads do not falsely report an array program failure.

The parameter page is derived from the public table. It is not a dump from a
physical chip. Its vendor suffix uses the visible13-byte prefix and fills
the rest with zero. Exact vendor bytes remain uncertain. The capability
fields retain the table's part values; they do not prove that this partial
model implements all operations of that part.

Parameter reads use25µs. Feature reads use1µs. A feature write goes busy only
after four real bytes, for1µs. These are documented maximum times, not
measured calibration. STATUS preserves the transfer cursor. READ MODE00h
resumes parameter/feature reads after status. NAND RESETffh discards the
transfer. Controller-only reset preserves the chip busy deadline. Global
QEMU reset clears the identification transfer with the child NAND reset.
RANDOM DATA READ05h/e0h in parameter mode uses the supplied two-byte column
to select a parameter byte. It does not fall through to array storage.
Columns beyond the three implemented copies fail visibly.

Only normal array mode with internal ECC disabled and timing mode0 is
implemented. Other feature values cause a visible failure status and a
trace fault. Readback stays at the actual supported value. ECC enable, OTP,
cache, multi-plane, unique ID, alternate timing, drive strength, wear,
calibrated electrical behavior and migration remain incomplete. This must
not count as a complete physical part model.

Array storage reuses the separately recorded QEMU NAND engine and timing
patch. Its300µs program and2ms erase intervals remain the development
baseline; these are not the M69A typical200µs/700µs timings. The declared
intervals are within its600µs/3ms maxima. The backing format remains every
complete2112-byte page, including spare data. No physical NAND is accessed.

The Rust core owns identification, bounded transfer state, feature validation
and CRC. C is only the adapter for the existing QEMU NAND bus API. The
machine name is `md-sim-s9-m69a-development`. No original-board acceptance
or mining claim follows from this profile.

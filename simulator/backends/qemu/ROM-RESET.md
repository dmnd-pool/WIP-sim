# Public Zynq ROM reset diagnostic

This optional mode runs the existing QEMU Cortex-A9 implementation. It does
not add a CPU emulator. The existing direct-SPL path stays unchanged.

The exact public ROM is128KiB, SHA-256
`c58e2bc1d837654457ed47064accb26c4fb48307737e60d6c2d05253e7ee3aea`.
Its author identifies XC7Z010-CLG400A. No record binds that ROM to the selected
S9 silicon revision. The mode is a reference diagnostic, not an exact target.

The machine requires two CPUs, `-bios`, and no direct kernel, initrd, DTB or
kernel command line. Both CPUs use the existing QEMU architectural reset
handler and start at address0. The upstream firmware path leaves each CPU's
`boot_info` pointer null. It creates no replacement reset instructions. The
ROM itself takes CPU1 into its WFE loop at0x64..0x6c.

The ROM is a read-only QEMU ROM region at0..0x1ffff. QEMU restores its original
bytes on system reset. Four64KiB aliases select the existing256KiB OCM RAM.
During ROM execution the low aliases are0x40000..0x7ffff. The actual ROM uses
initial stack0x7d4a0 and loads its first stage at0x40000. Its TTBR0/TTBR1
value is0x10000. The original descriptor0x00000c02 is a1MiB identity section,
so these virtual addresses are also physical addresses. SCTLR1 enables only
the MMU; its cache bits are clear. This is the temporary ROM view, distinct
from the documented post-BootROM low view at0. The byte audit is
`work/simulator/reviews/zynq-rom-address-basis-513c.json`. AMD UG585's Lockdown
Types page also identifies the internal FSBL window0x40000..0x70000.

Each documented OCM_CFG RAM_HI bit moves one bank to0xfffc0000 plus its bank
offset. The exact original code writes8 at0xfd34 before copying its high
handoff stubs. It later writes0x18. Bit4 is the private SWAP interface. The
public register page requires software to preserve that bit. Its full
hardware behavior is not established here. This mode rejects the private
bit and all other unsupported bits with a visible internal model error. It
does not return a fabricated success or silently ignore the write. The manual
names DevC ROM_SHADOW at0xf8007028 as a write-once ROM disable. Its documented
boot trace writes0xffffffff. That operation and the separate SWAP relationship
must be implemented from the complete evidence, not conflated.

System reset restores the low mapping and OCM_CFG0. RAM bytes are retained
across this reference warm reset. Their physical power-on contents and
retention are not claimed. The existing DDR and other support-device models
remain reference implementations. No initial status or undocumented DevC
register value is changed to advance the ROM. Only the selected ROM CPUs
disable QEMU's cached ignore-memory-failures policy. Unmapped bus accesses
can therefore produce the existing CPU abort. The diagnostic also enables
complete guest_errors/unimp logging. A bounded observer cancels the owned
run on its first unexpected emulator message. Raw bytes, offset and hash
remain saved. An unimplemented read must not become a passing zero value.

The C changes use QEMU's existing memory, reset and device interfaces. Rust
will capture the exact backend/input identities, execute it in the existing
isolated runtime, and retain every observed instruction and failure. C is
needed for this existing emulator interface; no acquired firmware runs on
the host.

Planned checks are complete ROM readback and write rejection, all16 RAM_HI
combinations, distinct data in all four banks, two real QMP system resets,
and explicit rejection of private SWAP. These are component tests. A separate
bounded CPU run must retain the original instruction trace and the first
unsupported device operation. Neither test alone closes the S9 board gate.

Primary register references:

- https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/Register-OCM_CFG-Details
- https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/Lockdown-Types

The current manufacturer pages were read through the web tool. The official
PDF transfer failed and remains recorded in513. A separate mirror transfer
then succeeded. Its internal cover and metadata identify UG585v1.12.2,
July1,2018; SHA-256e24b2574c52110f351e6aee0375b97f62556d2decd957e006646367ca43b7a66.
The original is preserved at
`artifacts/files/source/source-zynq-ug585-1122-mirror/ug585-Zynq-7000-TRM-v1.12.2.pdf`.
PDF pages159,205 and1689 contain the checked security description, boot
register values and OCM_CFG fields. The mirror identity is explicit; no
publisher signature or publisher-supplied digest was available.

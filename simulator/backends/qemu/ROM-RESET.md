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

## Reset reason in the ROM reference mode

The 2012 UG585v1.0 tables at pages1441–1443 specify RST_REASON0x250,
RST_REASON_CLR0x254 and persistent REBOOT_STATUS0x258. The original mirrored
PDF is `artifacts/files/source/source-zynq-ug585-v1-bdtic-513h/ug585-v1.pdf`,
SHA-25615bc69f0aaa6e94f4c9191278279c3e9070382760851874df10596e1f9bd758a.
The mirror adds its own watermark and metadata. Publisher byte identity is
not claimed. The page was checked as both text and a rendered image.

RST_REASON bits6..0 are POR, SRST_B, DBG_RST, SLC_RST, AWDT1_RST,
AWDT0_RST and SWDT_RST. It is read-only; reserved bits read zero. Writing
CLR bit0 clears these causes; zero and reserved bits have no effect. CLR
always reads zero. A newly constructed reference device initializes POR=1
and REBOOT_STATUS=0. Later reset callbacks preserve these registers. A guest
PSS_RST_CTRL.SOFT_RST write records SLC_RST before requesting the existing
QEMU reset. Reset clears the PSS control register. UG585v1.12.2 pages1608–1609
specify that control's reset-zero and generated-pulse behavior.

The exact original AMD Answer52030 API response is
`artifacts/files/source/source-zynq-reset-reason-api-513h/52030-solution.html`,
SHA-2563758538aebdfbd42afb1a4b87c6a2d6d39207f520c54aedae5cf989716579e4d.
Its narrative confirms accumulation and retention through every non-POR
reset, and POR clearing all causes except POR. Its RST_REASON summary table
conflicts with the full 2012 register table: it swaps DBG/SRST, omits SLC and
labels bit0 as a BootROM error. The model uses the full register table. This
conflict remains in the original evidence. The same answer's production
REBOOT_STATUS mapping agrees with the full table shifted by16 bits.

QEMU maps both QMP and guest reset to RESET_TYPE_COLD. That API label is not
a physical POR. POR is established only during realization; the existing
migrated register array holds persistent state. QMP reset represents a
control-plane warm reset and adds no guessed hardware cause. The new test
checks two actual PSS software resets and two QMP resets. It checks initial
POR, cause accumulation, read-only writes, clear-zero/one/reserved behavior,
read-zero CLR and independent REBOOT_STATUS retention. The default mode's
reset behavior is unchanged. No watchdog, external-reset or debug-reset
cause is implemented or accepted by this component test. Partial/unaligned
access to the two reason registers is explicitly unsupported and stops the
model; no physical subword access behavior is claimed.

These changes remove one observed missing interface in the real original
ROM trace. They do not establish a complete boot or bind the public ROM to
the selected S9 silicon. Private SWAP and ROM_SHADOW remain incomplete.

## Legacy LVTTL register

The same original 2012 UG585 table at page1516 identifies GPIOB_CFG_LVTTL
at0xf8000b10. It specifies a32-bit read/write register with reset value0.
Bits31:28 ignore writes and read zero. Bits27:0 are read/write reserved
fields whose meaning is not public. The BootROM register-value table gives
0x0c301166 for this register. The actual original-ROM513h diagnostic reaches
this address after the reset-clear operation. The later2018manual omits it.

The ROM-reference model stores the lower28 bits, ignores the upper nibble,
and clears the register on the existing reset callback. It retains the SLCR
write lock. The component test checks0, the original BootROM value, an independent all-bits storage vector, ignored
upper bits and two actual QMP resets. Partial and unaligned accesses stop as
unsupported. The default mode stays unchanged. This is register behavior
only; electrical thresholds, drive strength and propagation are not modeled
or calibrated by these tests. No input voltage or pin result is fabricated.

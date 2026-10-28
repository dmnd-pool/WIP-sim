ENTRY(_start)
PHDRS {
    original PT_LOAD FLAGS(5);
    ocm PT_LOAD FLAGS(6);
    harness PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}
SECTIONS {
    . = 0;
    .oracle_rom : { KEEP(*(.oracle.rom)) } :original
    . = 0x70000;
    .oracle_ocm (NOLOAD) : { KEEP(*(.oracle.ocm)) } :ocm
    . = 0x1000000;
    .text : { *(.text .text.*) } :harness
    .rodata : { *(.rodata .rodata.*) *(.oracle.sector) } :harness
    . = ALIGN(4096);
    .data : { *(.data .data.*) } :data
    .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } :data
    /DISCARD/ : { *(.ARM.exidx* .ARM.extab* .comment .note*) }
}
ASSERT(ADDR(.oracle_rom) == 0 && SIZEOF(.oracle_rom) == 131072, "unchanged original ROM identity")
ASSERT(ADDR(.oracle_ocm) == 0x70000 && SIZEOF(.oracle_ocm) == 4096, "explicit routine-global storage")

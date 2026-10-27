ENTRY(_start)
PHDRS {
    original PT_LOAD FLAGS(5);
    harness PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}
SECTIONS {
    . = 0x28b2c;
    .oracle_pll_frequency : { KEEP(*(.oracle.pll_frequency)) } :original
    . = 0x1000000;
    .text : { *(.text .text.*) } :harness
    .rodata : { *(.rodata .rodata.*) } :harness
    . = ALIGN(4096);
    .data : { *(.data .data.*) } :data
    .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } :data
    /DISCARD/ : { *(.ARM.exidx* .ARM.extab* .comment .note*) }
}
ASSERT(ADDR(.oracle_pll_frequency) == 0x28b2c && SIZEOF(.oracle_pll_frequency) == 108, "original frequency function identity")

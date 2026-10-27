/* Keep original instructions and literal addresses. Callback entries and
 * the bounded ABI wrapper are the only new executable code. */
ENTRY(_start)
PHDRS {
    callbacks PT_LOAD FLAGS(5);
    formatter PT_LOAD FLAGS(5);
    literals PT_LOAD FLAGS(5);
    globals PT_LOAD FLAGS(6);
    secondary PT_LOAD FLAGS(6);
    harness PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}
SECTIONS {
    . = 0xa248;
    .oracle_close : { KEEP(*(.oracle.close)) } :callbacks
    . = 0xa2fc;
    .oracle_open : { KEEP(*(.oracle.open)) } :callbacks
    . = 0xa638;
    .oracle_print : { KEEP(*(.oracle.print)) } :callbacks
    . = 0xa668;
    .oracle_putc : { KEEP(*(.oracle.putc)) } :callbacks
    . = 0x238f0;
    .oracle_core0 : { KEEP(*(.oracle.core0)) } :formatter
    . = 0x76be0;
    .oracle_mode : { KEEP(*(.oracle.mode)) } :literals
    . = 0x7980c;
    .oracle_status : { KEEP(*(.oracle.status)) } :literals
    . = 0x79848;
    .oracle_field_format : { KEEP(*(.oracle.field_format)) } :literals
    . = 0x7aad0;
    .oracle_labels : { KEEP(*(.oracle.labels)) } :literals
    . = 0x95000;
    .fixture_globals (NOLOAD) : { . += 0x1000; } :globals
    . = 0x2c9000;
    .fixture_secondary (NOLOAD) : { . += 0x1000; } :secondary
    . = 0x1000000;
    .text : { *(.text .text.*) } :harness
    .rodata : { *(.rodata .rodata.*) } :harness
    . = ALIGN(4096);
    .data : { *(.data .data.*) } :data
    .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } :data
    /DISCARD/ : { *(.ARM.exidx* .ARM.extab* .comment .note*) }
}
ASSERT(ADDR(.oracle_core0) == 0x238f0 && SIZEOF(.oracle_core0) == 1024, "formatter identity")
ASSERT(SIZEOF(.oracle_open) == 4 && SIZEOF(.oracle_close) == 4 && SIZEOF(.oracle_print) == 4 && SIZEOF(.oracle_putc) == 4, "callback entries")

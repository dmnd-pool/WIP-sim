/* Original relative branches and literal loads require the original addresses.
 * Only test callback entries and the C harness are new code. */
ENTRY(_start)
PHDRS {
    callbacks PT_LOAD FLAGS(5);
    collector PT_LOAD FLAGS(5);
    division PT_LOAD FLAGS(5);
    globals PT_LOAD FLAGS(6);
    queue PT_LOAD FLAGS(6);
    config PT_LOAD FLAGS(6);
    harness PT_LOAD FLAGS(5);
    data PT_LOAD FLAGS(6);
}
SECTIONS {
    . = 0xa404;
    .oracle_unlock : { KEEP(*(.oracle.unlock)) } :callbacks
    . = 0xa458;
    .oracle_lock : { KEEP(*(.oracle.lock)) } :callbacks
    . = 0x1c0c4;
    .oracle_collector : { KEEP(*(.oracle.collector)) } :collector
    . = 0x6269c;
    .oracle_delay : { KEEP(*(.oracle.delay)) } :division
    . = 0x75be8;
    .oracle_division : { KEEP(*(.oracle.division)) } :division
    . = 0x95000;
    .fixture_globals (NOLOAD) : { . += 0x4000; } :globals
    . = 0xb52000;
    .fixture_queue (NOLOAD) : { . += 0x2000; } :queue
    . = 0xc00000;
    .fixture_config (NOLOAD) : { . += 0x5000; } :config
    . = 0x1000000;
    .text : { *(.text .text.*) } :harness
    .rodata : { *(.rodata .rodata.*) } :harness
    . = ALIGN(4096);
    .data : { *(.data .data.*) } :data
    .bss (NOLOAD) : { *(.bss .bss.*) *(COMMON) } :data
    /DISCARD/ : { *(.ARM.exidx* .ARM.extab* .comment .note*) }
}
ASSERT(ADDR(.oracle_collector) == 0x1c0c4 && SIZEOF(.oracle_collector) == 916, "collector identity")
ASSERT(ADDR(.oracle_division) == 0x75be8 && SIZEOF(.oracle_division) == 168, "division identity")
ASSERT(SIZEOF(.oracle_unlock) == 4 && SIZEOF(.oracle_lock) == 4 && SIZEOF(.oracle_delay) == 4, "callback entries")

/* ARM32 ABI wrapper for the unchanged T17 core-register-0 formatter.
 * Inputs and bounded stdio callbacks belong to this test. The original code
 * supplies every reported field. No ASIC, filesystem service or clock is
 * modeled. The captured header reuses the packing oracle's syscall helpers. */
#include "/oracle-abi.h"

typedef unsigned char u8;
struct result { u32 value; u8 found; u8 padding[3]; };
struct envelope { u32 before; struct result result; u32 after; };
_Static_assert(sizeof(struct result) == 8, "original result ABI");
_Static_assert(sizeof(void *) == 4, "ARM32 ABI required");
extern void original_format(struct result *);
__asm__(".syntax unified\n.arm\n"
        ".section .oracle.core0,\"ax\",%progbits\n.balign 4\n"
        ".global original_format\n.type original_format,%function\n"
        "original_format:\n.incbin \"/miner\",112880,1024\n"
        ".size original_format,.-original_format\n"
        ".section .oracle.mode,\"ax\",%progbits\n.incbin \"/miner\",453600,3\n"
        ".section .oracle.status,\"ax\",%progbits\n.incbin \"/miner\",464908,12\n"
        ".section .oracle.field_format,\"ax\",%progbits\n.incbin \"/miner\",464968,14\n"
        ".section .oracle.labels,\"ax\",%progbits\n.incbin \"/miner\",469712,180\n"
        ".section .oracle.open,\"ax\",%progbits\nb fixture_open\n"
        ".section .oracle.print,\"ax\",%progbits\nb fixture_print\n"
        ".section .oracle.close,\"ax\",%progbits\nb fixture_close\n"
        ".section .oracle.putc,\"ax\",%progbits\nb fixture_putc\n.text\n");

#define WORD(address) (*(volatile u32 *)(address))
#define PRIMARY 0x00c00000u
#define SECONDARY 0x00c00020u
static u32 events[64][6], event_count, open_failures;

static void record(u32 tag, u32 a, u32 b, u32 c, u32 d, u32 e) {
    if (event_count >= 64) finish(10);
    u32 *event = events[event_count++];
    event[0] = tag; event[1] = a; event[2] = b;
    event[3] = c; event[4] = d; event[5] = e;
}

__attribute__((used)) u32 fixture_open(u32 path, u32 mode) {
    if ((path != PRIMARY && path != SECONDARY) || mode != 0x76be0u) finish(11);
    u32 which = path == PRIMARY ? 1 : 2;
    u32 handle = (open_failures & which) ? 0 : which * 0x100;
    record(1, path, mode, handle, 0, 0);
    return handle;
}

__attribute__((used)) int fixture_print(u32 stream, u32 format, ...) {
    if (stream != 0x100 && stream != 0x200) finish(12);
    if (format != 0x7aad0u && format != 0x79848u) finish(13);
    __builtin_va_list args;
    __builtin_va_start(args, format);
    u32 a = __builtin_va_arg(args, u32);
    u32 b = __builtin_va_arg(args, u32);
    u32 c = format == 0x7aad0u ? __builtin_va_arg(args, u32) : 0;
    __builtin_va_end(args);
    record(2, stream, format, a, b, c);
    return 0;
}

__attribute__((used)) int fixture_close(u32 stream) {
    /* The original calls fclose(NULL) after an unsuccessful fopen. Retain
     * that observation without invoking a host stdio implementation. */
    if (stream != 0 && stream != 0x100 && stream != 0x200) finish(14);
    record(3, stream, 0, 0, 0, 0);
    return 0;
}

__attribute__((used)) int fixture_putc(u32 character, u32 stream) {
    if (stream != 0x100 && stream != 0x200) finish(15);
    record(4, character, stream, 0, 0, 0);
    return (int)character;
}

static u32 case_id;
static void run_case(u32 raw, u8 found, u32 debug, u32 secondary, u32 failures) {
    struct envelope out = {0x13579bdf, {raw, found, {0xa5, 0xa5, 0xa5}}, 0x2468ace0};
    WORD(0x95820u) = debug;
    WORD(0x95824u) = PRIMARY;
    WORD(0x2c9010u) = secondary ? SECONDARY : 0;
    event_count = 0;
    open_failures = failures;
    original_format(&out.result);
    if (out.before != 0x13579bdf || out.after != 0x2468ace0 ||
        out.result.value != raw || out.result.found != found) finish(16);
    for (u32 n = 0; n < 3; ++n) if (out.result.padding[n] != 0xa5) finish(17);
    if (WORD(0x95820u) != debug || WORD(0x95824u) != PRIMARY ||
        WORD(0x2c9010u) != (secondary ? SECONDARY : 0)) finish(18);
    emit(case_id++); emit(raw); emit(found); emit(debug); emit(secondary); emit(failures);
    emit(event_count); emit(out.before); emit(out.after); emit(0xa5a5a5);
    for (u32 n = 0; n < event_count; ++n)
        for (u32 field = 0; field < 6; ++field) emit(events[n][field]);
}

__attribute__((noreturn)) void _start(void) {
    static const u32 cores[] = {0, 1, 0x1ff, 0x200, 0xffff};
    static const u32 explicit_raw[] = {
        0, 0x74, 0x01230074, 0xabcdff74, 0xffffffff, 0x10008, 0x10004, 0xdeadbeef
    };
    static const u32 debug_levels[] = {0, 3, 4, 5};
    emit(0x43304631); emit(1); emit(5192); emit(6);
    for (u32 value = 0; value < 256; ++value)
        for (u32 core = 0; core < 5; ++core)
            for (u32 middle = 0; middle < 2; ++middle)
                for (u32 found = 0; found < 2; ++found)
                    run_case(cores[core] << 16 | (middle ? 0xff00 : 0) | value,
                             (u8)found, 4, 0, 0);
    for (u32 i = 0; i < 8; ++i)
        run_case(explicit_raw[i], i == 7 ? 0 : 1, 4, 0, 0);
    for (u32 debug = 0; debug < 4; ++debug)
        for (u32 secondary = 0; secondary < 2; ++secondary)
            for (u32 failures = 0; failures < 4; ++failures)
                for (u32 found = 0; found < 2; ++found)
                    run_case(0xabcdff74, found ? 255 : 0, debug_levels[debug], secondary, failures);
    if (case_id != 5192) finish(19);
    finish(0);
}

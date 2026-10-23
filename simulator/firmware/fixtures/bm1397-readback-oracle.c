/* Controlled ARM32 ABI fixture. The collector and division helper are exact
 * vendor bytes. Queue inputs and callback counters belong to this test.
 * FOUND is a driver output. No ASIC, clock, lock or mining result is modeled.
 * The captured header reuses the existing packing oracle's syscall helpers. */
#include "/oracle-abi.h"

typedef unsigned char u8;
struct result { u32 value; u8 found; u8 padding[3]; };
struct entry { u32 value; u8 unused, address, reg, chain; };
struct test_case { u32 head, count; struct entry entries[2]; };
struct envelope { u32 before; struct result result; u32 after; };
_Static_assert(sizeof(struct result) == 8, "original result ABI");
_Static_assert(sizeof(struct entry) == 8, "original queue ABI");
_Static_assert(sizeof(void *) == 4, "ARM32 ABI required");

extern void original_collect(u32 chain, u32 asic, u32 reg, struct result *);
__asm__(".syntax unified\n.arm\n"
        ".section .oracle.collector,\"ax\",%progbits\n.balign 4\n"
        ".global original_collect\n.type original_collect,%function\n"
        "original_collect:\n.incbin \"/miner\",82116,916\n"
        ".size original_collect,.-original_collect\n"
        ".section .oracle.division,\"ax\",%progbits\n.balign 4\n"
        ".incbin \"/miner\",449512,168\n"
        ".section .oracle.lock,\"ax\",%progbits\n"
        "b fixture_lock\n"
        ".section .oracle.unlock,\"ax\",%progbits\n"
        "b fixture_unlock\n"
        ".section .oracle.delay,\"ax\",%progbits\n"
        "b fixture_delay\n.text\n");

#define QUEUE_BYTES (12u + 511u * 8u)
#define QUEUE ((volatile u8 *)0x00b52588u)
#define WORD(address) (*(volatile u32 *)(address))
static u32 lock_calls, unlock_calls, delay_one, delay_300, depth;

__attribute__((used)) int fixture_lock(u32 address) {
    if (address != 0x0018d3f8u || depth || ++lock_calls > 10) finish(10);
    depth = 1;
    return 0;
}

__attribute__((used)) int fixture_unlock(u32 address) {
    if (address != 0x0018d3f8u || depth != 1 || ++unlock_calls > 10) finish(11);
    depth = 0;
    return 0;
}

__attribute__((used)) void fixture_delay(u32 request) {
    if (depth) finish(12);
    if (request == 1) ++delay_one;
    else if (request == 300) ++delay_300;
    else finish(13);
    if (delay_one > 10 || delay_300 > 10) finish(14);
}

/* Only inputs appear here. All result words come from original_collect. */
#define ENTRY(value, address, reg, chain) {value, 0xa5, address, reg, chain}
static const struct test_case cases[] = {
    {0, 1, {ENTRY(0x01230074, 12, 0x40, 2), {0}}},
    {0, 1, {ENTRY(0x01230074, 12, 0x41, 2), {0}}},
    {0, 1, {ENTRY(0x01230074, 12, 0x40, 3), {0}}},
    {0, 1, {ENTRY(0x01230074, 16, 0x40, 2), {0}}},
    {0, 2, {ENTRY(0x11111111, 12, 0x41, 2), ENTRY(0x01230074, 12, 0x40, 2)}},
    {0, 2, {ENTRY(0x11111111, 12, 0x40, 3), ENTRY(0x01230074, 12, 0x40, 2)}},
    {0, 0, {{0}, {0}}},
    {510, 1, {ENTRY(0x01230074, 12, 0x40, 2), {0}}}
};

static u32 metadata(struct entry e) {
    return e.unused | (u32)e.address << 8 | (u32)e.reg << 16 | (u32)e.chain << 24;
}

__attribute__((noreturn)) void _start(void) {
    emit(0x52424331); /* readback collector output, version 1 */
    emit(1);
    emit(sizeof(cases) / sizeof(cases[0]));
    emit(24); /* words per case */
    WORD(0x00095820u) = 0; /* Disable original diagnostic file output. */
    WORD(0x00098a18u) = 0x00c00000u;
    *(volatile u8 *)(0x00c00000u + 0x4ddau) = 4; /* Explicit test divisor. */
    for (u32 i = 0; i < sizeof(cases) / sizeof(cases[0]); ++i) {
        u8 snapshot[QUEUE_BYTES];
        const struct test_case *test = &cases[i];
        for (u32 n = 0; n < QUEUE_BYTES; ++n) QUEUE[n] = 0;
        WORD(0x00b52588u) = 0x11223344u; /* Unused producer-index sentinel. */
        WORD(0x00b5258cu) = test->head;
        WORD(0x00b52590u) = test->count;
        for (u32 n = 0; n < test->count; ++n) {
            u32 index = test->head + n;
            if (index >= 511) index -= 511;
            const u8 *input = (const u8 *)&test->entries[n];
            for (u32 byte = 0; byte < 8; ++byte) QUEUE[12 + index * 8 + byte] = input[byte];
        }
        for (u32 n = 0; n < QUEUE_BYTES; ++n) snapshot[n] = QUEUE[n];
        struct envelope out = {0x13579bdf, {0xdeadbeef, 0, {0xa5, 0xa5, 0xa5}}, 0x2468ace0};
        lock_calls = unlock_calls = delay_one = delay_300 = depth = 0;
        original_collect(2, 3, 0x40, &out.result);
        if (depth || out.before != 0x13579bdf || out.after != 0x2468ace0) finish(15);
        for (u32 n = 0; n < 3; ++n) if (out.result.padding[n] != 0xa5) finish(16);
        for (u32 n = 0; n < QUEUE_BYTES; ++n)
            if ((n < 4 || n >= 12) && snapshot[n] != QUEUE[n]) finish(17);

        emit(i); emit(test->head); emit(test->count);
        emit(2); emit(3); emit(0x40);
        emit(test->entries[0].value); emit(metadata(test->entries[0]));
        emit(test->entries[1].value); emit(metadata(test->entries[1]));
        emit(0xdeadbeef); emit(0);
        emit(out.result.value); emit(out.result.found);
        emit(WORD(0x00b5258cu)); emit(WORD(0x00b52590u));
        emit(lock_calls); emit(unlock_calls); emit(delay_one); emit(delay_300);
        emit((u32)out.result.padding[0] | (u32)out.result.padding[1] << 8 | (u32)out.result.padding[2] << 16);
        emit(out.before); emit(out.after); emit(WORD(0x00b52588u));
    }
    finish(0);
}

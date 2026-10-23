/* ARM ABI test wrapper only. The two routines are unchanged vendor bytes.
 * This does not start the miner or model any ASIC response. */
typedef unsigned int u32;
extern void original_read(u32 *, u32, u32);
extern void original_write(u32 *, u32, u32);

__asm__(".syntax unified\n.arm\n.text\n.balign 4\n"
        ".global original_read\n.type original_read,%function\n"
        "original_read:\n.incbin \"/miner\",117936,64\n"
        ".size original_read,.-original_read\n"
        ".global original_write\n.type original_write,%function\n"
        "original_write:\n.incbin \"/miner\",118000,64\n"
        ".size original_write,.-original_write\n");

__attribute__((noreturn)) static void finish(int code) {
    register int r0 __asm__("r0") = code;
    register int r7 __asm__("r7") = 1;
    __asm__ volatile("svc #0" : : "r"(r0), "r"(r7) : "memory");
    __builtin_unreachable();
}

static void emit(u32 word) {
    register int r0 __asm__("r0") = 1;
    register const u32 *r1 __asm__("r1") = &word;
    register u32 r2 __asm__("r2") = sizeof(word);
    register int r7 __asm__("r7") = 4;
    __asm__ volatile("svc #0" : "+r"(r0) : "r"(r1), "r"(r2), "r"(r7) : "memory");
    if (r0 != 4) finish(2);
}

__attribute__((noreturn)) void _start(void) {
    u32 word;
    for (u32 core = 0; core < 1024; ++core)
        for (u32 reg = 0; reg < 32; ++reg) {
            original_read(&word, core, reg);
            emit(word);
        }
    for (u32 reg = 0; reg < 32; ++reg)
        for (u32 value = 0; value < 512; ++value) {
            original_write(&word, reg, value);
            emit(word);
        }
    finish(0);
}

/* ARM32 ABI fixture for one unchanged T17 frequency arithmetic routine.
 * No PLL, lock, clock, ASIC or timing behavior is modeled. */
#include "/oracle-abi.h"
extern int original_frequency(u32, u32);
__asm__(".syntax unified\n.arm\n"
        ".section .oracle.pll_frequency,\"ax\",%progbits\n.balign 4\n"
        ".global original_frequency\n.type original_frequency,%function\n"
        "original_frequency:\n.incbin \"/miner\",133932,108\n"
        ".size original_frequency,.-original_frequency\n.text\n");
static u32 case_id;
static void run_case(u32 word, u32 divider) {
    emit(case_id++); emit(word); emit(divider);
    emit((u32)original_frequency(word, divider));
}
static u32 word(u32 feedback, u32 reference, u32 post1, u32 post2) {
    return (feedback << 16) | (reference << 8) | (post1 << 4) | post2;
}
__attribute__((noreturn)) void _start(void) {
    static const u32 feedback[] = {1,15,16,72,112,250,255,256,2047,4095};
    static const u32 reference[] = {1,2,3,7,31,63};
    static const u32 divider[] = {1,2,3,15,16,255};
    static const u32 explicit_words[] = {0xc0700111,0x40480221,0x00010111};
    static const u32 ignored[] = {0,8,128,1u<<14,1u<<15,1u<<28,1u<<29,1u<<30,1u<<31};
    static const u32 upper[] = {0,0xff00,0xffffff00};
    emit(0x504c4c31); emit(1); emit(22134); emit(4);
    for (u32 f=0; f<10; ++f)
        for (u32 r=0; r<6; ++r)
            for (u32 p=1; p<8; ++p)
                for (u32 q=1; q<8; ++q)
                    for (u32 d=0; d<6; ++d)
                        run_case(word(feedback[f],reference[r],p,q),divider[d]);
    for (u32 f=1; f<4096; ++f) run_case(word(f,1,1,1),1);
    for (u32 r=1; r<64; ++r) run_case(word(4095,r,3,5),3);
    for (u32 d=1; d<256; ++d) run_case(word(112,2,2,1),d);
    for (u32 w=0; w<3; ++w)
        for (u32 i=0; i<9; ++i)
            for (u32 u=0; u<3; ++u)
                run_case(explicit_words[w] ^ ignored[i],upper[u] | 1);
    if (case_id != 22134) finish(3);
    finish(0);
}

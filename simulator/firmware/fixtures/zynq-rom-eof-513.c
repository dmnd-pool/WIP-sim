/* Original-ROM routine oracle. C is required for the ARM32 call/link ABI.
 * Filesystem structures and the last sector are explicit test inputs.
 * No hardware I/O, BootROM reset sequence or board identity is claimed. */
typedef unsigned int u32;
typedef unsigned char u8;
__asm__(".syntax unified\n.arm\n"
        ".section .oracle.rom,\"ax\",%progbits\n.balign 4\n"
        ".incbin \"/rom.bin\"\n"
        ".section .oracle.sector,\"a\",%progbits\n.balign 4\n"
        ".global selected_last_sector\nselected_last_sector:\n.incbin \"/work/last-sector.bin\"\n.text\n");
extern const u8 selected_last_sector[512];
__attribute__((section(".oracle.ocm"),aligned(4))) static volatile u8 ocm[4096];
__attribute__((aligned(4))) static u8 filesystem[548], file[28], output[512];
static void set32(u8 *p,u32 x) { for(u32 i=0;i<4;i++)p[i]=(u8)(x>>(8*i)); }
static u32 get32(const u8 *p) { return (u32)p[0]|((u32)p[1]<<8)|((u32)p[2]<<16)|((u32)p[3]<<24); }
__attribute__((noreturn)) static void finish(u32 code) {
    register u32 r0 __asm__("r0")=code;register u32 r7 __asm__("r7")=1;
    __asm__ volatile("svc #0"::"r"(r0),"r"(r7):"memory");__builtin_unreachable();
}
static void emit(const void *data,u32 len) {
    register u32 r0 __asm__("r0")=1;register const void *r1 __asm__("r1")=data;
    register u32 r2 __asm__("r2")=len;register u32 r7 __asm__("r7")=4;
    __asm__ volatile("svc #0":"+r"(r0):"r"(r1),"r"(r2),"r"(r7):"memory");
    if(r0!=len)finish(2);
}
static void test(u32 id,u32 position,u32 requested,u32 fault) {
    for(u32 i=0;i<sizeof(filesystem);i++)filesystem[i]=0;
    for(u32 i=0;i<sizeof(file);i++)file[i]=0;
    for(u32 i=0;i<sizeof(output);i++)output[i]=0xa5;
    filesystem[0]=2;filesystem[2]=4;filesystem[6]=0x35;filesystem[7]=0x71;
    set32(filesystem+32,0x12345);
    for(u32 i=0;i<512;i++)filesystem[36+i]=selected_last_sector[i];
    set32(file,(u32)filesystem);file[4]=0x35;file[5]=0x71;file[6]=1;
    set32(file+8,position);set32(file+12,79263);set32(file+24,0x12345);
    ocm[0x220]=0;
    if(fault==1)file[4]^=1;
    if(fault==2)file[6]=0;
    if(fault==3)file[6]=0x81;
    if(fault==4)ocm[0x220]=1;
    u32 count=0xdeadbeef;
    u32 (*original_read)(void*,void*,u32,u32*)=(void*)0x8f7c;
    u32 result=original_read(file,output,requested,&count);
    u32 words[7]={id,position,requested,fault,result,count,get32(file+8)};
    emit(words,sizeof(words));emit(output,sizeof(output));
}
__attribute__((noreturn)) void _start(void) {
    static const u32 positions[]={78849,79000,79262,79263};
    static const u32 requested[]={0,1,2,415,79263,0xffffffff};
    const u32 header[4]={0x454f4631,1,28,540};emit(header,sizeof(header));
    u32 id=0;
    for(u32 p=0;p<4;p++)for(u32 r=0;r<6;r++)test(id++,positions[p],requested[r],0);
    for(u32 fault=1;fault<=4;fault++)test(id++,79262,79263,fault);
    if(id!=28)finish(3);finish(0);
}

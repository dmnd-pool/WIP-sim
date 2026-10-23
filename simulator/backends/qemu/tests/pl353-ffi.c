/* Project-owned test of the Rust/C PL353 ABI. No miner firmware is used. */
#include "../md-sim-pl353.h"
#include <assert.h>
#include <stdlib.h>
struct Wire { unsigned command, address, count, ends; };
static void output(void *p, uint32_t kind, uint8_t byte) {
    struct Wire *w=p;
    if (kind==0) w->command=byte;
    if (kind==1) { w->address=byte; w->count++; }
}
static uint8_t input(void *p) { return ((struct Wire *)p)->command == 0x90 ? 0xec : 0xff; }
static void end(void *p) { ((struct Wire *)p)->ends++; }
int main(void) {
    MdPl353State *s=malloc(md_pl353_size()); assert(s);
    struct Wire w={0}; MdPl353Bus b={&w,output,input,end};
    md_pl353_init(s); uint32_t v=2;
    assert(md_pl353_access(s,&b,true,true,8,4,&v,0));
    v=0; assert(md_pl353_access(s,&b,false,true,0xff<<3,4,&v,10));
    assert(md_pl353_deadline(s)==5010); assert(!md_pl353_irq(s));
    md_pl353_tick(s,5010); assert(md_pl353_irq(s));
    v=0x10; assert(md_pl353_access(s,&b,true,true,0xc,4,&v,5010)); assert(!md_pl353_irq(s));
    v=0; assert(md_pl353_access(s,&b,false,true,(1<<21)|(0x90<<3),4,&v,5010));
    assert(w.command==0x90 && w.address==0 && w.count==1);
    assert(md_pl353_access(s,&b,false,false,1<<19,1,&v,5010)); assert(v==0xec);
    v=1; assert(!md_pl353_access(s,&b,true,true,0x18,4,&v,5010)); assert(md_pl353_faults(s)==1);
    md_pl353_reset(s,&b); assert(md_pl353_deadline(s)==0 && w.ends==1);
    md_pl353_enable_onfi(s);
    v=0x90;
    assert(md_pl353_access(s,&b,false,true,(1<<21)|(0xee<<3),4,&v,6000));
    assert(md_pl353_deadline(s)==7000);
    free(s);
    MdOnfiState *o=malloc(md_onfi_size()); assert(o);
    md_onfi_init(o); uint8_t byte=0;
    assert(md_onfi_output(o,0,0xec)); assert(md_onfi_output(o,1,0));
    assert(md_onfi_input(o,0,&byte) && byte=='O');
    assert(!md_onfi_output(o,0,0x70)); assert(md_onfi_status_mode(o));
    assert(md_onfi_input(o,0xc0,&byte) && byte==0xc0);
    assert(md_onfi_output(o,0,0));
    assert(md_onfi_input(o,0,&byte) && byte=='N');
    md_onfi_reset(o); assert(!md_onfi_input(o,0,&byte));
    assert(md_onfi_faults(o)==0);
    free(o); return 0;
}

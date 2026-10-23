/* SPDX-License-Identifier: GPL-2.0-or-later */
/* ABI and bounds regression. The scripted peripheral is test data only. */
#include <assert.h>
#include <stdlib.h>
#include <stdio.h>
#include <signal.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <unistd.h>
#include "../md-sim-axi-iic.h"
#ifndef MD_SIM_HOST_STD
/* Only the standalone no_std library supplies this fatal sentinel. The
 * standard Rust runtime personality requires a real unwinder context. */
extern int rust_eh_personality(int, unsigned, uint64_t, void *, void *);
#endif
typedef struct {
    unsigned starts, sends, stops, receives, nacks;
    uint8_t last;
    bool reading;
} Peripheral;
static bool start(void *p, uint8_t a, bool read) {
    Peripheral *s=p; s->starts++; return a==0x34 && read==s->reading;
}
static bool send(void *p, uint8_t b) {
    Peripheral *s=p; s->sends++;s->last=b; return true;
}
static uint8_t receive(void *p) {
    Peripheral *s=p; const uint8_t bytes[]={0xa5,0x5a};
    assert(s->receives<2); return bytes[s->receives++];
}
static void nack(void *p) { Peripheral *s=p; s->nacks++; }
static void end(void *p) { Peripheral *s=p;s->stops++; }
int main(void) {
    MdIicState *s=malloc(md_iic_size()); assert(s);
    Peripheral p={0}; MdIicBus bus={&p,start,send,receive,nack,end};
    md_iic_init(s); assert(md_iic_read(s,0x104,1)==0xc0);
    md_iic_write(s,0x100,1,1);
    md_iic_write(s,0x108,2,0x168);md_iic_write(s,0x108,2,0x2ab);
    assert(p.starts==0); md_iic_step(s,&bus); assert(p.starts==1);
    md_iic_step(s,&bus); assert(p.sends==1&&p.last==0xab&&p.stops==1);
    assert(!md_iic_pending(s));assert(md_iic_faults(s)==0);
    for(uint32_t offset=0;offset<0x10004;offset++) {
        (void)md_iic_read(s,offset,3); /* unsupported width must not touch FIFO */
    }
    assert(md_iic_faults(s)==0x10004); md_iic_reset(s);
    assert(md_iic_read(s,0x104,1)==0xc0);assert(!md_iic_irq(s));
    p=(Peripheral){.reading=true};
    md_iic_write(s,0x100,1,1);md_iic_write(s,0x120,1,15);
    md_iic_write(s,0x108,2,0x169);md_iic_write(s,0x108,2,0x202);
    md_iic_step(s,&bus);assert(p.starts==1&&p.receives==0);
    md_iic_step(s,&bus);assert(p.receives==1&&p.stops==0);
    md_iic_step(s,&bus);assert(p.receives==2&&p.nacks==1&&p.stops==1);
    assert(md_iic_read(s,0x10c,1)==0xa5);
    assert(md_iic_read(s,0x10c,1)==0x5a);
    assert(!md_iic_pending(s)&&md_iic_faults(s)==0);
    free(s);
#ifndef MD_SIM_HOST_STD
    pid_t child=fork(); assert(child>=0);
    if(child==0) {
        struct rlimit no_core={0,0}; assert(setrlimit(RLIMIT_CORE,&no_core)==0);
        alarm(2); /* An unsupported unwind must fail promptly, not spin. */
        (void)rust_eh_personality(1,1,0,NULL,NULL); _exit(1);
    }
    int status=0; assert(waitpid(child,&status,0)==child);
    assert(WIFSIGNALED(status)&&WTERMSIG(status)==SIGABRT);
    puts("AXI IIC C ABI, bounds and fatal-unwind checks passed");
#else
    puts("AXI IIC C ABI and bounds checks passed with the standard Rust runtime");
#endif
}

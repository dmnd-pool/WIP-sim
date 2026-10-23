#include "../md-sim-pic16.h"
#include <assert.h>
#include <stdio.h>

static void byte(MdPicState *s, uint64_t now, uint8_t value)
{
    assert(md_pic_start(s, now, false) == 0);
    assert(md_pic_send(s, now, value) == 0);
    md_pic_stop(s);
}
static void command(MdPicState *s, uint64_t now, uint8_t value)
{
    byte(s, now, 0x55);
    byte(s, now, 0xaa);
    byte(s, now, value);
}
int main(int argc, char **argv)
{
    assert(argc == 2);
    FILE *file = fopen(argv[1], "rb");
    assert(file);
    uint8_t app[19201];
    size_t length = fread(app, 1, sizeof(app), file);
    assert(length == 19200 && !ferror(file) && feof(file));
    assert(fclose(file) == 0);
    assert(!md_pic_new(NULL, 0));
    assert(!md_pic_new(app, 19201));
    MdPicState *s = md_pic_new(app, length);
    assert(s);
    app[0] = '0';
    assert(!md_pic_new(app, length));
    assert(md_pic_reset(s, 0) == 0);
    assert(md_pic_deadline(s) == 500000000ULL);
    assert(md_pic_start(s, 499999999ULL, false) == 1);
    command(s, 500000000ULL, 6);
    assert(md_pic_start(s, 599999999ULL, false) == 1);
    command(s, 600000000ULL, 0x17);
    assert(md_pic_start(s, 600000000ULL, true) == 0);
    uint32_t value = 0xdeadbeef;
    assert(md_pic_recv(s, 600000000ULL, &value) == 0 && value == 0x103);
    assert(md_pic_recv(s, 600000000ULL, &value) == 0 && value == 3);
    md_pic_stop(s);
    assert(md_pic_start(s, 600000000ULL, true) == 1);
    value = 0xdeadbeef;
    assert(md_pic_recv(s, 600000000ULL, &value) == 1 && value == 0xdeadbeef);
    command(s, 600000000ULL, 0x10);
    byte(s, 600000000ULL, 64);
    assert(md_pic_power(s) == 0x4002);
    command(s, 700000000ULL, 0x15);
    byte(s, 700000000ULL, 0x55);
    assert(md_pic_power(s) == 0x4003);
    command(s, 700000000ULL, 0x16);
    assert(md_pic_deadline(s) == 10700000000ULL);
    assert(md_pic_advance(s, 10699999999ULL) == 0);
    assert(md_pic_power(s) == 0x4003);
    assert(md_pic_advance(s, 10700000000ULL) == 0);
    assert(md_pic_power(s) == 0x4002);
    assert(md_pic_start(s, 10700000000ULL, false) == 0);
    command(s, 10700000000ULL, 0x16);
    assert(md_pic_power(s) == 0x4002);
    assert(md_pic_deadline(s) == UINT64_MAX);
    command(s, 10700000000ULL, 0x17);
    assert(md_pic_start(s, 10700000000ULL, true) == 0);
    assert(md_pic_recv(s, 10700000000ULL, &value) == 0 && value == 0x103);
    md_pic_stop(s);
    command(s, 11700000000ULL, 0x15);
    byte(s, 11700000000ULL, 1);
    assert(md_pic_deadline(s) == 21700000000ULL);
    command(s, 12700000000ULL, 0x15);
    byte(s, 12700000000ULL, 0);
    assert(md_pic_deadline(s) == UINT64_MAX);
    assert(md_pic_advance(s, 30000000000ULL) == 0);
    command(s, 31000000000ULL, 0x15);
    byte(s, 31000000000ULL, 1);
    assert(md_pic_deadline(s) == 40000000000ULL);
    assert(md_pic_advance(s, 39999999999ULL) == 0);
    assert(md_pic_power(s) == 0x4003);
    assert(md_pic_advance(s, 40000000000ULL) == 0);
    assert(md_pic_power(s) == 0x4002);
    byte(s, 40000000000ULL, 0x55);
    byte(s, 40000000000ULL, 0xaa);
    byte(s, 40000000000ULL, 0x55);
    assert(md_pic_start(s, 40000000000ULL, true) == 1);
    byte(s, 40000000000ULL, 0xaa);
    assert(md_pic_start(s, 40000000000ULL, true) == 1);
    value = 0xdeadbeef;
    assert(md_pic_recv(s, 40000000000ULL, &value) == 1 && value == 0xdeadbeef);
    assert(md_pic_power(s) == 0x4002);
    assert(md_pic_deadline(s) == UINT64_MAX);
    for (unsigned i = 0; i < 17; ++i) {
        byte(s, 40000000000ULL, 0);
    }
    command(s, 40000000000ULL, 0);
    for (unsigned i = 0; i < 16; ++i) {
        byte(s, 40000000000ULL, 0);
    }
    assert(md_pic_start(s, 40000000000ULL, true) == 1);
    command(s, 40000000000ULL, 0x17);
    assert(md_pic_start(s, 40000000000ULL, true) == 0);
    assert(md_pic_recv(s, 40000000000ULL, &value) == 0 && value == 0x103);
    md_pic_stop(s);
    assert(md_pic_reset(s, 40000000000ULL) == 0);
    assert(md_pic_power(s) == 0);
    byte(s, 40500000000ULL, 0x55);
    byte(s, 40500000000ULL, 0xaa);
    assert(md_pic_send(s, 40500000000ULL, 0x55) == 0x155);
    md_pic_free(s);
    md_pic_free(NULL);
    puts("PIC C ABI: exact program, byte transactions, busy NACK, latch, paused heartbeat, power cutoff and interrupted prefix pass");
}

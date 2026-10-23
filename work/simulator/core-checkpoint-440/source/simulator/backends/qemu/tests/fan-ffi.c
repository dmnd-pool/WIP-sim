#include "../md-sim-fan.h"
#include <assert.h>
#include <stddef.h>
#include <stdio.h>

_Static_assert(sizeof(MdFanConfig) == 24, "fan config ABI");
_Static_assert(offsetof(MdFanConfig, tach_channel) == 20, "tach ABI");
static uint32_t read_reg(MdFanState *s, uint32_t address)
{
    uint32_t value = 0xdeadbeef;
    assert(md_fan_read(s, address, 4, &value));
    return value;
}
int main(void)
{
    assert(!md_fan_new(NULL, 1));
    assert(!md_fan_new(NULL, 5));
    md_fan_free(NULL);
    MdFanConfig config = {0, 60, 120, 2, 0, 1};
    MdFanState *s = md_fan_new(&config, 1);
    assert(s);
    assert(md_fan_write(s, 16, 1, 100));
    assert(md_fan_advance(s, 3000000000ULL));
    assert(read_reg(s, 4) == 1);
    assert(!md_fan_advance(s, 65000000000ULL));
    assert(md_fan_advance_elapsed(s, 65250000000ULL));
    assert(read_reg(s, 4) == 1);
    assert(md_fan_next_sample(s) == 750000000ULL);
    assert(md_fan_stalled(s, 0, true));
    assert(md_fan_advance(s, 1000000000ULL));
    assert(read_reg(s, 4) == 0);
    assert(md_fan_stalled(s, 0, false));
    assert(md_fan_advance(s, 2000000000ULL));
    assert(read_reg(s, 4) == 1);
    assert(md_fan_powered(s, 0, false));
    assert(md_fan_advance_elapsed(s, 65000000000ULL));
    assert(read_reg(s, 4) == 0);
    assert(!md_fan_powered(s, 1, true));
    assert(!md_fan_write(s, 16, 1, 127));
    assert(read_reg(s, 16) == 100);
    uint32_t unchanged = 123;
    assert(!md_fan_read(s, 32, 4, &unchanged) && unchanged == 123);
    assert(!md_fan_advance(s, 60000000001ULL));
    md_fan_reset(s);
    assert(read_reg(s, 16) == 0);
    assert(md_fan_next_sample(s) == 1000000000ULL);
    md_fan_free(s);
    s = md_fan_new(NULL, 0);
    assert(s);
    assert(md_fan_write(s, 16, 1, 100));
    assert(md_fan_advance(s, 60000000000ULL));
    assert(read_reg(s, 4) == 0);
    md_fan_free(s);
    puts("fan C ABI: motion, delayed timer, stall, power, reset, absent motors and errors pass");
}

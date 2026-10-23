/* C ABI behavior check. No firmware or emulator execution. */
#include <assert.h>
#include <stdlib.h>
#include "md-sim-emc2101.h"
static uint8_t read_reg(MdEmc2101State *s, uint8_t reg, uint64_t now)
{
    assert(md_emc2101_start(s, false, now));
    assert(md_emc2101_write(s, reg, now));
    assert(md_emc2101_start(s, true, now));
    int32_t result = md_emc2101_read(s, now);
    assert(result >= 0 && result <= 255);
    md_emc2101_nack(s);
    md_emc2101_stop(s);
    return result;
}
int main(void)
{
    MdEmc2101State *s = calloc(1, md_emc2101_size());
    assert(s);
    assert(!md_emc2101_init(s, 3300000, false, 100000, 0, 0, 0));
    assert(md_emc2101_init(s, 3300000, false, 216, 0, 0, 0));
    assert(!md_emc2101_start(s, false, 9999999));
    assert(read_reg(s, 0xfd, 10000000) == 0x28);
    assert(read_reg(s, 0, 12999999) == 0);
    assert(read_reg(s, 0, 13000000) == 27);
    assert(read_reg(s, 1, 34000000) == 127);
    assert(read_reg(s, 0x10, 34000000) == 0);
    assert((read_reg(s, 2, 34000000) & 0x16) == 0x16);
    assert(read_reg(s, 0x46, 34000000) == 0xff);
    assert(read_reg(s, 0x47, 34000000) == 0xff);
    assert(md_emc2101_conversions(s) == 1);
    assert(md_emc2101_faults(s) == 0);
    md_emc2101_reset(s, 35000000);
    assert(!md_emc2101_start(s, false, 44999999));
    assert(read_reg(s, 0x4c, 45000000) == 0);
    free(s);
    return 0;
}

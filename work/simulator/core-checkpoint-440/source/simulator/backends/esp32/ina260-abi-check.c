/* SPDX-License-Identifier: GPL-2.0-or-later */
#include "md-sim-ina260.h"
#include <assert.h>
#include <stdlib.h>
static uint16_t word(MdIna260State *s, uint8_t reg, uint64_t now)
{
    assert(md_ina260_start(s, false, now));
    assert(md_ina260_write(s, reg, now));
    md_ina260_stop(s, now);
    assert(md_ina260_start(s, true, now));
    int32_t high = md_ina260_read(s, now), low = md_ina260_read(s, now);
    assert(high >= 0 && low >= 0);
    md_ina260_nack(s); md_ina260_stop(s, now);
    return (high << 8) | low;
}
int main(void)
{
    MdIna260State *s = calloc(1, md_ina260_size());
    assert(s);
    assert(!md_ina260_init(s, 3300000, UINT32_MAX, 0, 0));
    assert(md_ina260_init(s, 3300000, 11980000, -1250000, 0));
    assert(md_ina260_deadline(s) == 1100000);
    assert(word(s, 0xfe, 0) == 0x5449);
    assert(word(s, 0xff, 0) == 0x2270);
    assert(word(s, 1, 2200000) == 0xfc18);
    assert(word(s, 2, 2200000) == 0x2570);
    assert(word(s, 3, 2200000) == 1497);
    assert(md_ina260_faults(s) == 0 && md_ina260_conversions(s) == 1);
    md_ina260_reset(s, 2200000);
    assert(word(s, 0, 2200000) == 0x6127);
    assert(word(s, 1, 2200000) == 0);
    assert(md_ina260_alert_level(s));
    free(s);
    return 0;
}

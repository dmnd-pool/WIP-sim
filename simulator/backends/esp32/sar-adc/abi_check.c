/* SPDX-License-Identifier: GPL-2.0-or-later */
#include "../md-sim-sar.h"
#include <assert.h>
#include <stdlib.h>
int main(void)
{
    MdSarState *s = malloc(md_sar_size());
    assert(s != NULL);
    md_sar_init(s);
    md_sar_write(s, 0, 0x104, 4, 1U << 30, 0);
    md_sar_write(s, 0, 0x3c, 4, 3U << 29, 0);
    md_sar_write(s, 1, 0, 4, 0x05200769, 0);
    md_sar_write(s, 0, 0xc, 4, (1U << 18) | (1U << 17), 0);
    uint64_t due = md_sar_deadline(s);
    assert(due > 0 && due != UINT64_MAX);
    md_sar_advance(s, due - 1);
    assert((md_sar_read(s, 0, 0xc, 4) & 0x10000) == 0);
    md_sar_advance(s, due);
    assert((md_sar_read(s, 0, 0xc, 4) & 0x1ffff) == 0x10000);
    assert(md_sar_completed(s) == 1);
    md_sar_write(s, 0, UINT32_MAX, 4, 0, due);
    assert(md_sar_faults(s) == 1);
    md_sar_reset(s);
    assert(md_sar_completed(s) == 0);
    assert(md_sar_deadline(s) == UINT64_MAX);
    assert(md_sar_set_input(s, 0, 1, 550000));
    assert(!md_sar_set_input(s, 2, 1, 550000));
    assert(!md_sar_set_input(s, 0, 10, 550000));
    assert(!md_sar_set_input(s, 0, 1, -2));
    assert(!md_sar_set_input(s, 0, 1, 3300001));
    md_sar_reset(s);
    md_sar_write(s, 0, 0x104, 4, 1U << 30, 0);
    md_sar_write(s, 0, 0x3c, 4, 3U << 29, 0);
    md_sar_write(s, 0, 0x14, 4, 0, 0); /* no attenuation */
    md_sar_write(s, 0, 0xc, 4,
                 (1U << 31) | (2U << 19) | (1U << 18) | (1U << 17), 0);
    due = md_sar_deadline(s);
    assert(due != UINT64_MAX);
    md_sar_advance(s, due);
    /* 0.55 V / 1.1 V * 4095, truncated; physical input survives reset. */
    assert((md_sar_read(s, 0, 0xc, 4) & 0x1ffff) == (0x10000 | 2047));
    assert(md_sar_set_input(s, 0, 1, -1));
    md_sar_write(s, 0, 0xc, 4, 0, due);
    md_sar_write(s, 0, 0xc, 4,
                 (1U << 31) | (2U << 19) | (1U << 18) | (1U << 17), due);
    assert(md_sar_deadline(s) == UINT64_MAX);
    assert((md_sar_read(s, 0, 0xc, 4) & 0x10000) == 0);
    assert(md_sar_faults(s) == 1);
    free(s);
    return 0;
}

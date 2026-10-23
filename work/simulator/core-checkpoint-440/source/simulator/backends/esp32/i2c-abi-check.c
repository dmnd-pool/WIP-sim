/* SPDX-License-Identifier: GPL-2.0-or-later */
#include "md-sim-i2c.h"
#include <stdlib.h>
#include <assert.h>
static unsigned starts, stops;
static int32_t absent_device(void *opaque, uint32_t operation, uint32_t value)
{
    assert(opaque == &starts);
    if (operation == 0) { assert(value == 0x3c); starts++; return 1; }
    assert(operation == 3); stops++; return 0;
}
int main(void)
{
    MdI2cState *state = malloc(md_i2c_size());
    assert(state);
    md_i2c_init(state);
    md_i2c_write(state, 4, 4, 0x913, 0);
    md_i2c_write(state, 0x58, 4, 6 << 11, 0);
    md_i2c_write(state, 0x5c, 4, (1 << 11) | (1 << 8) | 1, 0);
    md_i2c_write(state, 0x60, 4, 2 << 11, 0);
    md_i2c_write(state, 0x1c, 4, 0x78, 0);
    md_i2c_write(state, 4, 4, 0x133, 0);
    for (unsigned i = 0; i < 8 && md_i2c_deadline(state) != UINT64_MAX; i++) {
        md_i2c_advance(state, md_i2c_deadline(state), &starts, absent_device);
    }
    assert(starts == 1 && stops == 1);
    assert(md_i2c_read(state, 0x20, 4) & (1 << 10));
    assert(!(md_i2c_read(state, 0x20, 4) & (1 << 7)));
    assert(md_i2c_faults(state) == 0);
    assert(md_i2c_bytes(state) == 1);
    free(state);
}

/* SPDX-License-Identifier: GPL-2.0-or-later */
#include "md-sim-ds4432.h"
#include <assert.h>
#include <stdlib.h>
int main(void)
{
    MdDs4432State *s = malloc(md_ds4432_size());
    assert(s);
    assert(!md_ds4432_init(s, 1, 80000, 3300000));
    assert(md_ds4432_init(s, 80000, 0, 3300000));
    assert(md_ds4432_start(s, false));
    assert(md_ds4432_write(s, 0xf8));
    assert(md_ds4432_write(s, 0xaa));
    md_ds4432_stop(s);
    assert(md_ds4432_start(s, false));
    assert(md_ds4432_write(s, 0xf8));
    assert(md_ds4432_start(s, true));
    assert(md_ds4432_read(s) == 0xaa);
    md_ds4432_nack(s);
    md_ds4432_stop(s);
    assert(md_ds4432_writes(s) == 1);
    assert(md_ds4432_faults(s) == 0);
    md_ds4432_reset(s);
    assert(md_ds4432_start(s, false));
    assert(md_ds4432_write(s, 0xf8));
    assert(md_ds4432_start(s, true));
    assert(md_ds4432_read(s) == 0);
    free(s);
}

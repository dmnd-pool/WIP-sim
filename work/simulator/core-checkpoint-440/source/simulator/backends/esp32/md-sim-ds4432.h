/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef MD_SIM_DS4432_H
#define MD_SIM_DS4432_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
typedef struct MdDs4432State MdDs4432State;
size_t md_ds4432_size(void);
bool md_ds4432_init(MdDs4432State *, uint32_t, uint32_t, uint32_t);
void md_ds4432_reset(MdDs4432State *);
bool md_ds4432_start(MdDs4432State *, bool);
bool md_ds4432_write(MdDs4432State *, uint8_t);
int32_t md_ds4432_read(MdDs4432State *);
void md_ds4432_stop(MdDs4432State *);
void md_ds4432_nack(MdDs4432State *);
uint64_t md_ds4432_faults(const MdDs4432State *);
uint64_t md_ds4432_writes(const MdDs4432State *);
#endif

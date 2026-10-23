/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef MD_SIM_INA260_H
#define MD_SIM_INA260_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
typedef struct MdIna260State MdIna260State;
size_t md_ina260_size(void);
bool md_ina260_init(MdIna260State *, uint32_t, uint32_t, int32_t, uint64_t);
void md_ina260_reset(MdIna260State *, uint64_t);
void md_ina260_advance(MdIna260State *, uint64_t);
bool md_ina260_start(MdIna260State *, bool, uint64_t);
bool md_ina260_write(MdIna260State *, uint8_t, uint64_t);
int32_t md_ina260_read(MdIna260State *, uint64_t);
void md_ina260_stop(MdIna260State *, uint64_t);
void md_ina260_nack(MdIna260State *);
bool md_ina260_inputs(MdIna260State *, uint32_t, int32_t, uint64_t);
uint64_t md_ina260_deadline(const MdIna260State *);
uint64_t md_ina260_faults(const MdIna260State *);
uint64_t md_ina260_writes(const MdIna260State *);
uint64_t md_ina260_conversions(const MdIna260State *);
bool md_ina260_alert_level(const MdIna260State *);
#endif

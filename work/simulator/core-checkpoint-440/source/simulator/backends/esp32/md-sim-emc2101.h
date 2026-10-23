/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef MD_SIM_EMC2101_H
#define MD_SIM_EMC2101_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
typedef struct MdEmc2101State MdEmc2101State;
size_t md_emc2101_size(void);
/* external_kind: 0=open, 1=short, 2=explicit temperature in eighths C. */
bool md_emc2101_init(MdEmc2101State *, uint32_t, bool, int32_t, uint32_t, int32_t, uint64_t);
void md_emc2101_reset(MdEmc2101State *, uint64_t);
void md_emc2101_advance(MdEmc2101State *, uint64_t);
bool md_emc2101_start(MdEmc2101State *, bool, uint64_t);
bool md_emc2101_write(MdEmc2101State *, uint8_t, uint64_t);
int32_t md_emc2101_read(MdEmc2101State *, uint64_t);
void md_emc2101_stop(MdEmc2101State *);
void md_emc2101_nack(MdEmc2101State *);
void md_emc2101_tach(MdEmc2101State *, bool, uint64_t);
bool md_emc2101_inputs(MdEmc2101State *, int32_t, uint32_t, int32_t, uint64_t);
uint64_t md_emc2101_deadline(const MdEmc2101State *);
uint64_t md_emc2101_faults(const MdEmc2101State *);
uint64_t md_emc2101_writes(const MdEmc2101State *);
uint64_t md_emc2101_conversions(const MdEmc2101State *);
bool md_emc2101_alert(const MdEmc2101State *);
uint8_t md_emc2101_fan_setting(const MdEmc2101State *, uint64_t);
#endif

/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef MD_SIM_SAR_H
#define MD_SIM_SAR_H
#include <stddef.h>
#include <stdbool.h>
#include <stdint.h>
typedef struct MdSarState MdSarState;
size_t md_sar_size(void);
void md_sar_init(MdSarState *);
void md_sar_reset(MdSarState *);
bool md_sar_set_input(MdSarState *, uint32_t, uint32_t, int32_t);
uint32_t md_sar_read(MdSarState *, uint32_t, uint32_t, uint32_t);
void md_sar_write(MdSarState *, uint32_t, uint32_t, uint32_t, uint32_t, uint64_t);
void md_sar_advance(MdSarState *, uint64_t);
uint64_t md_sar_deadline(const MdSarState *);
uint64_t md_sar_faults(const MdSarState *);
uint64_t md_sar_completed(const MdSarState *);
#endif

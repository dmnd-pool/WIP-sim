/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef MD_SIM_SWDT_H
#define MD_SIM_SWDT_H
#include <stdint.h>
#include <stdbool.h>
typedef struct MdSwdtRegs { uint32_t mode, control, zero; } MdSwdtRegs;
typedef struct MdSwdtExpiry { uint32_t irq_cycles, reset; } MdSwdtExpiry;
enum { MD_SWDT_INVALID = -1, MD_SWDT_NONE, MD_SWDT_CLOCK, MD_SWDT_RESTART };
void md_swdt_reset(MdSwdtRegs *state);
bool md_swdt_read(const MdSwdtRegs *state, uint32_t offset, uint32_t *value);
int32_t md_swdt_write(MdSwdtRegs *state, uint32_t offset, uint32_t value);
uint32_t md_swdt_reload(const MdSwdtRegs *state);
uint32_t md_swdt_divisor(const MdSwdtRegs *state);
void md_swdt_expire(MdSwdtRegs *state, MdSwdtExpiry *event);
#endif

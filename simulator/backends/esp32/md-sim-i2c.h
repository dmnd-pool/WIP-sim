/* SPDX-License-Identifier: GPL-2.0-or-later */
#pragma once
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>
typedef struct MdI2cState MdI2cState;
size_t md_i2c_size(void);
void md_i2c_init(MdI2cState *);
uint32_t md_i2c_read(MdI2cState *, uint64_t, uint32_t);
void md_i2c_write(MdI2cState *, uint64_t, uint32_t, uint32_t, uint64_t);
void md_i2c_advance(MdI2cState *, uint64_t, void *, int32_t (*)(void *, uint32_t, uint32_t));
bool md_i2c_take_stop(MdI2cState *);
bool md_i2c_irq(const MdI2cState *);
uint64_t md_i2c_deadline(const MdI2cState *);
uint64_t md_i2c_faults(const MdI2cState *);
uint64_t md_i2c_bytes(const MdI2cState *);

/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef MD_SIM_AXI_IIC_H
#define MD_SIM_AXI_IIC_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct MdIicState MdIicState;
typedef struct MdIicBus {
    void *opaque;
    bool (*start)(void *, uint8_t, bool);
    bool (*send)(void *, uint8_t);
    uint8_t (*receive)(void *);
    void (*nack)(void *);
    void (*end)(void *);
} MdIicBus;
size_t md_iic_size(void);
void md_iic_init(MdIicState *);
void md_iic_reset(MdIicState *);
uint32_t md_iic_read(MdIicState *, uint32_t, uint32_t);
void md_iic_write(MdIicState *, uint32_t, uint32_t, uint32_t);
void md_iic_step(MdIicState *, MdIicBus *);
bool md_iic_pending(const MdIicState *);
bool md_iic_irq(const MdIicState *);
uint64_t md_iic_faults(const MdIicState *);
#endif

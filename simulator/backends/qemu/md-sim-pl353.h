/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef MD_SIM_PL353_H
#define MD_SIM_PL353_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
typedef struct MdPl353State MdPl353State;
typedef struct MdPl353Bus {
    void *opaque;
    void (*output)(void *, uint32_t, uint8_t);
    uint8_t (*input)(void *);
    void (*end)(void *);
} MdPl353Bus;
size_t md_pl353_size(void);
void md_pl353_init(MdPl353State *);
void md_pl353_enable_onfi(MdPl353State *);
void md_pl353_reset(MdPl353State *, MdPl353Bus *);
void md_pl353_tick(MdPl353State *, uint64_t);
void md_pl353_chip_busy_until(MdPl353State *, uint64_t);
uint64_t md_pl353_deadline(const MdPl353State *);
bool md_pl353_irq(const MdPl353State *);
uint64_t md_pl353_faults(const MdPl353State *);
bool md_pl353_access(MdPl353State *, MdPl353Bus *, bool, bool,
                     uint32_t, uint32_t, uint32_t *, uint64_t);
typedef struct MdOnfiState MdOnfiState;
size_t md_onfi_size(void);
void md_onfi_init(MdOnfiState *);
void md_onfi_reset(MdOnfiState *);
bool md_onfi_output(MdOnfiState *, uint32_t, uint8_t);
bool md_onfi_input(MdOnfiState *, uint8_t, uint8_t *);
bool md_onfi_status_mode(const MdOnfiState *);
uint64_t md_onfi_faults(const MdOnfiState *);
#endif

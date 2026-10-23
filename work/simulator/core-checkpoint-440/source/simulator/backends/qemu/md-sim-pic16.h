/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef MD_SIM_PIC16_H
#define MD_SIM_PIC16_H
#include <stdbool.h>
#include <stdint.h>
typedef struct MdPicState MdPicState;
/* Normalized application must match the pinned program. State uses explicitly
 * erased development loader/calibration storage. No PIC CPU execution. */
MdPicState *md_pic_new(const uint8_t *, uint32_t);
void md_pic_free(MdPicState *);
/* Status0 success;1 ordinary NACK;2..7 model error;0x100|cmd unsupported.
 * Heartbeat expiry cuts power and leaves the application contract active. */
uint32_t md_pic_advance(MdPicState *, uint64_t);
uint32_t md_pic_reset(MdPicState *, uint64_t);
void md_pic_stop(MdPicState *);
uint32_t md_pic_start(MdPicState *, uint64_t, bool);
uint32_t md_pic_send(MdPicState *, uint64_t, uint8_t);
uint32_t md_pic_recv(MdPicState *, uint64_t, uint32_t *);
uint64_t md_pic_deadline(const MdPicState *);
uint32_t md_pic_power(const MdPicState *);
#endif

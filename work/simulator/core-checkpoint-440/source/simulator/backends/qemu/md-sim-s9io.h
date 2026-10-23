/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef MD_SIM_S9IO_H
#define MD_SIM_S9IO_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
typedef struct S9State MdS9State;
typedef struct HashBudget MdHashBudget;
struct DeviceState;
void md_s9io_set_hash_budget(struct DeviceState *, MdHashBudget *);
MdHashBudget *md_hash_budget_new(double hps);
void md_hash_budget_free(MdHashBudget *);
typedef struct MdS9Event {
    uint64_t clocks;
    uint32_t flags;
    uint8_t tx_byte, rx_byte, irq, txd;
} MdS9Event;
typedef struct MdS9Idle {
    uint64_t receive, transmit, uart, asic;
} MdS9Idle;
typedef struct MdS9Clock {
    uint64_t edge_ns, period_q32;
    uint32_t fraction;
} MdS9Clock;
typedef struct MdAsicTrace {
    uint64_t ns;
    uint32_t kind, value, extra;
    uint64_t grant, wall_ns;
} MdAsicTrace;
MdS9State *md_s9_new(uint32_t build_id);
MdS9State *md_s9_new_reference_v1(uint32_t build_id);
MdS9State *md_s9_new_bm1387(uint32_t build_id, uint16_t chips, uint16_t cores,
                           uint32_t oscillator_hz, uint8_t baud_divisor,
                           uint8_t identity_reserved, uint32_t reply_byte_capacity);
MdS9State *md_s9_new_bm1387_reference_mining(uint32_t build_id,
                                          const MdHashBudget *,
                                          uint32_t reply_byte_capacity,
                                          uint32_t fixed_filter_zero_bits);
const char *md_s9_error(const MdS9State *);
bool md_s9_asic_pin(MdS9State *, uint32_t pin, bool level, uint32_t phase_ns);
bool md_s9_take_asic_trace(MdS9State *, MdAsicTrace *);
uint32_t md_s9_core_period_ns(const MdS9State *);
void md_s9_free(MdS9State *);
bool md_s9_attach_tmp421_reference(MdS9State *);
bool md_s9_temperature_input_plan(MdS9State *, const uint8_t *, size_t);
void md_s9_reset(MdS9State *);
uint32_t md_s9_read(MdS9State *, uint32_t offset, uint32_t size, uint32_t *value);
bool md_s9_write(MdS9State *, uint32_t offset, uint32_t size, uint32_t value);
uint64_t md_s9_idle_limit(const MdS9State *, bool rxd);
void md_s9_idle_details(const MdS9State *, bool rxd, MdS9Idle *);
uint32_t md_s9_status(const MdS9State *);
bool md_s9_advance(MdS9State *, uint64_t clocks, bool rxd, MdS9Event *);
bool md_s9_advance_timed(MdS9State *, const MdS9Clock *, uint64_t clocks,
                         bool rxd, MdS9Event *);
uint64_t md_s9_idle_limit_timed(const MdS9State *, const MdS9Clock *, bool rxd);
uint64_t md_s9_peer_deadline(const MdS9State *);
bool md_s9_peer_advance(MdS9State *, uint64_t until_ns, uint64_t *reached);
bool md_s9_asic_pin_at(MdS9State *, uint32_t pin, bool level, uint64_t ns);
#endif

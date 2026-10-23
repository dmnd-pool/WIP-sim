/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef MD_SIM_FAN_H
#define MD_SIM_FAN_H
#include <stdbool.h>
#include <stdint.h>
typedef struct MdFanState MdFanState;
typedef struct MdFanConfig {
    uint32_t minimum_rpm, maximum_rpm, slew_rpm_per_second;
    uint32_t pulses_per_revolution, pwm_channel, tach_channel;
} MdFanConfig;
typedef struct MdFanInput {
    uint64_t at_ns;
    uint32_t motor, mask, values;
} MdFanInput;
MdFanState *md_fan_new(const MdFanConfig *, uint32_t);
bool md_fan_input_plan(MdFanState *, const uint8_t *, uint32_t);
bool md_fan_take_input(MdFanState *, MdFanInput *);
void md_fan_free(MdFanState *);
void md_fan_reset(MdFanState *);
bool md_fan_advance(MdFanState *, uint64_t);
bool md_fan_advance_elapsed(MdFanState *, uint64_t);
uint64_t md_fan_next_sample(const MdFanState *);
uint64_t md_fan_next_event(const MdFanState *);
bool md_fan_read(const MdFanState *, uint32_t, uint32_t, uint32_t *);
bool md_fan_write(MdFanState *, uint32_t, uint32_t, uint32_t);
bool md_fan_stalled(MdFanState *, uint32_t, bool);
bool md_fan_powered(MdFanState *, uint32_t, bool);
#endif

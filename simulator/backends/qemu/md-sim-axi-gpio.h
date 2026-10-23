/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef MD_SIM_AXI_GPIO_H
#define MD_SIM_AXI_GPIO_H
#include <stdbool.h>
#include <stdint.h>
typedef struct MdGpioState MdGpioState;
MdGpioState *md_gpio_new(uint32_t width, uint32_t direction, uint32_t reset_data,
                        uint32_t reset_tri, uint32_t inputs);
void md_gpio_free(MdGpioState *state);
void md_gpio_reset(MdGpioState *state);
bool md_gpio_read(const MdGpioState *state, uint32_t offset, uint32_t *value);
bool md_gpio_write(MdGpioState *state, uint32_t offset, uint32_t value);
void md_gpio_inputs(MdGpioState *state, uint32_t inputs);
uint32_t md_gpio_outputs(const MdGpioState *state);
#endif

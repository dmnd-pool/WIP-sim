/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef MD_SIM_PS_GPIO_H
#define MD_SIM_PS_GPIO_H
#include <stdint.h>
#include <stdbool.h>
typedef struct MdPsGpioState MdPsGpioState;
typedef struct MdPsGpioLogic { uint32_t value, known; } MdPsGpioLogic;
typedef struct MdPsGpioOutputs {
    MdPsGpioLogic data;
    uint32_t direction, enable;
} MdPsGpioOutputs;
/* The caller owns one state. All calls are serialized on the device thread.
 * False is an invalid operation, not an implicit zero. known marks determined bits.
 * Cold power state is created once; bus reset does not reset controller logic.
 */
MdPsGpioState *md_ps_gpio_new(void);
void md_ps_gpio_free(MdPsGpioState *state);
void md_ps_gpio_bus_reset(MdPsGpioState *state);
bool md_ps_gpio_read(const MdPsGpioState *state, uint32_t offset,
                     MdPsGpioLogic *value);
bool md_ps_gpio_write(MdPsGpioState *state, uint32_t offset, uint32_t value);
bool md_ps_gpio_inputs(MdPsGpioState *state, uint32_t bank,
                       MdPsGpioLogic value, uint32_t changed_mask);
bool md_ps_gpio_outputs(const MdPsGpioState *state, uint32_t bank,
                        MdPsGpioOutputs *value);
MdPsGpioLogic md_ps_gpio_irq(const MdPsGpioState *state);
#endif

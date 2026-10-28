/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Zynq7000 PS GPIO reference binding. Rust owns controller state.
 * External inputs are explicit settled samples. No mux, pull, strap,
 * synchronizer latency or physical pad behavior is inferred here.
 */
#include "qemu/osdep.h"
#include "qapi/error.h"
#include "qemu/error-report.h"
#include "qemu/module.h"
#include "hw/core/sysbus.h"
#include "hw/core/irq.h"
#include "hw/core/qdev-clock.h"
#include "migration/vmstate.h"
#include "system/runstate.h"
#include "md-sim-ps-gpio.h"
#include "trace.h"

#define TYPE_MD_SIM_PS_GPIO "md-sim-zynq-ps-gpio"
OBJECT_DECLARE_SIMPLE_TYPE(MdSimPsGpio, MD_SIM_PS_GPIO)
struct MdSimPsGpio {
    SysBusDevice parent_obj;
    MemoryRegion mmio;
    MdPsGpioState *state;
    Clock *pclk;
    bool bus_reset;
    qemu_irq irq;
    qemu_irq output[118], output_known[118], enable[118];
};
static const unsigned widths[4] = { 32, 22, 32, 32 };
static const unsigned starts[4] = { 0, 32, 54, 86 };

static void md_ps_gpio_stop(const char *operation, uint64_t offset)
{
    error_report("md-sim: unsupported PS GPIO %s at0x%" PRIx64,
                 operation, offset);
    qemu_system_vmstop_request(RUN_STATE_INTERNAL_ERROR);
}

static bool md_ps_gpio_update(MdSimPsGpio *s)
{
    for (unsigned bank = 0; bank < 4; bank++) {
        MdPsGpioOutputs out;
        if (!md_ps_gpio_outputs(s->state, bank, &out)) {
            md_ps_gpio_stop("output state", bank);
            return false;
        }
        for (unsigned bit = 0; bit < widths[bank]; bit++) {
            unsigned pin = starts[bank] + bit;
            bool known = (out.data.known >> bit) & 1;
            /* Consumers must honor known and enable. An unknown latch does
             * not publish an invented level on the output signal. */
            qemu_set_irq(s->output_known[pin], known);
            if (known) {
                qemu_set_irq(s->output[pin], (out.data.value >> bit) & 1);
            }
            qemu_set_irq(s->enable[pin], (out.enable >> bit) & 1);
        }
    }
    MdPsGpioLogic irq = md_ps_gpio_irq(s->state);
    if (!(irq.known & 1)) {
        md_ps_gpio_stop("unknown interrupt", 0);
        return false;
    }
    qemu_set_irq(s->irq, irq.value & 1);
    return true;
}

static void md_ps_gpio_pin(void *opaque, int pin, int level)
{
    MdSimPsGpio *s = opaque;
    unsigned bank = 0;
    if (!s->state || pin < 0 || pin >= 118 || (level != 0 && level != 1) ||
        !clock_is_enabled(s->pclk)) {
        md_ps_gpio_stop("input sample", pin);
        return;
    }
    while (bank < 3 && (unsigned)pin >= starts[bank + 1]) {
        bank++;
    }
    uint32_t bit = UINT32_C(1) << (pin - starts[bank]);
    MdPsGpioLogic next = { .value = level ? bit : 0, .known = UINT32_MAX };
    if (!md_ps_gpio_inputs(s->state, bank, next, bit)) {
        md_ps_gpio_stop("unavailable input", pin);
        return;
    }
    md_ps_gpio_update(s);
}

static MemTxResult md_ps_gpio_bus_read(void *opaque, hwaddr addr,
                                      uint64_t *data, unsigned size,
                                      MemTxAttrs attrs)
{
    MdSimPsGpio *s = opaque;
    MdPsGpioLogic result = { 0 };
    bool ok = !s->bus_reset && clock_is_enabled(s->pclk) &&
              size == 4 && !(addr & 3) && addr < 4096 &&
              md_ps_gpio_read(s->state, addr, &result);
    if (!ok || result.known != UINT32_MAX) {
        /* Failure data is never a successful guest read. The stop remains
         * necessary because the legacy board ignores transaction failures. */
        *data = 0;
        md_ps_gpio_stop(ok ? "unknown read" : "read", addr);
        trace_md_sim_ps_gpio_mmio(false, addr, 0, 1);
        return MEMTX_ERROR;
    }
    *data = result.value;
    trace_md_sim_ps_gpio_mmio(false, addr, result.value, 0);
    return MEMTX_OK;
}

static MemTxResult md_ps_gpio_bus_write(void *opaque, hwaddr addr,
                                       uint64_t data, unsigned size,
                                       MemTxAttrs attrs)
{
    MdSimPsGpio *s = opaque;
    bool ok = !s->bus_reset && clock_is_enabled(s->pclk) &&
              size == 4 && !(addr & 3) && addr < 4096 &&
              md_ps_gpio_write(s->state, addr, data);
    if (!ok) {
        trace_md_sim_ps_gpio_mmio(true, addr, data, 1);
        md_ps_gpio_stop("write", addr);
        return MEMTX_ERROR;
    }
    ok = md_ps_gpio_update(s);
    trace_md_sim_ps_gpio_mmio(true, addr, data, !ok);
    return ok ? MEMTX_OK : MEMTX_ERROR;
}

static const MemoryRegionOps md_ps_gpio_ops = {
    .read_with_attrs = md_ps_gpio_bus_read,
    .write_with_attrs = md_ps_gpio_bus_write,
    .endianness = DEVICE_LITTLE_ENDIAN,
    /* Permit these forms to reach the explicit rejection path. They must
     * not be silently expanded into a valid word transaction. */
    .valid = { .min_access_size = 1, .max_access_size = 8, .unaligned = true },
    .impl = { .min_access_size = 1, .max_access_size = 8, .unaligned = true },
};

static void md_ps_gpio_device_reset(DeviceState *dev)
{
    MdSimPsGpio *s = MD_SIM_PS_GPIO(dev);
    /* GPIO_CPU1X_RST resets the bus interface only (UG58514.4.2).
     * QMP/system reset is not a new physical power-on event. */
    md_ps_gpio_bus_reset(s->state);
    md_ps_gpio_update(s);
}

static void md_ps_gpio_bus_reset_pin(void *opaque, int pin, int level)
{
    MdSimPsGpio *s = opaque;
    s->bus_reset = level != 0;
    if (s->state && s->bus_reset) {
        md_ps_gpio_bus_reset(s->state);
    }
}

static void md_ps_gpio_realize(DeviceState *dev, Error **errp)
{
    MdSimPsGpio *s = MD_SIM_PS_GPIO(dev);
    s->state = md_ps_gpio_new();
    if (!s->state) {
        error_setg(errp, "PS GPIO state allocation failed");
    }
}

static void md_ps_gpio_init(Object *obj)
{
    MdSimPsGpio *s = MD_SIM_PS_GPIO(obj);
    memory_region_init_io(&s->mmio, obj, &md_ps_gpio_ops, s,
                         TYPE_MD_SIM_PS_GPIO, 4096);
    sysbus_init_mmio(SYS_BUS_DEVICE(obj), &s->mmio);
    sysbus_init_irq(SYS_BUS_DEVICE(obj), &s->irq);
    s->pclk = qdev_init_clock_in(DEVICE(obj), "pclk", NULL, NULL, 0);
    qdev_init_gpio_in_named(DEVICE(obj), md_ps_gpio_bus_reset_pin, "bus-reset", 1);
    qdev_init_gpio_in_named(DEVICE(obj), md_ps_gpio_pin, "input", 118);
    qdev_init_gpio_out_named(DEVICE(obj), s->output, "output", 118);
    qdev_init_gpio_out_named(DEVICE(obj), s->output_known, "output-known", 118);
    qdev_init_gpio_out_named(DEVICE(obj), s->enable, "output-enable", 118);
}

static void md_ps_gpio_finalize(Object *obj)
{
    md_ps_gpio_free(MD_SIM_PS_GPIO(obj)->state);
}

static const VMStateDescription md_ps_gpio_vmstate = {
    .name = TYPE_MD_SIM_PS_GPIO,
    .unmigratable = 1,
};

static void md_ps_gpio_class_init(ObjectClass *klass, const void *data)
{
    DeviceClass *dc = DEVICE_CLASS(klass);
    dc->realize = md_ps_gpio_realize;
    dc->vmsd = &md_ps_gpio_vmstate;
    device_class_set_legacy_reset(dc, md_ps_gpio_device_reset);
    dc->desc = "Zynq7000 PS GPIO reference; explicit settled external inputs";
}

static const TypeInfo md_ps_gpio_type = {
    .name = TYPE_MD_SIM_PS_GPIO,
    .parent = TYPE_SYS_BUS_DEVICE,
    .instance_size = sizeof(MdSimPsGpio),
    .instance_init = md_ps_gpio_init,
    .instance_finalize = md_ps_gpio_finalize,
    .class_init = md_ps_gpio_class_init,
};

static void md_ps_gpio_register_types(void)
{
    type_register_static(&md_ps_gpio_type);
}
type_init(md_ps_gpio_register_types)

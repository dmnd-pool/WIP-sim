/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Fixed-direction, single-channel AXI GPIO with settled input pins.
 * Atomic word MMIO only. No synchronizer or AXI handshake timing claim.
 */
#include "qemu/osdep.h"
#include "qemu/bitops.h"
#include "qapi/error.h"
#include "hw/core/sysbus.h"
#include "hw/core/irq.h"
#include "hw/core/qdev-properties.h"
#include "migration/vmstate.h"
#include "md-sim-axi-gpio.h"
#include "trace.h"

#define TYPE_MD_SIM_GPIO "md-sim-axi-gpio"
OBJECT_DECLARE_SIMPLE_TYPE(MdSimGpio, MD_SIM_GPIO)
struct MdSimGpio {
    SysBusDevice parent_obj;
    MemoryRegion mmio;
    MdGpioState *state;
    qemu_irq output[32];
    uint32_t width, direction, reset_data, reset_tri, inputs;
};
static void md_gpio_update(MdSimGpio *s)
{
    uint32_t pins = md_gpio_outputs(s->state);
    for (unsigned i = 0; i < 32; i++) {
        qemu_set_irq(s->output[i], (pins >> i) & 1);
    }
}
static void md_gpio_pin(void *opaque, int n, int level)
{
    MdSimGpio *s = opaque;
    s->inputs = deposit32(s->inputs, n, 1, level != 0);
    if (s->state) {
        md_gpio_inputs(s->state, s->inputs);
    }
}
static MemTxResult md_gpio_bus_read(void *opaque, hwaddr addr, uint64_t *data,
                                    unsigned size, MemTxAttrs attrs)
{
    MdSimGpio *s = opaque;
    uint32_t value = 0;
    bool ok = md_gpio_read(s->state, addr & 0x1ff, &value);
    *data = value;
    trace_md_sim_gpio_mmio(s->direction, false, addr, value, !ok);
    return ok ? MEMTX_OK : MEMTX_ERROR;
}
static MemTxResult md_gpio_bus_write(void *opaque, hwaddr addr, uint64_t data,
                                     unsigned size, MemTxAttrs attrs)
{
    MdSimGpio *s = opaque;
    bool ok = md_gpio_write(s->state, addr & 0x1ff, data);
    trace_md_sim_gpio_mmio(s->direction, true, addr, data, !ok);
    md_gpio_update(s);
    return ok ? MEMTX_OK : MEMTX_ERROR;
}
static const MemoryRegionOps md_gpio_ops = {
    .read_with_attrs = md_gpio_bus_read,
    .write_with_attrs = md_gpio_bus_write,
    .endianness = DEVICE_LITTLE_ENDIAN,
    .valid = { .min_access_size = 4, .max_access_size = 4, .unaligned = false },
    .impl = { .min_access_size = 4, .max_access_size = 4 },
};
static void md_gpio_device_reset(DeviceState *dev)
{
    MdSimGpio *s = MD_SIM_GPIO(dev);
    md_gpio_reset(s->state);
    md_gpio_update(s);
}
static void md_gpio_realize(DeviceState *dev, Error **errp)
{
    MdSimGpio *s = MD_SIM_GPIO(dev);
    s->state = md_gpio_new(s->width, s->direction, s->reset_data, s->reset_tri,
                           s->inputs);
    if (!s->state) {
        error_setg(errp, "AXI GPIO requires width1..32 and direction0..1");
    }
}
static void md_gpio_init(Object *obj)
{
    MdSimGpio *s = MD_SIM_GPIO(obj);
    memory_region_init_io(&s->mmio, obj, &md_gpio_ops, s, TYPE_MD_SIM_GPIO, 0x10000);
    sysbus_init_mmio(SYS_BUS_DEVICE(obj), &s->mmio);
    qdev_init_gpio_in_named(DEVICE(obj), md_gpio_pin, "input", 32);
    qdev_init_gpio_out_named(DEVICE(obj), s->output, "output", 32);
}
static void md_gpio_finalize(Object *obj)
{
    md_gpio_free(MD_SIM_GPIO(obj)->state);
}
static const Property md_gpio_properties[] = {
    DEFINE_PROP_UINT32("width", MdSimGpio, width, 0),
    DEFINE_PROP_UINT32("direction", MdSimGpio, direction, 2),
    DEFINE_PROP_UINT32("reset-data", MdSimGpio, reset_data, 0),
    DEFINE_PROP_UINT32("reset-tri", MdSimGpio, reset_tri, UINT32_MAX),
    DEFINE_PROP_UINT32("input-levels", MdSimGpio, inputs, 0),
};
static const VMStateDescription md_gpio_vmstate = {
    .name = TYPE_MD_SIM_GPIO,
    .unmigratable = 1,
};
static void md_gpio_class_init(ObjectClass *klass, const void *data)
{
    DeviceClass *dc = DEVICE_CLASS(klass);
    dc->realize = md_gpio_realize;
    device_class_set_legacy_reset(dc, md_gpio_device_reset);
    device_class_set_props(dc, md_gpio_properties);
    dc->vmsd = &md_gpio_vmstate;
    dc->desc = "Fixed-direction AXI GPIO; settled input pins, atomic word MMIO";
}
static const TypeInfo md_gpio_type = {
    .name = TYPE_MD_SIM_GPIO,
    .parent = TYPE_SYS_BUS_DEVICE,
    .instance_size = sizeof(MdSimGpio),
    .instance_init = md_gpio_init,
    .instance_finalize = md_gpio_finalize,
    .class_init = md_gpio_class_init,
};
static void md_gpio_register_types(void)
{
    type_register_static(&md_gpio_type);
}
type_init(md_gpio_register_types)

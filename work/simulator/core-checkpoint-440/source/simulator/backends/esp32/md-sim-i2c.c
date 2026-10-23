/* SPDX-License-Identifier: GPL-2.0-or-later */
/* QEMU I2C bus and virtual clock binding. Rust owns the S3 controller state. */
#include "qemu/osdep.h"
#include "qemu/module.h"
#include "qemu/timer.h"
#include "hw/sysbus.h"
#include "hw/irq.h"
#include "hw/i2c/i2c.h"
#include "hw/qdev-properties.h"
#include "migration/vmstate.h"
#include "md-sim-i2c.h"
#include "trace.h"
#define TYPE_MD_SIM_I2C "md-sim-esp32s3-i2c"
OBJECT_DECLARE_SIMPLE_TYPE(MdSimI2c, MD_SIM_I2C)
struct MdSimI2c {
    SysBusDevice parent_obj;
    MemoryRegion region;
    MdI2cState *state;
    QEMUTimer *timer;
    I2CBus *bus;
    qemu_irq irq;
    uint32_t index;
};
static int32_t md_bus(void *opaque, uint32_t operation, uint32_t value)
{
    MdSimI2c *s = opaque;
    int32_t result = -1;
    switch (operation) {
    case 0: result = i2c_start_transfer_reselect(s->bus, value & 0x7f, !!(value & 0x100)); break;
    case 1: result = i2c_send(s->bus, value); break;
    case 2: result = i2c_recv(s->bus); break;
    case 3: i2c_end_transfer(s->bus); result = 0; break;
    case 4: i2c_nack(s->bus); result = 0; break;
    }
    trace_md_sim_i2c_bus(s->index, operation, value, result);
    return result;
}
static void md_update(MdSimI2c *s)
{
    if (md_i2c_take_stop(s->state)) { md_bus(s, 3, 0); }
    qemu_set_irq(s->irq, md_i2c_irq(s->state));
    uint64_t due = md_i2c_deadline(s->state);
    if (due == UINT64_MAX) { timer_del(s->timer); }
    else { timer_mod_ns(s->timer, due); }
}
static void md_tick(void *opaque)
{
    MdSimI2c *s = opaque;
    md_i2c_advance(s->state, qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL), s, md_bus);
    trace_md_sim_i2c_state(s->index, md_i2c_bytes(s->state), md_i2c_faults(s->state));
    md_update(s);
}
static uint64_t md_read(void *opaque, hwaddr address, unsigned size)
{
    MdSimI2c *s = opaque;
    uint32_t value = md_i2c_read(s->state, address, size);
    trace_md_sim_i2c_mmio(s->index, false, address, size, value);
    md_update(s);
    return value;
}
static void md_write(void *opaque, hwaddr address, uint64_t value, unsigned size)
{
    MdSimI2c *s = opaque;
    md_i2c_write(s->state, address, size, value, qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL));
    trace_md_sim_i2c_mmio(s->index, true, address, size, value);
    md_update(s);
}
static const MemoryRegionOps md_ops = {
    .read = md_read, .write = md_write, .endianness = DEVICE_LITTLE_ENDIAN,
    .valid = {.min_access_size = 1, .max_access_size = 4},
    .impl = {.min_access_size = 1, .max_access_size = 4},
};
static void md_reset(DeviceState *dev)
{
    MdSimI2c *s = MD_SIM_I2C(dev);
    timer_del(s->timer);
    i2c_end_transfer(s->bus);
    md_i2c_init(s->state);
    qemu_set_irq(s->irq, 0);
}
static void md_init(Object *obj)
{
    MdSimI2c *s = MD_SIM_I2C(obj);
    s->state = g_malloc0(md_i2c_size());
    md_i2c_init(s->state);
    s->timer = timer_new_ns(QEMU_CLOCK_VIRTUAL, md_tick, s);
    memory_region_init_io(&s->region, obj, &md_ops, s, TYPE_MD_SIM_I2C, 0x100);
    sysbus_init_mmio(SYS_BUS_DEVICE(s), &s->region);
    sysbus_init_irq(SYS_BUS_DEVICE(s), &s->irq);
}
static void md_realize(DeviceState *dev, Error **errp)
{
    MdSimI2c *s = MD_SIM_I2C(dev);
    g_autofree char *name = g_strdup_printf("md-sim-i2c-%u", s->index);
    s->bus = i2c_init_bus(dev, name);
}
static void md_finalize(Object *obj)
{
    MdSimI2c *s = MD_SIM_I2C(obj);
    timer_free(s->timer);
    g_free(s->state);
}
static const VMStateDescription md_vmstate = {
    .name = TYPE_MD_SIM_I2C, .unmigratable = 1,
};
static const Property md_properties[] = {
    DEFINE_PROP_UINT32("index", MdSimI2c, index, 0),
};
static void md_class_init(ObjectClass *klass, void *data)
{
    DeviceClass *dc = DEVICE_CLASS(klass);
    dc->realize = md_realize;
    device_class_set_legacy_reset(dc, md_reset);
    device_class_set_props(dc, md_properties);
    dc->vmsd = &md_vmstate;
    dc->user_creatable = false;
}
static const TypeInfo md_info = {
    .name = TYPE_MD_SIM_I2C, .parent = TYPE_SYS_BUS_DEVICE,
    .instance_size = sizeof(MdSimI2c), .instance_init = md_init,
    .instance_finalize = md_finalize, .class_init = md_class_init,
};
static void md_register(void) { type_register_static(&md_info); }
type_init(md_register)

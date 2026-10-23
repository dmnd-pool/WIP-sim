/* SPDX-License-Identifier: GPL-2.0-or-later */
/* QEMU binding for the Rust AXI IIC dynamic-master engine.
 * Exact pinned QEMU source: 11661d857254b27fca5568c6deacf4f6b6f3460f.
 * No physical bus is opened. QEMU's I2C slave bus supplies ACK and data.
 */
#include "qemu/osdep.h"
#include "qapi/error.h"
#include "qemu/log.h"
#include "qemu/timer.h"
#include "hw/core/sysbus.h"
#include "hw/core/irq.h"
#include "hw/core/qdev-properties.h"
#include "hw/i2c/i2c.h"
#include "migration/vmstate.h"
#include "md-sim-axi-iic.h"
#include "trace.h"

#define TYPE_MD_SIM_AXI_IIC "md-sim-axi-iic"
OBJECT_DECLARE_SIMPLE_TYPE(MdSimAxiIic, MD_SIM_AXI_IIC)
struct MdSimAxiIic {
    SysBusDevice parent_obj;
    MemoryRegion mmio;
    qemu_irq irq;
    I2CBus *bus;
    MdIicState *state;
    MdIicBus callbacks;
    QEMUTimer *timer;
    uint32_t bus_hz;
};
static bool md_start(void *opaque, uint8_t address, bool receive)
{
    MdSimAxiIic *s = opaque;
    int result = i2c_start_transfer(s->bus, address, receive);
    trace_md_sim_axi_iic_start(address, receive, result);
    return result == 0;
}
static bool md_send(void *opaque, uint8_t data)
{
    MdSimAxiIic *s = opaque;
    int result = i2c_send(s->bus, data);
    trace_md_sim_axi_iic_send(data, result);
    return result == 0;
}
static uint8_t md_receive(void *opaque)
{
    MdSimAxiIic *s = opaque;
    uint8_t data = i2c_recv(s->bus);
    trace_md_sim_axi_iic_receive(data);
    return data;
}
static void md_nack(void *opaque)
{
    MdSimAxiIic *s = opaque;
    i2c_nack(s->bus);
}
static void md_end(void *opaque)
{
    MdSimAxiIic *s = opaque;
    i2c_end_transfer(s->bus);
    trace_md_sim_axi_iic_end();
}
static void md_update(MdSimAxiIic *s)
{
    qemu_set_irq(s->irq, md_iic_irq(s->state));
    if (md_iic_pending(s->state)) {
        if (!timer_pending(s->timer)) {
            /* Nine clocks per address/data byte, including ACK. Calibration
             * of FPGA timing and analogue clock stretching is still open. */
            timer_mod_ns(s->timer, qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL) +
                         DIV_ROUND_UP(9000000000ULL, s->bus_hz));
        }
    } else {
        timer_del(s->timer);
    }
}
static void md_tick(void *opaque)
{
    MdSimAxiIic *s = opaque;
    md_iic_step(s->state, &s->callbacks);
    md_update(s);
}
static uint64_t md_read(void *opaque, hwaddr addr, unsigned size)
{
    MdSimAxiIic *s = opaque;
    uint32_t value = md_iic_read(s->state, addr, size);
    trace_md_sim_axi_iic_mmio(false, addr, size, value,
                            md_iic_faults(s->state));
    md_update(s);
    return value;
}
static void md_write(void *opaque, hwaddr addr, uint64_t value, unsigned size)
{
    MdSimAxiIic *s = opaque;
    md_iic_write(s->state, addr, size, value);
    trace_md_sim_axi_iic_mmio(true, addr, size, value,
                            md_iic_faults(s->state));
    md_update(s);
}
static const MemoryRegionOps md_ops = {
    .read = md_read,
    .write = md_write,
    .endianness = DEVICE_LITTLE_ENDIAN,
    .valid = { .min_access_size = 1, .max_access_size = 4 },
    .impl = { .min_access_size = 1, .max_access_size = 4 },
};
static void md_reset(DeviceState *dev)
{
    MdSimAxiIic *s = MD_SIM_AXI_IIC(dev);
    timer_del(s->timer);
    md_iic_reset(s->state);
    /* Complete reset's bus release now. No transaction can survive reset. */
    if (md_iic_pending(s->state)) {
        md_iic_step(s->state, &s->callbacks);
    }
    md_update(s);
}
static void md_realize(DeviceState *dev, Error **errp)
{
    MdSimAxiIic *s = MD_SIM_AXI_IIC(dev);
    if (s->bus_hz == 0 || s->bus_hz > 400000) {
        error_setg(errp, "md-sim-axi-iic bus-hz must be 1..400000");
    }
}
static void md_init(Object *obj)
{
    MdSimAxiIic *s = MD_SIM_AXI_IIC(obj);
    s->state = g_malloc0(md_iic_size());
    md_iic_init(s->state);
    s->callbacks = (MdIicBus) { s, md_start, md_send, md_receive, md_nack, md_end };
    s->bus = i2c_init_bus(DEVICE(obj), "i2c");
    s->timer = timer_new_ns(QEMU_CLOCK_VIRTUAL, md_tick, s);
    memory_region_init_io(&s->mmio, obj, &md_ops, s, TYPE_MD_SIM_AXI_IIC, 0x10000);
    sysbus_init_mmio(SYS_BUS_DEVICE(obj), &s->mmio);
    sysbus_init_irq(SYS_BUS_DEVICE(obj), &s->irq);
}
static void md_finalize(Object *obj)
{
    MdSimAxiIic *s = MD_SIM_AXI_IIC(obj);
    timer_free(s->timer);
    g_free(s->state);
}
static const Property md_properties[] = {
    DEFINE_PROP_UINT32("bus-hz", MdSimAxiIic, bus_hz, 100000),
};
/* Refuse migration until an explicit versioned Rust state format exists. */
static const VMStateDescription md_vmstate = {
    .name = TYPE_MD_SIM_AXI_IIC,
    .unmigratable = 1,
};
static void md_class_init(ObjectClass *klass, const void *data)
{
    DeviceClass *dc = DEVICE_CLASS(klass);
    dc->realize = md_realize;
    device_class_set_legacy_reset(dc, md_reset);
    device_class_set_props(dc, md_properties);
    dc->vmsd = &md_vmstate;
    dc->desc = "md-sim AXI IIC dynamic master (Rust state engine)";
}
static const TypeInfo md_type = {
    .name = TYPE_MD_SIM_AXI_IIC,
    .parent = TYPE_SYS_BUS_DEVICE,
    .instance_size = sizeof(MdSimAxiIic),
    .instance_init = md_init,
    .instance_finalize = md_finalize,
    .class_init = md_class_init,
};
static void md_register_types(void)
{
    type_register_static(&md_type);
}
type_init(md_register_types)

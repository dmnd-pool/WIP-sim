/* SPDX-License-Identifier: GPL-2.0-or-later */
/* DS4432 I2C binding. Rust owns register and current state. */
#include "qemu/osdep.h"
#include "qemu/module.h"
#include "qapi/error.h"
#include "hw/i2c/i2c.h"
#include "hw/qdev-properties.h"
#include "migration/vmstate.h"
#include "md-sim-ds4432.h"
#include "trace.h"
#define TYPE_MD_SIM_DS4432 "md-sim-ds4432"
OBJECT_DECLARE_SIMPLE_TYPE(MdSimDs4432, MD_SIM_DS4432)
struct MdSimDs4432 {
    I2CSlave parent_obj;
    MdDs4432State *state;
    uint32_t rfs0;
    uint32_t rfs1;
    uint32_t supply_uv;
};
static int md_event(I2CSlave *slave, enum i2c_event event)
{
    MdSimDs4432 *s = MD_SIM_DS4432(slave);
    switch (event) {
    case I2C_START_SEND: return md_ds4432_start(s->state, false) ? 0 : -1;
    case I2C_START_RECV: return md_ds4432_start(s->state, true) ? 0 : -1;
    case I2C_FINISH: md_ds4432_stop(s->state); return 0;
    case I2C_NACK: md_ds4432_nack(s->state); return 0;
    default: return -1;
    }
}
static int md_send(I2CSlave *slave, uint8_t value)
{
    MdSimDs4432 *s = MD_SIM_DS4432(slave);
    int result = md_ds4432_write(s->state, value) ? 0 : -1;
    trace_md_sim_ds4432_bus(false, value, result, md_ds4432_writes(s->state), md_ds4432_faults(s->state));
    return result;
}
static uint8_t md_recv(I2CSlave *slave)
{
    MdSimDs4432 *s = MD_SIM_DS4432(slave);
    int32_t result = md_ds4432_read(s->state);
    trace_md_sim_ds4432_bus(true, result, result < 0 ? -1 : 0, md_ds4432_writes(s->state), md_ds4432_faults(s->state));
    return result < 0 ? 0xff : result;
}
static void md_restart(I2CSlave *slave)
{
    /* Repeated START abandons this byte phase without power reset. */
    md_ds4432_stop(MD_SIM_DS4432(slave)->state);
}
static bool md_match(I2CSlave *candidate, uint8_t address, bool broadcast,
                     I2CNodeList *current_devs)
{
    if (broadcast || address != 0x48) { return false; }
    I2CNode *node = g_new(I2CNode, 1);
    node->elt = candidate;
    QLIST_INSERT_HEAD(current_devs, node, next);
    return true;
}
static void md_reset(DeviceState *dev)
{
    MdSimDs4432 *s = MD_SIM_DS4432(dev);
    if (s->state) { md_ds4432_reset(s->state); }
}
static void md_realize(DeviceState *dev, Error **errp)
{
    MdSimDs4432 *s = MD_SIM_DS4432(dev);
    if (I2C_SLAVE(dev)->address != 0x48) {
        error_setg(errp, "DS4432 has fixed 7-bit address 0x48");
        return;
    }
    s->state = g_malloc0(md_ds4432_size());
    if (!md_ds4432_init(s->state, s->rfs0, s->rfs1, s->supply_uv)) {
        g_clear_pointer(&s->state, g_free);
        error_setg(errp, "DS4432 FS resistor must be disconnected (0) or 40000..160000 ohms");
    }
}
static void md_finalize(Object *obj)
{
    g_free(MD_SIM_DS4432(obj)->state);
}
static const VMStateDescription md_vmstate = {
    .name = TYPE_MD_SIM_DS4432, .unmigratable = 1,
};
static const Property md_properties[] = {
    DEFINE_PROP_UINT32("rfs0-ohms", MdSimDs4432, rfs0, 0),
    DEFINE_PROP_UINT32("rfs1-ohms", MdSimDs4432, rfs1, 0),
    DEFINE_PROP_UINT32("supply-uv", MdSimDs4432, supply_uv, 3300000),
};
static void md_class_init(ObjectClass *klass, void *data)
{
    DeviceClass *dc = DEVICE_CLASS(klass);
    I2CSlaveClass *sc = I2C_SLAVE_CLASS(klass);
    dc->realize = md_realize;
    device_class_set_legacy_reset(dc, md_reset);
    device_class_set_props(dc, md_properties);
    dc->vmsd = &md_vmstate;
    sc->event = md_event;
    sc->send = md_send;
    sc->recv = md_recv;
    sc->restart = md_restart;
    sc->match_and_add = md_match;
}
static const TypeInfo md_info = {
    .name = TYPE_MD_SIM_DS4432, .parent = TYPE_I2C_SLAVE,
    .instance_size = sizeof(MdSimDs4432),
    .instance_finalize = md_finalize, .class_init = md_class_init,
};
static void md_register(void) { type_register_static(&md_info); }
type_init(md_register)

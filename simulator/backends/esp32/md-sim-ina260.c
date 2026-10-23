/* SPDX-License-Identifier: GPL-2.0-or-later */
/* INA260 fixed A0=A1=GND fixture. Explicit electrical inputs are mandatory. */
#include "qemu/osdep.h"
#include "qemu/module.h"
#include "qemu/timer.h"
#include "qapi/error.h"
#include "hw/i2c/i2c.h"
#include "hw/irq.h"
#include "hw/qdev-properties.h"
#include "migration/vmstate.h"
#include "md-sim-ina260.h"
#include "trace.h"
#define TYPE_MD_SIM_INA260 "md-sim-ina260"
OBJECT_DECLARE_SIMPLE_TYPE(MdSimIna260, MD_SIM_INA260)
struct MdSimIna260 {
    I2CSlave parent_obj;
    MdIna260State *state;
    QEMUTimer *timer;
    qemu_irq alert;
    uint32_t supply_uv;
    uint32_t vin_uv;
    int32_t current_ua;
};
static uint64_t md_now(void) { return qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL); }
static void md_sync(MdSimIna260 *s)
{
    uint64_t due = md_ina260_deadline(s->state);
    if (due == UINT64_MAX || due > INT64_MAX) { timer_del(s->timer); }
    else { timer_mod_ns(s->timer, due); }
    qemu_set_irq(s->alert, md_ina260_alert_level(s->state));
    trace_md_sim_ina260_state(md_now(), md_ina260_conversions(s->state), md_ina260_faults(s->state));
}
static void md_timer(void *opaque)
{
    MdSimIna260 *s = opaque;
    md_ina260_advance(s->state, md_now());
    md_sync(s);
}
static int md_event(I2CSlave *slave, enum i2c_event event)
{
    MdSimIna260 *s = MD_SIM_INA260(slave);
    int result = 0;
    switch (event) {
    case I2C_START_SEND: result = md_ina260_start(s->state, false, md_now()) ? 0 : -1; break;
    case I2C_START_RECV: result = md_ina260_start(s->state, true, md_now()) ? 0 : -1; break;
    case I2C_FINISH: md_ina260_stop(s->state, md_now()); break;
    case I2C_NACK: md_ina260_nack(s->state); break;
    default: result = -1; break;
    }
    md_sync(s);
    return result;
}
static int md_send(I2CSlave *slave, uint8_t value)
{
    MdSimIna260 *s = MD_SIM_INA260(slave);
    int result = md_ina260_write(s->state, value, md_now()) ? 0 : -1;
    trace_md_sim_ina260_bus(false, value, result, md_ina260_writes(s->state), md_ina260_faults(s->state));
    md_sync(s);
    return result;
}
static uint8_t md_recv(I2CSlave *slave)
{
    MdSimIna260 *s = MD_SIM_INA260(slave);
    int32_t result = md_ina260_read(s->state, md_now());
    trace_md_sim_ina260_bus(true, result, result < 0 ? -1 : 0, md_ina260_writes(s->state), md_ina260_faults(s->state));
    md_sync(s);
    return result < 0 ? 0xff : result;
}
static void md_restart(I2CSlave *slave)
{
    MdSimIna260 *s = MD_SIM_INA260(slave);
    md_ina260_stop(s->state, md_now()); md_sync(s);
}
static bool md_match(I2CSlave *candidate, uint8_t address, bool broadcast, I2CNodeList *current_devs)
{
    /* ARA and dynamic address pins are not implemented. Never ACK these as a
     * normal register transfer. Existing QEMU handles actual bus selection. */
    if (broadcast || address != 0x40) { return false; }
    I2CNode *node = g_new(I2CNode, 1);
    node->elt = candidate;
    QLIST_INSERT_HEAD(current_devs, node, next);
    return true;
}
static void md_reset(DeviceState *dev)
{
    MdSimIna260 *s = MD_SIM_INA260(dev);
    if (s->state) { md_ina260_reset(s->state, md_now()); md_sync(s); }
}
static void md_realize(DeviceState *dev, Error **errp)
{
    MdSimIna260 *s = MD_SIM_INA260(dev);
    if (I2C_SLAVE(dev)->address != 0x40) {
        error_setg(errp, "INA260 fixture requires A0=A1=GND at address 0x40"); return;
    }
    s->state = g_malloc0(md_ina260_size());
    if (!md_ina260_init(s->state, s->supply_uv, s->vin_uv, s->current_ua, md_now())) {
        g_clear_pointer(&s->state, g_free);
        error_setg(errp, "INA260 needs supply 2.7..5.5V, explicit VIN 0..36V and current -15..15A"); return;
    }
    s->timer = timer_new_ns(QEMU_CLOCK_VIRTUAL, md_timer, s);
    qdev_init_gpio_out_named(dev, &s->alert, "alert", 1);
    md_sync(s);
}
static void md_finalize(Object *obj)
{
    MdSimIna260 *s = MD_SIM_INA260(obj);
    if (s->timer) { timer_free(s->timer); }
    g_free(s->state);
}
static const VMStateDescription md_vmstate = { .name = TYPE_MD_SIM_INA260, .unmigratable = 1 };
static const Property md_properties[] = {
    DEFINE_PROP_UINT32("supply-uv", MdSimIna260, supply_uv, 3300000),
    DEFINE_PROP_UINT32("vin-uv", MdSimIna260, vin_uv, UINT32_MAX),
    DEFINE_PROP_INT32("current-ua", MdSimIna260, current_ua, INT32_MIN),
};
static void md_class_init(ObjectClass *klass, void *data)
{
    DeviceClass *dc = DEVICE_CLASS(klass);
    I2CSlaveClass *sc = I2C_SLAVE_CLASS(klass);
    dc->realize = md_realize;
    device_class_set_legacy_reset(dc, md_reset);
    device_class_set_props(dc, md_properties);
    dc->vmsd = &md_vmstate;
    sc->event = md_event; sc->send = md_send; sc->recv = md_recv;
    sc->restart = md_restart; sc->match_and_add = md_match;
}
static const TypeInfo md_info = {
    .name = TYPE_MD_SIM_INA260, .parent = TYPE_I2C_SLAVE,
    .instance_size = sizeof(MdSimIna260), .instance_finalize = md_finalize,
    .class_init = md_class_init,
};
static void md_register(void) { type_register_static(&md_info); }
type_init(md_register)

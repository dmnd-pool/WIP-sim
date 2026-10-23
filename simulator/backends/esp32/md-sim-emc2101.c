/* SPDX-License-Identifier: GPL-2.0-or-later */
/* EMC2101-R host-load binding. Rust owns registers and conversion state. */
#include "qemu/osdep.h"
#include "qemu/module.h"
#include "qemu/timer.h"
#include "qapi/error.h"
#include "hw/i2c/i2c.h"
#include "hw/irq.h"
#include "hw/qdev-properties.h"
#include "migration/vmstate.h"
#include "md-sim-emc2101.h"
#include "trace.h"
#define TYPE_MD_SIM_EMC2101 "md-sim-emc2101"
OBJECT_DECLARE_SIMPLE_TYPE(MdSimEmc2101, MD_SIM_EMC2101)
struct MdSimEmc2101 {
    I2CSlave parent_obj;
    MdEmc2101State *state;
    QEMUTimer *timer;
    qemu_irq alert;
    uint32_t supply_uv;
    uint32_t external_kind;
    int32_t internal_eighths;
    int32_t external_eighths;
    bool boot_full;
};
static uint64_t md_now(void) { return qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL); }
static void md_sync(MdSimEmc2101 *s)
{
    uint64_t due = md_emc2101_deadline(s->state);
    if (due == UINT64_MAX || due > INT64_MAX) { timer_del(s->timer); }
    else { timer_mod_ns(s->timer, due); }
    qemu_set_irq(s->alert, !md_emc2101_alert(s->state));
    trace_md_sim_emc2101_state(md_now(), md_emc2101_conversions(s->state),
        md_emc2101_faults(s->state), md_emc2101_fan_setting(s->state, md_now()));
}
static void md_timer(void *opaque)
{
    MdSimEmc2101 *s = opaque;
    md_emc2101_advance(s->state, md_now());
    md_sync(s);
}
static int md_event(I2CSlave *slave, enum i2c_event event)
{
    MdSimEmc2101 *s = MD_SIM_EMC2101(slave);
    int result = 0;
    switch (event) {
    case I2C_START_SEND: result = md_emc2101_start(s->state, false, md_now()) ? 0 : -1; break;
    case I2C_START_RECV: result = md_emc2101_start(s->state, true, md_now()) ? 0 : -1; break;
    case I2C_FINISH: md_emc2101_stop(s->state); break;
    case I2C_NACK: md_emc2101_nack(s->state); break;
    default: result = -1; break;
    }
    md_sync(s);
    return result;
}
static int md_send(I2CSlave *slave, uint8_t value)
{
    MdSimEmc2101 *s = MD_SIM_EMC2101(slave);
    int result = md_emc2101_write(s->state, value, md_now()) ? 0 : -1;
    trace_md_sim_emc2101_bus(false, value, result, md_emc2101_writes(s->state), md_emc2101_faults(s->state));
    md_sync(s);
    return result;
}
static uint8_t md_recv(I2CSlave *slave)
{
    MdSimEmc2101 *s = MD_SIM_EMC2101(slave);
    int32_t result = md_emc2101_read(s->state, md_now());
    trace_md_sim_emc2101_bus(true, result, result < 0 ? -1 : 0, md_emc2101_writes(s->state), md_emc2101_faults(s->state));
    md_sync(s);
    return result < 0 ? 0xff : result;
}
static void md_restart(I2CSlave *slave) { md_emc2101_stop(MD_SIM_EMC2101(slave)->state); }
static bool md_match(I2CSlave *candidate, uint8_t address, bool broadcast, I2CNodeList *current_devs)
{
    if (broadcast || address != 0x4c) { return false; }
    I2CNode *node = g_new(I2CNode, 1);
    node->elt = candidate;
    QLIST_INSERT_HEAD(current_devs, node, next);
    return true;
}
static void md_tach(void *opaque, int index, int level)
{
    MdSimEmc2101 *s = opaque;
    md_emc2101_tach(s->state, level != 0, md_now());
    md_sync(s);
}
static void md_reset(DeviceState *dev)
{
    MdSimEmc2101 *s = MD_SIM_EMC2101(dev);
    if (s->state) { md_emc2101_reset(s->state, md_now()); md_sync(s); }
}
static void md_realize(DeviceState *dev, Error **errp)
{
    MdSimEmc2101 *s = MD_SIM_EMC2101(dev);
    if (I2C_SLAVE(dev)->address != 0x4c) {
        error_setg(errp, "EMC2101 has fixed 7-bit address 0x4c"); return;
    }
    s->state = g_malloc0(md_emc2101_size());
    if (!md_emc2101_init(s->state, s->supply_uv, s->boot_full, s->internal_eighths,
        s->external_kind, s->external_eighths, md_now())) {
        g_clear_pointer(&s->state, g_free);
        error_setg(errp, "EMC2101 needs supply 3.0..3.6V, explicit internal temperature, and valid external input");
        return;
    }
    s->timer = timer_new_ns(QEMU_CLOCK_VIRTUAL, md_timer, s);
    qdev_init_gpio_in_named(dev, md_tach, "tach", 1);
    qdev_init_gpio_out_named(dev, &s->alert, "alert", 1);
    md_sync(s);
}
static void md_finalize(Object *obj)
{
    MdSimEmc2101 *s = MD_SIM_EMC2101(obj);
    if (s->timer) { timer_free(s->timer); }
    g_free(s->state);
}
static const VMStateDescription md_vmstate = { .name = TYPE_MD_SIM_EMC2101, .unmigratable = 1 };
static const Property md_properties[] = {
    DEFINE_PROP_UINT32("supply-uv", MdSimEmc2101, supply_uv, 3300000),
    DEFINE_PROP_BOOL("host-boot-full", MdSimEmc2101, boot_full, false),
    DEFINE_PROP_INT32("internal-eighths", MdSimEmc2101, internal_eighths, INT32_MIN),
    DEFINE_PROP_UINT32("external-kind", MdSimEmc2101, external_kind, 0),
    DEFINE_PROP_INT32("external-eighths", MdSimEmc2101, external_eighths, 0),
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
    .name = TYPE_MD_SIM_EMC2101, .parent = TYPE_I2C_SLAVE,
    .instance_size = sizeof(MdSimEmc2101), .instance_finalize = md_finalize,
    .class_init = md_class_init,
};
static void md_register(void) { type_register_static(&md_info); }
type_init(md_register)

/* SPDX-License-Identifier: GPL-2.0-or-later */
/* S9 PIC protocol contract. This is not PIC instruction execution. */
#include "qemu/osdep.h"
#include "qapi/error.h"
#include "qemu/error-report.h"
#include "qemu/timer.h"
#include "hw/i2c/i2c.h"
#include "hw/core/irq.h"
#include "hw/core/qdev-properties.h"
#include "migration/vmstate.h"
#include "system/runstate.h"
#include "md-sim-pic16.h"
#include "trace.h"

#define TYPE_MD_SIM_PIC "md-sim-s9-pic16"
OBJECT_DECLARE_SIMPLE_TYPE(MdSimPic, MD_SIM_PIC)
struct MdSimPic {
    I2CSlave parent_obj;
    MdPicState *state;
    QEMUTimer *timer;
    qemu_irq power_enable;
    char *application;
    uint32_t last_power;
    bool failed;
};
static void md_pic_update(MdSimPic *s, uint32_t result, int64_t now)
{
    uint32_t power = md_pic_power(s->state);
    if (power != s->last_power) {
        /* A connected consumer can spend time advancing its model. Record
         * the producer transition before delivery, at the input timestamp,
         * rather than reading a later real-time virtual clock afterward. */
        trace_md_sim_pic_power(s->parent_obj.address, power, now);
        s->last_power = power;
    }
    qemu_set_irq(s->power_enable, power & 1);
    if (result > 1) {
        if (!s->failed) {
            error_report("S9 PIC0x%02x: unsupported contract state0x%x",
                         s->parent_obj.address, result);
            qemu_system_vmstop_request(RUN_STATE_INTERNAL_ERROR);
        }
        s->failed = true;
    }
    uint64_t deadline = md_pic_deadline(s->state);
    if (s->failed || deadline == UINT64_MAX) {
        timer_del(s->timer);
    } else if (deadline <= INT64_MAX) {
        timer_mod_ns(s->timer, deadline);
    } else {
        s->failed = true;
        timer_del(s->timer);
        error_report("S9 PIC timer exceeds QEMU virtual clock range");
        qemu_system_vmstop_request(RUN_STATE_INTERNAL_ERROR);
    }
}
static void md_pic_tick(void *opaque)
{
    MdSimPic *s = opaque;
    int64_t now = qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL);
    uint32_t result = md_pic_advance(s->state, now);
    trace_md_sim_pic_event(s->parent_obj.address, "timer", 0,
                           now, result);
    md_pic_update(s, result, now);
}
static int md_pic_bus_event(I2CSlave *slave, enum i2c_event event)
{
    MdSimPic *s = MD_SIM_PIC(slave);
    int64_t now = qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL);
    uint32_t result = 0;
    if (s->failed) {
        return 1;
    }
    switch (event) {
    case I2C_START_RECV:
    case I2C_START_SEND:
        result = md_pic_start(s->state, now, event == I2C_START_RECV);
        break;
    case I2C_FINISH:
    case I2C_NACK:
        md_pic_stop(s->state);
        break;
    default:
        result = 0x200;
    }
    trace_md_sim_pic_event(slave->address, "bus", event,
                           now, result);
    md_pic_update(s, result, now);
    return result != 0;
}
static int md_pic_bus_send(I2CSlave *slave, uint8_t data)
{
    MdSimPic *s = MD_SIM_PIC(slave);
    int64_t now = qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL);
    uint32_t result = s->failed ? 0x201 :
        md_pic_send(s->state, now, data);
    trace_md_sim_pic_event(slave->address, "send", data,
                           now, result);
    md_pic_update(s, result, now);
    return result != 0;
}
static uint8_t md_pic_bus_recv(I2CSlave *slave)
{
    MdSimPic *s = MD_SIM_PIC(slave);
    int64_t now = qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL);
    uint32_t data = 0, result = s->failed ? 0x201 :
        md_pic_recv(s->state, now, &data);
    /* I2CSlave.recv cannot signal failure. Stop the VM; the returned zero
     * on failure is an invalid transaction, never successful model data. */
    if (result == 1) {
        result = 0x202;
    }
    trace_md_sim_pic_event(slave->address, "receive", data,
                           now, result);
    md_pic_update(s, result, now);
    return data & 0xff;
}
static void md_pic_device_reset(DeviceState *dev)
{
    MdSimPic *s = MD_SIM_PIC(dev);
    if (s->state) {
        s->failed = false;
        int64_t now = qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL);
        uint32_t result = md_pic_reset(s->state, now);
        trace_md_sim_pic_event(s->parent_obj.address, "external-reset", 0,
                               now, result);
        md_pic_update(s, result, now);
    }
}
static void md_pic_realize(DeviceState *dev, Error **errp)
{
    MdSimPic *s = MD_SIM_PIC(dev);
    uint8_t bytes[19201];
    if (!s->application) {
        error_setg(errp, "PIC development model requires application-text");
        return;
    }
    FILE *file = fopen(s->application, "rb");
    if (!file) {
        error_setg_errno(errp, errno, "Cannot open PIC application");
        return;
    }
    size_t length = fread(bytes, 1, sizeof(bytes), file);
    bool invalid = ferror(file) || !feof(file) || length > 19200;
    fclose(file);
    if (!invalid) {
        s->state = md_pic_new(bytes, length);
    }
    if (!s->state) {
        error_setg(errp, "PIC application length, format or program identity differs");
    }
}
static void md_pic_init(Object *obj)
{
    MdSimPic *s = MD_SIM_PIC(obj);
    s->timer = timer_new_ns(QEMU_CLOCK_VIRTUAL, md_pic_tick, s);
    qdev_init_gpio_out_named(DEVICE(obj), &s->power_enable, "power-enable", 1);
}
static void md_pic_finalize(Object *obj)
{
    MdSimPic *s = MD_SIM_PIC(obj);
    timer_free(s->timer);
    md_pic_free(s->state);
}
static const Property md_pic_properties[] = {
    DEFINE_PROP_STRING("application-text", MdSimPic, application),
};
static const VMStateDescription md_pic_vmstate = {
    .name = TYPE_MD_SIM_PIC,
    .unmigratable = 1,
};
static void md_pic_class_init(ObjectClass *klass, const void *data)
{
    DeviceClass *dc = DEVICE_CLASS(klass);
    I2CSlaveClass *ic = I2C_SLAVE_CLASS(klass);
    ic->event = md_pic_bus_event;
    ic->send = md_pic_bus_send;
    ic->recv = md_pic_bus_recv;
    dc->realize = md_pic_realize;
    device_class_set_legacy_reset(dc, md_pic_device_reset);
    device_class_set_props(dc, md_pic_properties);
    dc->vmsd = &md_pic_vmstate;
    dc->desc = "S9 PIC protocol contract with derived erased storage; no PIC CPU";
}
static const TypeInfo md_pic_type = {
    .name = TYPE_MD_SIM_PIC,
    .parent = TYPE_I2C_SLAVE,
    .instance_size = sizeof(MdSimPic),
    .instance_init = md_pic_init,
    .instance_finalize = md_pic_finalize,
    .class_init = md_pic_class_init,
};
static void md_pic_register_types(void)
{
    type_register_static(&md_pic_type);
}
type_init(md_pic_register_types)

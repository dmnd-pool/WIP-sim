/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Development fan contract. Motor parameters are explicit fixtures. */
#include "qemu/osdep.h"
#include "qapi/error.h"
#include "qemu/error-report.h"
#include "qemu/timer.h"
#include "hw/core/sysbus.h"
#include "hw/core/irq.h"
#include "hw/core/qdev-properties.h"
#include "migration/vmstate.h"
#include "system/runstate.h"
#include "md-sim-fan.h"
#include "trace.h"

#define TYPE_MD_SIM_FAN "md-sim-fan"
OBJECT_DECLARE_SIMPLE_TYPE(MdSimFan, MD_SIM_FAN)
struct MdSimFan {
    SysBusDevice parent_obj;
    MemoryRegion mmio;
    MdFanState *state;
    QEMUTimer *timer;
    int64_t last_ns;
    char *input_plan;
    bool fixture_motors, failed;
};
static void md_fan_fail(MdSimFan *s)
{
    if (!s->failed) {
        error_report("Fan controller: unsupported model operation");
        s->failed = true;
        timer_del(s->timer);
        qemu_system_vmstop_request(RUN_STATE_INTERNAL_ERROR);
    }
}
static void md_fan_sync(MdSimFan *s)
{
    int64_t now = qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL);
    if (s->failed) {
        return;
    }
    if (now < s->last_ns) {
        error_report("Fan clock moved backwards: now=%" PRId64
                     " previous=%" PRId64, now, s->last_ns);
        md_fan_fail(s);
        return;
    }
    if (!md_fan_advance_elapsed(s->state, now - s->last_ns)) {
        error_report("Fan advance rejected: now=%" PRId64
                     " previous=%" PRId64 " elapsed=%" PRId64,
                     now, s->last_ns, now - s->last_ns);
        md_fan_fail(s);
        return;
    }
    s->last_ns = now;
    MdFanInput input;
    while (md_fan_take_input(s->state, &input)) {
        trace_md_sim_fan_input(input.at_ns, input.motor, input.mask, input.values);
    }
    timer_mod_ns(s->timer, now + md_fan_next_event(s->state));
}
static void md_fan_tick(void *opaque)
{
    md_fan_sync(opaque);
}
static void md_fan_stall_pin(void *opaque, int n, int level)
{
    MdSimFan *s = opaque;
    if (s->state && s->fixture_motors) {
        md_fan_sync(s);
        if (!md_fan_stalled(s->state, n, level != 0)) {
            md_fan_fail(s);
        }
    }
}
static void md_fan_power_pin(void *opaque, int n, int level)
{
    MdSimFan *s = opaque;
    if (s->state && s->fixture_motors) {
        md_fan_sync(s);
        if (!md_fan_powered(s->state, n, level != 0)) {
            md_fan_fail(s);
        }
    }
}
static MemTxResult md_fan_bus_read(void *opaque, hwaddr addr, uint64_t *data,
                                   unsigned size, MemTxAttrs attrs)
{
    MdSimFan *s = opaque;
    uint32_t value = 0;
    md_fan_sync(s);
    bool ok = !s->failed && md_fan_read(s->state, addr, size, &value);
    *data = value;
    trace_md_sim_fan_mmio(s->last_ns, false, addr, size, value, !ok);
    return ok ? MEMTX_OK : MEMTX_ERROR;
}
static MemTxResult md_fan_bus_write(void *opaque, hwaddr addr, uint64_t value,
                                    unsigned size, MemTxAttrs attrs)
{
    MdSimFan *s = opaque;
    md_fan_sync(s);
    bool ok = !s->failed && md_fan_write(s->state, addr, size, value);
    trace_md_sim_fan_mmio(s->last_ns, true, addr, size, value, !ok);
    if (!ok) {
        error_report("Fan write rejected: address=0x%" HWADDR_PRIx
                     " size=%u value=0x%" PRIx64, addr, size, value);
        md_fan_fail(s);
    }
    return ok ? MEMTX_OK : MEMTX_ERROR;
}
static const MemoryRegionOps md_fan_ops = {
    .read_with_attrs = md_fan_bus_read,
    .write_with_attrs = md_fan_bus_write,
    .endianness = DEVICE_LITTLE_ENDIAN,
    .valid = { .min_access_size = 1, .max_access_size = 4, .unaligned = false },
    .impl = { .min_access_size = 1, .max_access_size = 4 },
};
static void md_fan_device_reset(DeviceState *dev)
{
    MdSimFan *s = MD_SIM_FAN(dev);
    /* Motor motion is external state, so it survives controller reset. */
    md_fan_sync(s);
    md_fan_reset(s->state);
    s->failed = false;
    s->last_ns = qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL);
    timer_mod_ns(s->timer, s->last_ns + md_fan_next_event(s->state));
}
static void md_fan_realize(DeviceState *dev, Error **errp)
{
    MdSimFan *s = MD_SIM_FAN(dev);
    /* Explicit uncalibrated AM1 topology stimulus, not a physical fan identity.
     * PWM0 has no tach wire; PWM1 reports on channel1. */
    const MdFanConfig configs[] = {
        {900, 6000, 12000, 2, 0, UINT32_MAX},
        {900, 6000, 12000, 2, 1, 1},
    };
    s->state = md_fan_new(configs, s->fixture_motors ? 2 : 0);
    if (!s->state) {
        error_setg(errp, "Fan controller configuration rejected");
        return;
    }
    s->last_ns = qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL);
    if (s->input_plan && (s->last_ns != 0 || strlen(s->input_plan) > 4096 ||
        !md_fan_input_plan(s->state, (const uint8_t *)s->input_plan,
                           strlen(s->input_plan)))) {
        error_setg(errp, "Fan input plan rejected");
    }
}
static void md_fan_init(Object *obj)
{
    MdSimFan *s = MD_SIM_FAN(obj);
    s->timer = timer_new_ns(QEMU_CLOCK_VIRTUAL, md_fan_tick, s);
    memory_region_init_io(&s->mmio, obj, &md_fan_ops, s, TYPE_MD_SIM_FAN, 0x10000);
    sysbus_init_mmio(SYS_BUS_DEVICE(obj), &s->mmio);
    qdev_init_gpio_in_named(DEVICE(obj), md_fan_stall_pin, "stall", 2);
    qdev_init_gpio_in_named(DEVICE(obj), md_fan_power_pin, "power", 2);
}
static void md_fan_finalize(Object *obj)
{
    MdSimFan *s = MD_SIM_FAN(obj);
    timer_free(s->timer);
    md_fan_free(s->state);
}
static const Property md_fan_properties[] = {
    DEFINE_PROP_BOOL("reference-motors", MdSimFan, fixture_motors, false),
    DEFINE_PROP_STRING("input-plan", MdSimFan, input_plan),
};
static const VMStateDescription md_fan_vmstate = {
    .name = TYPE_MD_SIM_FAN,
    .unmigratable = 1,
};
static void md_fan_class_init(ObjectClass *klass, const void *data)
{
    DeviceClass *dc = DEVICE_CLASS(klass);
    dc->realize = md_fan_realize;
    device_class_set_legacy_reset(dc, md_fan_device_reset);
    device_class_set_props(dc, md_fan_properties);
    dc->vmsd = &md_fan_vmstate;
    dc->desc = "AM1 fan reference contract; explicit uncalibrated motor parameters";
}
static const TypeInfo md_fan_type = {
    .name = TYPE_MD_SIM_FAN,
    .parent = TYPE_SYS_BUS_DEVICE,
    .instance_size = sizeof(MdSimFan),
    .instance_init = md_fan_init,
    .instance_finalize = md_fan_finalize,
    .class_init = md_fan_class_init,
};
static void md_fan_register_types(void)
{
    type_register_static(&md_fan_type);
}
type_init(md_fan_register_types)

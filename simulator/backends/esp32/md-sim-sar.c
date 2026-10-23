/* SPDX-License-Identifier: GPL-2.0-or-later */
/* ESP32-S3 RTC SAR and internal analog configuration binding. The Rust
 * controller owns conversion state; QEMU supplies virtual time. */
#include "qemu/osdep.h"
#include "qemu/module.h"
#include "qemu/timer.h"
#include "qapi/error.h"
#include "hw/qdev-properties.h"
#include "hw/sysbus.h"
#include "migration/vmstate.h"
#include "md-sim-sar.h"
#include "trace.h"
#define TYPE_MD_SIM_SAR "md-sim-esp32s3-sar"
OBJECT_DECLARE_SIMPLE_TYPE(MdSimSar, MD_SIM_SAR)
typedef struct MdSarPort { MdSimSar *owner; uint32_t bank; } MdSarPort;
struct MdSimSar {
    SysBusDevice parent_obj;
    MemoryRegion regions[2];
    MdSarPort ports[2];
    MdSarState *state;
    QEMUTimer *timer;
    int32_t input_uv[20];
};
static void md_update(MdSimSar *s)
{
    uint64_t due = md_sar_deadline(s->state);
    if (due == UINT64_MAX) { timer_del(s->timer); }
    else { timer_mod_ns(s->timer, due); }
}
static void md_tick(void *opaque)
{
    MdSimSar *s = opaque;
    md_sar_advance(s->state, qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL));
    trace_md_sim_sar_complete(md_sar_completed(s->state), md_sar_faults(s->state));
    md_update(s);
}
static uint64_t md_read(void *opaque, hwaddr addr, unsigned size)
{
    MdSarPort *p = opaque;
    uint32_t value = md_sar_read(p->owner->state, p->bank, addr, size);
    trace_md_sim_sar_mmio(false, p->bank, addr, size, value, md_sar_faults(p->owner->state));
    return value;
}
static void md_write(void *opaque, hwaddr addr, uint64_t value, unsigned size)
{
    MdSarPort *p = opaque;
    md_sar_write(p->owner->state, p->bank, addr, size, value, qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL));
    trace_md_sim_sar_mmio(true, p->bank, addr, size, value, md_sar_faults(p->owner->state));
    md_update(p->owner);
}
static const MemoryRegionOps md_ops = {
    .read = md_read, .write = md_write, .endianness = DEVICE_LITTLE_ENDIAN,
    .valid = {.min_access_size = 1, .max_access_size = 4},
    .impl = {.min_access_size = 1, .max_access_size = 4},
};
static void md_reset(DeviceState *dev)
{
    MdSimSar *s = MD_SIM_SAR(dev);
    timer_del(s->timer);
    md_sar_reset(s->state);
}
static void md_realize(DeviceState *dev, Error **errp)
{
    MdSimSar *s = MD_SIM_SAR(dev);
    for (unsigned i = 0; i < 20; i++) {
        if (!md_sar_set_input(s->state, i / 10, i % 10, s->input_uv[i])) {
            error_setg(errp, "SAR ADC%u channel%u input must be -1 (disconnected) or 0..3300000 microvolts",
                       i / 10 + 1, i % 10);
            return;
        }
    }
}
/* Initial external sources. These values are independent of guest voltage
 * requests. They do not model a regulator, divider or board connection. */
#define MD_INPUT(unit, ch) \
    DEFINE_PROP_INT32("adc" #unit "-channel" #ch "-uv", MdSimSar, \
                      input_uv[((unit) - 1) * 10 + (ch)], -1)
static const Property md_properties[] = {
    MD_INPUT(1, 0), MD_INPUT(1, 1), MD_INPUT(1, 2), MD_INPUT(1, 3),
    MD_INPUT(1, 4), MD_INPUT(1, 5), MD_INPUT(1, 6), MD_INPUT(1, 7),
    MD_INPUT(1, 8), MD_INPUT(1, 9),
    MD_INPUT(2, 0), MD_INPUT(2, 1), MD_INPUT(2, 2), MD_INPUT(2, 3),
    MD_INPUT(2, 4), MD_INPUT(2, 5), MD_INPUT(2, 6), MD_INPUT(2, 7),
    MD_INPUT(2, 8), MD_INPUT(2, 9),
};
#undef MD_INPUT
static void md_init(Object *obj)
{
    MdSimSar *s = MD_SIM_SAR(obj);
    s->state = g_malloc0(md_sar_size());
    md_sar_init(s->state);
    s->timer = timer_new_ns(QEMU_CLOCK_VIRTUAL, md_tick, s);
    for (unsigned i = 0; i < 2; i++) {
        s->ports[i] = (MdSarPort) {s, i};
        memory_region_init_io(&s->regions[i], obj, &md_ops, &s->ports[i],
                             i ? "md-sim-sar-analog" : "md-sim-sar-sens", i ? 0x50 : 0x110);
        sysbus_init_mmio(SYS_BUS_DEVICE(obj), &s->regions[i]);
    }
}
static void md_finalize(Object *obj)
{
    MdSimSar *s = MD_SIM_SAR(obj);
    timer_free(s->timer);
    g_free(s->state);
}
static const VMStateDescription md_vmstate = {
    .name = TYPE_MD_SIM_SAR, .unmigratable = 1,
};
static void md_class_init(ObjectClass *klass, void *data)
{
    DeviceClass *dc = DEVICE_CLASS(klass);
    dc->realize = md_realize;
    device_class_set_props(dc, md_properties);
    device_class_set_legacy_reset(dc, md_reset);
    dc->vmsd = &md_vmstate;
    dc->user_creatable = false;
}
static const TypeInfo md_info = {
    .name = TYPE_MD_SIM_SAR, .parent = TYPE_SYS_BUS_DEVICE,
    .instance_size = sizeof(MdSimSar), .instance_init = md_init,
    .instance_finalize = md_finalize, .class_init = md_class_init,
};
static void md_register(void) { type_register_static(&md_info); }
type_init(md_register)

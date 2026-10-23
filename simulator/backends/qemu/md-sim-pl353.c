/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Rust PL353 controller + existing archived QEMU NAND storage engine.
 * This device uses private virtual NAND only. No physical controller access. */
#include "qemu/osdep.h"
#include "qemu/timer.h"
#include "qemu/log.h"
#include "hw/core/sysbus.h"
#include "hw/core/irq.h"
#include "hw/core/qdev-properties.h"
#include "hw/core/qdev-properties-system.h"
#include "migration/vmstate.h"
#include "qapi/error.h"
#include "md-sim-pl353.h"
#include "md-sim-nand.h"
#include "trace.h"

#define TYPE_MD_SIM_PL353 "md-sim-pl353"
OBJECT_DECLARE_SIMPLE_TYPE(MdSimPl353, MD_SIM_PL353)
struct MdSimPl353 {
    SysBusDevice parent_obj;
    MemoryRegion registers, aperture;
    DeviceState *nand;
    BlockBackend *drive;
    char *drive_id;
    MdPl353State *state;
    MdOnfiState *onfi;
    bool m69a;
    MdPl353Bus bus;
    QEMUTimer *timer;
    qemu_irq irq;
    uint8_t manufacturer_id, chip_id;
};
static void md_nand_out(void *opaque, uint32_t kind, uint8_t byte)
{
    MdSimPl353 *s = opaque;
    if (kind < 2) {
        trace_md_sim_pl353_pin(kind, byte);
    }
    if (s->onfi && md_onfi_output(s->onfi, kind, byte)) {
        trace_md_sim_pl353_onfi(kind, byte, md_onfi_faults(s->onfi));
        return;
    }
    /* The reused QEMU NAND API uses its final pin to enable spare bytes. */
    nand_setpins(s->nand, kind == 0, kind == 1, 0, 1, 1);
    nand_setio(s->nand, byte);
}
static uint8_t md_nand_in(void *opaque)
{
    MdSimPl353 *s = opaque;
    nand_setpins(s->nand, 0, 0, 0, 1, 1);
    if (s->onfi) {
        uint8_t value;
        uint8_t status = md_onfi_status_mode(s->onfi) ? nand_getio(s->nand) : 0;
        if (md_onfi_input(s->onfi, status, &value)) {
            trace_md_sim_pl353_onfi(3, value, md_onfi_faults(s->onfi));
            return value;
        }
    }
    return nand_getio(s->nand);
}
static void md_nand_end(void *opaque)
{
    MdSimPl353 *s = opaque;
    nand_setpins(s->nand, 0, 0, 1, 1, 1);
}
static void md_pl_update(MdSimPl353 *s)
{
    uint64_t deadline = md_pl353_deadline(s->state);
    qemu_set_irq(s->irq, md_pl353_irq(s->state));
    if (deadline) {
        timer_mod_ns(s->timer, deadline);
    } else {
        timer_del(s->timer);
    }
}
static void md_pl_tick(void *opaque)
{
    MdSimPl353 *s = opaque;
    uint64_t now = qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL);
    md_nand_tick(s->nand, now);
    md_pl353_tick(s->state, now);
    md_pl_update(s);
}
static MemTxResult md_pl_access(MdSimPl353 *s, bool registers, bool write,
                                hwaddr addr, unsigned size, uint64_t *value)
{
    uint32_t v = *value;
    uint64_t now = qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL);
    md_nand_tick(s->nand, now);
    bool ok = md_pl353_access(s->state, &s->bus, registers, write, addr,
                               size, &v, now);
    /* The chip's program/erase callback can sample the running virtual clock
     * after the controller access began. Its actual deadline is authoritative;
     * do not signal READY before the deferred storage operation completes. */
    md_pl353_chip_busy_until(s->state, md_nand_deadline(s->nand));
    *value = v;
    if (registers || !ok || !(addr & (1 << 19))) {
        trace_md_sim_pl353_access(registers, write, addr, size, v, ok);
    }
    md_pl_update(s);
    return ok ? MEMTX_OK : MEMTX_ERROR;
}
#define PL_ACCESSORS(name, regs) \
static MemTxResult name##_read(void *p, hwaddr a, uint64_t *v, \
                               unsigned n, MemTxAttrs attrs) \
{ *v = 0; return md_pl_access(p, regs, false, a, n, v); } \
static MemTxResult name##_write(void *p, hwaddr a, uint64_t v, \
                                unsigned n, MemTxAttrs attrs) \
{ return md_pl_access(p, regs, true, a, n, &v); }
PL_ACCESSORS(md_pl_regs, true)
PL_ACCESSORS(md_pl_data, false)
static const MemoryRegionOps md_pl_regs_ops = {
    .read_with_attrs = md_pl_regs_read, .write_with_attrs = md_pl_regs_write,
    .endianness = DEVICE_LITTLE_ENDIAN,
    .valid = { .min_access_size = 4, .max_access_size = 4 },
};
static const MemoryRegionOps md_pl_data_ops = {
    .read_with_attrs = md_pl_data_read, .write_with_attrs = md_pl_data_write,
    .endianness = DEVICE_LITTLE_ENDIAN,
    .valid = { .min_access_size = 1, .max_access_size = 4 },
};
static void md_pl_reset(Object *obj, ResetType type)
{
    MdSimPl353 *s = MD_SIM_PL353(obj);
    /* Global QEMU reset also invokes the child NAND chip reset. Its separate
     * identification transfer must not retain stale data/status mode. */
    if (s->onfi) {
        md_onfi_reset(s->onfi);
    }
    md_pl353_reset(s->state, &s->bus);
    md_pl353_chip_busy_until(s->state, md_nand_deadline(s->nand));
    md_pl_update(s);
}
static void md_pl_realize(DeviceState *dev, Error **errp)
{
    MdSimPl353 *s = MD_SIM_PL353(dev);
    if (s->manufacturer_id != (s->m69a ? 0x2c : 0xec) || s->chip_id != 0xda) {
        error_setg(errp, "PL353 fixture supports only explicit EC/DA x8 geometry");
        return;
    }
    if (s->drive_id) {
        s->drive = blk_by_name(s->drive_id);
        if (!s->drive) {
            error_setg(errp, "PL353 fixture drive-id does not name a block backend");
            return;
        }
    }
    if (s->drive && blk_getlength(s->drive) != 131072LL * 2112) {
        error_setg(errp, "PL353 fixture requires 131072 complete 2048+64-byte pages; data-only backing is unsupported");
        return;
    }
    s->nand = nand_init(s->drive, s->manufacturer_id, s->chip_id);
    if (s->m69a) {
        s->onfi = g_malloc0(md_onfi_size());
        md_onfi_init(s->onfi);
        md_pl353_enable_onfi(s->state);
    }
    /* nand_init realizes the device under QEMU's peripheral container. */
    object_unref(s->nand);
}
static void md_pl_init(Object *obj)
{
    MdSimPl353 *s = MD_SIM_PL353(obj);
    s->state = g_malloc0(md_pl353_size());
    md_pl353_init(s->state);
    s->bus = (MdPl353Bus){s, md_nand_out, md_nand_in, md_nand_end};
    s->timer = timer_new_ns(QEMU_CLOCK_VIRTUAL, md_pl_tick, s);
    memory_region_init_io(&s->registers, obj, &md_pl_regs_ops, s,
                          TYPE_MD_SIM_PL353 "-registers", 0x1000);
    memory_region_init_io(&s->aperture, obj, &md_pl_data_ops, s,
                          TYPE_MD_SIM_PL353 "-nand", 0x1000000);
    sysbus_init_mmio(SYS_BUS_DEVICE(s), &s->registers);
    sysbus_init_mmio(SYS_BUS_DEVICE(s), &s->aperture);
    sysbus_init_irq(SYS_BUS_DEVICE(s), &s->irq);
}
static void md_pl_finalize(Object *obj)
{
    MdSimPl353 *s = MD_SIM_PL353(obj);
    timer_free(s->timer);
    g_free(s->state);
    g_free(s->onfi);
}
static const VMStateDescription md_pl_vmstate = {
    .name = TYPE_MD_SIM_PL353, .unmigratable = 1,
};
static const Property md_pl_props[] = {
    /* The child NAND chip owns this backend. A second DRIVE property on the
     * controller would attach the same backend twice. */
    DEFINE_PROP_STRING("drive-id", MdSimPl353, drive_id),
    DEFINE_PROP_BOOL("m69a-development", MdSimPl353, m69a, false),
    DEFINE_PROP_UINT8("manufacturer-id", MdSimPl353, manufacturer_id, 0xec),
    DEFINE_PROP_UINT8("chip-id", MdSimPl353, chip_id, 0xda),
};
static void md_pl_class_init(ObjectClass *oc, const void *data)
{
    DeviceClass *dc = DEVICE_CLASS(oc);
    ResettableClass *rc = RESETTABLE_CLASS(oc);
    dc->realize = md_pl_realize;
    rc->phases.hold = md_pl_reset;
    dc->vmsd = &md_pl_vmstate;
    device_class_set_props(dc, md_pl_props);
}
static const TypeInfo md_pl_type = {
    .name = TYPE_MD_SIM_PL353, .parent = TYPE_SYS_BUS_DEVICE,
    .instance_size = sizeof(MdSimPl353), .instance_init = md_pl_init,
    .instance_finalize = md_pl_finalize, .class_init = md_pl_class_init,
};
static void md_pl_register(void) { type_register_static(&md_pl_type); }
type_init(md_pl_register)

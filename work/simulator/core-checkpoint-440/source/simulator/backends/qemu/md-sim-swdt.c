/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Zynq SWDT reference model. Rust owns registers; QEMU PTIMER owns time. */
#include "qemu/osdep.h"
#include "qapi/error.h"
#include "qemu/log.h"
#include "qemu/module.h"
#include "hw/core/sysbus.h"
#include "hw/core/irq.h"
#include "hw/core/qdev-clock.h"
#include "hw/core/ptimer.h"
#include "migration/vmstate.h"
#include "system/runstate.h"
#include "md-sim-swdt.h"
#include "trace.h"

#define TYPE_MD_SIM_SWDT "md-sim-zynq-swdt"
OBJECT_DECLARE_SIMPLE_TYPE(MdSimSwdt, MD_SIM_SWDT)
struct MdSimSwdt {
    SysBusDevice parent_obj;
    MemoryRegion mmio;
    MdSwdtRegs registers;
    Clock *pclk, *refclk;
    ptimer_state *counter, *pulse;
    qemu_irq irq;
    bool counting, pulsing;
};

static void md_swdt_period(MdSimSwdt *s)
{
    ptimer_transaction_begin(s->counter);
    if (!clock_is_enabled(s->refclk)) {
        ptimer_stop(s->counter);
    } else {
        ptimer_set_period_from_clock(s->counter, s->refclk,
                                    md_swdt_divisor(&s->registers));
        if (s->counting) {
            ptimer_run(s->counter, 1);
        }
    }
    ptimer_transaction_commit(s->counter);
}

static void md_swdt_refclk(void *opaque, ClockEvent event)
{
    md_swdt_period(opaque);
}

static void md_swdt_pclk(void *opaque, ClockEvent event)
{
    MdSimSwdt *s = opaque;
    ptimer_transaction_begin(s->pulse);
    if (!clock_is_enabled(s->pclk)) {
        ptimer_stop(s->pulse);
    } else {
        ptimer_set_period_from_clock(s->pulse, s->pclk, 1);
        if (s->pulsing) {
            ptimer_run(s->pulse, 1);
        }
    }
    ptimer_transaction_commit(s->pulse);
}

static void md_swdt_pulse_end(void *opaque)
{
    MdSimSwdt *s = opaque;
    s->pulsing = false;
    qemu_set_irq(s->irq, 0);
    trace_md_sim_swdt_event(qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL), "irq-low", 0);
}

static void md_swdt_timeout(void *opaque)
{
    MdSimSwdt *s = opaque;
    MdSwdtExpiry event;
    s->counting = false;
    md_swdt_expire(&s->registers, &event);
    trace_md_sim_swdt_event(qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL),
                           "zero", event.irq_cycles | (event.reset << 8));
    if (event.irq_cycles) {
        s->pulsing = true;
        qemu_set_irq(s->irq, 1);
        ptimer_transaction_begin(s->pulse);
        ptimer_stop(s->pulse);
        ptimer_set_count(s->pulse, event.irq_cycles);
        if (clock_is_enabled(s->pclk)) {
            ptimer_set_period_from_clock(s->pulse, s->pclk, 1);
            ptimer_run(s->pulse, 1);
        }
        ptimer_transaction_commit(s->pulse);
    }
    if (event.reset) {
        qemu_system_reset_request(SHUTDOWN_CAUSE_GUEST_RESET);
    }
}

static void md_swdt_reload_counter(MdSimSwdt *s)
{
    s->counting = true;
    ptimer_transaction_begin(s->counter);
    ptimer_stop(s->counter);
    ptimer_set_count(s->counter, md_swdt_reload(&s->registers));
    if (clock_is_enabled(s->refclk)) {
        ptimer_set_period_from_clock(s->counter, s->refclk,
                                    md_swdt_divisor(&s->registers));
        ptimer_run(s->counter, 1);
    }
    ptimer_transaction_commit(s->counter);
}

static MemTxResult md_swdt_bus_read(void *opaque, hwaddr addr, uint64_t *data,
                                  unsigned size, MemTxAttrs attrs)
{
    MdSimSwdt *s = opaque;
    uint32_t value = 0;
    /* An absent external clock can hang the physical APB. This synchronous
     * adapter returns a bus error; it never fabricates a successful access. */
    bool ok = clock_is_enabled(s->refclk) &&
              md_swdt_read(&s->registers, addr, &value);
    *data = value;
    trace_md_sim_swdt_mmio(qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL),
                          false, addr, value, !ok);
    return ok ? MEMTX_OK : MEMTX_ERROR;
}

static MemTxResult md_swdt_bus_write(void *opaque, hwaddr addr, uint64_t value,
                                   unsigned size, MemTxAttrs attrs)
{
    MdSimSwdt *s = opaque;
    int result = clock_is_enabled(s->refclk) ?
                 md_swdt_write(&s->registers, addr, value) : MD_SWDT_INVALID;
    trace_md_sim_swdt_mmio(qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL),
                          true, addr, value, result < 0);
    if (result == MD_SWDT_CLOCK) {
        md_swdt_period(s);
    } else if (result == MD_SWDT_RESTART) {
        md_swdt_reload_counter(s);
    }
    return result < 0 ? MEMTX_ERROR : MEMTX_OK;
}

static const MemoryRegionOps md_swdt_ops = {
    .read_with_attrs = md_swdt_bus_read,
    .write_with_attrs = md_swdt_bus_write,
    .endianness = DEVICE_LITTLE_ENDIAN,
    .valid = { .min_access_size = 4, .max_access_size = 4, .unaligned = false },
    .impl = { .min_access_size = 4, .max_access_size = 4 },
};

static void md_swdt_device_reset(DeviceState *dev)
{
    MdSimSwdt *s = MD_SIM_SWDT(dev);
    md_swdt_reset(&s->registers);
    s->pulsing = false;
    ptimer_transaction_begin(s->pulse);
    ptimer_stop(s->pulse);
    ptimer_set_count(s->pulse, 0);
    ptimer_transaction_commit(s->pulse);
    qemu_set_irq(s->irq, 0);
    md_swdt_reload_counter(s);
}

static void md_swdt_realize(DeviceState *dev, Error **errp)
{
    MdSimSwdt *s = MD_SIM_SWDT(dev);
    if (!clock_has_source(s->refclk) || !clock_has_source(s->pclk)) {
        error_setg(errp, "Zynq SWDT requires both clock connections");
    }
}

static void md_swdt_init(Object *obj)
{
    MdSimSwdt *s = MD_SIM_SWDT(obj);
    md_swdt_reset(&s->registers);
    s->counter = ptimer_init(md_swdt_timeout, s,
        PTIMER_POLICY_TRIGGER_ONLY_ON_DECREMENT | PTIMER_POLICY_NO_COUNTER_ROUND_DOWN);
    s->pulse = ptimer_init(md_swdt_pulse_end, s,
        PTIMER_POLICY_TRIGGER_ONLY_ON_DECREMENT | PTIMER_POLICY_NO_COUNTER_ROUND_DOWN);
    memory_region_init_io(&s->mmio, obj, &md_swdt_ops, s, "zynq-swdt", 0x1000);
    sysbus_init_mmio(SYS_BUS_DEVICE(obj), &s->mmio);
    sysbus_init_irq(SYS_BUS_DEVICE(obj), &s->irq);
    s->refclk = qdev_init_clock_in(DEVICE(obj), "refclk", md_swdt_refclk, s, ClockUpdate);
    s->pclk = qdev_init_clock_in(DEVICE(obj), "pclk", md_swdt_pclk, s, ClockUpdate);
}

static void md_swdt_finalize(Object *obj)
{
    MdSimSwdt *s = MD_SIM_SWDT(obj);
    ptimer_free(s->counter);
    ptimer_free(s->pulse);
}

static const VMStateDescription md_swdt_vmstate = {
    .name = TYPE_MD_SIM_SWDT,
    .unmigratable = true,
};

static void md_swdt_class_init(ObjectClass *klass, const void *data)
{
    DeviceClass *dc = DEVICE_CLASS(klass);
    dc->realize = md_swdt_realize;
    dc->vmsd = &md_swdt_vmstate;
    device_class_set_legacy_reset(dc, md_swdt_device_reset);
}

static const TypeInfo md_swdt_type = {
    .name = TYPE_MD_SIM_SWDT,
    .parent = TYPE_SYS_BUS_DEVICE,
    .instance_size = sizeof(MdSimSwdt),
    .instance_init = md_swdt_init,
    .instance_finalize = md_swdt_finalize,
    .class_init = md_swdt_class_init,
};
static void md_swdt_register_types(void) { type_register_static(&md_swdt_type); }
type_init(md_swdt_register_types)

/* SPDX-License-Identifier: GPL-2.0-or-later */
/* S9IO controller profiles with Rust state: legacy HDL50MHz, or the
 * explicit BM1387 reference-contract development profile200MHz.
 * QEMU MMIO is atomic: no CPU AXI channel stalls are claimed. At the same
 * timestamp, elapsed core edges precede MMIO or serial input changes.
 * This is a development fixture, not a mapping to the selected2022 bitstream.
 */
#include "qemu/osdep.h"
#include "qapi/error.h"
#include "qemu/error-report.h"
#include "qemu/timer.h"
#include "hw/core/sysbus.h"
#include "hw/core/irq.h"
#include "hw/core/qdev-properties.h"
#include "hw/core/qdev-clock.h"
#include "migration/vmstate.h"
#include "system/runstate.h"
#include "md-sim-s9io.h"
#include "trace.h"

#define TYPE_MD_SIM_S9IO "md-sim-s9io"
OBJECT_DECLARE_SIMPLE_TYPE(MdSimS9io, MD_SIM_S9IO)
struct MdSimS9io {
    SysBusDevice parent_obj;
    MemoryRegion mmio;
    qemu_irq irq[3], txd;
    MdS9State *state;
    MdHashBudget *hash_budget;
    QEMUTimer *timer;
    Clock *fabric_clk;
    unsigned __int128 edge_q32;
    int64_t last_ns, core_ns, epoch_ns, clock_change_ns;
    uint32_t build_id, link_id, fixed_filter_zero_bits;
    bool rxd, failed, reference_v1, bm1387_reference, tmp421_reference;
    char *temperature_input_plan;
    char trace_packet[301];
    unsigned trace_packet_bytes;
};

/* Board-only connection. The borrowed budget is cloned during realize.
 * It is not a raw-pointer QOM property and cannot be set by a guest. */
void md_s9io_set_hash_budget(DeviceState *dev, MdHashBudget *budget)
{
    MdSimS9io *s = MD_SIM_S9IO(dev);
    assert(!dev->realized && !s->state && budget);
    s->hash_budget = budget;
}

static void md_asic_trace(MdSimS9io *s)
{
    MdAsicTrace event;
    while (md_s9_take_asic_trace(s->state, &event)) {
        trace_md_sim_bm1387_event(s->link_id, s->epoch_ns + event.ns,
                                  event.kind, event.value, event.extra,
                                  event.grant, event.wall_ns);
        /* Compact diagnostics retain actual received bytes and the existing
         * applied-command boundary. This does not decode or execute packets.
         * Rust's bounded decoder rejects frames at or before150 bytes. */
        if (event.kind == 1) {
            static const char hex[] = "0123456789abcdef";
            if (s->trace_packet_bytes >= 150 || event.value > 255) {
                error_report("S9IO compact trace packet invariant");
                s->failed = true;
                qemu_system_vmstop_request(RUN_STATE_INTERNAL_ERROR);
                continue;
            }
            unsigned pos = s->trace_packet_bytes++ * 2;
            s->trace_packet[pos] = hex[event.value >> 4];
            s->trace_packet[pos + 1] = hex[event.value & 15];
            s->trace_packet[pos + 2] = '\0';
        } else if ((event.kind >= 4 && event.kind <= 7) || event.kind == 12 ||
                   ((event.kind == 8 || event.kind == 9) && !event.value)) {
            if (s->trace_packet_bytes) {
                trace_md_sim_bm1387_packet(s->link_id, s->epoch_ns + event.ns,
                                           event.kind, s->trace_packet);
            }
            s->trace_packet_bytes = 0;
            s->trace_packet[0] = '\0';
        }
        /* Keep replies, applied-command markers, faults, pins and every
         * computed hash. Omit only RX-byte duplication and TX pin edges. */
        if (event.kind != 1 && event.kind != 3) {
            trace_md_sim_bm1387_state(s->link_id, s->epoch_ns + event.ns,
                                      event.kind, event.value, event.extra,
                                      event.grant, event.wall_ns);
        }
    }
}

static void md_fail(MdSimS9io *s)
{
    s->failed = true;
    error_report("S9IO link%u: %s", s->link_id, md_s9_error(s->state));
    qemu_system_vmstop_request(RUN_STATE_INTERNAL_ERROR);
}

static void md_outputs(MdSimS9io *s)
{
    uint32_t status = md_s9_status(s->state);
    for (unsigned i = 0; i < 3; i++) {
        qemu_set_irq(s->irq[i], (status >> i) & 1);
    }
    qemu_set_irq(s->txd, (status >> 3) & 1);
}
static uint64_t md_period(MdSimS9io *s)
{
    return clock_has_source(s->fabric_clk) ? clock_get(s->fabric_clk) :
           (uint64_t)s->core_ns << 32;
}

static int64_t md_ceil_ns(unsigned __int128 q32)
{
    return (q32 + UINT32_MAX) >> 32;
}

static MdS9Clock md_clock_span(MdSimS9io *s)
{
    unsigned __int128 relative = s->edge_q32 -
                                ((unsigned __int128)s->epoch_ns << 32);
    return (MdS9Clock) {
        .edge_ns = relative >> 32,
        .fraction = relative,
        .period_q32 = md_period(s),
    };
}

static void md_advance_to(MdSimS9io *s, int64_t now)
{
    uint64_t calls = 0;
    int64_t sampled_at = 0;
    uint64_t period = md_period(s);
    unsigned __int128 target = (unsigned __int128)now << 32;
    while (!s->failed && period && target >= s->edge_q32 + period) {
        MdS9Event event;
        uint64_t clocks = MIN((target - s->edge_q32) / period,
                              (unsigned __int128)UINT64_MAX);
        MdS9Clock clock = md_clock_span(s);
        if (!md_s9_advance_timed(s->state, &clock, clocks, s->rxd, &event)) {
            md_asic_trace(s);
            md_fail(s);
            break;
        }
        assert(event.clocks > 0 && event.clocks <= clocks);
        s->edge_q32 += (unsigned __int128)event.clocks * period;
        s->last_ns = md_ceil_ns(s->edge_q32);
        if (event.flags) {
            trace_md_sim_s9io_event(s->link_id, s->last_ns, event.flags,
                                   event.tx_byte, event.rx_byte, event.irq,
                                   event.txd);
        }
        md_asic_trace(s);
        md_outputs(s);
        /* Diagnose prolonged catch-up without skipping a clock, changing a
         * guest timeout or inventing a device result. Ordinary callbacks do
         * not reach this sampling interval. */
        if (++calls % 4096 == 0) {
            int64_t real = qemu_clock_get_ns(QEMU_CLOCK_REALTIME);
            if (sampled_at && real - sampled_at >= 1000000000) {
                MdS9Idle idle;
                md_s9_idle_details(s->state, s->rxd, &idle);
                fprintf(stderr, "md-sim-s9io: slow_advance link=%u now=%"
                        PRId64 " last=%" PRId64 " calls=%" PRIu64
                        " idle=%" PRIu64 " status=0x%x rx=%" PRIu64
                        " tx=%" PRIu64 " uart=%" PRIu64 " asic=%" PRIu64 "\n",
                        s->link_id, now, s->last_ns, calls,
                        md_s9_idle_limit(s->state, s->rxd),
                        md_s9_status(s->state), idle.receive, idle.transmit,
                        idle.uart, idle.asic);
                sampled_at = real;
            } else if (!sampled_at) {
                sampled_at = real;
            }
        }
    }
    /* A gated fabric does not gate the ASIC oscillator, UART or hashing.
     * Also cover physical time after the last fractional core edge. Drain
     * after each peer deadline so a late callback cannot fill its trace queue. */
    uint64_t until = now - s->epoch_ns, reached = 0;
    while (!s->failed) {
        if (!md_s9_peer_advance(s->state, until, &reached)) {
            md_asic_trace(s);
            md_fail(s);
            break;
        }
        md_asic_trace(s);
        if (reached == until) {
            break;
        }
    }
}
static void md_advance(MdSimS9io *s)
{
    md_advance_to(s, qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL));
}
static void md_schedule(MdSimS9io *s)
{
    md_outputs(s);
    uint64_t period = md_period(s);
    MdS9Clock clock = md_clock_span(s);
    uint64_t idle = period ? md_s9_idle_limit_timed(s->state, &clock, s->rxd)
                          : UINT64_MAX;
    unsigned __int128 next = (unsigned __int128)UINT64_MAX << 32;
    if (period && idle != UINT64_MAX) {
        next = s->edge_q32 + ((unsigned __int128)idle + 1) * period;
    }
    uint64_t peer = md_s9_peer_deadline(s->state);
    if (peer != UINT64_MAX) {
        unsigned __int128 peer_time =
            ((unsigned __int128)s->epoch_ns + peer) << 32;
        next = MIN(next, peer_time);
    }
    if (s->failed || next > ((unsigned __int128)INT64_MAX << 32)) {
        timer_del(s->timer);
    } else {
        timer_mod_ns(s->timer, md_ceil_ns(next));
    }
}
static void md_clock_change(void *opaque, ClockEvent event)
{
    MdSimS9io *s = opaque;
    if (!s->state) {
        return;
    }
    if (event == ClockPreUpdate) {
        s->clock_change_ns = qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL);
        md_advance_to(s, s->clock_change_ns);
    } else {
        /* Frequency transitions restart the fractional edge phase. This is
         * an explicit digital timing policy, not a calibrated PLL waveform.
         * A stopped clock contributes no edges when it is re-enabled. */
        s->last_ns = s->clock_change_ns;
        s->edge_q32 = (unsigned __int128)s->last_ns << 32;
        md_schedule(s);
    }
}

static void md_tick(void *opaque)
{
    MdSimS9io *s = opaque;
    md_advance(s);
    md_schedule(s);
}
static void md_rxd(void *opaque, int n, int level)
{
    MdSimS9io *s = opaque;
    md_advance(s);
    s->rxd = level != 0;
    md_schedule(s);
}

static void md_asic_pin(void *opaque, int n, int level)
{
    MdSimS9io *s = opaque;
    /* With TCG's virtual real-time clock, two reads in one callback can
     * differ. Advance and apply the pin at the same captured timestamp. */
    int64_t now = qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL);
    md_advance_to(s, now);
    if (!s->failed) {
        if (!md_s9_asic_pin_at(s->state, n, level != 0, now - s->epoch_ns)) {
            md_fail(s);
        }
        md_asic_trace(s);
    }
    md_schedule(s);
}
static MemTxResult md_read(void *opaque, hwaddr addr, uint64_t *data,
                           unsigned size, MemTxAttrs attrs)
{
    MdSimS9io *s = opaque;
    uint32_t value = 0;
    md_advance(s);
    uint32_t error = md_s9_read(s->state, addr, size, &value);
    *data = value;
    trace_md_sim_s9io_mmio(s->link_id, false, addr, size, value, error);
    md_schedule(s);
    return error ? MEMTX_ERROR : MEMTX_OK;
}
static MemTxResult md_write(void *opaque, hwaddr addr, uint64_t value,
                            unsigned size, MemTxAttrs attrs)
{
    MdSimS9io *s = opaque;
    md_advance(s);
    bool ok = md_s9_write(s->state, addr, size, value);
    trace_md_sim_s9io_mmio(s->link_id, true, addr, size, value, !ok);
    md_schedule(s);
    return ok ? MEMTX_OK : MEMTX_ERROR;
}
static const MemoryRegionOps md_ops = {
    .read_with_attrs = md_read,
    .write_with_attrs = md_write,
    .endianness = DEVICE_LITTLE_ENDIAN,
    .valid = { .min_access_size = 1, .max_access_size = 4, .unaligned = false },
    .impl = { .min_access_size = 1, .max_access_size = 4 },
};
static void md_reset(DeviceState *dev)
{
    MdSimS9io *s = MD_SIM_S9IO(dev);
    timer_del(s->timer);
    md_s9_reset(s->state);
    s->failed = false;
    s->trace_packet_bytes = 0;
    s->trace_packet[0] = '\0';
    int64_t now = qemu_clock_get_ns(QEMU_CLOCK_VIRTUAL);
    s->last_ns = now - now % s->core_ns;
    if (clock_has_source(s->fabric_clk)) {
        s->last_ns = now;
    }
    s->edge_q32 = (unsigned __int128)s->last_ns << 32;
    s->epoch_ns = s->last_ns;
    md_schedule(s);
}
static void md_realize(DeviceState *dev, Error **errp)
{
    MdSimS9io *s = MD_SIM_S9IO(dev);
    if (!s->bm1387_reference && s->fixed_filter_zero_bits != 32) {
        error_setg(errp, "Fixed-filter adaptation requires BM1387 mining");
        return;
    }
    if (s->tmp421_reference && !s->bm1387_reference) {
        error_setg(errp, "Temperature fixture requires a BM1387 peer");
        return;
    }
    if (s->temperature_input_plan && !s->tmp421_reference) {
        error_setg(errp, "Temperature input plan requires the declared sensor fixture");
        return;
    }
    if (s->bm1387_reference) {
        if (!s->reference_v1) {
            error_setg(errp, "BM1387 peer requires the reference1.0.2 controller");
            return;
        }
        /* Explicit archived-source fixture. No resolved physical stepping,
         * per-hop delay, ASIC buffer depth or calibration claim. */
        if (!s->hash_budget) {
            error_setg(errp, "BM1387 requires the board's aggregate hash budget");
            return;
        }
        s->state = md_s9_new_bm1387_reference_mining(s->build_id,
                                                    s->hash_budget, 1024,
                                                    s->fixed_filter_zero_bits);
        s->hash_budget = NULL;
    } else {
        s->state = s->reference_v1 ? md_s9_new_reference_v1(s->build_id)
                                 : md_s9_new(s->build_id);
    }
    if (!s->state) {
        error_setg(errp, "Unsupported S9IO/ASIC profile");
        return;
    }
    if (s->tmp421_reference && !md_s9_attach_tmp421_reference(s->state)) {
        error_setg(errp, "Temperature fixture: %s", md_s9_error(s->state));
        return;
    }
    if (s->temperature_input_plan &&
        !md_s9_temperature_input_plan(s->state,
            (const uint8_t *)s->temperature_input_plan,
            strlen(s->temperature_input_plan))) {
        error_setg(errp, "Temperature input plan: %s", md_s9_error(s->state));
        return;
    }
    s->core_ns = md_s9_core_period_ns(s->state);
    s->rxd = true;
}
static void md_init(Object *obj)
{
    MdSimS9io *s = MD_SIM_S9IO(obj);
    s->timer = timer_new_ns(QEMU_CLOCK_VIRTUAL, md_tick, s);
    s->fabric_clk = qdev_init_clock_in(DEVICE(obj), "fabric-clk",
                        md_clock_change, s, ClockPreUpdate | ClockUpdate);
    memory_region_init_io(&s->mmio, obj, &md_ops, s, TYPE_MD_SIM_S9IO, 0x10000);
    sysbus_init_mmio(SYS_BUS_DEVICE(obj), &s->mmio);
    for (unsigned i = 0; i < 3; i++) {
        sysbus_init_irq(SYS_BUS_DEVICE(obj), &s->irq[i]);
    }
    qdev_init_gpio_in_named(DEVICE(obj), md_rxd, "rxd", 1);
    qdev_init_gpio_out_named(DEVICE(obj), &s->txd, "txd", 1);
    qdev_init_gpio_in_named(DEVICE(obj), md_asic_pin, "asic-power-reset-n", 2);
}
static void md_finalize(Object *obj)
{
    MdSimS9io *s = MD_SIM_S9IO(obj);
    timer_free(s->timer);
    md_s9_free(s->state);
}
static const Property md_properties[] = {
    /* Explicit development fixture ID "test", never a detected FPGA ID. */
    DEFINE_PROP_UINT32("build-id", MdSimS9io, build_id, 0x74657374),
    DEFINE_PROP_UINT32("link-id", MdSimS9io, link_id, 0),
    DEFINE_PROP_BOOL("reference-v1-bm1387", MdSimS9io, reference_v1, false),
    DEFINE_PROP_BOOL("bm1387-peer", MdSimS9io, bm1387_reference, false),
    DEFINE_PROP_UINT32("fixed-filter-zero-bits", MdSimS9io, fixed_filter_zero_bits, 32),
    DEFINE_PROP_BOOL("tmp421-reference", MdSimS9io, tmp421_reference, false),
    DEFINE_PROP_STRING("temperature-input-plan", MdSimS9io, temperature_input_plan),
};
static const VMStateDescription md_vmstate = {
    .name = TYPE_MD_SIM_S9IO,
    .unmigratable = 1,
};
static void md_class_init(ObjectClass *klass, const void *data)
{
    DeviceClass *dc = DEVICE_CLASS(klass);
    dc->realize = md_realize;
    device_class_set_legacy_reset(dc, md_reset);
    device_class_set_props(dc, md_properties);
    dc->vmsd = &md_vmstate;
    dc->desc = "Legacy S9IO0.2 development controller; atomic MMIO timing";
}
static const TypeInfo md_type = {
    .name = TYPE_MD_SIM_S9IO,
    .parent = TYPE_SYS_BUS_DEVICE,
    .instance_size = sizeof(MdSimS9io),
    .instance_init = md_init,
    .instance_finalize = md_finalize,
    .class_init = md_class_init,
};
static void md_register_types(void)
{
    type_register_static(&md_type);
}
type_init(md_register_types)

/* SPDX-License-Identifier: GPL-2.0-or-later */
/* Icarus VPI callback adapter. State, process ownership and evidence are Rust.
 * The original vpi_user.h is a required hashed build input. This module must
 * run only as an owned child of the bounded Rust transport, on inherited fd3.
 * Blocking I/O occurs at read-only synchronization, never in an HDL task.
 * The owner enforces a per-command monotonic deadline and kills/reaps the
 * entire child group on timeout. EOF and partial frames fail, without ACK.
 */
#include <vpi_user.h>
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

#define FRAME 128
#define FD 3
#define ARGS 12
#define MAX_EVENTS 4096
struct raw_event { uint64_t sequence, time_ps, kind, value; };
static struct raw_event events[MAX_EVENTS];
static unsigned event_count, flush_scheduled;
static vpiHandle request_handles[16], irq_handle, output_handle;
static vpiHandle completion_handles[6];
static uint64_t sequence, requested_ps, operation, arguments[ARGS];
static unsigned toggle, initialized, pending, applied, completion_scheduled, failed, finishing;

static uint64_t get_le(const unsigned char *p)
{
    uint64_t value = 0;
    for (unsigned i = 0; i < 8; i++) value |= (uint64_t)p[i] << (8 * i);
    return value;
}
static void put_le(unsigned char *p, uint64_t value)
{
    for (unsigned i = 0; i < 8; i++) p[i] = value >> (8 * i);
}
static uint64_t now_ps(void)
{
    s_vpi_time time = { .type = vpiSimTime };
    vpi_get_time(NULL, &time);
    return ((uint64_t)time.high << 32) | time.low;
}
static void fail(const char *reason)
{
    if (!failed) fprintf(stderr, "MDPL_FAILURE sequence=%llu time_ps=%llu reason=%s\n",
                         (unsigned long long)sequence,
                         (unsigned long long)now_ps(), reason);
    failed = 1;
    vpip_set_return_value(1);
    vpi_control(vpiFinish, 1);
}
static int transfer(void *data, size_t length, int writing)
{
    unsigned char *p = data;
    while (length) {
        ssize_t count = writing ? write(FD, p, length) : read(FD, p, length);
        if (count < 0 && errno == EINTR) continue;
        if (count <= 0) {
            fail(writing ? "response-write-failed" : "command-eof-or-read-failure");
            return 0;
        }
        p += count;
        length -= (size_t)count;
    }
    return 1;
}
static uint64_t logic(vpiHandle handle)
{
    s_vpi_value value = { .format = vpiVectorVal };
    vpi_get_value(handle, &value);
    return (uint64_t)(uint32_t)value.value.vector[0].aval |
           ((uint64_t)(uint32_t)value.value.vector[0].bval << 32);
}
static int integer(vpiHandle handle, uint64_t *result)
{
    s_vpi_value value = { .format = vpiVectorVal };
    vpi_get_value(handle, &value);
    unsigned words = (vpi_get(vpiSize, handle) + 31) / 32;
    if (!words || words > 2) { fail("integer-width"); return 0; }
    *result = 0;
    for (unsigned i = 0; i < words; i++) {
        if (value.value.vector[i].bval) { fail("unknown-integer"); return 0; }
        *result |= (uint64_t)(uint32_t)value.value.vector[i].aval << (32 * i);
    }
    return 1;
}
static void drive(vpiHandle handle, uint64_t word)
{
    s_vpi_vecval vector[2] = {
        { .aval = word, .bval = 0 }, { .aval = word >> 32, .bval = 0 }
    };
    s_vpi_value value = { .format = vpiVectorVal, .value.vector = vector };
    vpi_put_value(handle, &value, NULL, vpiNoDelay);
}
static int response_at(uint64_t seq, uint64_t time, uint64_t kind, const uint64_t args[ARGS])
{
    unsigned char bytes[FRAME] = {0};
    memcpy(bytes, "MDPLR01", 8);
    put_le(bytes + 8, seq);
    put_le(bytes + 16, time);
    put_le(bytes + 24, kind);
    for (unsigned i = 0; i < ARGS; i++) put_le(bytes + 32 + i * 8, args[i]);
    return transfer(bytes, sizeof bytes, 1);
}
static int response(uint64_t kind, const uint64_t args[ARGS])
{
    return response_at(sequence, now_ps(), kind, args);
}
static int flush_events(void)
{
    for (unsigned i = 0; i < event_count; i++) {
        uint64_t args[ARGS] = { events[i].value };
        if (!response_at(events[i].sequence, events[i].time_ps, events[i].kind, args)) return 0;
    }
    event_count = 0;
    return 1;
}
static PLI_INT32 flush_readonly(p_cb_data unused)
{
    (void)unused;
    flush_scheduled = 0;
    if (!failed) flush_events();
    return 0;
}
static void schedule(PLI_INT32 reason, uint64_t delay, PLI_INT32 (*function)(p_cb_data))
{
    s_vpi_time time = { .type = vpiSimTime, .high = delay >> 32, .low = delay };
    s_cb_data callback = { .reason = reason, .cb_rtn = function, .time = &time };
    if (!vpi_register_cb(&callback)) fail("callback-registration-failed");
}
static PLI_INT32 apply(p_cb_data unused)
{
    (void)unused;
    if (failed || !pending || now_ps() != requested_ps) {
        fail("command-application-time-or-state");
        return 0;
    }
    applied = 1;
    // All request words and the trigger are written in the future RW callback.
    // The HDL task consumes the entire batch, then lets the simulator settle.
    drive(request_handles[1], sequence);
    drive(request_handles[2], operation);
    drive(request_handles[3], requested_ps);
    for (unsigned i = 0; i < ARGS; i++) drive(request_handles[4 + i], arguments[i]);
    toggle ^= 1;
    drive(request_handles[0], toggle);
    return 0;
}
static int valid_arguments(void)
{
    unsigned used;
    if (operation == 1 || operation == 2) {
        if (arguments[0] > UINT32_MAX || (arguments[0] & 3)) return 0;
        used = operation == 1 ? 1 : 3;
        if (operation == 2 && (arguments[1] > UINT32_MAX || arguments[2] > 15)) return 0;
    } else if (operation == 3 || operation == 5) {
        used = 0;
    } else if (operation == 4) {
        uint64_t mask = arguments[8];
        if (!mask || (mask & ~UINT64_C(127))) return 0;
        for (unsigned i = 0; i < 4; i++) {
            if (!(mask & (1u << i)) && arguments[i]) return 0;
            if (arguments[i] && arguments[i] < (UINT64_C(1) << 32)) return 0;
        }
        if (arguments[4] > 15 || (!(mask & 16) && arguments[4])) return 0;
        if (arguments[5] > 1 || (!(mask & 32) && arguments[5])) return 0;
        if ((arguments[6] & UINT64_C(0xffff0000ffff0000)) || arguments[7] > 65535) return 0;
        if (!(mask & 64) && (arguments[6] || arguments[7])) return 0;
        used = 9;
    } else return 0;
    for (unsigned i = used; i < ARGS; i++) if (arguments[i]) return 0;
    return 1;
}
static void receive_future(void)
{
    unsigned char bytes[FRAME];
    if (failed || finishing || !transfer(bytes, sizeof bytes, 0)) return;
    uint64_t next_sequence = get_le(bytes + 8);
    requested_ps = get_le(bytes + 16);
    operation = get_le(bytes + 24);
    for (unsigned i = 0; i < ARGS; i++) arguments[i] = get_le(bytes + 32 + i * 8);
    if (memcmp(bytes, "MDPLQ01", 8) || sequence == UINT64_MAX ||
        next_sequence != sequence + 1 || requested_ps <= now_ps() ||
        !valid_arguments()) {
        fail("invalid-version-sequence-time-or-command");
        return;
    }
    sequence = next_sequence;
    pending = 1; applied = 0; completion_scheduled = 0;
    /* Future RW synchronization runs after existing active/inactive/NBA
     * work at this timestamp. Old clock edges therefore precede the batch,
     * independent of earlier command chunking. Newly driven work settles
     * before the RO completion callback. Never schedule this at delay0. */
    schedule(cbReadWriteSynch, requested_ps - now_ps(), apply);
}
static PLI_INT32 observed(p_cb_data callback)
{
    if (failed || finishing) return 0;
    if (event_count == MAX_EVENTS) { fail("raw-event-buffer-bound"); return 0; }
    events[event_count++] = (struct raw_event) {
        .sequence = sequence, .time_ps = now_ps(),
        .kind = callback->obj == irq_handle ? 2 : 3,
        .value = logic(callback->obj)
    };
    if (!flush_scheduled) {
        flush_scheduled = 1;
        schedule(cbReadOnlySynch, 0, flush_readonly);
    }
    return 0;
}
static PLI_INT32 ready(p_cb_data unused)
{
    (void)unused;
    uint64_t args[ARGS] = {0};
    if (now_ps() || failed) { fail("nonzero-initial-time"); return 0; }
    args[3] = logic(irq_handle);
    args[4] = logic(output_handle);
    if (flush_events() && response(5, args)) receive_future();
    return 0;
}
static PLI_INT32 completed(p_cb_data unused)
{
    (void)unused;
    uint64_t args[ARGS] = {0};
    if (failed || !pending || !applied || !completion_scheduled || now_ps() < requested_ps) {
        fail("completion-state-or-time"); return 0;
    }
    args[0] = logic(completion_handles[0]);
    args[1] = logic(completion_handles[1]);
    if (!integer(completion_handles[2], &args[2]) ||
        !integer(completion_handles[3], &args[8]) ||
        !integer(completion_handles[4], &args[5]) ||
        !integer(completion_handles[5], &args[6])) return 0;
    args[3] = logic(irq_handle);
    args[4] = logic(output_handle);
    args[7] = operation;
    pending = 0;
    if (flush_events() && response(1, args)) {
        if (operation == 5) { finishing = 1; close(FD); vpi_control(vpiFinish, 0); }
        else receive_future();
    }
    return 0;
}
static int handles(vpiHandle *out, const unsigned *widths, unsigned count)
{
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle iter = vpi_iterate(vpiArgument, call);
    if (!iter) { fail("missing-task-arguments"); return 0; }
    for (unsigned i = 0; i < count; i++) {
        out[i] = vpi_scan(iter);
        if (!out[i] || (unsigned)vpi_get(vpiSize, out[i]) != widths[i]) {
            fail("task-argument-width-or-count"); return 0;
        }
    }
    if (vpi_scan(iter)) { fail("extra-task-argument"); return 0; }
    return 1;
}
static PLI_INT32 initialize(PLI_BYTE8 *unused)
{
    (void)unused;
    vpiHandle args[18];
    const unsigned widths[18] = {1,64,64,64,64,64,64,64,64,64,64,64,64,64,64,64,20,19};
    if (initialized || vpi_get(vpiTimePrecision, NULL) != -12 ||
        !handles(args, widths, 18)) { fail("initialization-or-time-precision"); return 0; }
    for (unsigned i = 0; i < 16; i++) {
        if (vpi_get(vpiType, args[i]) != vpiReg) { fail("request-argument-not-register"); return 0; }
    }
    memcpy(request_handles, args, sizeof request_handles);
    irq_handle = args[16]; output_handle = args[17]; initialized = 1;
    for (unsigned i = 16; i < 18; i++) {
        s_vpi_time time = { .type = vpiSimTime };
        s_cb_data cb = { .reason = cbValueChange, .cb_rtn = observed, .obj = args[i], .time = &time };
        if (!vpi_register_cb(&cb)) { fail("output-callback-registration"); return 0; }
    }
    schedule(cbReadOnlySynch, 0, ready);
    return 0;
}
static PLI_INT32 complete(PLI_BYTE8 *unused)
{
    (void)unused;
    const unsigned widths[6] = {32,2,12,64,64,1};
    if (!initialized || !pending || !applied || completion_scheduled || now_ps() < requested_ps || !handles(completion_handles, widths, 6)) {
        fail("unexpected-completion-task"); return 0;
    }
    completion_scheduled = 1;
    schedule(cbReadOnlySynch, 0, completed);
    return 0;
}
static void register_tasks(void)
{
    s_vpi_systf_data init = { .type = vpiSysTask, .tfname = "$mdsim_init", .calltf = initialize };
    s_vpi_systf_data done = { .type = vpiSysTask, .tfname = "$mdsim_complete", .calltf = complete };
    vpi_register_systf(&init);
    vpi_register_systf(&done);
}
void (*vlog_startup_routines[])(void) = { register_tasks, NULL };

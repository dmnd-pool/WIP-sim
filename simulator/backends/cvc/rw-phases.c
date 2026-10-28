/* Test the existing VPI C ABI. This code is not a CPU or HDL scheduler. */
#include <vpi_user.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static vpiHandle incoming, edge_seen, sampled, echo_second;
static vpiHandle cancelled_future, cancelled_batch;
static const char *test_case;
static unsigned first_count, peer_count, nested_count, final_count, cancelled_count;
static void fail(const char *why)
{
    fprintf(stderr, "MD_RW_PHASE_FAILED %s\n", why);
    fflush(NULL);
    exit(1);
}
static uint64_t now(void)
{
    s_vpi_time time = { .type = vpiSimTime };
    vpi_get_time(NULL, &time);
    return ((uint64_t)time.high << 32) | time.low;
}
static int bit(vpiHandle handle)
{
    s_vpi_value value = { .format = vpiVectorVal };
    vpi_get_value(handle, &value);
    if (!value.value.vector || value.value.vector[0].bval != 0 ||
        (value.value.vector[0].aval & ~1u) != 0) return -1;
    return value.value.vector[0].aval;
}
static void put(int input)
{
    s_vpi_vecval bits = { .aval = input, .bval = 0 };
    s_vpi_value value = { .format = vpiVectorVal, .value.vector = &bits };
    if (now() != 5) fail("drive time");
    vpi_put_value(incoming, &value, NULL, vpiNoDelay);
}
static vpiHandle schedule(int reason, PLI_INT32 (*fn)(p_cb_data), unsigned delay)
{
    s_vpi_time time = { .type = vpiSimTime, .low = delay };
    s_cb_data cb = { .reason = reason, .cb_rtn = fn, .time = &time };
    vpiHandle handle = vpi_register_cb(&cb);
    if (!handle) fail("registration");
    return handle;
}
static PLI_INT32 victim(p_cb_data cb)
{
    (void)cb;
    cancelled_count++;
    fail("cancelled callback ran");
    return 0;
}
static PLI_INT32 observe(p_cb_data cb)
{
    (void)cb;
    int nested = !strcmp(test_case, "nested");
    int value = nested ? 0 : 1;
    if (now() != 5 || bit(incoming) != value || bit(edge_seen) != 1 ||
        bit(sampled) != 0 || bit(echo_second) != value || first_count != 1 ||
        peer_count != (unsigned)nested || nested_count != (unsigned)nested ||
        final_count != (unsigned)nested || cancelled_count != 0)
        fail("read-only settled state/counts");
    printf("MD_RW_PHASE case=%s time=5 input=%d edge=1 sampled=0 echo=%d first=1 peer=%u nested=%u final=%u cancelled=0\n",
           test_case, value, value, peer_count, nested_count, final_count);
    puts("MD_RW_PHASE_PASS");
    fflush(NULL);
    exit(0);
    return 0;
}
static PLI_INT32 final_rw(p_cb_data cb)
{
    (void)cb;
    if (now() != 5 || bit(echo_second) != 0 || nested_count != 1) fail("final new-NBA settling");
    final_count++;
    return 0;
}
static PLI_INT32 nested_rw(p_cb_data cb)
{
    (void)cb;
    if (now() != 5 || bit(echo_second) != 1 || peer_count != 1) fail("nested new-NBA settling");
    nested_count++;
    put(0);
    schedule(cbReadWriteSynch, final_rw, 0);
    return 0;
}
static PLI_INT32 peer(p_cb_data cb)
{
    (void)cb;
    // A detached RW batch remains in registration order. New NBA waves do not
    // run between its already queued first and peer callbacks.
    if (now() != 5 || first_count != 1 || bit(echo_second) != 0) fail("RW batch order");
    peer_count++;
    return 0;
}
static PLI_INT32 first(p_cb_data cb)
{
    (void)cb;
    if (now() != 5 || bit(sampled) != 0 || bit(edge_seen) != 1 || first_count != 0)
        fail("old-NBA settling");
    first_count++;
    put(1);
    if (!strcmp(test_case, "nested")) schedule(cbReadWriteSynch, nested_rw, 0);
    if (!strcmp(test_case, "cancel")) {
        // The future carrier has already moved into this RW batch. Removal
        // must cancel its current event, not the old carrier object.
        if (!vpi_remove_cb(cancelled_batch)) fail("remove queued future");
        vpiHandle handle = schedule(cbReadWriteSynch, victim, 0);
        if (!vpi_remove_cb(handle)) fail("remove new RW");
    }
    // Register RO now, before the nested RW/NBA work. It must observe the final
    // settled result after all newly registered RW phases have completed.
    schedule(cbReadOnlySynch, observe, 0);
    return 0;
}
static PLI_INT32 enqueue_zero(p_cb_data cb)
{
    (void)cb;
    if (now() != 5) fail("zero registration time");
    schedule(cbReadWriteSynch, first, 0);
    return 0;
}
static PLI_INT32 cancel_early(p_cb_data cb)
{
    (void)cb;
    if (now() != 1 || !vpi_remove_cb(cancelled_future)) fail("remove future carrier");
    return 0;
}
static PLI_INT32 start(p_cb_data cb)
{
    (void)cb;
    s_vpi_vlog_info info;
    if (!vpi_get_vlog_info(&info)) fail("arguments");
    for (int i = 0; i < info.argc; i++) {
        if (strncmp(info.argv[i], "+md_rw_case=", 12)) continue;
        if (test_case) fail("duplicate case");
        test_case = info.argv[i] + 12;
    }
    if (!test_case || (strcmp(test_case, "future") && strcmp(test_case, "zero") &&
        strcmp(test_case, "nested") && strcmp(test_case, "cancel"))) fail("case");
    incoming = vpi_handle_by_name("rw_phases.incoming", NULL);
    edge_seen = vpi_handle_by_name("rw_phases.edge_seen", NULL);
    sampled = vpi_handle_by_name("rw_phases.sampled", NULL);
    echo_second = vpi_handle_by_name("rw_phases.echo_second", NULL);
    if (!incoming || !edge_seen || !sampled || !echo_second) fail("handles");
    if (!strcmp(test_case, "zero")) schedule(cbAfterDelay, enqueue_zero, 5);
    else schedule(cbReadWriteSynch, first, 5);
    if (!strcmp(test_case, "nested")) schedule(cbReadWriteSynch, peer, 5);
    if (!strcmp(test_case, "cancel")) {
        cancelled_batch = schedule(cbReadWriteSynch, victim, 5);
        cancelled_future = schedule(cbReadWriteSynch, victim, 5);
        schedule(cbAfterDelay, cancel_early, 1);
    }
    return 0;
}
void mdsim_rw_phases_bootstrap(void)
{
    s_cb_data cb = { .reason = cbStartOfSimulation, .cb_rtn = start };
    if (!vpi_register_cb(&cb)) fail("start registration");
}
void (*vlog_startup_routines[])(void) = { mdsim_rw_phases_bootstrap, NULL };

/* New code for the existing VPI C ABI. No CPU or HDL kernel is implemented here. */
#include <vpi_user.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static vpiHandle incoming, edge_seen, sampled;
static int32_t get_bit(vpiHandle handle)
{
    s_vpi_value value = { .format = vpiVectorVal };
    vpi_get_value(handle, &value);
    if (!value.value.vector || value.value.vector[0].bval != 0 ||
        (value.value.vector[0].aval & ~1u) != 0) return -1;
    return value.value.vector[0].aval;
}
static uint64_t now(void)
{
    s_vpi_time time = { .type = vpiSimTime };
    vpi_get_time(NULL, &time);
    return ((uint64_t)time.high << 32) | time.low;
}
static void stop(int code)
{
    /* This explicit process outcome does not use vpiFinish's diagnostic level
     * as an exit status. Both stdout and stderr are retained by the owner. */
    fflush(NULL);
    exit(code);
}
static PLI_INT32 observe(p_cb_data cb)
{
    (void)cb;
    uint64_t time = now();
    int32_t input = get_bit(incoming), edge = get_bit(edge_seen), value = get_bit(sampled);
    printf("MD_RW_ORDER time=%llu input=%d edge=%d sampled=%d\n",
           (unsigned long long)time, input, edge, value);
    if (time != 5 || input != 1 || edge != 1 || value != 0) {
        fputs("MD_RW_ORDER_CONTRACT_FAILED\n", stderr);
        stop(1);
    }
    puts("MD_RW_ORDER_CONTRACT_PASS");
    stop(0);
    return 0;
}
static PLI_INT32 drive(p_cb_data cb)
{
    (void)cb;
    if (now() != 5) { fputs("MD_RW_ORDER_DRIVE_TIME_FAILED\n", stderr); stop(1); }
    s_vpi_vecval bit = { .aval = 1, .bval = 0 };
    s_vpi_value value = { .format = vpiVectorVal, .value.vector = &bit };
    vpi_put_value(incoming, &value, NULL, vpiNoDelay);
    s_vpi_time zero = { .type = vpiSimTime };
    s_cb_data event = { .reason = cbReadOnlySynch, .cb_rtn = observe, .time = &zero };
    if (!vpi_register_cb(&event)) { fputs("MD_RW_ORDER_RO_REGISTER_FAILED\n", stderr); stop(1); }
    return 0;
}
static PLI_INT32 start(p_cb_data cb)
{
    (void)cb;
    incoming = vpi_handle_by_name("rw_order.incoming", NULL);
    edge_seen = vpi_handle_by_name("rw_order.edge_seen", NULL);
    sampled = vpi_handle_by_name("rw_order.sampled", NULL);
    if (!incoming || !edge_seen || !sampled) { fputs("MD_RW_ORDER_HANDLES_FAILED\n", stderr); stop(1); }
    s_vpi_time future = { .type = vpiSimTime, .low = 5 };
    s_cb_data event = { .reason = cbReadWriteSynch, .cb_rtn = drive, .time = &future };
    if (!vpi_register_cb(&event)) { fputs("MD_RW_ORDER_RW_REGISTER_FAILED\n", stderr); stop(1); }
    return 0;
}
void mdsim_rw_bootstrap(void)
{
    s_cb_data event = { .reason = cbStartOfSimulation, .cb_rtn = start };
    if (!vpi_register_cb(&event)) { fputs("MD_RW_ORDER_START_REGISTER_FAILED\n", stderr); stop(1); }
}
void (*vlog_startup_routines[])(void) = { mdsim_rw_bootstrap, NULL };

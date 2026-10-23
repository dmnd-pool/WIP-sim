/* C ABI + powered FPGA/ASIC serial path. No firmware/mining acceptance. */
#include "../md-sim-s9io.h"
#include <assert.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>

_Static_assert(sizeof(MdAsicTrace) == 40, "ASIC event ABI size");
_Static_assert(offsetof(MdAsicTrace, kind) == 8, "ASIC kind ABI");
_Static_assert(offsetof(MdAsicTrace, extra) == 16, "ASIC extra ABI");
_Static_assert(offsetof(MdAsicTrace, grant) == 24, "ASIC grant ABI");
_Static_assert(offsetof(MdAsicTrace, wall_ns) == 32, "ASIC wall clock ABI");

static uint32_t read_word(MdS9State *s, uint32_t address)
{
    uint32_t word;
    assert(md_s9_read(s, address, 4, &word) == 0);
    return word;
}

int main(void)
{
    assert(!md_s9_new_bm1387(0, 65, 114, 25000000, 26, 0x90, 1024));
    assert(!md_hash_budget_new(0));
    assert(!md_hash_budget_new(-1));
    assert(!md_s9_new_bm1387_reference_mining(0, NULL, 1024, 32));
    MdHashBudget *budget = md_hash_budget_new(10000);
    assert(budget);
    assert(!md_s9_new_bm1387_reference_mining(0, budget, 1024, 0));
    assert(!md_s9_new_bm1387_reference_mining(0, budget, 1024, 33));
    MdS9State *s = md_s9_new_bm1387_reference_mining(0x74657374, budget, 1024, 32);
    MdS9State *second = md_s9_new_bm1387_reference_mining(0, budget, 1024, 12);
    MdS9State *third = md_s9_new_bm1387_reference_mining(0, budget, 1024, 1);
    assert(second && third);
    const char plan[] = "[{\"at_ns\":300000000,\"inputs\":{"
        "\"local_millicelsius\":25000,\"remote_millicelsius\":105000,"
        "\"diode\":\"Connected\",\"supply_valid\":true}}]";
    assert(md_s9_attach_tmp421_reference(third));
    assert(md_s9_temperature_input_plan(third, (const uint8_t *)plan,
                                      sizeof(plan) - 1));
    assert(!md_s9_temperature_input_plan(third, (const uint8_t *)plan,
                                       sizeof(plan) - 1));
    md_s9_reset(third);
    assert(!md_s9_temperature_input_plan(third, NULL, 0));
    md_s9_reset(third);
    assert(!md_s9_temperature_input_plan(third, (const uint8_t *)plan, 4097));
    md_s9_reset(third);
    assert(md_s9_asic_pin(third, 0, true, 0));
    assert(md_s9_attach_tmp421_reference(second));
    assert(!md_s9_attach_tmp421_reference(second));
    md_s9_reset(second);
    assert(md_s9_asic_pin(second, 0, true, 0));
    md_hash_budget_free(budget);
    md_s9_free(second);
    md_s9_free(third);
    assert(s && md_s9_core_period_ns(s) == 5);
    assert(read_word(s, 0) == 0x00901002);
    assert(md_s9_asic_pin(s, 0, true, 3));
    assert(md_s9_asic_pin(s, 1, true, 3));
    MdAsicTrace trace;
    for (unsigned kind = 8; kind <= 9; kind++) {
        assert(md_s9_take_asic_trace(s, &trace));
        assert(trace.ns == 3 && trace.kind == kind && trace.value == 1);
        assert(trace.grant == 0 && trace.wall_ns == 0);
    }
    assert(!md_s9_take_asic_trace(s, &trace));
    assert(md_s9_write(s, 0x10, 4, 0x6b));
    assert(md_s9_write(s, 8, 4, 8));
    assert(md_s9_write(s, 0x1008, 4, 4));
    assert(md_s9_write(s, 0x1004, 4, 0x00000554));
    /* The archived controller clears its FIFOs on the first enabled edge.
     * A command queued before that edge is intentionally lost. Check that
     * behavior, then send the real query after initialization has settled. */
    uint64_t settling = 2000;
    while (settling) {
        MdS9Event event;
        assert(md_s9_advance(s, settling, false, &event));
        assert(event.clocks > 0 && event.clocks <= settling);
        assert((event.flags & 1) == 0);
        settling -= event.clocks;
        assert(!md_s9_take_asic_trace(s, &trace));
    }
    assert(read_word(s, 0x100c) == 5);
    assert(md_s9_write(s, 0x1004, 4, 0x00000554));
    unsigned received = 0, transmitted = 0, commands = 0;
    static const unsigned command[] = { 0x54, 5, 0, 0, 0x19 };
    static const unsigned reply[] = { 0x13, 0x87, 0x90, 0, 0, 0, 7 };
    uint64_t remaining = 9000000;
    while (remaining) {
        MdS9Event event;
        assert(md_s9_advance(s, remaining, false, &event));
        assert(event.clocks > 0 && event.clocks <= remaining);
        remaining -= event.clocks;
        while (md_s9_take_asic_trace(s, &trace)) {
            switch (trace.kind) {
            case 1:
                assert(received < 5 && trace.value == command[received++]);
                break;
            case 2:
                assert(trace.value == reply[transmitted % 7]);
                transmitted++;
                break;
            case 3:
                assert(trace.value <= 1);
                break;
            case 4:
                assert(trace.value == 0x54 && trace.extra == 63);
                commands++;
                break;
            default:
                assert(0 && "unexpected error or control event");
            }
        }
    }
    assert(received == 5 && transmitted == 441 && commands == 1);
    assert(read_word(s, 0x18) == 0 && (md_s9_status(s) & 0x70) == 0);
    for (unsigned chip = 0; chip < 63; chip++) {
        assert((read_word(s, 0x100c) & 1) == 0);
        assert(read_word(s, 0x1000) == 0x00908713);
        assert(read_word(s, 0x1000) == 0x07000000);
    }
    assert((read_word(s, 0x100c) & 1) == 1);
    assert(!md_s9_asic_pin(s, 0, true, 5));
    assert(strstr(md_s9_error(s), "phase"));
    md_s9_reset(s);
    assert((md_s9_status(s) & 64) == 0);
    md_s9_free(s);
    puts("C ABI: 63 powered ASIC replies crossed timed serial and FPGA FIFO");
    return 0;
}

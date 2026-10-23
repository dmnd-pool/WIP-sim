/* C ABI and independent serial waveform regression; no guest boot claim. */
#include "../md-sim-s9io.h"
#include <assert.h>
#include <stddef.h>
#include <stdio.h>

_Static_assert(sizeof(MdS9Event) == 16, "event ABI size");
_Static_assert(offsetof(MdS9Event, flags) == 8, "event flags ABI");
_Static_assert(offsetof(MdS9Event, txd) == 15, "event TXD ABI");
_Static_assert(sizeof(MdS9Idle) == 32, "idle diagnostic ABI size");
_Static_assert(offsetof(MdS9Idle, asic) == 24, "idle ASIC ABI");

static uint32_t read_word(MdS9State *s, uint32_t offset)
{
    uint32_t value = 0xdeadbeef;
    assert(md_s9_read(s, offset, 4, &value) == 0);
    return value;
}
static void advance(MdS9State *s, uint64_t clocks, bool rxd)
{
    while (clocks) {
        MdS9Event event;
        assert(md_s9_advance(s, clocks, rxd, &event));
        assert(event.clocks > 0 && event.clocks <= clocks);
        clocks -= event.clocks;
    }
}
static void exercise(bool reference)
{
    MdS9State *s = reference ? md_s9_new_reference_v1(0x12345678)
                            : md_s9_new(0x12345678);
    assert(s);
    assert(read_word(s, 0) == (reference ? 0x00901002 : 0x00900200));
    assert(md_s9_core_period_ns(s) == (reference ? 5 : 20));
    assert(read_word(s, 4) == 0x12345678);
    assert(md_s9_status(s) == (reference ? 0 : 8));
    uint32_t value = 0xdeadbeef;
    assert(md_s9_read(s, 0x1000, 4, &value) == 2);
    assert(value == 0xdeadbeef);
    for (uint32_t size = 0; size <= 8; size++) {
        if (size == 1 || size == 2 || size == 4) {
            continue;
        }
        assert(md_s9_read(s, 0, size, &value) == 1);
        assert(!md_s9_write(s, 0, size, 1));
    }
    assert(md_s9_read(s, 0xffff, 2, &value) == 1);
    assert(md_s9_read(s, 0x10000, 4, &value) == 1);
    assert(md_s9_write(s, 0x10, 4, 0x12345678));
    assert(md_s9_write(s, 0x12, 1, 0xaa));
    assert(read_word(s, 0xc010) == 0x12aa5678);
    assert(md_s9_read(s, 0x12, 2, &value) == 0 && value == 0x12aa);
    md_s9_reset(s);
    assert(md_s9_idle_limit(s, true) == UINT64_MAX);
    MdS9Idle idle;
    md_s9_idle_details(s, true, &idle);
    assert(idle.receive == UINT64_MAX && idle.transmit == UINT64_MAX);
    assert(idle.uart == UINT64_MAX && idle.asic == UINT64_MAX);
    assert(md_s9_idle_limit(s, true) == UINT64_MAX);
    advance(s, UINT64_MAX, true);
    assert(read_word(s, 0x14) == 1);
    assert(md_s9_write(s, 8, 4, 8));
    MdS9Event empty;
    assert(md_s9_advance(s, 50000000, true, &empty));
    assert(empty.clocks == 50000000 && empty.flags == 0);
    assert(md_s9_idle_limit(s, true) == UINT64_MAX);
    assert(read_word(s, 0x14) == 1 && read_word(s, 8) == 8);
    assert(md_s9_write(s, 0x14, 4, 257));
    assert(md_s9_write(s, 8, 4, 8));
    assert(md_s9_write(s, 0x1008, 4, 4));
    advance(s, 100, true);
    /* Literal tc_send_cmd5/fifo_data1 and uart_data1 from the archived
     * FPGA testbench. The device computes the final checksum byte. */
    assert(md_s9_write(s, 0x1004, 4, 1364));
    static const uint8_t expected_tx[] = { 84, 5, 0, 0, 25 };
    unsigned tx_count = 0;
    uint64_t remaining = 2000;
    while (remaining) {
        MdS9Event event;
        assert(md_s9_advance(s, remaining, true, &event));
        assert(event.clocks > 0 && event.clocks <= remaining);
        remaining -= event.clocks;
        if (event.flags & 1) {
            assert(tx_count < sizeof(expected_tx));
            assert(event.tx_byte == expected_tx[tx_count++]);
        }
    }
    assert(tx_count == sizeof(expected_tx));
    static const uint8_t reply[] = { 0x13, 0x87, 0x90, 0, 0, 0, 7 };
    for (unsigned i = 0; i < sizeof(reply); i++) {
        for (unsigned bit = 0; bit < 10; bit++) {
            bool level = bit == 9 || (bit != 0 && ((reply[i] >> (bit - 1)) & 1));
            advance(s, 16, level);
        }
    }
    advance(s, 100, true);
    assert((md_s9_status(s) & 7) == 4);
    assert(read_word(s, 0x1000) == 0x00908713);
    assert((md_s9_status(s) & 7) == 0);
    assert(read_word(s, 0x1000) == 0x07000000);
    md_s9_reset(s);
    assert(read_word(s, 0x1000) == 0x07000000);
    assert(md_s9_write(s, 8, 4, 8));
    advance(s, 10, true);
    assert(md_s9_write(s, 0x1004, 4, 0x00000654));
    MdS9Event event;
    assert(!md_s9_advance(s, 100, true, &event));
    assert(md_s9_status(s) & 64);
    assert(!md_s9_advance(s, 1, true, &event));
    md_s9_reset(s);
    assert(!(md_s9_status(s) & 64));
    if (reference) {
        /* Unsupported mode must not emit the legacy BM1387 byte stream. */
        assert(md_s9_write(s, 8, 4, 0x18));
        assert(read_word(s, 8) == 0x18);
        assert(md_s9_idle_limit(s, true) == 0);
        assert(!md_s9_advance(s, 1, true, &event));
        assert(md_s9_status(s) & 64);
        md_s9_reset(s);
        assert(md_s9_status(s) == 0);
        assert(read_word(s, 0) == 0x00901002);
    }
    md_s9_free(s);
}
int main(void)
{
    exercise(false);
    exercise(true);
    md_s9_free(NULL);
    puts("S9IO C ABI, serial RX, byte lanes, reset and unsupported-state checks passed");
    return 0;
}

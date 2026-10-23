#include <assert.h>
#include "../md-sim-axi-gpio.h"

int main(void)
{
    assert(!md_gpio_new(0, 0, 0, 0, 0));
    assert(!md_gpio_new(256, 0, 0, 0, 0));
    assert(!md_gpio_new(32, 2, 0, 0, 0));
    MdGpioState *in = md_gpio_new(9, 0, 0, UINT32_MAX, 0xe0);
    MdGpioState *out = md_gpio_new(13, 1, 0, UINT32_MAX, UINT32_MAX);
    assert(in && out);
    uint32_t value = 0;
    assert(md_gpio_read(in, 0, &value) && value == 0xe0);
    assert(md_gpio_read(in, 4, &value) && value == 0x1ff);
    assert(md_gpio_write(in, 0, UINT32_MAX));
    assert(md_gpio_read(in, 0, &value) && value == 0xe0);
    assert(md_gpio_outputs(in) == 0);
    md_gpio_inputs(in, 0x1aa);
    md_gpio_reset(in);
    assert(md_gpio_read(in, 0, &value) && value == 0x1aa);
    assert(md_gpio_write(out, 0, 0xaa55));
    assert(md_gpio_outputs(out) == 0xa55);
    assert(md_gpio_read(out, 0, &value) && value == 0);
    assert(md_gpio_write(out, 4, 0x1234));
    assert(md_gpio_read(out, 4, &value) && value == 0x1234);
    assert(md_gpio_write(out, 0x120, UINT32_MAX));
    assert(md_gpio_read(out, 0x120, &value) && value == 0);
    assert(md_gpio_outputs(out) == 0xa55);
    for (uint32_t offset = 1; offset <= 3; offset++) {
        value = 0x76543210;
        assert(!md_gpio_read(out, offset, &value) && value == 0x76543210);
        assert(!md_gpio_write(out, offset, 0));
    }
    value = 0x76543210;
    assert(!md_gpio_read(out, UINT32_MAX, &value) && value == 0x76543210);
    assert(!md_gpio_write(out, UINT32_MAX, 0));
    md_gpio_reset(out);
    assert(md_gpio_outputs(out) == 0);
    assert(md_gpio_read(out, 4, &value) && value == 0x1fff);
    md_gpio_free(in);
    md_gpio_free(out);
    md_gpio_free(0);
    return 0;
}

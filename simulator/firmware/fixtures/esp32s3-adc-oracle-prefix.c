/* ABI adapter for exact upstream C arithmetic. This is not firmware.
 * Only source-file headers and OS/SDK callbacks are replaced. The derived
 * source manifest records every retained upstream range and parent hash. */
typedef unsigned char uint8_t;
typedef unsigned short uint16_t;
typedef unsigned int uint32_t;
typedef unsigned long long uint64_t;
typedef int int32_t;
typedef unsigned long size_t;
_Static_assert(sizeof(uint32_t)==4 && sizeof(uint64_t)==8, "fixed width ABI");
#define NULL ((void*)0)
#define assert(v) do { if (!(v)) __builtin_trap(); } while (0)
#define ESP_ERROR_CHECK(v) assert((v)==0)
#define ESP_LOGW(...) ((void)0)
#define ESP_LOGV(...) ((void)0)
#define ESP_EFUSE_ADC_CALIB_VER 1
#define ESP_EFUSE_ADC_CALIB_VER_MIN 1
#define ESP_EFUSE_ADC_CALIB_VER_MAX 1
#define ESP_OK 0
#define ESP_ERR_INVALID_ARG 1
typedef int esp_err_t;
enum {ADC_UNIT_1, ADC_UNIT_2};
enum {EFUSE_BLK0, EFUSE_BLK1, EFUSE_BLK2, EFUSE_BLK3, EFUSE_BLK4,
      EFUSE_BLK5, EFUSE_BLK6, EFUSE_BLK7, EFUSE_BLK8, EFUSE_BLK9, EFUSE_BLK10};
typedef struct {int efuse_block, bit_start, bit_count;} esp_efuse_desc_t;
typedef struct {int unit_id, atten;} adc_cali_curve_fitting_config_t;
typedef struct {
    uint8_t term_num;
    const uint64_t (*coeff)[2];
    const int32_t *sign;
} cali_chars_second_step_t;
static const uint8_t *current_image;
static int esp_efuse_get_field_size(const esp_efuse_desc_t **desc) {
    assert(desc[0] && !desc[1]);
    return desc[0]->bit_count;
}
static int esp_efuse_read_field_blob(const esp_efuse_desc_t **desc, void *out, int bits) {
    assert(bits==esp_efuse_get_field_size(desc) && bits<=32);
    const esp_efuse_desc_t *d=desc[0];
    assert(d->efuse_block>=0 && d->efuse_block<=10 && d->bit_start>=0);
    int size=d->efuse_block<2 ? 24 : 32;
    int offset=d->efuse_block<2 ? d->efuse_block*24 : 48+(d->efuse_block-2)*32;
    assert(d->bit_start+bits<=size*8);
    uint32_t value=0;
    for (int i=0;i<bits;i++) {
        int bit=d->bit_start+i;
        value|=((uint32_t)((current_image[offset+bit/8]>>(bit%8))&1))<<i;
    }
    *(uint32_t*)out=value;
    return 0;
}
static void *memset(void *out, int value, size_t count) {
    unsigned char *p=out;
    for (size_t i=0;i<count;i++) p[i]=(unsigned char)value;
    return out;
}

/* The source generator inserts explicit test images before this adapter. */
__attribute__((noreturn)) static void finish(int code) {
    __asm__ volatile("syscall" : : "a"(60L), "D"((long)code) : "rcx","r11","memory");
    __builtin_unreachable();
}
static void emit(uint32_t word) {
    uint8_t bytes[4]={(uint8_t)word,(uint8_t)(word>>8),(uint8_t)(word>>16),(uint8_t)(word>>24)};
    long result;
    __asm__ volatile("syscall" : "=a"(result) : "a"(1L),"D"(1L),"S"(bytes),"d"(4L) : "rcx","r11","memory");
    if (result!=4) finish(2);
}
__attribute__((noreturn,force_align_arg_pointer)) void _start(void) {
    emit(0x41444331); emit(1); emit(sizeof(images)/sizeof(images[0])); emit(4);
    for (uint32_t index=0;index<sizeof(images)/sizeof(images[0]);index++) {
        current_image=images[index];
        int version=esp_efuse_rtc_calib_get_ver();
        emit(index); emit((uint32_t)version);
        assert(version==1);
        for (uint32_t unit=0;unit<2;unit++) for (int atten=0;atten<4;atten++) {
            uint32_t digi, mv;
            emit(esp_efuse_rtc_calib_get_init_code(version,unit,atten));
            assert(esp_efuse_rtc_calib_get_cal_voltage(version,unit,atten,&digi,&mv)==0);
            emit(digi); emit(mv);
            if (index<4) {
                cali_chars_second_step_t ctx;
                adc_cali_curve_fitting_config_t config={(int)unit,atten};
                curve_fitting_get_second_step_coeff(&config,&ctx);
                uint32_t coefficient=65536*mv/digi;
                for (uint32_t raw=0;raw<=4095;raw++) {
                    uint64_t v=(uint64_t)raw*coefficient/65536;
                    emit((uint32_t)((int32_t)v-get_reading_error(v,&ctx)));
                }
            }
        }
    }
    finish(0);
}

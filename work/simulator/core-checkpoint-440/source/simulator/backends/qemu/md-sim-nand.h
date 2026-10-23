/* Existing QEMU 9.2.2 NAND pin interface. See vendor provenance.json. */
#ifndef MD_SIM_NAND_H
#define MD_SIM_NAND_H
#include "hw/core/qdev.h"
#include "system/block-backend.h"
DeviceState *nand_init(BlockBackend *, int, int);
void nand_setpins(DeviceState *, uint8_t, uint8_t, uint8_t, uint8_t, uint8_t);
void nand_getpins(DeviceState *, int *);
void nand_setio(DeviceState *, uint32_t);
uint32_t nand_getio(DeviceState *);
uint32_t nand_getbuswidth(DeviceState *);
void md_nand_tick(DeviceState *, uint64_t);
uint64_t md_nand_deadline(DeviceState *);
#endif

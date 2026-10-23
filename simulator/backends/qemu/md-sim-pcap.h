/* SPDX-License-Identifier: GPL-2.0-or-later */
#ifndef MD_SIM_PCAP_H
#define MD_SIM_PCAP_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
typedef struct MdPcap MdPcap;
typedef struct MdPcapProgress {
    uint64_t received, omitted_preamble;
    char sha256[65];
} MdPcapProgress;
MdPcap *md_pcap_new(const uint8_t *, size_t, const uint8_t *, size_t);
void md_pcap_free(MdPcap *);
void md_pcap_reset(MdPcap *);
void md_pcap_begin(MdPcap *);
void md_pcap_reject(MdPcap *);
uint32_t md_pcap_write(MdPcap *, const uint8_t *, size_t, MdPcapProgress *);
const char *md_pcap_error(const MdPcap *);
#endif

# Zynq system watchdog reference contract

This model replaces the pinned QEMU `zynq.swdt` placeholder. It uses the
existing QEMU CPU, Clock and PTIMER implementations. Rust implements the
register rules. C connects these rules to QEMU devices and timers.
This is a reference model. It is not an exact-board acceptance result.

The archived S9 device tree enables the device at `0xf8005000`. It selects
clock 45 and interrupt 41. The archived kernel driver is
`drivers/watchdog/cadence_wdt.c`, SHA256
`997a88046439043abd6609d9c3be1b68de6eaf57b56a5f5c5d564e9bb633c75f`.
Its archive SHA256 is
`5aa5c0915c93c6078984b13367edd12670e8ad25d922d352654f768d7005db8f`.
The driver writes the mode and count keys, then restarts with `0x1999`.
This source is a register-interface reference. It is not a verified source
match for the archived2022 Braiins kernel, which identifies itself as
`4.4.0-xilinx-g518c00cd977f-dirty`. Its actual interrupt message also differs
from this source member. The native fault tests retain the executing kernel
bytes, observed writes and actual interrupt message separately.

The [AMD functional description](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/Functional-Description?contentId=uWrBOoz9bl5UwhIJu9zzgw)
defines a counter that runs even with outputs disabled. At zero it stops.
A valid restart reloads the counter and prescaler and clears status.
The [counter register](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/Register-XWDTPS_CCR_OFFSET-Details?contentId=mTM6sIsyYb~Lxx_n4oyxKg)
selects divisors 8, 64, 512, and 4096 and a 24-bit reload value whose low 12 bits are ones.
The [mode register](https://docs.amd.com/r/en-US/ug585-zynq-7000-SoC-TRM/Register-XWDTPS_ZMR_OFFSET-Details?contentId=By12~sYsDAH7GekzEPxguA)
separately enables interrupt and reset outputs. Interrupt duration is 4, 8, 16,
or 32 CPU_1x clock cycles. The field keys read as zero.

Two QEMU one-shot timers implement the counter and interrupt pulse.
Clock selection follows SLCR `WDT_CLK_SEL`. The internal clock follows
CPU_1x, including its gate and divider. External MIO/EMIO clocks have no
connected source. No external clock edges are generated.

The following limits remain explicit:

- Register reset values are documented. The hidden counter value at reset
  is not established by the collected reference. The model loads the default
  reload value at reset. Driver tests must use an explicit restart.
- The archived driver names bits 6:4 as reset pulse length. UG585 v1.15 calls
  them reserved read/write bits. Readback is retained. They do not change
  the model's system reset request. External reset pin waveforms are absent.
- A missing reference clock causes a bus error. Physical APB hang timing is
  not implemented. Changing the source while enabled is undefined in UG585.
- QEMU PTIMER restarts fractional phase on clock or divisor changes.
  Dynamic prescaler sampling has no physical calibration evidence.
- Expiry requests a real QEMU guest reset. BootROM is not implemented.
  No ROM-written reboot reason is manufactured. Full reset-domain and
  reset-reason conformance remains open.
- Migration is rejected. No state is silently omitted from a snapshot.

Saved primary responses are in `work/simulator/qemu-s9/watchdog-reference-231`.
The source inventory and exact kernel member are in `watchdog-source-229`
and `work/simulator/kernel-watchdog-source-229`. Original inputs are unchanged.

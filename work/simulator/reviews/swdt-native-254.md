# Native watchdog fault review

This is an author review. The independent agents remain blocked by the
account limit. The independent review gate stays incomplete.

The Rust driver boots the unchanged archived2022 SD image from its original
SPL. The saved baseline supplies the existing private SD overlay. The test
uses the reference board models and no network interface. It runs archived
procd, ubus, the kernel driver and BusyBox devmem. No ARM test executable or
firmware patch supplies the result.

The native API starts with timeout30 and feed interval5. It selects timeout2
and interval1. After four guest seconds, actual register status must be zero.
The API then stops feeding. The real hardware timer must expire and raise its
IRQ. The actual kernel must report the interrupt, and devmem must read one.
After feeding resumes, status must return to zero. The test restores30/5 and
checks the native response.

The independent verifier does not call the Rust device model. It parses the
captured transactions. It checks keyed configuration, three status reads,
one deliberate IRQ-only expiry, the end of the pulse, and the restored count.
Both healthy and recovered feed sequences must span the observed hardware
timeout. Every feed gap and the final status-read gap must be shorter than
that timeout. Repeated output markers, echoed commands, reversed time, bus
errors, wrong status, missing recovery and missing restoration fail.

Failures are retained:

- Unit245 failed timestamp type inference. Unit246/247 found a marker at
  offset zero incorrectly rejected. Both were fixed and rerun.
- Native245 failed settings restoration. This API clamps the new feed interval
  against its old timeout. Restoration now sets timeout before interval.
- The first run used host-driven virtual time. Its actual pulse callback was
  delayed beyond the short-pulse assertion. Subsequent tests select an explicit
  instruction clock; the original failed run is unchanged.
- UBSan249 at instruction shift6 recorded a real expiry while feeding was
  enabled. A2.689-second guest feed gap exceeded the2.265-second timer. The
  strict audit rejected the run. No timing assertion was removed. The current
  test uses shift2 and retains both failed and successful clock cases.

Stronger final254 checks pass on UBSan232 and production239. Each has147
watchdog records, seven healthy pings and five recovered pings. The feed
spans exceed four seconds. Expiry follows the final ping by about2.26488732
seconds; the measured virtual IRQ pulse is36ns. Status reads are0,1,0. These
are simulator observations, not physical calibration measurements.

The actual kernel identifies itself as4.4.0-xilinx-g518c00cd977f-dirty. Its
interrupt text differs from the captured Bitmain source member. That member
is a register-interface reference, not a proven source match for this image.
The test records actual binary behavior and preserves this source uncertainty.

No exact-target, original-board, full dashboard or full-matrix gate is closed.

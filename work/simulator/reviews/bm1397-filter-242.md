# BM1397 ticket-register review

This is an author review. The independent agents are blocked by the recorded
account limit. This review does not close the independent review gate.

Register0x14 now retains the exact32-bit value per physical chain position.
Broadcast and addressed writes use the existing address policy. Readback and
the shared work converter receive the same value. Unpowered or reset chips
cannot provide a work filter. Reset and power loss clear the word to zero;
the separate fixed32-bit ASIC filter remains in force.

The new regression checks two addressed chips and every raw SHA digest bit.
It checks absent addresses, power/reset transitions and writes during reset.
The first run failed an old assertion that0x14 was unsupported. That check
now uses the next unresolved core command0x3c and still requires failure.
No test result was relabelled. The original failed log is retained.

All15 component tests,557 workspace tests in98 test binaries, strict Clippy,
formatting and497 guide links pass. Original probe035 uses unchanged factory
firmware and the same backend017. It exits1 at the real unsupported core
command0x3c=0x80008074. The ticket write is not reached. No complete firmware,
ASIC or mining result is claimed.

The source inspector also captures the existing archived PLL source and
its450MHz upstream test. These files give later divider evidence. They do
not resolve the core mailbox or establish physical PLL lock behavior.

Open work: core register semantics, timing, clock routing, regulator/reset
bindings, job queues, UART delivery, native Wi-Fi, exact board resolution,
fresh firmware mining and Core acceptance. The full matrix remains incomplete.

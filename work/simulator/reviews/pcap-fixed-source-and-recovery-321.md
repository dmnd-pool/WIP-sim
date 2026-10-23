# Fixed-source transfer and recovery review

This is an author review. Independent review remains blocked by the agent
usage limit. The full target and original-board gates remain incomplete.

The fixed-source DMA control previously stopped with an explicit model error.
The new path performs a fresh 32-bit read at the same source address for each
word. It preserves memory-read side effects. The incrementing path retains
bounded reads and checks its complete address extent. Fixed reads need only
one aligned source word. The trace records the source mode. Older traces lack
that field; the auditor reports their mode assumption separately.

The compiled test reads eight identical words from one RAM address, then
switches to incrementing reads and completes the selected configuration image.
An incorrect incrementing implementation encounters different data in that
first transfer. A separate full repeated stream fails at payload byte 32.
The old guard315 fails this positive test. The new sanitized backend320 and
release321 both pass 146 transactions and all four completed-image digests.
This is RAM-source evidence. No peripheral FIFO source was tested.

Review also found a sticky-failure defect. After rejecting PCAP_PR=0, the old
binding could accept a full image when that control was restored without a
programming reset. The compiled negative test records status 0x3004 where the
reference failure policy required 0. The fix calls the Rust rejection method
for every rejected transfer. It keeps the received prefix and prevents further
data until reset. The recovery test passes 136 transactions on both builds.
The ordinary transfer/reset test also passes 118 transactions on both builds.
Each suite checks eight actual interrupt edges and two QMP system resets.

The Rust tests check retained prefix identity, enable-only rejection and
explicit reset recovery. Auditor tests check fixed-address extent rules,
invalid source modes, duplicate fields and legacy traces. All 612 Rust tests
in 104 test binaries pass. Formatting, strict Clippy and release builds pass.
The C build uses undefined-behavior checks. Its build output has no compiler
warning or error diagnostic. Original firmware run321 on sanitized backend320
also passes independent mining checks: 1,642 computed hashes, two fresh jobs,
a clean-job transition, a difficulty change and 22 verified serial results.
Both block candidates pass Core acceptance and restart checks. Full native
dashboard cycle322 on release321 also passes. All four boots, signed update,
native browser, settings restoration and persistence pass. Its eight different
blocks pass Core acceptance and restart checks.

The model still uses synchronous DMA and a declared strict reference status
policy. These tests do not establish physical fault flags, peripheral FIFOs,
PCAP timing, the selected FPGA logic or the missing reset sequencer. The
existing configuration verifier remains necessary; status alone is not proof
of an actual configuration transfer.

Evidence:

- [Fixed-source change](../qemu-s9/pcap-derived-319/provenance.json).
- [Recovery defect and change](../qemu-s9/pcap-derived-320/provenance.json).
- [Old fixed-source failure](../qemu-s9/pcap-fixed-old-319/assessment.json).
- [Old recovery failure](../qemu-s9/pcap-control-old-320/assessment.json).
- [Sanitized fixed-source test](../qemu-s9/pcap-fixed-321/assessment.json).
- [Sanitized recovery test](../qemu-s9/pcap-control-321/assessment.json).
- [Release fixed-source test](../qemu-s9/pcap-release-fixed-322/assessment.json).
- [Release recovery test](../qemu-s9/pcap-release-control-322/assessment.json).
- [Source checkpoint](../qemu-s9/core-checkpoint-320/manifest.json).
- [Independent firmware mining audit](../qemu-s9/native-pcap-321/regtest-audit322.json).
- [Core candidate 0](../bitcoin-core/pcap321-candidate0-323/block-acceptance.json).
- [Core candidate 1](../bitcoin-core/pcap321-candidate1-323/block-acceptance.json).
- [Four-boot native cycle](../qemu-s9/pcap-dashboard-cycle-322/assessment.json).
- [Frozen cycle inputs](../qemu-s9/pcap-dashboard-cycle-322/driver-input.json).

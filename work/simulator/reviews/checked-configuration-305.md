# Checked configuration transfer review

This is an author review. The independent agents remain unavailable because
of their usage limit. The independent review gate is still open.

The original QEMU configuration port discarded FPGA DMA input. It also set
completion after interrupt-register writes. The strict contract now reads
guest memory and checks every transferred word against the captured image.
The image first passes container, packet, silicon ID, frame, padding and frame
error-code validation. This checks transfer integrity. It does not reconstruct
the logic that those configuration words implement.

The review found and fixed these defects:

- QEMU ignored the short global-property syntax because the device name has
  dots. The explicit driver/property/value form passes. Probe300 remains failed.
- A later configuration property could compete with the canonical property.
  The shared checker now rejects either form of competing option.
- The trace checker accepted an address plus length outside 32-bit memory.
  It now checks the complete range and requires an initial reset.
- Retained configuration evidence needed checks after input-store cleanup.
  Tests now prove retained-file use and reject changed bytes and symbolic links.

Both the release and undefined-behavior-sanitized backends pass 107 compiled
transactions. These include two system resets, programming reset, partial
DMA, full and sync-suffix transfers, interrupt writes and changed guest memory.
All three complete payload digests are checked. Original archived U-Boot also
transfers the actual selected SD payload before the firmware starts mining.

Local run302 and worker run304 use the same captured process. Their independent
audits verify 1,325 and 1,652 computed hashes respectively, two jobs, a clean
transition and a difficulty change. Four separate Core executions accept the
two candidates from each run and retain them after restart. Repeating frozen
seeds does not establish another fresh seed acquisition. The report retains
the exact original worker result and its equivalent coordinator encoding.
Cleanup acknowledges the original worker-result hash.

The full Rust check305 passed 606 tests in 104 test binaries. Formatting,
strict Clippy and release example builds passed. The affected hardened checker
has explicit negative tests. A complete replay306 is running against it. The
native dashboard and signed-update cycle307 will check this contract across
all four boots. Neither pending test is counted as a pass here.

Remaining work includes physical PCAP timing and error statuses, selected FPGA
logic, the reset sequencer, exact board identities, remote dashboard access,
independent review and the full target matrix. The model stops on unsupported
configuration paths. It does not invent hardware status to continue execution.
No original-board or exact-target completion gate is closed by this change.

Evidence: [hashed report](../reports/checked-configuration-305.json).

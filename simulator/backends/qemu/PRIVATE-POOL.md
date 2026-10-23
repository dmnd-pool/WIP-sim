# Private S9 pool development path

The archived S9 guest uses TCP to reach `10.0.2.100:3333`. The independent
Rust pool keeps each connection separate. The original shared character
stream was incorrect: a native Stratum V2 probe and a V1 submission could
enter the same stream. Run 025 retains the exact conflicting bytes. The
verifier does not remove those bytes or accept the damaged frame.

The command transport uses QEMU `guestfwd` with one process per connection.
Its guest network uses `restrict=on` and disables IPv6. This host blocks
the loopback socket pair that libslirp needs for this path. Run 026 records
that error. The outer process has no external network device.

The [private Ethernet adapter](PRIVATE-STREAM.md) provides another path.
A separate captured Linux VM runs the pool. It has one private interface
at `10.0.2.100/24` and no default route. The miner uses `10.0.2.15/24`.
The two emulators share a connected Unix socket pair. No host listener
or host network interface is required. The independent two-VM transport
test passes. Miner integration through `pool-vm=BUILD_DIRECTORY` passes in
[run 028](../../../work/simulator/qemu-s9/sd2022-private-pool-028/submission-audit-002.json).
The original firmware submits two independently verified accepted shares.
The audit also links the low-difficulty rejection to actual hardware output.

Build 060 enables libslirp 4.9.5 from installed package `libslirp-4.9.5-1`.
The shared library has SHA-256
`91ebce3a6429ccb3bf5331fdb06360ad2229d28c59cf0d43ae48e251535b4068`.
The [build report](../../../work/simulator/qemu-s9/backend-build-060-ubsan/build-report.json)
retains library, header and package-metadata hashes. The installed library's
source build has not been reproduced.

The original socket backend fails under this host's socket restrictions.
The earlier private pipe backend passed its
[startup probe](../../../work/simulator/qemu-s9/private-socket-probe-004/assessment.json).
Startup alone does not prove a TCP exchange. The
[firmware connection](../../../work/simulator/qemu-s9/sd2022-private-pool-015/pool-result.json)
retains the actual `mining.configure` request and verifier response.
That run fails because the verifier declined version rolling. It also
exhausts its 110-second emulator CPU budget during core startup.

The pool now negotiates the intersection of the client's mask and
`1fffe000`. It validates the sixth submission parameter and reconstructs
the full block version. The mask stays fixed after activation or the
first job. Unsupported extensions return false. Invalid masks return an
extension error. A minimum bit count above the intersection does not
disconnect the client.

The protocol basis is BIP 310 at archived commit
`927b6de9915c9262615a6399de51b200f81e5aa4`. Archive SHA-256 is
`f46ae5c782077fd37eb68d90ce7268a1dfa1137f744382db58deca352224b922`.
The exact BIP 310 member has SHA-256
`f130a8f0235e3ff5a1c8780eec4ff45e7dd9206ad90dcd5546468f109fa33103`.
See the [isolated inspection](../../../work/simulator/bip310-source-001/references.json).

The `local-pool` boot option requires the ASIC contract, a tested network
backend and a guest configuration script. The script writes a separate
private-overlay configuration before the original miner service starts.
The runner records the script, backend, library and generated pool work.
The installed firmware bytes remain unchanged. The recorded script sets
the actual guest configuration. Recent diagnostics disable autotuning and
set 100 MHz; those settings are explicit run inputs.

This diagnostic uses a separate mining-board resource profile. It permits
at most 360 CPU seconds with 3 GiB address space, 64 MiB file bounds and
private temporary storage. The runner sets the CPU allowance to its wall
budget minus ten seconds, with a ceiling of 330 CPU seconds. This follows
measured CPU exhaustion. It does
not change guest clock periods, watchdogs or the aggregate hash cap.
The pool stops before the outer wall deadline and exports its report even
if the guest emulator is paused. Run 029 produced two regtest candidates,
but its console exceeded the 240-second wall deadline. That run remains
failed. Run 031 added SD retention and write-through storage. It reached
the 230-second CPU ceiling before the full console test completed. That
run also remains failed. The runner now permits up to 360 wall seconds
and 330 CPU seconds. The pool still stops at 230 seconds. These limits
do not change guest clock periods.

The ASIC filter is native by default. The explicit
`adapt-fixed-filter-bits` option records a low-rate device adaptation.
Guest ticket-mask writes and readback remain consistent. Recent diagnostics
use four fixed bits and pool difficulties `2^-28` and `2^-27`. The
independent header audit accounts for both filters.
A successful connection, fresh pool job, or computed startup hash does
not establish mining acceptance. No
exact hardware target or full-board boot gate is passed by this development
path. Run 028 establishes fresh-work pool acceptance in its declared adapted
environment. The separate `regtest-genesis` option records the exact genesis
parent in pool-input schema 3. Block export requires successful execution and
the hardware/submission audit. Core acceptance requires a separate node run.

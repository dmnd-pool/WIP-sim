# Bitcoin ASIC catalogue and simulator implementation

This collection covers Bitcoin ASIC miners and the tools needed to study them.
It links devices, hashboards, control boards, ASIC chips, power supplies, firmware, source code, and documents.

The collection date is 19 October 2026.
Read [the current counts](reports/statistics.json) for the build time and download status.
A queued file is not a downloaded file.

## Simulator status and specification

Read the [detailed simulator specification](docs/simulator-specification.md) for the system to build.
It defines exact hardware profiles, firmware preparation, CPU and device models, low-rate hashing, dashboards, remote workers, and acceptance tests.
The [design basis](docs/simulator-design.md) gives the earlier evidence and reference implementations.
Use the [implementation prompt](docs/implementation-prompt.md) to assign the complete implementation task to an agent.
Use Rust first for new simulator code and C second. Use another language only when necessary, and record the reason.
Existing catalogue tools and upstream firmware can retain their languages.

The simulator implementation is **incomplete**. No exact physical miner target passes the full specification.
The Rust components and the source execution experiment below are working development tools.
The original-board, per-target Bitcoin Core, native-dashboard, remote-network, and full-matrix gates remain open.

Milestone 1.2 passed: adapted S9 mining and selected SSH/TLS access.
See the [handoff and resume steps](work/simulator/milestone-1.2-handoff.md).
The checkpoint was committed as `b276dd7a`. The user resumed work on
26 October. The full task remains incomplete. See the
[current continuation](work/simulator/continuation-2026-10-28.md).

The user requested a pause at [checkpoint518](work/simulator/checkpoint-518.md)
to save the weekly allowance. GPIO execution tests and the repaired CVC
component tests pass. Selected CVC FPGA execution and complete S9 boot remain
open. The checkpoint records the exact next steps and pending source work.

The isolated KVM support guest now passes Linux boot and a fresh private TCP
exchange. Its pinned x86 backend builds with undefined-behavior checks. See
the [independent evidence review](work/simulator/reviews/kvm-boot-build-independent-485.json).
The first selected native run [failed during evidence export](work/simulator/reviews/native-browser-failure-487.json).
The separate output transport now passes the [KVM stress check](work/simulator/reviews/support-trace-independent-489b.json)
and [worker receipt check](work/simulator/reviews/worker-trace-independent-490.json).
The [selected native browser and mining run495 passes](work/simulator/reports/native-browser-mining-pass-495.md).
It checks the actual dashboard through SSH and authenticated TLS, live native
telemetry and WebSockets, separate cookie stores, clock bounds, and shutdown.
The worker and client VM use one execution host. Exact-target and external
physical-host results are not inferred. Fourteen firmware submissions are
independently verified. Two resulting blocks pass Core acceptance and restart.

The [full shared CLI test500 also passes](work/simulator/reports/shared-cli-pass-500.md).
It creates and records fresh pool inputs on each invocation, then runs the
selected native SSH/TLS browser, independent mining verification and two Core
acceptance/restart checks. Run500 verifies15 firmware submissions. All1,069
workspace tests, strict Clippy and formatting passed for that source. The
checkpoint was committed as`f5fbf74e`. The user resumed implementation on
28 October. See the [saved checkpoint](work/simulator/checkpoint-500.md) and
the current continuation above.

These commands were tested in this prepared workspace:

```sh
work/simulator/cli-tools-500/md-sim demo s9-shared --check
work/simulator/cli-tools-500/md-sim demo s9-shared --out work/simulator/demos/s9-shared-500
```

Use a new output directory for another run; the shown path already exists.
The check verifies3,579 workspace and43 host files. It requires the pinned
firmware, helpers, Core, browser and KVM support environment. The report gives
the tested build command. Building the CLI alone does not install these inputs.
Dashboard URLs are inside the isolated client and stop with the suite.
Clean-install packaging and persistent host access remain incomplete.

### Adapted S9 development milestone

The [fresh demonstration passes](work/simulator/reports/adapted-s9-milestone-284.md).
Its four boots produced eight different independently verified blocks. Separate
Core nodes accepted all eight and retained them after restart.

The new start command runs the archived Braiins S9 firmware through four boots.
It uses a declared reference model with three chains of 63 BM1387 chips.
It checks fresh mining, eight separate Bitcoin Core block acceptance and
restart results, native settings persistence, a signed update and the native
dashboard in Firefox. This is an adapted development run. It is not an exact
physical S9 acceptance result. The [milestone record](work/simulator/milestones.md)
keeps the full specification and its remaining gates.

These commands use this prepared workspace. Run them from the repository root:

```bash
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=work/simulator/check-target-085 \
  cargo build -p md-sim --release --offline --locked
work/simulator/check-target-085/release/md-sim demo s9 --check
work/simulator/check-target-085/release/md-sim demo s9 \
  --out work/simulator/demos/s9-review-284
```

The output path already exists. Use a new path for another run. The check
command verifies 31 primary input hashes before any firmware starts. It needs
the archived firmware, prepared images, frozen helpers, QEMU backends, Core
binary and private browser payload listed in the
[input manifest](simulator/demos/s9-adapted.json).
It reports a missing or changed prerequisite as an error. Building the CLI
alone does not install these inputs. Clean-environment packaging is still open.
The offline build also needs the repository's vendored Cargo dependencies and
the declared Rust toolchain. Keep the linked archive storage available.

The full test took about 34 minutes in cycle263, including preparation and
independent checks. Each boot retains its own
logs, device transactions, adaptations, independent mining audit and Core
results under the chosen output directory. `run/progress.json` records each
completed settings phase. `assessment.json` is written at the end. A failure
returns a nonzero exit code and retains its evidence.
Finish other builds and tests before the browser phase. Demo280 exceeded its
native WebSocket deadline while other checks ran. Its failure is retained;
Demo284 passes with the same deadlines and assertions.

The default cap for this frozen demonstration is 1,000 hashes per second.
Its ASIC filter override and pool difficulty are explicit adaptations. The
archive stays unchanged. The complete model and firmware identities are in
the run's `input-manifest.json` and `run/driver-input.json`.

Dashboard access is limited to the private support VM in this command. Firefox
uses the actual native service at `http://10.0.2.15` inside that VM. The test
checks login, assets, page tables, live telemetry, WebSockets and isolated
cookies. It does not publish a host URL. Host network listeners are denied in
this environment. The selected local263 and SSH264 dashboard tests failed HTTP
readiness; they are not remote dashboard passes. SSH and TLS access for the
selected miner remain open gates.

The new [fabric clock and reset model](simulator/backends/qemu/FABRIC.md) is
tested separately. This demonstration keeps the frozen backend263 identity.
The selected FPGA bitstream still needs an exact source/build match or a
validated reconstruction. The [author review](work/simulator/reviews/adapted-s9-milestone-283.md)
records the execution boundary, regressions and unresolved review gate.

The [configuration inspection](work/simulator/reports/series7-inspection-288.json)
now checks the packet boundaries, silicon ID and both stored configuration
CRCs in the selected S9 bitstream. A CRC is a checksum for accidental data
changes. These checks do not decode the FPGA logic or prove board compatibility.
They run in the isolated preparation environment. The archived Braiins 0.2
testbench remains a separate reference for the controller model.

```sh
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=work/simulator/check-target-085 \
  cargo build -p md-sim-firmware --example s9_bitstream_probe --release --offline --locked
work/simulator/check-target-085/release/examples/s9_bitstream_probe \
  work/simulator/original-zynq/sd-bundle-006/boot/system.bit.gz \
  work/simulator/qemu-s9/series7-selected-288 --series7-idcode 03722093
```

Use a new output path for another run. `header.json` records the packet and
checksum results. Unsupported packet forms or failed checks return an error.
The current S9 bundle preparation also requires these checks. It keeps the
original firmware bytes unchanged. The frozen development demonstration above
retains its previously tested helpers and execution identity.

[Frame mapping](work/simulator/reports/series7-frame-mapping-291.json) uses a pinned Project X-Ray part database. It checks all 5,144
configuration frames, eight zero padding frames and each stored frame error
code in both retained S9 bitstreams. An error code detects damaged frame data.
Mapping does not reconstruct the FPGA logic. The tile and feature checks
below use separately pinned databases.

```sh
work/simulator/check-target-085/release/examples/s9_bitstream_probe \
  work/simulator/original-zynq/sd-bundle-006/boot/system.bit.gz \
  work/simulator/qemu-s9/frames-selected-291 --series7-idcode 03722093 \
  --part-json fixtures/fpga/xc7z010clg400-1/part.json \
  --part-sha256 1754ec1c7a8e0447a116984505cc422819d3be06389527de96bf192c5175d095
```

Use a new output path. The frame addresses, offsets, hashes and checked error
codes are in `header.json`. Bundle preparation also saves `fpga-frames.json`
and pins it in `bundle.json`. The [database provenance](fixtures/fpga/xc7z010clg400-1/provenance.json)
records the exact input. Firmware bytes stay unchanged.

The [tile mapping check470](work/simulator/reviews/tilegrid-reconstruction-470.md)
adds a hash-pinned Project X-Ray tile grid. It associates 5,746 configuration
ranges with physical FPGA tiles in the selected S9 image. An independent
byte audit checks every range. The report retains eight aliases and 222,968
frame words without a tile association. The tested command is in that report.
This maps configuration words. The [expanded feature probe475](work/simulator/reviews/fpga-next-reconstruction-475.md)
checks 28 pinned logic and routing feature tables against the selected image.
It finds 231,546 matching predicates. These account for all 327,095 set data
bits outside frame error codes. One monitor segment still lacks a table.
The [unchanged upstream checker476](work/simulator/reviews/feature-oracle-executed-476.md)
agrees on every full feature record, all eight aliases and all residual bits.
Complete routing, logic behavior and physical timing remain open.

```sh
work/simulator/check-target-085/release/examples/s9_feature_probe \
  simulator/firmware/s9-fpga-feature-inputs-475.json \
  work/simulator/qemu-s9/features-selected-475
work/simulator/qemu-s9/feature-oracle-driver-476/s9_feature_oracle \
  simulator/firmware/s9-fpga-feature-inputs-475.json \
  work/simulator/qemu-s9/features-selected-475/features.json \
  work/simulator/qemu-s9/feature-oracle-476b
```

Both commands passed in isolated processes with 512 MiB memory limits.
Use new output paths. These are static checks, not firmware execution.

The [FPGA assembly (FASM) export482d](work/simulator/reviews/fasm-export-executed-482d.md)
also passes. Its existing upstream assembler regenerates all 327,095 non-ECC
set bits. The original, regenerated and independent Rust bit-set digests agree.
The run uses a recorded one-byte tool-grammar correction and bounded parser
calls under a 1 GiB limit. The missing monitor table remains explicit.
This tested command needs a new output path when repeated:

```sh
work/simulator/qemu-s9/fasm-export-driver-482d/s9_fasm_export \
  simulator/firmware/s9-fpga-feature-inputs-475.json \
  simulator/firmware/s9-fasm-tool-inputs-escaped-quotes.json \
  work/simulator/qemu-s9/features-selected-475/features.json \
  work/simulator/qemu-s9/fasm-export-482d
```

The [selected FPGA converter](work/simulator/reviews/netlist-executed-497.md)
produces Verilog/XDC from that verified FASM file. This command also ran:

```sh
work/simulator/qemu-s9/netlist-driver-497/s9_netlist_reconstruct \
  simulator/firmware/s9-netlist-tool-inputs.json \
  work/simulator/qemu-s9/netlist-selected-497
```

Use a new output directory. Conversion does not establish HDL behavior or an
original-board boot. The next HDL validation gate remains open.

The [isolated Icarus12.0 tool build](work/simulator/reviews/hdl-tool-executed-502f.md)
now passes its upstream gperf tests, compiler checks, language vectors,
malformed-input rejection and installation restore test. This command ran:

```sh
work/simulator/qemu-s9/iverilog-driver-502f/build_iverilog_tool \
  /home/user/src/md-sim work/simulator/qemu-s9/iverilog-build-502f
```

Use a new output path for a repeat. The command needs the pinned source
archives and compiler snapshot in this prepared workspace. The saved tool
still requires its recorded library environment. The selected HDL check
will verify a smaller runtime closure. A compiler build is not FPGA behavior.

The report lists exact inputs and build commands. Complete netlist behavior,
physical timing and exact-board acceptance remain open.

Development builds omit debug symbols and incremental compilation to limit
disk use. To investigate a crash, set `CARGO_PROFILE_DEV_DEBUG=1` for that build.
Use one active Cargo target directory. Keep frozen release helpers and run
evidence separate from disposable debug caches. The
[cleanup record](work/simulator/reports/build-cache-cleanup-289.json) identifies
the removed caches. The [duplicate-input journal](work/simulator/reports/duplicate-build-input-cleanup-289.json)
gives a retained copy and checksum for each removed old build input.

The [independent ASIC clock tests](work/simulator/reports/asic-clock-295.json)
check that ASIC hashing and serial responses continue while the controller's
fabric clock is stopped. Compiled tests pass with undefined-behavior checks.
The same backend also runs the archived S9 firmware at a 10 H/s cap. An
independent audit verifies all 1,323 computed hashes and two fresh jobs.
Two separate Core nodes accept the firmware's block candidates and retain
them after restart. This is another adapted reference result. The frozen
development demonstration above retains its prior backend and input hashes.

The [checked configuration port](simulator/backends/qemu/PCAP.md) now reads and
checks the actual FPGA words sent by the guest. The earlier upstream QEMU port
reported completion without reading them. The expanded compiled test passes
118 transactions with undefined-behavior checks. It rejects corrupted guest
data and checks programming reset, two system resets and eight actual interrupt
edges. It caught and fixed stale reset interrupts and DMA completion caused by
register initialization. Physical initialization status remains a declared
reference adaptation; this does not close the original-board gate.
The fixed-address test passes 146 transactions. It reads the same 32-bit RAM
address for each word, then changes to incrementing reads. A separate
136-transaction test checks that rejected transfers require programming reset.

```sh
work/simulator/check-target-085/release/examples/qemu_ttc_reset \
  work/simulator/qemu-s9/backend-build-320-ubsan \
  work/simulator/qemu-s9/pcap-normal-321 pcap
work/simulator/check-target-085/release/examples/qemu_ttc_reset \
  work/simulator/qemu-s9/backend-build-320-ubsan \
  work/simulator/qemu-s9/pcap-fixed-321 pcap-fixed-source
work/simulator/check-target-085/release/examples/qemu_ttc_reset \
  work/simulator/qemu-s9/backend-build-320-ubsan \
  work/simulator/qemu-s9/pcap-control-321 pcap-control-recovery
```

Use a new output path for another run. The
[firmware transfer audit](work/simulator/qemu-s9/native-pcap-321/configuration-audit.json)
checks all 2,083,740 bytes sent by original U-Boot. Its
[independent mining audit](work/simulator/qemu-s9/native-pcap-321/regtest-audit322.json)
checks two fresh jobs and 1,642 computed hashes at a 10 H/s cap. Core accepts
[candidate 0](work/simulator/bitcoin-core/pcap321-candidate0-323/block-acceptance.json)
and [candidate 1](work/simulator/bitcoin-core/pcap321-candidate1-323/block-acceptance.json),
and retains both after restart. This verifies the configuration transfer in
the adapted reference environment on backend320, with undefined-behavior
checks enabled. The selected FPGA logic remains unresolved.

A prior prepared process also passes through a separate local worker.
The [transfer and mining report](work/simulator/reports/configuration-firmware-worker-319.json)
records destination hashes, result retrieval, cleanup, 1,318 independently
checked hashes and two Core-accepted blocks that survive restart. This uses
private inter-process communication (IPC). It does not test SSH, a remote
physical host or a published dashboard. It uses backend315, not backend320.

```sh
work/simulator/pcap-target-317/release/examples/s9_execute_prepared \
  work/simulator/qemu-s9/native-pcap-316 \
  work/simulator/qemu-s9/pcap-worker-318 \
  worker-ipc=work/simulator/ssh-target-263/release/md-sim-worker-ipc
```

Use a new output path to repeat this command. It needs the retained preparation
and its input store. It repeats the frozen job seeds; it does not acquire new
random seeds. Mining and Core evidence require the separate audit commands.

A separate [worker channel test](work/simulator/network-vm/native-worker-channel-355/assessment.json)
passes a fresh 1 MiB transfer, destination hash verification, download, actual
isolated process execution, result retrieval and cleanup. The native worker
runs beside a private client VM. Its support sandbox uses a non-root identity
with no retained capabilities. Each job has its own sandbox. This tests a
private virtual channel. The worker now waits for an exact result receipt from
the outer host before cleanup. The host verifies and saves the result before
it sends that receipt through a separate virtual port. A
[wrong receipt test](work/simulator/network-vm/native-worker-retention-353/assessment.json)
confirms that the worker refuses cleanup. The pinned support backend includes upstream virtual serial.
Its [build record](work/simulator/qemu-s9/backend-build-328-pool/build-report.json)
identifies the source, configuration and binary.

```sh
work/simulator/channel-target-354/release/examples/worker_channel_probe \
  work/simulator/ssh-target-263/release/md-sim-worker-ipc \
  work/simulator/qemu-s9/backend-build-328-pool \
  work/simulator/network-vm/native-worker-channel-355
```

Use a new output path to repeat the test. The two support executables have a
[checksum manifest](work/simulator/channel-target-354/manifest.sha256).
The [independent review](work/simulator/reviews/native-worker-independent-338.md)
records the transport, result-retention and isolation checks. The selected
native dashboard remains open on this layout.

The [SSH375 component test](work/simulator/network-vm/native-ssh-channel-375/assessment.json)
also passes. OpenSSH connects separate client and server namespaces in the
support VM to the native worker outside that VM. The test kills the first
client namespace. A new client finds the same running job and sees new output.
It then stops that job, retrieves the exact result and verifies cleanup.
The [review](work/simulator/reviews/native-ssh-outer-review-375.md) checks the
saved result, namespace changes, SSH sessions and cleanup receipt.

```sh
work/simulator/channel-target-375/release/examples/worker_channel_probe \
  work/simulator/channel-target-375/release/md-sim-worker-ipc \
  work/simulator/qemu-s9/backend-build-328-pool \
  work/simulator/network-vm/native-ssh-channel-375 \
  ssh-helper=work/simulator/channel-target-375/release/examples/remote_namespace_check
```

Use a new output path to repeat this command. Its
[executable hashes](work/simulator/channel-target-375/SHA256SUMS) are frozen.
This tests a support process and client loss. It does not test firmware,
dashboard access, an external physical host or loss of the complete VM.
This SSH component saves the result inside the VM before cleanup, then exports
it. The separate firmware354 path below tests the outer-host receipt.

Archived S9 firmware also passes this path with the outer-host receipt in
[firmware354](work/simulator/reports/native-worker-firmware-358.json).
Its [independent mining audit](work/simulator/network-vm/native-worker-firmware-354/execution/regtest-audit-356.json)
checks 1,322 computed hashes, two fresh jobs, a clean-job transition, a difficulty
change and 14 matched ASIC results. Separate Core nodes accept
[the first block](work/simulator/bitcoin-core/channel354-candidate0-357/block-acceptance.json)
and [the second block](work/simulator/bitcoin-core/channel354-candidate1-357/block-acceptance.json),
then retain each after restart. The host saves and verifies the complete
103,542,771-byte result before it sends the cleanup receipt. The run took
7 minutes 29 seconds. It uses adapted backend320. This is a local VM and native
worker, with no external physical-host claim.

```sh
work/simulator/channel-target-354/release/examples/worker_channel_probe \
  work/simulator/ssh-target-263/release/md-sim-worker-ipc \
  work/simulator/qemu-s9/backend-build-328-pool \
  work/simulator/network-vm/native-worker-firmware-354 \
  prepared=work/simulator/qemu-s9/native-pcap-321
```

Use a new output path to repeat it. This command requires the captured input
store in `native-pcap-321`. Independent mining and Core acceptance use the
separate audit commands. The command does not expose a browser URL.

The [shared SSH firmware run391](work/simulator/reports/shared-native-worker-394.md)
also passes execution, independent fresh-work verification and two separate
Core acceptance/restart checks. It uses archived S9 firmware, adapted
backend343 and a cap of 10 H/s. The independent audit checks 1,351 computed
hashes, two jobs, a clean-job transition and a difficulty change. The host
retains both the exact firmware result and the separate pool evidence before
it acknowledges worker cleanup. The complete run took about 12 minutes 28
seconds. Its report gives the tested command, frozen inputs and evidence.
This remains an adapted reference result on one physical host. It does not
publish a native dashboard URL. Review after the freeze found a broker
control-file issue; the current source fixes it, but the changed helper still
needs a fresh integration run. Keep that limit with the frozen command.

The later [selected access run431](work/simulator/reports/shared-access-failure-431.md)
passes native HTTP checks through SSH: login, original assets, API data, and
configuration change/readback/restoration. Its complete access suite fails
at TLS listener readiness. No TLS or browser pass is claimed for that run.
The failed evidence is retained.

After the bounded startup fix, [selected access run441](work/simulator/reports/shared-access-pass-441.md)
passes 15 native HTTP exchanges through SSH and 15 authenticated TLS exchanges
from a separate client in the support VM. Its same-run mining audit verifies
1,687 computations, two fresh jobs and five accepted shares from seven
submissions. Two isolated Core nodes accept distinct blocks and retain them
after restart. The report gives the exact tested commands, hashes and access
boundary. This remains adapted hardware on one execution host. Browser,
WebSocket, exact-target and full remote-workflow gates remain open.

The [recovered run453 mining evidence](work/simulator/reports/shared-recovery-pass-461.md)
also passes strict header verification and two Core acceptance/restart checks.
Recovery keeps the original browser failure unchanged. The report gives the
tested Rust retention and verification commands, input hashes and the
namespace startup fixes needed in the current host environment.

The [recovered run492 evidence](work/simulator/reports/shared-recovery-pass-492.md)
checks 1,660 computations, 12 firmware submissions and two more Core block
acceptance/restart results. Its browser failed because it sampled telemetry
before the first accepted share. That failure remains recorded. The next
run uses the existing native DOM readiness check before sampling, with the
same deadlines and positive-share assertions.

Both [source preparation paths](work/simulator/reviews/source-preparation-process-limit-467.md)
also pass with the corrected namespace startup. The installed-tool build and
the frozen-toolchain replay produce the same cgminer binary. Both pass the
C regression with the undefined-behavior sanitizer. The report gives the
tested commands and exact source, toolchain and output identities.

The [T17 cleanup report](work/simulator/reviews/t17-cleanup-resume-460.md)
records 128 verified duplicate files removed, totaling 1.51 GiB of logical data.
All 114 remaining files and 43 canonical restore copies passed the post-check.
Original downloads and unique evidence remain in place.

The [cleanup journal](work/simulator/reports/duplicate-build-input-cleanup-305.json)
records another 698 MB of removed duplicate build inputs and their restore
copies. Original firmware, binaries and run evidence remain available.

The [four-boot configuration and dashboard cycle](work/simulator/qemu-s9/pcap-dashboard-cycle-322/assessment.json)
also passes native signed installation, browser checks, settings restoration
and restart persistence. Its eight distinct blocks pass Core acceptance and
restart checks. That result uses release321 and its
[frozen run inputs](work/simulator/qemu-s9/pcap-dashboard-cycle-322/driver-input.json).
The
[reset review](work/simulator/reports/configuration-reset-review-316.json)
records the earlier fixes and their separate tests. Backend320 adds the tested
fixed-address RAM path and failure latch. Peripheral FIFO behavior, physical
status/timing and exact FPGA logic still need implementation and evidence.

The [checkpoint review](work/simulator/reviews/checkpoint-artifact-path-166.json)
found that the build helper could copy an old Rust library when Cargo used a
different output directory. Checkpoint166 uses Cargo's reported artifact path
and checks its hash before and after copying. Earlier retained binaries and
run results keep their identities. Their source-to-library link needs the
qualification recorded in that review. A source snapshot alone does not prove
which library was linked.

Review found a process namespace defect in earlier execution builds. Their
`/proc` view could belong to the enclosing environment. The corrected launcher
passes the new local, worker and actual SSH component checks. Firmware validation is in progress.
Earlier results do not establish isolation. See the
[finding, rejected first fix, and regression evidence](work/simulator/reviews/proc-namespace-127.md).

The corrected launcher has now run the archived S9 firmware with the
[reproducible QEMU build](work/simulator/qemu-s9/backend-build-reproducibility-130.json).
The [run result](work/simulator/qemu-s9/native-batch-131/result.json) and
[native HTTP checks](work/simulator/qemu-s9/native-batch-131/dashboard-result.json)
record the current validation. This run uses explicit reference hardware and
console adaptations. Independent replay passed
[before](work/simulator/qemu-s9/native-batch-131/regtest-audit-session-0.json) and
[after](work/simulator/qemu-s9/native-batch-131/regtest-audit-session-4.json) the
native miner restart. Four fresh block candidates passed separate Core nodes.
It does not close the exact-board gate. See also the
[network persistence evidence correction](work/simulator/reviews/network-baseline-131.md).

The corrected launcher also passes the
[signed update and dashboard restart cycle](work/simulator/qemu-s9/automated-update-cycle-proc-132/assessment.json).
Four boots each produced independently verified fresh work and two
Core-accepted blocks. The cycle checks native browser login, live telemetry,
WebSocket delivery, setting write/readback, restoration and restart persistence.
Disk inspection records the journal replay needed after the native update.
This is a local, adapted S9 reference run. Exact-board recovery and remote
publication are separate open gates.

The [browser review](work/simulator/reviews/boser-websocket-policy-162.json)
found a gap in these earlier checks. They checked login and real GraphQL data,
but did not require the native page tables to show that data. The current suite
adds that assertion. The archived Boser application also fixes its WebSocket
scheme to `ws://`. Its selected HTTPS gateway now records an explicit
`upgrade-insecure-requests` response-header policy. The original application
bytes stay unchanged. This change still needs the selected browser test to pass.

The [native NAND persistence review](work/simulator/reviews/nand-persistence-139.md)
records three linked boots with saved-data readback, erase, another restart,
and erased-data readback. Each boot has independent fresh-work verification,
two Core-accepted blocks and a clean SD check. These tests use generated
reference NAND and paired private disk overlays. Full-media writes, faults,
power-loss recovery and exact physical NAND behavior remain open.

The [automated NAND cycle](work/simulator/qemu-s9/automated-nand-cycle-203/assessment.json)
also passes with fresh 4160-byte data across three NAND pages. It requires
saved-data readback, erase, erased-data readback after restart, paired disk
identity checks, clean SD filesystems, and six separate Core block tests.
Each boot also checks the actual monotonic hash grants against the aggregate cap.
This exact command was tested with the retained source203 helpers and inputs:

```bash
work/simulator/ssh-target-203/release/examples/s9_nand_cycle \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/backend-build-201-ubsan \
  /tmp/md-sim-qemu-pool-003-ubsan \
  work/simulator/bitcoin-core/source-build-002/bitcoind \
  work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin \
  work/simulator/qemu-s9/automated-nand-cycle-203
```

The output already exists. Choose a new output path to repeat the test.
The [input manifest](work/simulator/qemu-s9/automated-nand-cycle-203/driver-input.json)
and [source snapshot](work/simulator/qemu-s9/core-checkpoint-203/manifest.json)
identify the tested code and prerequisites. The Rust driver sends commands to
the unchanged guest shell and its native `mtd`, `dd`, and `cmp` tools.
Rust or C test programs inside that guest would add executable code to the
firmware environment. The host coordinator and independent checks use Rust.
This command tests the adapted reference environment. It does not select an
exact physical target or close the full NAND gate.

The [controlled interruption cycle](work/simulator/qemu-s9/interrupted-nand-cycle-207/assessment.json)
also passes. It stops the owned emulator after native NAND writes complete.
The first SD observation records incorrect free-block and free-inode counts.
It does not count that disk as clean. Two later boots use the saved media and
the native firmware. Both require clean filesystem checks, NAND readback or
erase readback, verified hash grants, and two Core-accepted blocks per boot.
The interrupted boot also supplies two independently verified Core blocks.

This command passed with the [source207 inputs](work/simulator/qemu-s9/interrupted-nand-cycle-207/driver-input.json):

```bash
work/simulator/ssh-target-207/release/examples/s9_nand_cycle \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/backend-build-201-ubsan \
  /tmp/md-sim-qemu-pool-003-ubsan \
  work/simulator/bitcoin-core/source-build-002/bitcoind \
  work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin \
  work/simulator/qemu-s9/interrupted-nand-cycle-207 controlled-interruption
```

Choose a new output path to repeat it. The [phase evidence](work/simulator/qemu-s9/interrupted-nand-cycle-207/progress.json)
links all three executions, paired media, disk checks, mining audits and six
Core results. This is a software interruption after completed writes. It does
not establish physical power-loss behavior or interrupted-write atomicity.
The exact hardware and independent review gates remain open.

| Available now | Required work still open |
| --- | --- |
| Frozen catalogue import, exact-target validation, and complete inventory accounting | Complete exact hardware profiles and executable bundles for every applicable target |
| SHA-256d engine, independent share verifier, low-rate filters, and work transitions | Full board buses, ASIC chains, power, cooling, storage, and fault models |
| Archived cgminer source execution, independent fresh-work verification, and Core regtest block acceptance through an adapted S2 controller interface | Original archived board boot and mining through exact hardware models |
| Archived S9 SPL, U-Boot, kernel, native services and mining in a recorded reference environment | Exact controller, FPGA, power and cooling models, and the original-board gate |
| Hashed input storage, isolated process execution, live logs, cancellation, result transfer, cleanup, and private IPC tests; SSH, TLS and actual S9 native browser tests in a disconnected VM | Complete remote execution and native dashboard tests for each exact target |
| Strict incomplete matrix reports and source input manifests | All required per-target suites and physical calibration |

The commands in the next section were built and exercised.
The later sections marked **proposed workflow** remain requirements. They are not supported miner workflows.

The [TTC clock regression](work/simulator/qemu-s9/ttc-clock-qtest-214/assessment.json)
passes 328 compiled timer transactions with
[backend213](work/simulator/qemu-s9/backend-build-213-ubsan/build-report.json).
Both Triple Timer Counter (TTC) blocks now use the programmed Zynq clock.
The test checks all six channels, clock gating, prescalers, stopped counters,
and fractional event deadlines. The old backend fails 68 expectations in the
[retained baseline](work/simulator/qemu-s9/ttc-clock-baseline-211/assessment.json).
This is a device test. It does not establish exact-board boot or calibration.

The [system reset regression](work/simulator/qemu-s9/ttc-reset-qtest-218/assessment.json)
passes 384 transactions and two actual QEMU system resets. It checks all six
TTC channels, their saved register values, stopped counters, and clean process
exit. Backend213 fails the
[baseline reset check](work/simulator/qemu-s9/ttc-reset-baseline-217/assessment.json).
The following command passed with the separate reset patch in backend218:

```bash
work/simulator/qemu-s9/ttc-reset-driver-218/qemu_ttc_reset \
  work/simulator/qemu-s9/backend-build-218-ubsan \
  work/simulator/qemu-s9/ttc-reset-qtest-218
```

Choose a new output directory to repeat it. The test uses private pipes inside
the isolated process. It exposes no emulator control port.

The [firmware interruption cycle on backend213](work/simulator/qemu-s9/interrupted-nand-cycle-213/assessment.json)
also passes all three boots, mining rate checks, fresh-work audits and six Core
block acceptance and restart checks. Its
[phase evidence](work/simulator/qemu-s9/interrupted-nand-cycle-213/progress.json)
and [input manifest](work/simulator/qemu-s9/interrupted-nand-cycle-213/driver-input.json)
identify each result. This command was tested:

```bash
work/simulator/ssh-target-213/release/examples/s9_nand_cycle \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/backend-build-213-ubsan \
  /tmp/md-sim-qemu-pool-003-ubsan \
  work/simulator/bitcoin-core/source-build-002/bitcoind \
  work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin \
  work/simulator/qemu-s9/interrupted-nand-cycle-213 controlled-interruption
```

Choose a new output directory to repeat it. This remains an adapted reference
run and a software interruption after completed writes. It does not establish
an exact physical target or atomic recovery from an interrupted media write.

The [local208 mining evidence](work/simulator/network-vm/board-local-icount-208/mining-assessment-221.json)
contains two completed mining sessions and four Core-accepted blocks. Each
block remains accepted after a separate node restart. The native dashboard
test failed. These mining results do not change that failed result.
The following command regenerated observations from the retained execution:

```bash
work/simulator/evidence-driver-221/s9_retain_evidence \
  work/simulator/network-vm/board-local-icount-208/board-result \
  work/simulator/network-vm/board-local-icount-208/retained-replay-221
```

Choose a new output directory to repeat it. The tool checks the locked input
hashes, process identity and original execution streams before it writes the
derived reports. It preserves the original files and verdicts. The
[retention record](work/simulator/network-vm/board-local-icount-208/retained-replay-221/retention.json)
pins the inputs and tool. Separate mining audits and node tests are still
required. The [negative tests](work/simulator/network-vm/retention-negative-221/assessment.json)
reject changed input, execution identity and guest logs.

The [signed update cycle on backend218](work/simulator/qemu-s9/automated-update-cycle-218/assessment.json)
passes provisioning, signed installation, settings write, restoration and two
restart readbacks. Four boots produce eight independently checked blocks.
Separate Core instances accept each block and retain it after restart. The
native browser checks login, live mining data, WebSocket traffic and cookie
isolation. This command passed:

```bash
work/simulator/ssh-target-218/release/examples/s9_dashboard_cycle \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/backend-build-218-ubsan \
  /tmp/md-sim-qemu-pool-003-ubsan \
  work/simulator/qemu-s9/automated-update-cycle-218 \
  signed-update=work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin \
  browser-payload=work/simulator/network-vm/native-driver-024/payload.json
```

Choose a new output directory to repeat it. The
[input manifest](work/simulator/qemu-s9/automated-update-cycle-218/driver-input.json)
and [phase results](work/simulator/qemu-s9/automated-update-cycle-218/progress.json)
identify this adapted reference test. The exact-board, full native dashboard,
remote publication and full target matrix gates remain open.

The corrected timer model also passes the
[322-command down-counter test](work/simulator/qemu-s9/ttc-down-qtest-228/assessment.json)
and [reset interrupt test](work/simulator/qemu-s9/ttc-reset-irq-228/assessment.json).
The latter checks385 transactions and24 actual interrupt-wire transitions.
These frozen commands were tested:

```bash
work/simulator/ssh-target-227/release/examples/s9_backend_qtest \
  work/simulator/qemu-s9/backend-build-226-ubsan \
  work/simulator/qemu-s9/ttc-down-qtest-228 ttc-decrement
work/simulator/ssh-target-227/release/examples/qemu_ttc_reset \
  work/simulator/qemu-s9/backend-build-226-ubsan \
  work/simulator/qemu-s9/ttc-reset-irq-228
```

Choose new output directories to repeat them. See the
[author review](work/simulator/reviews/ttc-modes-decrement-226.md) for the source
identities, failed baselines and limits. These device tests do not establish
firmware or exact-board acceptance.

The Zynq system watchdog now has a Rust register model and a QEMU timer
interface. The [watchdog contract](simulator/backends/qemu/SWDT.md) states its
reference scope and limits. These frozen commands passed:

```bash
work/simulator/ssh-target-233/release/examples/s9_backend_qtest \
  work/simulator/qemu-s9/backend-build-232-ubsan \
  work/simulator/qemu-s9/swdt-qtest-234 swdt
work/simulator/ssh-target-233/release/examples/qemu_ttc_reset \
  work/simulator/qemu-s9/backend-build-232-ubsan \
  work/simulator/qemu-s9/swdt-reset-234 swdt
```

Use new output directories to repeat them. The
[146-response device test](work/simulator/qemu-s9/swdt-qtest-234/assessment.json)
checks interrupt timing and clock control. The
[reset test](work/simulator/qemu-s9/swdt-reset-234/assessment.json) includes a
reset caused by the watchdog and checks that old callbacks stop after reset.
See the [review and open limits](work/simulator/reviews/swdt-234.md).

The archived firmware also passes a native watchdog fault and recovery test.
This verified command uses the frozen Rust driver:

```bash
work/simulator/qemu-s9/watchdog-driver-254/s9_watchdog_check \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/backend-build-232-ubsan \
  work/simulator/qemu-s9/automated-update-cycle-004/provision/sd-snapshot-0.json \
  work/simulator/qemu-s9/watchdog-fault-254
```

Use a new output directory to repeat it. The test uses the firmware's own
`ubus` API to set a timeout, stop feeding, resume feeding and restore settings.
Its own `devmem` tool reads status. Rust verifies the actual device transactions
and kernel interrupt message. These shell commands exercise native guest tools;
adding a Rust or C guest executable would add code to the firmware under test.
The [UBSan result](work/simulator/qemu-s9/watchdog-fault-254/assessment.json)
and [production result](work/simulator/qemu-s9/watchdog-fault-release-254/assessment.json)
both show status `0 → 1 → 0`, a real interrupt and restored settings.
They use an explicit instruction clock. This is a reference fault test,
not physical timing calibration or a complete board result.
See the [fault-test review](work/simulator/reviews/swdt-native-254.md).

The shared `/tmp` filesystem rejected a test fixture with a quota error.
The complete553-test suite then passed with normal parallel execution and a
private temporary directory. The test assertions and isolation limits were
unchanged. These commands use that tested directory:

```bash
mkdir -p work/simulator/test-tmp-227
TMPDIR="$PWD/work/simulator/test-tmp-227" CARGO_INCREMENTAL=0 \
  CARGO_TARGET_DIR=work/simulator/check-target-085 \
  cargo test --workspace --all-targets --offline --locked
```

## Build and test the implemented components

Use 64-bit Linux, Rust 1.94.1, and Cargo 1.94.1 for the tested build.
The Rust dependency sources are in `vendor/`. `Cargo.lock` pins their versions and package checksums.
The project Cargo configuration uses these saved sources. No registry connection is needed.
An empty Cargo cache and a separate build directory were used for the clean build check.
The three release executables match the normal workspace build byte for byte.
The [v11 source manifest](work/simulator/reports/source-v11.json) pins that checkpoint's source files and vendored Rust dependencies.
The [v11 test log](work/simulator/reports/tests-v11.log) records 64 passing Rust tests.
Implementation has continued after that checkpoint. See the [active progress record](work/simulator/progress.md) for current changes and checks.
The support-image regression tests require GNU cpio at `/usr/bin/cpio`.
Isolated execution also requires util-linux (`unshare` and `prlimit`) and
Bubblewrap. Static executable inspection uses `readelf` inside the isolated
preparation environment. The runtime records the isolation tool hashes.

The BM1397 protocol component also has a verified test command:
`cargo test -p md-sim-firmware --lib bm1397 --offline --locked`.
It checks actual captured command bytes, reply checksums against a pinned
upstream driver, per-chip address assignment and register storage, reset,
and job hashes with an independent hash library.
The ticket register preserves all programmed bits for readback and work
filtering. Power loss and reset restore its documented value. The original
firmware still stops at the unsupported core-register command in
[probe 035](work/simulator/bitaxe/original-probe-035/stage-audit-root.json).
See [its evidence and open work](work/simulator/bitaxe/bm1397-progress.md).
This component does not establish a complete ASIC or miner execution path.
The core-command codec now checks49,152 outputs from the exact vendor ARM
routines. It keeps the encoded selector separate from physical core geometry.
It does not yet make the live model accept a core-register command.

The archived T17 update also supplies a static protocol reference. The
[inspection](work/simulator/bitaxe/t17-static-262/inspection.json) checks its
exact archive, nested image CRCs, extracted ARM executable and disassembly.
The [core mailbox analysis](work/simulator/bitaxe/t17-static-262/core-mailbox-analysis.json)
retains the exact instruction ranges and separates interpretation from measured
chip behavior. No firmware program executes in this inspection. This command
was tested; choose a new output path when repeating it:

```bash
cargo run --release -p md-sim-firmware --example t17_source_inspect --offline --locked -- \
  work/simulator/bitaxe/t17-static-264
```

The tool needs the acquired update named in its source, `debugfs`, `readelf`,
and Capstone5 at `/usr/lib/libcapstone.so.5`. The tested Capstone package is
5.0.9-1. The process manifest pins the executable and library bytes. Rust calls
the existing Capstone C interface; it does not implement a disassembler.
Analysis tools run in a private preparation namespace with a128MiB file bound
for the100MiB expanded disk. Earlier attempts with a64MiB file bound failed
and remain failed in the retained evidence.

The separate [T17 packing oracle](work/simulator/bitaxe/t17-core-oracle-272/assessment.json)
executes two unchanged64-byte vendor routines under QEMU ARM. All49,152 input
cases pass. This checks the driver word format; it does not boot the miner or
establish ASIC readiness. The [review](work/simulator/reviews/t17-core-oracle-272.md)
records source hashes, failed setup attempts, tool identities and limits.

The coordinator and verifier use Rust. A small C wrapper supplies the ARM32
ABI and Linux syscalls. The installed Rust targets have no ARM32 core library.
Embedded assembler copies the exact vendor instructions into the test ELF;
rewriting those instructions in Rust or C would remove the independent oracle.
The wrapper uses strict compiler warnings and undefined-behavior traps.
The original routines stay unchanged. This command uses the captured Rust1.99
linker, installed Clang23.1.1 and QEMU11.1.2:

```bash
cargo run --release -p md-sim-firmware --example t17_core_oracle --offline --locked -- \
  work/simulator/bitaxe/t17-static-262/bmminer \
  /home/user/.rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld \
  work/simulator/bitaxe/t17-core-oracle-273
```

Choose a new output path to repeat it. The linker and its LLVM library must
match the pinned hashes. The isolated process allows6GiB of virtual address
space for QEMU's4GiB ARM reservation. CPU, file, process and wall limits remain
separate. This is a development oracle with explicit prerequisites.

The optional [T17 collector oracle](work/simulator/reviews/t17-readback-executed-446.md)
passes eight cases with unchanged vendor instructions. It confirms that software
`FOUND` can leave stale data for the wrong chain or ASIC. Rust checks the results.
A small C fixture supplies the ARM32 ABI and bounded callbacks because the
installed Rust targets have no ARM32 core library. Strict warnings and UBSan
traps check the wrapper. Controlled queues and delay callbacks do not establish
ASIC readiness, physical timing, miner execution or original-board boot.
These build and run commands were tested; choose a new output directory:

```bash
CARGO_TARGET_DIR=work/simulator/check-target-085 \
  cargo build -p md-sim-firmware --example t17_core_oracle --release --offline --locked -j 1
work/simulator/check-target-085/release/examples/t17_core_oracle \
  work/simulator/bitaxe/t17-static-262/bmminer \
  /home/user/.rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld \
  work/simulator/bitaxe/t17-readback-oracle-446 --readback
```

The [core0 formatter oracle](work/simulator/reviews/t17-core0-executed-472.md)
also passes 5,192 cases with unchanged vendor instructions. An
[independent review](work/simulator/reviews/t17-core0-independent-472.md)
checks the original bytes, field extraction and all callback records.
Its C fixture uses the same required ARM32 interface. The new Rust readback
view checks the pending request and ASIC address. It does not report clock
readiness or enable the unsupported core-write path. This command was tested:

```bash
work/simulator/bitaxe/t17-core0-driver-472/t17_core_oracle \
  work/simulator/bitaxe/t17-static-262/bmminer \
  /home/user/.rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld \
  work/simulator/bitaxe/t17-core0-oracle-472 --core0
```

Use a new output directory to repeat it. The report pins the required tools.

The [PLL arithmetic oracle](work/simulator/reviews/t17-pll-executed-496.md)
passes 22,134 cases with an unchanged 108-byte T17 routine. Its C fixture uses
the same required ARM32 interface. Rust checks each original result and an
independent rational bound. This command was tested:

```bash
work/simulator/bitaxe/t17-pll-driver-496/t17_core_oracle \
  work/simulator/bitaxe/t17-static-262/bmminer \
  /home/user/.rustup/toolchains/1.99.0-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld \
  work/simulator/bitaxe/t17-pll-oracle-496 --pll-frequency
```

Use a new output directory to repeat it. The report pins its tools, source
and results. This arithmetic pass does not establish PLL lock, clock timing,
ASIC state or a valid physical operating range.

The [posted-control implementation501](work/simulator/reviews/bm1397-programmed-executed-501.md)
now stores evidenced core and PLL requests in the existing chip chain. The
[UART extension504](work/simulator/reviews/bm1397-uart-executed-504.md) records
FastUART and MiscControl words, with an exact requested CLKI baud view.
The [output-divider extension505](work/simulator/reviews/bm1397-divider-executed-505.md)
records`0x70` and checks the original software encoding. Original factory
probe039 completes the requested frequency ramp and reaches factory self-test.
It stops at unsupported hash-rate read`0x04`. The trace audit checks253
commands and245 posted updates, with no invented acknowledgement or nonce.
Physical clocks, UART timing, complete board execution and mining remain open.

The [S3 UART register tests now pass](work/simulator/reviews/esp32-uart-executed-506c.md).
They cover masks, mapped addresses, reset, two real controller deadlines and
the bounded trace. Original040 captures26 actual UART configuration writes
and stops at the same unsupported ASIC read. No mining job or nonce occurs.
The [UART model scope](simulator/backends/esp32/UART.md) keeps line timing,
clock-gate effects and physical calibration open.

The BM1387 packet and hashing checks are:
`cargo test -p md-sim-firmware --test bm1387_packets --lib --offline --locked`.
They check the archived command bytes and all three supported midstate counts.
An independent hash library checks the complete Bitcoin headers.
The fixed-work test computes its nonce with the shared rate-limited engine.
It preserves the programmed filter and decodes the computed reply as FPGA words.
See the [source review and remaining work](work/simulator/bm1387-source-review-001.json).
These tests do not establish fresh firmware mining or a powered ASIC chain.

The S9IO packet component has a verified test command:
`cargo test -p md-sim-firmware --test s9io_packets --offline --locked`.
It checks packet conversion, extended work IDs, checksum errors and receive
recovery against literal vectors from the archived FPGA test bench.
The clocked receive tests check bounded queues, interrupt latches,
backpressure and the serial checksum delays against the archived HDL.
The [source review](work/simulator/s9io-device-review-001.json)
records the queue and reset behaviors that the model preserves.
The [fixture](crates/md-sim-firmware/tests/fixtures/s9io-reference.json)
records its source hash. The transmit and register checks are:
`cargo test -p md-sim-firmware --test s9io_transmit --test s9io_registers --offline --locked`.
They cover transmit priority, stalls, timer clocks, byte strobes, FIFO limits,
interrupts, reset requests and last-work-ID readback.
The [transmit review](work/simulator/s9io-transmit-review-001.json)
records a full-queue interrupt defect that the model preserves.
The [register review](work/simulator/s9io-registers-review-001.json)
keeps the conflicting HDL and SVD version values explicit.
The UART, checksum and connected device checks are:
`cargo test -p md-sim-firmware --test s9io_uart --test s9io_crc --test s9io_receive --test s9io_device --offline --locked`.
They check clocked bit transmission, reception, baud changes and errors.
Independent bit waveforms test the connected register, checksum and UART path.
The tests use the original test bench's packets. They do not perform mining.
The AXI bus check is:
`cargo test -p md-sim-firmware --test s9io_axi --offline --locked`.
It checks bus handshakes, concurrent FIFO access, interrupt order and reset.
It retains two response-handling defects in the archived wrapper.
See the [AXI source review](work/simulator/s9io-axi-review-001.json).
The idle-clock check is:
`cargo test -p md-sim-firmware --test s9io_idle --offline --locked`.
It compares optimized advancement with each ordinary clock edge.
Bus and serial clocks keep their programmed rates. See the
[idle-clock review](work/simulator/s9io-idle-review-001.json).
The [QEMU interface checks](simulator/backends/qemu/S9IO.md) execute three
legacy controller blocks. Register, serial and interrupt routing tests pass.
Guest execution through this controller and physical timing calibration remain open.
These components do not establish a complete FPGA or miner execution path.
The [GPIO component checks](simulator/backends/qemu/GPIO.md) also test actual
input and output pins, register state and the selected SD image's declared
controller address and interrupt layout. Clock-domain and bitstream
correspondence work remains open.

The [ESP32 peripheral checks](simulator/backends/esp32/sar-adc/README.md)
include 37 Rust tests and a C interface check. The
[compiled ADC input test](work/simulator/bitaxe/sar-input-qtest-001/assessment.json)
passed 47 controller transactions. The
[original firmware stage record](work/simulator/bitaxe/esp-qemu-build-017/stage-review-root.json)
shows chip identification and the next unsupported ASIC command. It uses an
explicit test voltage. Physical power wiring and the full board gate remain open.

Run from the repository root:

```bash
cargo build --workspace --release --offline --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
cargo test --workspace --all-targets --offline --locked
python3 scripts/verify_bitcoin_fixture.py
python3 scripts/check_guide_links.py
```

The executables are in `target/release/`. Use `--help` to see their implemented commands.
The isolation tests also need Bubblewrap (`bwrap`), `unshare`, and permission to create user, mount, PID, and network namespaces.
The kernel must permit `close_range(CLOSE_RANGE_CLOEXEC)`. The launcher uses it
to prevent unrelated host descriptors from entering a guest. A denied call
fails the run.
They use installed Linux test programs. They do not boot miner firmware.

The runtime also has an explicit private Ethernet channel API. It creates its
own socket pairs, checks QEMU packet framing, and limits bytes and packets in
each direction. The caller must supply a bounded provider and record its
topology separately. A worker opens it only through a trusted, one-use binding
to the exact canonical process identity. Requests cannot install that binding.
Run its isolated process tests with
`cargo test -p md-sim-runtime --test private_channel --offline --locked -- --nocapture`.
These tests check full duplex traffic, host descriptor exclusion, packet
errors, backpressure, cancellation, and supervisor loss. They establish a
transport primitive. They do not establish firmware, mining, SSH, or dashboard
conformance. See the [private carrier report](work/simulator/reviews/private-channel-365.md).
The [worker binding report](work/simulator/reviews/private-channel-370.md)
records admission, result retrieval, token reuse, cancellation, and cleanup tests.
The actual worker executable is also tested with uploaded inputs and fresh
challenge frames. Run
`cargo test -p md-sim-coordinator --test worker_carrier --offline --locked -- --nocapture`.
The [CLI carrier report](work/simulator/reviews/actual-worker-carrier-379.md)
records both completed and cancelled isolated guest runs. Its peer is a local
test socket; this command does not establish support VM or firmware conformance.

Process specifications can set `"loopback": true` to enable local TCP inside
their private network namespace. The default leaves that interface down.
The setting is part of the run identity. It adds no host interface or route.
The worker must permit the required namespace and socket calls. A denied call
fails the run. The runtime does not use the host network as a fallback.

The worker limits saved input bytes to 16 GiB and 4,096 objects by default.
Each object can use at most 8 GiB. Partial transfers count toward both limits.
The store saves its policy and rejects a different policy on restart.
Use a new store for a different policy. Published inputs are never deleted by run cleanup.
The worker also limits active runs to two and retained runs to 1,024, with a 4 GiB evidence budget.
It reserves space for each active run's full trace before it starts the process.
Retrieve and acknowledge a completed or lost result before cleanup releases its space.
An abrupt worker loss ends the guest namespace. Restart retains a `lost` result and partial logs.
These behaviors were tested through private inherited IPC.
The [worker policy file](simulator/worker-limits.json) contains the default values.
`worker serve --limits FILE` accepts this format. Its Unix listener remains blocked in this session.
These storage limits are separate from per-process CPU and RAM limits.
They count file contents. Filesystem metadata uses additional space.
An optional `cgroup_parent` in the worker policy selects an existing, delegated
Linux cgroup-v2 directory. Its `cpu`, `memory`, and `pids` controllers must be
enabled. The worker creates a new child for each run. It limits total memory,
process count, and CPU usage across that run. It checks total CPU usage every
10 ms and limits CPU bandwidth to one core. Scheduling and cleanup can cause
small CPU budget overruns. The kernel enforces the memory bound directly.
`process execute` also accepts `--cgroup-parent PATH`. Without this option or
worker policy field, execution uses the stated per-process limits.

The [aggregate limit report](work/simulator/aggregate-limits/run-005/limits-report.json)
checks real CPU exhaustion, memory exhaustion, bounded process creation,
cancellation of four running child processes, and restart after an abrupt worker kill. The test ran in one
disconnected local VM. The host cgroup mount is read-only. Process creation
reached the per-user limit before the cgroup PID limit, so this result does
not establish that the cgroup PID limit triggered.
Recovery checks the saved group inode before it kills or removes the group.
The test also restarted the worker on its original Unix socket.
These are resource and recovery tests, not firmware tests.

The following commands ran that check. Use new output directories for another run.
The saved VM driver is a frozen build of `isolated_network_vm`; its source and
input identities are in the run directory.

```bash
cargo build -p md-sim-firmware --example aggregate_limits_check --release --offline --locked
target/release/examples/aggregate_limits_check prepare work/simulator/aggregate-limits/prepared-005
work/simulator/aggregate-limits/vm-driver-001 work/simulator/aggregate-limits/run-005 work/simulator/aggregate-limits/prepared-005/scenario.json
cargo build -p md-sim-firmware --example aggregate_limits_report --release --offline --locked
target/release/examples/aggregate_limits_report work/simulator/aggregate-limits/run-005 work/simulator/aggregate-limits/prepared-005 work/simulator/aggregate-limits/run-005/limits-report.json
```

The process contract now accepts an optional `stdin_object` with a saved SHA-256
identity and byte count. The supervisor sends these bytes through a pipe.
The guest does not receive the object store's file descriptor.
Worker `execute-token` and `lookup-token` requests use a caller-saved 64-character
lowercase hexadecimal token. A retry returns the original run. A different
process specification with the same token is rejected. Cleanup retains a small
receipt so a delayed retry cannot start the process again. Receipts count toward
the retained-run limit. Plan that limit before a large suite; cleanup releases
evidence bytes but does not release token slots. These behaviors have passed
private IPC and worker restart tests. They do not establish SSH transport.

```bash
target/release/md-sim doctor
target/release/md-sim catalog import --path .
target/release/md-sim targets list --device bitmain-antminer-s9
target/release/md-sim matrix plan --scope all-catalogued --pairing relevant --include-blocked --out work/simulator/plan-v11.json
target/release/md-sim matrix run work/simulator/plan-v11.json --workers local --suite conformance --require-complete --out work/simulator/matrix-v11
target/release/md-sim matrix report work/simulator/matrix-v11 --format html
```

`doctor` returns exit code 4 when a required host facility is unavailable.
This session permits isolated processes and private inherited interprocess communication (IPC).
It denies TCP socket creation, Unix socket binding, and standard socket operations.
The permitted IPC tests do not establish SSH, HTTP, TLS, or Bitcoin Core RPC support.
Use `doctor --bitcoin-core PATH` to identify a node binary outside `/usr/bin`.
This hashes the selected file. It does not execute it on the host or prove node acceptance.

`targets list` currently has no complete exact target to return.
`matrix run` retains all unresolved rows and returns exit code 3 with `--require-complete`.
It does not execute those rows or reuse source-experiment results as target passes.
Use a new result directory for each attempt. Prior results are retained.
The current matrix contains 4,354 entities, 13,506 artifact records, and 61,739 candidate relationship rows.
Classification is not firmware test coverage.
The [current strict matrix](work/simulator/matrix-v11/result.json) has zero passing exact targets.
The [CLI check record](work/simulator/reports/cli-v11.json) includes the expected incomplete result and rejection of a forged empty plan.

## Run the archived source experiment

This experiment rebuilds the archived `open-ckolivas-cgminer-source` artifact at commit
`b8491c66e7e22f23a9edf095dd1337ee581e88bd`.
It runs cgminer's own Stratum logic and S2 serial driver.
A Rust controller interface receives its work packets, restores the SHA-256 state, searches nonces, and returns actual results.
A separate hash implementation reconstructs and verifies the submitted headers.
Each run uses two fresh jobs, a clean-job transition, and a difficulty change.

The source fixture has eight chains with 32 chips per chain, as selected from the driver defaults.
This is **not a resolved physical S2 board**. It does not model the downstream FPGA or each ASIC bus.
The experiment is headless and has no native dashboard.
Its results do not pass an original firmware, original board, or exact-target gate.
The separate regtest mode has passed actual Core block import for this source fixture.

Preparation needs the archived source file, GCC, make, Autoconf, Automake, Libtool, pkg-config,
and the installed libcurl and Jansson development files.
The tested host has GCC 16.2.1, libcurl 8.22.0, and Jansson 2.15.1.
Preparation runs in a disposable namespace. It reads the archive without changing it.
The tested freezer below captures files from the Arch Linux package database.
It saves file bytes, permissions, links, package identities and hashes in a private tree.
Capture requires `/var/lib/pacman/local`; it is not a general Linux distribution installer.
The saved tree has 102 packages, 30,840 entries and 697,927,858 file bytes.
Build replay checks the complete saved tree and uses it as the only `/usr` input.
Both optimized and UBSAN builds pass the C regression and match the earlier firmware bytes exactly.
This establishes a frozen input set for this exact recipe. It does not establish package-source reproducibility.
Core and other firmware preparation paths retain their own toolchain limits.
Miner execution mounts only hashed executable and library files in a separate isolated environment.

```bash
cargo build -p md-sim-firmware --example freeze_cgminer_toolchain --offline --locked
target/debug/examples/freeze_cgminer_toolchain capture work/simulator/firmware/toolchain-snapshot-003
target/debug/examples/freeze_cgminer_toolchain build . work/simulator/firmware/toolchain-snapshot-003 work/simulator/firmware/prepared-closed-002
target/release/cgminer-lab run work/simulator/firmware/prepared-closed-002 work/simulator/firmware/v11-default
target/release/cgminer-lab run work/simulator/firmware/prepared-closed-002 work/simulator/firmware/v11-low-cap 1000
```

These exact commands refer to retained results. Use new output directories to repeat them.
For a UBSAN preparation, add `--ubsan` to the freezer's build command and use a new output directory.
The [toolchain manifest](work/simulator/firmware/toolchain-snapshot-003/manifest.json) pins the captured inputs.
The [closure report](work/simulator/firmware/toolchain-closure-report.json) records both rebuilds and distinguishes their build environment from the earlier adaptation baseline.
The older `cgminer-lab prepare` command still uses installed read-only build dependencies. It has no frozen toolchain claim.
The default aggregate cap is 10,000 hashes per second. The optional third run argument sets a lower cap.
The optional fourth argument sets the recorded job seed. Omit it to obtain fresh entropy.
The next two arguments set the epoch and first ASIC difficulty exponent.
The pool difficulty is 1.5 times the programmed ASIC difficulty.
Both expected intervals and mining deadlines account for these separate targets.
Lower caps lower the default difficulty. Work timestamps follow the mining budget.
The run saves input identities, guest logs, device packets, pool messages, headers, and verifier results.
A low-rate adaptation changes the guest's difficulty-one prefilter and its controller filter encoding.
A separate transport adaptation carries the original Stratum messages through inherited private IPC.
Queue replacement, partial writes, duplicate handling, and original C memory errors have recorded patches.
Read [the adaptation manifest](simulator/firmware/cgminer-source-adaptation.json) and
[the source patch](simulator/firmware/cgminer-low-rate.patch) before interpreting the results.
The original source archive remains unchanged.

The current [source execution report](work/simulator/firmware-progress.md) has five passing fixture runs.
They cover fresh jobs, the default cap, a 1,000 H/s cap, and a 100 H/s nonce-zero regression.
The full firmware also runs under the C undefined behavior sanitizer (UBSAN).
Every preparation runs a C regression against the exact patched helper header.

## Verify a firmware-produced regtest block

`run-regtest` uses the regtest genesis parent and a valid height-one coinbase.
It exports only blocks reconstructed from actual accepted firmware submissions.
The independent node was rebuilt from archived Bitcoin Core commit
`619334569b44b4278d0155bb8daf4c8535a47367`. The original Core source is unchanged.
Its [backend record](simulator/backends/bitcoin-core-619334569b44.json) pins the source, binary and preparation evidence.
Read the [Core execution report](work/simulator/bitcoin-core-progress.md) for build prerequisites, exact commands, retained failures and limits.

These commands were run:

```bash
target/release/cgminer-lab run-regtest work/simulator/firmware/prepared-closed-002 work/simulator/firmware/v11-regtest
cargo run -p md-sim-firmware --example node_prerequisite_probe --locked --offline -- work/simulator/bitcoin-core/source-build-002/bitcoind work/simulator/bitcoin-core/v11-regtest-candidate-0 work/simulator/firmware/v11-regtest/regtest-blocks.json 0
```

Use new output directories to repeat the test. The final argument selects the actual exported candidate.
Both fresh candidates from the optimized run and both from the UBSAN run passed Core import.
Each node test initializes genesis, restarts the unchanged node in the same private storage,
imports the selected block, and checks the exact block hash and height change from zero to one.
It then restarts Core again without an import argument and verifies the saved tip at height one.
No node-generated block is used. This transport uses a block file and has no RPC or network listener.
The [first acceptance report](work/simulator/bitcoin-core/v11-regtest-candidate-0/block-acceptance.json)
links the input identities, firmware submission, header, block and actual node logs.
This is source-fixture acceptance. Every exact mining target still needs its own test.

A separate [S9 diagnostic](work/simulator/original-board-progress.md) executes original archived kernel and userspace code.
It reaches init, procd, ubus and module loading. It uses one CPU, direct kernel entry and an explicit bypass of a missing FPGA I²C interface.
The [SD boot investigation](work/simulator/s9-boot-progress.md) now executes the archived SPL loader and U-Boot from the exact full SD image.
NAND, FPGA and reset models still prevent a complete board boot and miner test.
This diagnostic is not an original-board pass.

The [SPL contract inspector](work/simulator/reviews/s9-spl-extractor-executed-454.md)
checks the exact archived boot image and its loader suffix in isolation.
It decodes all 15 revision tables, the 100,000,001-read poll budget and the
wrapping timer calculation. This is static firmware evidence. It does not
execute ARM instructions or measure physical timing. These commands passed;
use a new output directory for another run:

```sh
CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=work/simulator/check-target-085 TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 cargo build -p md-sim-firmware --example inspect_s9_spl --release --offline --locked -j 1
TMPDIR=/home/user/src/md-sim/work/simulator/test-tmp-227 work/simulator/check-target-085/release/examples/inspect_s9_spl work/simulator/original-zynq/sd-bundle-291/boot/boot.bin work/simulator/original-zynq/sd-bundle-291/boot/spl-payload.bin work/simulator/original-zynq/spl-contract-454
```

The [original ROM read test](work/simulator/reviews/zynq-rom-read-path-executed-513b.md)
now passes 28 EOF and error cases in isolated QEMU ARM. It uses an unchanged
public Zynq ROM and the exact saved SD sector. The read routine stops at the
logical file end and leaves the remaining destination bytes unchanged.
The separate [ROM mapping and reset tests](work/simulator/reviews/zynq-rom-reset-executed-513e.md)
pass complete ROM protection, all memory-bank mappings and two resets per case.
The later [reset diagnostic audit](work/simulator/reviews/zynq-rom-reset-independent-execution-513g.json)
matches actual translated bytes to the original ROM and records its first
unsupported SLCR write. The later reset-reason and voltage-control models
pass five component suites. The unchanged ROM now stops at the unimplemented
PS GPIO interrupt-disable register. The later Rust GPIO model now passes144
transactions,14 interrupt transitions and two resets. Its rejection suite
passes nine faults. The unchanged ROM advances to unsupported SLCR0x500.
See the [GPIO execution report](work/simulator/reviews/ps-gpio-executed-513j.md).
The complete firmware boot remains open.
The reports include the tested commands and complete input hashes. The C
fixture supplies the ARM32 ABI and exact memory layout; Rust controls the
test and checks its results. Full reset execution and the ROM's match to
the selected S9 silicon remain open.

The [normal SD console run](work/simulator/qemu-s9/legacy-s9io-console-001/console.json)
also executes the archived BOSminer and BOSer web service.
The miner detects no hashboards. The guest's current FPGA address map differs
from the legacy S9IO development machine. This is a recorded implementation
gap, not a mining pass. The shared serial driver also passes a
[guest reboot and configuration restoration test](work/simulator/original-zynq/storage-warm-shared-001/result.json)
and rejects a [guest script failure](work/simulator/original-zynq/serial-failure-shared-001/result.json).
The following commands ran the normal-console diagnostic. Use a new output
directory for another run:

```sh
CARGO_INCREMENTAL=0 cargo build -p md-sim-firmware --example s9_boot_bundle --example s9_native_service --release --offline --locked
target/release/examples/s9_boot_bundle boot work/simulator/original-zynq/sd-bundle-005 work/simulator/qemu-s9/legacy-s9io-console-001 spl legacy-io backend=/tmp/md-sim-qemu-backend-037-ubsan budget=120 console=work/simulator/qemu-s9/console-inspect-001.sh
```

The input manifest identifies the original SD image, SPL suffix, backend,
libraries and console script. The script uses the original console and init.
The runner stops its own emulator after the script completes.
Script completion alone does not establish service health.

The [2022 layout diagnostic](work/simulator/qemu-s9/sd2022-presence-console-004/process-spec.json)
uses the guest's address map and explicit presence-pin inputs. The guest reads
all three asserted pins. BOSminer then reads the controller version and stops:
it requires S9IO 1.0.2, while the implemented legacy core reports 0.2.0.
The [version evidence](work/simulator/s9io-102-evidence-001.json) records this
failure and the selected FPGA image's exact identity. This is required model
work. It is not an incompatible-target exclusion or a mining pass.

```sh
target/release/examples/s9_boot_bundle boot work/simulator/original-zynq/sd-bundle-005 work/simulator/qemu-s9/sd2022-presence-console-004 spl sd2022-io backend=/tmp/md-sim-qemu-backend-040-ubsan budget=120 presence-pin-fixture=e0 console=work/simulator/qemu-s9/console-presence-005.sh
```

The serial driver now sends each script and its completion check as one shell
group. This prevents a running script from blocking delivery of its completion
command. The [serial review](work/simulator/serial-console-review-002.json)
links the guest failure-status and restart-persistence regressions.
The transport reconstructs script bytes from printable ASCII inside the guest.
This preserves literal tabs, which interactive BusyBox otherwise treats as
completion keys. Run 020 verifies a literal tab in the archived guest.
The component regression also checks quotes, control bytes, UTF-8, long
scripts, and failure status:

```bash
CARGO_INCREMENTAL=0 cargo test --release --offline --locked -p md-sim-firmware --lib serial_console::tests
```

SD preparation checks the exact archive identity before extraction. It also
checks the Zynq header checksum, both U-Boot checksums, the gzip checksum,
and the FPGA container length. The derived FPGA image has its own hash.
The [2022 preparation assessment](work/simulator/original-zynq/sd-bundle-006/fpga-assessment.json)
compares it with a separate isolated gzip extraction.
The following preparation commands also ran successfully:

```sh
target/release/examples/s9_boot_bundle prepare work/simulator/original-zynq/sd-bundle-006
target/release/examples/s9_boot_bundle inspect work/simulator/original-zynq/sd2020-inspection-001 sd2020
target/release/examples/s9_boot_bundle prepare work/simulator/original-zynq/sd2020-bundle-001 sd2020
```

The [2020 bundle](work/simulator/original-zynq/sd2020-bundle-001/boot/bundle.json)
uses archived image `a195af100de6200be308f016bf786d32c77df7e0094875ce882545849c49a574`.
Its boot components differ from the 2022 bundle. Preparation does not establish
FPGA model compatibility, firmware boot, or mining. The boot command selects
the exact input from the bundle and rejects conflicting image or stage metadata.
Use new output directories when you repeat these commands.

The [2020 firmware run](work/simulator/qemu-s9/sd2020-console-001/guest.log)
executes its original SPL, U-Boot and Linux. It then panics during a NAND
subpage write while attaching the empty development fixture. This is a failed
boot. The [preparation review](work/simulator/s9-multi-image-review-001.json)
records the exact inputs, checks and remaining NAND work.

A separate [S9IO 1.0.2 reference model](simulator/backends/qemu/S9IO-V1.md)
implements the documented 200 MHz serial clock and enable-low reset signal.
It stops on unsupported ASIC modes and the documented destructive
reconfiguration. Its shared FIFO internals remain an unvalidated inference
from the legacy hardware source. It is not an exact FPGA or miner pass.
The [compiled interface test](work/simulator/qemu-s9/reference-v1-qtest-001-ubsan/assessment.json)
checks serial rates, registers and interrupt routes. This command passed:

```sh
target/release/examples/s9_backend_qtest /tmp/md-sim-qemu-backend-042-ubsan work/simulator/qemu-s9/reference-v1-qtest-001-ubsan s9io-v1
```

The [fan component](simulator/backends/qemu/FAN.md) implements PWM registers,
motor motion, tachometer sampling, stall, power loss and reset. Its motor
parameters and sampling period are explicit development assumptions.
These commands passed:

```sh
cargo test -p md-sim-firmware --test fan_controller --offline --locked
target/release/examples/s9_backend_qtest /tmp/md-sim-qemu-backend-044-ubsan work/simulator/qemu-s9/fan-qtest-001-ubsan fan
target/release/examples/s9_backend_qtest /tmp/md-sim-qemu-backend-044-ubsan work/simulator/qemu-s9/fan-empty-qtest-001-ubsan fan-empty
target/release/examples/s9_boot_bundle boot work/simulator/original-zynq/sd-bundle-006 work/simulator/qemu-s9/sd2022-fan-console-001 spl sd2022-v1 backend=/tmp/md-sim-qemu-backend-044-ubsan budget=240 presence-pin-fixture=e0 fan-fixture console=work/simulator/qemu-s9/console-fan-check-001.sh
```

The last command completes a console diagnostic. The
[actual firmware trace](work/simulator/qemu-s9/sd2022-fan-console-001/assessment.json)
shows native BOSminer reading the fan speeds and writing both PWM channels.
It stays running, but all three hashchains fail because the board power
controllers are not connected. No mining or exact-target gate passes.
The explicit `fan-fixture` option selects uncalibrated motors.
The boot wall budget can be 30..240 seconds. Its CPU ceiling remains 110 seconds.
Live guest and backend logs are retained within the run's output budget.

The [CPU timer clock patch](simulator/backends/qemu/CPU-TIMERS.md) connects
the existing QEMU global and private timers to the guest's Zynq clock
controls. The compiled regression passes 100 transactions under the
undefined-behavior sanitizer. It fixes the previous fixed 100 MHz rate
and disabled-counter advancement. Physical clock calibration remains open.
These build and test commands were exercised:

```bash
CARGO_INCREMENTAL=0 cargo build --release --offline --locked -p md-sim-firmware --example checkpoint_qemu_core --example build_qemu_backend --example s9_backend_qtest
target/release/examples/checkpoint_qemu_core work/simulator/qemu-s9/core-sources-069
target/release/examples/build_qemu_backend /home/user/src/md-sim work/simulator/qemu-s9/samurai-build-003 /tmp/md-sim-qemu-backend-069-ubsan /tmp/md-sim-qemu-backend-031 ubsan core=work/simulator/qemu-s9/core-sources-069
target/release/examples/s9_backend_qtest /tmp/md-sim-qemu-backend-069-ubsan work/simulator/qemu-s9/cpu-timer-qtest-backend-069 cpu-timers
```

These development commands use the captured build inputs in this workspace.
Choose unused output directories for another run. They do not establish
clean installation or complete board acceptance.

The build requires `core=CHECKPOINT_DIRECTORY`. It verifies the model library
against that checkpoint before and during the isolated build. It no longer
selects a library from the mutable staging path. Build 068 exposed that path's
stale-library defect and failed at link time. Build 069 passes with the
explicit checkpoint. See [the retained build report](work/simulator/qemu-s9/backend-build-069-ubsan/build-report.json).

The [PIC component](simulator/backends/qemu/PIC16.md) implements the archived
power controller command contract. It checks the supplied application hash,
keeps separate flash and voltage state for each board, and handles busy
periods, reset, interrupted writes and heartbeat power cutoff. Its
[archived instruction analysis](simulator/firmware/pic16/heartbeat-analysis.md)
shows that disabled power pauses the software heartbeat counter. Expiry
cuts power while the application remains available. The declared10-second
timing estimate is unchanged and remains uncalibrated. It uses explicitly
erased development storage outside the application. It does not execute
PIC instructions or provide a factory calibration dump.
These component and compiled bus commands passed:

```sh
cargo test -p md-sim-firmware --test pic16_controller --offline --locked
CARGO_INCREMENTAL=0 cargo test --offline --manifest-path simulator/backends/qemu/axi-iic/Cargo.toml --features host-std --lib
target/release/examples/s9_backend_qtest work/simulator/qemu-s9/backend-078-ubsan work/simulator/qemu-s9/pic16-heartbeat-qtest-078 pic16
```

The current [compiled PIC regression](work/simulator/qemu-s9/pic16-heartbeat-qtest-078/assessment.json)
passes579 responses and seven exact power-state timestamps under UBSAN.
The same test [rejects backend077](work/simulator/qemu-s9/pic16-heartbeat-before-077/assessment.json),
which incorrectly stopped the emulator after a heartbeat deadline while power
was off. The [connected ASIC regression](work/simulator/qemu-s9/bm1387-heartbeat-qtest-078/assessment.json)
also passes with backend078. These remain component results.

The [compiled bus report](work/simulator/qemu-s9/pic16-qtest-002-ubsan/assessment.json)
checks all three controller addresses, separate flash pointers, application
words, voltage readback and power enable output. It also rejects empty FIFO
reads that could otherwise look like zero-valued responses.
The test found and fixed an extra receive-count delay in the AXI IIC model.
The count is internal control data and consumes no additional wire byte.
These results do not establish board power delivery or mining acceptance.

The [BM1387 development chain](simulator/backends/qemu/BM1387.md) connects
three 63-chip serial peers to the PIC power outputs and GPIO reset pins.
Its compiled test passes 859 register responses. It checks actual serial
replies, FIFO readback, power loss and reset:

```bash
target/release/examples/s9_backend_qtest /tmp/md-sim-qemu-backend-054-ubsan work/simulator/qemu-s9/bm1387-qtest-004-ubsan bm1387
```

Use a new output directory for another run. This is a component test.
The first firmware run exposed a clock-read defect in the C binding.
The fix passes that point and firmware initializes all three power
controllers. Build 056 completes setup on all three chains and receives
the guest's first core-startup work packet. It stops on its explicit
unimplemented-work error. The newer Rust integration passes fresh-work,
serial, rate-limit, replacement and reset component tests:

```bash
CARGO_INCREMENTAL=0 cargo test -p md-sim-firmware --test bm1387_mining --offline --locked
```

The serial wire model also saves and restores its device state. This includes
partial frames, line levels, deadlines and queued bytes. It rejects an unknown
state version, an impossible state, or a deadline that overflows the clock.
The [state review](work/simulator/reviews/serial-state-256.md) records the limits.
This tested command checks replay against byte and timing expectations:

```bash
CARGO_INCREMENTAL=0 cargo test -p md-sim-firmware --test serial8n1_wire --offline --locked
```

A complete board snapshot must also preserve the peer devices, CPUs and
network sessions at the same instant. That capability remains incomplete.

Build 059 executes the archived 2022 firmware and computes hashes from
its core-startup work. An independent SHA-256 implementation checks 23,612
attempts from 342 jobs across all three chains in the
[retained trace](work/simulator/qemu-s9/sd2022-bm1387-console-013/asic-hashes.json).
That run timed out after a PIC watchdog stop. Its trace is complete. None of these attempts
passed the native ASIC filter. Startup work does not satisfy fresh pool
mining. The source-derived core address map remains unvalidated for an
exact silicon stepping. This path has no mining or exact-board pass.

The following command audits the retained work and computed nonce events.
Use a new report path for another audit:

```bash
target/release/examples/s9_asic_audit hashes work/simulator/qemu-s9/sd2022-bm1387-console-013 work/simulator/qemu-s9/sd2022-bm1387-console-013/asic-hashes.json
```

The boot diagnostic accepts `asic-events` with `asic-contract` to omit
repeated FPGA register reads and writes from its log. It retains ASIC and
serial state events. The adaptation manifest records this reduced trace.
It does not provide a complete bus trace.
The `asic-packets` diagnostic retains receive packets and computed hashes
while omitting duplicate bytes, pin edges and FPGA register traffic.
It requires a backend with the recorded compact-trace contract. The
[BM1387 contract](simulator/backends/qemu/BM1387.md) defines the retained
events and the limits of this trace.

The [private pool development path](simulator/backends/qemu/PRIVATE-POOL.md)
connects the archived guest's TCP client to the Rust verifier. It uses
a separate Linux pool VM and the pinned QEMU private Ethernet adapter.
It has no host listener. Each TCP connection has its own verifier session.
The protocol tests cover BIP 310 version negotiation, fresh jobs, changed
difficulty, rejected submissions and cancellation:

```bash
CARGO_INCREMENTAL=0 cargo test --release --offline --locked -p md-sim-mining --lib --test stratum
```

The first guest connection exposed a missing version-rolling extension.
The extension now passes component tests. The
[firmware run](work/simulator/qemu-s9/sd2022-private-pool-022/pool-result.json)
negotiates it, subscribes, authorizes and receives a fresh job.
The independent [full-header audit](work/simulator/qemu-s9/sd2022-private-pool-022/pool-work-audit.json)
matches 34,352 computed hashes to actual fresh pool work. The trace is
complete. No hash passes the native ASIC filter. A power-controller
watchdog stops this run, and the firmware submits no share.
The [100 MHz diagnostic](work/simulator/qemu-s9/sd2022-private-pool-023/result.json)
reaches the unimplemented temperature-sensor pin route.
Both runs remain failed, and neither grants an exact-target pass.

This command reconstructs full headers from the retained pool negotiation
and actual firmware work. It checks the complete midstate and header tail:

```bash
target/release/examples/s9_asic_audit pool-work work/simulator/qemu-s9/sd2022-private-pool-022 work/simulator/qemu-s9/sd2022-private-pool-022/pool-work-audit.json
```

Use a new report path for another audit. This audit does not submit shares.
Its complete trace exposed a bus-idle interrupt defect. The
[AXI IIC contract note](simulator/backends/qemu/AXI-IIC.md) records the
cause, fix and timing limits. Firmware mining acceptance is still incomplete.

The later [original firmware run 028](work/simulator/qemu-s9/sd2022-private-pool-028/result.json)
finishes its console script. The archived SPL suffix, U-Boot, kernel and
bosminer execute with the recorded device adaptations. The
[submission audit](work/simulator/qemu-s9/sd2022-private-pool-028/submission-audit-002.json)
checks 233,825 computed hashes and 211 complete ASIC serial results.
Two fresh jobs produce pool-accepted submissions. A third submission is
correctly rejected for low difficulty. Each submitted header matches real
computed work and a transmitted hardware reply. The audit checks the clean
job transition and the difficulty change. It does not infer timing between
the separate ASIC and pool clocks.

This is an adapted reference environment. It uses three chains with 63 chips
each, a 10,000 H/s aggregate cap, fixed-filter adaptation to four bits,
100 MHz guest configuration, and explicit temperature, fan and presence
fixtures. The original firmware bytes are unchanged. Exact physical board
identity, BootROM execution, calibration and the full target suite remain open.
Run 028 uses random-parent jobs and is not Bitcoin Core acceptance evidence.

These commands were tested. Use new output paths to repeat them:

```sh
target/release/examples/s9_boot_bundle boot \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/sd2022-private-pool-028 spl sd2022-v1 \
  backend=/tmp/md-sim-qemu-backend-066-ubsan budget=240 \
  presence-pin-fixture=e0 fan-fixture pic-contract asic-contract asic-packets \
  local-pool pool-vm=/tmp/md-sim-qemu-pool-003-ubsan temperature-fixture \
  adapt-fixed-filter-bits=4 pool-difficulty=-28,-27 \
  console=work/simulator/qemu-s9/console-private-pool-014.sh
target/release/examples/s9_asic_audit submissions \
  work/simulator/qemu-s9/sd2022-private-pool-028 \
  work/simulator/qemu-s9/sd2022-private-pool-028/submission-audit-002.json
```

The [ARM build record](work/simulator/qemu-s9/backend-build-066-ubsan/build-report.json)
and [pool build record](work/simulator/qemu-s9/pool-build-003-ubsan/build-report.json)
pin the backend binaries and input sources. Both use undefined-behavior checks.
The [private Ethernet notes](simulator/backends/qemu/PRIVATE-STREAM.md) give the
separate transport test and its actual local VM topology.

[Run 030](work/simulator/qemu-s9/sd2022-private-pool-030/result.json)
adds the explicit `regtest-genesis` pool input. It uses the same backend and
device adaptations as run 028. Its wall budget is 360 seconds; its CPU and
pool limits remain 230 seconds. It finishes in 237.082 seconds. The
[hardware and block audit](work/simulator/qemu-s9/sd2022-private-pool-030/regtest-audit-001.json)
checks 278,156 computed hashes, 275 complete ASIC responses, and two fresh
firmware-submitted block candidates. The original Bitcoin Core binary
accepts [candidate 0](work/simulator/bitcoin-core/original-s9-030-candidate-0/block-acceptance.json)
and [candidate 1](work/simulator/bitcoin-core/original-s9-030-candidate-1/block-acceptance.json)
in separate isolated chain states. Each advances from height zero to one.
Each exact block remains the chain tip after a separate Core restart.
Core does not generate either block.

The following audit and node commands were tested. The node probe repeats
the hardware/submission audit before it imports a block. Use new output
paths for another verification:

```sh
target/release/examples/s9_asic_audit regtest \
  work/simulator/qemu-s9/sd2022-private-pool-030 \
  work/simulator/qemu-s9/sd2022-private-pool-030/regtest-audit-001.json
target/release/examples/node_prerequisite_probe \
  work/simulator/bitcoin-core/source-build-002/bitcoind \
  work/simulator/bitcoin-core/original-s9-030-candidate-0 \
  work/simulator/qemu-s9/sd2022-private-pool-030/regtest-audit-001.json 0
target/release/examples/node_prerequisite_probe \
  work/simulator/bitcoin-core/source-build-002/bitcoind \
  work/simulator/bitcoin-core/original-s9-030-candidate-1 \
  work/simulator/qemu-s9/sd2022-private-pool-030/regtest-audit-001.json 1
```

These results establish original archived firmware mining in the declared
adapted reference environment. They do not establish an exact physical S9
target, the original-board gate, measured calibration, or full-matrix coverage.

The [original boot SD test](simulator/backends/qemu/SD-STORAGE.md) now checks
native configuration changes across three emulator starts. It exports and
reloads exact disk snapshots. A separate filesystem reader verifies the
changed and restored files. The test exposed and fixed lost writes caused
by QEMU's default SD write-back cache. This is SD persistence evidence only;
NAND, full-board state, update/recovery and physical calibration remain open.
The independent disk reader now also inventories all saved overlay entries
and hashes each regular file. It records links and deleted-file markers
separately. Extended file attributes remain outside this inspection.

[Run 033](work/simulator/qemu-s9/sd2022-private-pool-033/result.json) combines
original firmware mining with the corrected SD cache and snapshot export.
It completes without trace loss. Its
[independent audit](work/simulator/qemu-s9/sd2022-private-pool-033/regtest-audit-001.json)
checks 267,118 computed hashes and 249 complete ASIC responses.
Core accepts both
[first](work/simulator/bitcoin-core/original-s9-033-candidate-0/block-acceptance.json)
and [second](work/simulator/bitcoin-core/original-s9-033-candidate-1/block-acceptance.json)
fresh candidates and retains each after restart. The runtime now shares
its bounded trace allowance across stdout and stderr. Failed runs 031 and
032 remain recorded; a CPU limit and then trace truncation prevented their
combined tests from passing.

The [original Boser service probe](simulator/backends/qemu/NATIVE-BOSER.md)
now checks the native page, exact application asset, login, session state,
advancing uptime, configuration write/readback and restoration from a separate
local VM. Run040 passes these checks with the original firmware. Restart
tests exposed native network resets when the reference environment generated
a new MAC address. The driver now provisions a recorded local MAC through the
guest's SD boot configuration. Cycle006 passes all three linked boots: write,
restore and verify. Each boot also produces two blocks accepted by Bitcoin
Core and retained after its restart. The S9IO partial-command regression
exposed and fixed excessive clock polling. The exact stalled state from the
earlier intermittent failure was not captured. The earlier035 fan error
also remains unresolved. The
[original browser run048](work/simulator/qemu-s9/original-browser-048/browser-result.json)
passes native login, an authenticated session check, exact application bytes,
advancing HTTP uptime and two distinct WebSocket samples. The actual Firefox
client runs in the private support VM. Its
[mining audit](work/simulator/qemu-s9/original-browser-048/regtest-audit-001.json)
checks 102,121 computed hashes and 117 complete ASIC responses. Core accepts
both fresh-job blocks and retains them after restart:
[first](work/simulator/bitcoin-core/original-s9-048-candidate-0/block-acceptance.json)
and [second](work/simulator/bitcoin-core/original-s9-048-candidate-1/block-acceptance.json).
Run048 has a miner/browser wall-clock mismatch. The
[explicit clock test049](work/simulator/qemu-s9/original-browser-049/browser-result.json)
corrects wall time through the original guest's date command and checks native
timestamps against browser request times. It also passes fresh-work auditing
and both Core acceptance tests. Device and monotonic clocks stay unchanged.
The [expanded browser test057](work/simulator/qemu-s9/original-browser-057/browser-result.json)
also checks live data for three boards with 63 chips each, temperatures,
fan channels and the selected pool. A second fresh profile cannot use the
first profile's authenticated session at the same origin. Both profiles are
stopped and their process groups are empty. The
[hardware audit](work/simulator/qemu-s9/original-browser-057/regtest-audit-001.json)
checks 42,154 computed hashes, 54 ASIC replies and two fresh jobs.
Core accepts both blocks and retains them after restart:
[first](work/simulator/bitcoin-core/original-s9-057-candidate-0-retry-001/block-acceptance.json)
and [second](work/simulator/bitcoin-core/original-s9-057-candidate-1-retry-001/block-acceptance.json).
The [LuCI test059](work/simulator/qemu-s9/original-luci-059/dashboard-result.json)
passes native backup download, wrong-token and wrong-method rejection, and
invalid firmware upload rejection. Its two fresh-job blocks also pass Core
acceptance and restart checks. Update installation, recovery, SSH, TLS and the
complete dashboard gate remain open for this path.
See the [tested command and limits](simulator/backends/qemu/NATIVE-BOSER.md).

[Adapted run075](work/simulator/qemu-s9/sd2022-private-pool-075/dashboard-result.json)
passes native stop, validation of the complete signed SD update, cancellation
and restart. The native validator returns the exact image SHA-256 in39.6s.
Both mining sessions pass independent hardware/header verification; four
fresh blocks pass Core acceptance and restart checks. This run uses the
separate fan-monitor source patch. The [evidence and command](simulator/backends/qemu/NATIVE-BOSER.md)
record that adaptation. No update was installed in075. Browser run076 with
backend078 failed its second-profile check on an empty HTTP408 response;
the packet capture and failure remain retained. Browser run081 passes with
the client fix. Its second profile did not need the permitted HTTP408 retry.

[Install run077](work/simulator/qemu-s9/native-update-cycle-001/install-077/dashboard-result.json)
then runs the native signed-image installer with `keep=1`. The archived
updater writes the four boot files and reaches a kernel restart. QEMU exits
with status0. The [saved disk inspection](work/simulator/qemu-s9/native-update-cycle-001/install-077-inspection/inspection.json)
confirms the expected boot-file hashes and the native settings archive.
The temporary monitor patch is absent from the new overlay. Both fresh
mining blocks from before installation pass Core acceptance and restart
checks. The next boot restored the mining and network configuration, but its
theme persistence test failed. The native default backup omits the theme file.
Run078 remains failed. With the explicit native keep-file configuration,
[run080](work/simulator/qemu-s9/native-update-cycle-002/install-080/dashboard-result.json)
passes backup verification, signed installation and native restart. Its
[disk inspection](work/simulator/qemu-s9/native-update-cycle-002/install-080-inspection/inspection.json)
also passes. [Post-update run081](work/simulator/qemu-s9/native-update-cycle-002/post-update-browser-081/dashboard-result.json)
reads the saved theme, restores its original value and passes the native
browser checks. Its two fresh blocks pass Core acceptance and restart checks.
[Run082](work/simulator/qemu-s9/native-update-cycle-002/restored-settings-082/dashboard-result.json)
boots the resulting disk and confirms the restored settings. The separate
run081 disk inspector fails on an ext4 header warning. The explicit native
reboot in run084 produces a new disk that passes the earlier file inspector.
The stronger filesystem check finds its journal-recovery flag still set.
An explicit journal replay on a disposable inspection copy passes the full
read-only check and file inspection. It does not prove a clean raw snapshot.
The automatic cycle's provisioning disk still fails after journal replay.
These failures remain retained. See the [active record](work/simulator/continuation-2026-10-21.md).
This is not a complete update gate.
See the [tested installation command](simulator/backends/qemu/NATIVE-BOSER.md).
The [pool response review](simulator/protocols/stratum-v1-response-review.md)
records the versioned rejection-format fix and its tests. Fresh firmware
run083 passes with the new contract; its two blocks pass Core acceptance and
restart checks. These checks do not establish full Stratum protocol conformance.

[Automatic cycle004](work/simulator/qemu-s9/automated-update-cycle-004/assessment.json)
passes four linked firmware boots. It provisions a private MAC, writes DARK,
installs the signed update, restores the original unset theme, and checks that
restoration after another restart. The native browser checks also pass.
All four disk inspections and eight Core block checks pass. Normal phases use
an explicit native read-only storage checkpoint before reboot. The installer
uses its own restart. See the
[tested cycle command and limits](simulator/backends/qemu/NATIVE-BOSER.md#automatic-signed-update-cycle).
These results do not close the exact physical board or full recovery gates.

The corrected process isolation and reproducible backend were then tested in
[cycle132](work/simulator/qemu-s9/automated-update-cycle-proc-132/assessment.json).
This run repeats all four boots, disk checks, browser checks and eight Core
checks. It uses LIGHT as the changed theme. This command passed:

```bash
work/simulator/ssh-target-132/release/examples/s9_dashboard_cycle \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/backend-recipe-130d \
  /tmp/md-sim-qemu-pool-003-ubsan \
  work/simulator/qemu-s9/automated-update-cycle-proc-132 \
  signed-update=work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin \
  browser-payload=work/simulator/network-vm/native-driver-024/payload.json
```

Use a new output directory to repeat it. The
[source checkpoint](work/simulator/qemu-s9/core-checkpoint-132/manifest.json)
and the run's command records identify the tested helpers and inputs.
Earlier cycle004 results do not establish isolation with the corrected launcher.

[Run087](work/simulator/qemu-s9/native-update-rejection-087/dashboard-result.json)
tests a damaged copy of that signed update. The isolated preparation changes
one FIT payload bit and preserves every other archive byte. Native LuCI rejects
the complete upload without offering installation. Mining then restarts.
Independent audits check both mining sessions. All four resulting blocks pass
Bitcoin Core acceptance and restart checks. See the
[input, command and evidence](simulator/backends/qemu/NATIVE-BOSER.md#damaged-signed-update).
This is one negative update test in the adapted reference environment.
The complete update and recovery gates remain open.

The same prepared board process also runs through the common local executor
and a separate local IPC worker. [Local088](work/simulator/qemu-s9/worker-reference-local-088/transport-assessment.json)
and [worker089](work/simulator/qemu-s9/worker-reference-ipc-089/transport-assessment.json)
pass native damaged-image rejection and independently verified mining before
and after restart. Each has four Core acceptance and persistence checks.
The worker verifies52 transferred objects, returns the complete result and
acknowledges cleanup. Its published input cache remains available for reuse.
See the [tested commands and topology](simulator/backends/qemu/NATIVE-BOSER.md#prepared-local-and-worker-execution).
This IPC test does not establish SSH, TLS or an external physical worker.
The later [worker094 run](work/simulator/qemu-s9/worker-reference-ipc-094/transport-assessment.json)
uses a fresh seed and passes four independent Core acceptance and restart checks.
The [cancellation test](work/simulator/qemu-s9/worker-reference-cancel-092/transport-assessment.json)
stops the selected firmware worker, retrieves its cancelled result and verifies cleanup.
The linked command guide records both tests and the separate SSH transport checks.
The [SSH123 component test](work/simulator/network-vm/ssh-live-123/remote-report.json)
also checks live coordinator logs before cancellation. The coordinator compares
the saved bytes with the complete verified result before it acknowledges cleanup.
This test uses separate client and worker namespaces in one disconnected local VM.
The [SSH204 reconnect test](work/simulator/network-vm/ssh-live-reconnect-204/remote-report.json)
closes a real SSH channel after each reply. Four reads of the selected live run
require three reconnects. Both log streams match the verified final result.
The test also checks the same worker build, tools and policy, artifact transfer,
status, stop, result retrieval, cleanup and the TLS component path. Interrupted
mutations are not replayed. These commands passed:

```sh
work/simulator/ssh-target-204/release/examples/isolated_network_vm work/simulator/network-vm/ssh-live-reconnect-204 work/simulator/network-vm/scenarios/ssh-live-reconnect-204.json
work/simulator/ssh-target-204/release/examples/remote_namespace_check report work/simulator/network-vm/ssh-live-reconnect-204
```

Use a new output directory to repeat the first command. Its scenario pins the
inputs. This is a transport test in a local VM, not a native miner dashboard test.
Archived board execution over SSH is still under test. Its failed attempts remain
in the [progress record](work/simulator/continuation-2026-10-21.md).

The [SSH209 artifact recovery test](work/simulator/network-vm/artifact-ack-loss-209/remote-report.json)
closes the channel after the worker accepts a chunk or publishes an object,
before the client receives its acknowledgement. The coordinator reports the
interrupted operation. A new connection reads the saved offset, resumes the
production transfer path, and checks the destination hash. Both retrieved
objects match their fresh input bytes. The local VM has separate client and
worker namespaces. These commands passed:

```sh
work/simulator/ssh-target-209/release/examples/isolated_network_vm work/simulator/network-vm/artifact-ack-loss-209 work/simulator/network-vm/scenarios/artifact-ack-loss-209.json
work/simulator/ssh-target-209/release/examples/remote_namespace_check report work/simulator/network-vm/artifact-ack-loss-209
```

Use a new output path to repeat the first command. This checks transport
recovery. It does not establish a native dashboard or exact hardware result.

Inspect the signed SD update before an installation test. Use a new output
directory for each invocation. These commands were tested:

```sh
cargo build --release --offline --locked -p md-sim-firmware --example s9_update_bundle
target/release/examples/s9_update_bundle work/simulator/original-zynq/sd-bundle-006/boot/bundle.json work/simulator/native-s9/signed-sd-update-002
```

The [inspection](work/simulator/native-s9/signed-sd-update-002/inspection.json)
hashes all six regular members inside the preparation namespace. Its SPL,
U-Boot, FPGA and FIT objects match the selected SD baseline. It saves the
update script as reference text. It does not run that script or establish
signature acceptance, installation or hardware compatibility.

Exact firmware member extraction now checks the selected regular file and its
size before accepting output. A missing file returns nonzero even if 7zip
returns zero. This verified command extracts the board power-controller
program from the pinned 2022 ramdisk:

```sh
target/release/examples/s9_native_service extract work/simulator/original-zynq/attempt-008/components work/simulator/native-s9/pic-extract-002 lib/antminer/hash_s8_app.txt
```

Its [member manifest](work/simulator/native-s9/pic-extract-002/member.json)
records 19200 bytes with SHA-256
`ec84da07ef98665c6258cc531dc2d74aa8be7018925c041325ca233aec52b46a`.
The archived Braiins source contains the same bytes. This establishes input
identity; it does not establish PIC execution or powered hashboards.

The [console failure regression](work/simulator/qemu-s9/sd2022-script-failure-001/result.json)
records actual guest script status 23. The boot CLI now returns nonzero when
a requested console diagnostic fails, times out, or loses trace data.
A diagnostic script can still complete while the miner has failed.
Its completion does not pass firmware health or mining gates.

The [native LuCI service report](work/simulator/native-s9/luci-007/service-report.json) verifies the original web service in the diagnostic guest.
It checks login, redirects, the exact archived stylesheet, live uptime, configuration write/readback, a web service restart, and exact configuration restoration.
The client runs inside the guest. Browser JavaScript, separate-client access, mining telemetry and full-board restart persistence remain open.
Repeat this check with a new output directory. Preserve the driver before a concurrent build can replace it:

```bash
cargo build -p md-sim-firmware --example s9_native_service --offline --locked
cp -n target/debug/examples/s9_native_service work/simulator/native-s9/service-driver-007
work/simulator/native-s9/service-driver-007 shell work/simulator/original-zynq/attempt-008/components work/simulator/native-s9/luci-007 simulator/firmware/s9-luci-service-check.sh
work/simulator/native-s9/service-driver-007 report work/simulator/native-s9/luci-007 work/simulator/native-s9/luci-007/service-report.json
```

These exact commands passed. The preserved run and driver already exist; use new names to repeat them.
The shell and Lua service test uses the archived guest's own interpreter and HTTP libraries.
The [network startup script](simulator/firmware/s9-native-network-start.sh)
and storage restart scripts also use the guest's existing shell, mount and
UCI commands. UCI is the firmware's configuration interface.
Only that guest has access to these diagnostic HTTP services, and no verified ARM32 Rust or C compiler is available in this environment.
This is guest test code. The host driver and independent evidence checker are Rust.

The optional [fan-monitor adaptation](simulator/backends/qemu/FAN.md) changes
one loop in the archived Lua safety script for the two-PWM reference model.
It retains the guest's original Lua interpreter. Replacing that existing
service with Rust or C would replace the firmware under test. Rust prepares
the patch, checks exact hashes and records the separate adapted output.
Run075 passes native maintenance and fresh mining with this patch. It does
not establish original-script conformance.

The [CPU reset regression](simulator/backends/qemu/tests/cpu-reset.S) uses ARM assembly.
It tests exact reset entry points and live registers across secondary CPU clock stops.
Rust or C cannot guarantee those register conditions. A Rust driver assembles and runs this test in isolation.
This project-owned test program is separate from archived miner firmware.

The new runtime, resolver, pool, and device interface are Rust.
The [private Ethernet adapter](simulator/backends/qemu/PRIVATE-STREAM.md)
extends QEMU's C network interface. It retains the existing emulator and
packet framing. Its two-VM TCP test passes; miner integration is still
under test. The pool service and launchers are Rust.
The [TMP421 component](simulator/backends/qemu/TMP421.md) adds timed sensor
state to the Rust ASIC path. Its explicit temperature fixture has no
physical-board or calibration claim. The pinned QEMU sensor model was
reviewed, but lacks the conversion timing and fault behavior needed here.
The archived S9 firmware now passes the reference
[105°C readback test](work/simulator/qemu-s9/temperature-hot-174/temperature-audit-177.json)
and [115°C shutdown and recovery test](work/simulator/qemu-s9/temperature-dangerous-174/temperature-audit-177.json).
The verifier checks both sensor bytes, fault flags, guest queries and power
transitions. Repeated controller readback counts as one I²C transaction.
The dangerous case cuts all three chains and resumes hashing after cooling.
Eight blocks from these runs pass independent replay, Core import and restart.
For example, see the [hot-run node result](work/simulator/bitcoin-core/temperature-hot-174-audit174-session0-job0/block-acceptance.json)
and [recovered-session node result](work/simulator/bitcoin-core/temperature-dangerous-174-audit174-session11-job1/block-acceptance.json).
These tests use the recorded reference sensor fixture and monitor adaptation.
They do not establish physical calibration or an exact-board pass.
The [ADC validity fault test](work/simulator/qemu-s9/temperature-supply-invalid-manual-recovery-181/temperature-audit-182.json)
and [shorted-diode test](work/simulator/qemu-s9/temperature-diode-short-manual-recovery-182/temperature-audit-182.json)
also check native error detection and manual recovery. The original service
command restarts the miner after the input is restored. Paired sensor replies,
power cycles and new computed work establish recovery on all three chains.
Four blocks from each run pass Core import and restart. See the
[ADC recovery node result](work/simulator/bitcoin-core/temperature-supply-invalid-manual181-audit182-session10-job1/block-acceptance.json)
and [shorted-diode recovery node result](work/simulator/bitcoin-core/temperature-diode-short-manual182-audit182-session9-job1/block-acceptance.json).
This is an ADC validity fault, not loss of all sensor power. Automatic recovery
and protective shutdown are not established by this test.
The [open-diode recovery run](work/simulator/qemu-s9/temperature-diode-open-manual-recovery-185/temperature-audit-185.json)
also passes with framed restart markers and the explicit pool connection budget.
It retains all14 connections. Its two mining sessions have785 and75 independently
matched submissions. Four candidates pass Core import and restart, including the
[recovered-session block](work/simulator/bitcoin-core/temperature-diode-open-manual185-audit185-session9-job1/block-acceptance.json).
These are source185 results in the adapted reference environment.

The [stalled-fan recovery run](work/simulator/qemu-s9/fan-stall-manual-recovery-190/fan-recovery-audit190.json)
checks the native fan-fault report, initial power cuts on all three chains,
an explicit native service restart, and resumed computation on every chain.
The [recovered mining session](work/simulator/qemu-s9/fan-stall-manual-recovery-190/regtest-audit190-session13.json)
passes independent fresh-work replay with complete connection accounting.
All three completed sessions produced two Core-accepted blocks each, including
the [block after manual recovery](work/simulator/bitcoin-core/fan-stall-manual190-audit190-session13-job1/block-acceptance.json).
This is an adapted S9 reference test. Automatic recovery remains incomplete.
The later [motor power-loss run](work/simulator/qemu-s9/fan-power-manual-recovery-193/fan-recovery-audit196.json)
and [stalled-motor run](work/simulator/qemu-s9/fan-stall-manual-recovery-193/fan-recovery-audit196.json)
both pass manual recovery on all three chains. The corrected PIC parser handles
the interrupted command from the original failure. That failure remains saved.
The audit checks the replacement process's native chain initialization logs.
It also checks sensor conversion timing. Each run has one real temperature
format transition; the audit retains the native reading instead of replacing it.
All six mining sessions pass independent verification. Twelve block candidates
pass Core import and restart. Some parallel Core attempts reached a process
resource limit. Their separate serial replays pass; the failed attempts remain
failures. See the [power-loss recovery block](work/simulator/bitcoin-core/fan-power-manual193-audit193-session14-job1/block-acceptance.json).
The following audit commands were tested with their retained tools:

```bash
work/simulator/ssh-target-190/release/examples/s9_asic_audit fan-recovery work/simulator/qemu-s9/fan-stall-manual-recovery-190 work/simulator/qemu-s9/fan-stall-manual-recovery-190/fan-recovery-audit190.json
work/simulator/ssh-target-196/release/examples/s9_asic_audit fan-recovery work/simulator/qemu-s9/fan-power-manual-recovery-193 work/simulator/qemu-s9/fan-power-manual-recovery-193/fan-recovery-audit196.json
```

Choose a new report path to repeat the command. The retained inputs,
[source snapshot](work/simulator/qemu-s9/core-checkpoint-190/manifest.json),
and verifier hash identify this check. This audit does not rerun the firmware.
The [10 H/s run](work/simulator/qemu-s9/native-low-rate10-grants-201/regtest-audit202-session0.json)
uses the unchanged archived S9 miner in the adapted reference environment.
It computes 1331 hashes and submits 12 independently verified results across
two fresh jobs, a clean-job transition and a difficulty change. The ASIC's
programmed ticket mask remains active. The separate fixed-bit filter override
is 1 and is recorded in the adaptation manifest. Both block candidates pass
Core import and restart: [first block](work/simulator/bitcoin-core/lowrate-grants201-audit202-job0/block-acceptance.json)
and [second block](work/simulator/bitcoin-core/lowrate-grants201-audit202-job1/block-acceptance.json).
The [rate audit](work/simulator/qemu-s9/native-low-rate10-grants-201/rate-audit202.json)
checks all 1331 ordered grants across three chains. The shortest interval is
114,487,347 ns, above the required 100,000,000 ns. These are actual monotonic
host timestamps at hash admission. Ordinary device clocks stay separate.
The rate check fails on missing timestamps or a truncated trace.
The runner keeps the live miner until the declared pool collection completes.
This verified command requires the retained baseline and backend listed below:

```bash
work/simulator/ssh-target-201/release/examples/s9_boot_bundle boot \
  work/simulator/original-zynq/sd-bundle-006 \
  work/simulator/qemu-s9/native-low-rate10-grants-201 spl sd2022-v1 \
  backend=work/simulator/qemu-s9/backend-build-201-ubsan budget=360 pool-seconds=300 \
  presence-pin-fixture=e0 fan-fixture pic-contract asic-contract asic-packets \
  local-pool pool-vm=/tmp/md-sim-qemu-pool-003-ubsan temperature-fixture \
  pool-difficulty=-31,-30 regtest-genesis hash-cap-hps=10 adapt-fixed-filter-bits=1 \
  batch-console console=work/simulator/qemu-s9/console-startup-diagnostic-133.sh \
  adapt-monitor-fan-source=work/simulator/native-s9/original-bosminer-monitor-031/stdout.bin \
  sd-state=work/simulator/qemu-s9/automated-update-cycle-004/provision/sd-snapshot-0.json
work/simulator/ssh-target-202/release/examples/s9_asic_audit rates work/simulator/qemu-s9/native-low-rate10-grants-201 work/simulator/qemu-s9/native-low-rate10-grants-201/rate-audit202.json
```

Use a new output directory to repeat it. These development inputs are not
a clean-install release package or an exact physical target.

The [timed cooling test](work/simulator/qemu-s9/temperature-hot-184/cooling-audit-184.json)
checks actual native fan commands and tachometer reads. Both commands rise
from1% to100% after the105°C sensor reading. The connected tachometer reaches
6000RPM. Commands and measured speed fall after the65°C reading returns.
These values describe the declared motor fixture. They are not physical calibration.
The tested audit command is:

```sh
work/simulator/ssh-target-184/release/examples/s9_asic_audit cooling work/simulator/qemu-s9/temperature-hot-184 work/simulator/qemu-s9/temperature-hot-184/cooling-audit-184.json
```

Use a new report filename to repeat the audit. New pool inputs also record
their connection budget. They allow up to128 lifetime connections, including
native protocol probes and reconnects. Earlier frozen inputs keep their16
connection limit. Pool windows, share checks and the first authorized session
selection remain separate from this resource budget.
The [reconnect test](work/simulator/network-vm/pool-reconnect-188/assessment.json)
passes32 V1 connections and a separate binary probe in two disconnected Linux
VMs. It checks every invalid-submit response and all33 allocation IDs. Completed
pool threads are joined during the window so that repeated connections release
their resources. This is a transport test. It does not count as firmware mining.
The tested command is:

```sh
work/simulator/ssh-target-188/release/examples/s9_pool_link work/simulator/network-vm/pool-reconnect-188 /tmp/md-sim-qemu-pool-003-ubsan
```

Use a new output directory to repeat it. The first two test failures and their
fixes remain in the [progress record](work/simulator/continuation-2026-10-21.md).
The existing cgminer program and its necessary source changes are C.
The S3 ADC arithmetic check also retains the original Espressif C functions.
Two small C fixtures supply their SDK callbacks and Linux process interface.
Rewriting those functions in Rust would remove the independent reference.
The model, source checks, isolated runner and result verifier use Rust.
The [retained comparison](work/simulator/reviews/adc-calibration-independent-execution-509.json)
passes 131,072 samples. It does not establish physical ADC calibration.
The existing Bitcoin Core C++ implementation, upstream build tools, and Python catalogue tools retain their languages.
The independent FPGA feature checker uses a small Python adapter to call the
unchanged, pinned Project X-Ray matcher and alias APIs. Rust or C would require
replacing those Python APIs and would remove this independent reference.
Rust captures the inputs, confines execution, compares the complete results,
and writes the assessment. The checker passes the two retained feature
comparisons. It does not establish a complete FPGA model or a board boot.
The FPGA assembly (FASM) exporter also uses a thin Python adapter. It calls the unchanged,
pinned Project X-Ray disassembler and assembler, and the existing FASM parser
and formatter. Rust or C would require replacing these upstream APIs and their
independent algorithms. Rust pins the complete tool input manifest, confines
execution, and compares features and configuration bits. The retained export
matches all positive predicates and passes the full non-ECC configuration
round trip. The bounded parser invocation keeps the memory cap and rejects
multiline formatter items. Its whole-parser fixture exposed an escaped-quote
bug in the original grammar. A [one-byte grammar correction](simulator/tools/fasm/grammar-adaptation.json)
is recorded as a separate tool adaptation, with parent, patch and result hashes.
The corrected whole/chunk fixture and selected export pass. See the
[execution report](work/simulator/reviews/fasm-export-executed-482d.md).
The next converter uses a thin Python adapter for the pinned fasm2bels APIs.
Rust or C would require replacing the existing primitive and routing algorithms.
Rust owns capture, exact input checks, isolation, limits and result retention.
Four optional imports move to their existing use sites in a separate
[tool adaptation](simulator/tools/fasm2bels/adaptation.json). The adapter reuses
the checked bounded parser on the exact verified FASM file. It does not relax
missing-sink or clock checks. Actual conversion passes and retains Verilog/XDC.
See the [conversion report](work/simulator/reviews/netlist-executed-497.md).
The derived converter now preserves all 11,122 cells and the exact original
routing constraints while declaring alias wires and making open RAM inputs explicit.
Strict selected HDL compilation passes with no diagnostics. The run also
passes 582 primitive assertions and both 510-case RAM comparisons. See the
[execution report and tested commands](work/simulator/reviews/hdl-selected-executed-502l.md).
The selected logic now passes 72 real bus transactions and two reset
sequences across all three controller blocks. See the [MAXIGP0 execution
report and tested command](work/simulator/reviews/maxigp0-executed-511e.md).
The test drives the declared PS7 interface; it does not execute a CPU.
The same 72 operations and two resets now pass through the live Rust-to-HDL
link. Its clock tests check event order, clock changes and unknown signal
values. See the [live execution report and tested commands](work/simulator/reviews/live-pl-executed-512e.md).
The [transport regression and measured throughput test](work/simulator/reviews/hdl-throughput-executed-514.md)
retain a passing bus regression and a failed speed test. A10us fabric interval
took2.472s; the100us case hit its10s command limit. Full firmware execution
needs a faster backend with the required signal and event behavior.
The [isolated CVC tool build](work/simulator/reviews/cvc-tool-executed-516.md)
passes its compiler, four-state smoke and restored-tool tests. The original
CVC scheduler failed the callback-order oracle. A separate recorded kernel
adaptation now passes33 comparisons against Icarus. See the
[scheduler execution report](work/simulator/reviews/cvc-derived-rw-executed-516c.md).
The later parser repair passes its literal failure fixture and all33
scheduler comparisons on the combined tool. See the
[combined component results](work/simulator/reviews/cvc-combined-components-executed-516f.json).
Selected S9 execution and throughput remain open. The retained upstream
warnings and source-available license are recorded separately.
The CPU bridge, physical timing, original-board and exact-target gates remain open.
The exact 2022 Bosminer ELF is now extracted and its version, build, control
and baud register instructions are checked with isolated Capstone. The selected
firmware passes a 100 MHz argument to its baud calculation. This is static
evidence, not a measured clock rate. See the [input hashes, instruction evidence
and tested commands](work/simulator/reviews/bosminer-static-executed-511c.md).

The HDL checks use Verilog test fixtures as inputs to the existing Icarus
compiler and vendor models. Rust or C source cannot test that Verilog parser
or express the same four-state HDL stimulus directly. Rust controls input
capture, isolation, execution and result checks. The selected MAXIGP0 fixture
uses the same HDL event interface to drive only the PS7 bus, clock and reset
ports. It does not replace a processor. The existing upstream
Icarus and gperf source retains its original C/C++ implementation.
The PS GPIO state is Rust. Its small C binding is required by QEMU's
SysBus, Clock and qdev interfaces.
The live link uses a small C module for Icarus's existing VPI callback interface.
Rust cannot provide this interface without an ABI boundary. Rust owns the
protocol, child process, limits and result checks. Verilog supplies the actual
HDL event and four-state signal behavior.
The browser tests use small JavaScript expressions for DOM access, button
events, HTTP requests, WebSocket checks and Firefox preferences. These use the browser's native
interface. Rust and C cannot replace that interface. Rust controls the test,
checks its results, and captures the installed Firefox inputs. See the
[browser progress record](work/simulator/browser/progress.md).
The [actual Firefox component run](work/simulator/browser/run-022-report.json)
passes JavaScript execution, live service changes, two fresh profiles and
cookie isolation. The component fixture does not establish a native firmware
dashboard pass.
The Rust access controller reuses OpenSSH and Nginx for SSH, TLS and HTTP transport.
These existing implementations retain their languages. The simulator does not add a new SSH or TLS implementation.

The gateway keeps browser policy unchanged by default. Its explicit
`browser_policy: "upgrade-insecure-requests"` option adds a Content Security
Policy header that asks the browser to use TLS for insecure requests.
The endpoint manifest records this adaptation. Firmware assets stay unchanged.
The [isolated transport test](work/simulator/network-vm/gateway-policy-001/remote-report.json)
checks the actual HTTP and WebSocket response headers, client authentication,
SSH, artifact transfer, uploads and streaming. It uses a protocol fixture in
separate local worker and client environments. It does not establish browser
upgrade behavior or native Boser operation through TLS.
These commands passed; use a new output directory to repeat them:

```sh
work/simulator/network-vm/gateway-policy-driver-001/vm-driver /tmp/md-sim-gateway-policy-001 work/simulator/network-vm/gateway-policy-driver-001/scenario.json
work/simulator/network-vm/gateway-policy-driver-001/reporter report /tmp/md-sim-gateway-policy-001
```

The [access progress record](work/simulator/access-progress.md) describes the tested SSH worker path and the gateway work.
The [selected-process stream test](work/simulator/network-vm/dashboard-stream-147/remote-report.json)
also passed with the run-owned dashboard bridge. It checked 65,568 fresh request
bytes, an early response, request half-close, and a fresh guest response through
both local access and an actual SSH tunnel. It stopped a different selected
worker with a stream open and checked connection and tunnel cleanup. The runtime
checks the endpoint receipt against the complete process specification. It mounts
one private Unix socket into the process; it does not share the host network.
This test uses a transport fixture. It does not establish native dashboard or
firmware acceptance. These exact commands passed with the frozen147 drivers:

```sh
work/simulator/ssh-target-147/release/examples/isolated_network_vm work/simulator/network-vm/dashboard-stream-147 work/simulator/network-vm/dashboard-driver-147/scenario.json
work/simulator/ssh-target-147/release/examples/remote_namespace_check report work/simulator/network-vm/dashboard-stream-147
```

The output already exists. Use a new output directory to repeat the test.
The SSH tunnel used `127.0.0.1:18089` inside the separate client
environment. The fixture exchanges bytes, so that address is not a web page.
Cleanup closed the tunnel and worker listener. No service remains at host localhost.
The SSH test ran in a local Linux VM with no host network attachment. It did not use an external physical worker.

The [selected TLS transport test](work/simulator/network-vm/dashboard-stream-155/remote-report.json)
also passed. It transferred and checked the complete run manifest, then published
that worker's private Unix endpoint at `https://192.0.2.1:9443/` inside the test VM.
It checked a fresh request and guest response, client-certificate authentication,
the selected run header, client disconnect, reconnect, worker stop and cleanup.
The directory and socket permissions are explicit. The test uses separate client
and worker environments. It is a protocol fixture, not native firmware acceptance.
These exact commands passed with the [frozen source](work/simulator/qemu-s9/core-checkpoint-155/manifest.json):

```sh
work/simulator/ssh-target-155/release/examples/isolated_network_vm work/simulator/network-vm/dashboard-stream-155 work/simulator/network-vm/dashboard-driver-155/scenario.json
work/simulator/ssh-target-155/release/examples/remote_namespace_check report work/simulator/network-vm/dashboard-stream-155
```

Use a new output directory to repeat the test. Cleanup removes its listeners;
the test address is not an active dashboard on the host.

The [native remote service report](work/simulator/network-vm/native-024/remote-report.json) also passed with the actual archived S9 web services and Firefox.
The worker and client used separate network, mount, PID, IPC and UTS namespaces inside that disconnected local VM.
The test checked native login, assets, live uptime, configuration write/readback, service restart, guest reboot and exact restoration.
The same emulator process booted twice. A fresh configuration value survived the reboot, and the guest's boot ID changed.
Firefox submitted the real login form and ran the dashboard's polling script on both endpoints.
A second fresh browser profile checked cookie isolation on the same origin.
It reached the same selected service through an SSH tunnel and a gateway with client-certificate authentication.
The selected run was `70c6d7580b47a5e06f2067cf24e993ff010fd48703eaf199cd10fefb674329c6`.

These exact commands passed with the [frozen drivers and inputs](work/simulator/network-vm/native-driver-024/identities.json):

```bash
work/simulator/network-vm/native-driver-024/vm-driver /tmp/md-sim-native-network-024 work/simulator/network-vm/native-driver-024/scenario.json
work/simulator/network-vm/native-driver-024/reporter report /tmp/md-sim-native-network-024
```

The saved output already exists. Use a new output directory to repeat the run.
During the test, the separate client used `http://127.0.0.1:18081/` for its SSH tunnel and `https://192.0.2.1:8443/` for authenticated TLS.
These addresses belonged to the private VM environments. Cleanup stopped the services; no dashboard remains at the host's localhost address.
This is adapted native execution with an explicit SD storage fixture.
Original board acceptance, unclean power loss and mining remain unverified in this test.
See the [result summary](work/simulator/network-vm/native-024/summary.json),
[reboot evidence](work/simulator/network-vm/native-024/reboot.json),
[browser evidence](work/simulator/network-vm/native-024/browser-report.json),
and [execution record](work/simulator/network-vm-progress.md).

The separate [cold restart check](work/simulator/native-s9/storage-report-004/report.json)
and [warm restart check](work/simulator/native-s9/storage-warm-report-002/report.json)
passed with the archived kernel. Native UCI wrote a fresh configuration
value. Each second boot read it and restored the original file.
The cold check used two emulator processes. The warm check used a guest
reboot in the same emulator process.
A separately isolated 7z process read the saved ext2 images and compared the
restored file with the original archive member.
The disk is a new, run-owned 8 MiB fixture. This check does not establish
physical NAND behavior, unclean power loss, or persistence of the
published dashboard run.

These commands were tested. The output directories already exist.
Use new output names to repeat them.

```bash
work/simulator/native-s9/storage-cycle-driver-004/driver storage-cycle work/simulator/original-zynq/attempt-008/components work/simulator/native-s9/storage-cycle-004
work/simulator/native-s9/storage-cycle-driver-004/reporter work/simulator/native-s9/storage-cycle-004 work/simulator/native-s9/storage-report-004
work/simulator/native-s9/storage-cycle-driver-004/driver storage-warm-cycle work/simulator/original-zynq/attempt-008/components work/simulator/native-s9/storage-warm-cycle-002
work/simulator/native-s9/storage-cycle-driver-004/reporter work/simulator/native-s9/storage-warm-cycle-002 work/simulator/native-s9/storage-warm-report-002
```

The [driver inputs](work/simulator/native-s9/storage-cycle-driver-004/identities.json)
and [saved disk identities](work/simulator/native-s9/storage-warm-cycle-002/storage-images.json)
record the test state. The guest shell uses the archived UCI and mount tools.
The host driver, image export and verifier are Rust.

The credential command below was tested with OpenSSL 3.6.5. It creates a private test certificate authority, separate server/client certificates, and credential-provider JSON files:

```bash
cargo build -p md-sim-access --offline --locked
target/debug/md-sim-access credentials work/simulator/access-credentials-003 127.0.0.2 aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
```

The last value is the component test's run identity. Use the actual run hash and selected address for a real endpoint.
Use a new directory when repeating the command. Test certificates expire after one day.
Keep private keys outside exported reports. The [client credential sample](simulator/gateway-client.sample.json) contains no secrets.
The complete remote-dashboard workflow for every exact target remains unverified.

Read [the implementation checkpoint](work/simulator/progress.md) for commands, failures, open work, and the next action.
Read [the independent review](work/simulator/review.md) before using this development state.

The default simulator mode will compute valid Bitcoin work at a very low rate.
The initial aggregate cap will be 10,000 hashes per second, with lower values permitted.
The system will lower test-pool difficulty and adjust mining deadlines and expected rates.
If firmware has fixed mining thresholds, use a recorded low-rate adaptation and retain the original file.
Keep ordinary bus, network, and thermal-protection timing separate from mining timeouts.
Synthetic successful shares must not replace valid work.

## Run and test one miner locally — proposed workflow

The first executable release must add verified installation instructions here.
Select an exact target from the target list. A model name alone is insufficient.
The target must state the control board, hashboards, chips, PSU, firmware hash, execution mode, and any adaptations.

Replace `TARGET_ID` with an ID from `targets list`.
Replace `RUN_ID` with the ID returned by `run`.
The S9 name below is a catalogue example; it does not claim that S9 emulation is implemented.

```text
md-sim doctor
md-sim catalog import --path .
md-sim targets list --device bitmain-antminer-s9
md-sim targets explain TARGET_ID
md-sim prepare TARGET_ID
md-sim run TARGET_ID --name lab-miner --hashing functional --hash-cap-hps 10000 --low-rate --clock realtime --dashboard 127.0.0.1:18080
md-sim status RUN_ID
md-sim logs RUN_ID --follow
md-sim test TARGET_ID --suite smoke --out reports/simulator/smoke
md-sim test TARGET_ID --suite conformance --out reports/simulator/conformance
md-sim stop RUN_ID
```

Open the dashboard URL returned by `run`, normally `http://127.0.0.1:18080/` in this example.
Use the firmware's own login when the native dashboard is available.
The simulator must run the firmware's web server and API, as well as its page assets.
For a headless or unsupported UI, it must identify the alternative page as the simulator console.
For multiple dashboards in one browser, use separate hostnames or IP addresses, or a tested cookie-isolation method.
Different ports alone do not isolate login cookies.

Each test starts a separate instance. It must not change the interactive miner's storage.
The reports must identify original binaries, source builds, patched binaries, and adapted environments separately.
A successful API model must not be reported as execution of the original firmware.

## Run and test a miner remotely — proposed workflow

Install the same simulator release and required backends on a Linux worker.
Use an SSH account that can run that worker.
Replace `minerlab@192.0.2.10` with the worker's account and address.
The documentation address is not a configured server.

```text
md-sim workers add lab --transport ssh --address minerlab@192.0.2.10
md-sim workers check lab
md-sim doctor --worker lab
md-sim prepare TARGET_ID --worker lab
md-sim run TARGET_ID --worker lab --hashing functional --hash-cap-hps 10000 --low-rate --dashboard 127.0.0.1:18080
md-sim dashboard tunnel RUN_ID --worker lab --listen 127.0.0.1:18080
```

The remote `run` binds the dashboard to the worker's loopback address.
The tunnel exposes it on the user's computer.
Keep the tunnel open, then visit `http://127.0.0.1:18080/` on the user's computer.
The miner, hashboard models, and test pool all run on the remote worker.
The remote worker must verify the bytes of every transferred input.

From a second terminal, test the actual tunneled dashboard:

```text
md-sim dashboard test RUN_ID --worker lab --url http://127.0.0.1:18080/ --client local --miner-auth-file /path/to/client/miner-auth.json --allow-config-write --out reports/simulator/tunnel-ui
```

Replace the client credential path with the configured test credentials.
This command checks the selected running instance. `--allow-config-write` permits a reversible settings check.

To expose a dashboard on a remote IP, configure a certificate, gateway credentials, and the permitted client network on the worker.
The certificate must be valid for the address used by the browser.
The paths below refer to files on the worker. Replace all example values.

```text
md-sim dashboard publish RUN_ID --worker lab --listen 192.0.2.10:18443 --tls-cert /etc/md-sim/lab.crt --tls-key /etc/md-sim/lab.key --auth-file /etc/md-sim/dashboard-users --allow-client 192.0.2.0/24
md-sim dashboard test RUN_ID --worker lab --url https://192.0.2.10:18443/ --client local --gateway-auth-file /path/to/client/gateway-auth.json --miner-auth-file /path/to/client/miner-auth.json --ca-file /path/to/client/lab-ca.crt --allow-config-write --out reports/simulator/published-ui
md-sim test TARGET_ID --worker lab --suite conformance --out reports/simulator/remote
md-sim dashboard unpublish RUN_ID --worker lab
md-sim stop RUN_ID --worker lab
```

The example published URL is `https://192.0.2.10:18443/`.
Gateway authentication is separate from the miner's own login.
The `dashboard test` credential and CA paths are on the client. Replace them with the configured files.
The first executable release must document how to create these files.
The gateway must preserve live updates, WebSockets, cookies, uploads, and settings where the firmware uses them.
The dashboard test checks the published instance from the client's browser engine.
The target conformance test starts a fresh instance on the worker.
Remote reports are downloaded to the client's `--out` directory.
Raw worker control, debugger, and miner-management ports remain private.

## Test every hardware and firmware item — proposed workflow

Generate one frozen plan from the complete Bitcoin ASIC catalogue.
Resolve artifact-level model, controller, storage, and version constraints before running a pair.
The current device matrix lists candidates. It does not establish that each candidate can run.

```text
md-sim matrix plan --scope all-catalogued --pairing relevant --include-blocked --out plans/all.json
md-sim matrix run plans/all.json --workers local --suite conformance --jobs 2 --resume --out reports/simulator/local
md-sim workers sync lab --plan plans/all.json
md-sim matrix run plans/all.json --workers lab --suite conformance --jobs 2 --resume --out reports/simulator/remote
md-sim matrix report reports/simulator/local --format html
md-sim matrix report reports/simulator/remote --format html
```

The plan must include every hardware and firmware record, including items with no runnable target.
Use `--pairing cartesian` for an explicit classification of every requested hardware/profile and firmware pair.
Known incompatible pairs must be classified before execution.
Use `--require-complete` on `matrix run` when any blocked, unknown, or untested applicable item must make the overall command fail.

Each report must show runnable, blocked, unknown, incompatible, and not-applicable items.
Test results must distinguish pass, fail, blocked, inconclusive, and not-run.
Missing firmware, unknown board wiring, unsupported peripherals, and fixed low-rate limits need precise reasons.
They must not become silent skips or passes.

Use `smoke` for initial startup and a work vector.
Use `conformance` for required mining, management, fault, and isolation tests.
Use `update-recovery`, `calibration`, and `soak` for their separate capabilities.
Compare local and remote results only when target, firmware, adaptation, backend, scenario, and suite hashes match.
See [the test gates](docs/simulator-specification.md#19-acceptance-tests-and-gates) for exact acceptance requirements.

## Use the catalogue now

1. Open [the local catalogue](catalog/index.html) in a web browser.
2. Enter a model, board, chip, firmware, or tool name.
3. Select a record to see its sources and file links.
4. Read each relationship and its limits.
5. Open the artifact record to find the saved file and its SHA-256 value.

The browser works without a server or an Internet connection.
External source links need an Internet connection.
The browser does not load external scripts, fonts, or analytics.
Select `Link to this record` to retain a direct link to an item.

## Find the information you need

| Need | Start point |
| --- | --- |
| Search the complete main index | [Local browser](catalog/index.html) |
| Find a device | [Device index](docs/indexes/device.md) |
| Find an ASIC chip | [Chip index](docs/indexes/chip.md) |
| Find a hashboard | [Hashboard index](docs/indexes/hashboard.md) |
| Find a control board | [Control-board index](docs/indexes/control-board.md) |
| Find a power supply | [PSU index](docs/indexes/psu.md) |
| Find firmware | [Firmware index](docs/indexes/firmware.md) |
| Compare published compatibility claims | [Compatibility CSV](catalog/csv/compatibility.csv) |
| Read Bitmain and replacement-firmware evidence | [Bitmain guide](docs/bitmain.md) |
| Read MicroBT, Canaan, Innosilicon, Ebang, and other vendor evidence | [Vendor guide](docs/vendors.md) |
| Read open hardware and legacy Bitcoin ASIC evidence | [Open mining guide](docs/open-mining.md) |
| Read Auradine, Bitdeer, Proto, and ePIC evidence | [Additional vendor guide](docs/emerging-vendors.md) |
| Find BiXBiT stock, transition, and PSU files | Search `BiXBiT` in the browser; read the file role and hardware constraints |
| Find ASIC.TO and HashCore files | Search `ASIC.TO` or `HashCore`; proxy application files have a separate record |
| Find LuxOS API and fleet tools | Search `LuxOS`; Python clients and stock recovery files are separate from miner firmware |
| Implement the simulator | [Detailed specification](docs/simulator-specification.md) |
| Review simulator evidence | [Design basis](docs/simulator-design.md) |
| Define profiles and run contracts | [Version 2 requirements](docs/simulator-specification.md#5-versioned-data-contracts); [initial version 1 schema](catalog/schema/simulation-profile.schema.json) |
| Read the evidence rules | [Evidence method](docs/evidence-method.md) |
| Understand a term | [Terms](docs/terms.md) |
| Find English translations | [Translation guide](translations/README.md) |
| Review missing evidence | [Gap list](catalog/csv/gaps.csv) |
| Review failed downloads and saved alternatives | [Download failure index](reports/download-failures.csv) |
| Review coverage | [Coverage report](reports/coverage.md) |
| Compare evidence for each device | [Device matrix](reports/device-matrix.csv) |
| Check Bitcoin header byte order and hashing | [Known header fixture](fixtures/bitcoin/genesis-header.json) |
| Search document and translation text | `python3 scripts/catalog.py documents "BM1387"` |

## Read the records correctly

A device name does not identify every internal board revision.
A firmware filename does not prove compatibility.
A shared ASIC chip does not make two hashboards interchangeable.
A download date does not establish the first product release year.

Each relationship has an evidence status.
`vendor-claimed` means that a supplier lists the combination.
`source-code` means that the collected code supports the statement.
`community-reported` identifies evidence from a third party.
`inferred` identifies a conclusion that still needs confirmation.

Read `constraints`, `hardware_revision`, and `firmware_version` before you use a relationship.
No local boot or mining test is implied by a download or a source-code inspection.
Missing evidence means `unknown`.
It does not mean compatible or incompatible.

## Select a file for its intended use

| File role | Intended use | Check before use |
| --- | --- | --- |
| Miner update | Update an installed firmware system | Exact model, controller, current version, and update path |
| SD or flash recovery | Restore a boot or storage layout | Board revision, flash type, image offsets, and recovery procedure |
| Installer | Transfer or install another payload | Embedded payloads, target controller, and required network service |
| PSU firmware | Change the power-supply controller | Exact PSU model and controller revision |
| Host tool or API client | Manage or inspect a miner | Host operating system, API version, and supported device operations |
| Source snapshot | Study or rebuild an implementation | Commit, licence, submodules, toolchain, and build target |
| Protocol fixture | Test a defined message or response | Protocol version, byte order, and source device |

A supplier can republish stock firmware beside its own software.
The supplier's download address alone does not identify the firmware author.
The BiXBiT archive records this distinction.
The LuxOS collection includes API clients and stock recovery images; these do not supply a complete LuxOS miner image.
HashCore proxy container layers contain a host application. They are not miner flash images.

Use the device matrix to locate missing board, chip, PSU, date, and firmware evidence.
The matrix lists candidate files from direct claims. It does not report a local boot test.

Each release year has a `date_basis` field.
This field distinguishes a product launch, a firmware publication, a source event, and a roadmap estimate.
Unknown years remain empty.
Discovery-list dates remain unverified until a stronger source confirms them.
For some open hardware, the dated event is the first known commit for a model-specific design file.
That event is not a verified first sale or first public release.

## Use the command line

Use Python 3.10 or later.
The catalogue tools use the Python standard library.
Run these commands from this directory.

```bash
python3 scripts/catalog.py search S9 --kind device
python3 scripts/catalog.py search BM1387 --json
python3 scripts/catalog.py show bitmain-antminer-s9
python3 scripts/catalog.py links bitmain-antminer-s9
python3 scripts/catalog.py check-pair braiins-os-legacy-s9 bitmain-antminer-s9
python3 scripts/catalog.py artifacts --status downloaded --kind firmware
python3 scripts/catalog.py artifacts --status queued
python3 scripts/catalog.py members .dts --limit 20
python3 scripts/catalog.py members bm1387 --artifact braiins
python3 scripts/catalog.py documents "temperature AND sensor" --limit 10
python3 scripts/catalog.py documents "PSU" --artifact bitdeer
python3 scripts/catalog.py gaps --json
python3 scripts/catalog.py stats
```

`check-pair` reports direct claims only.
It does not infer compatibility through a device family or a shared board.
`target-label-only` means that the evidence is limited to an archive filename or directory association.

Use these commands after you add records or download files.

```bash
python3 scripts/inspect_artifacts.py
python3 scripts/report_coverage.py
python3 scripts/catalog.py build
python3 scripts/verify_archive.py
python3 scripts/report_device_matrix.py
python3 scripts/report_download_failures.py
python3 scripts/index_archive_members.py
python3 scripts/index_documents.py
python3 scripts/verify_bitcoin_fixture.py
python3 -m unittest discover -s tests -v
```

These commands validate the catalogue and reference code.
They do not boot firmware or test a virtual miner.

`build` checks references and creates the browser, CSV files, and SQLite database.
`verify_archive.py` reads each downloaded physical file and checks its SHA-256 value.
It includes the main archive and the retained supplementary files.
It checks one shared file once, even when several records link to it.
Read [the validation report](reports/archive-validation.json) for the result and scope.
This check can take time on a large archive.

Use `python3 scripts/verify_archive.py --incremental` to check only new or changed files.
It reuses a previous hash only when the path, size, modification time, change time, file identity, and expected hash agree.
The report states how many previous checks it reused.
Use the command without `--incremental` for a new read of every file.

## Use the data in another tool

The [SQLite database](catalog/catalog.sqlite) has six tables and a text-search index.
Each table preserves the complete record in its `json` column.
The CSV files are in [catalog/csv](catalog/csv).
The JSONL files are in [catalog](catalog).
One JSONL line is one record.

The separate [archive member database](catalog/archive-members.sqlite) indexes files inside saved archives.
Use the `members` command to search these paths.
Each result gives the parent artifact ID and checksum.
This index covers the outer archive. It does not replace filesystem extraction.
Read [the archive index report](reports/archive-index.json) for any truncated member lists.

The [document database](catalog/documents.sqlite) contains extracted document text and registered English text companions.
Use the `documents` command with a word, a quoted phrase, or an SQLite FTS5 expression.
Each result includes a short excerpt, the text-file path, and the original artifact checksum.
Read [the document index report](reports/document-index.json) for its scope.
Image labels and files that have no text extraction are outside this text index.

Example SQL:

```sql
SELECT e.name, r.relation, r.status, target.name, r.json
FROM relationships AS r
JOIN entities AS e ON e.id = r.from_id
JOIN entities AS target ON target.id = r.to_id
WHERE e.id = 'bitmain-antminer-s9'
   OR target.id = 'bitmain-antminer-s9';
```

Keep the record IDs when you import data.
Keep the source IDs and constraints with each compatibility claim.
Do not reduce the compatibility graph to an unrestricted yes/no table.

## Directory map

| Path | Contents |
| --- | --- |
| `artifacts/files/` | Original downloaded files, grouped by type and artifact ID |
| `artifacts/manifests/` | URLs, download status, timestamps, sizes, hashes, and errors |
| `catalog/parts/` | Research records used to build the main catalogue |
| `catalog/*.jsonl` | Combined Bitcoin ASIC records |
| `catalog/catalog.sqlite` | Searchable database |
| `catalog/csv/` | Spreadsheet and import files |
| `catalog/discovery/` | Model leads and archive paths that need verification |
| `catalog/supplementary/` | Records collected before the Bitcoin-only scope was confirmed |
| `sources/document-text/` | Text extracted from PDF documents |
| `sources/web-text/` | Visible text extracted from saved web pages |
| `fixtures/` | Fixed reference inputs for future simulator tests |
| `translations/` | English companions and translation records |
| `reports/` | Counts, validation results, file inspection, and coverage reports |
| `scripts/` | Collection, inspection, indexing, and search tools |
| `work/` | Collection scripts, logs, intermediate evidence, and download state |

The main index excludes non-Bitcoin mining devices and software used only for other algorithms.
Shared source archives can contain unrelated drivers.
Their Bitcoin ASIC role is recorded separately.

## What this collection establishes

This is a research catalogue and a local archive.
It is the evidence base for a simulator specification.
It is not a working universal miner emulator.

Public downloads do not supply every ASIC command, board circuit, boot key, FPGA design, or calibration table.
The [simulator specification](docs/simulator-specification.md) defines the required models, interfaces, and tests.
It separates API simulation, source-based simulation, process emulation, and board emulation.

The archive includes original binaries and source snapshots.
A source snapshot can lack submodules, toolchains, dependencies, or private components.
A Git LFS pointer is marked as a reference file, not a firmware payload.
A measured hash identifies the saved bytes; it does not establish authorship or a redistribution licence.

The collection has explicit access failures and unknown values.
It cannot establish that every past public file is still available.
Use the gap list and collection logs to continue the search.

## English and ASD-STE100

The guides use short sentences, direct instructions, and defined technical terms.
These rules follow the writing method in ASD-STE100 Issue 9.
The text has not received a complete approved-word audit or formal ASD certification.

Original files remain unchanged.
English translations are separate companions.
Each translation states its source and its coverage.
OCR output is evidence for review; it is not proof that every character was read correctly.

## Archive storage

Large new downloads use `/home/user/mnt/small/md-sim-archive-20261019`.
Workspace file paths point to this folder through directory links.
The registered storage roots are in [catalog/storage.json](catalog/storage.json).
Keep that drive mounted when you open linked artifacts.

When you copy this collection to another computer, copy the linked file contents too.
A copy of symbolic links alone will not include the external payloads.
Update `catalog/storage.json` after you change the storage location.

To resume files already indexed as queued, use this command:

```bash
python3 scripts/resume_downloads.py --workers 8
```

Add `--retry-failed` to retry recorded failures.
Add `--prefix ID_PREFIX` to select one collection.
Check the failure index before you retry.
For a rate limit, wait for the stated retry period or service guidance.
A retry does not bypass a login, access restriction, or deleted URL.
Run `catalog.py build` after the downloads finish.
